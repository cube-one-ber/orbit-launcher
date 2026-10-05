//! Artwork selection, bounded HTTPS downloads and an offline cache; independent of Qt.
use crate::{
    model::{Artwork, ArtworkKind, Game, MinecraftArtwork, Settings},
    providers::file_url,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

const MOJANG: &str = "https://launchercontent.mojang.com";
const PATCH_NOTES: &str = "https://launchercontent.mojang.com/v2/javaPatchNotes.json";
const IMAGE_LIMIT: usize = 10 * 1024 * 1024;
const JSON_LIMIT: usize = 4 * 1024 * 1024;
const JSON_AGE: Duration = Duration::from_secs(24 * 3600);
const RETRY_AGE: Duration = Duration::from_secs(15 * 60);

pub trait Fetcher: Send + Sync {
    fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>, String>;
}
pub struct HttpFetcher(ureq::Agent);
impl Default for HttpFetcher {
    fn default() -> Self {
        Self(
            ureq::Agent::config_builder()
                .https_only(true)
                .max_redirects(4)
                .timeout_global(Some(Duration::from_secs(6)))
                .user_agent(concat!(
                    "Orbit/",
                    env!("CARGO_PKG_VERSION"),
                    " (game launcher; artwork cache)"
                ))
                .build()
                .into(),
        )
    }
}
impl Fetcher for HttpFetcher {
    fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
        self.0
            .get(url)
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .with_config()
            .limit(limit as u64)
            .read_to_vec()
            .map_err(|e| e.to_string())
    }
}

pub struct ArtworkService<F: Fetcher> {
    fetcher: F,
    directory: PathBuf,
    online: bool,
    prefer_minecraft_updates: bool,
    cancel: Arc<AtomicBool>,
    deadline: Instant,
    requests: AtomicUsize,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}
impl<F: Fetcher> ArtworkService<F> {
    pub fn new(directory: PathBuf, online: bool, cancel: Arc<AtomicBool>, fetcher: F) -> Self {
        Self {
            fetcher,
            directory,
            online,
            prefer_minecraft_updates: true,
            cancel,
            deadline: Instant::now() + Duration::from_secs(60),
            requests: AtomicUsize::new(0),
            locks: Mutex::new(HashMap::new()),
        }
    }
    pub fn prefer_minecraft_updates(mut self, value: bool) -> Self {
        self.prefer_minecraft_updates = value;
        self
    }
    fn lock(&self, url: &str) -> Arc<Mutex<()>> {
        self.locks
            .lock()
            .unwrap()
            .entry(url.into())
            .or_default()
            .clone()
    }
    fn path(&self, url: &str, extension: &str) -> PathBuf {
        self.directory
            .join(format!("{:x}.{extension}", Sha256::digest(url.as_bytes())))
    }
    fn request(&self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
        if !self.online || self.cancel.load(Ordering::Relaxed) || Instant::now() > self.deadline {
            return Err("Artwork downloads disabled or cancelled".into());
        }
        let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
        if parsed.scheme() != "https"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err("Artwork requires an HTTPS URL without credentials".into());
        }
        if fresh(&self.path(url, "miss"), RETRY_AGE) {
            return Err("Artwork retry deferred".into());
        }
        // Bound each scan, including metadata requests and fallbacks. Cached resources cost nothing.
        if self.requests.fetch_add(1, Ordering::Relaxed) >= 48 {
            return Err("Artwork request budget reached".into());
        }
        let result = self.fetcher.get(url, limit).and_then(|bytes| {
            if bytes.len() > limit {
                Err("Artwork response is too large".into())
            } else {
                Ok(bytes)
            }
        });
        if result.is_err() {
            self.mark_failure(url);
        }
        result
    }
    fn mark_failure(&self, url: &str) {
        let _ = write_atomic(&self.path(url, "miss"), b"retry later");
    }
    fn json(&self, url: &str) -> Option<Value> {
        let lock = self.lock(url);
        let _guard = lock.lock().unwrap();
        let path = self.path(url, "json");
        let stale = fs::read(&path)
            .ok()
            .filter(|b| b.len() <= JSON_LIMIT)
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        if stale.is_some() && (!self.online || fresh(&path, JSON_AGE)) {
            return stale;
        }
        let Some(bytes) = self.request(url, JSON_LIMIT).ok() else {
            return stale;
        };
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) => {
                let _ = write_atomic(&path, &bytes);
                Some(value)
            }
            Err(_) => {
                self.mark_failure(url);
                stale
            }
        }
    }
    fn image(&self, url: &str) -> Option<String> {
        let lock = self.lock(url);
        let _guard = lock.lock().unwrap();
        let path = self.path(url, "png");
        if path.is_file()
            && image::ImageReader::open(&path)
                .ok()?
                .into_dimensions()
                .is_ok()
        {
            return Some(file_url(&path));
        }
        let bytes = self.request(url, IMAGE_LIMIT).ok()?;
        match normalize_image(&bytes) {
            Ok(png) if !self.cancel.load(Ordering::Relaxed) => {
                write_atomic(&path, &png).ok()?;
                Some(file_url(&path))
            }
            Ok(_) => None,
            Err(_) => {
                self.mark_failure(url);
                None
            }
        }
    }
    /// Explicit covers win; Minecraft drop/gallery preferences precede tiny icons.
    pub fn resolve(&self, game: &mut Game, override_path: Option<&str>) {
        if let Some(path) = override_path {
            let url = file_url(Path::new(path));
            if !url.is_empty() {
                set_art(game, url, ArtworkKind::Cover, "Custom cover");
                return;
            }
        }
        if !game.artwork.is_empty() {
            // Remote extension artwork is downloaded through the same validated cache.
            if game.artwork.starts_with("https://") {
                let url = game.artwork.clone();
                game.artwork.clear();
                if let Some(image) = self.image(&url) {
                    set_art(game, image, game.art.kind, "Provider artwork");
                    return;
                }
            } else if let Ok(url) = url::Url::parse(&game.artwork)
                && let Ok(path) = url.to_file_path()
                && path.is_file()
            {
                if game.art.source.is_empty() {
                    game.art.source = "Local artwork".into();
                }
                return;
            } else {
                game.artwork.clear();
            }
        }
        if self.prefer_minecraft_updates && self.minecraft_cover(game) {
            return;
        }
        if let Some(project) = game.art.modrinth_project.clone().filter(|p| valid_id(p))
            && let Some(metadata) =
                self.json(&format!("https://api.modrinth.com/v2/project/{project}"))
        {
            let mut gallery: Vec<_> = metadata["gallery"]
                .as_array()
                .into_iter()
                .flatten()
                .collect();
            gallery.sort_by_key(|item| {
                (
                    !item["featured"].as_bool().unwrap_or(false),
                    item["ordering"].as_i64().unwrap_or(0),
                )
            });
            for item in gallery.into_iter().take(3) {
                for field in ["raw_url", "url"] {
                    if let Some(url) = item[field].as_str()
                        && let Some(image) = self.image(url)
                    {
                        set_art(game, image, ArtworkKind::Cover, "Modrinth modpack gallery");
                        return;
                    }
                }
            }
            if game.art.icon.is_empty()
                && let Some(url) = metadata["icon_url"].as_str()
            {
                game.art.icon = url.into();
            }
        }
        if !self.prefer_minecraft_updates && self.minecraft_cover(game) {
            return;
        }
        if let Some(id) = game.art.roblox_universe.filter(|id| *id > 0) {
            let url = format!(
                "https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds={id}&countPerUniverse=1&defaults=true&size=768x432&format=Png&isCircular=false"
            );
            if let Some(metadata) = self.json(&url) {
                for entry in metadata["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|e| e["universeId"].as_u64() == Some(id))
                {
                    for thumbnail in entry["thumbnails"].as_array().into_iter().flatten() {
                        if thumbnail["state"] == "Completed"
                            && let Some(url) = thumbnail["imageUrl"]
                                .as_str()
                                .filter(|url| roblox_image_url(url))
                            && let Some(image) = self.image(url)
                        {
                            set_art(game, image, ArtworkKind::Cover, "Roblox experience artwork");
                            return;
                        }
                    }
                }
            }
            let url = format!(
                "https://thumbnails.roblox.com/v1/games/icons?universeIds={id}&size=512x512&format=Png&isCircular=false"
            );
            if let Some(metadata) = self.json(&url) {
                for thumbnail in metadata["data"].as_array().into_iter().flatten() {
                    if thumbnail["targetId"].as_u64() == Some(id)
                        && thumbnail["state"] == "Completed"
                        && let Some(url) = thumbnail["imageUrl"]
                            .as_str()
                            .filter(|url| roblox_image_url(url))
                        && let Some(image) = self.image(url)
                    {
                        set_art(game, image, ArtworkKind::Icon, "Roblox experience icon");
                        return;
                    }
                }
            }
        }
        for url in game.art.remote.clone() {
            if let Some(image) = self.image(&url) {
                set_art(
                    game,
                    image,
                    ArtworkKind::Cover,
                    &format!("{} artwork", provider_name(&game.provider)),
                );
                return;
            }
        }
        if let Some(id) = game
            .art
            .gog_product
            .clone()
            .or_else(|| {
                (game.provider == "gog")
                    .then(|| game.id.strip_prefix("gog:").map(str::to_owned))
                    .flatten()
            })
            .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
            && let Some(metadata) = self.json(&format!("https://api.gog.com/products/{id}"))
        {
            for field in ["background", "image"] {
                if let Some(url) = metadata["images"][field].as_str() {
                    let url = if url.starts_with("//") {
                        format!("https:{url}")
                    } else {
                        url.to_owned()
                    };
                    if let Some(image) = self.image(&url) {
                        set_art(game, image, ArtworkKind::Cover, "GOG artwork");
                        return;
                    }
                }
            }
            if game.art.icon.is_empty()
                && let Some(icon) = metadata["images"]["sidebarIcon2x"]
                    .as_str()
                    .or_else(|| metadata["images"]["sidebarIcon"].as_str())
            {
                game.art.icon = if icon.starts_with("//") {
                    format!("https:{icon}")
                } else {
                    icon.into()
                };
            }
        }
        // Exact public store names only: no fuzzy matching or guessing a different game's cover.
        if game.art.minecraft_version.is_none()
            && game.provider != "steam"
            && game.provider != "roblox"
            && let Some(id) = self.steam_match(&game.title)
        {
            for url in steam_urls(&id) {
                if let Some(image) = self.image(&url) {
                    set_art(game, image, ArtworkKind::Cover, "Steam store artwork");
                    return;
                }
            }
        }
        if !game.art.icon.is_empty() {
            let icon = game.art.icon.clone();
            let image = if icon.starts_with("https://") {
                self.image(&icon)
            } else {
                url::Url::parse(&icon)
                    .ok()
                    .and_then(|u| u.to_file_path().ok())
                    .filter(|p| p.is_file())
                    .map(|_| icon)
            };
            if let Some(image) = image {
                set_art(game, image, ArtworkKind::Icon, "Application icon");
            }
        }
    }
    fn minecraft_cover(&self, game: &mut Game) -> bool {
        if let Some(version) = game.art.minecraft_version.clone() {
            if let Some(drop) = minecraft_drop(&version)
                && let Some(image) = self.image(&format!("{MOJANG}{}", drop.image))
            {
                set_art(
                    game,
                    image,
                    ArtworkKind::Cover,
                    &format!("Minecraft · {}", drop.name),
                );
                return true;
            }
            // The live official index also handles older releases and future snapshots without guessing.
            if let Some(notes) = self.json(PATCH_NOTES)
                && let Some(entry) = notes["entries"].as_array().and_then(|entries| {
                    entries
                        .iter()
                        .find(|e| e["version"].as_str() == Some(&version))
                })
                && let Some(relative) = entry["image"]["url"].as_str()
                && let Ok(url) = url::Url::parse(MOJANG).and_then(|base| base.join(relative))
                && url.host_str() == Some("launchercontent.mojang.com")
                && let Some(image) = self.image(url.as_str())
            {
                set_art(
                    game,
                    image,
                    ArtworkKind::Cover,
                    &format!("Minecraft · {version}"),
                );
                return true;
            }
        }
        false
    }
    fn steam_match(&self, title: &str) -> Option<String> {
        let mut url = url::Url::parse("https://store.steampowered.com/api/storesearch/").ok()?;
        url.query_pairs_mut()
            .append_pair("term", title)
            .append_pair("l", "english")
            .append_pair("cc", "US");
        let metadata = self.json(url.as_str())?;
        exact_steam_match(&metadata, title)
    }
}

fn fresh(path: &Path, age: Duration) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|elapsed| elapsed < age)
}
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Missing cache directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn normalize_image(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|e| e.to_string())?;
    let image = if decoded.width() > 1280 || decoded.height() > 1280 {
        decoded.thumbnail(1280, 1280)
    } else {
        decoded
    };
    let mut output = Cursor::new(vec![]);
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(output.into_inner())
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
fn set_art(game: &mut Game, url: String, kind: ArtworkKind, source: &str) {
    game.artwork = url;
    game.art.kind = kind;
    game.art.source = source.into();
}
fn roblox_image_url(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| host == "rbxcdn.com" || host.ends_with(".rbxcdn.com"))
    })
}
pub fn provider_name(id: &str) -> &str {
    match id {
        "steam" => "Steam",
        "prism" => "Prism Launcher",
        "multimc" => "MultiMC",
        "polymc" => "PolyMC",
        "atlauncher" => "ATLauncher",
        "modrinth" => "Modrinth Launcher",
        "lutris" => "Lutris",
        "epic" => "Epic Games",
        "gog" => "GOG Galaxy",
        "heroic" => "Heroic Games Launcher",
        "legendary" => "Legendary",
        "nile" => "Nile (Amazon Games)",
        "roblox" => "Roblox",
        "battlenet" => "Battle.net",
        "ubisoft" => "Ubisoft Connect",
        "itch" => "itch.io",
        "desktop" => "Desktop games",
        _ => "Application",
    }
}
pub fn steam_urls(id: &str) -> Vec<String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return vec![];
    }
    ["library_hero.jpg", "header.jpg", "library_600x900.jpg"]
        .map(|name| format!("https://cdn.akamai.steamstatic.com/steam/apps/{id}/{name}"))
        .into()
}
pub fn exact_steam_match(metadata: &Value, title: &str) -> Option<String> {
    let title = normalized_title(title);
    if title.is_empty() {
        return None;
    }
    let matches: HashSet<_> = metadata["items"]
        .as_array()?
        .iter()
        .filter(|v| {
            v["name"]
                .as_str()
                .is_some_and(|s| normalized_title(s) == title)
                && v["type"].as_str().is_none_or(|t| t == "app")
        })
        .filter_map(|v| v["id"].as_u64())
        .collect();
    (matches.len() == 1).then(|| matches.iter().next().unwrap().to_string())
}
fn normalized_title(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Deserialize)]
pub struct MinecraftDrop {
    line: String,
    min: u32,
    max: u32,
    pub name: String,
    pub image: String,
}
pub fn minecraft_drop(version: &str) -> Option<MinecraftDrop> {
    let numeric = version.split('-').next()?;
    let mut parts = numeric.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    let patch = parts
        .next()
        .map(str::parse::<u32>)
        .transpose()
        .ok()?
        .unwrap_or(0);
    if parts.next().is_some() {
        return None;
    }
    let line = format!("{major}.{minor}");
    let drops: Vec<MinecraftDrop> =
        serde_json::from_str(include_str!("data/minecraft-artwork.json"))
            .expect("bundled artwork catalog");
    drops
        .into_iter()
        .find(|d| d.line == line && (d.min..=d.max).contains(&patch))
}

/// Prefer purpose-made landscape images over icons and small thumbnails.
pub fn local_cover(directory: &Path) -> String {
    crate::providers::first_art(
        [
            "cover",
            "header",
            "banner",
            "library_header",
            "library_hero",
            "background",
        ]
        .into_iter()
        .flat_map(|name| {
            ["webp", "jpg", "png", "jpeg"].map(move |ext| directory.join(format!("{name}.{ext}")))
        }),
    )
}
pub fn local_icon(directory: &Path) -> String {
    crate::providers::first_art(["icon", "logo"].into_iter().flat_map(|name| {
        ["png", "webp", "ico", "svg"].map(move |ext| directory.join(format!("{name}.{ext}")))
    }))
}
pub fn apply_cached(library: &mut crate::model::Library, settings: &Settings, directory: PathBuf) {
    let service = ArtworkService::new(
        directory,
        false,
        Arc::new(AtomicBool::new(false)),
        HttpFetcher::default(),
    )
    .prefer_minecraft_updates(settings.minecraft_artwork == MinecraftArtwork::Updates);
    for game in &mut library.games {
        service.resolve(
            game,
            settings.artwork_overrides.get(&game.id).map(String::as_str),
        );
    }
    share_covers(&mut library.games);
}
pub fn share_covers(games: &mut [Game]) {
    let covers: HashMap<_, _> = games
        .iter()
        .filter(|g| !g.artwork.is_empty() && g.art.kind == ArtworkKind::Cover)
        .map(|g| {
            (
                (
                    normalized_title(&g.title),
                    g.art.minecraft_version.clone(),
                    g.art.roblox_universe,
                    g.provider == "roblox",
                ),
                (g.artwork.clone(), g.art.source.clone()),
            )
        })
        .collect();
    for game in games {
        if (game.artwork.is_empty() || game.art.kind == ArtworkKind::Icon)
            && let Some((url, source)) = covers.get(&(
                normalized_title(&game.title),
                game.art.minecraft_version.clone(),
                game.art.roblox_universe,
                game.provider == "roblox",
            ))
        {
            set_art(game, url.clone(), ArtworkKind::Cover, source);
        }
    }
}

pub struct ArtworkUpdate {
    pub id: String,
    pub artwork: String,
    pub art: Artwork,
}
/// Up to four workers; refresh cancellation and per-pass limits keep networking bounded.
pub fn download(
    games: Vec<Game>,
    settings: Settings,
    directory: PathBuf,
    cancel: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<ArtworkUpdate>,
) {
    let service = Arc::new(
        ArtworkService::new(
            directory,
            settings.online_artwork,
            cancel.clone(),
            HttpFetcher::default(),
        )
        .prefer_minecraft_updates(settings.minecraft_artwork == MinecraftArtwork::Updates),
    );
    let queue = Mutex::new(games.into_iter());
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let service = &service;
            let queue = &queue;
            let tx = &tx;
            let settings = &settings;
            let cancel = &cancel;
            scope.spawn(move || {
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let Some(mut game) = queue.lock().unwrap().next() else {
                        break;
                    };
                    // Cached icon fallbacks are replaceable by covers during online enrichment.
                    if game.art.kind == ArtworkKind::Icon {
                        game.artwork.clear();
                    }
                    let override_path =
                        settings.artwork_overrides.get(&game.id).map(String::as_str);
                    service.resolve(&mut game, override_path);
                    if tx
                        .send(ArtworkUpdate {
                            id: game.id,
                            artwork: game.artwork,
                            art: game.art,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }
    });
}

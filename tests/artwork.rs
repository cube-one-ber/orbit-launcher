use orbit_launcher::{
    artwork::{self, ArtworkService, Fetcher},
    model::{ArtworkKind, Game},
};
use serde_json::json;
use std::{
    collections::HashMap,
    fs,
    io::Cursor,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

#[derive(Default)]
struct FakeFetcher {
    responses: HashMap<String, Vec<u8>>,
    requests: Arc<Mutex<Vec<String>>>,
}
impl Fetcher for FakeFetcher {
    fn get(&self, url: &str, _: usize) -> Result<Vec<u8>, String> {
        self.requests.lock().unwrap().push(url.into());
        self.responses
            .get(url)
            .cloned()
            .ok_or_else(|| "Fixture not found".into())
    }
}
fn game() -> Game {
    serde_json::from_value(json!({"id":"fixture:1", "title":"Fixture game", "provider":"fixture", "subtitle":"", "artwork":""})).unwrap()
}
fn png() -> Vec<u8> {
    let mut bytes = Cursor::new(vec![]);
    image::DynamicImage::new_rgb8(24, 16)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}
#[test]
fn roblox_uses_its_own_landscape_art_and_keeps_cached_art_offline() {
    let temp = tempfile::tempdir().unwrap();
    let metadata = "https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds=42&countPerUniverse=1&defaults=true&size=768x432&format=Png&isCircular=false";
    let cover = "https://tr.rbxcdn.com/fixture/768/432/Image/Png";
    let mut fetcher = FakeFetcher::default();
    fetcher.responses.insert(metadata.into(), serde_json::to_vec(&json!({"data":[{"universeId":99,"thumbnails":[{"state":"Completed","imageUrl":"https://wrong.example/art.png"}]},{"universeId":42,"thumbnails":[{"state":"Pending","imageUrl":cover},{"state":"Completed","imageUrl":cover}]}]})).unwrap());
    fetcher.responses.insert(cover.into(), png());
    let requests = fetcher.requests.clone();
    let mut entry = game();
    entry.provider = "roblox".into();
    entry.art.roblox_universe = Some(42);
    service(&temp, true, fetcher).resolve(&mut entry, None);
    assert_eq!(entry.art.source, "Roblox experience artwork");
    assert_eq!(entry.art.kind, ArtworkKind::Cover);
    assert_eq!(requests.lock().unwrap().as_slice(), [metadata, cover]);
    let mut offline = game();
    offline.provider = "roblox".into();
    offline.art.roblox_universe = Some(42);
    service(&temp, false, FakeFetcher::default()).resolve(&mut offline, None);
    assert_eq!(offline.artwork, entry.artwork);
}
#[test]
fn roblox_falls_back_to_the_exact_game_icon_without_steam_title_matching() {
    let temp = tempfile::tempdir().unwrap();
    let metadata = "https://thumbnails.roblox.com/v1/games/icons?universeIds=42&size=512x512&format=Png&isCircular=false";
    let icon = "https://tr.rbxcdn.com/fixture/512/512/Image/Png";
    let mut fetcher = FakeFetcher::default();
    fetcher.responses.insert(metadata.into(),serde_json::to_vec(&json!({"data":[{"targetId":42,"state":"Completed","imageUrl":"https://rbxcdn.com.attacker.example/icon.png"},{"targetId":99,"state":"Completed","imageUrl":icon},{"targetId":42,"state":"Completed","imageUrl":icon}]})).unwrap());
    fetcher.responses.insert(icon.into(), png());
    let requests = fetcher.requests.clone();
    let mut entry = game();
    entry.provider = "roblox".into();
    entry.art.roblox_universe = Some(42);
    service(&temp, true, fetcher).resolve(&mut entry, None);
    assert_eq!(entry.art.kind, ArtworkKind::Icon);
    assert_eq!(entry.art.source, "Roblox experience icon");
    assert!(
        !requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.contains("steampowered") || r.contains("attacker"))
    );
    let mut steam = game();
    steam.artwork = "file:///fixture-steam.png".into();
    entry.artwork.clear();
    let mut mixed = [steam, entry.clone()];
    orbit_launcher::artwork::share_covers(&mut mixed);
    assert!(mixed[1].artwork.is_empty());
    let mut games = vec![entry.clone(), entry];
    games[0].art.roblox_universe = Some(99);
    games[0].artwork = "file:///other-universe.png".into();
    games[0].art.kind = ArtworkKind::Cover;
    orbit_launcher::artwork::share_covers(&mut games);
    assert!(games[1].artwork.is_empty());
}
fn service(
    temp: &tempfile::TempDir,
    online: bool,
    fetcher: FakeFetcher,
) -> ArtworkService<FakeFetcher> {
    ArtworkService::new(
        temp.path().join("cache"),
        online,
        Arc::new(AtomicBool::new(false)),
        fetcher,
    )
}

#[test]
fn minecraft_hotfixes_match_their_drop_not_the_latest_release() {
    for (version, expected) in [
        ("1.20.1", "Trails & Tales"),
        ("1.20.6", "Armored Paws"),
        ("1.21.1", "Tricky Trials"),
        ("1.21.3", "Bundles of Bravery"),
        ("1.21.4", "The Garden Awakens"),
        ("1.21.5", "Spring to Life"),
        ("1.21.8", "Chase the Skies"),
        ("1.21.10", "The Copper Age"),
        ("1.21.11", "Mounts of Mayhem"),
        ("26.1.2", "Tiny Takeover"),
        ("26.2", "Chaos Cubed"),
        ("26.3.1", "Wilderness Bound"),
    ] {
        assert_eq!(
            artwork::minecraft_drop(version).unwrap().name,
            expected,
            "{version}"
        );
    }
    for version in ["26.4", "1.21.12", "24w03a", "unknown", "1.99.0", "26.1.0.1"] {
        assert!(
            artwork::minecraft_drop(version).is_none(),
            "Never guess artwork for {version}"
        );
    }
}

#[test]
fn custom_covers_win_without_network_requests() {
    let temp = tempfile::tempdir().unwrap();
    let cover = temp.path().join("Zoë #1 cover.png");
    fs::write(&cover, png()).unwrap();
    let fetcher = FakeFetcher::default();
    let requests = fetcher.requests.clone();
    let mut game = game();
    game.art.minecraft_version = Some("1.21.5".into());
    service(&temp, true, fetcher).resolve(&mut game, Some(cover.to_str().unwrap()));
    assert_eq!(game.art.source, "Custom cover");
    assert!(game.artwork.contains("%20%231"));
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn downloaded_images_are_normalized_cached_and_available_offline() {
    let temp = tempfile::tempdir().unwrap();
    let url = "https://artwork.example/cover.png";
    let mut fetcher = FakeFetcher::default();
    fetcher.responses.insert(url.into(), png());
    let requests = fetcher.requests.clone();
    let mut first = game();
    first.art.remote = vec![url.into()];
    let online = service(&temp, true, fetcher);
    online.resolve(&mut first, None);
    online.resolve(&mut first, None);
    assert_eq!(requests.lock().unwrap().len(), 1);
    let path = url::Url::parse(&first.artwork)
        .unwrap()
        .to_file_path()
        .unwrap();
    assert_eq!(image::image_dimensions(&path).unwrap(), (24, 16));
    let fetcher = FakeFetcher::default();
    let offline_requests = fetcher.requests.clone();
    let mut second = game();
    second.art.remote = vec![url.into()];
    service(&temp, false, fetcher).resolve(&mut second, None);
    assert_eq!(first.artwork, second.artwork);
    assert!(offline_requests.lock().unwrap().is_empty());
}

#[test]
fn malformed_images_fall_back_and_do_not_retry_every_refresh() {
    let temp = tempfile::tempdir().unwrap();
    let invalid = "https://artwork.example/broken.jpg";
    let valid = "https://artwork.example/good.png";
    let mut fetcher = FakeFetcher::default();
    fetcher
        .responses
        .insert(invalid.into(), b"<html>Not an image</html>".to_vec());
    fetcher.responses.insert(valid.into(), png());
    let requests = fetcher.requests.clone();
    let service = service(&temp, true, fetcher);
    for _ in 0..2 {
        let mut game = game();
        game.art.remote = vec![invalid.into(), valid.into()];
        service.resolve(&mut game, None);
        assert!(!game.artwork.is_empty());
    }
    assert_eq!(*requests.lock().unwrap(), [invalid, valid]);
    assert_eq!(
        fs::read_dir(temp.path().join("cache"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "png"))
            .count(),
        1
    );
}

#[test]
fn modpack_preference_uses_the_featured_gallery_before_minecraft_art() {
    let temp = tempfile::tempdir().unwrap();
    let featured = "https://cdn.modrinth.com/featured.png";
    let mut fetcher = FakeFetcher::default();
    fetcher.responses.insert("https://api.modrinth.com/v2/project/example-pack".into(), json!({
        "gallery":[{"url":"https://cdn.modrinth.com/not-featured.png", "featured":false,"ordering":0},{"url":featured, "featured":true,"ordering":5}]
    }).to_string().into_bytes());
    fetcher.responses.insert(featured.into(), png());
    let requests = fetcher.requests.clone();
    let mut game = game();
    game.provider = "modrinth".into();
    game.art.minecraft_version = Some("1.21.5".into());
    game.art.modrinth_project = Some("example-pack".into());
    service(&temp, true, fetcher)
        .prefer_minecraft_updates(false)
        .resolve(&mut game, None);
    assert_eq!(game.art.source, "Modrinth modpack gallery");
    assert_eq!(game.art.kind, ArtworkKind::Cover);
    assert_eq!(requests.lock().unwrap().len(), 2);
}

#[test]
fn minecraft_defaults_to_its_installed_drop_before_modpack_artwork() {
    let temp = tempfile::tempdir().unwrap();
    let drop = artwork::minecraft_drop("1.21.5").unwrap();
    let url = format!("https://launchercontent.mojang.com{}", drop.image);
    let mut fetcher = FakeFetcher::default();
    fetcher.responses.insert(url.clone(), png());
    let requests = fetcher.requests.clone();
    let mut game = game();
    game.art.minecraft_version = Some("1.21.5".into());
    game.art.modrinth_project = Some("example-pack".into());
    service(&temp, true, fetcher).resolve(&mut game, None);
    assert_eq!(game.art.source, "Minecraft · Spring to Life");
    assert_eq!(*requests.lock().unwrap(), [url]);
}

#[test]
fn heroic_gog_games_use_their_product_id_for_official_artwork() {
    let temp = tempfile::tempdir().unwrap();
    let url = "https://images.gog.example/background.jpg";
    let mut fetcher = FakeFetcher::default();
    fetcher.responses.insert(
        "https://api.gog.com/products/9876001".into(),
        json!({"images":{"background":url}})
            .to_string()
            .into_bytes(),
    );
    fetcher.responses.insert(url.into(), png());
    let requests = fetcher.requests.clone();
    let mut game = game();
    game.provider = "heroic".into();
    game.id = "heroic:fixture:gog:9876001".into();
    game.art.gog_product = Some("9876001".into());
    service(&temp, true, fetcher).resolve(&mut game, None);
    assert_eq!(game.art.source, "GOG artwork");
    assert_eq!(
        *requests.lock().unwrap(),
        ["https://api.gog.com/products/9876001", url]
    );
}

#[test]
fn snapshot_artwork_uses_its_exact_official_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let mut fetcher = FakeFetcher::default();
    fetcher.responses.insert(
        "https://launchercontent.mojang.com/v2/javaPatchNotes.json".into(),
        json!({"entries":[
            {"version":"26.4-snapshot-2", "image":{"url":"/v2/images/snapshot.jpg"}},
            {"version":"26.3", "image":{"url":"/v2/images/unrelated.jpg"}}
        ]})
        .to_string()
        .into_bytes(),
    );
    fetcher.responses.insert(
        "https://launchercontent.mojang.com/v2/images/snapshot.jpg".into(),
        png(),
    );
    let mut game = game();
    game.art.minecraft_version = Some("26.4-snapshot-2".into());
    service(&temp, true, fetcher).resolve(&mut game, None);
    assert_eq!(game.art.source, "Minecraft · 26.4-snapshot-2");
}

#[test]
fn store_matching_rejects_ambiguous_and_similar_titles() {
    let data = json!({"items":[{"id":1, "name":"Hades II"},{"id":2, "name":"Hades"}]});
    assert_eq!(artwork::exact_steam_match(&data, "HADES"), Some("2".into()));
    assert_eq!(artwork::exact_steam_match(&data, "Hades Deluxe"), None);
    assert_eq!(
        artwork::exact_steam_match(
            &json!({"items":[{"id":1,"name":"Hades"},{"id":2,"name":"Hades"}]}),
            "Hades"
        ),
        None
    );
}

#[test]
fn local_icons_remain_icons_and_offline_mode_never_creates_a_cache() {
    let temp = tempfile::tempdir().unwrap();
    let icon = temp.path().join("icon.png");
    fs::write(&icon, png()).unwrap();
    let mut game = game();
    game.art.icon = orbit_launcher::providers::file_url(&icon);
    let fetcher = FakeFetcher::default();
    let requests = fetcher.requests.clone();
    service(&temp, false, fetcher).resolve(&mut game, None);
    assert_eq!(game.art.kind, ArtworkKind::Icon);
    assert_eq!(game.artwork, game.art.icon);
    assert!(!temp.path().join("cache").exists());
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn matching_installed_games_share_covers_across_sources() {
    let mut first = game();
    first.title = "Hollow Knight".into();
    first.artwork = "file:///cover.png".into();
    let mut second = game();
    second.title = "Hollow Knight".into();
    second.provider = "lutris".into();
    let mut games = [first, second];
    artwork::share_covers(&mut games);
    assert_eq!(games[0].artwork, games[1].artwork);
    assert_eq!(games[1].art.kind, ArtworkKind::Cover);
}

#[test]
fn minecraft_instances_only_share_covers_when_versions_match() {
    let mut first = game();
    first.title = "Minecraft".into();
    first.artwork = "file:///spring-to-life.png".into();
    first.art.minecraft_version = Some("1.21.5".into());
    let mut second = first.clone();
    second.artwork.clear();
    second.art.minecraft_version = Some("26.3".into());
    let mut third = first.clone();
    third.artwork.clear();
    third.provider = "modrinth".into();
    let mut games = [first, second, third];
    artwork::share_covers(&mut games);
    assert!(games[1].artwork.is_empty());
    assert_eq!(games[0].artwork, games[2].artwork);
}

#[test]
fn artwork_rejects_unsafe_urls_and_oversized_images_before_caching() {
    let temp = tempfile::tempdir().unwrap();
    let oversized = "https://artwork.example/huge.png";
    let mut fetcher = FakeFetcher::default();
    fetcher
        .responses
        .insert(oversized.into(), vec![0; 10 * 1024 * 1024 + 1]);
    let requests = fetcher.requests.clone();
    let mut game = game();
    game.provider = "steam".into();
    game.art.remote = vec![
        "http://artwork.example/cover.png".into(),
        "https://user:password@artwork.example/cover.png".into(),
        oversized.into(),
    ];
    service(&temp, true, fetcher).resolve(&mut game, None);
    assert!(game.artwork.is_empty());
    assert_eq!(*requests.lock().unwrap(), [oversized]);
    assert!(
        !fs::read_dir(temp.path().join("cache"))
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.path().extension().is_some_and(|ext| ext == "png"))
    );
}

#[test]
fn cancellation_prevents_network_work() {
    let temp = tempfile::tempdir().unwrap();
    let fetcher = FakeFetcher::default();
    let requests = fetcher.requests.clone();
    let service = ArtworkService::new(
        temp.path().join("cache"),
        true,
        Arc::new(AtomicBool::new(true)),
        fetcher,
    );
    let mut game = game();
    game.art.remote = vec!["https://artwork.example/cover.png".into()];
    service.resolve(&mut game, None);
    assert!(requests.lock().unwrap().is_empty());
    assert!(!temp.path().join("cache").exists());
}

#[test]
fn legacy_game_manifests_and_artwork_preferences_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let mut settings = orbit_launcher::model::Settings {
        online_artwork: false,
        minecraft_artwork: orbit_launcher::model::MinecraftArtwork::Modpacks,
        ..Default::default()
    };
    settings
        .artwork_overrides
        .insert("prism:fixture".into(), "/pictures/cover.png".into());
    settings.custom_games.push(game());
    orbit_launcher::store::save(temp.path(), &settings).unwrap();
    let restored = orbit_launcher::store::load(temp.path()).unwrap();
    assert!(!restored.online_artwork);
    assert_eq!(restored.minecraft_artwork, settings.minecraft_artwork);
    assert_eq!(restored.artwork_overrides, settings.artwork_overrides);
    assert_eq!(restored.custom_games[0].art.kind, ArtworkKind::Cover);
}

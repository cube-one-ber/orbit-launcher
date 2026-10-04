//! Providers discover local records; they never modify launcher-owned files.
mod battlenet;
mod binary_metadata;
mod desktop;
mod epic;
mod gog;
mod heroic;
mod installed;
mod itch;
mod legendary;
mod lutris;
mod modrinth;
mod prism;
mod roblox;
mod steam;
mod steam_shortcuts;
mod ubisoft;
use crate::{model::*, store};
pub use roblox::report_signature as roblox_report_signature;
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
pub use {
    battlenet::BattleNet, desktop::Desktop, epic::Epic, gog::Gog, heroic::Heroic, itch::Itch,
    legendary::Legendary, lutris::Lutris, modrinth::Modrinth, prism::Prism, roblox::Roblox,
    steam::Steam, ubisoft::Ubisoft,
};

pub trait Provider: Send {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn available(&self) -> bool {
        true
    }
    fn note(&self) -> &str {
        ""
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>);
}
pub fn file_url(path: &Path) -> String {
    if path.is_file() {
        url::Url::from_file_path(path)
            .map(|u| u.to_string())
            .unwrap_or_default()
    } else {
        String::new()
    }
}
pub fn first_art(paths: impl IntoIterator<Item = PathBuf>) -> String {
    paths
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| file_url(&p))
        .unwrap_or_default()
}
pub fn roots(config: &SourceConfig, defaults: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    defaults
        .into_iter()
        .chain(config.paths.iter().cloned())
        .filter(|p| p.exists())
        .filter(|p| seen.insert(fs::canonicalize(p).unwrap_or_else(|_| p.clone())))
        .collect()
}
pub fn launcher(config: &SourceConfig, flatpak: bool, binary: &str, app_id: &str) -> Vec<String> {
    if !config.command.is_empty() {
        return config.command.clone();
    }
    if flatpak {
        vec!["flatpak".into(), "run".into(), app_id.into()]
    } else {
        vec![binary.into()]
    }
}
pub fn is_flatpak(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == ".var")
}

#[derive(Deserialize)]
struct Manifest {
    id: String,
    name: String,
    games: Vec<Game>,
}
struct ManifestProvider(Manifest);
impl Provider for ManifestProvider {
    fn id(&self) -> &str {
        &self.0.id
    }
    fn name(&self) -> &str {
        &self.0.name
    }
    fn discover(&self, _: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let mut games = self.0.games.clone();
        for game in &mut games {
            game.id = format!("{}:{}", self.id(), game.id);
            game.provider = self.id().into();
        }
        (games, vec![])
    }
}
pub fn discover(settings: &Settings, config_dir: &Path) -> Library {
    let mut providers: Vec<Box<dyn Provider>> = vec![
        Box::new(Steam),
        Box::new(Lutris),
        Box::new(Prism),
        Box::new(Modrinth),
        Box::new(Heroic),
        Box::new(Legendary),
        Box::new(Roblox),
        Box::new(Epic),
        Box::new(Gog),
        Box::new(BattleNet),
        Box::new(Ubisoft),
        Box::new(Itch),
        Box::new(Desktop),
    ];
    let mut library = Library::default();
    let mut provider_ids: HashSet<String> = [
        "steam",
        "lutris",
        "prism",
        "modrinth",
        "heroic",
        "legendary",
        "roblox",
        "epic",
        "gog",
        "battlenet",
        "ubisoft",
        "itch",
        "desktop",
        "custom",
    ]
    .map(String::from)
    .into();
    if let Ok(entries) = fs::read_dir(config_dir.join("providers")) {
        let mut files: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        for path in files {
            let parsed = fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str::<Manifest>(&s).map_err(|e| e.to_string()));
            match parsed {
                Ok(m)
                    if !m.id.is_empty()
                        && m.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                        && provider_ids.insert(m.id.clone()) =>
                {
                    providers.push(Box::new(ManifestProvider(m)))
                }
                other => library.providers.push(ProviderStatus {
                    id: path.display().to_string(),
                    name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                    errors: vec![match other {
                        Err(e) => e,
                        _ => {
                            "Provider ID must be unique and alphanumeric (hyphens allowed).".into()
                        }
                    }],
                    ..Default::default()
                }),
            }
        }
    }
    for provider in providers {
        let config = settings
            .sources
            .get(provider.id())
            .cloned()
            .unwrap_or_default();
        let (games, mut errors) = if config.enabled && provider.available() {
            provider.discover(&config)
        } else {
            (vec![], vec![])
        };
        if config.enabled && provider.available() {
            for path in &config.paths {
                if !path.exists() {
                    errors.push(format!("Library path does not exist: {}", path.display()));
                }
            }
        }
        library.providers.push(ProviderStatus {
            id: provider.id().into(),
            name: provider.name().into(),
            enabled: config.enabled && provider.available(),
            available: provider.available(),
            note: provider.note().into(),
            count: games.len(),
            errors,
        });
        library.games.extend(games);
    }
    library.games.extend(settings.custom_games.clone());
    let mut seen = HashSet::new();
    library.games.retain(|g| seen.insert(g.id.clone()));
    decorate(&mut library, settings);
    library.games.sort_by_key(|g| g.title.to_lowercase());
    library
}
pub fn decorate(library: &mut Library, settings: &Settings) {
    for game in &mut library.games {
        game.favorite = settings.favorites.contains(&game.id);
        game.last_played = game
            .last_played
            .max(*settings.played.get(&game.id).unwrap_or(&0));
    }
}
pub fn launch(game: &Game) -> Result<(), String> {
    if let Some(uri) = &game.launch_uri {
        return crate::platform::open_game_uri(uri);
    }
    let (program, args) = game
        .command
        .split_first()
        .ok_or("No launch command configured")?;
    if program.trim().is_empty() {
        return Err("The executable cannot be empty".into());
    }
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Console launchers should not flash a terminal over the game. Windows
        // ignores this flag for GUI executables, which retain their normal UI.
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    command
        .args(args)
        .envs(&game.environment)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dir) = &game.directory {
        command.current_dir(dir);
    }
    let mut child = command.spawn().map_err(|e| {
        format!("Could not start {program}: {e}. Check the source's command in settings.")
    })?;
    // Reap without blocking Qt. A successful dispatch does not imply a running game.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
pub fn default_data(name: &str) -> PathBuf {
    store::data_home().join(name)
}

use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GogInfo {
    game_id: String,
    name: String,
    #[serde(default)]
    root_game_id: String,
}
pub struct Gog;
impl Provider for Gog {
    fn id(&self) -> &str {
        "gog"
    }
    fn name(&self) -> &str {
        "GOG Galaxy"
    }
    fn available(&self) -> bool {
        cfg!(windows)
    }
    fn note(&self) -> &str {
        if cfg!(windows) {
            "Installed GOG games. Launches through Galaxy."
        } else {
            "GOG Galaxy integration requires Windows."
        }
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let mut defaults = crate::platform::gog_registry_paths();
        if cfg!(windows) {
            defaults.extend(
                platform
                    .program_files
                    .iter()
                    .map(|p| p.join("GOG Galaxy/Games")),
            );
        }
        let mut games = vec![];
        let mut errors = vec![];
        let mut seen = HashSet::new();
        for root in roots(config, defaults) {
            // Accept either a game folder or a library containing immediate game folders.
            let entries = match fs::read_dir(&root) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("{}: {e}", root.display()));
                    continue;
                }
            };
            let mut files = vec![];
            for entry in entries.flatten().map(|e| e.path()) {
                if entry.is_dir() {
                    if let Ok(items) = fs::read_dir(entry) {
                        files.extend(items.flatten().map(|e| e.path()));
                    }
                } else {
                    files.push(entry);
                }
            }
            for path in files.into_iter().filter(|p| {
                p.extension().is_some_and(|e| e == "info")
                    && p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("goggame-"))
            }) {
                let result = fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|b| serde_json::from_slice::<GogInfo>(&b).map_err(|e| e.to_string()));
                match result {
                    Ok(info) => {
                        if info.game_id.is_empty()
                            || !info.game_id.chars().all(|c| c.is_ascii_digit())
                            || (!info.root_game_id.is_empty() && info.root_game_id != info.game_id)
                            || !seen.insert(info.game_id.clone())
                        {
                            continue;
                        }
                        let directory = path.parent().unwrap_or(&root);
                        let mut command = if config.command.is_empty() {
                            vec![crate::platform::executable(
                                crate::platform::galaxy_registry_executable(),
                                &platform.galaxy_command(),
                            )]
                        } else {
                            config.command.clone()
                        };
                        command.extend([
                            "/command=runGame".into(),
                            format!("/gameId={}", info.game_id),
                            format!("/path={}", directory.display()),
                        ]);
                        games.push(Game {
                            id: format!("gog:{}", info.game_id),
                            title: info.name,
                            provider: "gog".into(),
                            subtitle: "GOG Galaxy".into(),
                            artwork: crate::artwork::local_cover(directory),
                            art: Artwork {
                                icon: first_art([
                                    directory.join(format!("goggame-{}.ico", info.game_id))
                                ]),
                                ..Default::default()
                            },
                            launch_notice: String::new(),
                            command,
                            environment: Default::default(),
                            launch_uri: None,
                            directory: None,
                            favorite: false,
                            last_played: 0,
                        });
                    }
                    Err(e) => errors.push(format!("{}: {e}", path.display())),
                }
            }
        }
        (games, errors)
    }
}

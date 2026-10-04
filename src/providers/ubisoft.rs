use super::*;
use prost::Message;

// uplay_install.state: title at field 22, game ID at field 28.
// Accept integer and textual wire representations without treating metadata as commands.
#[derive(Message)]
struct InstallState {
    #[prost(string, tag = "22")]
    title: String,
    #[prost(uint64, tag = "28")]
    id: u64,
}
#[derive(Message)]
struct TextInstallState {
    #[prost(string, tag = "22")]
    title: String,
    #[prost(string, tag = "28")]
    id: String,
}
pub struct Ubisoft;
impl Provider for Ubisoft {
    fn id(&self) -> &str {
        "ubisoft"
    }
    fn name(&self) -> &str {
        "Ubisoft Connect"
    }
    fn available(&self) -> bool {
        cfg!(windows)
    }
    fn note(&self) -> &str {
        if cfg!(windows) {
            "Requests the selected game directly. Connect may show login or update prompts."
        } else {
            "Ubisoft Connect integration requires Windows."
        }
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let mut records = crate::platform::ubisoft_registry_installs();
        let mut defaults: Vec<_> = records.iter().map(|r| r.path.clone()).collect();
        if cfg!(windows) {
            defaults.extend(
                platform
                    .program_files
                    .iter()
                    .map(|p| p.join("Ubisoft/Ubisoft Game Launcher/games")),
            );
        }
        let mut errors = vec![];
        for root in roots(config, defaults) {
            let mut folders = vec![root.clone()];
            if let Ok(entries) = fs::read_dir(&root) {
                folders.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
            }
            for folder in folders {
                let file = folder.join("uplay_install.state");
                if !file.is_file() {
                    continue;
                }
                let result = binary_metadata::read(&file).and_then(|bytes| {
                    InstallState::decode(bytes.as_slice())
                        .map(|s| (s.id.to_string(), s.title))
                        .or_else(|_| {
                            TextInstallState::decode(bytes.as_slice()).map(|s| (s.id, s.title))
                        })
                        .map_err(|e| e.to_string())
                });
                match result {
                    Ok((id, title)) if binary_metadata::positive_id(&id) => {
                        // Prefer the marker's real title to registry folder-name fallbacks.
                        records.insert(
                            0,
                            crate::platform::LauncherInstall {
                                id,
                                title,
                                path: folder,
                            },
                        );
                    }
                    Ok(_) => errors.push(format!("{}: Invalid Ubisoft game ID", file.display())),
                    Err(e) => errors.push(format!("{}: {e}", file.display())),
                }
            }
        }
        let mut seen = HashSet::new();
        let mut games = vec![];
        for record in records {
            if !binary_metadata::positive_id(&record.id)
                || !record.path.is_absolute()
                || !record.path.is_dir()
                || !seen.insert(record.id.clone())
            {
                continue;
            }
            let title = installed::title_or_folder(Some(&record.title), &record.path);
            let mut game = installed::game("ubisoft", &record.id, title, &record.path);
            let uri = format!("uplay://launch/{}/0", record.id);
            if config.command.is_empty() {
                game.launch_uri = Some(uri);
            } else {
                game.command = config.command.clone();
                game.command.push(uri);
            }
            games.push(game);
        }
        (games, errors)
    }
}

use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EpicManifest {
    app_name: String,
    display_name: String,
    install_location: PathBuf,
    #[serde(default)]
    main_game_app_name: String,
    #[serde(default, rename = "bIsIncompleteInstall")]
    incomplete: bool,
    #[serde(default)]
    launch_executable: String,
    #[serde(default)]
    catalog_namespace: String,
    #[serde(default)]
    catalog_item_id: String,
    #[serde(default)]
    app_categories: Vec<String>,
}
pub struct Epic;
impl Provider for Epic {
    fn id(&self) -> &str {
        "epic"
    }
    fn name(&self) -> &str {
        "Epic Games"
    }
    fn available(&self) -> bool {
        cfg!(windows)
    }
    fn note(&self) -> &str {
        if cfg!(windows) {
            "Installed games from Epic Games Launcher."
        } else {
            "Epic Games Launcher integration requires Windows."
        }
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let defaults = if cfg!(windows) {
            vec![
                platform
                    .program_data
                    .join("Epic/EpicGamesLauncher/Data/Manifests"),
            ]
        } else {
            vec![]
        };
        let mut games = vec![];
        let mut errors = vec![];
        let mut seen = HashSet::new();
        for root in roots(config, defaults) {
            let entries = match fs::read_dir(&root) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("{}: {e}", root.display()));
                    continue;
                }
            };
            for path in entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "item"))
            {
                let parsed = fs::read(&path).map_err(|e| e.to_string()).and_then(|v| {
                    serde_json::from_slice::<EpicManifest>(&v).map_err(|e| e.to_string())
                });
                match parsed {
                    Ok(m) => {
                        if m.incomplete
                            || m.app_name.is_empty()
                            || m.launch_executable.is_empty()
                            || !m.install_location.is_dir()
                            || m.app_categories.iter().any(|c| c.starts_with("plugins"))
                            || (m.app_categories.iter().any(|c| c == "addons")
                                && !m.app_categories.iter().any(|c| c == "addons/launchable"))
                            || (!m.main_game_app_name.is_empty()
                                && m.main_game_app_name != m.app_name)
                            || !seen.insert(m.app_name.clone())
                        {
                            continue;
                        }
                        let mut uri = url::Url::parse("com.epicgames.launcher://apps/")
                            .expect("constant URL");
                        let target =
                            if m.catalog_namespace.is_empty() || m.catalog_item_id.is_empty() {
                                m.app_name.clone()
                            } else {
                                format!(
                                    "{}:{}:{}",
                                    m.catalog_namespace, m.catalog_item_id, m.app_name
                                )
                            };
                        uri.path_segments_mut()
                            .expect("hierarchical URL")
                            .pop_if_empty()
                            .push(&target);
                        // Epic's full launch target is namespace:item:app, with encoded colons.
                        let encoded_path = uri.path().replace(':', "%3A");
                        uri.set_path(&encoded_path);
                        uri.query_pairs_mut()
                            .append_pair("action", "launch")
                            .append_pair("silent", "true");
                        let uri = uri.to_string();
                        let mut command = config.command.clone();
                        let launch_uri = if command.is_empty() {
                            Some(uri.clone())
                        } else {
                            command.push(uri);
                            None
                        };
                        games.push(Game {
                            id: format!("epic:{}", m.app_name),
                            title: m.display_name,
                            provider: "epic".into(),
                            subtitle: "Epic Games".into(),
                            artwork: crate::artwork::local_cover(&m.install_location),
                            art: Artwork {
                                icon: first_art([
                                    m.install_location
                                        .join(&m.launch_executable)
                                        .with_extension("ico"),
                                    m.install_location.join("icon.png"),
                                    m.install_location.join("icon.ico"),
                                ]),
                                ..Default::default()
                            },
                            launch_notice: String::new(),
                            command,
                            launch_uri,
                            environment: Default::default(),
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

use super::{installed, *};

pub struct Legendary;
impl Provider for Legendary {
    fn id(&self) -> &str {
        "legendary"
    }
    fn name(&self) -> &str {
        "Legendary"
    }
    fn note(&self) -> &str {
        "Installed Epic games start through Legendary's CLI without a launcher window. Each game retains its configuration folder. Sign in with Legendary before playing; configure its executable if it is not on PATH."
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let mut games = vec![];
        let mut errors = vec![];
        let mut seen = HashSet::new();
        for path in roots(config, platform.legendary_roots()) {
            let root = if path.is_file() {
                path.parent().unwrap_or(&path).to_path_buf()
            } else {
                path
            };
            let root = fs::canonicalize(&root).unwrap_or(root);
            if !seen.insert(root.clone()) {
                continue;
            }
            let key = url::Url::from_directory_path(&root)
                .map(|u| u.to_string())
                .unwrap_or_default();
            for installed in installed::epic_games(&root, &mut errors) {
                let mut command = if config.command.is_empty() {
                    vec![platform.legendary_command()]
                } else {
                    config.command.clone()
                };
                command.extend(["launch".into(), "--".into(), installed.id.clone()]);
                games.push(Game {
                    id: format!("legendary:{key}:{}", installed.id),
                    title: installed.title,
                    provider: self.id().into(),
                    subtitle: "Epic Games · Legendary".into(),
                    artwork: crate::artwork::local_cover(&installed.path),
                    art: installed.art,
                    launch_notice: String::new(),
                    command,
                    environment: [(
                        "LEGENDARY_CONFIG_PATH".into(),
                        root.to_string_lossy().into_owned(),
                    )]
                    .into(),
                    launch_uri: None,
                    directory: None,
                    favorite: false,
                    last_played: 0,
                });
            }
        }
        (games, errors)
    }
}

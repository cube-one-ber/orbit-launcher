use super::{installed, *};

pub struct Nile;
impl Provider for Nile {
    fn id(&self) -> &str {
        "nile"
    }
    fn name(&self) -> &str {
        "Nile (Amazon Games)"
    }
    fn available(&self) -> bool {
        cfg!(any(target_os = "linux", windows))
    }
    fn note(&self) -> &str {
        "Installed Amazon games launch through Nile's CLI. Sign in and sync with Nile first; command overrides can include Wine, prefix or wrapper options. Heroic-managed games remain in Heroic."
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let mut games = vec![];
        let mut errors = vec![];
        let mut seen = HashSet::new();
        // Validate the logical folder name before deduplicating aliases: a
        // renamed storage folder must not mask a valid symlink named nile.
        for path in platform
            .nile_roots()
            .into_iter()
            .chain(config.paths.iter().cloned())
            .filter(|path| path.exists())
        {
            let root = if path.is_file() {
                path.parent().unwrap_or(&path).to_path_buf()
            } else {
                path
            };
            let canonical = fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
            // Nile always appends "nile" to its override.
            if root.file_name().is_none_or(|name| {
                if cfg!(windows) {
                    !name.to_string_lossy().eq_ignore_ascii_case("nile")
                } else {
                    name != "nile"
                }
            }) {
                errors.push(format!(
                    "{}: Nile's data folder must be named nile; choose its nile subfolder",
                    root.display()
                ));
                continue;
            }
            if !seen.insert(canonical.clone()) {
                continue;
            }
            let key = url::Url::from_directory_path(&canonical)
                .map(|u| u.to_string())
                .unwrap_or_default();
            let mut seen_games = HashSet::new();
            for installed in installed::amazon_games(&root, &mut errors, true) {
                if !installed.path.join("fuel.json").is_file()
                    || !seen_games.insert(installed.id.clone())
                {
                    continue;
                }
                let mut command = if config.command.is_empty() {
                    vec![platform.nile_command()]
                } else {
                    config.command.clone()
                };
                // Nile's argparse launch command accepts runtime options alongside the ID.
                if !command.iter().any(|arg| arg == "launch") {
                    command.push("launch".into());
                }
                command.extend(["--".into(), installed.id.clone()]);
                let mut game = installed::game(
                    self.id(),
                    &format!("{key}:{}", installed.id),
                    installed.title,
                    &installed.path,
                );
                game.subtitle = "Amazon Games · Nile".into();
                game.art = installed.art;
                game.command = command;
                game.environment.insert(
                    "NILE_CONFIG_PATH".into(),
                    root.parent()
                        .unwrap_or(&root)
                        .to_string_lossy()
                        .into_owned(),
                );
                games.push(game);
            }
        }
        (games, errors)
    }
}

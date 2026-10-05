//! ATLauncher owns Java, authentication and modpack launch settings.
use super::{installed, *};

pub struct ATLauncher;
impl Provider for ATLauncher {
    fn id(&self) -> &str {
        "atlauncher"
    }
    fn name(&self) -> &str {
        "ATLauncher"
    }
    fn available(&self) -> bool {
        cfg!(any(target_os = "linux", windows))
    }
    fn note(&self) -> &str {
        "Play starts the selected Minecraft instance directly. Add data folders containing instances, including portable installs. ATLauncher retains accounts, Java and mod loaders; setup or error prompts may still appear."
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let mut games = vec![];
        let mut errors = vec![];
        let mut seen = HashSet::new();
        for root in roots(config, platform.atlauncher_roots()) {
            if !root.is_dir() {
                errors.push(format!(
                    "{}: Choose an ATLauncher data folder",
                    root.display()
                ));
                continue;
            }
            let root = fs::canonicalize(&root).unwrap_or(root);
            let path = root.join("instances");
            let entries = match fs::read_dir(&path) {
                Ok(entries) => entries,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    errors.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            let mut instances: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join("instance.json").is_file())
                .collect();
            instances.sort();
            for instance in instances {
                let instance = fs::canonicalize(&instance).unwrap_or(instance);
                if !seen.insert(instance.clone()) {
                    continue;
                }
                let file = instance.join("instance.json");
                let Some(metadata) = installed::read_json(&file, &mut errors) else {
                    continue;
                };
                let info = &metadata["launcher"];
                let Some(title) = info["name"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty() && !s.contains('\0'))
                else {
                    errors.push(format!("{}: Missing launcher.name", file.display()));
                    continue;
                };
                let version = metadata["id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
                let key = url::Url::from_directory_path(&instance)
                    .map(|u| u.to_string())
                    .unwrap_or_default();
                let mut command = if !config.command.is_empty() {
                    config.command.clone()
                } else if is_flatpak(&root) {
                    // The packaged shell wrapper expands $@ without quoting. Invoke
                    // its bundled Java directly so paths and names remain single args.
                    vec![
                        "flatpak".into(),
                        "run".into(),
                        "--command=java".into(),
                        "com.atlauncher.ATLauncher".into(),
                        "-jar".into(),
                        "/app/bin/ATLauncher.jar".into(),
                        "--no-launcher-update".into(),
                    ]
                } else {
                    platform.atlauncher_command(&root)
                };
                command.extend([
                    format!("--working-dir={}", root.display()),
                    format!("--launch={title}"),
                ]);
                let mut game = installed::game(self.id(), &key, title.into(), &instance);
                game.subtitle = format!(
                    "Minecraft {} · ATLauncher",
                    version.as_deref().unwrap_or("")
                );
                game.art.minecraft_version = version;
                game.art.modrinth_project =
                    info["modrinthProject"]["id"].as_str().map(str::to_owned);
                // ATLauncher keeps instance artwork separately from Minecraft files.
                if game.artwork.is_empty() {
                    game.artwork = first_art([instance.join("instance.png")]);
                }
                game.command = command;
                games.push(game);
            }
        }
        (games, errors)
    }
}

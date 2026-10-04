use super::*;
use std::collections::BTreeMap;
fn ini(path: &Path) -> Result<BTreeMap<String, String>, String> {
    Ok(fs::read_to_string(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with(['#', ';', '[']) {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.trim().into(), value.trim().into()))
        })
        .collect())
}
pub struct Prism;
impl Provider for Prism {
    fn id(&self) -> &str {
        "prism"
    }
    fn name(&self) -> &str {
        "Prism Launcher"
    }
    fn note(&self) -> &str {
        "Play starts the instance directly, skipping Prism's main window. Prism still manages accounts, Java and mod loaders."
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let roots = roots(config, platform.prism_roots());
        let mut games = vec![];
        let mut errors = vec![];
        for root in roots {
            let settings = root.join("prismlauncher.cfg");
            let cfg = if settings.exists() {
                match ini(&settings) {
                    Ok(v) => v,
                    Err(e) => {
                        errors.push(e);
                        BTreeMap::new()
                    }
                }
            } else {
                BTreeMap::new()
            };
            let path = cfg
                .get("InstanceDir")
                .map(|s| root.join(s))
                .unwrap_or_else(|| root.join("instances"));
            if !path.exists() {
                continue;
            }
            let entries = match fs::read_dir(&path) {
                Ok(e) => e,
                Err(e) => {
                    errors.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            for instance in entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join("instance.cfg").exists())
            {
                let properties = match ini(&instance.join("instance.cfg")) {
                    Ok(p) => p,
                    Err(e) => {
                        errors.push(e);
                        continue;
                    }
                };
                let id = instance
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let windows_binary = platform.prism_command(&root);
                let mut command = launcher(
                    config,
                    is_flatpak(&root),
                    if cfg!(windows) {
                        &windows_binary
                    } else {
                        "prismlauncher"
                    },
                    "org.prismlauncher.PrismLauncher",
                );
                // Prism's CLI suppresses its main window when --launch is set.
                // Do not add --show-window; account/error prompts may still be necessary.
                command.extend([
                    "--dir".into(),
                    root.to_string_lossy().into(),
                    "--launch".into(),
                    id.clone(),
                ]);
                let icon = properties.get("iconKey").cloned().unwrap_or_default();
                let icons = cfg
                    .get("IconsDir")
                    .map(|s| root.join(s))
                    .unwrap_or_else(|| root.join("icons"));
                let key = url::Url::from_file_path(&instance)
                    .map(|u| u.to_string())
                    .unwrap_or_default();
                let version = match fs::read(instance.join("mmc-pack.json")) {
                    Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                        Ok(pack) => pack["components"].as_array().and_then(|items| {
                            items
                                .iter()
                                .find(|v| v["uid"] == "net.minecraft")
                                .and_then(|v| v["version"].as_str())
                                .map(str::to_owned)
                        }),
                        Err(e) => {
                            errors
                                .push(format!("{}: {e}", instance.join("mmc-pack.json").display()));
                            None
                        }
                    },
                    Err(_) => properties.get("IntendedVersion").cloned(),
                };
                let project = properties
                    .get("ManagedPackType")
                    .filter(|v| *v == "modrinth")
                    .and_then(|_| properties.get("ManagedPackID"))
                    .cloned();
                games.push(Game {
                    id: format!("prism:{key}"),
                    title: properties.get("name").cloned().unwrap_or(id),
                    provider: "prism".into(),
                    subtitle: format!(
                        "Minecraft {} · Prism Launcher",
                        version.as_deref().unwrap_or("")
                    ),
                    artwork: crate::artwork::local_cover(&instance),
                    art: Artwork {
                        minecraft_version: version,
                        modrinth_project: project,
                        icon: first_art([
                            instance.join("icon.png"),
                            instance.join("icon.webp"),
                            icons.join(format!("{icon}.png")),
                            icons.join(format!("{icon}.webp")),
                            icons.join(format!("{icon}.svg")),
                        ]),
                        ..Default::default()
                    },
                    launch_notice: String::new(),
                    command,
                    environment: Default::default(),
                    launch_uri: None,
                    directory: None,
                    favorite: false,
                    last_played: properties
                        .get("lastLaunchTime")
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(|t| if t > 10_000_000_000 { t / 1000 } else { t })
                        .unwrap_or(0),
                });
            }
        }
        (games, errors)
    }
}

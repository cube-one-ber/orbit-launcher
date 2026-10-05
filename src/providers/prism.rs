use super::*;
use std::{collections::BTreeMap, io::Read};
fn ini(path: &Path) -> Result<BTreeMap<String, String>, String> {
    Ok(read_metadata(path)
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
/// Prism, MultiMC and PolyMC retain the same local instance format and CLI.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InstanceLauncher {
    Prism,
    MultiMC,
    PolyMC,
}
impl InstanceLauncher {
    fn id(self) -> &'static str {
        match self {
            Self::Prism => "prism",
            Self::MultiMC => "multimc",
            Self::PolyMC => "polymc",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Prism => "Prism Launcher",
            Self::MultiMC => "MultiMC",
            Self::PolyMC => "PolyMC",
        }
    }
    fn settings_file(self) -> &'static str {
        match self {
            Self::Prism => "prismlauncher.cfg",
            Self::MultiMC => "multimc.cfg",
            Self::PolyMC => "polymc.cfg",
        }
    }
    fn flatpak_id(self) -> &'static str {
        match self {
            Self::Prism => "org.prismlauncher.PrismLauncher",
            Self::PolyMC => "org.polymc.PolyMC",
            Self::MultiMC => "",
        }
    }
    fn command(self, platform: &crate::platform::Platform, root: &Path) -> String {
        match self {
            Self::Prism if cfg!(windows) => platform.prism_command(root),
            Self::Prism => "prismlauncher".into(),
            Self::MultiMC => platform.multimc_command(root),
            Self::PolyMC => platform.polymc_command(root),
        }
    }
}
macro_rules! instance_provider {
    ($provider:ident, $note:literal) => {
        pub struct $provider;
        impl Provider for $provider {
            fn id(&self) -> &str {
                InstanceLauncher::$provider.id()
            }
            fn name(&self) -> &str {
                InstanceLauncher::$provider.name()
            }
            fn available(&self) -> bool {
                InstanceLauncher::$provider == InstanceLauncher::Prism
                    || cfg!(any(target_os = "linux", windows))
            }
            fn note(&self) -> &str {
                $note
            }
            fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
                discover_instances(config, InstanceLauncher::$provider)
            }
        }
    };
}
instance_provider!(
    Prism,
    "Play starts the instance directly, skipping Prism's main window. Prism still manages accounts, Java and mod loaders."
);
instance_provider!(
    MultiMC,
    "Play starts the instance directly through MultiMC. Add portable data folders containing multimc.cfg; MultiMC retains accounts, Java and mod loaders."
);
instance_provider!(
    PolyMC,
    "Play starts the instance directly through PolyMC, including Flatpak installations. PolyMC retains accounts, Java and mod loaders; its console and post-game settings still apply."
);

fn read_metadata(path: &Path) -> Result<String, std::io::Error> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(std::io::Error::other("Instance metadata exceeds 1 MiB"));
    }
    String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
fn discover_instances(config: &SourceConfig, kind: InstanceLauncher) -> (Vec<Game>, Vec<String>) {
    let platform = crate::platform::Platform::current();
    let defaults = match kind {
        InstanceLauncher::Prism => platform.prism_roots(),
        InstanceLauncher::MultiMC => platform.multimc_roots(),
        InstanceLauncher::PolyMC => platform.polymc_roots(),
    };
    let roots = roots(config, defaults);
    let mut seen = HashSet::new();
    let mut games = vec![];
    let mut errors = vec![];
    for root in roots {
        if !root.is_dir() {
            errors.push(format!(
                "{}: Choose a {} data folder",
                root.display(),
                kind.name()
            ));
            continue;
        }
        let settings = root.join(kind.settings_file());
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
            if !seen.insert(fs::canonicalize(&instance).unwrap_or_else(|_| instance.clone())) {
                continue;
            }
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
            let mut command = if !config.command.is_empty() {
                config.command.clone()
            } else if is_flatpak(&root) && kind != InstanceLauncher::MultiMC {
                vec!["flatpak".into(), "run".into(), kind.flatpak_id().into()]
            } else {
                vec![kind.command(&platform, &root)]
            };
            // These launchers support direct instance requests without --show.
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
            let version = match read_metadata(&instance.join("mmc-pack.json")) {
                Ok(bytes) => match serde_json::from_str::<serde_json::Value>(&bytes) {
                    Ok(pack) => pack["components"].as_array().and_then(|items| {
                        items
                            .iter()
                            .find(|v| v["uid"] == "net.minecraft")
                            .and_then(|v| v["version"].as_str())
                            .map(str::to_owned)
                    }),
                    Err(e) => {
                        errors.push(format!("{}: {e}", instance.join("mmc-pack.json").display()));
                        None
                    }
                },
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        errors.push(format!("{}: {e}", instance.join("mmc-pack.json").display()));
                    }
                    properties.get("IntendedVersion").cloned()
                }
            };
            let project = properties
                .get("ManagedPackType")
                .filter(|v| *v == "modrinth")
                .and_then(|_| properties.get("ManagedPackID"))
                .cloned();
            games.push(Game {
                id: format!("{}:{key}", kind.id()),
                title: properties.get("name").cloned().unwrap_or(id),
                provider: kind.id().into(),
                subtitle: format!(
                    "Minecraft {} · {}",
                    version.as_deref().unwrap_or(""),
                    kind.name()
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
                source_rank: 0,
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

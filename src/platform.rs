//! OS integration kept separate from parsers so Windows layouts are testable on Linux.
use std::{
    env,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
}
#[derive(Clone, Debug)]
pub struct Platform {
    pub os: Os,
    pub home: PathBuf,
    pub roaming: PathBuf,
    pub local: PathBuf,
    pub config_home: PathBuf,
    pub program_data: PathBuf,
    pub program_files: Vec<PathBuf>,
    pub steam_registry: Vec<PathBuf>,
}
#[derive(Clone, Debug)]
pub struct LauncherInstall {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
}
impl Platform {
    pub fn current() -> Self {
        let os = if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        };
        let mut platform = Self::from_environment(os, |key| env::var_os(key).map(PathBuf::from));
        platform.steam_registry = steam_registry_paths();
        platform
    }
    pub fn from_environment(os: Os, get: impl Fn(&str) -> Option<PathBuf>) -> Self {
        let home = if os == Os::Windows {
            get("USERPROFILE").or_else(|| get("HOME"))
        } else {
            get("HOME")
        }
        .unwrap_or_else(|| PathBuf::from("."));
        Self {
            os,
            roaming: get("APPDATA").unwrap_or_else(|| home.join("AppData/Roaming")),
            local: get("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData/Local")),
            config_home: get("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
            program_data: get("PROGRAMDATA").unwrap_or_else(|| PathBuf::from("C:/ProgramData")),
            program_files: vec![
                get("ProgramFiles(x86)").unwrap_or_else(|| PathBuf::from("C:/Program Files (x86)")),
                get("ProgramFiles").unwrap_or_else(|| PathBuf::from("C:/Program Files")),
            ],
            home,
            steam_registry: vec![],
        }
    }
    pub fn steam_roots(&self) -> Vec<PathBuf> {
        if self.os == Os::Windows {
            self.steam_registry
                .iter()
                .cloned()
                .chain(self.program_files.iter().map(|p| p.join("Steam")))
                .collect()
        } else {
            vec![
                crate::store::data_home().join("Steam"),
                self.home.join(".steam/steam"),
                self.home.join(".steam/root"),
                self.home
                    .join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            ]
        }
    }
    pub fn downloads_dir(&self) -> PathBuf {
        if self.os == Os::Linux
            && let Ok(text) = std::fs::read_to_string(self.config_home.join("user-dirs.dirs"))
        {
            for line in text.lines() {
                if let Some(value) = line
                    .trim()
                    .strip_prefix("XDG_DOWNLOAD_DIR=\"")
                    .and_then(|v| v.strip_suffix('"'))
                {
                    let path = if let Some(relative) = value.strip_prefix("$HOME/") {
                        self.home.join(relative)
                    } else {
                        PathBuf::from(value)
                    };
                    if path.is_absolute() {
                        return path;
                    }
                }
            }
        }
        self.home.join("Downloads")
    }
    pub fn prism_roots(&self) -> Vec<PathBuf> {
        if self.os == Os::Windows {
            let mut roots = vec![self.roaming.join("PrismLauncher")];
            roots.extend(self.prism_executables().into_iter().filter_map(|p| {
                let dir = p.parent()?;
                dir.join("portable.txt").exists().then(|| dir.to_path_buf())
            }));
            roots
        } else {
            vec![
                crate::store::data_home().join("PrismLauncher"),
                self.home
                    .join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher"),
            ]
        }
    }
    pub fn modrinth_roots(&self) -> Vec<PathBuf> {
        let data = if self.os == Os::Windows {
            self.roaming.clone()
        } else {
            crate::store::data_home()
        };
        let mut roots: Vec<_> = ["ModrinthApp", "com.modrinth.theseus"]
            .into_iter()
            .map(|name| data.join(name))
            .collect();
        if self.os == Os::Linux {
            for app in ["com.modrinth.ModrinthApp", "com.modrinth.theseus"] {
                roots.extend(
                    ["ModrinthApp", "com.modrinth.theseus"]
                        .map(|name| self.home.join(".var/app").join(app).join("data").join(name)),
                );
            }
        }
        if let Some(dir) = env::var_os("THESEUS_CONFIG_DIR") {
            roots.insert(0, dir.into());
        }
        roots
    }
    pub fn modrinth_command(&self) -> String {
        if self.os == Os::Linux {
            return "modrinth-app".into();
        }
        executable(
            ["Modrinth App", "ModrinthApp", "Programs/Modrinth App"]
                .into_iter()
                .map(|dir| self.local.join(dir).join("Modrinth App.exe"))
                .chain(
                    self.program_files
                        .iter()
                        .map(|dir| dir.join("Modrinth App/Modrinth App.exe")),
                )
                .chain(find_on_path("Modrinth App.exe")),
            "Modrinth App.exe",
        )
    }
    pub fn heroic_roots(&self) -> Vec<PathBuf> {
        if self.os == Os::Windows {
            vec![self.roaming.join("heroic")]
        } else {
            vec![
                self.config_home.join("heroic"),
                self.home
                    .join(".var/app/com.heroicgameslauncher.hgl/config/heroic"),
            ]
        }
    }
    pub fn legendary_roots(&self) -> Vec<PathBuf> {
        // Legendary uses ~/.config even on Windows; APPDATA is not its default.
        let mut roots = vec![self.config_home.join("legendary")];
        if let Some(path) = env::var_os("LEGENDARY_CONFIG_PATH") {
            roots.insert(0, path.into());
        }
        roots
    }
    pub fn heroic_command(&self) -> String {
        if self.os == Os::Linux {
            return "heroic".into();
        }
        executable(
            [
                "Programs/heroic",
                "Programs/Heroic",
                "Programs/Heroic Games Launcher",
            ]
            .into_iter()
            .map(|folder| self.local.join(folder).join("Heroic.exe"))
            .chain(
                self.program_files
                    .iter()
                    .map(|folder| folder.join("Heroic/Heroic.exe")),
            )
            .chain(find_on_path("Heroic.exe")),
            "Heroic.exe",
        )
    }
    pub fn legendary_command(&self) -> String {
        let binary = if self.os == Os::Windows {
            "legendary.exe"
        } else {
            "legendary"
        };
        executable(find_on_path(binary), binary)
    }
    pub fn prism_executables(&self) -> Vec<PathBuf> {
        std::iter::once(self.local.join("Programs/PrismLauncher/prismlauncher.exe"))
            .chain(
                self.program_files
                    .iter()
                    .map(|p| p.join("PrismLauncher/prismlauncher.exe")),
            )
            .chain(find_on_path("prismlauncher.exe"))
            .collect()
    }
    pub fn steam_command(&self, root: &Path) -> String {
        executable(
            std::iter::once(root.join("steam.exe"))
                .chain(self.steam_roots().iter().map(|p| p.join("steam.exe"))),
            "steam.exe",
        )
    }
    pub fn prism_command(&self, root: &Path) -> String {
        executable(
            std::iter::once(root.join("prismlauncher.exe")).chain(self.prism_executables()),
            "prismlauncher.exe",
        )
    }
    pub fn galaxy_command(&self) -> String {
        executable(
            self.program_files
                .iter()
                .map(|p| p.join("GOG Galaxy/GalaxyClient.exe")),
            "GalaxyClient.exe",
        )
    }
    pub fn itch_roots(&self) -> Vec<PathBuf> {
        let base = if self.os == Os::Windows {
            &self.roaming
        } else {
            &self.config_home
        };
        ["itch", "kitch"].map(|app| base.join(app)).into()
    }
    pub fn itch_command(&self, app: &str) -> String {
        let binary = if self.os == Os::Windows {
            "itch-setup.exe"
        } else {
            "itch-setup"
        };
        let path = if self.os == Os::Windows {
            self.local.join(app).join(binary)
        } else {
            self.home.join(format!(".{app}")).join(binary)
        };
        executable(std::iter::once(path).chain(find_on_path(binary)), binary)
    }
    pub fn battlenet_command(&self) -> String {
        executable(
            self.program_files
                .iter()
                .map(|p| p.join("Battle.net/Battle.net.exe"))
                .chain(battlenet_registry_client())
                .chain(find_on_path("Battle.net.exe")),
            "Battle.net.exe",
        )
    }
}
pub fn executable(paths: impl IntoIterator<Item = PathBuf>, fallback: &str) -> String {
    paths
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| fallback.into())
}
fn find_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|p| p.join(name))
            .find(|p| p.is_file())
    })
}
#[cfg(windows)]
fn steam_registry_paths() -> Vec<PathBuf> {
    use winreg::{RegKey, enums::*};
    let mut paths = vec![];
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            if let Ok(key) = RegKey::predef(hive)
                .open_subkey_with_flags(r"SOFTWARE\Valve\Steam", KEY_READ | view)
            {
                paths.extend(steam_paths_from_key(&key));
            }
        }
    }
    paths
}
#[cfg(windows)]
fn steam_paths_from_key(key: &winreg::RegKey) -> Vec<PathBuf> {
    ["SteamPath", "InstallPath"]
        .into_iter()
        .filter_map(|name| key.get_value::<String, _>(name).ok().map(PathBuf::from))
        .collect()
}
#[cfg(not(windows))]
fn steam_registry_paths() -> Vec<PathBuf> {
    vec![]
}

/// Only recognized game-launch protocols are dispatched. No command interpreter is involved.
pub fn validate_game_uri(uri: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(uri).map_err(|e| e.to_string())?;
    let modrinth = parsed.scheme() == "modrinth"
        && parsed.host_str() == Some("launch")
        && parsed.path_segments().is_some_and(|mut parts| {
            parts.next() == Some("instance")
                && parts.next().is_some_and(|id| !id.is_empty())
                && parts.next().is_none()
        });
    let roblox = parsed.scheme() == "roblox"
        && parsed.host_str() == Some("experiences")
        && parsed.path() == "/start"
        && {
            let mut query = parsed.query_pairs();
            query.next().is_some_and(|(key, id)| {
                key == "placeId"
                    && !id.is_empty()
                    && id.bytes().all(|b| b.is_ascii_digit())
                    && id.parse::<u64>().is_ok_and(|n| n > 0)
            }) && query.next().is_none()
        };
    let ubisoft = parsed.scheme() == "uplay"
        && parsed.host_str() == Some("launch")
        && parsed.query().is_none()
        && parsed.path_segments().is_some_and(|mut parts| {
            parts.next().is_some_and(|id| {
                !id.is_empty()
                    && id.bytes().all(|b| b.is_ascii_digit())
                    && id.parse::<u64>().is_ok_and(|n| n > 0)
            }) && parts.next() == Some("0")
                && parts.next().is_none()
        });
    if (parsed.scheme() != "com.epicgames.launcher" && !modrinth && !roblox && !ubisoft)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.fragment().is_some()
        || uri.contains('\0')
    {
        return Err("Unsupported game launch protocol".into());
    }
    Ok(parsed)
}
pub fn open_game_uri(uri: &str) -> Result<(), String> {
    let parsed = validate_game_uri(uri)?;
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let wide: Vec<u16> = uri.encode_utf16().chain(Some(0)).collect();
        // SAFETY: wide is NUL-terminated and remains alive for the synchronous call;
        // all optional pointers are null, and no shell command string is constructed.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        } as isize;
        if result <= 32 {
            let launcher = match parsed.scheme() {
                "modrinth" => "Modrinth Launcher",
                "roblox" => "Roblox",
                "uplay" => "Ubisoft Connect",
                _ => "Epic Games Launcher",
            };
            Err(format!(
                "Windows could not open {launcher} (error {result}). Install it or configure its executable in Sources."
            ))
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        if !["modrinth", "roblox"].contains(&parsed.scheme()) {
            return Err("This game requires its Windows launcher.".into());
        }
        let handler = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let mut child = std::process::Command::new(handler)
            .arg(uri)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| {
                format!("Could not open {}: {e}. Install a protocol handler or configure its command in Sources.", if parsed.scheme() == "roblox" { "Roblox" } else { "Modrinth Launcher" })
            })?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

#[cfg(windows)]
pub fn gog_registry_paths() -> Vec<PathBuf> {
    use winreg::{RegKey, enums::*};
    let mut paths = vec![];
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            if let Ok(games) = RegKey::predef(hive)
                .open_subkey_with_flags(r"SOFTWARE\GOG.com\Games", KEY_READ | view)
            {
                for name in games.enum_keys().flatten() {
                    if let Ok(game) = games.open_subkey_with_flags(name, KEY_READ | view)
                        && let Ok(path) = game.get_value::<String, _>("path")
                    {
                        paths.push(PathBuf::from(path));
                    }
                }
            }
        }
    }
    paths
}
#[cfg(not(windows))]
pub fn gog_registry_paths() -> Vec<PathBuf> {
    vec![]
}

#[cfg(windows)]
pub fn galaxy_registry_executable() -> Option<PathBuf> {
    use winreg::{RegKey, enums::*};
    [KEY_WOW64_32KEY, KEY_WOW64_64KEY]
        .into_iter()
        .find_map(|view| {
            let key = RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(r"SOFTWARE\GOG.com\GalaxyClient\paths", KEY_READ | view)
                .ok()?;
            Some(PathBuf::from(key.get_value::<String, _>("client").ok()?).join("GalaxyClient.exe"))
        })
}
#[cfg(not(windows))]
pub fn galaxy_registry_executable() -> Option<PathBuf> {
    None
}

#[cfg(windows)]
pub fn ubisoft_registry_installs() -> Vec<LauncherInstall> {
    use winreg::{RegKey, enums::*};
    let mut records = vec![];
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            if let Ok(key) = RegKey::predef(hive)
                .open_subkey_with_flags(r"SOFTWARE\Ubisoft\Launcher\Installs", KEY_READ | view)
            {
                records.extend(ubisoft_installs_from_key(&key));
            }
        }
    }
    records
}
#[cfg(windows)]
fn ubisoft_installs_from_key(key: &winreg::RegKey) -> Vec<LauncherInstall> {
    key.enum_keys()
        .flatten()
        .filter_map(|id| {
            let game = key.open_subkey(&id).ok()?;
            let path: String = game.get_value("InstallDir").ok()?;
            Some(LauncherInstall {
                id,
                title: game.get_value("DisplayName").unwrap_or_default(),
                path: PathBuf::from(path),
            })
        })
        .collect()
}
#[cfg(not(windows))]
pub fn ubisoft_registry_installs() -> Vec<LauncherInstall> {
    vec![]
}

#[cfg(windows)]
#[derive(Default)]
struct UninstallEntry {
    title: String,
    path: PathBuf,
    uninstall: String,
}
#[cfg(windows)]
fn uninstall_entries() -> Vec<UninstallEntry> {
    use winreg::{RegKey, enums::*};
    let mut records = vec![];
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            if let Ok(key) = RegKey::predef(hive).open_subkey_with_flags(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
                KEY_READ | view,
            ) {
                records.extend(uninstall_entries_from_key(&key));
            }
        }
    }
    records
}
#[cfg(windows)]
fn uninstall_entries_from_key(key: &winreg::RegKey) -> Vec<UninstallEntry> {
    key.enum_keys()
        .flatten()
        .filter_map(|id| {
            let game = key.open_subkey(id).ok()?;
            Some(UninstallEntry {
                title: game.get_value("DisplayName").unwrap_or_default(),
                path: PathBuf::from(game.get_value::<String, _>("InstallLocation").ok()?),
                uninstall: game.get_value("UninstallString").unwrap_or_default(),
            })
        })
        .collect()
}
#[cfg(windows)]
pub fn battlenet_registry_installs() -> Vec<LauncherInstall> {
    battlenet_installs_from_entries(uninstall_entries())
}
#[cfg(windows)]
fn battlenet_installs_from_entries(entries: Vec<UninstallEntry>) -> Vec<LauncherInstall> {
    entries
        .into_iter()
        .filter_map(|entry| {
            if !entry.uninstall.to_ascii_lowercase().contains("battle.net")
                || entry.title.ends_with("Test")
                || entry.title.ends_with("Beta")
            {
                return None;
            }
            // Inspect uninstall metadata only. Never execute the uninstall command.
            let id = entry
                .uninstall
                .split("--uid=")
                .nth(1)?
                .split_whitespace()
                .next()?
                .trim_matches('"');
            Some(LauncherInstall {
                id: id.into(),
                title: entry.title,
                path: entry.path,
            })
        })
        .collect()
}
#[cfg(not(windows))]
pub fn battlenet_registry_installs() -> Vec<LauncherInstall> {
    vec![]
}
#[cfg(windows)]
fn battlenet_registry_client() -> Option<PathBuf> {
    uninstall_entries()
        .into_iter()
        .find(|r| r.title == "Battle.net")
        .map(|r| r.path.join("Battle.net.exe"))
}
#[cfg(not(windows))]
fn battlenet_registry_client() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_uses_roaming_and_profile_folders() {
        let platform = Platform::from_environment(Os::Windows, |name| match name {
            "USERPROFILE" => Some(PathBuf::from("C:/Users/Zoë Example")),
            "APPDATA" => Some(PathBuf::from("D:/Roaming Profile")),
            _ => None,
        });
        assert_eq!(platform.home, PathBuf::from("C:/Users/Zoë Example"));
        assert_eq!(
            platform.prism_roots()[0],
            PathBuf::from("D:/Roaming Profile/PrismLauncher")
        );
        assert!(
            platform
                .modrinth_roots()
                .contains(&PathBuf::from("D:/Roaming Profile/ModrinthApp"))
        );
        assert!(
            platform
                .modrinth_roots()
                .contains(&PathBuf::from("D:/Roaming Profile/com.modrinth.theseus"))
        );
        assert!(
            platform
                .steam_roots()
                .contains(&PathBuf::from("C:/Program Files (x86)/Steam"))
        );
    }
    #[test]
    fn windows_executables_do_not_require_path() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("steam.exe"), []).unwrap();
        std::fs::write(temp.path().join("prismlauncher.exe"), []).unwrap();
        let platform = Platform::from_environment(Os::Windows, |_| None);
        assert_eq!(
            platform.steam_command(temp.path()),
            temp.path().join("steam.exe").to_string_lossy()
        );
        assert_eq!(
            platform.prism_command(temp.path()),
            temp.path().join("prismlauncher.exe").to_string_lossy()
        );
    }

    #[cfg(windows)]
    #[test]
    fn steam_registry_values_preserve_unicode_paths() {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};
        let hive = RegKey::predef(HKEY_CURRENT_USER);
        let name = format!(
            r"Software\OrbitTests\{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let (key, _) = hive.create_subkey(&name).unwrap();
        key.set_value("SteamPath", &r"D:\Games\Zoë Steam").unwrap();
        key.set_value("InstallPath", &r"\\server\share\Steam")
            .unwrap();
        let paths = steam_paths_from_key(&key);
        drop(key);
        // Remove test-owned data before asserting; launcher-owned keys are never touched.
        hive.delete_subkey_all(&name).unwrap();
        assert_eq!(
            paths,
            [
                PathBuf::from(r"D:\Games\Zoë Steam"),
                PathBuf::from(r"\\server\share\Steam")
            ]
        );
    }
}

#[cfg(all(test, windows))]
mod common_launcher_registry_tests {
    use super::*;
    #[test]
    fn installed_registry_records_preserve_paths_without_executing_uninstall_commands() {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};
        let hive = RegKey::predef(HKEY_CURRENT_USER);
        let name = format!(
            r"Software\OrbitTests\common-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let (key, _) = hive.create_subkey(&name).unwrap();
        let (ubisoft, _) = key.create_subkey("Ubisoft").unwrap();
        let (game, _) = ubisoft.create_subkey("12345").unwrap();
        game.set_value("InstallDir", &r"D:\Games\Zoë game").unwrap();
        drop(game);
        let uplay = ubisoft_installs_from_key(&ubisoft);
        let (uninstall, _) = key.create_subkey("Uninstall").unwrap();
        for (id, title) in [
            ("healthy", "Diablo fixture"),
            ("beta", "Diablo fixture Beta"),
        ] {
            let (game, _) = uninstall.create_subkey(id).unwrap();
            game.set_value("InstallLocation", &r"\\server\share\Diablo")
                .unwrap();
            game.set_value("DisplayName", &title).unwrap();
            game.set_value(
                "UninstallString",
                &r#""C:\Apps\Battle.net\Battle.net.exe" --uid=diablo3 --uninstall"#,
            )
            .unwrap();
        }
        let battle = battlenet_installs_from_entries(uninstall_entries_from_key(&uninstall));
        drop(uninstall);
        drop(ubisoft);
        drop(key);
        hive.delete_subkey_all(&name).unwrap();
        assert_eq!(uplay.len(), 1);
        assert_eq!(uplay[0].id, "12345");
        assert_eq!(uplay[0].path, PathBuf::from(r"D:\Games\Zoë game"));
        assert_eq!(battle.len(), 1);
        assert_eq!(battle[0].id, "diablo3");
        assert_eq!(battle[0].path, PathBuf::from(r"\\server\share\Diablo"));
    }
}

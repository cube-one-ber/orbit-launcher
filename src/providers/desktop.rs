//! Standalone Linux games from desktop entries. GIO owns launch semantics,
//! including Exec field codes, quoting, Flatpak arguments and working folders.
use super::*;
use std::{collections::BTreeMap, env, io::Read};

pub struct Desktop;
impl Provider for Desktop {
    fn id(&self) -> &str {
        "desktop"
    }
    fn name(&self) -> &str {
        "Desktop games"
    }
    fn available(&self) -> bool {
        cfg!(target_os = "linux")
    }
    fn note(&self) -> &str {
        if self.available() {
            "Standalone games and emulators from Linux application menus, including Flatpak. Play uses gio launch."
        } else {
            "Desktop game discovery currently requires Linux. Add executables as custom games on other platforms."
        }
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let directories = roots(
            &SourceConfig::default(),
            config
                .paths
                .iter()
                .cloned()
                .chain(default_directories())
                .collect(),
        );
        discover_directories(config, directories)
    }
}
fn data_directories() -> Vec<PathBuf> {
    let mut directories = vec![store::data_home()];
    directories.extend(
        env::var_os("XDG_DATA_DIRS")
            .filter(|s| !s.is_empty())
            .map(|paths| {
                env::split_paths(&paths)
                    .filter(|p| p.is_absolute())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec!["/usr/local/share".into(), "/usr/share".into()]),
    );
    directories.extend([
        store::data_home().join("flatpak/exports/share"),
        PathBuf::from("/var/lib/flatpak/exports/share"),
        PathBuf::from("/var/lib/snapd/desktop"),
    ]);
    directories
}
fn default_directories() -> Vec<PathBuf> {
    data_directories()
        .into_iter()
        .map(|dir| dir.join("applications"))
        .collect()
}
fn desktop_files(root: &Path, errors: &mut Vec<String>) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.into()];
    }
    let mut files = vec![];
    let mut pending = vec![(root.to_path_buf(), 0)];
    let mut visited = 0;
    while let Some((directory, depth)) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                errors.push(format!("{}: {error}", directory.display()));
                continue;
            }
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > 10_000 {
                errors.push(format!(
                    "{}: Desktop scan exceeds 10,000 entries",
                    root.display()
                ));
                return files;
            }
            let path = entry.path();
            // Do not follow directory symlinks or unbounded directory trees.
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) && depth < 8 {
                pending.push((path, depth + 1));
            } else if path.extension().is_some_and(|ext| ext == "desktop") && path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}
fn entry(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut bytes = vec![];
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 256 * 1024 {
        return Err("Desktop entry exceeds 256 KiB".into());
    }
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let mut active = false;
    let mut values = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            active = line == "[Desktop Entry]";
        } else if active
            && !line.starts_with('#')
            && let Some((key, value)) = line.split_once('=')
        {
            values.insert(key.trim().into(), unescape(value));
        }
    }
    Ok(values)
}
fn unescape(value: &str) -> String {
    let mut chars = value.chars();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('s') => out.push(' '),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(c) => {
                    out.push('\\');
                    out.push(c);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}
fn localized(values: &BTreeMap<String, String>, key: &str) -> String {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|key| env::var(key).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default();
    let (base, modifier) = locale.split_once('@').unwrap_or((&locale, ""));
    let base = base.split('.').next().unwrap_or(base);
    let language = base.split('_').next().unwrap_or(base);
    let candidates = [
        format!("{base}@{modifier}"),
        base.into(),
        format!("{language}@{modifier}"),
        language.into(),
    ];
    candidates
        .iter()
        .filter_map(|locale| values.get(&format!("{key}[{locale}]")))
        .chain(values.get(key))
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}
fn executable_exists(program: &str) -> bool {
    fn executable(path: &Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            path.is_file()
        }
    }
    if Path::new(program).is_absolute() {
        return executable(Path::new(program));
    }
    if program.contains('/') || program.is_empty() {
        return false;
    }
    env::var_os("PATH")
        .is_some_and(|paths| env::split_paths(&paths).any(|dir| executable(&dir.join(program))))
}
fn exec_program(exec: &str) -> Option<String> {
    let mut out = String::new();
    let mut quoted = false;
    let mut chars = exec.trim().chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => quoted = !quoted,
            '\\' => out.push(chars.next()?),
            c if c.is_whitespace() && !quoted => break,
            c => out.push(c),
        }
    }
    (!quoted && !out.is_empty()).then_some(out)
}
fn managed_launcher(id: &str, exec: &str, program: &str) -> bool {
    let known = [
        "steam",
        "com.valvesoftware.steam",
        "lutris",
        "net.lutris.lutris",
        "prismlauncher",
        "org.prismlauncher.prismlauncher",
        "multimc",
        "polymc",
        "org.polymc.polymc",
        "atlauncher",
        "atlauncher.sh",
        "com.atlauncher.atlauncher",
        "nile",
        "modrinth-app",
        "com.modrinth.modrinthapp",
        "com.modrinth.theseus",
        "heroic",
        "com.heroicgameslauncher.hgl",
        "legendary",
        "itch",
        "kitch",
        "io.itch.itch",
        "app.orbit.launcher",
        "orbit",
    ];
    let id = id.trim_end_matches(".desktop").to_ascii_lowercase();
    let program = Path::new(program)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    known.contains(&id.as_str())
        || known.contains(&program.as_str())
        || ["steam://", "lutris:", "heroic://", "modrinth://launch/"]
            .iter()
            .any(|uri| exec.contains(uri))
}
fn icon_url(icon: &str, root: &Path) -> String {
    if Path::new(icon).is_absolute() {
        return file_url(Path::new(icon));
    }
    if icon.is_empty() || icon.contains('/') || icon.contains('\\') {
        return String::new();
    }
    let data = root.parent().unwrap_or(root).to_path_buf();
    first_art(
        std::iter::once(data)
            .chain(data_directories())
            .flat_map(|dir| {
                [
                    "icons/hicolor/512x512/apps",
                    "icons/hicolor/256x256/apps",
                    "icons/hicolor/128x128/apps",
                    "icons/hicolor/64x64/apps",
                    "icons/hicolor/48x48/apps",
                    "icons/hicolor/scalable/apps",
                    "pixmaps",
                ]
                .into_iter()
                .flat_map(move |folder| {
                    let folder = dir.join(folder);
                    [format!("{icon}.png"), format!("{icon}.svg"), icon.into()]
                        .map(|name| folder.join(name))
                })
            }),
    )
}
fn discover_directories(
    config: &SourceConfig,
    directories: Vec<PathBuf>,
) -> (Vec<Game>, Vec<String>) {
    let mut games = vec![];
    let mut errors = vec![];
    let mut seen = HashSet::new();
    for root in directories {
        for file in desktop_files(&root, &mut errors) {
            let relative = file
                .strip_prefix(&root)
                .ok()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| file.file_name().map(Path::new).unwrap_or(&file));
            let id = relative.to_string_lossy().replace(['/', '\\'], "-");
            // Hidden user entries must mask the matching system entry too.
            if !seen.insert(id.clone()) {
                continue;
            }
            let values = match entry(&file) {
                Ok(values) => values,
                Err(error) => {
                    errors.push(format!("{}: {error}", file.display()));
                    continue;
                }
            };
            let value = |key: &str| values.get(key).map(String::as_str).unwrap_or("");
            if value("Type") != "Application"
                || value("Hidden") == "true"
                || value("NoDisplay") == "true"
                || !value("Categories").split(';').any(|c| c == "Game")
            {
                continue;
            }
            let title = localized(&values, "Name");
            let Some(program) = exec_program(value("Exec")) else {
                continue;
            };
            if title.is_empty()
                || managed_launcher(&id, value("Exec"), &program)
                || !executable_exists(&program)
                || (!value("TryExec").is_empty() && !executable_exists(value("TryExec")))
            {
                continue;
            }
            let icon = icon_url(
                value("Icon"),
                if root.is_file() {
                    root.parent().unwrap_or(&root)
                } else {
                    &root
                },
            );
            let mut command = if config.command.is_empty() {
                vec!["gio".into(), "launch".into()]
            } else {
                config.command.clone()
            };
            command.push(file.to_string_lossy().into_owned());
            games.push(Game {
                id: format!("desktop:{id}"),
                title,
                provider: "desktop".into(),
                subtitle: "Installed · Desktop game".into(),
                artwork: icon.clone(),
                art: Artwork {
                    kind: ArtworkKind::Icon,
                    icon,
                    ..Default::default()
                },
                command,
                environment: Default::default(),
                launch_uri: None,
                directory: None,
                launch_notice: String::new(),
                favorite: false,
                source_rank: 0,
                last_played: 0,
            });
        }
    }
    (games, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fn fixture(name: &str, extra: &str) -> String {
        format!(
            "[Desktop Entry]\nType=Application\nName={name}\nCategories=Game;\nExec=\"{}\" --title \"arg with spaces\" %U\n{extra}\n[Desktop Action Test]\nName=Wrong title\nExec=wrong\n",
            // Desktop-value escaping is decoded before quoted Exec escaping.
            std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .replace('\\', "\\\\\\\\")
        )
    }
    #[test]
    fn native_flatpak_and_emulator_entries_keep_desktop_launch_semantics() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("applications");
        let icon = temp.path().join("icons/hicolor/128x128/apps/fixture.png");
        write(&icon, "icon");
        for (id, name) in [
            ("native", "Native game"),
            ("org.example.Flatpak", "Flatpak game"),
            ("emulators/retro", "Emulator"),
        ] {
            write(
                &root.join(format!("{id}.desktop")),
                &fixture(name, "Icon=fixture\nTerminal=true\nPath=/tmp"),
            );
        }
        let (games, errors) = discover_directories(&SourceConfig::default(), vec![root.clone()]);
        assert!(errors.is_empty());
        assert_eq!(games.len(), 3);
        let game = games.iter().find(|g| g.title == "Native game").unwrap();
        assert_eq!(game.id, "desktop:native.desktop");
        assert_eq!(
            game.command,
            [
                "gio",
                "launch",
                root.join("native.desktop").to_str().unwrap()
            ]
        );
        assert_eq!(game.art.kind, ArtworkKind::Icon);
        assert!(game.artwork.ends_with("fixture.png"));
        assert!(
            games
                .iter()
                .any(|g| g.id == "desktop:emulators-retro.desktop")
        );
    }
    #[test]
    fn excludes_hidden_missing_non_games_and_managed_launchers() {
        let temp = tempfile::tempdir().unwrap();
        for (id, extra) in [
            ("hidden", "Hidden=true"),
            ("nodisplay", "NoDisplay=true"),
            ("missing", "TryExec=/no/such/orbit-fixture-game"),
            ("office", "Categories=Office;"),
            ("steam", ""),
            ("com.heroicgameslauncher.hgl", ""),
            ("shortcut", "Exec=steam steam://rungameid/123"),
            ("uninstalled", "Exec=/no/such/orbit-fixture-game"),
        ] {
            write(
                &temp.path().join(format!("{id}.desktop")),
                &fixture(id, extra),
            );
        }
        let (games, errors) =
            discover_directories(&SourceConfig::default(), vec![temp.path().into()]);
        assert!(errors.is_empty());
        assert!(games.is_empty());
    }
    #[test]
    fn user_overrides_mask_system_entries_and_preserve_ids_and_commands() {
        let temp = tempfile::tempdir().unwrap();
        let user = temp.path().join("user");
        let system = temp.path().join("system");
        write(&user.join("hidden.desktop"), "[Desktop Entry]\nHidden=true");
        write(
            &system.join("hidden.desktop"),
            &fixture("Hidden system game", ""),
        );
        write(&user.join("game.desktop"), &fixture("Preferred game", ""));
        write(&system.join("game.desktop"), &fixture("System game", ""));
        let config = SourceConfig {
            command: vec!["custom-desktop-launcher".into()],
            ..Default::default()
        };
        let (games, _) = discover_directories(&config, vec![user.clone(), system]);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Preferred game");
        assert_eq!(
            games[0].command,
            [
                "custom-desktop-launcher",
                user.join("game.desktop").to_str().unwrap()
            ]
        );
    }
    #[test]
    fn quoted_executables_and_desktop_value_escapes_are_preserved() {
        assert_eq!(
            exec_program("\"/games/Zoë Adventure/game\" --flag %U").as_deref(),
            Some("/games/Zoë Adventure/game")
        );
        assert!(exec_program("\"unterminated").is_none());
        assert_eq!(unescape(r"Game\sname\\path\nnext"), "Game name\\path\nnext");
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn gio_dispatch_preserves_arguments_working_folder_and_field_codes() {
        use std::os::unix::fs::PermissionsExt;
        if !executable_exists("gio") {
            eprintln!("GIO dispatch check skipped: gio is not installed");
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let program = temp.path().join("game with spaces");
        write(
            &program,
            "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\" > result.txt\n",
        );
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        let file = temp.path().join("game.desktop");
        write(
            &file,
            &format!(
                "[Desktop Entry]\nType=Application\nName=GIO dispatch fixture\nCategories=Game;\nExec=\"{}\" \"arg with spaces\" %c %k %U %%\nPath={}\nTerminal=false\n",
                program.display(),
                temp.path().display()
            ),
        );
        let (games, errors) = discover_directories(&SourceConfig::default(), vec![file.clone()]);
        assert!(errors.is_empty());
        assert_eq!(games.len(), 1);
        super::super::launch(&games[0]).unwrap();
        let result = temp.path().join("result.txt");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let content = loop {
            if let Ok(content) = fs::read_to_string(&result)
                && content.ends_with("%\n")
            {
                break content;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "GIO did not dispatch the fixture game"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let mut arguments = content.lines().collect::<Vec<_>>();
        // Older `gio launch` constructs the app from a keyfile, losing its
        // filename and omitting %k. Newer GIO constructs it from the filename.
        // Both must preserve every other argument and the configured directory.
        if arguments.len() == 5 {
            assert_eq!(arguments.remove(3), file.to_str().unwrap());
        }
        assert_eq!(
            arguments,
            [
                temp.path().to_str().unwrap(),
                "arg with spaces",
                "GIO dispatch fixture",
                "%"
            ]
        );
    }
}

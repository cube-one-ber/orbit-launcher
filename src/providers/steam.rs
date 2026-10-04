use super::*;
use std::collections::BTreeMap;
use std::path::Component;

#[derive(Debug, Clone)]
enum Value {
    Text(String),
    Object(BTreeMap<String, Value>),
}
impl Value {
    fn object(&self) -> Option<&BTreeMap<String, Value>> {
        if let Self::Object(o) = self {
            Some(o)
        } else {
            None
        }
    }
    fn text(&self) -> Option<&str> {
        if let Self::Text(s) = self {
            Some(s)
        } else {
            None
        }
    }
}
// Valve KeyValues: quoted strings, nested braces, escapes and line comments.
fn parse(input: &str) -> Result<BTreeMap<String, Value>, String> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => (),
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '{' | '}' => tokens.push(c.to_string()),
            '"' => {
                let mut value = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' => {
                            let next = chars.next().ok_or("Incomplete escape")?;
                            if next != '\\' && next != '"' {
                                value.push('\\');
                            }
                            value.push(next);
                        }
                        _ => value.push(c),
                    }
                }
                if !closed {
                    return Err("Unterminated KeyValues string".into());
                }
                tokens.push(format!("s{value}"));
            }
            _ => return Err("Unexpected KeyValues token".into()),
        }
    }
    fn object(
        tokens: &[String],
        i: &mut usize,
        depth: usize,
    ) -> Result<BTreeMap<String, Value>, String> {
        if depth > 32 {
            return Err("KeyValues nesting exceeds 32 levels".into());
        }
        let mut out = BTreeMap::new();
        while *i < tokens.len() {
            if tokens[*i] == "}" {
                if depth == 0 {
                    return Err("Unexpected closing brace".into());
                }
                *i += 1;
                return Ok(out);
            }
            let key = tokens[*i]
                .strip_prefix('s')
                .ok_or("Expected key")?
                .to_lowercase();
            *i += 1;
            let token = tokens.get(*i).ok_or("Missing value")?;
            *i += 1;
            let value = if token == "{" {
                Value::Object(object(tokens, i, depth + 1)?)
            } else {
                Value::Text(token.strip_prefix('s').ok_or("Expected value")?.into())
            };
            out.insert(key, value);
        }
        if depth > 0 {
            return Err("Unclosed object".into());
        }
        Ok(out)
    }
    object(&tokens, &mut 0, 0)
}
fn read(path: &Path) -> Result<BTreeMap<String, Value>, String> {
    let bytes =
        super::binary_metadata::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(std::str::from_utf8(&bytes).map_err(|e| format!("{}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()))
}
pub struct Steam;
impl Provider for Steam {
    fn id(&self) -> &str {
        "steam"
    }
    fn name(&self) -> &str {
        "Steam"
    }
    fn note(&self) -> &str {
        "Installed games and the most recent account's non-Steam shortcuts. Play requests Steam's tray mode; login and update prompts may still appear."
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let roots = roots(config, platform.steam_roots());
        let mut games = vec![];
        let mut errors = vec![];
        let mut seen = HashSet::new();
        let mut seen_games = HashSet::new();
        for root in roots {
            let configs = user_configs(&root, &mut errors);
            discover_shortcuts(
                &root,
                &configs,
                config,
                &mut games,
                &mut errors,
                &mut seen_games,
            );
            let mut libraries = vec![root.clone()];
            let folders = root.join("steamapps/libraryfolders.vdf");
            if folders.exists() {
                match read(&folders) {
                    Ok(data) => {
                        if let Some(entries) = data.get("libraryfolders").and_then(Value::object) {
                            for (key, entry) in entries {
                                if key.parse::<u32>().is_ok()
                                    && let Some(path) =
                                        entry.text().or_else(|| entry.object()?.get("path")?.text())
                                {
                                    libraries.push(PathBuf::from(path));
                                }
                            }
                        }
                    }
                    Err(e) => errors.push(e),
                }
            }
            for library in libraries {
                let apps = library.join("steamapps");
                if !seen.insert(fs::canonicalize(&apps).unwrap_or_else(|_| apps.clone())) {
                    continue;
                }
                if !apps.exists() {
                    continue;
                }
                let entries = match fs::read_dir(&apps) {
                    Ok(e) => e,
                    Err(e) => {
                        errors.push(format!("{}: {e}", apps.display()));
                        continue;
                    }
                };
                for file in entries.flatten().map(|e| e.path()).filter(|p| {
                    p.extension().is_some_and(|e| e == "acf")
                        && p.file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with("appmanifest_"))
                }) {
                    let data = match read(&file) {
                        Ok(d) => d,
                        Err(e) => {
                            errors.push(e);
                            continue;
                        }
                    };
                    let Some(app) = data.get("appstate").and_then(Value::object) else {
                        continue;
                    };
                    let Some(id) = app
                        .get("appid")
                        .and_then(Value::text)
                        .filter(|s| s.parse::<u32>().is_ok_and(|id| id > 0))
                    else {
                        continue;
                    };
                    let title = app.get("name").and_then(Value::text).unwrap_or(id).trim();
                    // Steam's FullyInstalled flag is bit 4.
                    if app
                        .get("stateflags")
                        .and_then(Value::text)
                        .and_then(|s| s.parse::<u32>().ok())
                        .is_none_or(|f| f & 4 == 0)
                    {
                        continue;
                    }
                    if title.starts_with("Steamworks")
                        || title.starts_with("Steam Linux Runtime")
                        || matches!(title, "Proton Experimental" | "Proton Hotfix")
                        || title
                            .strip_prefix("Proton ")
                            .is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit()))
                        || title.starts_with("GE-Proton")
                    {
                        continue;
                    }
                    // A stale manifest must not resurrect a moved/deleted game.
                    if let Some(folder) = app.get("installdir").and_then(Value::text) {
                        let folder = Path::new(folder);
                        if folder.components().count() != 1
                            || !matches!(folder.components().next(), Some(Component::Normal(_)))
                            || !apps.join("common").join(folder).is_dir()
                        {
                            continue;
                        }
                    }
                    if !seen_games.insert(format!("steam:{id}")) {
                        continue;
                    }
                    let mut command = steam_command(config, &root);
                    command.extend(["-applaunch".into(), id.into()]);
                    let cache = root.join("appcache/librarycache");
                    let mut artwork = custom_cover(&configs, id);
                    if artwork.is_empty() {
                        artwork = first_art(
                            [
                                "library_header_2x",
                                "library_header",
                                "header",
                                "library_hero",
                                "library_600x900_2x",
                                "library_600x900",
                            ]
                            .into_iter()
                            .flat_map(|name| {
                                ["jpg", "png", "webp", "jpeg"]
                                    .map(|ext| cache.join(format!("{id}_{name}.{ext}")))
                            }),
                        );
                    }
                    if artwork.is_empty()
                        && let Ok(entries) = fs::read_dir(cache.join(id))
                    {
                        let mut covers: Vec<_> = entries
                            .flatten()
                            .map(|e| e.path())
                            .filter(|p| {
                                p.file_name().is_some_and(|n| {
                                    [
                                        "library_header",
                                        "header",
                                        "library_hero",
                                        "library_600x900",
                                    ]
                                    .iter()
                                    .any(|s| n.to_string_lossy().contains(s))
                                }) && p.extension().is_some_and(|ext| {
                                    ["jpg", "png", "webp", "jpeg"]
                                        .contains(&ext.to_string_lossy().as_ref())
                                })
                            })
                            .collect();
                        covers.sort_by_key(|p| {
                            let name = p.file_name().unwrap_or_default().to_string_lossy();
                            (
                                [
                                    "library_header",
                                    "header",
                                    "library_hero",
                                    "library_600x900",
                                ]
                                .iter()
                                .position(|s| name.contains(s))
                                .unwrap_or(9),
                                name.to_string(),
                            )
                        });
                        artwork = first_art(covers);
                    }
                    games.push(Game {
                        id: format!("steam:{id}"),
                        title: title.into(),
                        provider: "steam".into(),
                        subtitle: "Steam · Installed".into(),
                        artwork,
                        art: Artwork {
                            remote: crate::artwork::steam_urls(id),
                            ..Default::default()
                        },
                        launch_notice: String::new(),
                        command,
                        environment: Default::default(),
                        launch_uri: None,
                        directory: None,
                        favorite: false,
                        source_rank: 0,
                        last_played: app
                            .get("lastplayed")
                            .and_then(Value::text)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0),
                    });
                }
            }
        }
        (games, errors)
    }
}
fn steam_command(config: &SourceConfig, root: &Path) -> Vec<String> {
    let platform = crate::platform::Platform::current();
    let binary = if cfg!(windows) {
        platform.steam_command(root)
    } else {
        "steam".into()
    };
    let mut command = launcher(config, is_flatpak(root), &binary, "com.valvesoftware.Steam");
    if !command.iter().any(|arg| arg == "-silent") {
        command.push("-silent".into());
    }
    command
}
fn user_configs(root: &Path, errors: &mut Vec<String>) -> Vec<PathBuf> {
    let login = root.join("config/loginusers.vdf");
    if login.is_file() {
        match read(&login) {
            Ok(data) => {
                if let Some(users) = data.get("users").and_then(Value::object) {
                    for (id, user) in users {
                        if user
                            .object()
                            .and_then(|u| u.get("mostrecent"))
                            .and_then(Value::text)
                            == Some("1")
                            && let Ok(id) = id.parse::<u64>()
                        {
                            return vec![
                                root.join("userdata")
                                    .join((id as u32).to_string())
                                    .join("config"),
                            ];
                        }
                    }
                }
            }
            Err(error) => errors.push(error),
        }
    }
    let mut configs: Vec<_> = fs::read_dir(root.join("userdata"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|user| user.path().join("config"))
        .filter(|dir| dir.is_dir())
        .collect();
    // Without login metadata prefer the most recently used local account.
    configs.sort_by_key(|dir| {
        (
            std::cmp::Reverse(
                [dir.join("localconfig.vdf"), dir.join("shortcuts.vdf")]
                    .into_iter()
                    .filter_map(|p| fs::metadata(p).ok()?.modified().ok())
                    .max(),
            ),
            dir.clone(),
        )
    });
    configs.truncate(1);
    configs
}
fn discover_shortcuts(
    root: &Path,
    configs: &[PathBuf],
    config: &SourceConfig,
    games: &mut Vec<Game>,
    errors: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for user in configs {
        let file = user.join("shortcuts.vdf");
        if !file.is_file() {
            continue;
        }
        let shortcuts = match super::steam_shortcuts::read(&file) {
            Ok(shortcuts) => shortcuts,
            Err(error) => {
                errors.push(format!("{}: {error}", file.display()));
                continue;
            }
        };
        for shortcut in shortcuts {
            let game_id = (u64::from(shortcut.app_id) << 32) | 0x0200_0000;
            let id = format!("steam:shortcut:{game_id}");
            if !seen.insert(id.clone()) {
                continue;
            }
            let mut artwork = custom_cover(configs, &shortcut.app_id.to_string());
            if artwork.is_empty() {
                artwork = custom_cover(configs, &game_id.to_string());
            }
            let icon = file_url(Path::new(&shortcut.icon));
            let mut command = steam_command(config, root);
            command.push(format!("steam://rungameid/{game_id}"));
            games.push(Game {
                id,
                title: shortcut.title,
                provider: "steam".into(),
                subtitle: "Non-Steam game · Steam".into(),
                art: Artwork {
                    kind: if artwork.is_empty() {
                        ArtworkKind::Icon
                    } else {
                        ArtworkKind::Cover
                    },
                    icon: icon.clone(),
                    ..Default::default()
                },
                artwork: if artwork.is_empty() { icon } else { artwork },
                command,
                launch_notice: String::new(),
                environment: Default::default(),
                launch_uri: None,
                directory: None,
                favorite: false,
                source_rank: 0,
                last_played: shortcut.last_played,
            });
        }
    }
}
fn custom_cover(configs: &[PathBuf], id: &str) -> String {
    first_art(configs.iter().map(|dir| dir.join("grid")).flat_map(|dir| {
        [format!("{id}_hero"), id.into(), format!("{id}p")]
            .into_iter()
            .flat_map(move |name| {
                ["png", "jpg", "webp"].map(|ext| dir.join(format!("{name}.{ext}")))
            })
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_nested_and_escaped_values() {
        let p = parse(
            "// comment\n\"libraryfolders\" { \"0\" { \"path\" \"/a/with \\\"quotes\\\"\" } }",
        )
        .unwrap();
        assert_eq!(
            p["libraryfolders"].object().unwrap()["0"].object().unwrap()["path"].text(),
            Some("/a/with \"quotes\"")
        );
    }
    #[test]
    fn windows_library_paths_preserve_backslashes_and_unicode() {
        let parsed = parse(r#""libraryfolders" { "0" { "path" "D:\\Games\\Zoë Library" } "1" { "path" "\\\\server\\share\\Steam" } }"#).unwrap();
        let folders = parsed["libraryfolders"].object().unwrap();
        assert_eq!(
            folders["0"].object().unwrap()["path"].text(),
            Some(r"D:\Games\Zoë Library")
        );
        assert_eq!(
            folders["1"].object().unwrap()["path"].text(),
            Some(r"\\server\share\Steam")
        );
    }
    #[test]
    fn rejects_truncated_files() {
        assert!(parse("\"appstate\" { \"appid\" \"1\"").is_err());
        assert!(parse("\"broken").is_err());
    }
}

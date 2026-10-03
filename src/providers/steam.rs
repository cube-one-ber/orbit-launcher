use super::*;
use std::collections::BTreeMap;

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
        nested: bool,
    ) -> Result<BTreeMap<String, Value>, String> {
        let mut out = BTreeMap::new();
        while *i < tokens.len() {
            if tokens[*i] == "}" {
                if !nested {
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
                Value::Object(object(tokens, i, true)?)
            } else {
                Value::Text(token.strip_prefix('s').ok_or("Expected value")?.into())
            };
            out.insert(key, value);
        }
        if nested {
            return Err("Unclosed object".into());
        }
        Ok(out)
    }
    object(&tokens, &mut 0, false)
}
fn read(path: &Path) -> Result<BTreeMap<String, Value>, String> {
    parse(&fs::read_to_string(path).map_err(|e| e.to_string())?)
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
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let roots = roots(config, platform.steam_roots());
        let mut games = vec![];
        let mut errors = vec![];
        let mut seen = HashSet::new();
        for root in roots {
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
                let entries = match fs::read_dir(&apps) {
                    Ok(e) => e,
                    Err(e) => {
                        errors.push(format!("{}: {e}", apps.display()));
                        continue;
                    }
                };
                for file in entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|e| e == "acf"))
                {
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
                        .filter(|s| s.parse::<u32>().is_ok())
                    else {
                        continue;
                    };
                    let title = app.get("name").and_then(Value::text).unwrap_or(id);
                    // Steam's FullyInstalled flag is bit 4.
                    if app
                        .get("stateflags")
                        .and_then(Value::text)
                        .and_then(|s| s.parse::<u32>().ok())
                        .is_some_and(|f| f & 4 == 0)
                    {
                        continue;
                    }
                    if title.starts_with("Steamworks")
                        || title.starts_with("Steam Linux Runtime")
                        || title.starts_with("Proton")
                    {
                        continue;
                    }
                    let windows_binary = platform.steam_command(&root);
                    let mut command = launcher(
                        config,
                        is_flatpak(&root),
                        if cfg!(windows) {
                            &windows_binary
                        } else {
                            "steam"
                        },
                        "com.valvesoftware.Steam",
                    );
                    command.push(format!("steam://rungameid/{id}"));
                    let cache = root.join("appcache/librarycache");
                    let mut artwork = first_art([
                        cache.join(format!("{id}_library_600x900.jpg")),
                        cache.join(format!("{id}_library_600x900_2x.jpg")),
                        cache.join(format!("{id}_header.jpg")),
                    ]);
                    if artwork.is_empty()
                        && let Ok(entries) = fs::read_dir(cache.join(id))
                    {
                        artwork = first_art(entries.flatten().map(|e| e.path()).filter(|p| {
                            p.file_name()
                                .is_some_and(|n| n.to_string_lossy().contains("library_600x900"))
                        }));
                    }
                    games.push(Game {
                        id: format!("steam:{id}"),
                        title: title.into(),
                        provider: "steam".into(),
                        subtitle: "Steam · Installed".into(),
                        artwork,
                        command,
                        launch_uri: None,
                        directory: None,
                        favorite: false,
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

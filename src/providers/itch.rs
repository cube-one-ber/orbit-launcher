use super::*;
use rusqlite::{Connection, OpenFlags};

pub struct Itch;
impl Provider for Itch {
    fn id(&self) -> &str {
        "itch"
    }
    fn name(&self) -> &str {
        "itch.io"
    }
    fn available(&self) -> bool {
        cfg!(any(target_os = "linux", windows))
    }
    fn note(&self) -> &str {
        if self.available() {
            "Native games launch headlessly with recent itch-setup; required prompts fall back to the app."
        } else {
            "itch.io direct game launching is supported on Linux and Windows."
        }
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let mut games = vec![];
        let mut errors = vec![];
        let mut seen = HashSet::new();
        for root in roots(config, platform.itch_roots()) {
            let file = if root.is_file() {
                root
            } else {
                root.join("db/butler.db")
            };
            if !file.exists() {
                continue;
            }
            let key = fs::canonicalize(&file).unwrap_or_else(|_| file.clone());
            if !seen.insert(key.clone()) {
                continue;
            }
            let Some(data) = file.parent().and_then(Path::parent) else {
                continue;
            };
            let app = data.file_name().unwrap_or_default().to_string_lossy();
            if !["itch", "kitch"].contains(&app.as_ref())
                || file
                    .parent()
                    .and_then(Path::file_name)
                    .is_none_or(|n| n != "db")
            {
                errors.push(format!(
                    "{}: Expected an itch or kitch data folder containing db/butler.db",
                    file.display()
                ));
                continue;
            }
            // Windows itch-setup resolves roaming data through the OS known-folder API.
            // Do not silently launch a different profile for an unsupported relocated DB.
            let selected_data = fs::canonicalize(data).unwrap_or_else(|_| data.to_path_buf());
            if platform.os == crate::platform::Os::Windows
                && !platform
                    .itch_roots()
                    .iter()
                    .any(|p| fs::canonicalize(p).is_ok_and(|p| p == selected_data))
            {
                errors.push(format!(
                    "{}: Windows itch data must belong to the current roaming profile",
                    file.display()
                ));
                continue;
            }
            let result = read_games(&key, data, &app, config, &platform, &mut errors);
            match result {
                Ok(found) => games.extend(found),
                Err(e) => errors.push(format!("{}: {e}", file.display())),
            }
        }
        (games, errors)
    }
}
fn read_games(
    file: &Path,
    data: &Path,
    app: &str,
    config: &SourceConfig,
    platform: &crate::platform::Platform,
    errors: &mut Vec<String>,
) -> Result<Vec<Game>, rusqlite::Error> {
    let db = Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.busy_timeout(std::time::Duration::from_secs(2))?;
    // Query only public game metadata and install locations; profiles/tokens are untouched.
    let mut statement = db.prepare("SELECT c.game_id, g.title, g.still_cover_url, g.cover_url, c.custom_install_folder, l.path, c.install_folder_name FROM caves c JOIN games g ON g.id = c.game_id LEFT JOIN install_locations l ON l.id = c.install_location_id WHERE COALESCE(c.morphing, 0) = 0 ORDER BY c.last_touched_at DESC, c.id")?;
    let rows = statement.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            r.get::<_, Option<String>>(6)?.unwrap_or_default(),
        ))
    })?;
    let mut seen = HashSet::new();
    let mut games = vec![];
    for row in rows {
        let (id, title, still, cover, custom, location, folder) = match row {
            Ok(row) => row,
            Err(e) => {
                errors.push(format!("{}: Invalid install row: {e}", file.display()));
                continue;
            }
        };
        if id <= 0 {
            continue;
        }
        let path = if custom.is_empty() {
            let relative = Path::new(&folder);
            if folder.is_empty()
                || relative
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                errors.push(format!("{}: Invalid itch install folder", file.display()));
                continue;
            }
            PathBuf::from(location).join(relative)
        } else {
            PathBuf::from(custom)
        };
        let Some(path) = installed::installed_path(&path.to_string_lossy(), file, errors) else {
            continue;
        };
        if !seen.insert(id) {
            continue;
        }
        let key = url::Url::from_file_path(file)
            .map(|u| u.to_string())
            .unwrap_or_default();
        let mut game = installed::game(
            "itch",
            &format!("{key}:{id}"),
            installed::title_or_folder(Some(&title), &path),
            &path,
        );
        game.subtitle = format!("Installed · {app}");
        game.art.remote = [still, cover]
            .into_iter()
            .filter(|s| s.starts_with("https://"))
            .collect();
        game.art.remote.dedup();
        game.command = if config.command.is_empty() {
            vec![platform.itch_command(app)]
        } else {
            config.command.clone()
        };
        game.command.extend([
            "--appname".into(),
            app.into(),
            "--run-game".into(),
            id.to_string(),
        ]);
        if platform.os == crate::platform::Os::Linux {
            game.environment.insert(
                "XDG_CONFIG_HOME".into(),
                data.parent().unwrap_or(data).to_string_lossy().into_owned(),
            );
        }
        games.push(game);
    }
    Ok(games)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, PathBuf, crate::platform::Platform) {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("config/itch");
        fs::create_dir_all(data.join("db")).unwrap();
        fs::create_dir_all(temp.path().join("Installed game")).unwrap();
        let file = data.join("db/butler.db");
        let db = Connection::open(&file).unwrap();
        db.execute_batch("CREATE TABLE games (id INTEGER PRIMARY KEY, title TEXT, still_cover_url TEXT, cover_url TEXT); CREATE TABLE install_locations (id TEXT PRIMARY KEY, path TEXT); CREATE TABLE caves (id TEXT PRIMARY KEY, game_id INTEGER, morphing INTEGER, custom_install_folder TEXT, install_location_id TEXT, install_folder_name TEXT, last_touched_at TEXT);").unwrap();
        db.execute(
            "INSERT INTO install_locations VALUES ('home',?1)",
            [temp.path().to_string_lossy()],
        )
        .unwrap();
        db.execute("INSERT INTO games VALUES (123,'Fixture app','https://artwork.example/still.png','https://artwork.example/animated.gif')", []).unwrap();
        db.execute(
            "INSERT INTO caves VALUES ('valid',123,0,'','home','Installed game','2026-10-01')",
            [],
        )
        .unwrap();
        let platform = crate::platform::Platform::from_environment(
            crate::platform::Os::Linux,
            |key| match key {
                "HOME" => Some(temp.path().into()),
                "XDG_CONFIG_HOME" => Some(temp.path().join("config")),
                _ => None,
            },
        );
        (temp, file, platform)
    }
    #[test]
    fn itch_database_is_read_only_and_launch_preserves_configuration_and_covers() {
        let (_temp, file, platform) = fixture();
        let before = fs::read(&file).unwrap();
        let data = file.parent().unwrap().parent().unwrap();
        let mut errors = vec![];
        let cfg = SourceConfig {
            command: vec!["portable itch-setup".into()],
            ..Default::default()
        };
        let games = read_games(&file, data, "itch", &cfg, &platform, &mut errors).unwrap();
        assert!(errors.is_empty());
        assert_eq!(games.len(), 1);
        assert_eq!(
            games[0].command,
            [
                "portable itch-setup",
                "--appname",
                "itch",
                "--run-game",
                "123"
            ]
        );
        assert_eq!(
            games[0].environment["XDG_CONFIG_HOME"],
            data.parent().unwrap().to_string_lossy()
        );
        assert!(games[0].id.ends_with(":123"));
        assert_eq!(
            games[0].art.remote,
            [
                "https://artwork.example/still.png",
                "https://artwork.example/animated.gif"
            ]
        );
        assert_eq!(before, fs::read(file).unwrap());
    }
    #[test]
    fn itch_skips_missing_morphing_external_and_bad_rows_but_keeps_custom_installs() {
        let (temp, file, platform) = fixture();
        let db = Connection::open(&file).unwrap();
        db.execute_batch("INSERT INTO caves VALUES ('duplicate',123,0,'','home','Installed game','2026-09-01'); INSERT INTO games VALUES (124,'Custom fixture',NULL,NULL),(125,'Missing',NULL,NULL),(126,'Morphing',NULL,NULL),(127,'Invalid path',NULL,NULL),(128,'Bad row',NULL,NULL); INSERT INTO caves VALUES ('missing',125,0,'','home','Missing folder',NULL),('morphing',126,1,'','home','Installed game',NULL),('traversal',127,0,'','home','../Installed game',NULL),('invalid',128,0,12,'home','Installed game',NULL),('external',0,0,'','home','Installed game',NULL);").unwrap();
        db.execute(
            "INSERT INTO caves VALUES ('custom',124,0,?1,'missing-location','ignored',NULL)",
            [temp.path().join("Installed game").to_string_lossy()],
        )
        .unwrap();
        let mut errors = vec![];
        let games = read_games(
            &file,
            file.parent().unwrap().parent().unwrap(),
            "itch",
            &SourceConfig::default(),
            &platform,
            &mut errors,
        )
        .unwrap();
        assert_eq!(games.len(), 2);
        assert!(games.iter().any(|g| g.title == "Custom fixture"));
        assert!(
            errors
                .iter()
                .any(|e| e.contains("Invalid itch install folder"))
        );
        // SQLite TEXT affinity converts 12 to a string: it is rejected as a relative path.
        assert!(errors.iter().any(|e| e.contains("absolute")));
    }
    #[test]
    fn itch_windows_launch_uses_setup_without_rewriting_roaming_environment() {
        let (_temp, file, mut platform) = fixture();
        platform.os = crate::platform::Os::Windows;
        let mut errors = vec![];
        let games = read_games(
            &file,
            file.parent().unwrap().parent().unwrap(),
            "kitch",
            &SourceConfig::default(),
            &platform,
            &mut errors,
        )
        .unwrap();
        assert!(games[0].command[0].ends_with("itch-setup.exe"));
        assert_eq!(
            &games[0].command[1..],
            ["--appname", "kitch", "--run-game", "123"]
        );
        assert!(games[0].environment.is_empty());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn itch_discovery_deduplicates_database_aliases_and_reports_corrupt_files() {
        let (_temp, file, _platform) = fixture();
        let data = file.parent().unwrap().parent().unwrap();
        let cfg = SourceConfig {
            paths: vec![file.clone(), data.into()],
            ..Default::default()
        };
        let (games, errors) = Itch.discover(&cfg);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(games.iter().filter(|g| g.title == "Fixture app").count(), 1);
        fs::write(&file, "broken database").unwrap();
        let (_, errors) = Itch.discover(&cfg);
        assert!(errors.iter().any(|e| e.contains("butler.db")));
    }
}

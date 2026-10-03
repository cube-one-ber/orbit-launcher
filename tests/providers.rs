use orbit_launcher::{
    model::*,
    providers::{self, Lutris, Prism, Provider, Steam},
    store,
};
use std::{fs, path::Path};
fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn source(path: &Path) -> SourceConfig {
    SourceConfig {
        paths: vec![path.into()],
        ..Default::default()
    }
}
fn isolated() -> Settings {
    let mut settings = Settings::default();
    for cfg in settings.sources.values_mut() {
        cfg.enabled = false;
    }
    settings
}
#[test]
fn steam_follows_external_library_and_deduplicates() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Steam");
    let extra = temp.path().join("External library");
    write(
        &root.join("steamapps/libraryfolders.vdf"),
        &format!(
            "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} }}",
            root.display().to_string().replace('\\', "\\\\"),
            extra.display().to_string().replace('\\', "\\\\")
        ),
    );
    write(
        &extra.join("steamapps/appmanifest_9876543.acf"),
        "\"AppState\" { \"appid\" \"9876543\" \"name\" \"Fixture Adventure\" \"StateFlags\" \"4\" }",
    );
    write(
        &extra.join("steamapps/appmanifest_9876544.acf"),
        "\"AppState\" { \"appid\" \"9876544\" \"name\" \"Not installed\" \"StateFlags\" \"2\" }",
    );
    let mut config = source(&root);
    config.command = vec!["fixture-steam".into()];
    let (games, _) = Steam.discover(&config);
    let matches: Vec<_> = games.iter().filter(|g| g.id == "steam:9876543").collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].command,
        ["fixture-steam", "steam://rungameid/9876543"]
    );
    assert!(!games.iter().any(|g| g.id == "steam:9876544"));
}
#[test]
fn steam_corrupt_manifest_does_not_hide_other_games() {
    let t = tempfile::tempdir().unwrap();
    write(&t.path().join("steamapps/broken.acf"), "\"AppState\" {");
    write(
        &t.path().join("steamapps/good.acf"),
        "\"AppState\" { \"appid\" \"9876555\" \"name\" \"Healthy fixture\" }",
    );
    let (games, errors) = Steam.discover(&source(t.path()));
    assert!(games.iter().any(|g| g.id == "steam:9876555"));
    assert!(errors.iter().any(|s| s.contains("broken.acf")));
}
#[test]
fn flatpak_and_command_override_are_preserved() {
    let config = SourceConfig::default();
    assert_eq!(
        providers::launcher(&config, true, "steam", "com.valvesoftware.Steam"),
        ["flatpak", "run", "com.valvesoftware.Steam"]
    );
    let config = SourceConfig {
        command: vec!["/path with spaces/Steam".into()],
        ..Default::default()
    };
    assert_eq!(
        providers::launcher(&config, true, "steam", "app"),
        ["/path with spaces/Steam"]
    );
}
#[test]
fn lutris_reads_only_installed_games_without_modifying_database() {
    let t = tempfile::tempdir().unwrap();
    let db = t.path().join("pga.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch("CREATE TABLE games (id INTEGER, name TEXT, slug TEXT, runner TEXT, installed INTEGER); INSERT INTO games VALUES(42,'Fixture Lutris','fixture','wine',1),(43,'Uninstalled','gone','wine',0);").unwrap();
    drop(conn);
    let before = fs::read(&db).unwrap();
    let (games, _) = Lutris.discover(&source(t.path()));
    let game = games.iter().find(|g| g.title == "Fixture Lutris").unwrap();
    assert_eq!(game.command, ["lutris", "lutris:rungameid/42"]);
    assert!(!games.iter().any(|g| g.title == "Uninstalled"));
    assert_eq!(before, fs::read(db).unwrap());
}
#[test]
fn lutris_bad_schema_is_reported() {
    let t = tempfile::tempdir().unwrap();
    rusqlite::Connection::open(t.path().join("pga.db")).unwrap();
    let (_, errors) = Lutris.discover(&source(t.path()));
    assert!(errors.iter().any(|e| e.contains("no such table")));
}
#[test]
fn prism_respects_instance_dir_and_passes_id_as_one_argument() {
    let t = tempfile::tempdir().unwrap();
    write(
        &t.path().join("prismlauncher.cfg"),
        "[General]\nInstanceDir=custom instances\n",
    );
    write(
        &t.path().join("custom instances/My world/instance.cfg"),
        "[General]\nname=Fixture world\nlastLaunchTime=1700000000000\n",
    );
    let (games, _) = Prism.discover(&source(t.path()));
    let game = games.iter().find(|g| g.title == "Fixture world").unwrap();
    assert_eq!(game.command.last().unwrap(), "My world");
    assert_eq!(game.command[1], "--dir");
    assert!(game.command.iter().any(|arg| arg == "--launch"));
    assert!(
        !game
            .command
            .iter()
            .any(|arg| arg == "--show-window" || arg == "--show")
    );
    assert_eq!(game.last_played, 1700000000);
}
#[test]
fn extensions_have_namespaced_ids_and_can_be_disabled() {
    let t = tempfile::tempdir().unwrap();
    write(
        &t.path().join("providers/example.json"),
        r#"{"id":"example","name":"Example","games":[{"id":"one","title":"Example game","provider":"ignored","subtitle":"Local","artwork":"","command":["example","arg with spaces"]}]}"#,
    );
    let mut settings = isolated();
    let library = providers::discover(&settings, t.path());
    assert_eq!(library.games.len(), 1);
    assert_eq!(library.games[0].id, "example:one");
    assert_eq!(library.games[0].provider, "example");
    settings.sources.insert(
        "example".into(),
        SourceConfig {
            enabled: false,
            ..Default::default()
        },
    );
    assert!(providers::discover(&settings, t.path()).games.is_empty());
}
#[test]
fn extensions_cannot_replace_builtin_providers() {
    let t = tempfile::tempdir().unwrap();
    write(
        &t.path().join("providers/collision.json"),
        r#"{"id":"steam","name":"Imposter","games":[]}"#,
    );
    let library = providers::discover(&isolated(), t.path());
    assert!(
        library
            .providers
            .iter()
            .any(|p| p.errors.iter().any(|e| e.contains("unique")))
    );
}
#[test]
fn saved_metadata_survives_refresh() {
    let t = tempfile::tempdir().unwrap();
    let mut settings = isolated();
    settings.custom_games.push(Game {
        id: "custom:1".into(),
        title: "Test".into(),
        provider: "custom".into(),
        subtitle: "".into(),
        artwork: "".into(),
        art: Artwork::default(),
        launch_notice: String::new(),
        command: vec!["test".into()],
        launch_uri: None,
        directory: None,
        favorite: false,
        last_played: 0,
    });
    settings.favorites.push("custom:1".into());
    settings.played.insert("custom:1".into(), 123);
    store::save(t.path(), &settings).unwrap();
    let library = providers::discover(&store::load(t.path()).unwrap(), t.path());
    assert!(library.games[0].favorite);
    assert_eq!(library.games[0].last_played, 123);
}
#[test]
fn file_urls_escape_special_characters() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("cover #1.png");
    fs::write(&p, []).unwrap();
    assert!(providers::file_url(&p).ends_with("cover%20%231.png"));
}
#[test]
fn missing_executable_reports_actionable_error() {
    let game = Game {
        id: "x".into(),
        title: "x".into(),
        provider: "custom".into(),
        subtitle: "".into(),
        artwork: "".into(),
        art: Artwork::default(),
        launch_notice: String::new(),
        command: vec!["/no/such/orbit-test-executable".into()],
        launch_uri: None,
        directory: None,
        favorite: false,
        last_played: 0,
    };
    assert!(
        providers::launch(&game)
            .unwrap_err()
            .contains("Could not start")
    );
}
#[test]
#[cfg(unix)]
fn launch_arguments_are_not_interpreted_by_a_shell() {
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("args");
    let script = t.path().join("capture");
    write(&script, "#!/bin/sh\nprintf '%s' \"$2\" > \"$1\"\n");
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let literal = "$(touch SHOULD_NOT_EXIST); arg with spaces";
    let game = Game {
        id: "x".into(),
        title: "x".into(),
        provider: "custom".into(),
        subtitle: "".into(),
        artwork: "".into(),
        art: Artwork::default(),
        launch_notice: String::new(),
        command: vec![
            script.display().to_string(),
            out.display().to_string(),
            literal.into(),
        ],
        launch_uri: None,
        directory: Some(t.path().into()),
        favorite: false,
        last_played: 0,
    };
    providers::launch(&game).unwrap();
    for _ in 0..100 {
        if fs::read_to_string(&out).is_ok_and(|s| s == literal) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(fs::read_to_string(out).unwrap(), literal);
    assert!(!t.path().join("SHOULD_NOT_EXIST").exists());
}

#[test]
fn configured_missing_paths_are_reported() {
    let t = tempfile::tempdir().unwrap();
    let mut settings = isolated();
    settings
        .sources
        .insert("prism".into(), source(&t.path().join("missing")));
    let library = providers::discover(&settings, t.path());
    assert!(
        library
            .providers
            .iter()
            .find(|p| p.id == "prism")
            .unwrap()
            .errors
            .iter()
            .any(|e| e.contains("does not exist"))
    );
}

#[test]
fn epic_skips_incomplete_and_invalid_manifests_without_hiding_games() {
    let t = tempfile::tempdir().unwrap();
    let installed = t.path().join("An Installed Game");
    fs::create_dir_all(&installed).unwrap();
    let manifests = t.path().join("Manifests");
    let good = serde_json::json!({
        "AppName":"test-app", "DisplayName":"Epic fixture", "InstallLocation":installed,
        "MainGameAppName":"test-app", "LaunchExecutable":"Game.exe", "bIsIncompleteInstall":false
    });
    write(&manifests.join("good.item"), &good.to_string());
    let mut incomplete = good.clone();
    incomplete["AppName"] = "incomplete".into();
    incomplete["bIsIncompleteInstall"] = true.into();
    write(&manifests.join("incomplete.item"), &incomplete.to_string());
    write(&manifests.join("broken.item"), "invalid json");
    let (games, errors) = providers::Epic.discover(&source(&manifests));
    let fixture = games.iter().find(|g| g.id == "epic:test-app").unwrap();
    assert_eq!(fixture.title, "Epic fixture");
    assert!(
        fixture
            .launch_uri
            .as_ref()
            .unwrap()
            .starts_with("com.epicgames.launcher://apps/test-app?")
    );
    assert!(!games.iter().any(|g| g.id == "epic:incomplete"));
    assert!(errors.iter().any(|e| e.contains("broken.item")));
}

#[test]
fn epic_override_receives_uri_as_a_single_argument() {
    let t = tempfile::tempdir().unwrap();
    let installed = t.path().join("Game");
    fs::create_dir_all(&installed).unwrap();
    write(&t.path().join("game.item"), &serde_json::json!({"AppName":"app with spaces", "DisplayName":"Game", "InstallLocation":installed,"LaunchExecutable":"game.exe"}).to_string());
    let mut cfg = source(t.path());
    cfg.command = vec!["EpicGamesLauncher.exe".into()];
    let (games, _) = providers::Epic.discover(&cfg);
    let game = games
        .iter()
        .find(|g| g.id == "epic:app with spaces")
        .unwrap();
    assert!(game.launch_uri.is_none());
    assert_eq!(game.command.len(), 2);
    assert!(game.command[1].contains("app%20with%20spaces"));
}

#[test]
fn gog_deduplicates_games_and_ignores_dlc_metadata() {
    let t = tempfile::tempdir().unwrap();
    let game_dir = t.path().join("Games/Fixture GOG");
    write(
        &game_dir.join("goggame-9876500000.info"),
        r#"{"gameId":"9876500000","name":"GOG fixture","rootGameId":"9876500000"}"#,
    );
    write(
        &game_dir.join("goggame-9876500001.info"),
        r#"{"gameId":"9876500001","name":"DLC fixture","rootGameId":"9876500000"}"#,
    );
    write(&game_dir.join("goggame-broken.info"), "bad JSON");
    let cfg = SourceConfig {
        paths: vec![game_dir.clone(), t.path().join("Games")],
        command: vec!["GalaxyClient.exe".into()],
        ..Default::default()
    };
    let (games, errors) = providers::Gog.discover(&cfg);
    let fixtures: Vec<_> = games.iter().filter(|g| g.id == "gog:9876500000").collect();
    assert_eq!(fixtures.len(), 1);
    assert_eq!(
        fixtures[0].command,
        [
            "GalaxyClient.exe".to_string(),
            "/command=runGame".into(),
            "/gameId=9876500000".into(),
            format!("/path={}", game_dir.display())
        ]
    );
    assert!(!games.iter().any(|g| g.title == "DLC fixture"));
    assert!(errors.iter().any(|e| e.contains("broken.info")));
}

#[test]
fn provider_availability_matches_the_os() {
    assert_eq!(providers::Epic.available(), cfg!(windows));
    assert_eq!(providers::Gog.available(), cfg!(windows));
    assert_eq!(providers::Lutris.available(), !cfg!(windows));
    assert!(providers::Steam.available());
    assert!(providers::Prism.available());
}

#[test]
fn unsupported_launch_protocols_are_rejected() {
    for uri in [
        "file:///tmp/test",
        "https://example.com",
        "powershell://example",
        "com.epicgames.launcher://apps/test\0",
    ] {
        assert!(orbit_launcher::platform::open_game_uri(uri).is_err());
    }
}

#[test]
fn epic_uses_catalog_identity_and_excludes_engine_plugins() {
    let t = tempfile::tempdir().unwrap();
    let game = serde_json::json!({
        "AppName":"test-app", "DisplayName":"Game", "InstallLocation":t.path(),
        "LaunchExecutable":"Game.exe", "CatalogNamespace":"namespace", "CatalogItemId":"item",
        "AppCategories":["games"]
    });
    write(&t.path().join("good.item"), &game.to_string());
    let mut plugin = game.clone();
    plugin["AppName"] = "plugin".into();
    plugin["AppCategories"] = serde_json::json!(["plugins/engine"]);
    write(&t.path().join("plugin.item"), &plugin.to_string());
    let (games, _) = providers::Epic.discover(&source(t.path()));
    let fixture = games.iter().find(|g| g.id == "epic:test-app").unwrap();
    assert_eq!(
        fixture.launch_uri.as_deref(),
        Some("com.epicgames.launcher://apps/namespace%3Aitem%3Atest-app?action=launch&silent=true")
    );
    assert!(!games.iter().any(|g| g.id == "epic:plugin"));
}

use orbit_launcher::{
    model::SourceConfig,
    providers::{self, Prism, Provider, Steam},
};
use std::{fs, path::Path};

fn source(path: &Path) -> SourceConfig {
    SourceConfig {
        paths: vec![path.into()],
        ..Default::default()
    }
}
fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn modrinth_legacy_db(path: &Path) -> rusqlite::Connection {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let connection = rusqlite::Connection::open(path).unwrap();
    connection.execute_batch("CREATE TABLE settings(custom_dir TEXT); CREATE TABLE profiles(path TEXT, name TEXT, install_stage TEXT, icon_path TEXT, game_version TEXT, mod_loader TEXT, last_played INTEGER, linked_project_id TEXT);").unwrap();
    connection
}

#[test]
fn legacy_profiles_respect_custom_directory_and_leave_database_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("ModrinthApp");
    let data = temp.path().join("Zoë Minecraft library");
    fs::create_dir_all(data.join("profiles/My world")).unwrap();
    let db = root.join("app.db");
    let connection = modrinth_legacy_db(&db);
    connection
        .execute("INSERT INTO settings VALUES(?1)", [data.to_str().unwrap()])
        .unwrap();
    connection.execute_batch("INSERT INTO profiles VALUES('My world','My Minecraft world','installed',NULL,'1.21.5','fabric',123,'example-pack'); INSERT INTO profiles VALUES('unfinished','Not installed','installing',NULL,'1.21.5','vanilla',NULL,NULL);").unwrap();
    drop(connection);
    let before = fs::read(&db).unwrap();
    let mut cfg = source(&root);
    cfg.command = vec!["Modrinth App.exe".into()];
    let (games, errors) = providers::Modrinth.discover(&cfg);
    assert!(errors.is_empty(), "{errors:?}");
    let game = games
        .iter()
        .find(|g| g.title == "My Minecraft world")
        .unwrap();
    assert_eq!(game.command, ["Modrinth App.exe"]);
    assert!(game.launch_uri.is_none());
    assert!(!game.launch_notice.is_empty());
    assert_eq!(game.art.minecraft_version.as_deref(), Some("1.21.5"));
    assert_eq!(game.art.modrinth_project.as_deref(), Some("example-pack"));
    assert_eq!(game.last_played, 123);
    assert!(!games.iter().any(|g| g.title == "Not installed"));
    assert_eq!(before, fs::read(db).unwrap());
}

#[test]
fn current_instances_launch_by_encoded_id_and_use_active_content_version() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("profiles/My world")).unwrap();
    let db = temp.path().join("app.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection.execute_batch("CREATE TABLE instances(id TEXT, path TEXT, name TEXT, install_stage TEXT, icon_path TEXT, applied_content_set_id TEXT, last_played INTEGER);
        CREATE TABLE instance_content_sets(id TEXT, instance_id TEXT, game_version TEXT, loader TEXT);
        CREATE TABLE instance_links(instance_id TEXT, modrinth_project_id TEXT);
        INSERT INTO instances VALUES('instance # ü','My world','Modern world','installed',NULL,'active',321);
        INSERT INTO instance_content_sets VALUES('active','instance # ü','26.1.2','vanilla');
        INSERT INTO instance_content_sets VALUES('pending','instance # ü','26.2','vanilla');
        INSERT INTO instance_links VALUES('instance # ü',NULL);").unwrap();
    drop(connection);
    let mut cfg = source(temp.path());
    cfg.paths.push(db.clone());
    let (games, errors) = providers::Modrinth.discover(&cfg);
    assert!(errors.is_empty(), "{errors:?}");
    let game = games.iter().find(|g| g.title == "Modern world").unwrap();
    assert_eq!(
        games.iter().filter(|g| g.title == "Modern world").count(),
        1
    );
    assert_eq!(game.art.minecraft_version.as_deref(), Some("26.1.2"));
    let uri = game.launch_uri.as_ref().unwrap();
    assert_eq!(uri, "modrinth://launch/instance/instance%20%23%20%C3%BC");
    assert!(orbit_launcher::platform::validate_game_uri(uri).is_ok());
    assert!(game.launch_notice.is_empty());
    cfg.command = vec!["/Apps/Modrinth App".into(), "--custom-argument".into()];
    let (games, _) = providers::Modrinth.discover(&cfg);
    let game = games.iter().find(|g| g.title == "Modern world").unwrap();
    assert_eq!(
        game.command,
        ["/Apps/Modrinth App", "--custom-argument", uri]
    );
    assert!(game.launch_uri.is_none());
}

#[test]
fn bad_rows_and_paths_do_not_hide_healthy_profiles() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("profiles/valid")).unwrap();
    let connection = modrinth_legacy_db(&temp.path().join("app.db"));
    connection.execute_batch("INSERT INTO profiles VALUES('valid','Healthy','installed',NULL,'1.20.1','fabric',NULL,NULL);
        INSERT INTO profiles VALUES('valid',NULL,'installed',NULL,'1.20.1','fabric',NULL,NULL);
        INSERT INTO profiles VALUES('../escape','Escaped','installed',NULL,'1.20.1','fabric',NULL,NULL);
        INSERT INTO profiles VALUES('missing','Missing','installed',NULL,'1.20.1','fabric',NULL,NULL);").unwrap();
    drop(connection);
    let (games, errors) = providers::Modrinth.discover(&source(temp.path()));
    assert_eq!(games.iter().filter(|g| g.title == "Healthy").count(), 1);
    assert!(
        !games
            .iter()
            .any(|g| g.title == "Escaped" || g.title == "Missing")
    );
    assert!(errors.iter().any(|e| e.contains("Invalid instance path")));
    assert!(errors.len() >= 2);
}

#[test]
fn database_schema_errors_are_actionable() {
    let temp = tempfile::tempdir().unwrap();
    rusqlite::Connection::open(temp.path().join("app.db")).unwrap();
    let (_, errors) = providers::Modrinth.discover(&source(temp.path()));
    assert!(
        errors
            .iter()
            .any(|e| e.contains("Unsupported Modrinth database layout"))
    );
}

#[test]
fn prism_exposes_minecraft_version_and_preserves_custom_cover_priority() {
    let temp = tempfile::tempdir().unwrap();
    let instance = temp.path().join("instances/Vanilla world");
    write(
        &instance.join("instance.cfg"),
        "name=Vanilla world\niconKey=custom\n",
    );
    write(
        &instance.join("mmc-pack.json"),
        r#"{"components":[{"uid":"net.fabricmc.fabric-loader","version":"0.16.0"},{"uid":"net.minecraft","version":"1.21.5"}]}"#,
    );
    write(&instance.join("cover.webp"), "fixture image path");
    write(&temp.path().join("icons/custom.png"), "fixture icon path");
    let mut cfg = source(temp.path());
    cfg.command = vec!["prismlauncher".into()];
    let (games, _) = Prism.discover(&cfg);
    let game = games.iter().find(|g| g.title == "Vanilla world").unwrap();
    assert_eq!(game.art.minecraft_version.as_deref(), Some("1.21.5"));
    assert!(game.artwork.ends_with("cover.webp"));
    assert!(game.art.icon.ends_with("custom.png"));
    assert_eq!(
        game.command,
        [
            "prismlauncher".to_string(),
            "--dir".into(),
            temp.path().display().to_string(),
            "--launch".into(),
            "Vanilla world".into()
        ]
    );
}

#[test]
fn steam_prefers_custom_art_and_landscape_cache_variants() {
    let temp = tempfile::tempdir().unwrap();
    write(
        &temp.path().join("steamapps/appmanifest_98765001.acf"),
        "\"AppState\" { \"appid\" \"98765001\" \"name\" \"Artwork fixture\" \"StateFlags\" \"4\" }",
    );
    write(
        &temp
            .path()
            .join("appcache/librarycache/98765001/hash_library_header.webp"),
        "fixture cover path",
    );
    write(
        &temp
            .path()
            .join("appcache/librarycache/98765001/hash_library_600x900.jpg"),
        "fixture portrait path",
    );
    let cfg = source(temp.path());
    let (games, _) = Steam.discover(&cfg);
    let game = games.iter().find(|g| g.id == "steam:98765001").unwrap();
    assert!(game.artwork.ends_with("hash_library_header.webp"));
    write(
        &temp
            .path()
            .join("userdata/fixture/config/grid/98765001.png"),
        "custom cover path",
    );
    let (games, _) = Steam.discover(&cfg);
    let game = games.iter().find(|g| g.id == "steam:98765001").unwrap();
    assert!(game.artwork.ends_with("config/grid/98765001.png"));
}

#[test]
fn modrinth_protocol_allows_launches_and_rejects_install_links() {
    for uri in [
        "modrinth://modpack/example",
        "modrinth://launch/instance/",
        "modrinth://launch/profile/test",
        "modrinth://launch/instance/a/b",
        "modrinth://user@launch/instance/test",
    ] {
        assert!(
            orbit_launcher::platform::validate_game_uri(uri).is_err(),
            "{uri}"
        );
    }
    assert!(
        orbit_launcher::platform::validate_game_uri("modrinth://launch/instance/example").is_ok()
    );
}

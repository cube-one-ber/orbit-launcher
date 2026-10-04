use orbit_launcher::{
    model::*,
    platform::{Os, Platform},
    providers::{self, Heroic, Legendary, Provider},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn write(path: &Path, value: Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}
fn source(path: &Path) -> SourceConfig {
    SourceConfig {
        paths: vec![path.into()],
        ..Default::default()
    }
}
fn installation(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn heroic_discovers_installed_epic_gog_and_amazon_with_store_specific_launches() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("heroic");
    let epic = installation(temp.path(), "Epic game");
    let gog = installation(temp.path(), "GOG game");
    let amazon = installation(temp.path(), "Amazon game");
    let epic_id = "Fixture # ü";
    write(
        &root.join("legendaryConfig/legendary/installed.json"),
        json!({epic_id:{"title":"Epic fixture","install_path":epic,"is_dlc":false}}),
    );
    write(
        &root.join("gog_store/installed.json"),
        json!({"installed":[{"appName":"9876001","install_path":gog,"is_dlc":false}]}),
    );
    write(
        &root.join("store_cache/gog_library.json"),
        json!({"games":[{"app_name":"9876001","title":"GOG fixture","art_background":"https://artwork.example/gog.webp"},{"app_name":"not-installed","title":"Owned only"}]}),
    );
    write(
        &root.join("nile_config/nile/installed.json"),
        json!([{"id":"amazon-fixture","path":amazon}]),
    );
    write(
        &root.join("nile_config/nile/library.json"),
        json!([{"product":{"id":"amazon-fixture","title":"Amazon fixture","productDetail":{"iconUrl":"https://artwork.example/amazon-icon.png","details":{"backgroundUrl1":"https://artwork.example/amazon.jpg"}}}}]),
    );
    // Credentials are outside discovery's read set, even when malformed.
    fs::write(
        root.join("gog_store/auth.json"),
        "invalid credentials fixture",
    )
    .unwrap();
    let before = fs::read(root.join("gog_store/installed.json")).unwrap();
    let mut cfg = source(&root);
    cfg.command = vec!["Heroic AppImage".into(), "--no-gui".into()];
    let (games, errors) = Heroic.discover(&cfg);
    assert!(errors.is_empty(), "{errors:?}");
    for (title, id, runner) in [
        ("Epic fixture", epic_id, "legendary"),
        ("GOG fixture", "9876001", "gog"),
        ("Amazon fixture", "amazon-fixture", "nile"),
    ] {
        let game = games.iter().find(|g| g.title == title).unwrap();
        assert_eq!(&game.command[..2], ["Heroic AppImage", "--no-gui"]);
        let uri = url::Url::parse(&game.command[2]).unwrap();
        assert_eq!(uri.scheme(), "heroic");
        assert_eq!(uri.host_str(), Some("launch"));
        let params: std::collections::HashMap<_, _> = uri.query_pairs().collect();
        assert_eq!(params["appName"], id);
        assert_eq!(params["runner"], runner);
        assert_eq!(params["gui"], "false");
        assert!(game.id.contains(&format!(":{runner}:")));
    }
    assert!(!games.iter().any(|g| g.title == "Owned only"));
    let gog = games.iter().find(|g| g.title == "GOG fixture").unwrap();
    assert_eq!(gog.art.gog_product.as_deref(), Some("9876001"));
    assert_eq!(gog.art.remote, ["https://artwork.example/gog.webp"]);
    let amazon = games.iter().find(|g| g.title == "Amazon fixture").unwrap();
    assert_eq!(amazon.art.remote, ["https://artwork.example/amazon.jpg"]);
    assert_eq!(amazon.art.icon, "https://artwork.example/amazon-icon.png");
    assert_eq!(
        before,
        fs::read(root.join("gog_store/installed.json")).unwrap()
    );
}

#[test]
fn heroic_skips_dlc_missing_and_unfinished_installs_and_reports_bad_rows() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("heroic");
    let good = installation(temp.path(), "Healthy game");
    let partial = installation(temp.path(), "Partial game");
    fs::write(partial.join(".gogdl-resume"), "in progress").unwrap();
    write(
        &root.join("gog_store/installed.json"),
        json!({"installed":[
        {"appName":"healthy","install_path":good},{"appName":"dlc","install_path":good,"is_dlc":true},
        {"appName":"partial","install_path":partial},{"appName":"missing","install_path":temp.path().join("missing")},
        {"appName":"relative","install_path":"relative"},{"appName":"../bad","install_path":good},null]}),
    );
    write(
        &root.join("store_cache/gog_library.json"),
        json!({"games":[{"app_name":"healthy","title":"Healthy fixture","is_installed":false}]}),
    );
    let (games, errors) = Heroic.discover(&source(&root));
    let own: Vec<_> = games
        .iter()
        .filter(|g| {
            g.id.contains("healthy")
                || g.id.contains("partial")
                || g.id.contains("relative")
                || g.id.contains("dlc")
        })
        .collect();
    assert_eq!(own.len(), 1);
    assert_eq!(own[0].title, "Healthy fixture");
    assert!(errors.iter().any(|e| e.contains("absolute")));
    assert!(errors.iter().any(|e| e.contains("Invalid GOG appName")));
}

#[test]
fn heroic_corrupt_store_does_not_hide_other_stores_and_cache_is_optional() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("heroic");
    let game = installation(temp.path(), "Named from folder");
    write(
        &root.join("gog_store/installed.json"),
        json!({"installed":[]}),
    );
    fs::write(root.join("gog_store/installed.json"), "broken JSON").unwrap();
    write(
        &root.join("nile_config/nile/installed.json"),
        json!([{"id":"healthy-amazon","path":game}]),
    );
    let (games, errors) = Heroic.discover(&source(&root));
    assert!(games.iter().any(|g| g.title == "Named from folder"));
    assert!(
        errors
            .iter()
            .any(|e| e.contains("gog_store") && e.contains("installed.json"))
    );
}

#[test]
fn heroic_flatpak_launch_and_root_aliases_preserve_one_entry() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp
        .path()
        .join(".var/app/com.heroicgameslauncher.hgl/config/heroic");
    let game = installation(temp.path(), "Game");
    write(
        &root.join("gog_store/installed.json"),
        json!({"installed":[{"appName":"flatpak-fixture","install_path":game}]}),
    );
    let mut cfg = source(&root);
    cfg.paths.push(root.join("."));
    let (games, errors) = Heroic.discover(&cfg);
    assert!(errors.is_empty(), "{errors:?}");
    let own: Vec<_> = games
        .iter()
        .filter(|g| g.id.ends_with(":gog:flatpak-fixture"))
        .collect();
    assert_eq!(own.len(), 1);
    assert_eq!(
        &own[0].command[..3],
        ["flatpak", "run", "com.heroicgameslauncher.hgl"]
    );
    cfg.command = vec!["portable-heroic".into()];
    let (games, _) = Heroic.discover(&cfg);
    assert_eq!(
        games
            .iter()
            .find(|g| g.id.ends_with(":gog:flatpak-fixture"))
            .unwrap()
            .command[0],
        "portable-heroic"
    );
    let game = games
        .iter()
        .find(|g| g.id.ends_with(":gog:flatpak-fixture"))
        .unwrap();
    assert_eq!(game.command[1], "--no-gui");
    assert!(game.command.last().unwrap().contains("gui=false"));
}

#[test]
fn legendary_preserves_configuration_and_prefers_landscape_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Zoë Legendary config");
    let game = installation(temp.path(), "Game with spaces");
    let file = root.join("installed.json");
    write(
        &file,
        json!({"FixtureApp":{"app_name":"FixtureApp","title":"Legendary fixture","install_path":game}}),
    );
    write(
        &root.join("metadata/FixtureApp.json"),
        json!({"app_name":"FixtureApp","metadata":{"keyImages":[{"type":"DieselGameBoxTall","url":"https://artwork.example/tall.jpg"},{"type":"DieselGameBox","url":"https://artwork.example/wide.jpg"}]}}),
    );
    let before = fs::read(&file).unwrap();
    let mut cfg = source(&root);
    cfg.paths.push(file.clone());
    cfg.command = vec!["custom-legendary".into()];
    let (games, errors) = Legendary.discover(&cfg);
    assert!(errors.is_empty(), "{errors:?}");
    let own: Vec<_> = games
        .iter()
        .filter(|g| g.title == "Legendary fixture")
        .collect();
    assert_eq!(own.len(), 1);
    assert_eq!(
        own[0].command,
        ["custom-legendary", "launch", "--", "FixtureApp"]
    );
    assert_eq!(
        own[0].environment["LEGENDARY_CONFIG_PATH"],
        fs::canonicalize(&root).unwrap().to_string_lossy()
    );
    assert_eq!(
        own[0].art.remote,
        [
            "https://artwork.example/wide.jpg",
            "https://artwork.example/tall.jpg"
        ]
    );
    assert_eq!(before, fs::read(file).unwrap());
}

#[test]
fn legendary_rejects_bad_records_dlc_preloads_and_incomplete_installs() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("legendary");
    let path = installation(temp.path(), "Game");
    write(
        &root.join("installed.json"),
        json!({
            "healthy":{"title":"Healthy Epic fixture","install_path":path},
            "dlc":{"title":"DLC","install_path":path,"is_dlc":true},
            "preload":{"title":"Preload","install_path":path,"is_preloaded":true},
            "partial":{"title":"Partial","install_path":path,"needs_verification":true},
            "UE_5.6":{"title":"Engine","install_path":path},
            "mod":{"title":"Mod","install_path":path},
            "mismatch":{"app_name":"other","install_path":path},
            "../outside":{"install_path":path},"broken":{"install_path":3}
        }),
    );
    write(
        &root.join("metadata/mod.json"),
        json!({"metadata":{"categories":[{"path":"mods"}]}}),
    );
    let (games, errors) = Legendary.discover(&source(&root));
    let canonical = fs::canonicalize(&root)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let own: Vec<_> = games
        .iter()
        .filter(|g| g.environment.get("LEGENDARY_CONFIG_PATH") == Some(&canonical))
        .collect();
    assert_eq!(own.len(), 1);
    assert_eq!(own[0].title, "Healthy Epic fixture");
    assert!(errors.iter().any(|e| e.contains("Invalid Epic app name")));
    assert!(errors.iter().any(|e| e.contains("does not match")));
    assert!(errors.iter().any(|e| e.contains("broken")));
}

#[test]
fn separate_legendary_configurations_have_distinct_stable_ids() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("A");
    let b = temp.path().join("B");
    let path = installation(temp.path(), "Game");
    for root in [&a, &b] {
        write(
            &root.join("installed.json"),
            json!({"same-app":{"title":"Same app fixture","install_path":path}}),
        );
    }
    let mut cfg = source(&a);
    cfg.paths.push(b);
    let (games, _) = Legendary.discover(&cfg);
    let own: Vec<_> = games
        .iter()
        .filter(|g| g.title == "Same app fixture")
        .collect();
    assert_eq!(own.len(), 2);
    assert_ne!(own[0].id, own[1].id);
    assert_ne!(own[0].environment, own[1].environment);
}

#[test]
fn new_sources_migrate_defaults_can_be_disabled_and_reserve_provider_ids() {
    let mut settings: Settings =
        serde_json::from_value(json!({"sources":{"steam":{"enabled":false}}})).unwrap();
    assert!(!settings.sources.contains_key("heroic"));
    for config in settings.sources.values_mut() {
        config.enabled = false;
    }
    for id in [
        "lutris",
        "prism",
        "modrinth",
        "heroic",
        "legendary",
        "epic",
        "gog",
    ] {
        settings.sources.insert(
            id.into(),
            SourceConfig {
                enabled: false,
                ..Default::default()
            },
        );
    }
    let temp = tempfile::tempdir().unwrap();
    write(
        &temp.path().join("providers/collision.json"),
        json!({"id":"heroic","name":"Collision","games":[]}),
    );
    let library = providers::discover(&settings, temp.path());
    for id in ["heroic", "legendary"] {
        assert!(library.providers.iter().any(|p| p.id == id && !p.enabled));
    }
    assert!(
        library
            .providers
            .iter()
            .any(|p| p.errors.iter().any(|e| e.contains("unique")))
    );
    assert!(Settings::default().sources.contains_key("legendary"));
}

#[test]
fn windows_and_linux_launcher_locations_follow_upstream_conventions() {
    let windows = Platform::from_environment(Os::Windows, |key| match key {
        "USERPROFILE" => Some("C:/Users/Zoë Example".into()),
        "APPDATA" => Some("D:/Roaming".into()),
        _ => None,
    });
    assert_eq!(windows.heroic_roots(), [PathBuf::from("D:/Roaming/heroic")]);
    assert!(
        windows
            .legendary_roots()
            .contains(&PathBuf::from("C:/Users/Zoë Example/.config/legendary"))
    );
    let linux = Platform::from_environment(Os::Linux, |key| match key {
        "HOME" => Some("/home/example".into()),
        "XDG_CONFIG_HOME" => Some("/custom/config".into()),
        _ => None,
    });
    assert_eq!(
        linux.heroic_roots()[0],
        PathBuf::from("/custom/config/heroic")
    );
    assert!(
        linux
            .heroic_roots()
            .iter()
            .any(|p| p.ends_with(".var/app/com.heroicgameslauncher.hgl/config/heroic"))
    );
    assert!(
        linux
            .legendary_roots()
            .contains(&PathBuf::from("/custom/config/legendary"))
    );
}

#[cfg(windows)]
#[test]
fn windows_console_launchers_start_without_a_console_window() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("console-handle.txt");
    let mut game: Game = serde_json::from_value(json!({"id":"custom:console-fixture", "title":"Console fixture", "provider":"custom", "subtitle":"", "artwork":""})).unwrap();
    game.command = vec!["powershell.exe".into(), "-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), r#"Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class OrbitConsoleProbe { [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow(); }'; [System.IO.File]::WriteAllText($env:ORBIT_LAUNCH_TEST_OUTPUT, [OrbitConsoleProbe]::GetConsoleWindow().ToInt64().ToString())"#.into()];
    game.environment.insert(
        "ORBIT_LAUNCH_TEST_OUTPUT".into(),
        output.to_string_lossy().into_owned(),
    );
    providers::launch(&game).unwrap();
    for _ in 0..200 {
        if output.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(
        fs::read_to_string(output).unwrap(),
        "0",
        "Console launcher should have no console window"
    );
}

#[cfg(unix)]
#[test]
fn legendary_dispatch_uses_the_discovered_configuration_without_changing_process_environment() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Zoë config with spaces");
    let path = installation(temp.path(), "Game");
    write(
        &root.join("installed.json"),
        json!({"--fixture-app":{"title":"Dispatch fixture","install_path":path}}),
    );
    let script = temp.path().join("capture");
    let output = temp.path().join("capture-output");
    fs::write(&script,"#!/bin/sh\nprintf '%s\\n' \"$LEGENDARY_CONFIG_PATH\" \"$@\" > \"$ORBIT_LAUNCH_TEST_OUTPUT\"\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let original = std::env::var_os("LEGENDARY_CONFIG_PATH");
    let mut cfg = source(&root);
    cfg.command = vec![script.display().to_string()];
    let (games, _) = Legendary.discover(&cfg);
    let mut game = games
        .into_iter()
        .find(|g| g.title == "Dispatch fixture")
        .unwrap();
    game.environment.insert(
        "ORBIT_LAUNCH_TEST_OUTPUT".into(),
        output.display().to_string(),
    );
    providers::launch(&game).unwrap();
    for _ in 0..100 {
        if output.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let result = fs::read_to_string(output).unwrap();
    assert_eq!(
        result,
        format!(
            "{}\nlaunch\n--\n--fixture-app\n",
            fs::canonicalize(root).unwrap().display()
        )
    );
    assert_eq!(std::env::var_os("LEGENDARY_CONFIG_PATH"), original);
}

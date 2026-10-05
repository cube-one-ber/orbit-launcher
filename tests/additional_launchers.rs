use orbit_launcher::{
    model::{Settings, SourceConfig},
    platform::{Os, Platform},
    providers::{self, ATLauncher, MultiMC, Nile, PolyMC, Provider},
};
use serde_json::json;
use std::{fs, path::Path};

fn write(path: &Path, content: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn config(root: &Path) -> SourceConfig {
    SourceConfig {
        paths: vec![root.into()],
        command: vec!["fixture-launcher".into()],
        ..Default::default()
    }
}
fn mc_instance(path: &Path, title: &str) {
    write(
        &path.join("instance.cfg"),
        format!(
            "[General]\nname={title}\niconKey=custom\nManagedPackType=modrinth\nManagedPackID=fixture-project\nlastLaunchTime=1700000000000\n"
        ),
    );
    write(
        &path.join("mmc-pack.json"),
        json!({"components":[{"uid":"net.minecraft","version":"1.21.5"}]}).to_string(),
    );
}
fn at_instance(path: &Path, title: &str) {
    write(
        &path.join("instance.json"),
        json!({"id":"1.21.5","launcher":{"name":title,"modrinthProject":{"id":"fixture-project"}}})
            .to_string(),
    );
}
fn amazon(root: &Path, game: &Path, id: &str) {
    write(&game.join("fuel.json"), "{}");
    write(
        &root.join("installed.json"),
        json!([{"id":id,"path":game}]).to_string(),
    );
    write(&root.join("library.json"), json!([{"id":"entitlement-id","product":{"id":id,"title":"Amazon fixture","productDetail":{"details":{"backgroundUrl1":"https://example.com/cover.jpg"}}}}]).to_string());
}

#[test]
fn multimc_and_polymc_follow_custom_directories_and_preserve_metadata_read_only() {
    let temp = tempfile::tempdir().unwrap();
    for (provider, settings_file) in [
        (&MultiMC as &dyn Provider, "multimc.cfg"),
        (&PolyMC as &dyn Provider, "polymc.cfg"),
    ] {
        let root = temp.path().join(provider.id());
        let instance = root.join("custom instances/Zoë world");
        mc_instance(&instance, "Fixture world");
        write(
            &root.join(settings_file),
            "[General]\nInstanceDir=custom instances\nIconsDir=custom icons\n",
        );
        write(&root.join("custom icons/custom.png"), []);
        write(&instance.join("cover.webp"), []);
        let before = fs::read(instance.join("instance.cfg")).unwrap();
        let mut source = config(&root);
        source.paths.push(root.clone());
        let (games, errors) = provider.discover(&source);
        assert!(errors.is_empty(), "{errors:?}");
        let matches: Vec<_> = games
            .iter()
            .filter(|g| g.title == "Fixture world")
            .collect();
        assert_eq!(matches.len(), 1);
        let game = matches[0];
        assert_eq!(game.provider, provider.id());
        assert!(game.id.starts_with(&format!("{}:file:", provider.id())));
        assert_eq!(
            game.command,
            [
                "fixture-launcher",
                "--dir",
                root.to_str().unwrap(),
                "--launch",
                "Zoë world"
            ]
        );
        assert_eq!(game.last_played, 1700000000);
        assert_eq!(game.art.minecraft_version.as_deref(), Some("1.21.5"));
        assert_eq!(
            game.art.modrinth_project.as_deref(),
            Some("fixture-project")
        );
        assert!(game.art.icon.ends_with("custom.png"));
        assert!(game.artwork.ends_with("cover.webp"));
        assert_eq!(fs::read(instance.join("instance.cfg")).unwrap(), before);
    }
}

#[test]
fn polymc_flatpak_launches_directly_and_command_override_wins() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join(".var/app/org.polymc.PolyMC/data/PolyMC");
    mc_instance(&root.join("instances/Creative world"), "Flatpak fixture");
    let mut source = config(&root);
    source.command.clear();
    let (games, _) = PolyMC.discover(&source);
    let game = games.iter().find(|g| g.title == "Flatpak fixture").unwrap();
    assert_eq!(&game.command[..3], ["flatpak", "run", "org.polymc.PolyMC"]);
    assert_eq!(game.command.last().unwrap(), "Creative world");
    source.command = vec!["/portable launcher/PolyMC.AppImage".into()];
    let (games, _) = PolyMC.discover(&source);
    assert_eq!(
        games
            .iter()
            .find(|g| g.title == "Flatpak fixture")
            .unwrap()
            .command[0],
        source.command[0]
    );
}

#[test]
fn malformed_and_oversized_minecraft_metadata_do_not_hide_other_instances() {
    let temp = tempfile::tempdir().unwrap();
    mc_instance(&temp.path().join("instances/healthy"), "Healthy fixture");
    mc_instance(&temp.path().join("instances/broken"), "Broken pack fixture");
    write(
        &temp.path().join("instances/broken/mmc-pack.json"),
        "invalid json",
    );
    write(
        &temp.path().join("instances/oversized/instance.cfg"),
        vec![b'x'; 1024 * 1024 + 1],
    );
    let (games, errors) = MultiMC.discover(&config(temp.path()));
    assert!(games.iter().any(|g| g.title == "Healthy fixture"));
    assert!(
        !games
            .iter()
            .any(|g| g.command.last().is_some_and(|id| id == "oversized"))
    );
    assert!(errors.iter().any(|e| e.contains("mmc-pack.json")));
    assert!(errors.iter().any(|e| e.contains("exceeds 1 MiB")));
}

#[cfg(unix)]
#[test]
fn shared_instance_parser_preserves_prism_ids_and_symlink_launch_names() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("data");
    let actual = temp.path().join("actual world");
    mc_instance(&actual, "Existing Prism fixture");
    fs::create_dir_all(root.join("instances")).unwrap();
    let instance = root.join("instances/Alias world");
    std::os::unix::fs::symlink(&actual, &instance).unwrap();
    let (games, errors) = providers::Prism.discover(&config(&root));
    assert!(errors.is_empty(), "{errors:?}");
    let game = games
        .iter()
        .find(|g| g.title == "Existing Prism fixture")
        .unwrap();
    assert_eq!(
        game.id,
        format!("prism:{}", url::Url::from_file_path(&instance).unwrap())
    );
    assert_eq!(game.command.last().unwrap(), "Alias world");
}

#[test]
fn atlauncher_uses_instance_name_and_data_folder_as_separate_literal_arguments() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("ATLauncher data");
    let instance = root.join("instances/Adventure");
    at_instance(&instance, "Zoë's Adventure & friends");
    write(&instance.join("instance.png"), []);
    write(&root.join("instances/broken/instance.json"), "bad JSON");
    write(&root.join("instances/unfinished/instance.json"), "{}");
    let before = fs::read(instance.join("instance.json")).unwrap();
    let (games, errors) = ATLauncher.discover(&config(&root));
    let game = games
        .iter()
        .find(|g| g.title == "Zoë's Adventure & friends")
        .unwrap();
    let root = fs::canonicalize(root).unwrap();
    assert_eq!(
        game.command,
        [
            "fixture-launcher".into(),
            format!("--working-dir={}", root.display()),
            "--launch=Zoë's Adventure & friends".into()
        ]
    );
    assert_eq!(game.art.minecraft_version.as_deref(), Some("1.21.5"));
    assert_eq!(
        game.art.modrinth_project.as_deref(),
        Some("fixture-project")
    );
    assert!(game.artwork.ends_with("instance.png"));
    assert!(errors.iter().any(|e| e.contains("broken")));
    assert!(errors.iter().any(|e| e.contains("launcher.name")));
    assert_eq!(before, fs::read(instance.join("instance.json")).unwrap());
}

#[test]
fn atlauncher_flatpak_preserves_arguments_by_using_the_bundled_java() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join(".var/app/com.atlauncher.ATLauncher/data");
    at_instance(&root.join("instances/Adventure"), "Adventure with spaces");
    let mut source = config(&root);
    source.command.clear();
    let (games, _) = ATLauncher.discover(&source);
    let game = games
        .iter()
        .find(|g| g.title == "Adventure with spaces")
        .unwrap();
    assert_eq!(
        &game.command[..6],
        [
            "flatpak",
            "run",
            "--command=java",
            "com.atlauncher.ATLauncher",
            "-jar",
            "/app/bin/ATLauncher.jar"
        ]
    );
    assert_eq!(
        game.command.last().unwrap(),
        "--launch=Adventure with spaces"
    );
}

#[test]
fn nile_keeps_configuration_identity_artwork_and_runtime_override() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("profile with spaces/nile");
    let game_dir = temp.path().join("Amazon game");
    amazon(&root, &game_dir, "-fixture-id");
    let before = fs::read(root.join("installed.json")).unwrap();
    let mut source = config(&root);
    source.paths.push(root.join("installed.json"));
    source.command = vec![
        "fixture-nile".into(),
        "launch".into(),
        "--wine-prefix".into(),
        "prefix with spaces".into(),
    ];
    let (games, errors) = Nile.discover(&source);
    assert!(errors.is_empty(), "{errors:?}");
    let matches: Vec<_> = games
        .iter()
        .filter(|g| g.title == "Amazon fixture")
        .collect();
    assert_eq!(matches.len(), 1);
    let game = matches[0];
    assert_eq!(
        game.command,
        [
            "fixture-nile",
            "launch",
            "--wine-prefix",
            "prefix with spaces",
            "--",
            "-fixture-id"
        ]
    );
    assert_eq!(
        game.environment["NILE_CONFIG_PATH"],
        root.parent().unwrap().to_str().unwrap()
    );
    assert_eq!(game.art.remote, ["https://example.com/cover.jpg"]);
    assert_eq!(before, fs::read(root.join("installed.json")).unwrap());
    let (again, _) = Nile.discover(&source);
    assert_eq!(
        game.id,
        again
            .iter()
            .find(|g| g.title == "Amazon fixture")
            .unwrap()
            .id
    );
    let another = temp.path().join("another/nile");
    amazon(&another, &game_dir, "-fixture-id");
    let (other, _) = Nile.discover(&config(&another));
    assert_ne!(
        game.id,
        other
            .iter()
            .find(|g| g.title == "Amazon fixture")
            .unwrap()
            .id
    );
}

#[test]
fn nile_skips_missing_unlaunchable_and_invalid_records_independently() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("nile");
    let game_dir = temp.path().join("healthy");
    amazon(&root, &game_dir, "healthy");
    let unfinished = temp.path().join("unfinished");
    fs::create_dir_all(&unfinished).unwrap();
    write(&root.join("installed.json"), json!([
        {"id":"healthy","path":game_dir}, {"id":"healthy","path":game_dir},
        {"id":"unfinished","path":unfinished}, {"id":"no-library-entry","path":game_dir},
        {"id":"missing","path":temp.path().join("missing")}, {"id":"../invalid","path":game_dir},
        {"id":"relative","path":"relative/path"}
    ]).to_string());
    let mut library: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("library.json")).unwrap()).unwrap();
    library
        .as_array_mut()
        .unwrap()
        .push(json!({"product":{"id":"unfinished","title":"Incomplete fixture"}}));
    write(&root.join("library.json"), library.to_string());
    let (games, errors) = Nile.discover(&config(&root));
    assert_eq!(
        games.iter().filter(|g| g.title == "Amazon fixture").count(),
        1
    );
    assert!(!games.iter().any(|g| g.title == "Incomplete fixture"));
    assert!(errors.iter().any(|e| e.contains("Invalid Amazon")));
    assert!(errors.iter().any(|e| e.contains("must be absolute")));
    assert!(errors.iter().any(|e| e.contains("nile library sync")));
    write(&root.join("installed.json"), "bad JSON");
    let (_, errors) = Nile.discover(&config(&root));
    assert!(errors.iter().any(|e| e.contains("installed.json")));
}

#[test]
fn nile_rejects_configuration_names_the_upstream_cli_cannot_select() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("renamed");
    amazon(&root, &temp.path().join("game"), "id");
    let (games, errors) = Nile.discover(&config(&root));
    assert!(!games.iter().any(|g| g.title == "Amazon fixture"));
    assert!(errors.iter().any(|e| e.contains("must be named nile")));
    #[cfg(unix)]
    {
        // A logical nile folder can point to a differently named storage folder.
        let link = temp.path().join("nile");
        std::os::unix::fs::symlink(&root, &link).unwrap();
        let mut source = config(&link);
        source.paths.insert(0, root.clone());
        let (games, errors) = Nile.discover(&source);
        assert!(errors.iter().any(|e| e.contains("must be named nile")));
        let game = games.iter().find(|g| g.title == "Amazon fixture").unwrap();
        assert_eq!(
            game.environment["NILE_CONFIG_PATH"],
            temp.path().to_str().unwrap()
        );
    }
}

#[test]
fn additional_sources_can_be_disabled_in_old_settings_and_ids_are_reserved() {
    let temp = tempfile::tempdir().unwrap();
    let mut settings: Settings = serde_json::from_value(json!({"sources":{}})).unwrap();
    for id in Settings::default().sources.keys() {
        settings.sources.insert(
            id.clone(),
            SourceConfig {
                enabled: false,
                ..Default::default()
            },
        );
    }
    for id in ["multimc", "polymc", "atlauncher", "nile"] {
        assert!(Settings::default().sources.contains_key(id));
        write(
            &temp.path().join(format!("providers/{id}.json")),
            json!({"id":id,"name":"Collision","games":[]}).to_string(),
        );
    }
    let library = providers::discover(&settings, temp.path());
    assert!(library.games.is_empty());
    assert_eq!(
        library
            .providers
            .iter()
            .filter(|p| p.errors.iter().any(|e| e.contains("unique")))
            .count(),
        4
    );
    for id in ["multimc", "polymc", "atlauncher", "nile"] {
        assert!(library.providers.iter().any(|p| p.id == id && !p.enabled));
    }
}

#[test]
fn portable_executable_resolution_and_windows_data_paths_are_testable_on_linux() {
    let temp = tempfile::tempdir().unwrap();
    let windows = Platform::from_environment(Os::Windows, |key| match key {
        "APPDATA" => Some(temp.path().join("Roaming")),
        _ => None,
    });
    for binary in ["MultiMC.exe", "polymc.exe", "ATLauncher.exe"] {
        write(&temp.path().join(binary), []);
    }
    assert_eq!(
        windows.multimc_command(temp.path()),
        temp.path().join("MultiMC.exe").to_str().unwrap()
    );
    assert_eq!(
        windows.polymc_command(temp.path()),
        temp.path().join("polymc.exe").to_str().unwrap()
    );
    assert_eq!(
        windows.atlauncher_command(temp.path()),
        [temp.path().join("ATLauncher.exe").to_str().unwrap()]
    );
    assert_eq!(
        windows.polymc_roots()[0],
        temp.path().join("Roaming/PolyMC")
    );
    let linux = Platform::from_environment(Os::Linux, |_| None);
    write(&temp.path().join("ATLauncher.jar"), []);
    assert_eq!(
        linux.atlauncher_command(temp.path()),
        [
            "java",
            "-jar",
            temp.path().join("ATLauncher.jar").to_str().unwrap()
        ]
    );
    for provider in [&MultiMC as &dyn Provider, &PolyMC, &ATLauncher, &Nile] {
        assert_eq!(
            provider.available(),
            cfg!(any(target_os = "linux", windows))
        );
    }
}

#[cfg(unix)]
#[test]
fn nile_dispatch_sets_only_the_child_configuration_and_preserves_arguments() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("profile/nile");
    amazon(&root, &temp.path().join("game"), "id with spaces");
    let output = temp.path().join("output");
    let script = temp.path().join("capture");
    write(
        &script,
        "#!/bin/sh\nout=$1\nshift\nprintf '%s\\n' \"$NILE_CONFIG_PATH\" \"$@\" > \"$out\"\n",
    );
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let mut source = config(&root);
    source.command = vec![
        script.to_str().unwrap().into(),
        output.to_str().unwrap().into(),
    ];
    let env_before = std::env::var_os("NILE_CONFIG_PATH");
    let (games, _) = Nile.discover(&source);
    let game = games.iter().find(|g| g.title == "Amazon fixture").unwrap();
    providers::launch(game).unwrap();
    let expected = format!(
        "{}\nlaunch\n--\nid with spaces\n",
        root.parent().unwrap().display()
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if fs::read_to_string(&output).is_ok_and(|s| s == expected) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Fixture dispatch did not complete"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(std::env::var_os("NILE_CONFIG_PATH"), env_before);
}

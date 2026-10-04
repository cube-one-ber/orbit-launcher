use orbit_launcher::{
    model::*,
    platform::{Os, Platform, validate_game_uri},
    providers::{self, BattleNet, Provider, Ubisoft},
};
use prost::Message;
use std::{fs, path::Path};

#[derive(Message)]
struct Database {
    #[prost(message, repeated, tag = "1")]
    products: Vec<Product>,
}
#[derive(Message)]
struct Product {
    #[prost(string, tag = "1")]
    internal: String,
    #[prost(string, tag = "2")]
    id: String,
    #[prost(message, optional, tag = "3")]
    data: Option<Data>,
}
#[derive(Message)]
struct Data {
    #[prost(string, tag = "1")]
    path: String,
}
fn product(internal: &str, id: &str, path: &Path) -> Product {
    Product {
        internal: internal.into(),
        id: id.into(),
        data: Some(Data {
            path: path.to_string_lossy().into_owned(),
        }),
    }
}
fn config(path: &Path) -> SourceConfig {
    SourceConfig {
        paths: vec![path.into()],
        command: vec!["fixture launcher with spaces.exe".into()],
        ..Default::default()
    }
}
#[test]
fn battlenet_reads_real_protobuf_shape_and_dispatches_known_products_only() {
    let temp = tempfile::tempdir().unwrap();
    let game = temp.path().join("Zoë game");
    fs::create_dir(&game).unwrap();
    let file = temp.path().join("product.db");
    let db = Database {
        products: vec![
            product("wow", "WoW", &game),
            product("wow", "WoW", &game),
            product("prometheus", "Pro", &game),
            product("fenris", "Fen", &game),
            product("hs_beta", "WTCG", &game),
            product("wow_classic", "WoW", &game),
            product("wow_beta", "WoW", &game),
            product("agent", "agent", &game),
            product("s2", "S2", &temp.path().join("missing")),
            product("w3", "W3", Path::new("relative")),
        ],
    };
    fs::write(&file, db.encode_to_vec()).unwrap();
    let before = fs::read(&file).unwrap();
    let mut cfg = config(&file);
    cfg.paths.push(temp.path().into());
    let (games, errors) = BattleNet.discover(&cfg);
    assert_eq!(games.len(), 4);
    assert!(errors.iter().any(|e| e.contains("absolute")));
    for (id, title) in [
        ("WoW", "World of Warcraft"),
        ("Pro", "Overwatch 2"),
        ("Fen", "Diablo IV"),
        ("WTCG", "Hearthstone"),
    ] {
        let g = games
            .iter()
            .find(|g| g.id == format!("battlenet:{id}"))
            .unwrap();
        assert_eq!(g.title, title);
        assert_eq!(
            g.command,
            [
                "fixture launcher with spaces.exe".to_owned(),
                format!("--exec=launch {id}")
            ]
        );
        assert!(g.launch_uri.is_none());
    }
    assert!(games.iter().any(|g| !g.art.remote.is_empty()));
    assert_eq!(before, fs::read(file).unwrap());
}
#[test]
fn battlenet_reports_corrupt_or_oversized_metadata_without_hiding_other_roots() {
    let temp = tempfile::tempdir().unwrap();
    let good = temp.path().join("product.db");
    fs::write(
        &good,
        Database {
            products: vec![product("diablo3", "D3", temp.path())],
        }
        .encode_to_vec(),
    )
    .unwrap();
    let broken = temp.path().join("bad.db");
    fs::write(&broken, [0xff, 0xff]).unwrap();
    let huge = temp.path().join("huge.db");
    fs::File::create(&huge)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    let mut cfg = config(&broken);
    cfg.paths.extend([good, huge]);
    let (games, errors) = BattleNet.discover(&cfg);
    assert_eq!(games.len(), 1);
    assert_eq!(errors.len(), 2);
    assert!(errors.iter().any(|e| e.contains("16 MiB")));
}
#[derive(Message)]
struct UbisoftState {
    #[prost(string, tag = "22")]
    title: String,
    #[prost(uint64, tag = "28")]
    id: u64,
}
#[derive(Message)]
struct UbisoftTextState {
    #[prost(string, tag = "22")]
    title: String,
    #[prost(string, tag = "28")]
    id: String,
}
#[test]
fn ubisoft_imports_install_markers_from_libraries_and_direct_game_folders() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("Folder title");
    let b = temp.path().join("Text ID");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let file = a.join("uplay_install.state");
    fs::write(
        &file,
        UbisoftState {
            title: "Assassin's Creed fixture".into(),
            id: 500123,
        }
        .encode_to_vec(),
    )
    .unwrap();
    fs::write(
        b.join("uplay_install.state"),
        UbisoftTextState {
            title: "Text fixture".into(),
            id: "500124".into(),
        }
        .encode_to_vec(),
    )
    .unwrap();
    let before = fs::read(&file).unwrap();
    let mut cfg = config(temp.path());
    cfg.paths.push(a);
    let (games, errors) = Ubisoft.discover(&cfg);
    assert!(errors.is_empty(), "{errors:?}");
    let own: Vec<_> = games
        .iter()
        .filter(|g| g.id.starts_with("ubisoft:50012"))
        .collect();
    assert_eq!(own.len(), 2);
    assert_eq!(
        own.iter().find(|g| g.id == "ubisoft:500123").unwrap().title,
        "Assassin's Creed fixture"
    );
    for g in own {
        assert_eq!(
            g.command[1],
            format!(
                "uplay://launch/{}/0",
                g.id.strip_prefix("ubisoft:").unwrap()
            )
        );
        assert!(g.launch_uri.is_none());
    }
    cfg.command.clear();
    let (games, _) = Ubisoft.discover(&cfg);
    assert!(
        games
            .iter()
            .filter(|g| g.id.starts_with("ubisoft:50012"))
            .all(|g| g.launch_uri.is_some() && g.command.is_empty())
    );
    assert_eq!(before, fs::read(file).unwrap());
}
#[test]
fn ubisoft_rejects_bad_ids_and_corrupt_markers_independently() {
    let temp = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        ("bad", vec![0xff]),
        (
            "zero",
            UbisoftState {
                title: "Zero".into(),
                id: 0,
            }
            .encode_to_vec(),
        ),
        (
            "injection",
            UbisoftTextState {
                title: "Invalid".into(),
                id: "123/0?arg=1".into(),
            }
            .encode_to_vec(),
        ),
        (
            "valid",
            UbisoftState {
                title: "Valid fixture".into(),
                id: 500125,
            }
            .encode_to_vec(),
        ),
    ] {
        let dir = temp.path().join(name);
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("uplay_install.state"), bytes).unwrap();
    }
    let (games, errors) = Ubisoft.discover(&config(temp.path()));
    assert!(games.iter().any(|g| g.id == "ubisoft:500125"));
    assert_eq!(errors.len(), 3);
}
#[test]
fn ubisoft_protocol_accepts_only_direct_game_launches() {
    assert!(validate_game_uri("uplay://launch/123/0").is_ok());
    for uri in [
        "uplay://install/123",
        "uplay://launch/0/0",
        "uplay://launch/123/1",
        "uplay://launch/123/0/extra",
        "uplay://launch/123/0?arg=1",
        "uplay://user@launch/123/0",
        "uplay://launch/%31/0",
        "uplay://launch/123/0#x",
    ] {
        assert!(validate_game_uri(uri).is_err(), "{uri}");
    }
}
#[test]
fn common_source_defaults_migrate_reserve_ids_and_follow_platform_paths() {
    let temp = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    for cfg in settings.sources.values_mut() {
        cfg.enabled = false;
    }
    for id in ["battlenet", "ubisoft", "itch"] {
        assert!(settings.sources.contains_key(id));
        fs::create_dir_all(temp.path().join("providers")).unwrap();
        fs::write(
            temp.path().join(format!("providers/{id}.json")),
            serde_json::to_vec(&serde_json::json!({"id":id,"name":"collision","games":[]}))
                .unwrap(),
        )
        .unwrap();
    }
    let library = providers::discover(&settings, temp.path());
    assert_eq!(
        library
            .providers
            .iter()
            .filter(|p| p.errors.iter().any(|e| e.contains("unique")))
            .count(),
        3
    );
    for id in ["battlenet", "ubisoft", "itch"] {
        assert!(library.providers.iter().any(|p| p.id == id && !p.enabled));
    }
    let windows = Platform::from_environment(Os::Windows, |key| match key {
        "APPDATA" => Some("D:/Roaming".into()),
        _ => None,
    });
    assert_eq!(
        windows.itch_roots()[0],
        std::path::PathBuf::from("D:/Roaming/itch")
    );
    let linux = Platform::from_environment(Os::Linux, |key| match key {
        "XDG_CONFIG_HOME" => Some("/tmp/config".into()),
        _ => None,
    });
    assert_eq!(
        linux.itch_roots()[1],
        std::path::PathBuf::from("/tmp/config/kitch")
    );
}

#[test]
fn common_launcher_executables_resolve_without_path() {
    let temp = tempfile::tempdir().unwrap();
    let mut platform = Platform::from_environment(Os::Windows, |_| None);
    platform.program_files = vec![temp.path().join("Programs")];
    platform.local = temp.path().join("Local");
    let battle = platform.program_files[0].join("Battle.net/Battle.net.exe");
    let itch = platform.local.join("itch/itch-setup.exe");
    for file in [&battle, &itch] {
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, []).unwrap();
    }
    assert_eq!(
        fs::canonicalize(platform.battlenet_command()).unwrap(),
        fs::canonicalize(&battle).unwrap()
    );
    assert_eq!(
        fs::canonicalize(platform.itch_command("itch")).unwrap(),
        fs::canonicalize(&itch).unwrap()
    );
    platform.os = Os::Linux;
    platform.home = temp.path().into();
    let linux = platform.home.join(".kitch/itch-setup");
    fs::create_dir_all(linux.parent().unwrap()).unwrap();
    fs::write(&linux, []).unwrap();
    assert_eq!(
        fs::canonicalize(platform.itch_command("kitch")).unwrap(),
        fs::canonicalize(&linux).unwrap()
    );
}

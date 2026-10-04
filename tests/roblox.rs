use orbit_launcher::{
    model::SourceConfig,
    platform::{Os, Platform, validate_game_uri},
    providers::{Provider, Roblox},
};
use serde_json::{Value, json};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn report() -> Value {
    json!({"version":1,"exported_at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),"user_id":12345,"username":"Example_Player",
        "games":(1..=8).map(|n|json!({"universe_id":n,"place_id":100+n,"title":format!("Game {n}"),"weekly_minutes":n*60})).collect::<Vec<_>>()})
}
fn discover(value: Value, command: Vec<String>) -> (Vec<orbit_launcher::model::Game>, Vec<String>) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("weekly report.json");
    fs::write(&path, value.to_string()).unwrap();
    let before = fs::read(&path).unwrap();
    let result = Roblox.discover(&SourceConfig {
        paths: vec![path.clone()],
        command,
        ..Default::default()
    });
    assert_eq!(
        fs::read(path).unwrap(),
        before,
        "Reports belong to the browser"
    );
    result
}
#[test]
fn top_five_use_personal_playtime_and_stable_account_ids() {
    let (games, errors) = discover(report(), vec![]);
    assert!(errors.is_empty());
    assert_eq!(games.len(), 5);
    assert_eq!(games[0].id, "roblox:12345:8");
    assert_eq!(games[4].title, "Game 4");
    assert_eq!(games[0].subtitle, "#1 · 8h 0m last week · @Example_Player");
    assert_eq!(games[0].source_rank, 1);
    assert_eq!(games[4].source_rank, 5);
    assert_eq!(
        games[0].launch_uri.as_deref(),
        Some("roblox://experiences/start?placeId=108")
    );
    assert_eq!(games[0].art.roblox_universe, Some(8));
    let mut other = report();
    other["user_id"] = json!(6789);
    let (other, _) = discover(other, vec![]);
    assert_ne!(other[0].id, games[0].id);
}
#[test]
fn duplicate_invalid_and_unplayed_experiences_do_not_fill_top_five() {
    let mut value = report();
    let games = value["games"].as_array_mut().unwrap();
    games.push(games[7].clone());
    games.push(json!({"universe_id":9,"place_id":109,"title":"Never played","weekly_minutes":0}));
    games
        .push(json!({"universe_id":10,"place_id":0,"title":"Invalid place","weekly_minutes":1000}));
    games.push(
        json!({"universe_id":11,"place_id":111,"title":"Impossible time","weekly_minutes":20000}),
    );
    let (games, errors) = discover(value, vec![]);
    assert_eq!(games.len(), 5);
    assert_eq!(games[1].title, "Game 7");
    assert!(errors.is_empty());
}
#[test]
fn command_override_receives_only_a_generated_game_join_uri() {
    let (games, _) = discover(
        report(),
        vec!["flatpak".into(), "run".into(), "org.vinegarhq.Sober".into()],
    );
    assert!(games[0].launch_uri.is_none());
    assert_eq!(
        games[0].command,
        [
            "flatpak",
            "run",
            "org.vinegarhq.Sober",
            "roblox://experiences/start?placeId=108"
        ]
    );
}
#[test]
fn malformed_future_and_credential_containing_reports_are_rejected() {
    for (key, value) in [
        ("version", json!(2)),
        ("exported_at", json!(u64::MAX)),
        ("username", json!("\0")),
        ("cookie", json!("never-read-this")),
    ] {
        let mut input = report();
        input[key] = value;
        let (games, errors) = discover(input, vec![]);
        assert!(games.is_empty());
        assert!(!errors.is_empty());
        assert!(!errors.join(" ").contains("never-read-this"));
    }
    let mut input = report();
    input["games"][0]["command"] = json!(["untrusted"]);
    assert!(discover(input, vec![]).0.is_empty());
}
#[test]
fn stale_and_empty_reports_are_honest_and_missing_files_are_actionable() {
    let mut input = report();
    input["exported_at"] = json!(1);
    let (games, errors) = discover(input, vec![]);
    assert_eq!(games.len(), 5);
    assert!(errors[0].contains("last sync"));
    let mut input = report();
    input["games"] = json!([]);
    let (games, errors) = discover(input, vec![]);
    assert!(games.is_empty());
    assert!(errors.is_empty());
    let temp = tempfile::tempdir().unwrap();
    let config = SourceConfig {
        paths: vec![temp.path().join("missing.json")],
        ..Default::default()
    };
    assert!(Roblox.discover(&config).1[0].contains("Could not read"));
    fs::write(
        temp.path().join("oversized.json"),
        vec![b' '; 256 * 1024 + 1],
    )
    .unwrap();
    assert!(
        Roblox
            .discover(&SourceConfig {
                paths: vec![temp.path().join("oversized.json")],
                ..Default::default()
            })
            .1[0]
            .contains("256 KiB")
    );
    assert!(
        Roblox
            .discover(&SourceConfig {
                paths: vec![temp.path().into(), temp.path().into()],
                ..Default::default()
            })
            .1[0]
            .contains("one Roblox report")
    );
}
#[test]
fn roblox_protocol_accepts_only_one_positive_place_id() {
    assert!(validate_game_uri("roblox://experiences/start?placeId=1818").is_ok());
    for uri in [
        "roblox://navigation/home",
        "roblox://experiences/start?placeId=0",
        "roblox://experiences/start?placeId=-1",
        "roblox://experiences/start?placeId=12&placeId=13",
        "roblox://experiences/start?placeId=12&auth=secret",
        "roblox://experiences/start?placeId=12#fragment",
        "roblox://user@experiences/start?placeId=12",
        "roblox://experiences:20/start?placeId=12",
        "roblox://experiences/start?placeId=12%0A",
        "roblox://experiences/start?placeId=18446744073709551616",
    ] {
        assert!(validate_game_uri(uri).is_err(), "{uri}");
    }
}
#[test]
fn download_folder_detection_respects_xdg_and_windows_profiles() {
    let temp = tempfile::tempdir().unwrap();
    let mut platform = Platform::from_environment(Os::Windows, |_| None);
    platform.home = temp.path().join("Zoë Profile");
    assert_eq!(platform.downloads_dir(), platform.home.join("Downloads"));
    platform.os = Os::Linux;
    platform.config_home = temp.path().into();
    fs::write(
        temp.path().join("user-dirs.dirs"),
        "XDG_DOWNLOAD_DIR=\"$HOME/My downloads\"\n",
    )
    .unwrap();
    assert_eq!(platform.downloads_dir(), platform.home.join("My downloads"));
    fs::write(
        temp.path().join("user-dirs.dirs"),
        "XDG_DOWNLOAD_DIR=\"relative\"\n",
    )
    .unwrap();
    assert_eq!(platform.downloads_dir(), platform.home.join("Downloads"));
}

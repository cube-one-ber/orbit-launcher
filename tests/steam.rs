use orbit_launcher::{
    model::*,
    providers::{Provider, Steam},
};
use std::{fs, path::Path};

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
fn config(root: &Path) -> SourceConfig {
    SourceConfig {
        paths: vec![root.into()],
        command: vec!["fixture-steam".into()],
        ..Default::default()
    }
}
fn text(bytes: &mut Vec<u8>, key: &str, value: &str) {
    bytes.push(1);
    bytes.extend(key.bytes());
    bytes.push(0);
    bytes.extend(value.bytes());
    bytes.push(0);
}
fn number(bytes: &mut Vec<u8>, key: &str, value: u32) {
    bytes.push(2);
    bytes.extend(key.bytes());
    bytes.push(0);
    bytes.extend(value.to_le_bytes());
}
fn shortcuts(id: u32, title: &str, icon: &str, hidden: bool) -> Vec<u8> {
    let mut bytes = b"\0shortcuts\0\x000\0".to_vec();
    number(&mut bytes, "appid", id);
    text(&mut bytes, "AppName", title);
    text(&mut bytes, "Exe", "\"/games/Zoë's adventure/game.exe\"");
    text(&mut bytes, "StartDir", "\"/games/Zoë's adventure\"");
    text(
        &mut bytes,
        "LaunchOptions",
        "--flag %command%; $(touch should-not-run)",
    );
    text(&mut bytes, "icon", icon);
    number(&mut bytes, "IsHidden", u32::from(hidden));
    number(&mut bytes, "LastPlayTime", 1_750_000_001);
    // Real records also contain nested tag objects.
    bytes.extend(b"\0tags\0");
    text(&mut bytes, "0", "Favorite");
    bytes.extend([8, 8, 8, 8]);
    bytes
}
fn manifest(root: &Path, id: u32, title: &str, fields: &str) {
    write(
        &root.join(format!("steamapps/appmanifest_{id}.acf")),
        format!("\"AppState\" {{ \"appid\" \"{id}\" \"name\" \"{title}\" {fields} }}"),
    );
}
#[test]
fn shortcuts_preserve_unsigned_ids_steam_settings_artwork_and_history() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("userdata/1/config/shortcuts.vdf");
    let cover = temp.path().join("userdata/1/config/grid/4063516280.png");
    write(&cover, "cover");
    let bytes = shortcuts(0xf234_5678, "Fixture non-Steam adventure", "", false);
    write(&file, &bytes);
    let (games, errors) = Steam.discover(&config(temp.path()));
    assert!(
        errors
            .iter()
            .all(|error| !error.contains(temp.path().to_str().unwrap()))
    );
    let game_id = (u64::from(0xf234_5678u32) << 32) | 0x0200_0000;
    let game = games
        .iter()
        .find(|g| g.id == format!("steam:shortcut:{game_id}"))
        .unwrap();
    assert_eq!(
        game.command,
        [
            "fixture-steam".to_string(),
            "-silent".into(),
            format!("steam://rungameid/{game_id}")
        ]
    );
    assert_eq!(game.last_played, 1_750_000_001);
    assert_eq!(game.subtitle, "Non-Steam game · Steam");
    assert!(game.artwork.ends_with("4063516280.png"));
    assert_eq!(fs::read(&file).unwrap(), bytes);
    // Some writers close the top-level map with EOF instead of a final marker.
    write(&file, &bytes[..bytes.len() - 1]);
    let (games, _) = Steam.discover(&config(temp.path()));
    assert!(games.iter().any(|g| g.id == game.id));
}
#[test]
fn only_the_most_recent_account_supplies_shortcuts_and_custom_covers() {
    let temp = tempfile::tempdir().unwrap();
    write(
        &temp.path().join("config/loginusers.vdf"),
        "\"users\" { \"76561197960265729\" { \"MostRecent\" \"0\" } \"76561197960265730\" { \"MostRecent\" \"1\" } }",
    );
    write(
        &temp.path().join("userdata/1/config/shortcuts.vdf"),
        shortcuts(0xe234_5678, "Other account game", "", false),
    );
    write(
        &temp.path().join("userdata/2/config/shortcuts.vdf"),
        shortcuts(0xf234_5678, "Active account game", "", false),
    );
    manifest(
        temp.path(),
        98765020,
        "Account cover fixture",
        "\"StateFlags\" \"4\"",
    );
    for user in [1, 2] {
        write(
            &temp
                .path()
                .join(format!("userdata/{user}/config/grid/98765020.png")),
            "cover",
        );
    }
    let (games, _) = Steam.discover(&config(temp.path()));
    assert!(games.iter().any(|g| g.title == "Active account game"));
    assert!(!games.iter().any(|g| g.title == "Other account game"));
    assert!(
        games
            .iter()
            .find(|g| g.id == "steam:98765020")
            .unwrap()
            .artwork
            .contains("userdata/2/config/grid")
    );
}
#[test]
fn hidden_shortcuts_and_corrupt_metadata_do_not_hide_installed_games() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("userdata/1/config/shortcuts.vdf");
    manifest(
        temp.path(),
        98765021,
        "Healthy installed fixture",
        "\"StateFlags\" \"4\"",
    );
    write(&file, shortcuts(0xf234_5678, "Hidden shortcut", "", true));
    let (games, _) = Steam.discover(&config(temp.path()));
    assert!(!games.iter().any(|g| g.title == "Hidden shortcut"));
    write(&file, b"\0shortcuts\0\0unfinished\0");
    let (games, errors) = Steam.discover(&config(temp.path()));
    assert!(games.iter().any(|g| g.id == "steam:98765021"));
    assert!(errors.iter().any(|error| error.contains("shortcuts.vdf")));
}
#[test]
fn installed_filter_excludes_stale_incomplete_tools_and_invalid_paths() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("steamapps/common/Healthy game")).unwrap();
    for (id, title, fields) in [
        (
            98765022,
            "Healthy fixture",
            "\"StateFlags\" \"4\" \"installdir\" \"Healthy game\"",
        ),
        (
            98765023,
            "Missing directory",
            "\"StateFlags\" \"4\" \"installdir\" \"Gone\"",
        ),
        (98765024, "Incomplete", "\"StateFlags\" \"2\""),
        (98765025, "Unknown state", ""),
        (98765026, "Proton Experimental", "\"StateFlags\" \"4\""),
        (
            98765027,
            "Steam Linux Runtime 3.0 (sniper)",
            "\"StateFlags\" \"4\"",
        ),
        (
            98765028,
            "Traversal",
            "\"StateFlags\" \"4\" \"installdir\" \"..\"",
        ),
        (98765029, "Proton Adventure", "\"StateFlags\" \"4\""),
        (0, "Invalid ID", "\"StateFlags\" \"4\""),
    ] {
        manifest(temp.path(), id, title, fields);
    }
    let cfg = SourceConfig {
        command: vec!["fixture-steam".into(), "-silent".into()],
        ..config(temp.path())
    };
    let (games, _) = Steam.discover(&cfg);
    let fixtures: Vec<_> = games
        .iter()
        .filter(|g| {
            g.id.strip_prefix("steam:")
                .and_then(|id| id.parse::<u32>().ok())
                .is_some_and(|id| (98765022..=98765029).contains(&id))
        })
        .collect();
    assert_eq!(fixtures.len(), 2);
    assert!(fixtures.iter().any(|g| g.title == "Proton Adventure"));
    assert_eq!(
        fixtures
            .iter()
            .find(|g| g.title == "Healthy fixture")
            .unwrap()
            .command,
        ["fixture-steam", "-silent", "-applaunch", "98765022"]
    );
    assert!(!games.iter().any(|g| g.id == "steam:0"));
}
#[test]
fn flatpak_shortcuts_launch_quietly_and_icon_fallback_keeps_proportions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp
        .path()
        .join(".var/app/com.valvesoftware.Steam/.local/share/Steam");
    let icon = temp.path().join("icon.png");
    write(&icon, "icon");
    write(
        &root.join("userdata/1/config/shortcuts.vdf"),
        shortcuts(
            0xf234_5678,
            "Flatpak shortcut fixture",
            icon.to_str().unwrap(),
            false,
        ),
    );
    let (games, _) = Steam.discover(&SourceConfig {
        paths: vec![root],
        ..Default::default()
    });
    let game = games
        .iter()
        .find(|g| g.title == "Flatpak shortcut fixture")
        .unwrap();
    assert_eq!(
        &game.command[..4],
        ["flatpak", "run", "com.valvesoftware.Steam", "-silent"]
    );
    assert_eq!(game.art.kind, ArtworkKind::Icon);
    assert!(game.artwork.ends_with("icon.png"));
}

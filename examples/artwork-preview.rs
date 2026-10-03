//! Explicit network check and cache preparation for screenshot previews. Never launches a game.
use orbit_launcher::{
    artwork::{self, ArtworkService, HttpFetcher},
    model::Game,
};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

fn main() {
    let directory = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| orbit_launcher::store::cache_dir().join("artwork"));
    let service = ArtworkService::new(
        directory.clone(),
        true,
        Arc::new(AtomicBool::new(false)),
        HttpFetcher::default(),
    );
    for (title, provider, id, version, project) in [
        ("The Outer Worlds", "steam", "578650", "", ""),
        ("Hollow Knight", "steam", "367520", "", ""),
        ("Celeste", "steam", "504230", "", ""),
        ("Hades", "steam", "1145360", "", ""),
        ("Stardew Valley", "steam", "413150", "", ""),
        ("No Man’s Sky", "steam", "275850", "", ""),
        ("Disco Elysium", "steam", "632470", "", ""),
        ("Minecraft", "prism", "fixture", "1.21.5", ""),
        ("Minecraft Tiny Takeover", "prism", "fixture", "26.1.2", ""),
        ("Minecraft Chaos Cubed", "prism", "fixture", "26.2", ""),
        ("Minecraft Wilderness Bound", "prism", "fixture", "26.3", ""),
        (
            "Fabulously Optimized",
            "modrinth",
            "fixture",
            "1.21.5",
            "1KVo5zza",
        ),
        ("The Witcher", "gog", "1207658924", "", ""),
        ("Hades", "epic", "fixture", "", ""),
    ] {
        let mut game: Game = serde_json::from_value(json!({"id":format!("{provider}:{id}"), "title":title, "provider":provider, "subtitle":"", "artwork":""})).unwrap();
        if provider == "steam" {
            game.art.remote = artwork::steam_urls(id);
        }
        if !version.is_empty() {
            game.art.minecraft_version = Some(version.into());
        }
        if !project.is_empty() {
            game.art.modrinth_project = Some(project.into());
        }
        service.resolve(&mut game, None);
        assert!(
            !game.artwork.is_empty(),
            "No artwork was retrieved for {title}"
        );
        println!("{title}: {}", game.art.source);
    }
    println!("Validated artwork cache: {}", directory.display());
    let service = service.prefer_minecraft_updates(false);
    let mut pack: Game = serde_json::from_value(json!({"id":"modrinth:gallery", "title":"Fabulously Optimized", "provider":"modrinth", "subtitle":"", "artwork":""})).unwrap();
    pack.art.minecraft_version = Some("1.21.5".into());
    pack.art.modrinth_project = Some("1KVo5zza".into());
    service.resolve(&mut pack, None);
    assert_eq!(pack.art.source, "Modrinth modpack gallery");
    println!("Modpack preference: {}", pack.art.source);
}

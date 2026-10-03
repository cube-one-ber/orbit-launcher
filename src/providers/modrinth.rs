//! Read-only support for both Modrinth's legacy profiles and current instances database.
use super::*;
use rusqlite::{Connection, OpenFlags, OptionalExtension};

pub struct Modrinth;
impl Provider for Modrinth {
    fn id(&self) -> &str {
        "modrinth"
    }
    fn name(&self) -> &str {
        "Modrinth Launcher"
    }
    fn note(&self) -> &str {
        "Installed Minecraft instances. Current instances launch directly; older profiles open the launcher."
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let mut games = vec![];
        let mut errors = vec![];
        let mut seen = HashSet::new();
        for root in roots(config, platform.modrinth_roots()) {
            let db = if root.is_file() {
                root.clone()
            } else {
                root.join("app.db")
            };
            if !db.is_file() {
                continue;
            }
            let identity = fs::canonicalize(&db).unwrap_or_else(|_| db.clone());
            if !seen.insert(identity) {
                continue;
            }
            match read_database(&db, config, &platform) {
                Ok((found, warnings)) => {
                    games.extend(found);
                    errors.extend(warnings);
                }
                Err(e) => errors.push(format!("{}: {e}", db.display())),
            }
        }
        (games, errors)
    }
}

fn read_database(
    db: &Path,
    config: &SourceConfig,
    platform: &crate::platform::Platform,
) -> Result<(Vec<Game>, Vec<String>), String> {
    let connection = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|e| e.to_string())?;
    let current: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'instances')", [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    let root = db.parent().ok_or("Database has no parent folder")?;
    let custom: Option<String> = connection
        .query_row("SELECT custom_dir FROM settings LIMIT 1", [], |row| {
            row.get(0)
        })
        .optional()
        .ok()
        .flatten()
        .flatten();
    let mut errors = vec![];
    let data = if let Some(custom) = custom.filter(|s| !s.is_empty()) {
        let path = PathBuf::from(custom);
        if path.is_absolute() {
            path
        } else {
            errors.push(format!(
                "{}: Modrinth's custom directory is not absolute",
                db.display()
            ));
            root.to_path_buf()
        }
    } else {
        root.to_path_buf()
    };
    // The active content set identifies the installed version, not a pending update.
    let query = if current {
        "SELECT i.id, i.path, i.name, i.install_stage, i.icon_path, c.game_version, c.loader, i.last_played, l.modrinth_project_id
         FROM instances i
         LEFT JOIN instance_content_sets c ON c.id = i.applied_content_set_id AND c.instance_id = i.id
         LEFT JOIN instance_links l ON l.instance_id = i.id"
    } else {
        "SELECT path, path, name, install_stage, icon_path, game_version, mod_loader, last_played, linked_project_id FROM profiles"
    };
    let mut statement = connection
        .prepare(query)
        .map_err(|e| format!("Unsupported Modrinth database layout: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let key = url::Url::from_file_path(db).map_err(|_| "Database path must be absolute")?;
    let mut games = vec![];
    for row in rows {
        let (instance_id, path, name, stage, icon, version, loader, played, project) = match row {
            Ok(row) => row,
            Err(e) => {
                errors.push(format!("{}: {e}", db.display()));
                continue;
            }
        };
        if stage != "installed" || path.is_empty() || name.trim().is_empty() {
            continue;
        }
        // Stored paths are relative to profiles. Never treat a damaged path as another installation.
        let relative = Path::new(&path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|p| !matches!(p, std::path::Component::Normal(_)))
        {
            errors.push(format!("{}: Invalid instance path: {path}", db.display()));
            continue;
        }
        let instance = data.join("profiles").join(relative);
        if !instance.is_dir() {
            continue;
        }
        let mut command = config.command.clone();
        let launch_uri = if current {
            if instance_id.is_empty() {
                continue;
            }
            let mut uri = url::Url::parse("modrinth://launch/instance/").expect("constant URL");
            uri.path_segments_mut()
                .expect("hierarchical URL")
                .pop_if_empty()
                .push(&instance_id);
            if command.is_empty() {
                Some(uri.to_string())
            } else {
                command.push(uri.to_string());
                None
            }
        } else {
            // Older Modrinth builds do not implement an instance-launch deep link.
            command = launcher(
                config,
                is_flatpak(root),
                &platform.modrinth_command(),
                "com.modrinth.ModrinthApp",
            );
            None
        };
        let icon = icon
            .map(|p| first_art([data.join(&p), root.join(&p), instance.join(&p)]))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| first_art([instance.join("icon.png"), instance.join("icon.webp")]));
        let version = version.filter(|v| !v.is_empty());
        let loader = loader
            .filter(|v| v != "vanilla" && !v.is_empty())
            .map(|v| format!(" · {v}"))
            .unwrap_or_default();
        games.push(Game {
            id: format!("modrinth:{key}:{path}"), title: name,
            provider: "modrinth".into(),
            subtitle: format!("Minecraft {}{loader} · Modrinth Launcher", version.as_deref().unwrap_or("")),
            artwork: crate::artwork::local_cover(&instance),
            art: Artwork { icon, minecraft_version: version, modrinth_project: project, ..Default::default() },
            launch_notice: if current { String::new() } else { "Opened Modrinth Launcher. Select this profile there to play; this older launcher has no direct instance launch support.".into() },
            command, launch_uri, directory: None, favorite: false,
            last_played: played.unwrap_or(0).max(0) as u64,
        });
    }
    Ok((games, errors))
}

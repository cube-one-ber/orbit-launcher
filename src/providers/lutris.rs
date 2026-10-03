use super::*;
use rusqlite::{Connection, OpenFlags};
pub struct Lutris;
impl Provider for Lutris {
    fn id(&self) -> &str {
        "lutris"
    }
    fn name(&self) -> &str {
        "Lutris"
    }
    fn available(&self) -> bool {
        !cfg!(windows)
    }
    fn note(&self) -> &str {
        if cfg!(windows) {
            "Lutris is available on Linux only."
        } else {
            ""
        }
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let roots = roots(
            config,
            vec![
                default_data("lutris"),
                store::home().join(".var/app/net.lutris.Lutris/data/lutris"),
            ],
        );
        let mut games = vec![];
        let mut errors = vec![];
        for root in roots {
            let path = if root.is_file() {
                root.clone()
            } else {
                root.join("pga.db")
            };
            if !path.exists() {
                continue;
            }
            let result = (|| -> Result<Vec<Game>, rusqlite::Error> {
                let connection =
                    Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                connection.busy_timeout(std::time::Duration::from_secs(2))?;
                let mut statement = connection
                    .prepare("SELECT id, name, slug, runner FROM games WHERE installed = 1")?;
                let rows = statement.query_map([], |row| {
                    let id: i64 = row.get(0)?;
                    let slug: String = row.get::<_, Option<String>>(2)?.unwrap_or_default();
                    let runner: String = row.get::<_, Option<String>>(3)?.unwrap_or_default();
                    let data = path.parent().unwrap_or(&root);
                    let slug_ref = slug.as_str();
                    let mut command =
                        launcher(config, is_flatpak(&root), "lutris", "net.lutris.Lutris");
                    command.push(format!("lutris:rungameid/{id}"));
                    // Include database identity: native and Flatpak IDs can overlap.
                    let key = url::Url::from_file_path(&path)
                        .map(|u| u.to_string())
                        .unwrap_or_default();
                    Ok(Game {
                        id: format!("lutris:{key}:{id}"),
                        title: row.get(1)?,
                        provider: "lutris".into(),
                        subtitle: format!("Lutris · {runner}"),
                        artwork: first_art(
                            [
                                data.to_path_buf(),
                                crate::store::cache_dir()
                                    .parent()
                                    .unwrap_or(data)
                                    .join("lutris"),
                            ]
                            .into_iter()
                            .flat_map(|dir| {
                                ["coverart", "banners"].into_iter().flat_map(move |folder| {
                                    ["webp", "jpg", "png", "jpeg"]
                                        .map(|ext| dir.join(format!("{folder}/{slug_ref}.{ext}")))
                                })
                            }),
                        ),
                        art: Artwork {
                            icon: first_art(
                                ["png", "webp", "svg"]
                                    .map(|ext| data.join(format!("icons/{slug}.{ext}"))),
                            ),
                            ..Default::default()
                        },
                        launch_notice: String::new(),
                        command,
                        launch_uri: None,
                        directory: None,
                        favorite: false,
                        last_played: 0,
                    })
                })?;
                rows.collect()
            })();
            match result {
                Ok(found) => games.extend(found),
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        (games, errors)
    }
}

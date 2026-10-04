use super::*;
use prost::Message;

#[derive(Message)]
struct ProductDatabase {
    #[prost(message, repeated, tag = "1")]
    products: Vec<Product>,
}
#[derive(Message)]
struct Product {
    #[prost(string, tag = "1")]
    internal_id: String,
    #[prost(string, tag = "2")]
    product_id: String,
    #[prost(message, optional, tag = "3")]
    data: Option<InstallData>,
}
#[derive(Message)]
struct InstallData {
    #[prost(string, tag = "1")]
    path: String,
}
#[derive(Deserialize)]
struct CatalogEntry {
    id: String,
    internal: String,
    title: String,
    #[serde(default)]
    remote: Vec<String>,
    #[serde(default)]
    icon: String,
}
pub struct BattleNet;
impl Provider for BattleNet {
    fn id(&self) -> &str {
        "battlenet"
    }
    fn name(&self) -> &str {
        "Battle.net"
    }
    fn available(&self) -> bool {
        cfg!(windows)
    }
    fn note(&self) -> &str {
        if cfg!(windows) {
            "Requests the selected game directly. Battle.net may show login or update prompts."
        } else {
            "Battle.net integration requires Windows."
        }
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let catalog: Vec<CatalogEntry> =
            serde_json::from_str(include_str!("../data/battlenet.json"))
                .expect("Bundled Battle.net catalog");
        let mut records = crate::platform::battlenet_registry_installs();
        let mut errors = vec![];
        let mut seen_files = HashSet::new();
        let defaults = if cfg!(windows) {
            vec![platform.program_data.join("Battle.net/Agent")]
        } else {
            vec![]
        };
        for root in roots(config, defaults) {
            let file = if root.is_file() {
                root
            } else {
                root.join("product.db")
            };
            if !file.exists() {
                continue;
            }
            if !seen_files.insert(fs::canonicalize(&file).unwrap_or_else(|_| file.clone())) {
                continue;
            }
            match binary_metadata::read(&file).and_then(|bytes| {
                ProductDatabase::decode(bytes.as_slice()).map_err(|e| e.to_string())
            }) {
                Ok(db) => {
                    for product in db.products {
                        let entry = catalog.iter().find(|c| {
                            c.internal.eq_ignore_ascii_case(&product.internal_id)
                                || c.id.eq_ignore_ascii_case(&product.internal_id)
                        });
                        if let (Some(entry), Some(data)) = (entry, product.data) {
                            records.push(crate::platform::LauncherInstall {
                                id: entry.id.clone(),
                                title: entry.title.clone(),
                                path: PathBuf::from(data.path),
                            });
                        }
                    }
                }
                Err(e) => errors.push(format!("{}: {e}", file.display())),
            }
        }
        let mut seen = HashSet::new();
        let mut games = vec![];
        for record in records {
            let entry = catalog.iter().find(|c| {
                c.id.eq_ignore_ascii_case(&record.id) || record.id.eq_ignore_ascii_case(&c.internal)
            });
            let Some(entry) = entry else {
                continue;
            };
            let Some(path) = installed::installed_path(
                &record.path.to_string_lossy(),
                &record.path,
                &mut errors,
            ) else {
                continue;
            };
            if !seen.insert(entry.id.clone()) {
                continue;
            }
            let mut game = installed::game("battlenet", &entry.id, entry.title.clone(), &path);
            game.art.remote = entry.remote.clone();
            if game.art.icon.is_empty() {
                game.art.icon = entry.icon.clone();
            }
            game.command = if config.command.is_empty() {
                vec![platform.battlenet_command()]
            } else {
                config.command.clone()
            };
            // One argv item: quotes belong to shell syntax, not to the argument.
            game.command.push(format!("--exec=launch {}", entry.id));
            games.push(game);
        }
        (games, errors)
    }
}

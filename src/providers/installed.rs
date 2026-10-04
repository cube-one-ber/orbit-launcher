//! Shared, read-only installed-game metadata for Legendary and Heroic's Epic backend.
use super::*;
use serde_json::Value;

pub(super) struct Installed {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub art: Artwork,
}

pub(super) fn game(provider: &str, id: &str, title: String, path: &Path) -> Game {
    Game {
        id: format!("{provider}:{id}"),
        title,
        provider: provider.into(),
        subtitle: format!("Installed · {}", crate::artwork::provider_name(provider)),
        artwork: crate::artwork::local_cover(path),
        art: Artwork {
            icon: crate::artwork::local_icon(path),
            ..Default::default()
        },
        launch_notice: String::new(),
        command: vec![],
        environment: Default::default(),
        launch_uri: None,
        directory: None,
        favorite: false,
        last_played: 0,
        source_rank: 0,
    }
}

pub(super) fn read_json(path: &Path, errors: &mut Vec<String>) -> Option<Value> {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(value) => Some(value),
            Err(e) => {
                errors.push(format!("{}: {e}", path.display()));
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            errors.push(format!("{}: {e}", path.display()));
            None
        }
    }
}
pub(super) fn valid_id(id: &str) -> bool {
    !id.trim().is_empty()
        && id != "."
        && id != ".."
        && !id.chars().any(|c| c.is_control() || c == '/' || c == '\\')
}
pub(super) fn installed_path(path: &str, file: &Path, errors: &mut Vec<String>) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        errors.push(format!(
            "{}: Installation path must be absolute",
            file.display()
        ));
        return None;
    }
    path.is_dir().then_some(path)
}
pub(super) fn title_or_folder(title: Option<&str>, path: &Path) -> String {
    title
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
}
pub(super) fn merge_art(art: &mut Artwork, metadata: &Value) {
    for key in ["art_background", "art_cover", "art_square"] {
        if let Some(url) = metadata[key].as_str().filter(|s| s.starts_with("https://"))
            && !art.remote.iter().any(|item| item == url)
        {
            art.remote.push(url.into());
        }
    }
    if art.icon.is_empty() {
        art.icon = metadata["art_square"].as_str().unwrap_or("").into();
    }
}

#[derive(Deserialize)]
struct EpicInstall {
    install_path: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    app_name: String,
    #[serde(default)]
    is_dlc: bool,
    #[serde(default)]
    needs_verification: bool,
    #[serde(default)]
    is_preloaded: bool,
}
pub(super) fn epic_games(root: &Path, errors: &mut Vec<String>) -> Vec<Installed> {
    let file = root.join("installed.json");
    let Some(value) = read_json(&file, errors) else {
        return vec![];
    };
    let Some(records) = value.as_object() else {
        errors.push(format!(
            "{}: Expected an object keyed by Epic app name",
            file.display()
        ));
        return vec![];
    };
    let mut games = vec![];
    for (id, record) in records {
        if !valid_id(id) {
            errors.push(format!("{}: Invalid Epic app name", file.display()));
            continue;
        }
        let info: EpicInstall = match serde_json::from_value(record.clone()) {
            Ok(info) => info,
            Err(e) => {
                errors.push(format!("{}: {id}: {e}", file.display()));
                continue;
            }
        };
        if !info.app_name.is_empty() && info.app_name != *id {
            errors.push(format!(
                "{}: App name does not match installed key: {id}",
                file.display()
            ));
            continue;
        }
        if info.is_dlc || info.needs_verification || info.is_preloaded || id.starts_with("UE_") {
            continue;
        }
        let Some(path) = installed_path(&info.install_path, &file, errors) else {
            continue;
        };
        let metadata = read_json(&root.join("metadata").join(format!("{id}.json")), errors);
        let inner = metadata.as_ref().map(|value| &value["metadata"]);
        if inner.is_some_and(|m| {
            !m["mainGameItem"].is_null()
                || m["categories"].as_array().is_some_and(|items| {
                    items
                        .iter()
                        .any(|c| matches!(c["path"].as_str(), Some("mods" | "plugins")))
                })
        }) {
            continue;
        }
        let mut art = Artwork {
            icon: crate::artwork::local_icon(&path),
            ..Default::default()
        };
        for kind in [
            "DieselGameBox",
            "OfferImageWide",
            "DieselGameBoxTall",
            "OfferImageTall",
        ] {
            for item in inner
                .and_then(|m| m["keyImages"].as_array())
                .into_iter()
                .flatten()
            {
                if item["type"].as_str() == Some(kind)
                    && let Some(url) = item["url"].as_str().filter(|s| s.starts_with("https://"))
                {
                    art.remote.push(url.into());
                }
            }
        }
        let title = if info.title.trim().is_empty() {
            title_or_folder(
                metadata.as_ref().and_then(|m| m["app_title"].as_str()),
                &path,
            )
        } else {
            info.title
        };
        games.push(Installed {
            id: id.clone(),
            title,
            path,
            art,
        });
    }
    games
}

//! Browser-owned authentication; Orbit reads only a bounded, credential-free report.
use super::Provider;
use crate::{model::*, platform::Platform};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub const REPORT_NAME: &str = "orbit-roblox-top-games.json";
const MAX_REPORT: u64 = 256 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    version: u8,
    exported_at: u64,
    user_id: u64,
    username: String,
    games: Vec<Experience>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Experience {
    universe_id: u64,
    place_id: u64,
    title: String,
    weekly_minutes: u64,
}
pub struct Roblox;
impl Provider for Roblox {
    fn id(&self) -> &str {
        "roblox"
    }
    fn name(&self) -> &str {
        "Roblox"
    }
    fn note(&self) -> &str {
        "Your five most played experiences from the last week. Install the Orbit Roblox browser extension and visit Roblox while signed in to sync. Orbit watches its report in Downloads; choose a report path if your browser saves elsewhere. Play joins directly using the Roblox protocol. Linux requires a compatible handler such as Sober."
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        match read_report(config) {
            Ok(Some(report)) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if report.version != 1
                    || report.user_id == 0
                    || report.exported_at == 0
                    || report.exported_at > now.saturating_add(300)
                    || report.games.len() > 20
                    || report.username.is_empty()
                    || report.username.len() > 100
                    || report.username.chars().any(char::is_control)
                {
                    return (
                        vec![],
                        vec!["Invalid Roblox report. Sync again using the Orbit extension.".into()],
                    );
                }
                let mut errors = vec![];
                if now.saturating_sub(report.exported_at) > 86400 {
                    errors.push("Roblox report is over a day old. Visit Roblox with the extension enabled to update it; the displayed ranking is from the last sync.".into());
                }
                let mut entries = report.games;
                let mut seen = HashSet::new();
                entries.sort_by(|a, b| {
                    b.weekly_minutes
                        .cmp(&a.weekly_minutes)
                        .then(a.universe_id.cmp(&b.universe_id))
                });
                let had_entries = !entries.is_empty();
                entries.retain(|e| {
                    e.universe_id > 0
                        && e.place_id > 0
                        && e.weekly_minutes > 0
                        && e.weekly_minutes <= 7 * 24 * 60
                        && !e.title.trim().is_empty()
                        && e.title.len() <= 1200
                        && !e.title.chars().any(char::is_control)
                        && seen.insert(e.universe_id)
                });
                if had_entries && entries.is_empty() {
                    return (vec![], vec!["Roblox report contains no valid played experiences. Sync again using the extension.".into()]);
                }
                let games = entries
                    .into_iter()
                    .take(5)
                    .enumerate()
                    .map(|(index, e)| {
                        let uri = format!("roblox://experiences/start?placeId={}", e.place_id);
                        let (command, launch_uri) = if config.command.is_empty() {
                            (vec![], Some(uri))
                        } else {
                            let mut command = config.command.clone();
                            command.push(uri);
                            (command, None)
                        };
                        Game {
                            id: format!("roblox:{}:{}", report.user_id, e.universe_id),
                            title: e.title,
                            provider: "roblox".into(),
                            subtitle: format!(
                                "#{} · {}h {}m last week · @{}",
                                index + 1,
                                e.weekly_minutes / 60,
                                e.weekly_minutes % 60,
                                report.username
                            ),
                            artwork: String::new(),
                            art: Artwork {
                                roblox_universe: Some(e.universe_id),
                                ..Default::default()
                            },
                            launch_notice: String::new(),
                            command,
                            environment: Default::default(),
                            launch_uri,
                            directory: None,
                            favorite: false,
                            last_played: 0,
                            source_rank: (index + 1) as u8,
                        }
                    })
                    .collect();
                (games, errors)
            }
            Ok(None) => (vec![], vec![]),
            Err(error) => (vec![], vec![error]),
        }
    }
}
pub fn report_path(config: &SourceConfig) -> Result<PathBuf, String> {
    if config.paths.len() > 1 {
        return Err("Choose one Roblox report path to avoid mixing different accounts.".into());
    }
    let path = config
        .paths
        .first()
        .cloned()
        .unwrap_or_else(|| Platform::current().downloads_dir().join(REPORT_NAME));
    Ok(if path.is_dir() {
        path.join(REPORT_NAME)
    } else {
        path
    })
}
fn read_report(config: &SourceConfig) -> Result<Option<Report>, String> {
    let path = report_path(config)?;
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && config.paths.is_empty() => {
            return Ok(None);
        }
        Err(e) => {
            return Err(format!(
                "Could not read Roblox report {}: {e}",
                path.display()
            ));
        }
    };
    let mut bytes = Vec::new();
    file.take(MAX_REPORT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_REPORT {
        return Err("Roblox report exceeds 256 KiB.".into());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "Invalid Roblox report. Sync again using the Orbit extension.".into())
}
/// Cheap polling fingerprint; report parsing and library scanning stay on the worker.
pub fn report_signature(config: &SourceConfig) -> Option<(PathBuf, SystemTime, u64)> {
    let path = report_path(config).ok()?;
    let metadata = fs::metadata(&path).ok()?;
    Some((path, metadata.modified().ok()?, metadata.len()))
}

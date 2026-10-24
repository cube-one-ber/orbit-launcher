//! State behind the instance manager window, independent of Qt.
//!
//! The bridge owns one session for the open instance, runs network [`Job`]s on worker threads
//! with [`run`] and merges their [`Outcome`]s back with [`Session::apply`]. Preview mode uses
//! sample content and never reads or changes files.
use super::{ContentItem, ContentKind, InstanceSettings, RemoteDetails, Report, SearchPage};
use crate::model::{Game, MinecraftInstance};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Job {
    Inspect(ContentKind),
    Search {
        kind: ContentKind,
        query: String,
        offset: u64,
    },
    Install {
        kind: ContentKind,
        project: String,
    },
    Update {
        kind: ContentKind,
        file_name: Option<String>,
    },
}
impl Job {
    pub fn kind(&self) -> ContentKind {
        match self {
            Self::Inspect(kind) => *kind,
            Self::Search { kind, .. } | Self::Install { kind, .. } | Self::Update { kind, .. } => {
                *kind
            }
        }
    }
    fn description(&self) -> &'static str {
        match self {
            Self::Inspect(_) => "Checking Modrinth for details and updates…",
            Self::Search { .. } => "Searching Modrinth…",
            Self::Install { .. } => "Downloading from Modrinth…",
            Self::Update { .. } => "Updating from Modrinth…",
        }
    }
}
pub enum Output {
    Details(BTreeMap<String, RemoteDetails>),
    Page(SearchPage),
    Report(Report),
}
pub struct Outcome {
    pub game_id: String,
    pub job: Job,
    pub result: Result<Output, String>,
}

struct SearchState {
    kind: ContentKind,
    query: String,
    offset: u64,
    page: SearchPage,
    error: String,
}

pub struct Session {
    pub game_id: String,
    title: String,
    pub instance: MinecraftInstance,
    demo: bool,
    /// Modrinth details keyed by file name without `.disabled`.
    remote: HashMap<ContentKind, BTreeMap<String, RemoteDetails>>,
    inspected: HashSet<ContentKind>,
    search: Option<SearchState>,
    pub status: String,
    pub error: String,
    completion: Option<(String, String)>,
}

impl Session {
    pub fn new(game: &Game, demo: bool) -> Option<Self> {
        Some(Self {
            game_id: game.id.clone(),
            title: game.title.clone(),
            instance: game.instance.clone()?,
            demo,
            remote: HashMap::new(),
            inspected: HashSet::new(),
            search: None,
            status: String::new(),
            error: String::new(),
            completion: None,
        })
    }
    fn report(&mut self, result: Result<String, String>) -> bool {
        match result {
            Ok(status) => {
                self.status = status;
                self.error.clear();
                true
            }
            Err(error) => {
                self.status.clear();
                self.error = error;
                false
            }
        }
    }
    fn preview(&mut self) -> bool {
        self.report(Err(
            "Preview mode · Instance changes are disabled. Run Orbit without --demo to manage your instances.".into(),
        ))
    }
    /// Notes progress for a job that is about to run.
    pub fn started(&mut self, job: &Job) {
        self.status = job.description().into();
        if self.completion.is_none() {
            self.error.clear();
        }
    }
    pub fn installed_projects(&self, kind: ContentKind) -> HashSet<String> {
        self.remote
            .get(&kind)
            .into_iter()
            .flat_map(|r| r.values().map(|d| d.project_id.clone()))
            .collect()
    }
    pub fn was_inspected(&self, kind: ContentKind) -> bool {
        self.inspected.contains(&kind)
    }

    pub fn toggle(&mut self, kind: ContentKind, file_name: &str) -> bool {
        if self.demo {
            return self.preview();
        }
        let enable = file_name.ends_with(".disabled");
        let result = super::set_enabled(&self.instance, kind, file_name, enable).map(|name| {
            format!(
                "{} {}",
                if enable { "Enabled" } else { "Disabled" },
                super::canonical(&name)
            )
        });
        self.report(result)
    }
    pub fn remove(&mut self, kind: ContentKind, file_name: &str) -> bool {
        if self.demo {
            return self.preview();
        }
        let result = super::remove(&self.instance, kind, file_name)
            .map(|_| format!("Removed {}", super::canonical(file_name)));
        if result.is_ok()
            && let Some(remote) = self.remote.get_mut(&kind)
        {
            remote.remove(super::canonical(file_name));
        }
        if result.is_ok() {
            let installed = self.installed_projects(kind);
            if let Some(search) = self.search.as_mut().filter(|s| s.kind == kind) {
                for hit in &mut search.page.hits {
                    hit.installed = installed.contains(&hit.project_id);
                }
            }
        }
        self.report(result)
    }
    pub fn add_file(&mut self, kind: ContentKind, path: &Path) -> bool {
        if self.demo {
            return self.preview();
        }
        let result =
            super::add_file(&self.instance, kind, path).map(|name| format!("Added {name}"));
        self.report(result)
    }
    pub fn save_settings(&mut self, json: &str) -> bool {
        if self.demo {
            return self.preview();
        }
        if !super::supports_settings(&self.instance) {
            return self.report(Err(format!(
                "{} keeps these settings in its own database. Change them there.",
                super::launcher_name(&self.instance.launcher)
            )));
        }
        let result = serde_json::from_str::<InstanceSettings>(json)
            .map_err(|e| format!("Invalid settings: {e}"))
            .and_then(|settings| {
                super::write_settings(&self.instance.root, &settings)?;
                self.title = settings.name.trim().into();
                Ok(format!(
                    "Settings saved. If {} is open, restart it to use them.",
                    super::launcher_name(&self.instance.launcher)
                ))
            });
        self.report(result)
    }
    pub fn folder(&self, kind: Option<ContentKind>) -> Result<PathBuf, String> {
        if self.demo {
            return Err("Preview mode · Instance folders are not available.".into());
        }
        let path = match kind {
            Some(kind) => super::folder(&self.instance, kind),
            None => self.instance.root.clone(),
        };
        std::fs::create_dir_all(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(path)
    }

    /// Completes a job immediately with sample data in preview mode.
    pub fn demo_job(&mut self, job: &Job) {
        match job {
            Job::Inspect(kind) => {
                let details = demo_items(*kind)
                    .into_iter()
                    .filter_map(|(item, details)| Some((item.name, details?)))
                    .collect();
                self.remote.insert(*kind, details);
                self.inspected.insert(*kind);
                self.status.clear();
            }
            Job::Search {
                kind,
                query,
                offset,
            } => {
                let installed = self.installed_projects(*kind);
                let hits: Vec<_> = demo_hits(*kind)
                    .into_iter()
                    .filter(|h| {
                        query.is_empty() || h.title.to_lowercase().contains(&query.to_lowercase())
                    })
                    .map(|mut h| {
                        h.installed = installed.contains(&h.project_id);
                        h
                    })
                    .collect();
                self.search = Some(SearchState {
                    kind: *kind,
                    query: query.clone(),
                    offset: *offset,
                    page: SearchPage {
                        total_hits: hits.len() as u64,
                        hits,
                        offset: *offset,
                    },
                    error: String::new(),
                });
                self.status.clear();
            }
            Job::Install { .. } | Job::Update { .. } => {
                self.preview();
            }
        }
    }

    /// Merges a finished job. Returns follow-up work, such as refreshing details after changes.
    pub fn apply(&mut self, outcome: Outcome) -> Option<Job> {
        if outcome.game_id != self.game_id {
            return None;
        }
        let kind = outcome.job.kind();
        match (outcome.job, outcome.result) {
            (Job::Inspect(_), Ok(Output::Details(details))) => {
                self.remote.insert(
                    kind,
                    details
                        .into_iter()
                        .map(|(name, d)| (super::canonical(&name).to_string(), d))
                        .collect(),
                );
                self.inspected.insert(kind);
                let installed = self.installed_projects(kind);
                if let Some(search) = self.search.as_mut().filter(|s| s.kind == kind) {
                    for hit in &mut search.page.hits {
                        hit.installed = installed.contains(&hit.project_id);
                    }
                }
                let updates = self.remote[&kind]
                    .values()
                    .filter(|d| d.update.is_some())
                    .count();
                self.status = match updates {
                    0 => String::new(),
                    1 => "1 update available.".into(),
                    n => format!("{n} updates available."),
                };
                self.error.clear();
                if let Some((status, error)) = self.completion.take() {
                    self.status = [status, self.status.clone()]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · ");
                    self.error = error;
                }
                None
            }
            (Job::Search { query, offset, .. }, result) => {
                let (page, error) = match result {
                    Ok(Output::Page(page)) => (page, String::new()),
                    Ok(_) => (SearchPage::default(), String::new()),
                    Err(e) => (SearchPage::default(), e),
                };
                self.search = Some(SearchState {
                    kind,
                    query,
                    offset,
                    page,
                    error,
                });
                self.status.clear();
                None
            }
            (job @ (Job::Install { .. } | Job::Update { .. }), Ok(Output::Report(report))) => {
                if let Job::Install { project, .. } = &job
                    && let Some(search) = self.search.as_mut().filter(|s| s.kind == kind)
                {
                    for hit in search
                        .page
                        .hits
                        .iter_mut()
                        .filter(|h| &h.project_id == project)
                    {
                        hit.installed = true;
                    }
                }
                let mut lines = report.changed;
                if lines.is_empty() && matches!(job, Job::Update { .. }) {
                    lines.push("Everything is up to date.".into());
                }
                self.status = lines.join(" · ");
                self.error = report.notes.join("\n");
                self.completion = Some((self.status.clone(), self.error.clone()));
                Some(Job::Inspect(kind))
            }
            (_, Err(error)) => {
                let (status, notes) = self.completion.take().unwrap_or_default();
                self.status = status;
                self.error = [notes, error]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                None
            }
            (_, Ok(_)) => None,
        }
    }

    fn items(&self, kind: ContentKind) -> (Vec<ContentItem>, String) {
        if self.demo {
            let items = demo_items(kind).into_iter().map(|(item, _)| item).collect();
            return (items, String::new());
        }
        match super::list(&self.instance, kind) {
            Ok(items) => (items, String::new()),
            Err(e) => (vec![], e),
        }
    }
    fn settings(&self) -> (Value, String) {
        let launcher = self.instance.launcher.as_str();
        if self.demo && launcher != "modrinth" {
            let settings = InstanceSettings {
                name: self.title.clone(),
                ..Default::default()
            };
            return (json!(settings), String::new());
        }
        if !super::supports_settings(&self.instance) {
            return (Value::Null, String::new());
        }
        match super::read_settings(&self.instance.root) {
            Ok(settings) => (json!(settings), String::new()),
            Err(e) => (Value::Null, e),
        }
    }
    /// The JSON model shown by the instance window.
    pub fn view(&self) -> Value {
        let mut content = serde_json::Map::new();
        for kind in ContentKind::ALL {
            let (items, error) = self.items(kind);
            let remote = self.remote.get(&kind);
            let items: Vec<Value> = items
                .into_iter()
                .map(|item| {
                    let details = remote.and_then(|r| r.get(&item.name));
                    let fallback = if item.folder {
                        item.name.clone()
                    } else {
                        Path::new(&item.name)
                            .file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| item.name.clone())
                    };
                    json!({
                        "file_name": item.file_name,
                        "name": item.name,
                        "enabled": item.enabled,
                        "folder": item.folder,
                        "size": item.size,
                        "title": details.map_or(fallback, |d| d.title.clone()),
                        "project_id": details.map(|d| d.project_id.clone()).unwrap_or_default(),
                        "icon": details.map(|d| d.icon.clone()).unwrap_or_default(),
                        "version": details.map(|d| d.version.clone()).unwrap_or_default(),
                        "update": details.and_then(|d| d.update.clone()).unwrap_or_default(),
                    })
                })
                .collect();
            let updates = items.iter().filter(|i| i["update"] != "").count();
            content.insert(
                kind.folder().into(),
                json!({
                    "items": items,
                    "error": error,
                    "unavailable": super::unavailable_reason(&self.instance, kind).unwrap_or_default(),
                    "inspected": self.inspected.contains(&kind),
                    "updates": updates,
                }),
            );
        }
        let (settings, settings_error) = self.settings();
        let search = self.search.as_ref().map(|s| {
            json!({
                "kind": s.kind.folder(),
                "query": s.query,
                "offset": s.offset,
                "total": s.page.total_hits,
                "hits": s.page.hits,
                "error": s.error,
            })
        });
        let instance = &self.instance;
        let loader = instance.loader.as_ref().map(|l| {
            let mut name = match l.as_str() {
                "neoforge" => "NeoForge".to_string(),
                "liteloader" => "LiteLoader".to_string(),
                other => {
                    let mut chars = other.chars();
                    chars
                        .next()
                        .map(|c| c.to_uppercase().chain(chars).collect())
                        .unwrap_or_default()
                }
            };
            if let Some(version) = &instance.loader_version {
                name = format!("{name} {version}");
            }
            name
        });
        let summary: Vec<String> = [
            instance
                .minecraft_version
                .as_ref()
                .map(|v| format!("Minecraft {v}")),
            Some(loader.unwrap_or_else(|| "Vanilla".into())),
            Some(super::launcher_name(&instance.launcher).into()),
        ]
        .into_iter()
        .flatten()
        .collect();
        json!({
            "game_id": self.game_id,
            "title": self.title,
            "launcher": instance.launcher,
            "launcher_name": super::launcher_name(&instance.launcher),
            "summary": summary.join(" · "),
            "modpack": instance.modpack.clone().unwrap_or_default(),
            "content": content,
            "settings": settings,
            "settings_error": settings_error,
            "search": search,
            "status": self.status,
            "error": self.error,
            "demo": self.demo,
        })
    }
}

/// Runs a network job on a worker thread. Icons use the bounded artwork cache when allowed.
pub fn run(
    job: &Job,
    instance: &MinecraftInstance,
    installed: &HashSet<String>,
    icon_cache: Option<PathBuf>,
) -> Result<Output, String> {
    let api = super::Modrinth::default();
    let icons = icon_cache.map(|dir| {
        crate::artwork::ArtworkService::new(
            dir,
            true,
            Arc::new(AtomicBool::new(false)),
            crate::artwork::HttpFetcher::default(),
        )
    });
    let icon = |url: &str| {
        icons
            .as_ref()
            .filter(|_| !url.is_empty())
            .and_then(|service| service.cached_image(url))
            .unwrap_or_default()
    };
    match job {
        Job::Inspect(kind) => {
            let mut details = super::inspect(&api, instance, *kind)?;
            for detail in details.values_mut() {
                detail.icon = icon(&detail.icon);
            }
            Ok(Output::Details(details))
        }
        Job::Search {
            kind,
            query,
            offset,
        } => {
            let mut page = super::search(&api, instance, *kind, query, *offset, installed)?;
            for hit in &mut page.hits {
                hit.icon = icon(hit.icon_url.as_deref().unwrap_or_default());
            }
            Ok(Output::Page(page))
        }
        Job::Install { kind, project } => {
            super::install(&api, instance, *kind, project).map(Output::Report)
        }
        Job::Update { kind, file_name } => {
            super::update(&api, instance, *kind, file_name.as_deref()).map(Output::Report)
        }
    }
}

fn demo_items(kind: ContentKind) -> Vec<(ContentItem, Option<RemoteDetails>)> {
    let entries: &[(&str, &str, &str, &str, &str)] = match kind {
        ContentKind::Mods => &[
            (
                "sodium-fabric-0.8.13+mc1.21.1.jar",
                "AANobbMI",
                "Sodium",
                "0.8.13",
                "",
            ),
            (
                "iris-fabric-1.8.7+mc1.21.1.jar",
                "YL57xq9U",
                "Iris Shaders",
                "1.8.7",
                "1.8.8",
            ),
            (
                "modmenu-11.0.3.jar.disabled",
                "mOgUt4GM",
                "Mod Menu",
                "11.0.3",
                "",
            ),
            ("local-tweaks.jar", "", "", "", ""),
        ],
        ContentKind::ResourcePacks => &[(
            "Faithful 32x - 1.21.zip",
            "Faithful",
            "Faithful 32x",
            "1.21",
            "",
        )],
        ContentKind::ShaderPacks => &[(
            "ComplementaryReimagined_r5.9.3.zip",
            "HVnmMxH1",
            "Complementary Shaders - Reimagined",
            "r5.9.3",
            "",
        )],
    };
    entries
        .iter()
        .map(|(file, project, title, version, update)| {
            let name = super::canonical(file).to_string();
            (
                ContentItem {
                    file_name: file.to_string(),
                    enabled: !file.ends_with(".disabled"),
                    name,
                    folder: false,
                    size: 1_048_576,
                },
                (!project.is_empty()).then(|| RemoteDetails {
                    project_id: project.to_string(),
                    title: title.to_string(),
                    icon: String::new(),
                    version: version.to_string(),
                    update: (!update.is_empty()).then(|| update.to_string()),
                }),
            )
        })
        .collect()
}
fn demo_hits(kind: ContentKind) -> Vec<super::SearchHit> {
    let hits: &[(&str, &str, &str, &str)] = match kind {
        ContentKind::Mods => &[
            (
                "AANobbMI",
                "Sodium",
                "CaffeineMC",
                "A modern rendering engine for Minecraft.",
            ),
            (
                "gvQqBUqZ",
                "Lithium",
                "CaffeineMC",
                "General-purpose optimization for game logic.",
            ),
            (
                "P7dR8mSH",
                "Fabric API",
                "modmuss50",
                "Core library for Fabric mods.",
            ),
        ],
        ContentKind::ResourcePacks => &[(
            "Faithful",
            "Faithful 32x",
            "Faithful Resource Pack",
            "Classic textures at twice the resolution.",
        )],
        ContentKind::ShaderPacks => &[(
            "HVnmMxH1",
            "Complementary Shaders - Reimagined",
            "EminGT",
            "Vanilla-style shaders with modern lighting.",
        )],
    };
    hits.iter()
        .map(|(id, title, author, description)| super::SearchHit {
            project_id: id.to_string(),
            slug: id.to_lowercase(),
            title: title.to_string(),
            author: author.to_string(),
            description: description.to_string(),
            downloads: 1_000_000,
            ..Default::default()
        })
        .collect()
}

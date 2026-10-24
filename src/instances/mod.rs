//! User-requested changes to Minecraft instances from Prism Launcher, MultiMC, PolyMC and the
//! Modrinth App.
//!
//! Providers only read launcher data. This module runs when the user asks Orbit to change an
//! instance: it renames, adds or removes files in the instance's content folders, downloads
//! verified files from Modrinth and edits Prism-family `instance.cfg` settings. These are the
//! same on-disk operations the launchers perform. Orbit never writes a launcher database; the
//! Modrinth App reconciles its content folders itself, as Prism's folder watcher does.
pub mod api;
mod config;
mod packwiz;
pub mod session;

use crate::model::MinecraftInstance;
pub use api::{HttpTransport, Modrinth, SearchHit, SearchPage, Transport};
pub use config::{InstanceSettings, read_settings, read_value, read_values, write_settings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

/// Required dependencies are followed until this many files would be installed.
const INSTALL_LIMIT: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Mods,
    ResourcePacks,
    ShaderPacks,
}
impl ContentKind {
    pub const ALL: [Self; 3] = [Self::Mods, Self::ResourcePacks, Self::ShaderPacks];
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.folder() == id)
    }
    /// Content folder name inside the game directory; also the kind's ID.
    pub fn folder(self) -> &'static str {
        match self {
            Self::Mods => "mods",
            Self::ResourcePacks => "resourcepacks",
            Self::ShaderPacks => "shaderpacks",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Mods => "mod",
            Self::ResourcePacks => "resource pack",
            Self::ShaderPacks => "shader pack",
        }
    }
    fn project_type(self) -> &'static str {
        match self {
            Self::Mods => "mod",
            Self::ResourcePacks => "resourcepack",
            Self::ShaderPacks => "shader",
        }
    }
    fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Mods => &["jar", "zip", "litemod"],
            Self::ResourcePacks | Self::ShaderPacks => &["zip"],
        }
    }
    fn accepts(self, file_name: &str) -> bool {
        Path::new(canonical(file_name))
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| self.extensions().iter().any(|x| x.eq_ignore_ascii_case(e)))
    }
}

/// One entry in a content folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ContentItem {
    /// Current on-disk name, including any `.disabled` suffix.
    pub file_name: String,
    /// Name without the `.disabled` suffix.
    pub name: String,
    pub enabled: bool,
    /// Unpacked resource or shader packs are listed but cannot be toggled or removed here.
    pub folder: bool,
    pub size: u64,
}

/// Modrinth details for an installed file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RemoteDetails {
    pub project_id: String,
    pub title: String,
    /// Remote icon; the caller may replace it with a cached local file URL.
    pub icon: String,
    pub version: String,
    /// Newer compatible version number, if one exists.
    pub update: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Human-readable summaries of completed changes.
    pub changed: Vec<String>,
    /// Problems that did not stop the whole request.
    pub notes: Vec<String>,
}

pub fn launcher_name(launcher: &str) -> &'static str {
    match launcher {
        "prism" => "Prism Launcher",
        "multimc" => "MultiMC",
        "polymc" => "PolyMC",
        "modrinth" => "Modrinth App",
        _ => "the launcher",
    }
}
/// Prism-family instances keep editable settings in instance.cfg.
pub fn supports_settings(instance: &MinecraftInstance) -> bool {
    matches!(instance.launcher.as_str(), "prism" | "multimc" | "polymc")
        && instance.root.join("instance.cfg").is_file()
}
/// Prism and PolyMC read packwiz metadata; MultiMC and the Modrinth App identify files by hash.
fn writes_metadata(instance: &MinecraftInstance) -> bool {
    matches!(instance.launcher.as_str(), "prism" | "polymc")
}
pub fn folder(instance: &MinecraftInstance, kind: ContentKind) -> PathBuf {
    instance.game_dir.join(kind.folder())
}
fn canonical(file_name: &str) -> &str {
    file_name.strip_suffix(".disabled").unwrap_or(file_name)
}

/// Modrinth loaders whose mods this instance can load.
pub fn loaders(instance: &MinecraftInstance, kind: ContentKind) -> Vec<String> {
    if kind != ContentKind::Mods {
        return vec![];
    }
    match instance.loader.as_deref() {
        None => vec![],
        // Quilt loads Fabric mods, and NeoForge 1.20.1 still loads Forge mods.
        Some("quilt") => vec!["quilt".into(), "fabric".into()],
        Some("neoforge") if instance.minecraft_version.as_deref() == Some("1.20.1") => {
            vec!["neoforge".into(), "forge".into()]
        }
        Some(loader) => vec![loader.into()],
    }
}

/// Explains why content of this kind cannot be added from Modrinth.
pub fn unavailable_reason(instance: &MinecraftInstance, kind: ContentKind) -> Option<String> {
    if instance
        .minecraft_version
        .as_deref()
        .is_none_or(str::is_empty)
    {
        return Some(format!(
            "Orbit could not read this instance's Minecraft version. Open it once in {} and refresh.",
            launcher_name(&instance.launcher)
        ));
    }
    if kind == ContentKind::Mods && instance.loader.is_none() {
        return Some(format!(
            "This instance has no mod loader. Add Fabric, Quilt, Forge or NeoForge in {} first.",
            launcher_name(&instance.launcher)
        ));
    }
    None
}

/// A single path component that cannot leave the content folder.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.contains(['/', '\\', '\0'])
        && matches!(
            Path::new(name).components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        )
}
/// File names accepted from Modrinth or local files: portable to Windows as well.
fn portable_name(name: &str) -> bool {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = name.split('.').next().unwrap_or_default();
    plain_name(name)
        && !name.ends_with(['.', ' '])
        && !name.contains(['<', '>', ':', '"', '|', '?', '*'])
        && !name.chars().any(char::is_control)
        && !RESERVED
            .iter()
            .any(|r| r.eq_ignore_ascii_case(stem.trim_end()))
}
/// Packwiz metadata names derived from project slugs.
pub(crate) fn safe_stem(name: &str) -> bool {
    name.len() <= 64
        && portable_name(name)
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}
fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Lists content files with an accepted extension, enabled or disabled, sorted by name.
pub fn list(instance: &MinecraftInstance, kind: ContentKind) -> Result<Vec<ContentItem>, String> {
    let dir = folder(instance, kind);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    let mut items = vec![];
    for entry in entries.flatten() {
        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };
        if file_name.starts_with('.') {
            continue;
        }
        // Follow links: the Modrinth App may link files from its shared content store.
        let Ok(metadata) = fs::metadata(entry.path()) else {
            continue;
        };
        if metadata.is_dir() {
            if kind != ContentKind::Mods {
                items.push(ContentItem {
                    name: file_name.clone(),
                    file_name,
                    enabled: true,
                    folder: true,
                    size: 0,
                });
            }
            continue;
        }
        if !kind.accepts(&file_name) {
            continue;
        }
        items.push(ContentItem {
            name: canonical(&file_name).into(),
            enabled: !file_name.ends_with(".disabled"),
            file_name,
            folder: false,
            size: metadata.len(),
        });
    }
    items.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.file_name.cmp(&b.file_name))
    });
    Ok(items)
}

/// Resolves a listed file name, rejecting anything outside the content folder.
fn existing(
    instance: &MinecraftInstance,
    kind: ContentKind,
    file_name: &str,
) -> Result<PathBuf, String> {
    if !plain_name(file_name) || !kind.accepts(file_name) {
        return Err(format!("{file_name} is not a {} file.", kind.label()));
    }
    let path = folder(instance, kind).join(file_name);
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => Ok(path),
        Ok(_) => Err("Folders cannot be changed here. Open the folder instead.".into()),
        Err(_) => Err(format!(
            "{file_name} is no longer in this instance. The list has been refreshed."
        )),
    }
}
fn busy_hint(error: std::io::Error) -> String {
    format!("{error}. Close Minecraft if this instance is running, then try again.")
}

/// Enables or disables a file with the `.disabled` suffix used by all four launchers.
/// Returns the new file name.
pub fn set_enabled(
    instance: &MinecraftInstance,
    kind: ContentKind,
    file_name: &str,
    enabled: bool,
) -> Result<String, String> {
    let path = existing(instance, kind, file_name)?;
    if file_name.ends_with(".disabled") != enabled {
        return Ok(file_name.into());
    }
    let target = if enabled {
        canonical(file_name).to_string()
    } else {
        format!("{file_name}.disabled")
    };
    let target_path = folder(instance, kind).join(&target);
    if exists(&target_path) {
        return Err(format!(
            "Both {} and {target} exist. Remove one copy first.",
            file_name
        ));
    }
    fs::rename(&path, &target_path)
        .map_err(|e| format!("Could not rename {file_name}: {}", busy_hint(e)))?;
    Ok(target)
}

/// Deletes a content file and any Prism metadata describing it.
pub fn remove(
    instance: &MinecraftInstance,
    kind: ContentKind,
    file_name: &str,
) -> Result<(), String> {
    let path = existing(instance, kind, file_name)?;
    fs::remove_file(&path)
        .map_err(|e| format!("Could not remove {file_name}: {}", busy_hint(e)))?;
    if writes_metadata(instance) {
        packwiz::remove_for_file(&folder(instance, kind), canonical(file_name));
    }
    Ok(())
}

/// Copies a local file into the content folder without replacing existing content.
pub fn add_file(
    instance: &MinecraftInstance,
    kind: ContentKind,
    source: &Path,
) -> Result<String, String> {
    let name = source
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| canonical(n).to_string())
        .unwrap_or_default();
    if !portable_name(&name) || !kind.accepts(&name) {
        let extensions: Vec<_> = kind.extensions().iter().map(|e| format!(".{e}")).collect();
        return Err(format!(
            "Choose a {} file ({}).",
            kind.label(),
            extensions.join(", ")
        ));
    }
    if !source.is_file() {
        return Err(format!("{} does not exist.", source.display()));
    }
    let dir = folder(instance, kind);
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if exists(&dir.join(&name)) || exists(&dir.join(format!("{name}.disabled"))) {
        return Err(format!("{name} is already in this instance."));
    }
    let mut temp = temp_file(&dir)?;
    let mut input = fs::File::open(source).map_err(|e| format!("{}: {e}", source.display()))?;
    std::io::copy(&mut input, temp.as_file_mut())
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|e| format!("Could not copy {name}: {e}"))?;
    temp.persist_noclobber(dir.join(&name))
        .map_err(|e| format!("Could not add {name}: {}", e.error))?;
    Ok(name)
}

fn temp_file(dir: &Path) -> Result<tempfile::NamedTempFile, String> {
    // Hidden and without a content extension, so launchers ignore partial files.
    tempfile::Builder::new()
        .prefix(".orbit-")
        .suffix(".part")
        .tempfile_in(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))
}

fn sha512_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hash = Sha512::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
/// SHA-512 hashes of listed files, keyed by on-disk name.
fn hashed_files(
    instance: &MinecraftInstance,
    kind: ContentKind,
    only: Option<&str>,
) -> Result<Vec<(String, String)>, String> {
    let dir = folder(instance, kind);
    let mut files = vec![];
    for item in list(instance, kind)?.into_iter().filter(|i| !i.folder) {
        if only.is_some_and(|name| name != item.file_name) {
            continue;
        }
        let hash = sha512_file(&dir.join(&item.file_name))?;
        files.push((item.file_name, hash));
    }
    if let Some(name) = only
        && files.is_empty()
    {
        existing(instance, kind, name)?;
    }
    Ok(files)
}

fn compatible(version: &api::Version, instance: &MinecraftInstance, kind: ContentKind) -> bool {
    let game = instance.minecraft_version.as_deref().unwrap_or_default();
    let loaders = loaders(instance, kind);
    version.game_versions.iter().any(|v| v == game)
        && (loaders.is_empty() || version.loaders.iter().any(|l| loaders.contains(l)))
        && version.primary_file().is_some()
}
/// Prefers stable releases, then the instance's own loader, then the newest upload.
fn best_version<T: Transport>(
    api: &Modrinth<T>,
    instance: &MinecraftInstance,
    kind: ContentKind,
    project: &str,
) -> Result<Option<api::Version>, String> {
    let loaders = loaders(instance, kind);
    let mut versions =
        api.project_versions(project, &loaders, instance.minecraft_version.as_deref())?;
    versions.retain(|v| compatible(v, instance, kind));
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    let shader_loader = match instance.loader.as_deref() {
        Some("forge") => "optifine",
        _ => "iris",
    };
    let rank = |v: &api::Version| {
        let stability = match v.version_type.as_str() {
            "release" => 0,
            "beta" => 1,
            _ => 2,
        };
        let loader = match kind {
            ContentKind::Mods => loaders
                .iter()
                .position(|l| v.loaders.contains(l))
                .unwrap_or(usize::MAX),
            ContentKind::ShaderPacks => usize::from(!v.loaders.iter().any(|l| l == shader_loader)),
            ContentKind::ResourcePacks => 0,
        };
        (stability, loader)
    };
    Ok(versions
        .into_iter()
        .enumerate()
        .min_by_key(|(i, v)| (rank(v), *i))
        .map(|(_, v)| v))
}

/// Searches Modrinth for content compatible with the instance and marks installed projects.
pub fn search<T: Transport>(
    api: &Modrinth<T>,
    instance: &MinecraftInstance,
    kind: ContentKind,
    query: &str,
    offset: u64,
    installed: &HashSet<String>,
) -> Result<SearchPage, String> {
    if let Some(reason) = unavailable_reason(instance, kind) {
        return Err(reason);
    }
    let mut facets = vec![vec![format!("project_type:{}", kind.project_type())]];
    let loaders = loaders(instance, kind);
    if !loaders.is_empty() {
        facets.push(loaders.iter().map(|l| format!("categories:{l}")).collect());
    }
    if let Some(version) = &instance.minecraft_version {
        facets.push(vec![format!("versions:{version}")]);
    }
    let mut page = api.search(query, &facets, offset, 20)?;
    for hit in &mut page.hits {
        hit.installed = installed.contains(&hit.project_id);
    }
    Ok(page)
}

/// Identifies installed files on Modrinth and finds compatible updates, keyed by on-disk name.
/// Files that Modrinth does not host are omitted.
pub fn inspect<T: Transport>(
    api: &Modrinth<T>,
    instance: &MinecraftInstance,
    kind: ContentKind,
) -> Result<BTreeMap<String, RemoteDetails>, String> {
    let files = hashed_files(instance, kind, None)?;
    if files.is_empty() {
        return Ok(BTreeMap::new());
    }
    let hashes: Vec<String> = files.iter().map(|(_, h)| h.clone()).collect();
    let current = api.versions_by_hash(&hashes)?;
    let latest = if unavailable_reason(instance, kind).is_none() {
        api.latest_by_hash(&hashes, &loaders(instance, kind), &game_versions(instance))?
    } else {
        HashMap::new()
    };
    let projects = projects_by_id(api, current.values().map(|v| v.project_id.clone()));
    let mut details = BTreeMap::new();
    for (name, hash) in files {
        let Some(version) = current.get(&hash) else {
            continue;
        };
        let project = projects.get(&version.project_id);
        details.insert(
            name.clone(),
            RemoteDetails {
                project_id: version.project_id.clone(),
                title: project
                    .map(|p| p.title.clone())
                    .unwrap_or_else(|| canonical(&name).into()),
                icon: project.and_then(|p| p.icon_url.clone()).unwrap_or_default(),
                version: version.version_number.clone(),
                update: latest
                    .get(&hash)
                    .filter(|l| {
                        newer(l, version)
                            && l.project_id == version.project_id
                            && compatible(l, instance, kind)
                    })
                    .map(|l| l.version_number.clone()),
            },
        );
    }
    Ok(details)
}
fn game_versions(instance: &MinecraftInstance) -> Vec<String> {
    instance.minecraft_version.iter().cloned().collect()
}
fn newer(candidate: &api::Version, installed: &api::Version) -> bool {
    candidate.id != installed.id
        && candidate.date_published > installed.date_published
        && candidate.primary_file().is_some()
}
fn projects_by_id<T: Transport>(
    api: &Modrinth<T>,
    ids: impl IntoIterator<Item = String>,
) -> HashMap<String, api::Project> {
    let mut ids: Vec<String> = ids.into_iter().collect();
    ids.sort();
    ids.dedup();
    // Titles are cosmetic; a failed lookup must not undo completed work.
    api.projects(&ids)
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect()
}

/// Downloads and verifies a version file, then places it under `name` without replacing files.
fn download<T: Transport>(
    api: &Modrinth<T>,
    dir: &Path,
    file: &api::VersionFile,
) -> Result<tempfile::NamedTempFile, String> {
    let mut temp = temp_file(dir)?;
    api.download(file, temp.as_file_mut())?;
    temp.as_file()
        .sync_all()
        .map_err(|e| format!("Could not save {}: {e}", file.filename))?;
    Ok(temp)
}
fn place(temp: tempfile::NamedTempFile, dir: &Path, name: &str) -> Result<(), String> {
    temp.persist_noclobber(dir.join(name))
        .map(|_| ())
        .map_err(|e| format!("Could not save {name}: {}", e.error))
}

/// Installs the best compatible version of a Modrinth project and, for mods, its required
/// dependencies that are not already installed.
pub fn install<T: Transport>(
    api: &Modrinth<T>,
    instance: &MinecraftInstance,
    kind: ContentKind,
    project_id: &str,
) -> Result<Report, String> {
    install_version(api, instance, kind, project_id, None)
}

fn install_version<T: Transport>(
    api: &Modrinth<T>,
    instance: &MinecraftInstance,
    kind: ContentKind,
    project_id: &str,
    version_id: Option<&str>,
) -> Result<Report, String> {
    if let Some(reason) = unavailable_reason(instance, kind) {
        return Err(reason);
    }
    if !api::valid_id(project_id) {
        return Err("Invalid Modrinth project.".into());
    }
    let dir = folder(instance, kind);
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let hashes: Vec<String> = hashed_files(instance, kind, None)?
        .into_iter()
        .map(|(_, h)| h)
        .collect();
    let mut present: HashMap<String, String> = if hashes.is_empty() {
        HashMap::new()
    } else {
        api.versions_by_hash(&hashes)?
            .into_values()
            .map(|v| (v.project_id, v.id))
            .collect()
    };
    if present.contains_key(project_id) {
        return Err(format!(
            "This {} is already installed. Use Check for updates to update it.",
            kind.label()
        ));
    }
    let mut report = Report::default();
    let mut queue = VecDeque::from([(Some(project_id.to_string()), version_id.map(str::to_owned))]);
    let mut planned: Vec<api::Version> = vec![];
    while let Some((project, pinned)) = queue.pop_front() {
        let requested = planned.is_empty();
        if planned.len() >= INSTALL_LIMIT {
            return Err(format!(
                "This install exceeds {INSTALL_LIMIT} files. Add the dependencies in the owning launcher."
            ));
        }
        // Dependencies may name only a version; that version identifies the project.
        let pinned = pinned.as_deref().map(|id| api.version(id)).transpose()?;
        let Some(project) = project.or_else(|| pinned.as_ref().map(|v| v.project_id.clone()))
        else {
            continue;
        };
        if let Some(existing) = present.get(&project) {
            if pinned.as_ref().is_some_and(|v| &v.id != existing) {
                return Err(format!(
                    "Required dependency {project} conflicts with its installed or selected version. Resolve it in the launcher first."
                ));
            }
            continue;
        }
        let version = match pinned {
            Some(version)
                if compatible(&version, instance, kind) && version.project_id == project =>
            {
                version
            }
            Some(_) => {
                return Err(format!(
                    "Required dependency {project} is not compatible with this instance."
                ));
            }
            None => match best_version(api, instance, kind, &project) {
                Ok(Some(version)) => version,
                Err(e) => return Err(e),
                Ok(None) if requested => {
                    return Err(format!(
                        "No version supports Minecraft {}{}.",
                        instance.minecraft_version.as_deref().unwrap_or_default(),
                        instance
                            .loader
                            .as_deref()
                            .filter(|_| kind == ContentKind::Mods)
                            .map(|l| format!(" with {l}"))
                            .unwrap_or_default()
                    ));
                }
                Ok(None) => {
                    return Err(format!(
                        "No compatible version of required dependency {project} was found."
                    ));
                }
            },
        };
        present.insert(project, version.id.clone());
        if kind == ContentKind::Mods {
            for dependency in version
                .dependencies
                .iter()
                .filter(|d| d.dependency_type == "required")
            {
                match (&dependency.project_id, &dependency.version_id) {
                    (Some(p), _) if !api::valid_id(p) => (),
                    (Some(p), None) if present.contains_key(p) => (),
                    (None, None) => {
                        if let Some(file) = &dependency.file_name
                            && (!plain_name(file) || !exists(&dir.join(file)))
                        {
                            return Err(format!(
                                "Requires {file}, which is not hosted on Modrinth. Add it first."
                            ));
                        }
                    }
                    (p, v) => queue.push_back((p.clone(), v.clone())),
                }
            }
        }
        planned.push(version);
    }
    let projects = projects_by_id(api, planned.iter().map(|v| v.project_id.clone()));
    // Verify every required download before adding any files to the instance.
    let mut staged = Vec::new();
    for version in &planned {
        let file = version
            .primary_file()
            .expect("compatible versions have files");
        if !portable_name(&file.filename) || !kind.accepts(&file.filename) {
            return Err(format!(
                "Modrinth file name {} is not supported.",
                file.filename
            ));
        }
        if exists(&dir.join(&file.filename))
            || exists(&dir.join(format!("{}.disabled", file.filename)))
            || staged.iter().any(|(name, _)| name == &file.filename)
        {
            return Err(format!(
                "{} is already in this instance or install.",
                file.filename
            ));
        }
        staged.push((file.filename.clone(), download(api, &dir, file)?));
    }
    for (index, (version, (_, temp))) in planned.iter().zip(staged).enumerate() {
        let file = version
            .primary_file()
            .expect("compatible versions have files");
        let project = projects.get(&version.project_id);
        let title = project.map_or(file.filename.as_str(), |p| p.title.as_str());
        let result = place(temp, &dir, &file.filename);
        match result {
            Ok(()) => {
                if writes_metadata(instance)
                    && let Some(project) = project
                    && let Err(e) = packwiz::write(&dir, project, version, file)
                {
                    report
                        .notes
                        .push(format!("{title}: Prism metadata not written: {e}"));
                }
                report.changed.push(if index == 0 {
                    format!("Installed {title} {}", version.version_number)
                } else {
                    format!("Installed dependency {title}")
                });
            }
            Err(e) if index == 0 => return Err(e),
            Err(e) => report.notes.push(format!("{title}: {e}")),
        }
    }
    Ok(report)
}

/// Replaces outdated Modrinth files with their newest compatible versions, keeping each file's
/// enabled state. `only` limits the update to one on-disk file name.
pub fn update<T: Transport>(
    api: &Modrinth<T>,
    instance: &MinecraftInstance,
    kind: ContentKind,
    only: Option<&str>,
) -> Result<Report, String> {
    if let Some(reason) = unavailable_reason(instance, kind) {
        return Err(reason);
    }
    if let Some(name) = only {
        existing(instance, kind, name)?;
    }
    // Resolve dependencies against the whole instance even when updating just one file.
    let files = hashed_files(instance, kind, None)?;
    let hashes: Vec<String> = files.iter().map(|(_, h)| h.clone()).collect();
    if hashes.is_empty() {
        return Ok(Report::default());
    }
    let current = api.versions_by_hash(&hashes)?;
    let mut present: HashMap<String, String> = current
        .values()
        .map(|v| (v.project_id.clone(), v.id.clone()))
        .collect();
    let latest = api.latest_by_hash(&hashes, &loaders(instance, kind), &game_versions(instance))?;
    let projects = projects_by_id(api, current.values().map(|v| v.project_id.clone()));
    let dir = folder(instance, kind);
    let mut report = Report::default();
    for (name, hash) in files {
        if only.is_some_and(|only| only != name) {
            continue;
        }
        let (Some(installed), Some(candidate)) = (current.get(&hash), latest.get(&hash)) else {
            continue;
        };
        if !newer(candidate, installed)
            || candidate.project_id != installed.project_id
            || !compatible(candidate, instance, kind)
        {
            continue;
        }
        let file = candidate.primary_file().expect("checked by newer");
        let project = projects.get(&installed.project_id);
        let title = project.map_or(canonical(&name), |p| p.title.as_str());
        let disabled = name.ends_with(".disabled");
        let target = if disabled {
            format!("{}.disabled", file.filename)
        } else {
            file.filename.clone()
        };
        let result = (|| {
            if !portable_name(&file.filename) || !kind.accepts(&file.filename) {
                return Err(format!(
                    "Modrinth file name {} is not supported.",
                    file.filename
                ));
            }
            let replacing = canonical(&name) == file.filename;
            if !replacing
                && (exists(&dir.join(&file.filename))
                    || exists(&dir.join(format!("{}.disabled", file.filename))))
            {
                return Err(format!("{} is already in this instance.", file.filename));
            }
            // Keep the original if another launcher changed it during the download.
            let temp = download(api, &dir, file)?;
            if kind == ContentKind::Mods {
                for dependency in candidate
                    .dependencies
                    .iter()
                    .filter(|d| d.dependency_type == "required")
                {
                    let pinned = dependency
                        .version_id
                        .as_deref()
                        .map(|v| api.version(v))
                        .transpose()?;
                    let project = dependency
                        .project_id
                        .as_deref()
                        .or_else(|| pinned.as_ref().map(|v| v.project_id.as_str()));
                    let Some(project) = project else {
                        if let Some(file) = &dependency.file_name
                            && (!plain_name(file) || !exists(&dir.join(file)))
                        {
                            return Err(format!("Add required dependency {file} before updating."));
                        }
                        continue;
                    };
                    if let Some(version) = present.get(project) {
                        if pinned.as_ref().is_some_and(|p| &p.id != version) {
                            return Err(format!(
                                "Required dependency {project} needs version {}. Update it in the launcher first.",
                                pinned.as_ref().unwrap().version_number
                            ));
                        }
                        continue;
                    }
                    let dependencies = install_version(
                        api,
                        instance,
                        kind,
                        project,
                        dependency.version_id.as_deref(),
                    )?;
                    report.changed.extend(dependencies.changed);
                    report.notes.extend(dependencies.notes);
                    let hashes: Vec<_> = hashed_files(instance, kind, None)?
                        .into_iter()
                        .map(|(_, h)| h)
                        .collect();
                    present = api
                        .versions_by_hash(&hashes)?
                        .into_values()
                        .map(|v| (v.project_id, v.id))
                        .collect();
                }
            }
            let original = dir.join(&name);
            if sha512_file(&original)? != hash {
                return Err(format!(
                    "{name} changed during the update. Check for updates again."
                ));
            }
            if replacing {
                // Atomic replacement also replaces a content-store symlink, keeping its target intact.
                temp.persist(&original)
                    .map_err(|e| format!("Could not replace {name}: {}", busy_hint(e.error)))?;
            } else {
                place(temp, &dir, &target)?;
                if let Err(error) = fs::remove_file(&original) {
                    // Roll back the new name so the instance cannot load both versions.
                    let _ = fs::remove_file(dir.join(&target));
                    return Err(format!("Could not replace {name}: {}", busy_hint(error)));
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                if writes_metadata(instance) {
                    packwiz::remove_for_file(&dir, canonical(&name));
                    if let Some(project) = project
                        && let Err(e) = packwiz::write(&dir, project, candidate, file)
                    {
                        report
                            .notes
                            .push(format!("{title}: Prism metadata not written: {e}"));
                    }
                }
                report.changed.push(format!(
                    "Updated {title} {} → {}",
                    installed.version_number, candidate.version_number
                ));
                present.insert(candidate.project_id.clone(), candidate.id.clone());
            }
            Err(e) => report.notes.push(format!("{title}: {e}")),
        }
    }
    Ok(report)
}

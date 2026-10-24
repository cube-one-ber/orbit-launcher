use orbit_launcher::{
    instances::{self, ContentKind, InstanceSettings, Modrinth, Transport},
    model::{MinecraftInstance, SourceConfig},
    providers::{self, Prism, Provider},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha512};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

fn write(path: &Path, content: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn sha512(bytes: &[u8]) -> String {
    format!("{:x}", Sha512::digest(bytes))
}
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}
fn instance(root: &Path, launcher: &str, loader: Option<&str>) -> MinecraftInstance {
    MinecraftInstance {
        launcher: launcher.into(),
        root: root.into(),
        game_dir: root.join("minecraft"),
        minecraft_version: Some("1.21.1".into()),
        loader: loader.map(Into::into),
        ..Default::default()
    }
}

/// An in-memory Modrinth API that also records requests.
#[derive(Default)]
struct Fake {
    versions: Vec<Value>,
    projects: Vec<Value>,
    downloads: HashMap<String, Vec<u8>>,
    /// Installed file hash → newest compatible version ID.
    latest: HashMap<String, String>,
    requests: Mutex<Vec<String>>,
}
impl Fake {
    fn project(&mut self, id: &str, slug: &str, title: &str, kind: &str) {
        self.projects.push(json!({"id":id,"slug":slug,"title":title,"project_type":kind,
            "client_side":"required","server_side":"optional","icon_url":format!("https://cdn.modrinth.com/data/{id}/icon.png")}));
    }
    /// Adds a version and returns its file's content.
    #[allow(clippy::too_many_arguments)]
    fn version(
        &mut self,
        project: &str,
        id: &str,
        number: &str,
        kind: &str,
        date: &str,
        loaders: &[&str],
        filename: &str,
        dependencies: Value,
    ) -> Vec<u8> {
        let bytes = format!("{project} {id} {filename}").into_bytes();
        let url = format!("https://cdn.modrinth.com/data/{project}/versions/{id}/{filename}");
        self.versions.push(
            json!({"id":id,"project_id":project,"name":number,"version_number":number,
            "version_type":kind,"loaders":loaders,"game_versions":["1.21.1"],"date_published":date,
            "dependencies":dependencies,"files":[{"hashes":{"sha1":"0","sha512":sha512(&bytes)},
            "url":url,"filename":filename,"primary":true,"size":bytes.len()}]}),
        );
        self.downloads.insert(url, bytes.clone());
        bytes
    }
    fn find_version(&self, id: &str) -> Option<&Value> {
        self.versions.iter().find(|v| v["id"] == id)
    }
    fn log(&self, entry: String) {
        self.requests.lock().unwrap().push(entry);
    }
    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}
impl Transport for Fake {
    fn get(&self, url: &str, _: u64) -> Result<Vec<u8>, String> {
        self.log(format!("GET {url}"));
        let url = url::Url::parse(url).unwrap();
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        let path = url.path().trim_start_matches("/v2/");
        let parts: Vec<_> = path.split('/').collect();
        let value = match parts.as_slice() {
            ["project", project, "version"] => {
                let loaders: Option<Vec<String>> = query
                    .get("loaders")
                    .map(|l| serde_json::from_str(l).unwrap());
                let mut found: Vec<_> = self
                    .versions
                    .iter()
                    .filter(|v| v["project_id"] == *project)
                    .filter(|v| {
                        loaders.as_ref().is_none_or(|loaders| {
                            v["loaders"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|l| loaders.iter().any(|x| l == x))
                        })
                    })
                    .cloned()
                    .collect();
                found.sort_by(|a, b| {
                    b["date_published"]
                        .as_str()
                        .cmp(&a["date_published"].as_str())
                });
                json!(found)
            }
            ["version", id] => self.find_version(id).cloned().ok_or("HTTP 404")?,
            ["projects"] => {
                let ids: Vec<String> = serde_json::from_str(&query["ids"]).unwrap();
                json!(
                    self.projects
                        .iter()
                        .filter(|p| ids.iter().any(|id| p["id"] == *id))
                        .collect::<Vec<_>>()
                )
            }
            ["search"] => json!({"hits": self.projects.iter().map(|p| json!({"project_id":p["id"],
                "slug":p["slug"],"title":p["title"],"author":"Fixture","description":null,"downloads":5}))
                .collect::<Vec<_>>(), "offset":0, "total_hits": self.projects.len()}),
            other => return Err(format!("unexpected request {other:?}")),
        };
        Ok(value.to_string().into_bytes())
    }
    fn post_json(&self, url: &str, body: &Value, _: u64) -> Result<Vec<u8>, String> {
        self.log(format!("POST {url} {body}"));
        let hashes: Vec<String> = serde_json::from_value(body["hashes"].clone()).unwrap();
        let mut response = serde_json::Map::new();
        for hash in hashes {
            let version = if url.ends_with("/version_files/update") {
                self.latest.get(&hash).and_then(|id| self.find_version(id))
            } else {
                self.versions
                    .iter()
                    .find(|v| v["files"][0]["hashes"]["sha512"] == hash)
            };
            if let Some(version) = version {
                response.insert(hash, version.clone());
            }
        }
        Ok(Value::Object(response).to_string().into_bytes())
    }
    fn download(&self, url: &str, limit: u64, out: &mut dyn Write) -> Result<(), String> {
        self.log(format!("DOWNLOAD {url}"));
        let bytes = self.downloads.get(url).ok_or("HTTP 404")?;
        // Like ureq, fail once a body reaches the limit.
        out.write_all(&bytes[..bytes.len().min(limit as usize)])
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 >= limit {
            return Err("Download interrupted: body exceeds limit".into());
        }
        Ok(())
    }
}

#[test]
fn prism_instances_describe_loader_game_folder_and_modpack() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Prism data");
    let quilt = root.join("instances/quilt-pack");
    write(
        &quilt.join("instance.cfg"),
        "[General]\nConfigVersion=1.3\nManagedPack=true\nManagedPackName=Fixture Pack\nname=\"Create, Above and Beyond\"\n",
    );
    write(
        &quilt.join("mmc-pack.json"),
        json!({"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"org.quiltmc.quilt-loader","version":"0.26.0"}]}).to_string(),
    );
    fs::create_dir_all(quilt.join(".minecraft/mods")).unwrap();
    let vanilla = root.join("instances/vanilla-fixture");
    write(&vanilla.join("instance.cfg"), "name=Vanilla fixture\n");
    write(
        &vanilla.join("mmc-pack.json"),
        json!({"components":[{"uid":"net.minecraft","version":"1.20.4"}]}).to_string(),
    );
    fs::create_dir_all(vanilla.join(".minecraft")).unwrap();
    fs::create_dir_all(vanilla.join("minecraft")).unwrap();
    let config = SourceConfig {
        paths: vec![root.clone()],
        command: vec!["prismlauncher".into()],
        ..Default::default()
    };
    let (games, errors) = Prism.discover(&config);
    assert!(errors.is_empty(), "{errors:?}");
    let game = games
        .iter()
        .find(|g| g.title == "Create, Above and Beyond")
        .unwrap();
    let details = game.instance.as_ref().unwrap();
    assert_eq!(details.launcher, "prism");
    assert_eq!(details.root, quilt);
    assert_eq!(details.game_dir, quilt.join(".minecraft"));
    assert_eq!(details.loader.as_deref(), Some("quilt"));
    assert_eq!(details.loader_version.as_deref(), Some("0.26.0"));
    assert_eq!(details.modpack.as_deref(), Some("Fixture Pack"));
    assert_eq!(
        instances::loaders(details, ContentKind::Mods),
        ["quilt", "fabric"]
    );
    let vanilla_game = games.iter().find(|g| g.title == "Vanilla fixture").unwrap();
    let details = vanilla_game.instance.as_ref().unwrap();
    assert_eq!(details.game_dir, vanilla.join("minecraft"));
    assert_eq!(details.loader, None);
    assert!(instances::supports_settings(details));
    assert!(
        instances::unavailable_reason(details, ContentKind::Mods)
            .unwrap()
            .contains("no mod loader")
    );
    assert!(instances::unavailable_reason(details, ContentKind::ResourcePacks).is_none());
}

#[test]
fn content_files_toggle_add_and_remove_with_prism_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let mc = instance(temp.path(), "prism", Some("fabric"));
    let mods = mc.game_dir.join("mods");
    write(&mods.join("sodium.jar"), "sodium");
    write(&mods.join("old-mod.jar.disabled"), "old");
    write(&mods.join("notes.txt"), "ignored");
    write(&mods.join(".hidden.jar"), "ignored");
    write(
        &mods.join(".index/sodium.pw.toml"),
        "filename = \"sodium.jar\"\nname = \"Sodium\"\n",
    );
    write(
        &mods.join(".index/other.pw.toml"),
        "filename = \"other.jar\"\n",
    );
    fs::create_dir_all(mc.game_dir.join("resourcepacks/Unpacked pack")).unwrap();
    let items = instances::list(&mc, ContentKind::Mods).unwrap();
    assert_eq!(
        items
            .iter()
            .map(|i| (i.file_name.as_str(), i.name.as_str(), i.enabled))
            .collect::<Vec<_>>(),
        [
            ("old-mod.jar.disabled", "old-mod.jar", false),
            ("sodium.jar", "sodium.jar", true)
        ]
    );
    let packs = instances::list(&mc, ContentKind::ResourcePacks).unwrap();
    assert!(packs[0].folder && packs[0].file_name == "Unpacked pack");
    assert!(
        instances::list(&mc, ContentKind::ShaderPacks)
            .unwrap()
            .is_empty()
    );

    assert_eq!(
        instances::set_enabled(&mc, ContentKind::Mods, "sodium.jar", false).unwrap(),
        "sodium.jar.disabled"
    );
    assert_eq!(
        instances::set_enabled(&mc, ContentKind::Mods, "old-mod.jar.disabled", true).unwrap(),
        "old-mod.jar"
    );
    write(&mods.join("sodium.jar"), "duplicate");
    assert!(
        instances::set_enabled(&mc, ContentKind::Mods, "sodium.jar.disabled", true)
            .unwrap_err()
            .contains("Remove one copy")
    );
    fs::remove_file(mods.join("sodium.jar")).unwrap();
    for bad in [
        "../escape.jar",
        "sub/dir.jar",
        ".index",
        "notes.txt",
        "missing.jar",
    ] {
        assert!(
            instances::set_enabled(&mc, ContentKind::Mods, bad, true).is_err(),
            "{bad}"
        );
        assert!(
            instances::remove(&mc, ContentKind::Mods, bad).is_err(),
            "{bad}"
        );
    }
    assert!(
        instances::remove(&mc, ContentKind::ResourcePacks, "Unpacked pack").is_err(),
        "folders are never deleted"
    );
    assert!(mc.game_dir.join("resourcepacks/Unpacked pack").is_dir());

    instances::remove(&mc, ContentKind::Mods, "sodium.jar.disabled").unwrap();
    assert_eq!(names(&mods.join(".index")), ["other.pw.toml"]);
    assert!(!mods.join("sodium.jar.disabled").exists());

    let source = temp.path().join("Downloads/Zoë's pack.zip");
    write(&source, "pack");
    assert_eq!(
        instances::add_file(&mc, ContentKind::ResourcePacks, &source).unwrap(),
        "Zoë's pack.zip"
    );
    assert_eq!(
        fs::read(mc.game_dir.join("resourcepacks/Zoë's pack.zip")).unwrap(),
        b"pack"
    );
    assert!(
        instances::add_file(&mc, ContentKind::ResourcePacks, &source)
            .unwrap_err()
            .contains("already")
    );
    assert!(instances::add_file(&mc, ContentKind::Mods, &source.with_extension("exe")).is_err());
    assert!(
        instances::add_file(
            &mc,
            ContentKind::ShaderPacks,
            &temp.path().join("missing.zip")
        )
        .is_err()
    );
    assert!(
        !names(&mc.game_dir.join("resourcepacks"))
            .iter()
            .any(|n| n.ends_with(".part"))
    );
}

/// A Fabric instance with Fabric API installed; Iris requires Sodium and Fabric API.
fn iris_fixture(temp: &Path, launcher: &str) -> (MinecraftInstance, Fake) {
    let mc = instance(temp, launcher, Some("fabric"));
    let mut fake = Fake::default();
    fake.project("IRIS0001", "iris", "Iris Shaders", "mod");
    fake.project("SODIUM01", "sodium", "Sodium", "mod");
    fake.project("FABRICAP", "fabric-api", "Fabric API", "mod");
    let api = fake.version(
        "FABRICAP",
        "FAPI0001",
        "0.100.0",
        "release",
        "2026-01-01T00:00:00Z",
        &["fabric"],
        "fabric-api.jar",
        json!([]),
    );
    write(&mc.game_dir.join("mods/fabric-api.jar"), api);
    let deps = json!([{"project_id":"SODIUM01","version_id":null,"dependency_type":"required"},
        {"project_id":"FABRICAP","version_id":null,"dependency_type":"required"},
        {"project_id":"OPTIONAL","version_id":null,"dependency_type":"optional"}]);
    fake.version(
        "IRIS0001",
        "IRISREL1",
        "1.8.0",
        "release",
        "2026-02-01T00:00:00Z",
        &["fabric"],
        "iris-1.8.0.jar",
        deps.clone(),
    );
    fake.version(
        "IRIS0001",
        "IRISBETA",
        "1.9.0-beta",
        "beta",
        "2026-03-01T00:00:00Z",
        &["fabric"],
        "iris-1.9.0-beta.jar",
        deps,
    );
    fake.version(
        "IRIS0001",
        "IRISNEO1",
        "1.8.0-neoforge",
        "release",
        "2026-04-01T00:00:00Z",
        &["neoforge"],
        "iris-neoforge.jar",
        json!([]),
    );
    fake.version(
        "SODIUM01",
        "SODIUM08",
        "0.8.0",
        "release",
        "2026-02-01T00:00:00Z",
        &["fabric", "quilt"],
        "sodium-0.8.0.jar",
        json!([]),
    );
    (mc, fake)
}

#[test]
fn install_resolves_dependencies_verifies_downloads_and_writes_prism_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let (mc, fake) = iris_fixture(temp.path(), "prism");
    let api = Modrinth::new(fake);
    let report = instances::install(&api, &mc, ContentKind::Mods, "IRIS0001").unwrap();
    assert_eq!(
        report.changed,
        [
            "Installed Iris Shaders 1.8.0",
            "Installed dependency Sodium"
        ]
    );
    assert!(report.notes.is_empty(), "{:?}", report.notes);
    let mods = mc.game_dir.join("mods");
    assert_eq!(
        names(&mods),
        [
            ".index",
            "fabric-api.jar",
            "iris-1.8.0.jar",
            "sodium-0.8.0.jar"
        ]
    );
    assert_eq!(
        names(&mods.join(".index")),
        ["iris.pw.toml", "sodium.pw.toml"]
    );
    let metadata = fs::read_to_string(mods.join(".index/iris.pw.toml")).unwrap();
    for line in [
        "filename = \"iris-1.8.0.jar\"",
        "name = \"Iris Shaders\"",
        "side = \"both\"",
        "hash-format = \"sha512\"",
        "mode = \"url\"",
        "[update.modrinth]",
        "mod-id = \"IRIS0001\"",
        "version = \"IRISREL1\"",
        "x-prismlauncher-release-type = \"Release\"",
        "x-prismlauncher-loaders = [\"fabric\"]",
    ] {
        assert!(metadata.contains(line), "{line}\n{metadata}");
    }
    assert!(metadata.contains("addonId = \"SODIUM01\", type = \"REQUIRED\""));
    let downloads: Vec<_> = api_requests(&api)
        .into_iter()
        .filter(|r| r.starts_with("DOWNLOAD"))
        .collect();
    assert_eq!(downloads.len(), 2, "Fabric API is already installed");
    // A second request is refused before downloading anything.
    assert!(
        instances::install(&api, &mc, ContentKind::Mods, "IRIS0001")
            .unwrap_err()
            .contains("already installed")
    );
    assert!(instances::install(&api, &mc, ContentKind::Mods, "../bad").is_err());
}
fn api_requests(api: &Modrinth<Fake>) -> Vec<String> {
    api.transport().requests()
}

#[test]
fn corrupt_downloads_leave_no_files() {
    let temp = tempfile::tempdir().unwrap();
    let (mc, mut fake) = iris_fixture(temp.path(), "prism");
    for bytes in fake.downloads.values_mut() {
        *bytes.last_mut().unwrap() ^= 1;
    }
    let api = Modrinth::new(fake);
    let error = instances::install(&api, &mc, ContentKind::Mods, "IRIS0001").unwrap_err();
    assert!(error.contains("failed verification"), "{error}");
    assert_eq!(names(&mc.game_dir.join("mods")), ["fabric-api.jar"]);

    let (mc, mut fake) = iris_fixture(&temp.path().join("oversized"), "prism");
    for bytes in fake.downloads.values_mut() {
        bytes.push(b'!');
    }
    let api = Modrinth::new(fake);
    let error = instances::install(&api, &mc, ContentKind::Mods, "IRIS0001").unwrap_err();
    assert!(error.contains("exceeds limit"), "{error}");
    assert_eq!(names(&mc.game_dir.join("mods")), ["fabric-api.jar"]);
}

#[test]
fn packs_install_without_a_loader_and_metadata_stays_out_of_multimc() {
    let temp = tempfile::tempdir().unwrap();
    let mc = instance(temp.path(), "multimc", None);
    let mut fake = Fake::default();
    fake.project("FAITHFUL", "faithful", "Faithful", "resourcepack");
    fake.version(
        "FAITHFUL",
        "FAITH001",
        "1.21",
        "release",
        "2026-01-01T00:00:00Z",
        &["minecraft"],
        "Faithful.zip",
        json!([]),
    );
    let api = Modrinth::new(fake);
    assert!(
        instances::install(&api, &mc, ContentKind::Mods, "FAITHFUL")
            .unwrap_err()
            .contains("no mod loader")
    );
    instances::install(&api, &mc, ContentKind::ResourcePacks, "FAITHFUL").unwrap();
    assert_eq!(names(&mc.game_dir.join("resourcepacks")), ["Faithful.zip"]);

    let page = instances::search(
        &api,
        &mc,
        ContentKind::ResourcePacks,
        "faith",
        0,
        &HashSet::from(["FAITHFUL".to_string()]),
    )
    .unwrap();
    assert!(page.hits[0].installed && page.hits[0].description.is_empty());
    let search = api_requests(&api)
        .into_iter()
        .find(|r| r.contains("/search"))
        .unwrap();
    let url = url::Url::parse(search.trim_start_matches("GET ")).unwrap();
    let facets: Value =
        serde_json::from_str(&url.query_pairs().find(|(k, _)| k == "facets").unwrap().1).unwrap();
    assert_eq!(
        facets,
        json!([["project_type:resourcepack"], ["versions:1.21.1"]])
    );
}

#[test]
fn quilt_searches_include_fabric_mods() {
    let temp = tempfile::tempdir().unwrap();
    let mc = instance(temp.path(), "modrinth", Some("quilt"));
    let api = Modrinth::new(Fake::default());
    instances::search(&api, &mc, ContentKind::Mods, "", 20, &HashSet::new()).unwrap();
    let request = api_requests(&api).pop().unwrap();
    let url = url::Url::parse(request.trim_start_matches("GET ")).unwrap();
    let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query["index"], "downloads");
    assert_eq!(query["offset"], "20");
    assert_eq!(
        serde_json::from_str::<Value>(&query["facets"]).unwrap(),
        json!([
            ["project_type:mod"],
            ["categories:quilt", "categories:fabric"],
            ["versions:1.21.1"]
        ])
    );
}

#[test]
fn updates_replace_files_and_keep_them_disabled() {
    let temp = tempfile::tempdir().unwrap();
    let (mc, mut fake) = iris_fixture(temp.path(), "prism");
    let old = fake
        .downloads
        .iter()
        .find(|(url, _)| url.ends_with("sodium-0.8.0.jar"))
        .unwrap()
        .1
        .clone();
    write(&mc.game_dir.join("mods/sodium-0.8.0.jar.disabled"), &old);
    fake.version(
        "SODIUM01",
        "SODIUM09",
        "0.9.0",
        "release",
        "2026-05-01T00:00:00Z",
        &["fabric"],
        "sodium-0.9.0.jar",
        json!([]),
    );
    fake.latest.insert(sha512(&old), "SODIUM09".into());
    let api = Modrinth::new(fake);
    let details = instances::inspect(&api, &mc, ContentKind::Mods).unwrap();
    let sodium = &details["sodium-0.8.0.jar.disabled"];
    assert_eq!(sodium.title, "Sodium");
    assert_eq!(sodium.version, "0.8.0");
    assert_eq!(sodium.update.as_deref(), Some("0.9.0"));
    assert_eq!(details["fabric-api.jar"].update, None);

    let report = instances::update(
        &api,
        &mc,
        ContentKind::Mods,
        Some("sodium-0.8.0.jar.disabled"),
    )
    .unwrap();
    assert_eq!(report.changed, ["Updated Sodium 0.8.0 → 0.9.0"]);
    let mods = mc.game_dir.join("mods");
    assert_eq!(
        names(&mods),
        [".index", "fabric-api.jar", "sodium-0.9.0.jar.disabled"]
    );
    let metadata = fs::read_to_string(mods.join(".index/sodium.pw.toml")).unwrap();
    assert!(metadata.contains("filename = \"sodium-0.9.0.jar\""));
    assert!(metadata.contains("version = \"SODIUM09\""));
    // Nothing is left to update.
    let report = instances::update(&api, &mc, ContentKind::Mods, None).unwrap();
    assert!(report.changed.is_empty());
}

#[test]
fn failed_required_download_keeps_the_instance_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let (mc, mut fake) = iris_fixture(temp.path(), "prism");
    for (url, bytes) in &mut fake.downloads {
        if url.ends_with("sodium-0.8.0.jar") {
            *bytes.last_mut().unwrap() ^= 1;
        }
    }
    let error =
        instances::install(&Modrinth::new(fake), &mc, ContentKind::Mods, "IRIS0001").unwrap_err();
    assert!(error.contains("failed verification"), "{error}");
    assert_eq!(names(&mc.game_dir.join("mods")), ["fabric-api.jar"]);
}

#[test]
fn updates_with_the_same_filename_preserve_original_on_failure() {
    let temp = tempfile::tempdir().unwrap();
    let (mc, mut fake) = iris_fixture(temp.path(), "modrinth");
    let original = fs::read(mc.game_dir.join("mods/fabric-api.jar")).unwrap();
    let replacement = fake.version(
        "FABRICAP",
        "FAPI0002",
        "0.101.0",
        "release",
        "2026-06-01T00:00:00Z",
        &["fabric"],
        "fabric-api.jar",
        json!([]),
    );
    fake.latest.insert(sha512(&original), "FAPI0002".into());
    let url = fake
        .downloads
        .iter()
        .find(|(_, bytes)| **bytes == replacement)
        .unwrap()
        .0
        .clone();
    fake.downloads
        .insert(url.clone(), vec![0; replacement.len()]);
    let api = Modrinth::new(fake);
    let report = instances::update(&api, &mc, ContentKind::Mods, None).unwrap();
    assert!(report.changed.is_empty());
    assert!(report.notes[0].contains("failed verification"));
    assert_eq!(
        fs::read(mc.game_dir.join("mods/fabric-api.jar")).unwrap(),
        original
    );
    let (mc, mut fake) = iris_fixture(&temp.path().join("healthy"), "modrinth");
    fake.version(
        "FABRICAP",
        "FAPI0002",
        "0.101.0",
        "release",
        "2026-06-01T00:00:00Z",
        &["fabric"],
        "fabric-api.jar",
        json!([]),
    );
    fake.latest.insert(sha512(&original), "FAPI0002".into());
    fake.downloads.insert(url, replacement.clone());
    let report = instances::update(&Modrinth::new(fake), &mc, ContentKind::Mods, None).unwrap();
    assert_eq!(report.changed, ["Updated Fabric API 0.100.0 → 0.101.0"]);
    assert_eq!(
        fs::read(mc.game_dir.join("mods/fabric-api.jar")).unwrap(),
        replacement
    );
    assert_eq!(names(&mc.game_dir.join("mods")), ["fabric-api.jar"]);
}

#[test]
fn updates_install_new_required_dependencies_before_replacing_the_original() {
    for corrupt in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let (mc, mut fake) = iris_fixture(temp.path(), "prism");
        let original = fs::read(mc.game_dir.join("mods/fabric-api.jar")).unwrap();
        let replacement = fake.version(
            "FABRICAP",
            "FAPI0002",
            "0.101.0",
            "release",
            "2026-06-01T00:00:00Z",
            &["fabric"],
            "fabric-api-new.jar",
            json!([{"project_id":"SODIUM01","version_id":"SODIUM08","dependency_type":"required"}]),
        );
        fake.latest.insert(sha512(&original), "FAPI0002".into());
        if corrupt {
            for (url, bytes) in &mut fake.downloads {
                if url.ends_with("sodium-0.8.0.jar") {
                    *bytes.last_mut().unwrap() ^= 1;
                }
            }
        }
        let report = instances::update(
            &Modrinth::new(fake),
            &mc,
            ContentKind::Mods,
            Some("fabric-api.jar"),
        )
        .unwrap();
        if corrupt {
            assert!(report.changed.is_empty());
            assert!(report.notes[0].contains("failed verification"));
            assert_eq!(
                fs::read(mc.game_dir.join("mods/fabric-api.jar")).unwrap(),
                original
            );
            assert_eq!(names(&mc.game_dir.join("mods")), ["fabric-api.jar"]);
        } else {
            assert_eq!(report.changed.len(), 2, "{report:?}");
            assert_eq!(
                fs::read(mc.game_dir.join("mods/fabric-api-new.jar")).unwrap(),
                replacement
            );
            assert!(mc.game_dir.join("mods/sodium-0.8.0.jar").is_file());
            assert!(!mc.game_dir.join("mods/fabric-api.jar").exists());
        }
    }
}

#[test]
fn session_preserves_install_notes_after_refreshing_details() {
    use instances::session::{Job, Outcome, Output, Session};
    let temp = tempfile::tempdir().unwrap();
    let game = serde_json::from_value(json!({"id":"test-instance","title":"Fabric","provider":"prism","subtitle":"Minecraft","artwork":"","instance":instance(temp.path(), "prism", Some("fabric"))})).unwrap();
    let mut session = Session::new(&game, false).unwrap();
    let follow_up = session
        .apply(Outcome {
            game_id: "test-instance".into(),
            job: Job::Install {
                kind: ContentKind::Mods,
                project: "IRIS0001".into(),
            },
            result: Ok(Output::Report(instances::Report {
                changed: vec!["Installed Iris".into()],
                notes: vec!["Prism metadata could not be written".into()],
            })),
        })
        .unwrap();
    session.started(&follow_up);
    session.apply(Outcome {
        game_id: "test-instance".into(),
        job: follow_up,
        result: Ok(Output::Details(Default::default())),
    });
    let view = session.view();
    assert_eq!(view["status"], "Installed Iris");
    assert_eq!(view["error"], "Prism metadata could not be written");
    let mut preview = Session::new(&game, true).unwrap();
    assert!(!preview.toggle(ContentKind::Mods, "local.jar"));
    assert!(!preview.save_settings("{}"));
    assert!(names(temp.path()).is_empty());
}

#[test]
fn modrinth_profiles_change_through_files_without_touching_the_database() {
    let temp = tempfile::tempdir().unwrap();
    let profile = temp.path().join("profiles/Fabric world");
    write(&profile.join("mods/lithium.jar"), "lithium");
    let db = temp.path().join("app.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection.execute_batch("CREATE TABLE instances(id TEXT, path TEXT, name TEXT, install_stage TEXT, icon_path TEXT, applied_content_set_id TEXT, last_played INTEGER);
        CREATE TABLE instance_content_sets(id TEXT, instance_id TEXT, game_version TEXT, loader TEXT);
        CREATE TABLE instance_links(instance_id TEXT, modrinth_project_id TEXT);
        INSERT INTO instances VALUES('fabric-id','Fabric world','Fabric world','installed',NULL,'set',1);
        INSERT INTO instance_content_sets VALUES('set','fabric-id','1.21.1','fabric');").unwrap();
    drop(connection);
    let before = fs::read(&db).unwrap();
    let config = SourceConfig {
        paths: vec![temp.path().into()],
        ..Default::default()
    };
    let (games, errors) = providers::Modrinth.discover(&config);
    assert!(errors.is_empty(), "{errors:?}");
    let game = games.iter().find(|g| g.title == "Fabric world").unwrap();
    let mc = game.instance.clone().unwrap();
    assert_eq!(mc.launcher, "modrinth");
    assert_eq!(mc.game_dir, profile);
    assert_eq!(mc.loader.as_deref(), Some("fabric"));
    assert_eq!(mc.modpack, None);
    assert!(!instances::supports_settings(&mc));
    instances::set_enabled(&mc, ContentKind::Mods, "lithium.jar", false).unwrap();
    let source = temp.path().join("extra.jar");
    write(&source, "extra");
    instances::add_file(&mc, ContentKind::Mods, &source).unwrap();
    assert_eq!(
        names(&profile.join("mods")),
        ["extra.jar", "lithium.jar.disabled"]
    );
    assert_eq!(before, fs::read(db).unwrap());
}

#[test]
fn settings_edits_change_only_requested_keys() {
    let temp = tempfile::tempdir().unwrap();
    let cfg = temp.path().join("instance.cfg");
    let original = "[General]\r\nConfigVersion=1.3\r\nInstanceType=OneSix\r\nJvmArgs=-Dkeep=1\r\nname=Old name\r\ntotalTimePlayed=42\r\n\r\n[UI]\r\nname=unrelated\r\n";
    write(&cfg, original);
    let settings = instances::read_settings(temp.path()).unwrap();
    assert_eq!(settings.name, "Old name");
    assert_eq!(settings.java_args, "-Dkeep=1");
    assert!(!settings.override_memory && settings.max_memory == 4096);
    let edited = InstanceSettings {
        name: "New, \"quoted\" name".into(),
        notes: "Line one\nLine #2".into(),
        override_memory: true,
        min_memory: 1024,
        max_memory: 6144,
        ..settings.clone()
    };
    instances::write_settings(temp.path(), &edited).unwrap();
    assert_eq!(
        fs::read_to_string(&cfg).unwrap(),
        "[General]\r\nConfigVersion=1.3\r\nInstanceType=OneSix\r\nJvmArgs=-Dkeep=1\r\nname=\"New, \\\"quoted\\\" name\"\r\ntotalTimePlayed=42\r\nnotes=Line one\\nLine #2\r\nOverrideMemory=true\r\nMinMemAlloc=1024\r\nMaxMemAlloc=6144\r\n\r\n[UI]\r\nname=unrelated\r\n"
    );
    assert_eq!(instances::read_settings(temp.path()).unwrap(), edited);

    // MultiMC's legacy format escapes comments and newlines its own way.
    write(&cfg, "InstanceType=OneSix\nname=Legacy\n");
    let legacy = InstanceSettings {
        name: "Legacy #1".into(),
        notes: "a\nb".into(),
        override_java_args: true,
        java_args: "-XX:+UseG1GC".into(),
        ..InstanceSettings::default()
    };
    instances::write_settings(temp.path(), &legacy).unwrap();
    assert_eq!(
        fs::read_to_string(&cfg).unwrap(),
        "InstanceType=OneSix\nname=Legacy \\#1\nnotes=a\\nb\nOverrideJavaArgs=true\nJvmArgs=-XX:+UseG1GC\n"
    );
    assert_eq!(
        instances::read_settings(temp.path()).unwrap().name,
        "Legacy #1"
    );

    let before = fs::read(&cfg).unwrap();
    for invalid in [
        InstanceSettings {
            name: " ".into(),
            ..legacy.clone()
        },
        InstanceSettings {
            override_memory: true,
            min_memory: 8192,
            max_memory: 1024,
            ..legacy.clone()
        },
        InstanceSettings {
            java_args: "-Da\n-Db".into(),
            ..legacy.clone()
        },
    ] {
        assert!(instances::write_settings(temp.path(), &invalid).is_err());
    }
    assert_eq!(before, fs::read(&cfg).unwrap());
    assert!(instances::write_settings(&PathBuf::from("/nonexistent-orbit"), &legacy).is_err());
}

/// Exercises the public Modrinth API: `cargo test --test instances -- --ignored`.
#[test]
#[ignore = "requires network access to api.modrinth.com"]
fn live_modrinth_search_install_and_inspect() {
    let temp = tempfile::tempdir().unwrap();
    let mc = instance(temp.path(), "prism", Some("fabric"));
    let api = Modrinth::default();
    let page = instances::search(&api, &mc, ContentKind::Mods, "iris", 0, &HashSet::new()).unwrap();
    let iris = page.hits.iter().find(|h| h.slug == "iris").unwrap();
    let report = instances::install(&api, &mc, ContentKind::Mods, &iris.project_id).unwrap();
    assert!(
        report.changed.iter().any(|c| c.contains("Sodium")),
        "{report:?}"
    );
    let details = instances::inspect(&api, &mc, ContentKind::Mods).unwrap();
    assert!(
        details.values().any(|d| d.title.contains("Iris")),
        "{details:?}"
    );
    assert!(names(&mc.game_dir.join("mods/.index")).len() >= 2);
    let shaders =
        instances::search(&api, &mc, ContentKind::ShaderPacks, "", 0, &HashSet::new()).unwrap();
    instances::install(
        &api,
        &mc,
        ContentKind::ShaderPacks,
        &shaders.hits[0].project_id,
    )
    .unwrap();
    let packs = instances::list(&mc, ContentKind::ShaderPacks).unwrap();
    assert_eq!(packs.len(), 1);
    eprintln!(
        "{report:?}\n{:?}\n{packs:?}",
        names(&mc.game_dir.join("mods"))
    );
}

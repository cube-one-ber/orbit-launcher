//! A small, bounded Modrinth API v2 client for instance content.
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha512};
use std::{collections::HashMap, io::Write, time::Duration};

pub const API: &str = "https://api.modrinth.com/v2";
const JSON_LIMIT: u64 = 16 * 1024 * 1024;
/// Largest single file Orbit downloads into an instance.
pub const FILE_LIMIT: u64 = 1024 * 1024 * 1024;

/// HTTP access, replaceable in tests.
pub trait Transport: Send + Sync {
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String>;
    fn post_json(&self, url: &str, body: &Value, limit: u64) -> Result<Vec<u8>, String>;
    /// Streams at most `limit` bytes of a download into `out`.
    fn download(&self, url: &str, limit: u64, out: &mut dyn Write) -> Result<(), String>;
}

pub struct HttpTransport {
    api: ureq::Agent,
    files: ureq::Agent,
}
impl Default for HttpTransport {
    fn default() -> Self {
        let agent = |timeout| {
            ureq::Agent::config_builder()
                .https_only(true)
                .max_redirects(4)
                .timeout_connect(Some(Duration::from_secs(15)))
                .timeout_global(Some(timeout))
                .user_agent(concat!(
                    "Orbit/",
                    env!("CARGO_PKG_VERSION"),
                    " (game launcher; Minecraft instance content; https://github.com/cube-one-ber/orbit-launcher)"
                ))
                .build()
                .into()
        };
        Self {
            api: agent(Duration::from_secs(30)),
            files: agent(Duration::from_secs(30 * 60)),
        }
    }
}
fn http_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(429) => {
            "Modrinth is limiting requests. Wait a minute and try again.".into()
        }
        ureq::Error::StatusCode(code) => format!("Modrinth returned HTTP {code}"),
        other => format!("Could not reach Modrinth: {other}"),
    }
}
impl Transport for HttpTransport {
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        self.api
            .get(url)
            .call()
            .map_err(http_error)?
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map_err(http_error)
    }
    fn post_json(&self, url: &str, body: &Value, limit: u64) -> Result<Vec<u8>, String> {
        self.api
            .post(url)
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(http_error)?
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map_err(http_error)
    }
    fn download(&self, url: &str, limit: u64, out: &mut dyn Write) -> Result<(), String> {
        let mut response = self.files.get(url).call().map_err(http_error)?;
        let mut reader = response.body_mut().with_config().limit(limit).reader();
        std::io::copy(&mut reader, out)
            .map(|_| ())
            .map_err(|e| format!("Download interrupted: {e}"))
    }
}

/// Treats JSON `null` like a missing field.
fn nullable<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Hashes {
    pub sha1: Option<String>,
    pub sha512: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VersionFile {
    pub hashes: Hashes,
    #[serde(deserialize_with = "nullable")]
    pub url: String,
    #[serde(deserialize_with = "nullable")]
    pub filename: String,
    #[serde(deserialize_with = "nullable")]
    pub primary: bool,
    #[serde(deserialize_with = "nullable")]
    pub size: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub file_name: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub dependency_type: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Version {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub project_id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    #[serde(deserialize_with = "nullable")]
    pub version_number: String,
    #[serde(deserialize_with = "nullable")]
    pub version_type: String,
    #[serde(deserialize_with = "nullable")]
    pub loaders: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub game_versions: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub files: Vec<VersionFile>,
    #[serde(deserialize_with = "nullable")]
    pub dependencies: Vec<Dependency>,
    #[serde(deserialize_with = "nullable")]
    pub date_published: String,
}
impl Version {
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub slug: String,
    #[serde(deserialize_with = "nullable")]
    pub title: String,
    pub icon_url: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub project_type: String,
    #[serde(deserialize_with = "nullable")]
    pub client_side: String,
    #[serde(deserialize_with = "nullable")]
    pub server_side: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchHit {
    #[serde(deserialize_with = "nullable")]
    pub project_id: String,
    #[serde(deserialize_with = "nullable")]
    pub slug: String,
    #[serde(deserialize_with = "nullable")]
    pub title: String,
    #[serde(deserialize_with = "nullable")]
    pub author: String,
    #[serde(deserialize_with = "nullable")]
    pub description: String,
    pub icon_url: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub downloads: u64,
    /// Cached local icon for display; filled after the search.
    #[serde(skip_deserializing)]
    pub icon: String,
    #[serde(skip_deserializing)]
    pub installed: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchPage {
    #[serde(deserialize_with = "nullable")]
    pub hits: Vec<SearchHit>,
    #[serde(deserialize_with = "nullable")]
    pub offset: u64,
    #[serde(deserialize_with = "nullable")]
    pub total_hits: u64,
}

/// Modrinth IDs and slugs placed in request paths.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Downloads accepted from Modrinth version metadata.
pub fn trusted_download(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.host_str()
                .is_some_and(|h| h == "cdn.modrinth.com" || h.ends_with(".modrinth.com"))
    })
}

pub struct Modrinth<T: Transport = HttpTransport> {
    transport: T,
}
impl Default for Modrinth<HttpTransport> {
    fn default() -> Self {
        Self::new(HttpTransport::default())
    }
}
impl<T: Transport> Modrinth<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
    pub fn transport(&self) -> &T {
        &self.transport
    }
    fn url(path: &str, query: &[(&str, String)]) -> String {
        let mut url = url::Url::parse(&format!("{API}{path}")).expect("constant API URL");
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
        }
        url.to_string()
    }
    fn get<D: DeserializeOwned>(&self, url: &str) -> Result<D, String> {
        let bytes = self.transport.get(url, JSON_LIMIT)?;
        serde_json::from_slice(&bytes).map_err(|e| format!("Unexpected Modrinth response: {e}"))
    }
    fn post<D: DeserializeOwned>(&self, url: &str, body: &Value) -> Result<D, String> {
        let bytes = self.transport.post_json(url, body, JSON_LIMIT)?;
        serde_json::from_slice(&bytes).map_err(|e| format!("Unexpected Modrinth response: {e}"))
    }

    /// `facets` are AND-ed groups of OR-ed Modrinth search facets.
    pub fn search(
        &self,
        query: &str,
        facets: &[Vec<String>],
        offset: u64,
        limit: u64,
    ) -> Result<SearchPage, String> {
        let index = if query.trim().is_empty() {
            "downloads"
        } else {
            "relevance"
        };
        self.get(&Self::url(
            "/search",
            &[
                ("query", query.trim().chars().take(200).collect()),
                ("facets", json!(facets).to_string()),
                ("index", index.into()),
                ("offset", offset.to_string()),
                ("limit", limit.clamp(1, 100).to_string()),
            ],
        ))
    }
    pub fn project_versions(
        &self,
        project: &str,
        loaders: &[String],
        game_version: Option<&str>,
    ) -> Result<Vec<Version>, String> {
        if !valid_id(project) {
            return Err("Invalid Modrinth project".into());
        }
        let mut query = vec![("include_changelog", "false".to_string())];
        if !loaders.is_empty() {
            query.push(("loaders", json!(loaders).to_string()));
        }
        if let Some(version) = game_version {
            query.push(("game_versions", json!([version]).to_string()));
        }
        self.get(&Self::url(&format!("/project/{project}/version"), &query))
    }
    pub fn version(&self, id: &str) -> Result<Version, String> {
        if !valid_id(id) {
            return Err("Invalid Modrinth version".into());
        }
        self.get(&Self::url(&format!("/version/{id}"), &[]))
    }
    /// Installed versions keyed by SHA-512 file hash.
    pub fn versions_by_hash(&self, hashes: &[String]) -> Result<HashMap<String, Version>, String> {
        let mut found = HashMap::new();
        for chunk in hashes.chunks(500) {
            let response: HashMap<String, Version> = self.post(
                &Self::url("/version_files", &[]),
                &json!({"hashes": chunk, "algorithm": "sha512"}),
            )?;
            found.extend(response);
        }
        Ok(found)
    }
    /// Newest compatible versions keyed by the SHA-512 hash of the installed file.
    pub fn latest_by_hash(
        &self,
        hashes: &[String],
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<HashMap<String, Version>, String> {
        let mut found = HashMap::new();
        for chunk in hashes.chunks(500) {
            let mut body = json!({"hashes": chunk, "algorithm": "sha512"});
            if !loaders.is_empty() {
                body["loaders"] = json!(loaders);
            }
            if !game_versions.is_empty() {
                body["game_versions"] = json!(game_versions);
            }
            let response: HashMap<String, Version> =
                self.post(&Self::url("/version_files/update", &[]), &body)?;
            found.extend(response);
        }
        Ok(found)
    }
    pub fn projects(&self, ids: &[String]) -> Result<Vec<Project>, String> {
        let ids: Vec<_> = ids.iter().filter(|id| valid_id(id)).collect();
        let mut projects = vec![];
        for chunk in ids.chunks(100) {
            let mut page: Vec<Project> = self.get(&Self::url(
                "/projects",
                &[("ids", json!(chunk).to_string())],
            ))?;
            projects.append(&mut page);
        }
        Ok(projects)
    }
    /// Streams a version file into `out`, verifying its declared size and SHA-512 hash.
    pub fn download(&self, file: &VersionFile, out: &mut dyn Write) -> Result<(), String> {
        if !trusted_download(&file.url) {
            return Err(format!("Refusing download from {}", file.url));
        }
        let expected = file
            .hashes
            .sha512
            .as_deref()
            .filter(|h| h.len() == 128 && h.chars().all(|c| c.is_ascii_hexdigit()))
            .ok_or("Modrinth did not provide a SHA-512 hash for this file")?
            .to_ascii_lowercase();
        if file.size > FILE_LIMIT {
            return Err(format!("{} is larger than 1 GiB", file.filename));
        }
        let mut writer = Verifying {
            inner: out,
            hash: Sha512::new(),
            size: 0,
        };
        // A zero declared size is unknown; the hash still pins the content. ureq rejects a body
        // that reaches its limit, so allow one byte more and check the exact size below.
        let limit = if file.size == 0 {
            FILE_LIMIT
        } else {
            file.size + 1
        };
        self.transport.download(&file.url, limit, &mut writer)?;
        if file.size != 0 && writer.size != file.size {
            return Err(format!(
                "{} was incomplete ({} of {} bytes)",
                file.filename, writer.size, file.size
            ));
        }
        if format!("{:x}", writer.hash.finalize()) != expected {
            return Err(format!(
                "{} failed verification and was discarded",
                file.filename
            ));
        }
        Ok(())
    }
}

struct Verifying<'a> {
    inner: &'a mut dyn Write,
    hash: Sha512,
    size: u64,
}
impl Write for Verifying<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.hash.update(&buf[..written]);
        self.size += written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

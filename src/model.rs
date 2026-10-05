use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtworkKind {
    #[default]
    Cover,
    Icon,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MinecraftArtwork {
    #[default]
    Updates,
    Modpacks,
}

/// Provider hints and resolved artwork presentation. Older manifests need only `artwork`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Artwork {
    pub kind: ArtworkKind,
    pub source: String,
    pub icon: String,
    pub remote: Vec<String>,
    pub minecraft_version: Option<String>,
    pub modrinth_project: Option<String>,
    pub gog_product: Option<String>,
    pub roblox_universe: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub subtitle: String,
    pub artwork: String,
    #[serde(default)]
    pub art: Artwork,
    #[serde(default)]
    pub launch_notice: String,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub launch_uri: Option<String>,
    #[serde(default)]
    pub directory: Option<PathBuf>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub last_played: u64,
    /// Provider-defined ranking, where 1 is first and 0 means unranked.
    #[serde(default)]
    pub source_rank: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SourceConfig {
    pub enabled: bool,
    pub paths: Vec<PathBuf>,
    pub command: Vec<String>,
}
impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            paths: vec![],
            command: vec![],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub density: String,
    pub view: String,
    pub sources: BTreeMap<String, SourceConfig>,
    pub favorites: Vec<String>,
    pub played: BTreeMap<String, u64>,
    pub custom_games: Vec<Game>,
    pub online_artwork: bool,
    pub minecraft_artwork: MinecraftArtwork,
    pub artwork_overrides: BTreeMap<String, String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            density: "comfortable".into(),
            view: "grid".into(),
            sources: [
                "steam",
                "lutris",
                "prism",
                "multimc",
                "polymc",
                "atlauncher",
                "modrinth",
                "heroic",
                "legendary",
                "nile",
                "roblox",
                "epic",
                "gog",
                "battlenet",
                "ubisoft",
                "itch",
                "desktop",
            ]
            .into_iter()
            .map(|id| (id.into(), SourceConfig::default()))
            .collect(),
            favorites: vec![],
            played: BTreeMap::new(),
            custom_games: vec![],
            online_artwork: true,
            minecraft_artwork: MinecraftArtwork::Updates,
            artwork_overrides: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    pub count: usize,
    pub enabled: bool,
    pub errors: Vec<String>,
    pub available: bool,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Library {
    pub games: Vec<Game>,
    pub providers: Vec<ProviderStatus>,
}

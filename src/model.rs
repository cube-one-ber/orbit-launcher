use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub subtitle: String,
    pub artwork: String,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub launch_uri: Option<String>,
    #[serde(default)]
    pub directory: Option<PathBuf>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub last_played: u64,
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
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            density: "comfortable".into(),
            view: "grid".into(),
            sources: ["steam", "lutris", "prism", "epic", "gog"]
                .into_iter()
                .map(|id| (id.into(), SourceConfig::default()))
                .collect(),
            favorites: vec![],
            played: BTreeMap::new(),
            custom_games: vec![],
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

use crate::model::Settings;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn config_dir() -> PathBuf {
    std::env::var_os("ORBIT_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                return crate::platform::Platform::current().roaming.join("Orbit");
            }
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join(".config"))
                .join("orbit")
        })
}
pub fn home() -> PathBuf {
    if cfg!(windows) {
        return crate::platform::Platform::current().home;
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
pub fn data_home() -> PathBuf {
    if cfg!(windows) {
        return crate::platform::Platform::current().roaming;
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"))
}
pub fn cache_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ORBIT_CACHE_DIR") {
        return dir.into();
    }
    // Configuration overrides also isolate downloads during portable/fixture runs.
    if let Some(dir) = std::env::var_os("ORBIT_CONFIG_DIR") {
        return PathBuf::from(dir).join("cache");
    }
    if cfg!(windows) {
        return crate::platform::Platform::current()
            .local
            .join("Orbit/cache");
    }
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".cache"))
        .join("orbit")
}
pub fn load(dir: &Path) -> Result<Settings, String> {
    let file = dir.join("settings.json");
    if !file.exists() {
        return Ok(Settings::default());
    }
    let bytes = fs::read(&file).map_err(|e| format!("Cannot read {}: {e}", file.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("Invalid settings in {}: {e}", file.display()))
}
pub fn save(dir: &Path, settings: &Settings) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    use std::io::Write;
    // Same-directory tempfile + atomic replace works on both Unix and Windows.
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    file.persist(dir.join("settings.json"))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        s.favorites.push("steam:42".into());
        save(dir.path(), &s).unwrap();
        assert_eq!(load(dir.path()).unwrap().favorites, s.favorites);
    }
    #[test]
    fn repeated_saves_replace_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        save(dir.path(), &settings).unwrap();
        settings.theme = crate::model::Theme::Light;
        settings.favorites.push("steam:7".into());
        save(dir.path(), &settings).unwrap();
        let restored = load(dir.path()).unwrap();
        assert_eq!(restored.theme, crate::model::Theme::Light);
        assert_eq!(restored.favorites, settings.favorites);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn legacy_settings_preserve_library_and_default_to_dark() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), r##"{"accent":"#9ae8c0","favorites":["steam:42"],"played":{"steam:42":123},"sources":{"steam":{"paths":[],"enabled":false,"command":[]}}}"##).unwrap();
        let mut settings = load(dir.path()).unwrap();
        assert_eq!(settings.theme, crate::model::Theme::Dark);
        assert_eq!(settings.favorites, ["steam:42"]);
        assert_eq!(settings.played["steam:42"], 123);
        assert!(!settings.sources["steam"].enabled);
        settings.theme = crate::model::Theme::Light;
        save(dir.path(), &settings).unwrap();
        assert_eq!(load(dir.path()).unwrap().theme, crate::model::Theme::Light);
    }
    #[test]
    fn malformed_settings_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), "bad json").unwrap();
        assert!(load(dir.path()).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("settings.json")).unwrap(),
            "bad json"
        );
    }
}

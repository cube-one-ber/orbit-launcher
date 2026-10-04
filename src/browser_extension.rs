//! Prepare the bundled extension without changing browser profiles or policies.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
const FILES: &[(&str, &str)] = &[
    (
        "report.js",
        include_str!("../browser-extension/roblox/report.js"),
    ),
    (
        "content.js",
        include_str!("../browser-extension/roblox/content.js"),
    ),
    (
        "background.js",
        include_str!("../browser-extension/roblox/background.js"),
    ),
    (
        "popup.html",
        include_str!("../browser-extension/roblox/popup.html"),
    ),
    (
        "popup.css",
        include_str!("../browser-extension/roblox/popup.css"),
    ),
    (
        "popup.js",
        include_str!("../browser-extension/roblox/popup.js"),
    ),
];
pub fn prepare(config_dir: &Path, browser: &str) -> Result<PathBuf, String> {
    let manifest = match browser {
        "chrome" | "chromium" => include_str!("../browser-extension/roblox/manifest.json"),
        "firefox" => include_str!("../browser-extension/roblox/manifest.firefox.json"),
        _ => return Err("Unsupported extension browser".into()),
    };
    let directory = config_dir
        .join("extensions/roblox")
        .join(if browser == "firefox" {
            "firefox"
        } else {
            "chromium"
        });
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    for (name, contents) in FILES.iter().copied().chain([("manifest.json", manifest)]) {
        let mut file = tempfile::NamedTempFile::new_in(&directory).map_err(|e| e.to_string())?;
        file.write_all(contents.as_bytes())
            .map_err(|e| e.to_string())?;
        file.persist(directory.join(name))
            .map_err(|e| e.to_string())?;
    }
    fs::canonicalize(directory).map_err(|e| e.to_string())
}
/// Open the browser's supported setup page. Browser approval remains necessary.
pub fn open_setup() -> Result<(), String> {
    let platform = crate::platform::Platform::current();
    let candidates: Vec<PathBuf> = if cfg!(windows) {
        platform
            .program_files
            .iter()
            .map(|p| p.join("Google/Chrome/Application/chrome.exe"))
            .chain([platform.local.join("Google/Chrome/Application/chrome.exe")])
            .collect()
    } else if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
        ]
    } else {
        vec![]
    };
    let program = candidates.into_iter().find(|p| p.is_file()).or_else(|| {
        std::env::var_os("PATH").and_then(|path| {
            let names = if cfg!(windows) { vec!["chrome.exe", "chromium.exe"] } else { vec!["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"] };
            std::env::split_paths(&path).flat_map(|p| names.iter().map(move |n| p.join(n))).find(|p| p.is_file())
        })
    }).ok_or("Extension files are ready. Open chrome://extensions in Chrome or Chromium, enable Developer mode, then choose Load unpacked and select the displayed folder.")?;
    let mut child = Command::new(program)
        .arg("chrome://extensions/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_is_bundled_repeatable_and_preserves_other_files() {
        let temp = tempfile::tempdir().unwrap();
        let dir = prepare(temp.path(), "chrome").unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["manifest_version"], 3);
        assert_eq!(manifest["background"]["service_worker"], "background.js");
        assert!(
            !manifest["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p == "cookies")
        );
        for (name, contents) in FILES {
            assert_eq!(fs::read_to_string(dir.join(name)).unwrap(), *contents);
        }
        fs::write(dir.join("keep.txt"), "keep").unwrap();
        assert_eq!(prepare(temp.path(), "chromium").unwrap(), dir);
        assert_eq!(fs::read_to_string(dir.join("keep.txt")).unwrap(), "keep");
        let firefox = prepare(temp.path(), "firefox").unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(firefox.join("manifest.json")).unwrap()).unwrap();
        assert!(manifest["background"]["scripts"].is_array());
        assert!(prepare(temp.path(), "unknown").is_err());
    }
}

//! Prism Launcher and PolyMC content metadata (`<folder>/.index/<slug>.pw.toml`, packwiz format).
//!
//! Prism uses these files to show where content came from and to update it. Orbit writes the
//! same fields as Prism's `Packwiz::V1::updateModIndex` for Modrinth downloads and removes
//! entries whose file it removes.
use super::api::{Project, Version, VersionFile};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const ENTRY_LIMIT: u64 = 256 * 1024;
const KNOWN_LOADERS: [&str; 11] = [
    "neoforge",
    "forge",
    "cauldron",
    "liteloader",
    "quilt",
    "fabric",
    "babric",
    "bta-babric",
    "legacy-fabric",
    "ornithe",
    "rift",
];

fn index_dir(folder: &Path) -> PathBuf {
    folder.join(".index")
}

/// Records a Modrinth download so Prism shows its source and can update it.
pub fn write(
    folder: &Path,
    project: &Project,
    version: &Version,
    file: &VersionFile,
) -> Result<(), String> {
    let slug = if super::safe_stem(&project.slug) {
        &project.slug
    } else if super::safe_stem(&project.id) {
        &project.id
    } else {
        return Err("Invalid Modrinth project identity for Prism metadata".into());
    };
    let hash = file
        .hashes
        .sha512
        .as_deref()
        .ok_or("Missing SHA-512 hash")?;
    let side = match (
        matches!(project.client_side.as_str(), "required" | "optional"),
        matches!(project.server_side.as_str(), "required" | "optional"),
    ) {
        (true, true) => "both",
        (false, true) => "server",
        (true, false) => "client",
        (false, false) => "",
    };
    let loaders: Vec<_> = KNOWN_LOADERS
        .iter()
        .filter(|l| version.loaders.iter().any(|v| v == *l))
        .map(|l| string(l))
        .collect();
    let versions: Vec<_> = version.game_versions.iter().map(|v| string(v)).collect();
    let dependencies: Vec<_> = version
        .dependencies
        .iter()
        .filter_map(|d| {
            let project = d.project_id.as_deref()?;
            let mut entry = format!(
                "{{ addonId = {}, type = {}",
                string(project),
                string(&d.dependency_type.to_uppercase())
            );
            if let Some(version) = &d.version_id {
                entry.push_str(&format!(", version = {}", string(version)));
            }
            entry.push_str(" }");
            Some(entry)
        })
        .collect();
    let release = match version.version_type.as_str() {
        "release" => "Release",
        "beta" => "Beta",
        "alpha" => "Alpha",
        _ => "Unknown",
    };
    let text = format!(
        "filename = {}\nname = {}\nside = {}\nx-prismlauncher-dependencies = [{}]\nx-prismlauncher-loaders = [{}]\nx-prismlauncher-lock-update = false\nx-prismlauncher-mc-versions = [{}]\nx-prismlauncher-release-type = {}\nx-prismlauncher-version-number = {}\n\n[download]\nhash = {}\nhash-format = \"sha512\"\nmode = \"url\"\nurl = {}\n\n[update.modrinth]\nmod-id = {}\nversion = {}\n",
        string(&file.filename),
        string(&project.title),
        string(side),
        dependencies.join(", "),
        loaders.join(", "),
        versions.join(", "),
        string(release),
        string(&version.version_number),
        string(hash),
        string(&file.url),
        string(&project.id),
        string(&version.id),
    );
    let dir = index_dir(folder);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // Prism matches index names case-insensitively. Replace the selected path atomically.
    let name = format!("{slug}.pw.toml");
    let target = entries(&dir)
        .into_iter()
        .find(|existing| {
            existing
                .file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&name))
        })
        .unwrap_or_else(|| dir.join(name));
    let mut temp = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut temp, text.as_bytes()).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(target).map_err(|e| e.to_string())?;
    Ok(())
}

/// Removes metadata describing `file_name` (without any `.disabled` suffix).
pub fn remove_for_file(folder: &Path, file_name: &str) {
    for entry in entries(&index_dir(folder)) {
        if read_filename(&entry).as_deref() == Some(file_name) {
            let _ = fs::remove_file(entry);
        }
    }
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.to_string_lossy().ends_with(".pw.toml"))
        .collect()
}

/// Reads the top-level `filename` string without a general TOML parser.
fn read_filename(path: &Path) -> Option<String> {
    let mut text = String::new();
    fs::File::open(path)
        .ok()?
        .take(ENTRY_LIMIT)
        .read_to_string(&mut text)
        .ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim().trim_matches('"') != "filename" {
            continue;
        }
        let value = value.trim();
        if let Some(literal) = value.strip_prefix('\'') {
            return literal.split('\'').next().map(str::to_owned);
        }
        let mut chars = value.strip_prefix('"')?.chars();
        let mut out = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(out),
                '\\' => match chars.next()? {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'b' => out.push('\x08'),
                    'f' => out.push('\x0c'),
                    escape @ ('u' | 'U') => {
                        let width = if escape == 'u' { 4 } else { 8 };
                        let digits: String = chars.by_ref().take(width).collect();
                        out.push(
                            u32::from_str_radix(&digits, 16)
                                .ok()
                                .and_then(char::from_u32)?,
                        );
                    }
                    other => out.push(other),
                },
                c => out.push(c),
            }
        }
        return None;
    }
    None
}

/// A TOML basic string.
fn string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filenames_are_read_from_basic_and_literal_strings() {
        let dir = tempfile::tempdir().unwrap();
        let basic = dir.path().join("a.pw.toml");
        fs::write(
            &basic,
            "name = \"A\"\nfilename = \"a \\\"b\\\" \\u00E9.jar\"\n[download]\nfilename = \"no\"\n",
        )
        .unwrap();
        assert_eq!(read_filename(&basic).as_deref(), Some("a \"b\" é.jar"));
        let literal = dir.path().join("b.pw.toml");
        fs::write(&literal, "filename = 'C:\\b.jar'\n").unwrap();
        assert_eq!(read_filename(&literal).as_deref(), Some("C:\\b.jar"));
        assert_eq!(string("a\"\\\u{1}"), "\"a\\\"\\\\\\u0001\"");
    }
}

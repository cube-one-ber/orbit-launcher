//! Format-preserving edits to Prism Launcher, MultiMC and PolyMC `instance.cfg` files.
//!
//! Current Prism builds write Qt `QSettings` INI files with a `[General]` section and a
//! `ConfigVersion` key. MultiMC, PolyMC and older Prism builds write plain `key=value` lines with
//! their own escaping. Orbit replaces only the keys it edits, encodes them the way the owning
//! launcher reads them, and keeps every other line byte for byte.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io::Read, io::Write, path::Path};

const FILE_LIMIT: u64 = 1024 * 1024;
pub const DEFAULT_MIN_MEMORY: u32 = 512;
pub const DEFAULT_MAX_MEMORY: u32 = 4096;

/// Instance settings that Orbit can edit; all other launcher settings stay untouched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceSettings {
    pub name: String,
    pub notes: String,
    pub override_memory: bool,
    /// Minimum and maximum Java heap sizes in MiB.
    pub min_memory: u32,
    pub max_memory: u32,
    pub override_java_args: bool,
    pub java_args: String,
}
impl Default for InstanceSettings {
    fn default() -> Self {
        Self {
            name: String::new(),
            notes: String::new(),
            override_memory: false,
            min_memory: DEFAULT_MIN_MEMORY,
            max_memory: DEFAULT_MAX_MEMORY,
            override_java_args: false,
            java_args: String::new(),
        }
    }
}
impl InstanceSettings {
    pub fn validate(&self) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("The instance needs a name.".into());
        }
        if name.chars().count() > 128 || name.chars().any(char::is_control) {
            return Err("Use a single-line name of up to 128 characters.".into());
        }
        if self.notes.len() > 64 * 1024 {
            return Err("Notes are limited to 64 KiB.".into());
        }
        if self.override_memory
            && (self.min_memory < 128
                || self.max_memory > 1024 * 1024
                || self.min_memory > self.max_memory)
        {
            return Err(
                "Memory must be at least 128 MiB, and the minimum cannot exceed the maximum."
                    .into(),
            );
        }
        if self.java_args.len() > 8 * 1024 || self.java_args.contains(['\n', '\r']) {
            return Err("Java arguments must be one line of up to 8 KiB.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    /// Qt `QSettings` INI, used by Prism once `ConfigVersion` exists.
    Qt,
    /// MultiMC-style `key=value` lines.
    Legacy,
}

struct Document {
    lines: Vec<String>,
    newline: &'static str,
    trailing_newline: bool,
    format: Format,
}
impl Document {
    fn parse(text: &str) -> Self {
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let trailing_newline = text.is_empty() || text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        let lines = if text.is_empty() {
            vec![]
        } else {
            body.split('\n')
                .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
                .collect()
        };
        let mut document = Self {
            lines,
            newline,
            trailing_newline,
            format: Format::Legacy,
        };
        document.format = Format::Qt;
        if document.find("ConfigVersion").is_none() {
            document.format = Format::Legacy;
        }
        document
    }
    fn render(&self) -> String {
        let mut text = self.lines.join(self.newline);
        if self.trailing_newline && !self.lines.is_empty() {
            text.push_str(self.newline);
        }
        text
    }
    /// The line and raw value of a top-level (Qt `[General]`) key.
    fn find(&self, key: &str) -> Option<(usize, String)> {
        // Both launchers keep the last occurrence of a repeated key.
        self.entries()
            .into_iter()
            .rfind(|(_, name, _)| name == key)
            .map(|(index, _, raw)| (index, raw))
    }
    fn entries(&self) -> Vec<(usize, String, String)> {
        let mut general = true;
        let mut found = vec![];
        for (index, line) in self.lines.iter().enumerate() {
            let trimmed = line.trim();
            if self.format == Format::Qt {
                if trimmed.starts_with('[') {
                    general = trimmed == "[General]";
                    continue;
                }
                if !general || trimmed.starts_with([';', '#']) {
                    continue;
                }
            }
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            found.push((index, name.trim().to_string(), value.to_string()));
        }
        found
    }
    fn decode(&self, raw: &str) -> String {
        match self.format {
            Format::Qt => qt_decode(raw),
            Format::Legacy => legacy_decode(raw),
        }
    }
    fn get(&self, key: &str) -> Option<String> {
        let (_, raw) = self.find(key)?;
        Some(self.decode(&raw))
    }
    fn set(&mut self, key: &str, value: &str) {
        let encoded = match self.format {
            Format::Qt => qt_encode(value),
            Format::Legacy => legacy_encode(value),
        };
        let line = format!("{key}={encoded}");
        if let Some((index, _)) = self.find(key) {
            self.lines[index] = line;
            return;
        }
        if self.format == Format::Legacy {
            self.lines.push(line);
            return;
        }
        // Add new keys to the end of the general section, creating it if necessary.
        let header = self.lines.iter().position(|l| l.trim() == "[General]");
        let start = header.map_or(0, |h| h + 1);
        let end = self.lines[start..]
            .iter()
            .position(|l| l.trim().starts_with('['))
            .map_or(self.lines.len(), |p| start + p);
        if header.is_none() && end != 0 && end == self.lines.len() {
            // Keys before any section belong to General in QSettings.
            self.lines.push(line);
        } else if header.is_none() {
            self.lines.splice(0..0, ["[General]".to_string(), line]);
        } else {
            let mut insert = end;
            while insert > start && self.lines[insert - 1].trim().is_empty() {
                insert -= 1;
            }
            self.lines.insert(insert, line);
        }
    }
}

/// Reads the editable settings of a Prism-family instance folder.
pub fn read_settings(instance_root: &Path) -> Result<InstanceSettings, String> {
    let document = load(&instance_root.join("instance.cfg"))?;
    let defaults = InstanceSettings::default();
    let flag = |key| document.get(key).is_some_and(|v| v.trim() == "true");
    let number = |key, default| {
        document
            .get(key)
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(default)
    };
    Ok(InstanceSettings {
        name: document.get("name").unwrap_or_else(|| {
            instance_root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into()
        }),
        notes: document.get("notes").unwrap_or_default(),
        override_memory: flag("OverrideMemory"),
        min_memory: number("MinMemAlloc", defaults.min_memory),
        max_memory: number("MaxMemAlloc", defaults.max_memory),
        override_java_args: flag("OverrideJavaArgs"),
        java_args: document.get("JvmArgs").unwrap_or_default(),
    })
}

/// Writes changed settings, leaving overridden values untouched when their override is off.
pub fn write_settings(instance_root: &Path, settings: &InstanceSettings) -> Result<(), String> {
    settings.validate()?;
    let path = instance_root.join("instance.cfg");
    let mut document = load(&path)?;
    let notes = settings.notes.replace("\r\n", "\n").replace('\r', "\n");
    let mut values = vec![
        ("name", settings.name.trim().to_string()),
        ("notes", notes),
        ("OverrideMemory", settings.override_memory.to_string()),
        ("OverrideJavaArgs", settings.override_java_args.to_string()),
    ];
    if settings.override_memory {
        values.push(("MinMemAlloc", settings.min_memory.to_string()));
        values.push(("MaxMemAlloc", settings.max_memory.to_string()));
    }
    if settings.override_java_args {
        values.push(("JvmArgs", settings.java_args.trim().to_string()));
    }
    let mut changed = false;
    for (key, value) in values {
        let unchanged = match document.get(key) {
            Some(current) => current == value,
            // Missing booleans default to false and missing notes to empty.
            None => value.is_empty() || value == "false",
        };
        if !unchanged {
            document.set(key, &value);
            changed = true;
        }
    }
    if !changed {
        return Ok(());
    }
    let parent = path.parent().ok_or("Instance has no folder")?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(document.render().as_bytes())
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    if let Ok(metadata) = fs::metadata(&path) {
        let _ = fs::set_permissions(file.path(), metadata.permissions());
    }
    file.persist(&path)
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    Ok(())
}

fn load(path: &Path) -> Result<Document, String> {
    let mut bytes = vec![];
    fs::File::open(path)
        .and_then(|f| f.take(FILE_LIMIT + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() as u64 > FILE_LIMIT {
        return Err(format!(
            "{}: Instance metadata exceeds 1 MiB",
            path.display()
        ));
    }
    let text = String::from_utf8(bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Document::parse(&text))
}

/// Decodes a Qt INI value. Unquoted commas make a Qt string list, shown joined with ", ".
pub(crate) fn qt_decode(raw: &str) -> String {
    let mut items = vec![];
    let mut current = String::new();
    // Length of `current` up to its last quoted or escaped character, which is never trimmed.
    let mut kept = 0;
    let mut quoted = false;
    let mut chars = raw.trim().chars().peekable();
    // Qt drops unquoted whitespace around each list item.
    let finish = |current: &mut String, kept: &mut usize| {
        let item = format!("{}{}", &current[..*kept], current[*kept..].trim_end());
        current.clear();
        *kept = 0;
        item
    };
    while let Some(c) = chars.next() {
        if quoted && c != '"' && c != '\\' {
            current.push(c);
            kept = current.len();
            continue;
        }
        match c {
            '"' => quoted = !quoted,
            ',' => items.push(finish(&mut current, &mut kept)),
            c if c.is_whitespace() && current.is_empty() => (),
            '\\' => match chars.next() {
                Some('a') => current.push('\x07'),
                Some('b') => current.push('\x08'),
                Some('f') => current.push('\x0c'),
                Some('n') => current.push('\n'),
                Some('r') => current.push('\r'),
                Some('t') => current.push('\t'),
                Some('v') => current.push('\x0b'),
                Some('x') => {
                    let mut value = 0u32;
                    while let Some(digit) = chars.peek().and_then(|d| d.to_digit(16)) {
                        value = value.saturating_mul(16).saturating_add(digit);
                        chars.next();
                    }
                    current.extend(char::from_u32(value));
                }
                Some(d @ '0'..='7') => {
                    let mut value = d.to_digit(8).unwrap_or(0);
                    while let Some(digit) = chars.peek().and_then(|d| d.to_digit(8)) {
                        value = value * 8 + digit;
                        chars.next();
                    }
                    current.extend(char::from_u32(value));
                }
                Some(other) => current.push(other),
                None => (),
            },
            other => current.push(other),
        }
        if c == '\\' {
            kept = current.len();
        }
    }
    items.push(finish(&mut current, &mut kept));
    let mut value = items.join(", ");
    if value.starts_with("@@") {
        value.remove(0);
    }
    value
}

/// Mirrors `QSettingsPrivate::iniEscapedString` for string values.
pub(crate) fn qt_encode(value: &str) -> String {
    let value = if value.starts_with('@') {
        format!("@{value}")
    } else {
        value.to_string()
    };
    let mut out = String::with_capacity(value.len());
    let mut needs_quotes = false;
    let mut escape_hex = false;
    for c in value.chars() {
        if matches!(c, ';' | ',' | '=') {
            needs_quotes = true;
        }
        if escape_hex && c.is_ascii_hexdigit() {
            out.push_str(&format!("\\x{:x}", c as u32));
            continue;
        }
        escape_hex = false;
        match c {
            '\0' => {
                out.push_str("\\0");
                escape_hex = true;
            }
            '\x07' => out.push_str("\\a"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x0b' => out.push_str("\\v"),
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) <= 0x1f => {
                out.push_str(&format!("\\x{:x}", c as u32));
                escape_hex = true;
            }
            c => out.push(c),
        }
    }
    if needs_quotes || out.starts_with(' ') || out.ends_with(' ') {
        format!("\"{out}\"")
    } else {
        out
    }
}

/// Mirrors MultiMC's and Prism's legacy INI parser, including escaped `#` comments.
pub(crate) fn legacy_decode(raw: &str) -> String {
    let mut value = raw;
    let bytes = raw.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'#' && (index == 0 || bytes[index - 1] != b'\\') {
            value = &raw[..index];
            break;
        }
    }
    let mut out = String::new();
    let mut chars = value.trim().chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => (),
        }
    }
    if out.contains([';', '=', ',']) && out.len() >= 2 && out.starts_with('"') && out.ends_with('"')
    {
        out = out[1..out.len() - 1].to_string();
    }
    out
}
pub(crate) fn legacy_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\\' => out.push_str("\\\\"),
            '#' => out.push_str("\\#"),
            '\r' => (),
            c => out.push(c),
        }
    }
    out
}

/// Reads one top-level value from instance or launcher settings, decoding either format.
pub fn read_value(path: &Path, key: &str) -> Result<Option<String>, String> {
    Ok(load(path)?.get(key))
}
/// Decoded top-level (Qt `[General]`) values of instance or launcher settings.
pub fn read_values(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let document = load(path)?;
    Ok(document
        .entries()
        .into_iter()
        .map(|(_, key, raw)| (key, document.decode(&raw)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qt_values_round_trip() {
        for value in [
            "Plain name",
            "Create, Above and Beyond",
            "key=value; more",
            " padded ",
            "quote \" and \\ slash",
            "line one\nline two\ttab",
            "@at sign",
            "Zoë ✓ 日本",
            "\u{1}1",
        ] {
            assert_eq!(qt_decode(&qt_encode(value)), value, "{value:?}");
        }
        assert_eq!(qt_encode("a,b"), "\"a,b\"");
        assert_eq!(qt_decode("a, b"), "a, b");
        assert_eq!(qt_decode("\"a\", \"b\""), "a, b");
    }
    #[test]
    fn legacy_values_round_trip() {
        for value in [
            "Plain",
            "Has # hash",
            "C:\\Games\\Minecraft",
            "two\nlines",
            "a=b",
        ] {
            assert_eq!(legacy_decode(&legacy_encode(value)), value, "{value:?}");
        }
        assert_eq!(legacy_decode("value # comment"), "value");
    }
    #[test]
    fn qt_keys_are_added_to_the_general_section() {
        let mut document =
            Document::parse("[General]\nConfigVersion=1.3\nname=Old\n\n[Other]\nname=keep\n");
        document.set("name", "New, name");
        document.set("notes", "hello");
        assert_eq!(
            document.render(),
            "[General]\nConfigVersion=1.3\nname=\"New, name\"\nnotes=hello\n\n[Other]\nname=keep\n"
        );
    }
}

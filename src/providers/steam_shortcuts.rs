//! Read Steam's binary KeyValues without executing or rewriting shortcuts.
use super::*;
use std::collections::BTreeMap;

#[derive(Debug)]
enum Value {
    Object(BTreeMap<String, Value>),
    Text(String),
    Number(u32),
    Other,
}
impl Value {
    fn object(&self) -> Option<&BTreeMap<String, Value>> {
        if let Self::Object(value) = self {
            Some(value)
        } else {
            None
        }
    }
    fn text(&self) -> Option<&str> {
        if let Self::Text(value) = self {
            Some(value)
        } else {
            None
        }
    }
    fn number(&self) -> Option<u32> {
        if let Self::Number(value) = self {
            Some(*value)
        } else {
            None
        }
    }
}
struct Reader<'a> {
    remaining: &'a [u8],
}
impl Reader<'_> {
    fn take(&mut self, size: usize) -> Result<&[u8], String> {
        if size > self.remaining.len() {
            return Err("Truncated binary KeyValues".into());
        }
        let (value, rest) = self.remaining.split_at(size);
        self.remaining = rest;
        Ok(value)
    }
    fn string(&mut self) -> Result<String, String> {
        let length = self
            .remaining
            .iter()
            .position(|b| *b == 0)
            .ok_or("Unterminated binary KeyValues string")?;
        let value = String::from_utf8_lossy(self.take(length)?).into_owned();
        self.take(1)?;
        Ok(value)
    }
    fn object(&mut self, depth: usize) -> Result<BTreeMap<String, Value>, String> {
        if depth > 32 {
            return Err("Binary KeyValues nesting exceeds 32 levels".into());
        }
        let mut values = BTreeMap::new();
        loop {
            // Valve KeyValues also permits EOF to close the top-level object.
            // Nested objects still require their end marker.
            if depth == 0 && self.remaining.is_empty() {
                return Ok(values);
            }
            let kind = self.take(1)?[0];
            if kind == 8 {
                return Ok(values);
            }
            let key = self.string()?.to_ascii_lowercase();
            let value = match kind {
                0 => Value::Object(self.object(depth + 1)?),
                1 => Value::Text(self.string()?),
                2 => Value::Number(u32::from_le_bytes(self.take(4)?.try_into().unwrap())),
                3 | 4 | 6 => {
                    self.take(4)?;
                    Value::Other
                }
                7 | 10 => {
                    self.take(8)?;
                    Value::Other
                }
                5 => {
                    let mut units = vec![];
                    loop {
                        let unit = u16::from_le_bytes(self.take(2)?.try_into().unwrap());
                        if unit == 0 {
                            break;
                        }
                        units.push(unit);
                    }
                    Value::Text(String::from_utf16_lossy(&units))
                }
                _ => return Err(format!("Unknown binary KeyValues type: {kind}")),
            };
            values.insert(key, value);
        }
    }
}
pub(super) struct Shortcut {
    pub app_id: u32,
    pub title: String,
    pub icon: String,
    pub last_played: u64,
}
pub(super) fn read(path: &Path) -> Result<Vec<Shortcut>, String> {
    let bytes = super::binary_metadata::read(path)?;
    let mut reader = Reader { remaining: &bytes };
    if bytes.is_empty() {
        return Ok(vec![]);
    }
    let values = reader.object(0)?;
    if !reader.remaining.is_empty() {
        return Err("Trailing binary KeyValues data".into());
    }
    let entries = values
        .get("shortcuts")
        .and_then(Value::object)
        .ok_or("Missing shortcuts object")?;
    Ok(entries
        .values()
        .filter_map(|entry| {
            let entry = entry.object()?;
            let app_id = entry.get("appid")?.number()?;
            let title = entry.get("appname")?.text()?.trim();
            let exe = entry.get("exe")?.text()?.trim();
            // Shortcut IDs have their high bit set; their signed int32 storage
            // must be preserved as an unsigned ID when constructing rungameid.
            if app_id & 0x8000_0000 == 0
                || title.is_empty()
                || exe.is_empty()
                || entry.get("ishidden").and_then(Value::number) == Some(1)
            {
                return None;
            }
            Some(Shortcut {
                app_id,
                title: title.into(),
                icon: entry
                    .get("icon")
                    .and_then(Value::text)
                    .unwrap_or("")
                    .trim_matches('"')
                    .into(),
                last_played: u64::from(
                    entry
                        .get("lastplaytime")
                        .and_then(Value::number)
                        .unwrap_or(0),
                ),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_truncation_unknown_types_and_deep_objects() {
        for bytes in [
            &b"\0shortcuts\0\0game\0"[..],
            &b"\x0cunknown\0"[..],
            &b"\x01key\0value"[..],
        ] {
            assert!(Reader { remaining: bytes }.object(0).is_err());
        }
        let mut bytes = b"\0key\0".repeat(34);
        bytes.extend(vec![8; 35]);
        assert!(Reader { remaining: &bytes }.object(0).is_err());
    }
}

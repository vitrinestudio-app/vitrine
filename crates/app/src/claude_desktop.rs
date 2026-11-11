//! "Add to Claude Desktop": the `mcpServers.vitrine-studio` entry in Claude
//! Desktop's `claude_desktop_config.json` (ADR 0015).
//!
//! | platform | file |
//! |---|---|
//! | macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
//! | Windows | `%APPDATA%\Claude\claude_desktop_config.json` |
//! | Linux | `~/.config/Claude/claude_desktop_config.json` |
//!
//! The entry starts the CLI's stdio proxy: `{"command": "<abs path to
//! vitrine>", "args": ["mcp"]}`. [`plan`] shows what would be written;
//! [`install`] writes it only when the caller confirms (the GUI asks the
//! user), merging into the existing JSON in its key order, after a backup
//! `claude_desktop_config.json.bak-<UTC timestamp>`. Tests and the dev
//! bridge point [`CONFIG_ENV`] (or `AppState::with_claude_desktop_config`)
//! at a temp file, so the real config is never touched.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{CmdResult, CommandError};

/// Overrides the path of Claude Desktop's config file (tests, dev).
pub const CONFIG_ENV: &str = "VITRINE_CLAUDE_DESKTOP_CONFIG";
/// Overrides the path of the `vitrine` CLI binary written into configs.
pub const CLI_ENV: &str = "VITRINE_CLI";
/// Name of the server entry.
pub const SERVER_KEY: &str = "vitrine-studio";

/// Claude Desktop's config file on this platform (`None` without a home
/// folder). [`CONFIG_ENV`] wins.
pub fn default_config_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(CONFIG_ENV).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    // `config_dir`: macOS ~/Library/Application Support, Windows
    // %APPDATA% (roaming), Linux $XDG_CONFIG_HOME or ~/.config.
    let base = directories::BaseDirs::new()?;
    Some(
        base.config_dir()
            .join("Claude")
            .join("claude_desktop_config.json"),
    )
}

/// The `vitrine` CLI binary: [`CLI_ENV`], else `vitrine` next to the
/// running executable (dev builds: `target/<profile>/vitrine` next to
/// `vitrine-gui`).
///
/// TODO(Phase 6): bundled builds ship the CLI inside the app bundle
/// (macOS `Vitrine Studio.app/Contents/MacOS/vitrine`, Windows/Linux next to
/// the GUI binary); check the path once bundling exists.
pub fn cli_path() -> PathBuf {
    if let Some(p) = std::env::var_os(CLI_ENV).filter(|v| !v.is_empty()) {
        return PathBuf::from(p);
    }
    let name = format!("vitrine{}", std::env::consts::EXE_SUFFIX);
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join(&name)))
        .unwrap_or_else(|| PathBuf::from(name))
}

/// The server entry for `cli`.
pub fn entry_for(cli: &Path) -> serde_json::Value {
    serde_json::json!({ "command": cli.to_string_lossy(), "args": ["mcp"] })
}

/// State of the entry in the config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    /// No `mcpServers.vitrine-studio` (or no file).
    Missing,
    /// Present and identical.
    Same,
    /// Present with other values (e.g. the CLI moved).
    Different,
    /// The file is not valid JSON (or not an object): nothing is written.
    Invalid,
}

/// What "Add to Claude Desktop" would do.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub config_path: PathBuf,
    pub config_exists: bool,
    pub state: EntryState,
    /// The current `mcpServers.vitrine-studio` entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<serde_json::Value>,
    /// The entry that will be written.
    pub entry: serde_json::Value,
    /// The whole file after the change (`None` when invalid).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub cli_path: PathBuf,
    pub cli_exists: bool,
    /// Why the file cannot be changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of [`install`].
#[derive(Debug, Clone, Serialize)]
pub struct Installed {
    /// `false`: the entry was already there.
    pub written: bool,
    pub config_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<PathBuf>,
}

/// Plans adding the entry for `cli` to `config`.
pub fn plan(config: &Path, cli: &Path) -> Plan {
    let entry = entry_for(cli);
    let mut out = Plan {
        config_path: config.to_owned(),
        config_exists: config.is_file(),
        state: EntryState::Missing,
        current: None,
        entry: entry.clone(),
        preview: None,
        cli_path: cli.to_owned(),
        cli_exists: cli.is_file(),
        error: None,
    };
    let text = match std::fs::read_to_string(config) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            out.state = EntryState::Invalid;
            out.error = Some(format!("cannot read {}: {e}", config.display()));
            return out;
        }
    };
    match merge(&text, &entry) {
        Ok((state, current, preview)) => {
            out.state = state;
            out.current = current;
            out.preview = Some(preview);
        }
        Err(e) => {
            out.state = EntryState::Invalid;
            out.error = Some(e);
        }
    }
    out
}

/// Writes the entry for `cli` into `config` (backup first). Only with
/// `confirmed` (the user saw [`plan`] and agreed).
pub fn install(config: &Path, cli: &Path, confirmed: bool) -> CmdResult<Installed> {
    if !confirmed {
        return Err(CommandError::invalid(
            "writing Claude Desktop's config needs the user's confirmation",
        ));
    }
    let p = plan(config, cli);
    let preview = match (p.state, p.preview) {
        (EntryState::Invalid, _) | (_, None) => {
            return Err(CommandError::invalid(p.error.unwrap_or_else(|| {
                format!("{} cannot be changed", config.display())
            })));
        }
        (EntryState::Same, _) => {
            return Ok(Installed {
                written: false,
                config_path: config.to_owned(),
                backup: None,
            });
        }
        (_, Some(text)) => text,
    };
    let backup = if p.config_exists {
        let name = format!(
            "{}.bak-{}",
            config
                .file_name()
                .map_or_else(|| "config".into(), |n| n.to_string_lossy().into_owned()),
            utc_stamp(std::time::SystemTime::now())
        );
        let backup = config.with_file_name(name);
        std::fs::copy(config, &backup)
            .map_err(|e| CommandError::io(format!("backup {}: {e}", backup.display())))?;
        Some(backup)
    } else {
        None
    };
    if let Some(dir) = config.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| CommandError::io(format!("{}: {e}", dir.display())))?;
    }
    vitrine_core::io::write_atomic(config, preview.as_bytes())?;
    Ok(Installed {
        written: true,
        config_path: config.to_owned(),
        backup,
    })
}

/// Merges `entry` as `mcpServers.vitrine-studio` into the JSON `text` (empty =
/// a new file). Returns the previous state, the current entry and the new
/// text (2-space indent, key order kept).
fn merge(
    text: &str,
    entry: &serde_json::Value,
) -> Result<(EntryState, Option<serde_json::Value>, String), String> {
    let mut root = if text.trim().is_empty() {
        Ordered::Object(Vec::new())
    } else {
        serde_json::from_str::<Ordered>(text).map_err(|e| format!("not valid JSON: {e}"))?
    };
    let Ordered::Object(top) = &mut root else {
        return Err("the top level is not a JSON object".to_owned());
    };
    let i = top
        .iter()
        .position(|(k, _)| k == "mcpServers")
        .unwrap_or_else(|| {
            top.push(("mcpServers".to_owned(), Ordered::Object(Vec::new())));
            top.len() - 1
        });
    let servers = &mut top[i].1;
    let Ordered::Object(servers) = servers else {
        return Err("`mcpServers` is not a JSON object".to_owned());
    };
    let new = Ordered::from(entry.clone());
    let (state, current) = if let Some((_, v)) = servers.iter_mut().find(|(k, _)| k == SERVER_KEY) {
        let current = serde_json::to_value(&*v).ok();
        let state = if current.as_ref() == Some(entry) {
            EntryState::Same
        } else {
            EntryState::Different
        };
        *v = new;
        (state, current)
    } else {
        servers.push((SERVER_KEY.to_owned(), new));
        (EntryState::Missing, None)
    };
    let mut out = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    out.push('\n');
    Ok((state, current, out))
}

/// `YYYYMMDD-HHMMSS` (UTC).
fn utc_stamp(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// A JSON value that keeps the key order of objects (the workspace's
/// `serde_json` sorts keys).
#[derive(Debug, Clone, PartialEq)]
enum Ordered {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Ordered>),
    Object(Vec<(String, Ordered)>),
}

impl From<serde_json::Value> for Ordered {
    fn from(v: serde_json::Value) -> Self {
        use serde_json::Value as V;
        match v {
            V::Null => Self::Null,
            V::Bool(b) => Self::Bool(b),
            V::Number(n) => Self::Number(n),
            V::String(s) => Self::String(s),
            V::Array(a) => Self::Array(a.into_iter().map(Self::from).collect()),
            // `entry_for` writes `command` before `args`; keep that order.
            V::Object(m) => {
                let mut items: Vec<(String, Ordered)> =
                    m.into_iter().map(|(k, v)| (k, Self::from(v))).collect();
                items.sort_by_key(|(k, _)| u8::from(k != "command"));
                Self::Object(items)
            }
        }
    }
}

impl Serialize for Ordered {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => s.serialize_unit(),
            Self::Bool(b) => s.serialize_bool(*b),
            Self::Number(n) => n.serialize(s),
            Self::String(v) => s.serialize_str(v),
            Self::Array(a) => {
                let mut seq = s.serialize_seq(Some(a.len()))?;
                for v in a {
                    seq.serialize_element(v)?;
                }
                seq.end()
            }
            Self::Object(m) => {
                let mut map = s.serialize_map(Some(m.len()))?;
                for (k, v) in m {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Ordered, E> {
                Ok(Ordered::Null)
            }
            fn visit_none<E: de::Error>(self) -> Result<Ordered, E> {
                Ok(Ordered::Null)
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Ordered, E> {
                Ok(Ordered::Bool(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Ordered, E> {
                Ok(Ordered::Number(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Ordered, E> {
                Ok(Ordered::Number(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Ordered, E> {
                serde_json::Number::from_f64(v)
                    .map(Ordered::Number)
                    .ok_or_else(|| E::custom("not a JSON number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Ordered, E> {
                Ok(Ordered::String(v.to_owned()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Ordered, E> {
                Ok(Ordered::String(v))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = a.next_element()? {
                    out.push(v);
                }
                Ok(Ordered::Array(out))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Ordered, A::Error> {
                let mut out: Vec<(String, Ordered)> = Vec::new();
                while let Some((k, v)) = a.next_entry::<String, Ordered>()? {
                    // Duplicate keys: the last one wins, like serde_json.
                    out.retain(|(o, _)| *o != k);
                    out.push((k, v));
                }
                Ok(Ordered::Object(out))
            }
        }
        d.deserialize_any(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXISTING: &str = r#"{
  "globalShortcut": "Alt+Space",
  "mcpServers": {
    "zeta": { "command": "/usr/bin/zeta", "args": ["serve"], "env": {"B": "1", "A": "2"} },
    "alpha": { "command": "npx", "args": ["-y", "alpha"] }
  },
  "afterwards": [1, 2.5, true, null]
}"#;

    #[test]
    fn merge_keeps_entries_and_order_and_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("Claude").join("claude_desktop_config.json");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, EXISTING).unwrap();
        let cli = tmp.path().join("bin").join("vitrine");

        // No write without confirmation.
        assert!(install(&config, &cli, false).is_err());
        assert_eq!(std::fs::read_to_string(&config).unwrap(), EXISTING);

        let p = plan(&config, &cli);
        assert_eq!(p.state, EntryState::Missing);
        assert!(!p.cli_exists);
        let done = install(&config, &cli, true).unwrap();
        assert!(done.written);
        let backup = done.backup.unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), EXISTING);
        assert!(
            backup
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("claude_desktop_config.json.bak-20")
        );

        let text = std::fs::read_to_string(&config).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["globalShortcut"], "Alt+Space");
        assert_eq!(v["mcpServers"]["zeta"]["env"]["B"], "1");
        assert_eq!(v["mcpServers"]["alpha"]["args"][1], "alpha");
        assert_eq!(v["afterwards"][1], 2.5);
        assert_eq!(v["mcpServers"]["vitrine-studio"], entry_for(&cli));
        // Key order of the original file is kept, the new entry is last.
        let pos = |s: &str| text.find(s).unwrap();
        assert!(pos("globalShortcut") < pos("mcpServers") && pos("mcpServers") < pos("afterwards"));
        assert!(pos("\"zeta\"") < pos("\"alpha\"") && pos("\"alpha\"") < pos("\"vitrine-studio\""));
        assert!(pos("\"B\"") < pos("\"A\""));
        assert!(
            pos("\"command\": \"/") < pos("\"args\": [\n        \"mcp\""),
            "{text}"
        );

        // Again: nothing to do, no further backup.
        assert_eq!(plan(&config, &cli).state, EntryState::Same);
        let again = install(&config, &cli, true).unwrap();
        assert!(!again.written && again.backup.is_none());

        // The CLI moved: detected as different and replaced in place.
        let moved = tmp.path().join("other").join("vitrine");
        let p = plan(&config, &moved);
        assert_eq!(p.state, EntryState::Different);
        assert_eq!(p.current.unwrap(), entry_for(&cli));
        install(&config, &moved, true).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["vitrine-studio"], entry_for(&moved));
        assert_eq!(v["mcpServers"].as_object().unwrap().len(), 3);
    }

    #[test]
    fn new_file_and_broken_files() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("new").join("claude_desktop_config.json");
        let cli = Path::new("/opt/vitrine/vitrine");
        let done = install(&config, cli, true).unwrap();
        assert!(done.written && done.backup.is_none());
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"mcpServers": {"vitrine-studio": entry_for(cli)}})
        );

        for broken in ["{ nope", "[1, 2]", r#"{"mcpServers": 3}"#] {
            std::fs::write(&config, broken).unwrap();
            let p = plan(&config, cli);
            assert_eq!(p.state, EntryState::Invalid, "{broken}");
            assert!(p.error.is_some());
            assert!(install(&config, cli, true).is_err());
            assert_eq!(std::fs::read_to_string(&config).unwrap(), broken);
        }
    }

    #[test]
    fn utc_stamps() {
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_762_790_405);
        assert_eq!(utc_stamp(t), "20251110-160005");
        assert_eq!(utc_stamp(std::time::UNIX_EPOCH), "19700101-000000");
    }
}

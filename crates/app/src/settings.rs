//! App-wide settings (KONZEPT §9.2, §10.3): a YAML file in the platform
//! config folder, and the recent-projects list in SQLite in the platform
//! data folder (`rusqlite`, bundled).
//!
//! | file | macOS | Linux | Windows |
//! |---|---|---|---|
//! | `settings.yaml` | `~/Library/Application Support/app.vitrinestudio/` | `~/.config/vitrinestudio/` | `%APPDATA%\vitrinestudio\config\` |
//! | `app.sqlite` | same folder | `~/.local/share/vitrinestudio/` | `%APPDATA%\vitrinestudio\data\` |
//! | `frames/` (imported frames, ADR 0012) | same folder | data folder | data folder |
//!
//! `VITRINE_APP_DIR` moves everything below one folder (`config/`, `data/`).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use vitrine_core::io::{from_yaml, to_yaml, write_atomic};
use vitrine_core::model::{FrameStyle, Lang};

use crate::error::{CmdResult, CommandError};
use crate::stores::StoreAccount;

/// Where app data lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// `settings.yaml`.
    pub config_dir: PathBuf,
    /// `app.sqlite` (recent projects).
    pub data_dir: PathBuf,
}

impl AppPaths {
    /// The platform folders (`directories`), or below `$VITRINE_APP_DIR`
    /// when set (like the CLI); `None` without a home folder.
    pub fn platform() -> Option<Self> {
        if let Some(dir) =
            std::env::var_os(vitrine_core::frame_import::APP_DIR_ENV).filter(|v| !v.is_empty())
        {
            return Some(Self::in_dir(Path::new(&dir)));
        }
        let dirs = directories::ProjectDirs::from("app", "", "vitrinestudio")?;
        Some(Self {
            config_dir: dirs.config_dir().to_owned(),
            data_dir: dirs.data_dir().to_owned(),
        })
    }

    /// Both below one folder (tests, dev bridge).
    pub fn in_dir(dir: &Path) -> Self {
        Self {
            config_dir: dir.join("config"),
            data_dir: dir.join("data"),
        }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.yaml")
    }

    pub fn database(&self) -> PathBuf {
        self.data_dir.join("app.sqlite")
    }
}

/// Last window position and size (logical px).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub maximized: bool,
}

/// App settings (`settings.yaml`). Unknown fields are rejected so typos
/// are noticed; missing fields take their defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)] // independent switches
pub struct Settings {
    /// UI language (`en`, `de`); `None` follows the system.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_language: Option<String>,
    /// Default language of new projects.
    pub default_project_language: String,
    /// Frame style of new projects (`stylized`, `detailed`, `imported:<pack>`).
    pub default_frame_style: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowState>,
    /// Use system fonts as a last resort for characters no bundled or
    /// project font has (preview and export then depend on this machine's
    /// fonts). Off by default; never used by tests, CLI or CI.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub system_font_fallback: bool,
    /// The GUI serves MCP on 127.0.0.1 so AI assistants can connect (ADR 0015).
    /// On by default.
    #[serde(skip_serializing_if = "is_true")]
    pub mcp_enabled: bool,
    /// Preferred port of the GUI's MCP endpoint (`None` or taken: a free
    /// port).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_port: Option<u16>,
    /// `vitrine mcp` starts the GUI when none is running (otherwise it
    /// works headless). On by default.
    #[serde(skip_serializing_if = "is_true")]
    pub mcp_autostart_gui: bool,
    /// Store accounts (Phase 5.1): public facts only, secrets are in the
    /// system keychain (`crate::stores`). Changed only by the
    /// `store_account_*` commands, never by `settings_set`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub store_accounts: Vec<StoreAccount>,
    /// AI assistants may upload through `vitrine mcp --headless` (no
    /// GUI to confirm in). Off by default (ADR 0018).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub mcp_allow_headless_upload: bool,
    /// The GUI's quick guide ("How Vitrine Studio is organised") was shown
    /// once on opening a project; it is not shown automatically again
    /// (per viewer, not per project). Still reachable from the Help menu.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub guide_seen: bool,
    /// The GUI checks GitHub Releases for a new version once a day and
    /// asks before installing (ADR 0026). On by default; no telemetry
    /// (one anonymous GET of `latest.json`).
    #[serde(skip_serializing_if = "is_true")]
    pub update_check: bool,
    /// Unix time (s) of the last update check (manual or automatic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_checked_at: Option<i64>,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if signature
fn is_true(v: &bool) -> bool {
    *v
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ui_language: None,
            default_project_language: "en".to_owned(),
            default_frame_style: "detailed".to_owned(),
            window: None,
            system_font_fallback: false,
            mcp_enabled: true,
            mcp_port: None,
            mcp_autostart_gui: true,
            store_accounts: Vec::new(),
            mcp_allow_headless_upload: false,
            guide_seen: false,
            update_check: true,
            update_checked_at: None,
        }
    }
}

/// UI languages with a catalog in `ui/i18n/`.
pub const UI_LANGUAGES: [&str; 2] = ["en", "de"];

impl Settings {
    pub fn check(&self) -> Result<(), String> {
        if let Some(l) = &self.ui_language
            && !UI_LANGUAGES.contains(&l.as_str())
        {
            return Err(format!(
                "unsupported UI language `{l}` (supported: {})",
                UI_LANGUAGES.join(", ")
            ));
        }
        Lang::parse(&self.default_project_language)?;
        FrameStyle::parse(&self.default_frame_style)?;
        crate::stores::check_accounts(&self.store_accounts)?;
        Ok(())
    }
}

/// Loads and saves `settings.yaml`.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Current settings; defaults if the file is missing. A broken file is
    /// an error (it is not overwritten silently).
    pub fn load(&self) -> CmdResult<Settings> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
            Err(e) => return Err(CommandError::io(format!("{}: {e}", self.path.display()))),
        };
        if text.trim().is_empty() {
            return Ok(Settings::default());
        }
        from_yaml(&text).map_err(|e| CommandError::io(format!("{}: {e}", self.path.display())))
    }

    pub fn save(&self, settings: &Settings) -> CmdResult<()> {
        settings.check().map_err(CommandError::invalid)?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| CommandError::io(format!("{}: {e}", dir.display())))?;
        }
        let yaml = to_yaml(settings).map_err(|e| CommandError::io(e.to_string()))?;
        write_atomic(&self.path, yaml.as_bytes())?;
        Ok(())
    }

    /// Merges a partial JSON object into the current settings, checks and
    /// saves the result (`null` clears an optional field).
    pub fn update(&self, patch: &serde_json::Value) -> CmdResult<Settings> {
        let serde_json::Value::Object(patch) = patch else {
            return Err(CommandError::invalid("settings must be an object"));
        };
        if patch.contains_key("store_accounts") {
            return Err(CommandError::invalid(
                "store accounts are changed with store_account_import / store_account_remove",
            ));
        }
        let mut current = serde_json::to_value(self.load()?)?;
        if let serde_json::Value::Object(map) = &mut current {
            for (k, v) in patch {
                if v.is_null() {
                    map.remove(k);
                } else {
                    map.insert(k.clone(), v.clone());
                }
            }
        }
        let settings: Settings = serde_json::from_value(current)?;
        self.save(&settings)?;
        Ok(settings)
    }
}

/// A recently opened project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentProject {
    pub path: PathBuf,
    pub name: String,
    /// Unix time (s) of the last opening.
    pub opened_at: i64,
    /// The folder still has a `project.yaml`.
    pub exists: bool,
}

/// Recent projects in `app.sqlite`.
#[derive(Debug, Clone)]
pub struct RecentStore {
    path: PathBuf,
}

/// Number of recent projects kept.
pub const RECENT_LIMIT: usize = 20;

impl RecentStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn open(&self) -> CmdResult<rusqlite::Connection> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| CommandError::io(format!("{}: {e}", dir.display())))?;
        }
        let db = rusqlite::Connection::open(&self.path)?;
        db.busy_timeout(std::time::Duration::from_secs(2))?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS recent_projects (
                path TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                opened_at INTEGER NOT NULL
            );",
        )?;
        Ok(db)
    }

    /// Records that `path` was opened now.
    pub fn touch(&self, path: &Path, name: &str) -> CmdResult<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let db = self.open()?;
        let path = path.to_string_lossy();
        // Same second: keep the most recent call on top.
        let latest: Option<i64> = db
            .query_row("SELECT MAX(opened_at) FROM recent_projects", [], |r| {
                r.get(0)
            })
            .unwrap_or(None);
        let at = latest.map_or(now, |l| now.max(l + i64::from(l >= now)));
        db.execute(
            "INSERT INTO recent_projects (path, name, opened_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET name = excluded.name, opened_at = excluded.opened_at",
            rusqlite::params![path, name, at],
        )?;
        db.execute(
            "DELETE FROM recent_projects WHERE path NOT IN
             (SELECT path FROM recent_projects ORDER BY opened_at DESC LIMIT ?1)",
            [i64::try_from(RECENT_LIMIT).unwrap_or(i64::MAX)],
        )?;
        Ok(())
    }

    /// Most recent first.
    pub fn list(&self) -> CmdResult<Vec<RecentProject>> {
        let db = self.open()?;
        let mut stmt = db
            .prepare("SELECT path, name, opened_at FROM recent_projects ORDER BY opened_at DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (path, name, opened_at) = row?;
            let path = PathBuf::from(path);
            out.push(RecentProject {
                exists: path.join(vitrine_core::io::PROJECT_FILE).is_file(),
                path,
                name,
                opened_at,
            });
        }
        Ok(out)
    }

    pub fn remove(&self, path: &Path) -> CmdResult<()> {
        self.open()?.execute(
            "DELETE FROM recent_projects WHERE path = ?1",
            [path.to_string_lossy()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_partial_update() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(tmp.path());
        let store = SettingsStore::new(paths.settings_file());
        assert_eq!(store.load().unwrap(), Settings::default());
        let s = Settings {
            ui_language: Some("de".into()),
            default_project_language: "de-AT".into(),
            default_frame_style: "detailed".into(),
            window: Some(WindowState {
                x: 10,
                y: 20,
                width: 1280,
                height: 800,
                maximized: false,
            }),
            system_font_fallback: true,
            mcp_enabled: false,
            mcp_port: Some(4100),
            mcp_autostart_gui: true,
            store_accounts: Vec::new(),
            mcp_allow_headless_upload: false,
            guide_seen: true,
            update_check: false,
            update_checked_at: Some(1_762_790_405),
        };
        store.save(&s).unwrap();
        assert_eq!(store.load().unwrap(), s);
        let yaml = std::fs::read_to_string(paths.settings_file()).unwrap();
        assert!(yaml.contains("ui_language: de"), "{yaml}");
        assert!(yaml.contains("mcp_enabled: false"), "{yaml}");
        assert!(yaml.contains("guide_seen: true"), "{yaml}");
        assert!(yaml.contains("update_check: false"), "{yaml}");
        assert!(
            !yaml.contains("mcp_autostart_gui"),
            "defaults stay out: {yaml}"
        );
        assert!(Settings::default().mcp_enabled && Settings::default().mcp_autostart_gui);
        assert!(Settings::default().update_check);

        let updated = store
            .update(&serde_json::json!({"ui_language": null, "default_frame_style": "stylized"}))
            .unwrap();
        assert_eq!(updated.ui_language, None);
        assert_eq!(updated.default_frame_style, "stylized");
        assert_eq!(updated.window, s.window);
        assert_eq!(store.load().unwrap(), updated);

        assert!(
            store
                .update(&serde_json::json!({"ui_language": "xx"}))
                .is_err()
        );
        assert!(store.update(&serde_json::json!({"nope": 1})).is_err());
        assert!(
            store
                .update(&serde_json::json!({"store_accounts": []}))
                .is_err()
        );
        assert_eq!(store.load().unwrap(), updated);
    }

    #[test]
    fn recent_projects_most_recent_first() {
        let tmp = tempfile::tempdir().unwrap();
        let store = RecentStore::new(AppPaths::in_dir(tmp.path()).database());
        assert_eq!(store.list().unwrap(), Vec::new());
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(a.join("project.yaml"), "x").unwrap();
        store.touch(&a, "A").unwrap();
        store.touch(&b, "B").unwrap();
        store.touch(&a, "A2").unwrap();
        let list = store.list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "A2");
        assert!(list[0].exists);
        assert!(!list[1].exists);
        store.remove(&b).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
    }
}

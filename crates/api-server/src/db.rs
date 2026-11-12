//! SQLite: template metadata, abuse reports, login sessions and pending
//! OAuth states (`<data>/api.sqlite`).
//!
//! Schema versions (`PRAGMA user_version`), applied by [`init`] and
//! idempotent (columns are only added when missing):
//! - 0: the first deployment (tags as a free JSON list).
//! - 1 (2026-11-12): `devices` (derived device tags), `samples` (number of
//!   sample screenshots), `sprites` (gallery sprite metadata per canvas);
//!   `tags` becomes the facet map `{"style", "color", "use"}` (old lists
//!   are sorted into facets, unknown words dropped). Devices and sample
//!   counts of existing rows are filled from the stored packages by
//!   `crate::backfill`.
//! - 2 (2026-11-12): each canvas object in `canvases` gets `devices` (the
//!   derived device tags of that canvas alone; the website shows the
//!   canvas of the device a visitor filters by). Existing rows are marked
//!   for the backfill again.

use std::fmt::Write as _;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};
use vitrine_core::template::TemplateTags;

/// Current schema version.
pub const SCHEMA_VERSION: i64 = 2;

/// Moderation state of a template.
pub const PENDING: &str = "pending";
pub const APPROVED: &str = "approved";
pub const REJECTED: &str = "rejected";

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    init(&db)?;
    Ok(db)
}

pub fn init(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS templates (
             id TEXT PRIMARY KEY,
             name TEXT NOT NULL,
             description TEXT NOT NULL DEFAULT '',
             author TEXT NOT NULL DEFAULT '',
             tags TEXT NOT NULL DEFAULT '[]',
             version TEXT NOT NULL,
             uploader TEXT NOT NULL,
             uploader_id INTEGER,
             status TEXT NOT NULL,
             canvases TEXT NOT NULL DEFAULT '[]',
             fonts TEXT NOT NULL DEFAULT '[]',
             size INTEGER NOT NULL,
             downloads INTEGER NOT NULL DEFAULT 0,
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL,
             moderation_note TEXT NOT NULL DEFAULT ''
         );
         CREATE TABLE IF NOT EXISTS reports (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             template_id TEXT NOT NULL,
             reason TEXT NOT NULL,
             message TEXT NOT NULL DEFAULT '',
             created_at INTEGER NOT NULL,
             resolved INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS sessions (
             token_hash TEXT PRIMARY KEY,
             login TEXT NOT NULL,
             github_id INTEGER NOT NULL,
             created_at INTEGER NOT NULL,
             expires_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS sessions_expiry ON sessions (expires_at);
         CREATE TABLE IF NOT EXISTS oauth_states (
             state_hash TEXT PRIMARY KEY,
             verifier TEXT NOT NULL,
             expires_at INTEGER NOT NULL
         );",
    )?;
    migrate(db)
}

/// Does `table` have `column`?
fn has_column(db: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut stmt = db.prepare(&format!("PRAGMA table_info({table})"))?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(names.iter().any(|n| n == column))
}

/// Brings an older database to [`SCHEMA_VERSION`] (safe to run again).
fn migrate(db: &Connection) -> rusqlite::Result<()> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    for (column, ddl) in [
        ("devices", "devices TEXT NOT NULL DEFAULT '[]'"),
        ("samples", "samples INTEGER NOT NULL DEFAULT 0"),
        ("sprites", "sprites TEXT NOT NULL DEFAULT '[]'"),
        ("backfilled", "backfilled INTEGER NOT NULL DEFAULT 0"),
    ] {
        if !has_column(db, "templates", column)? {
            db.execute_batch(&format!("ALTER TABLE templates ADD COLUMN {ddl}"))?;
        }
    }
    // Free tag lists → facet maps.
    let rows: Vec<(String, String)> = {
        let mut stmt = db.prepare("SELECT id, tags FROM templates")?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    for (id, tags) in rows {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(&tags) {
            let map = tags_json(&TemplateTags::from_flat(&list));
            db.execute(
                "UPDATE templates SET tags = ?2 WHERE id = ?1",
                params![id, map.to_string()],
            )?;
        }
    }
    if version < 2 {
        // Per-canvas devices come from the stored packages.
        db.execute_batch("UPDATE templates SET backfilled = 0")?;
    }
    db.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
}

/// The tags as the API shows them: all three facets, always present.
pub fn tags_json(t: &TemplateTags) -> Value {
    json!({ "style": t.style, "color": t.color, "use": t.use_ })
}

/// A template row as the API shows it.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TemplateRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub tags: Value,
    pub version: String,
    pub license: &'static str,
    pub uploader: String,
    /// Numeric GitHub user id of the uploader (absent for static tokens
    /// configured without one). Not shown in responses.
    #[serde(skip)]
    pub uploader_id: Option<i64>,
    pub status: String,
    pub canvases: Value,
    pub fonts: Value,
    pub size: i64,
    pub downloads: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// Derived device tags (`["phone", "banner"]`).
    pub devices: Value,
    /// Number of sample screenshots in the package (0 = placeholders only).
    pub samples: i64,
    /// Sprite metadata per canvas ([`crate::sprites::SpriteMeta`]); shown
    /// inside the canvases by the routes.
    #[serde(skip)]
    pub sprites: Value,
}

const COLUMNS: &str = "id, name, description, author, tags, version, uploader, status, canvases, fonts, size, downloads, created_at, updated_at, uploader_id, devices, samples, sprites";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TemplateRow> {
    let json = |i: usize| -> rusqlite::Result<Value> {
        let s: String = r.get(i)?;
        Ok(serde_json::from_str(&s).unwrap_or(Value::Null))
    };
    Ok(TemplateRow {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        author: r.get(3)?,
        tags: json(4)?,
        version: r.get(5)?,
        license: vitrine_core::template::LICENSE,
        uploader: r.get(6)?,
        status: r.get(7)?,
        canvases: json(8)?,
        fonts: json(9)?,
        size: r.get(10)?,
        downloads: r.get(11)?,
        created_at: r.get(12)?,
        updated_at: r.get(13)?,
        uploader_id: r.get(14)?,
        devices: json(15)?,
        samples: r.get(16)?,
        sprites: json(17)?,
    })
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

pub fn get(db: &Connection, id: &str) -> rusqlite::Result<Option<TemplateRow>> {
    db.query_row(
        &format!("SELECT {COLUMNS} FROM templates WHERE id = ?1"),
        [id],
        row,
    )
    .optional()
}

/// What [`search`] filters by.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// Substring of name, description or tags.
    pub q: Option<String>,
    /// Any tag value in any facet (`?tag=`, kept for old clients).
    pub tag: Option<String>,
    /// Per facet (`style`, `color`, `use`, `devices`) the wanted values: a
    /// template matches when it has one of them in every given facet.
    pub facets: Vec<(String, Vec<String>)>,
}

/// Templates with `status` matching `filter`, newest first.
pub fn search(
    db: &Connection,
    status: &str,
    filter: &Filter,
    limit: u32,
    offset: u32,
) -> rusqlite::Result<Vec<TemplateRow>> {
    let like = |s: &str| {
        let escaped = s
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        format!("%{escaped}%")
    };
    let mut sql = format!("SELECT {COLUMNS} FROM templates WHERE status = ?1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(status.to_owned())];
    let mut arg = |sql: &mut String, v: Box<dyn rusqlite::ToSql>| {
        args.push(v);
        let _ = write!(sql, "?{}", args.len());
    };
    if let Some(q) = filter.q.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND (name LIKE ");
        arg(&mut sql, Box::new(like(q)));
        sql.push_str(" ESCAPE '\\' OR description LIKE ");
        arg(&mut sql, Box::new(like(q)));
        sql.push_str(" ESCAPE '\\' OR tags LIKE ");
        arg(&mut sql, Box::new(like(q)));
        sql.push_str(" ESCAPE '\\')");
    }
    if let Some(t) = filter.tag.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND (tags LIKE ");
        arg(&mut sql, Box::new(like(&format!("\"{t}\""))));
        sql.push_str(" ESCAPE '\\' OR devices LIKE ");
        arg(&mut sql, Box::new(like(&format!("\"{t}\""))));
        sql.push_str(" ESCAPE '\\')");
    }
    for (facet, values) in &filter.facets {
        if values.is_empty() {
            continue;
        }
        // json_each over the facet's list; the facet names are checked by
        // the route (a fixed set), values are bound.
        let source = if facet == "devices" {
            "json_each(templates.devices)".to_owned()
        } else {
            format!("json_each(templates.tags, '$.\"{facet}\"')")
        };
        let _ = write!(sql, " AND EXISTS (SELECT 1 FROM {source} WHERE value IN (");
        for (i, v) in values.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            arg(&mut sql, Box::new(v.clone()));
        }
        sql.push_str("))");
    }
    sql.push_str(" ORDER BY updated_at DESC, id LIMIT ");
    arg(&mut sql, Box::new(limit));
    sql.push_str(" OFFSET ");
    arg(&mut sql, Box::new(offset));
    let mut stmt = db.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(AsRef::as_ref).collect();
    stmt.query_map(refs.as_slice(), row)?.collect()
}

/// Every template row (any status), for maintenance jobs.
pub fn all(db: &Connection) -> rusqlite::Result<Vec<TemplateRow>> {
    let mut stmt = db.prepare(&format!("SELECT {COLUMNS} FROM templates ORDER BY id"))?;
    stmt.query_map([], row)?.collect()
}

/// Stores the sprite metadata of a template.
pub fn set_sprites(db: &Connection, id: &str, sprites: &Value) -> rusqlite::Result<()> {
    db.execute(
        "UPDATE templates SET sprites = ?2 WHERE id = ?1",
        params![id, sprites.to_string()],
    )?;
    Ok(())
}

/// Ids of rows whose derived fields were not filled from the package yet
/// (rows from before schema version 2).
pub fn not_backfilled(db: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = db.prepare("SELECT id FROM templates WHERE backfilled = 0 ORDER BY id")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

/// Fields derived from a stored package (see [`set_derived`]).
#[derive(Debug)]
pub struct Derived {
    pub devices: Value,
    pub samples: i64,
    pub tags: Value,
    /// The manifest's canvases with `devices` per canvas.
    pub canvases: Value,
}

/// Stores the fields derived from the package (devices, sample count,
/// tags, per-canvas devices) and marks the row as backfilled.
pub fn set_derived(db: &Connection, id: &str, d: &Derived) -> rusqlite::Result<()> {
    db.execute(
        "UPDATE templates SET devices = ?2, samples = ?3, tags = ?4, canvases = ?5, backfilled = 1 WHERE id = ?1",
        params![
            id,
            d.devices.to_string(),
            d.samples,
            d.tags.to_string(),
            d.canvases.to_string()
        ],
    )?;
    Ok(())
}

/// Inserts or replaces a template row (a new version resets moderation;
/// the uploader login follows a renamed GitHub account).
pub fn upsert(db: &Connection, t: &TemplateRow) -> rusqlite::Result<()> {
    db.execute(
        "INSERT INTO templates (id, name, description, author, tags, version, uploader, status, canvases, fonts, size, downloads, created_at, updated_at, uploader_id, devices, samples, sprites, backfilled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12, ?12, ?13, ?14, ?15, ?16, 1)
         ON CONFLICT(id) DO UPDATE SET name = ?2, description = ?3, author = ?4, tags = ?5,
             version = ?6, uploader = ?7, status = ?8, canvases = ?9, fonts = ?10, size = ?11,
             updated_at = ?12, uploader_id = COALESCE(?13, uploader_id), devices = ?14,
             samples = ?15, sprites = ?16, backfilled = 1",
        params![
            t.id,
            t.name,
            t.description,
            t.author,
            t.tags.to_string(),
            t.version,
            t.uploader,
            t.status,
            t.canvases.to_string(),
            t.fonts.to_string(),
            t.size,
            t.updated_at,
            t.uploader_id,
            t.devices.to_string(),
            t.samples,
            t.sprites.to_string(),
        ],
    )?;
    Ok(())
}

pub fn count_download(db: &Connection, id: &str) -> rusqlite::Result<()> {
    db.execute(
        "UPDATE templates SET downloads = downloads + 1 WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn set_status(db: &Connection, id: &str, status: &str, note: &str) -> rusqlite::Result<bool> {
    let n = db.execute(
        "UPDATE templates SET status = ?2, moderation_note = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, status, note, now()],
    )?;
    Ok(n > 0)
}

/// An abuse report.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub id: i64,
    pub template_id: String,
    pub reason: String,
    pub message: String,
    pub created_at: i64,
}

pub fn add_report(
    db: &Connection,
    template: &str,
    reason: &str,
    message: &str,
) -> rusqlite::Result<()> {
    db.execute(
        "INSERT INTO reports (template_id, reason, message, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![template, reason, message, now()],
    )?;
    Ok(())
}

pub fn open_reports(db: &Connection) -> rusqlite::Result<Vec<Report>> {
    let mut stmt = db.prepare(
        "SELECT id, template_id, reason, message, created_at FROM reports WHERE resolved = 0 ORDER BY created_at",
    )?;
    stmt.query_map([], |r| {
        Ok(Report {
            id: r.get(0)?,
            template_id: r.get(1)?,
            reason: r.get(2)?,
            message: r.get(3)?,
            created_at: r.get(4)?,
        })
    })?
    .collect()
}

pub fn resolve_reports(db: &Connection, template: &str) -> rusqlite::Result<()> {
    db.execute(
        "UPDATE reports SET resolved = 1 WHERE template_id = ?1",
        [template],
    )?;
    Ok(())
}

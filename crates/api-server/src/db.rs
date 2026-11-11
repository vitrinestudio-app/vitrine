//! SQLite: template metadata, abuse reports, login sessions and pending
//! OAuth states (`<data>/api.sqlite`).

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

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
    )
}

/// A template row as the API shows it.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TemplateRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub tags: serde_json::Value,
    pub version: String,
    pub license: &'static str,
    pub uploader: String,
    /// Numeric GitHub user id of the uploader (absent for static tokens
    /// configured without one). Not shown in responses.
    #[serde(skip)]
    pub uploader_id: Option<i64>,
    pub status: String,
    pub canvases: serde_json::Value,
    pub fonts: serde_json::Value,
    pub size: i64,
    pub downloads: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

const COLUMNS: &str = "id, name, description, author, tags, version, uploader, status, canvases, fonts, size, downloads, created_at, updated_at, uploader_id";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TemplateRow> {
    let json = |i: usize| -> rusqlite::Result<serde_json::Value> {
        let s: String = r.get(i)?;
        Ok(serde_json::from_str(&s).unwrap_or(serde_json::Value::Null))
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

/// Approved templates matching `q` (name, description, tags) and `tag`,
/// newest first.
pub fn search(
    db: &Connection,
    status: &str,
    q: Option<&str>,
    tag: Option<&str>,
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
    let q = q.map_or_else(|| "%".to_owned(), like);
    let tag = tag.map_or_else(|| "%".to_owned(), |t| like(&format!("\"{t}\"")));
    let mut stmt = db.prepare(&format!(
        "SELECT {COLUMNS} FROM templates
         WHERE status = ?1
           AND (name LIKE ?2 ESCAPE '\\' OR description LIKE ?2 ESCAPE '\\' OR tags LIKE ?2 ESCAPE '\\')
           AND tags LIKE ?3 ESCAPE '\\'
         ORDER BY updated_at DESC, id LIMIT ?4 OFFSET ?5"
    ))?;
    stmt.query_map(params![status, q, tag, limit, offset], row)?
        .collect()
}

/// Inserts or replaces a template row (a new version resets moderation;
/// the uploader login follows a renamed GitHub account).
pub fn upsert(db: &Connection, t: &TemplateRow) -> rusqlite::Result<()> {
    db.execute(
        "INSERT INTO templates (id, name, description, author, tags, version, uploader, status, canvases, fonts, size, downloads, created_at, updated_at, uploader_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12, ?12, ?13)
         ON CONFLICT(id) DO UPDATE SET name = ?2, description = ?3, author = ?4, tags = ?5,
             version = ?6, uploader = ?7, status = ?8, canvases = ?9, fonts = ?10, size = ?11,
             updated_at = ?12, uploader_id = COALESCE(?13, uploader_id)",
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

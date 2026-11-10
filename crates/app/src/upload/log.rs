//! The upload log: `<project>/.cache/uploads.sqlite` (rusqlite, bundled).
//!
//! | table | rows |
//! |---|---|
//! | `plans` | one per plan: confirm token, fingerprint, expiry, used flag, plan JSON |
//! | `runs` | one per execution: token, times, status, report JSON |
//! | `sets` | one per set and run: store, app, locale, slot, target, lang, image hashes, status, error |
//! | `files` | one per image upload: set, position, file name, SHA-256, bytes, store id, status, error |
//! | `events` | redacted HTTP events and notes per run (method + path + status; never tokens, keys, signed URLs) |
//!
//! A set with status `done` is what the next plan compares against
//! ([`UploadLog::last_success`]): same store, app, locale, slot and the
//! same image hashes in the same order → `unchanged`.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::error::{CmdResult, CommandError};

/// File name below `<project>/.cache/`.
pub const LOG_FILE: &str = "uploads.sqlite";

#[allow(clippy::needless_pass_by_value)] // used as `map_err(db_error)`
fn db_error(e: rusqlite::Error) -> CommandError {
    CommandError::io(format!("upload log: {e}"))
}

fn i(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// Path of the log of the project in `dir`.
pub fn log_path(dir: &Path) -> PathBuf {
    dir.join(".cache").join(LOG_FILE)
}

/// A stored plan.
#[derive(Debug, Clone)]
pub struct StoredPlan {
    pub fingerprint: String,
    pub expires_at: u64,
    pub used: bool,
    pub plan_json: String,
}

/// One run with its sets (for `upload_log`).
#[derive(Debug, Clone, Serialize)]
pub struct RunRecord {
    pub id: i64,
    pub started_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    pub status: String,
    pub sets: Vec<SetRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<EventRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetRecord {
    pub store: String,
    pub app: String,
    pub locale: String,
    pub slot: String,
    pub target: String,
    pub lang: String,
    pub images: usize,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub files: Vec<FileRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRecord {
    pub index: u32,
    pub file_name: String,
    pub sha256: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_id: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRecord {
    pub at: u64,
    pub level: String,
    pub message: String,
}

/// Open connection to a project's upload log.
#[derive(Debug)]
pub struct UploadLog {
    db: Connection,
}

impl UploadLog {
    /// Opens (and creates) the log of the project in `dir`.
    pub fn open(dir: &Path) -> CmdResult<Self> {
        let path = log_path(dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CommandError::io(format!("{}: {e}", parent.display())))?;
        }
        let db = Connection::open(&path).map_err(db_error)?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(db_error)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS plans (
                token TEXT PRIMARY KEY,
                fingerprint TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                used INTEGER NOT NULL DEFAULT 0,
                plan_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                token TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                finished_at INTEGER,
                status TEXT NOT NULL,
                report_json TEXT
            );
            CREATE TABLE IF NOT EXISTS sets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id INTEGER NOT NULL,
                store TEXT NOT NULL,
                app TEXT NOT NULL,
                locale TEXT NOT NULL,
                slot TEXT NOT NULL,
                target TEXT NOT NULL,
                lang TEXT NOT NULL,
                hashes TEXT NOT NULL,
                status TEXT NOT NULL,
                error TEXT,
                started_at INTEGER NOT NULL,
                finished_at INTEGER
            );
            CREATE INDEX IF NOT EXISTS sets_key ON sets (store, app, locale, slot, status);
            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                set_id INTEGER NOT NULL,
                idx INTEGER NOT NULL,
                file_name TEXT NOT NULL,
                sha256 TEXT NOT NULL,
                bytes INTEGER NOT NULL,
                store_id TEXT,
                status TEXT NOT NULL,
                error TEXT
            );
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id INTEGER NOT NULL,
                at INTEGER NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL
            );",
        )
        .map_err(db_error)?;
        Ok(Self { db })
    }

    // ---- plans ---------------------------------------------------------------------

    pub fn save_plan(
        &self,
        token: &str,
        fingerprint: &str,
        created_at: u64,
        expires_at: u64,
        plan_json: &str,
    ) -> CmdResult<()> {
        // Old plans are of no use; keep the table small.
        self.db
            .execute(
                "DELETE FROM plans WHERE expires_at < ?1",
                [i(created_at.saturating_sub(24 * 3600))],
            )
            .map_err(db_error)?;
        self.db
            .execute(
                "INSERT OR REPLACE INTO plans (token, fingerprint, created_at, expires_at, used, plan_json)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5)",
                params![token, fingerprint, i(created_at), i(expires_at), plan_json],
            )
            .map_err(db_error)?;
        Ok(())
    }

    pub fn plan(&self, token: &str) -> CmdResult<Option<StoredPlan>> {
        self.db
            .query_row(
                "SELECT fingerprint, expires_at, used, plan_json FROM plans WHERE token = ?1",
                [token],
                |r| {
                    Ok(StoredPlan {
                        fingerprint: r.get(0)?,
                        expires_at: u64::try_from(r.get::<_, i64>(1)?).unwrap_or(0),
                        used: r.get::<_, i64>(2)? != 0,
                        plan_json: r.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(db_error)
    }

    /// Marks the token used (a token starts one run).
    pub fn use_plan(&self, token: &str) -> CmdResult<bool> {
        let n = self
            .db
            .execute(
                "UPDATE plans SET used = 1 WHERE token = ?1 AND used = 0",
                [token],
            )
            .map_err(db_error)?;
        Ok(n == 1)
    }

    // ---- runs ----------------------------------------------------------------------

    pub fn start_run(&self, token: &str, at: u64) -> CmdResult<i64> {
        self.db
            .execute(
                "INSERT INTO runs (token, started_at, status) VALUES (?1, ?2, 'running')",
                params![token, i(at)],
            )
            .map_err(db_error)?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn finish_run(&self, run: i64, status: &str, at: u64, report_json: &str) -> CmdResult<()> {
        self.db
            .execute(
                "UPDATE runs SET status = ?2, finished_at = ?3, report_json = ?4 WHERE id = ?1",
                params![run, status, i(at), report_json],
            )
            .map_err(db_error)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_set(
        &self,
        run: i64,
        store: &str,
        app: &str,
        locale: &str,
        slot: &str,
        target: &str,
        lang: &str,
        hashes: &[String],
        at: u64,
    ) -> CmdResult<i64> {
        let hashes = serde_json::to_string(hashes).unwrap_or_default();
        self.db
            .execute(
                "INSERT INTO sets (run_id, store, app, locale, slot, target, lang, hashes, status, started_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'running', ?9)",
                params![run, store, app, locale, slot, target, lang, hashes, i(at)],
            )
            .map_err(db_error)?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn finish_set(
        &self,
        set: i64,
        status: &str,
        error: Option<&str>,
        at: u64,
    ) -> CmdResult<()> {
        self.db
            .execute(
                "UPDATE sets SET status = ?2, error = ?3, finished_at = ?4 WHERE id = ?1",
                params![set, status, error, i(at)],
            )
            .map_err(db_error)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn file(
        &self,
        set: i64,
        index: u32,
        file_name: &str,
        sha256: &str,
        bytes: u64,
        store_id: Option<&str>,
        status: &str,
        error: Option<&str>,
    ) -> CmdResult<()> {
        self.db
            .execute(
                "INSERT INTO files (set_id, idx, file_name, sha256, bytes, store_id, status, error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    set,
                    index,
                    file_name,
                    sha256,
                    i(bytes),
                    store_id,
                    status,
                    error
                ],
            )
            .map_err(db_error)?;
        Ok(())
    }

    pub fn event(&self, run: i64, level: &str, message: &str, at: u64) -> CmdResult<()> {
        self.db
            .execute(
                "INSERT INTO events (run_id, at, level, message) VALUES (?1, ?2, ?3, ?4)",
                params![run, i(at), level, message],
            )
            .map_err(db_error)?;
        Ok(())
    }

    /// Image hashes of the last successful upload of a set.
    pub fn last_success(
        &self,
        store: &str,
        app: &str,
        locale: &str,
        slot: &str,
    ) -> CmdResult<Option<(Vec<String>, u64)>> {
        let row: Option<(String, i64)> = self
            .db
            .query_row(
                "SELECT hashes, COALESCE(finished_at, started_at) FROM sets
                 WHERE store = ?1 AND app = ?2 AND locale = ?3 AND slot = ?4 AND status = 'done'
                 ORDER BY id DESC LIMIT 1",
                params![store, app, locale, slot],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        Ok(row.map(|(h, at)| {
            (
                serde_json::from_str(&h).unwrap_or_default(),
                u64::try_from(at).unwrap_or(0),
            )
        }))
    }

    /// The last `limit` runs, newest first (with events when `events`).
    pub fn runs(&self, limit: usize, events: bool) -> CmdResult<Vec<RunRecord>> {
        let mut stmt = self
            .db
            .prepare(
                "SELECT id, started_at, finished_at, status FROM runs ORDER BY id DESC LIMIT ?1",
            )
            .map_err(db_error)?;
        let runs: Vec<(i64, i64, Option<i64>, String)> = stmt
            .query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map_err(db_error)?
            .collect::<Result<_, _>>()
            .map_err(db_error)?;
        let to_u = |v: i64| u64::try_from(v).unwrap_or(0);
        let mut out = Vec::new();
        for (id, started, finished, status) in runs {
            out.push(RunRecord {
                id,
                started_at: to_u(started),
                finished_at: finished.map(to_u),
                status,
                sets: self.sets_of(id)?,
                events: if events {
                    self.events_of(id)?
                } else {
                    Vec::new()
                },
            });
        }
        Ok(out)
    }

    fn sets_of(&self, run: i64) -> CmdResult<Vec<SetRecord>> {
        let mut stmt = self
            .db
            .prepare(
                "SELECT id, store, app, locale, slot, target, lang, hashes, status, error
                 FROM sets WHERE run_id = ?1 ORDER BY id",
            )
            .map_err(db_error)?;
        #[allow(clippy::type_complexity)]
        let rows: Vec<(i64, [String; 8], Option<String>)> = stmt
            .query_map([run], |r| {
                Ok((
                    r.get(0)?,
                    [
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                        r.get(8)?,
                    ],
                    r.get(9)?,
                ))
            })
            .map_err(db_error)?
            .collect::<Result<_, _>>()
            .map_err(db_error)?;
        let mut out = Vec::new();
        for (id, [store, app, locale, slot, target, lang, hashes, status], error) in rows {
            let images = serde_json::from_str::<Vec<String>>(&hashes).map_or(0, |h| h.len());
            out.push(SetRecord {
                store,
                app,
                locale,
                slot,
                target,
                lang,
                images,
                status,
                error,
                files: self.files_of(id)?,
            });
        }
        Ok(out)
    }

    fn files_of(&self, set: i64) -> CmdResult<Vec<FileRecord>> {
        let mut stmt = self
            .db
            .prepare(
                "SELECT idx, file_name, sha256, bytes, store_id, status, error
                 FROM files WHERE set_id = ?1 ORDER BY id",
            )
            .map_err(db_error)?;
        stmt.query_map([set], |r| {
            Ok(FileRecord {
                index: r.get(0)?,
                file_name: r.get(1)?,
                sha256: r.get(2)?,
                bytes: u64::try_from(r.get::<_, i64>(3)?).unwrap_or(0),
                store_id: r.get(4)?,
                status: r.get(5)?,
                error: r.get(6)?,
            })
        })
        .map_err(db_error)?
        .collect::<Result<_, _>>()
        .map_err(db_error)
    }

    fn events_of(&self, run: i64) -> CmdResult<Vec<EventRecord>> {
        let mut stmt = self
            .db
            .prepare("SELECT at, level, message FROM events WHERE run_id = ?1 ORDER BY id")
            .map_err(db_error)?;
        stmt.query_map([run], |r| {
            Ok(EventRecord {
                at: u64::try_from(r.get::<_, i64>(0)?).unwrap_or(0),
                level: r.get(1)?,
                message: r.get(2)?,
            })
        })
        .map_err(db_error)?
        .collect::<Result<_, _>>()
        .map_err(db_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_runs_and_last_success() {
        let tmp = tempfile::tempdir().unwrap();
        let log = UploadLog::open(tmp.path()).unwrap();
        log.save_plan("t1", "fp", 10, 100, "{}").unwrap();
        let p = log.plan("t1").unwrap().unwrap();
        assert_eq!(
            (p.fingerprint.as_str(), p.expires_at, p.used),
            ("fp", 100, false)
        );
        assert!(log.use_plan("t1").unwrap());
        assert!(!log.use_plan("t1").unwrap(), "a token runs once");
        assert!(log.plan("nope").unwrap().is_none());

        let run = log.start_run("t1", 11).unwrap();
        let hashes = vec!["a".to_owned(), "b".to_owned()];
        let set = log
            .start_set(
                run,
                "apple",
                "123",
                "de-DE",
                "APP_IPHONE_67",
                "t",
                "de",
                &hashes,
                11,
            )
            .unwrap();
        assert!(
            log.last_success("apple", "123", "de-DE", "APP_IPHONE_67")
                .unwrap()
                .is_none()
        );
        log.file(set, 1, "01.png", "a", 5, Some("s1"), "done", None)
            .unwrap();
        log.finish_set(set, "done", None, 12).unwrap();
        log.event(run, "info", "PUT /v1/x 200", 12).unwrap();
        log.finish_run(run, "completed", 13, "{}").unwrap();
        let (h, at) = log
            .last_success("apple", "123", "de-DE", "APP_IPHONE_67")
            .unwrap()
            .unwrap();
        assert_eq!((h, at), (hashes, 12));
        let runs = log.runs(5, true).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].sets[0].files[0].store_id.as_deref(), Some("s1"));
        assert_eq!(runs[0].events.len(), 1);
    }
}

//! # vitrine-gallery-server
//!
//! Backend of the template gallery (`api.vitrinestudio.app`, phase 6.3,
//! ADR 0023): browse, search and download approved templates without an
//! account; upload with server-side validation (`vitrine_core::template`)
//! and trusted previews (`vitrine_render::template`); abuse reports; a
//! moderation queue for the admin. SQLite for metadata, files on disk,
//! configuration from the environment ([`Config`]). API sketch:
//! `docs/gallery/API.md`; deployment: `docs/gallery/DEPLOY.md`.
//!
//! Status: skeleton. Uploader login is meant to be GitHub OAuth (feature
//! `github-oauth`, routes are TODO stubs answering 501); until then upload
//! tokens are issued by the owner (`GALLERY_UPLOAD_TOKENS`).

mod auth;
mod config;
mod db;
mod error;
mod ratelimit;
mod routes;
mod storage;

use std::sync::{Arc, Mutex};

pub use config::Config;
pub use routes::router;

/// Shared server state.
#[derive(Debug)]
pub struct AppState {
    pub config: Config,
    db: Mutex<rusqlite::Connection>,
    storage: storage::Storage,
    limiter: ratelimit::RateLimiter,
}

impl AppState {
    /// Opens (creates) the database and the file store below
    /// `config.data_dir`.
    pub fn open(config: Config) -> anyhow::Result<Arc<Self>> {
        std::fs::create_dir_all(&config.data_dir)?;
        let db = db::open(&config.data_dir.join("gallery.sqlite"))?;
        let storage = storage::Storage::new(config.data_dir.join("templates"));
        let limiter = ratelimit::RateLimiter::new(config.rate_limit_per_hour);
        Ok(Arc::new(Self {
            config,
            db: Mutex::new(db),
            storage,
            limiter,
        }))
    }

    fn db(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

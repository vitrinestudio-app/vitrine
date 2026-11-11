//! # vitrine-api-server
//!
//! Backend of `api.vitrinestudio.app` (ADR 0023, 0025). Today it serves the
//! template gallery: browse, search and download approved templates without
//! an account; upload with server-side validation (`vitrine_core::template`)
//! and trusted previews (`vitrine_render::template`); abuse reports; a
//! moderation queue for the admin; uploader login with GitHub (device flow
//! for the app/CLI, web flow for a website) and server-issued session
//! tokens. SQLite for metadata, files on disk, configuration from
//! `VITRINE_API_*` environment variables ([`Config`]). API:
//! `docs/api/API.md`; deployment: `docs/api/DEPLOY.md`.
//!
//! Layout: one module per API area below [`routes`] (`templates`, `admin`,
//! `auth`), shared [`AppState`], [`Config`], `db`, `error`, `ratelimit`. A
//! new area adds `routes/<area>.rs` with a `routes()` router and merges it
//! in [`router`].

mod auth;
mod config;
mod db;
mod error;
mod ratelimit;
mod routes;
mod storage;

use std::sync::{Arc, Mutex};

pub use config::{Config, DEFAULT_GITHUB_CLIENT_ID, DEFAULT_OFFICIAL_LOGIN, StaticToken};
pub use routes::router;

/// Serves the API on `listener` (client addresses for rate limiting from
/// the connection) until `shutdown` completes.
pub async fn serve(
    listener: tokio::net::TcpListener,
    state: Arc<AppState>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

/// Name of the SQLite database in the data folder.
pub const DB_FILE: &str = "api.sqlite";

/// Shared server state.
#[derive(Debug)]
pub struct AppState {
    pub config: Config,
    db: Mutex<rusqlite::Connection>,
    storage: storage::Storage,
    limiter: ratelimit::RateLimiter,
    read_limiter: ratelimit::RateLimiter,
    github: auth::github::Github,
}

impl AppState {
    /// Opens (creates) the database and the file store below
    /// `config.data_dir`.
    pub fn open(config: Config) -> anyhow::Result<Arc<Self>> {
        std::fs::create_dir_all(&config.data_dir)?;
        let db = db::open(&config.data_dir.join(DB_FILE))?;
        let storage = storage::Storage::new(config.data_dir.join("templates"));
        let limiter = ratelimit::RateLimiter::new(config.rate_limit_per_hour);
        let read_limiter = ratelimit::RateLimiter::new(config.read_rate_limit_per_hour);
        let github = auth::github::Github::new(
            &config.github_url,
            &config.github_api_url,
            &config.github_client_id,
        )?;
        Ok(Arc::new(Self {
            config,
            db: Mutex::new(db),
            storage,
            limiter,
            read_limiter,
            github,
        }))
    }

    fn db(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

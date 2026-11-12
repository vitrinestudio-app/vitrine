//! `vitrine-api`: the backend of `api.vitrinestudio.app` (template gallery,
//! GitHub login). Configuration via `VITRINE_API_*` environment variables
//! (see `Config`, `docs/api/DEPLOY.md`).

use vitrine_api_server::{AppState, Config, serve};

fn main() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    let bind = config.bind;
    let state = AppState::open(config)?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let listener = tokio::net::TcpListener::bind(bind).await?;
        eprintln!("vitrine-api listening on {bind}");
        // Fill derived fields and missing sprites of older templates.
        tokio::spawn(vitrine_api_server::backfill(state.clone()));
        serve(listener, state, shutdown()).await?;
        Ok(())
    })
}

/// Ctrl-C, or SIGTERM (`docker stop`) on Unix.
async fn shutdown() {
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = term => {}
    }
}

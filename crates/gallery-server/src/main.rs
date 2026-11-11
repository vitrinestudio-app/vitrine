//! `vitrine-gallery`: the template gallery backend. Configuration via
//! `GALLERY_*` environment variables (see `Config`, `docs/gallery/DEPLOY.md`).

use std::net::SocketAddr;

use vitrine_gallery_server::{AppState, Config, router};

fn main() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    let bind = config.bind;
    let state = AppState::open(config)?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let listener = tokio::net::TcpListener::bind(bind).await?;
        eprintln!("vitrine-gallery listening on {bind}");
        axum::serve(
            listener,
            router(state).into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown())
        .await?;
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

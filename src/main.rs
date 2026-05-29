use std::sync::Arc;

use agent_sidecar::{
    api::{AppState, build_router},
    codex::CodexRunner,
    config::AppConfig,
    database::DatabaseReadiness,
    secret_filter::SecretFilter,
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = AppConfig::from_env()?;
    let secret_filter = Arc::new(SecretFilter::from_config(
        &config.secret_filter,
        &config.database,
    )?);
    let provider = Arc::new(CodexRunner::new(
        config.codex.clone(),
        config.database.clone(),
        secret_filter,
    ));
    let database = DatabaseReadiness::new(&config.database);

    let app = build_router(AppState { provider, database }).layer(TraceLayer::new_for_http());
    let listener = TcpListener::bind(config.bind_addr).await?;

    tracing::info!(
        bind_addr = %config.bind_addr,
        policy_workspace = %config.codex.policy_workspace.display(),
        "agent sidecar listening"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("agent_sidecar=info,tower_http=info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().compact())
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl-c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install terminate handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

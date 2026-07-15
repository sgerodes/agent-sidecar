use std::sync::Arc;

use agent_sidecar::{
    api::{AppState, build_router},
    codex::CodexRunner,
    config::AppConfig,
    database::DatabaseReadiness,
    executor::ExecutorAi,
    secret_filter::SecretFilter,
    security::SecurityAi,
    service::ChatService,
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
        config.database.as_ref(),
    )?);

    let executor_env = if config.executor.database_access_enabled {
        config
            .database
            .as_ref()
            .map(|database| database.codex_env())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let executor_runner = CodexRunner::new(
        config.executor.codex.clone(),
        secret_filter.clone(),
        executor_env,
    );
    let executor = Arc::new(ExecutorAi::from_prompt_file(
        executor_runner,
        &config.executor.prompt_path,
        config.executor.database_access_enabled,
    )?);

    let security_ai = if config.security_ai.enabled {
        let security_runner = CodexRunner::new(
            config.security_ai.codex.clone(),
            secret_filter.clone(),
            Vec::new(),
        );
        Some(Arc::new(SecurityAi::from_prompt_file(
            security_runner,
            &config.security_ai.prompt_path,
        )?))
    } else {
        None
    };

    let database = config.database.as_ref().map(DatabaseReadiness::new);
    let chat_service = Arc::new(ChatService::new(executor, security_ai));
    let app = build_router(AppState {
        chat_service,
        database,
    })
    .layer(TraceLayer::new_for_http());
    let listener = TcpListener::bind(config.bind_addr).await?;

    tracing::info!(
        bind_addr = %config.bind_addr,
        policy_workspace = %config.executor.codex.policy_workspace.display(),
        security_ai_enabled = config.security_ai.enabled,
        executor_db_access_enabled = config.executor.database_access_enabled,
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
            .expect("failed to install terminate signal handler")
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

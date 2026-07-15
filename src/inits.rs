use crate::config::app::AppConfig;

pub fn init_app() -> Result<(), config::ConfigError>{
    AppConfig::load()?;
    let config = crate::config::app::get();
    init_logging(config);
    Ok(())
}

fn init_logging(config: &AppConfig) {
    tracing_subscriber::fmt()
        .with_env_filter(config.log_config.log_level.as_str())
        .init();

    tracing::info!(log_level = config.log_config.log_level.as_str(), "Application started with");
}


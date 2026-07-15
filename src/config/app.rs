use serde::Deserialize;
use std::sync::OnceLock;
use crate::config::ai::AiConfig;
use crate::config::log::LogConfig;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub log_config: LogConfig,
    #[serde(default)]
    pub ai_config: AiConfig,
}


impl AppConfig {
    pub fn load() -> Result<(), config::ConfigError> {
        let c = config::Config::builder()
            .build()?
            .try_deserialize()?;
        initialize(c);
        Ok(())
    }
}

pub fn initialize(config: AppConfig) {
    CONFIG
        .set(config)
        .expect("configuration initialized more than once");
}

pub fn get() -> &'static AppConfig {
    CONFIG
        .get()
        .expect("configuration accessed before initialization")
}
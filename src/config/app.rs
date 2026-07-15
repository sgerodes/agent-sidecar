use serde::Deserialize;
use std::sync::OnceLock;
use crate::config::logs;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub log_level: logs::LogLevel,
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
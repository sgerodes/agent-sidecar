use thiserror::Error;
use ::config::ConfigError;

#[derive(Debug, Error)]
pub enum SidecarError {
    #[error("failed to load configuration: {0}")]
    Config(#[from] ConfigError),

}
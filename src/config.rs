use std::{
    env, fmt,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use sqlx::postgres::{PgConnectOptions, PgSslMode};
use thiserror::Error;

const API_BILLING_ENV_VARS: &[&str] = &[
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_API_KEY",
];

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub codex: CodexConfig,
    pub database: PostgresConfig,
    pub secret_filter: SecretFilterConfig,
}

#[derive(Debug, Clone)]
pub struct CodexConfig {
    pub command: String,
    pub model: Option<String>,
    pub timeout: Duration,
    pub policy_workspace: PathBuf,
    pub response_schema_path: PathBuf,
    pub codex_home: Option<PathBuf>,
    pub sandbox: String,
    pub path_env: String,
}

#[derive(Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    pub sslmode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SecretFilterConfig {
    pub secret_file_paths: Vec<PathBuf>,
    pub canary_secrets: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid {key}: {message}")]
    InvalidValue { key: &'static str, message: String },

    #[error("missing required environment variable {0}")]
    MissingEnv(&'static str),

    #[error(
        "API-billed model usage is outside this project scope; unset {0} and use subscription CLI auth"
    )]
    ApiBillingEnvPresent(&'static str),
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        reject_api_billing_env()?;

        Ok(Self {
            bind_addr: parse_env("SIDECAR_BIND_ADDR", "0.0.0.0:8080")?,
            codex: CodexConfig::from_env()?,
            database: PostgresConfig::from_env()?,
            secret_filter: SecretFilterConfig::from_env(),
        })
    }
}

impl CodexConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let policy_workspace = path_env_or(
            "SIDECAR_POLICY_WORKSPACE",
            "/opt/agent-sidecar/policy".as_ref(),
        );
        let response_schema_path = env::var("SIDECAR_RESPONSE_SCHEMA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| policy_workspace.join("response.schema.json"));

        Ok(Self {
            command: env_or("SIDECAR_CODEX_COMMAND", "codex"),
            model: optional_env("SIDECAR_CODEX_MODEL"),
            timeout: Duration::from_secs(parse_env("SIDECAR_PROVIDER_TIMEOUT_SECONDS", "120")?),
            policy_workspace,
            response_schema_path,
            codex_home: optional_env("SIDECAR_CODEX_HOME").map(PathBuf::from),
            sandbox: env_or("SIDECAR_CODEX_SANDBOX", "read-only"),
            path_env: env_or("SIDECAR_CODEX_PATH", "/usr/local/bin:/usr/bin:/bin"),
        })
    }
}

impl PostgresConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            host: required_env("PGHOST")?,
            port: parse_env("PGPORT", "5432")?,
            database: required_env("PGDATABASE")?,
            user: required_env("PGUSER")?,
            password: required_env("PGPASSWORD")?,
            sslmode: optional_env("PGSSLMODE"),
        })
    }

    pub fn codex_env(&self) -> Vec<(String, String)> {
        let mut values = vec![
            ("PGHOST".to_owned(), self.host.clone()),
            ("PGPORT".to_owned(), self.port.to_string()),
            ("PGDATABASE".to_owned(), self.database.clone()),
            ("PGUSER".to_owned(), self.user.clone()),
            ("PGPASSWORD".to_owned(), self.password.clone()),
        ];

        if let Some(sslmode) = &self.sslmode {
            values.push(("PGSSLMODE".to_owned(), sslmode.clone()));
        }

        values
    }

    pub fn connect_options(&self) -> Result<PgConnectOptions, ConfigError> {
        let mut options = PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .database(&self.database)
            .username(&self.user)
            .password(&self.password);

        if let Some(sslmode) = &self.sslmode {
            options = options.ssl_mode(parse_ssl_mode(sslmode)?);
        }

        Ok(options)
    }
}

impl fmt::Debug for PostgresConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("password", &"<redacted>")
            .field("sslmode", &self.sslmode)
            .finish()
    }
}

impl SecretFilterConfig {
    fn from_env() -> Self {
        Self {
            secret_file_paths: split_paths("SIDECAR_SECRET_FILE_PATHS"),
            canary_secrets: split_values("SIDECAR_CANARY_SECRETS"),
        }
    }
}

fn reject_api_billing_env() -> Result<(), ConfigError> {
    for key in API_BILLING_ENV_VARS {
        if optional_env(key).is_some() {
            return Err(ConfigError::ApiBillingEnvPresent(key));
        }
    }

    Ok(())
}

fn required_env(key: &'static str) -> Result<String, ConfigError> {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or(ConfigError::MissingEnv(key))
}

fn optional_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn env_or(key: &str, default: &str) -> String {
    optional_env(key).unwrap_or_else(|| default.to_owned())
}

fn path_env_or(key: &str, default: &Path) -> PathBuf {
    optional_env(key)
        .map(PathBuf::from)
        .unwrap_or_else(|| default.to_path_buf())
}

fn parse_env<T>(key: &'static str, default: &str) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
{
    env::var(key)
        .unwrap_or_else(|_| default.to_owned())
        .parse::<T>()
        .map_err(|error| ConfigError::InvalidValue {
            key,
            message: error.to_string(),
        })
}

fn split_values(key: &str) -> Vec<String> {
    optional_env(key)
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn split_paths(key: &str) -> Vec<PathBuf> {
    split_values(key).into_iter().map(PathBuf::from).collect()
}

fn parse_ssl_mode(value: &str) -> Result<PgSslMode, ConfigError> {
    match value {
        "disable" => Ok(PgSslMode::Disable),
        "prefer" => Ok(PgSslMode::Prefer),
        "require" => Ok(PgSslMode::Require),
        "verify-ca" => Ok(PgSslMode::VerifyCa),
        "verify-full" => Ok(PgSslMode::VerifyFull),
        other => Err(ConfigError::InvalidValue {
            key: "PGSSLMODE",
            message: format!("unsupported sslmode {other:?}"),
        }),
    }
}

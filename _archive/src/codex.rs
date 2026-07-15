use std::{process::Stdio, sync::Arc, time::Instant};

use serde::de::DeserializeOwned;
use thiserror::Error;
use tokio::{io::AsyncWriteExt, process::Command, time};

use crate::{
    config::CodexConfig,
    secret_filter::{SecretDetection, SecretFilter},
};

#[derive(Debug, Clone)]
pub struct CodexRunner {
    config: CodexConfig,
    secret_filter: Arc<SecretFilter>,
    extra_env: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct CodexJsonResult<T> {
    pub output: T,
    pub stats: ProviderRunStats,
}

#[derive(Debug, Clone)]
pub struct ProviderRunStats {
    pub duration_ms: u128,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("failed to build provider prompt")]
    Prompt(#[source] serde_json::Error),

    #[error("failed to spawn provider")]
    Spawn(#[source] std::io::Error),

    #[error("failed to write provider prompt")]
    WritePrompt(#[source] std::io::Error),

    #[error("provider run timed out")]
    Timeout,

    #[error("provider process failed")]
    ProcessFailed {
        code: Option<i32>,
        stdout_bytes: usize,
        stderr_bytes: usize,
    },

    #[error("provider output failed secret egress checks")]
    SecretDetected {
        stream: &'static str,
        detection: SecretDetection,
    },

    #[error("provider returned invalid structured output")]
    InvalidOutput(#[source] serde_json::Error),
}

impl CodexRunner {
    pub fn new(
        config: CodexConfig,
        secret_filter: Arc<SecretFilter>,
        extra_env: Vec<(String, String)>,
    ) -> Self {
        Self {
            config,
            secret_filter,
            extra_env,
        }
    }

    pub async fn run_json<T>(&self, prompt: &str) -> Result<CodexJsonResult<T>, ProviderError>
    where
        T: DeserializeOwned,
    {
        let started_at = Instant::now();
        let mut command = self.build_command();

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(ProviderError::Spawn)?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .await
                .map_err(ProviderError::WritePrompt)?;
        }

        let output = time::timeout(self.config.timeout, child.wait_with_output())
            .await
            .map_err(|_| ProviderError::Timeout)?
            .map_err(ProviderError::Spawn)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if let Some(detection) = self.secret_filter.scan_text(&stdout) {
            return Err(ProviderError::SecretDetected {
                stream: "stdout",
                detection,
            });
        }

        if let Some(detection) = self.secret_filter.scan_text(&stderr) {
            return Err(ProviderError::SecretDetected {
                stream: "stderr",
                detection,
            });
        }

        if !output.status.success() {
            return Err(ProviderError::ProcessFailed {
                code: output.status.code(),
                stdout_bytes: output.stdout.len(),
                stderr_bytes: output.stderr.len(),
            });
        }

        let parsed = parse_provider_output::<T>(&stdout).map_err(ProviderError::InvalidOutput)?;

        Ok(CodexJsonResult {
            output: parsed,
            stats: ProviderRunStats {
                duration_ms: started_at.elapsed().as_millis(),
                stdout_bytes: output.stdout.len(),
                stderr_bytes: output.stderr.len(),
            },
        })
    }

    fn build_command(&self) -> Command {
        let mut command = Command::new(&self.config.command);

        command
            .arg("exec")
            .arg("--sandbox")
            .arg(&self.config.sandbox)
            .arg("--ask-for-approval")
            .arg("never")
            .arg("--ephemeral")
            .arg("--skip-git-repo-check")
            .arg("--ignore-user-config")
            .arg("--color")
            .arg("never")
            .arg("--cd")
            .arg(&self.config.policy_workspace)
            .arg("--output-schema")
            .arg(&self.config.response_schema_path);

        if let Some(model) = &self.config.model {
            command.arg("--model").arg(model);
        }

        command.arg("-");

        command
            .current_dir(&self.config.policy_workspace)
            .env_clear()
            .env("PATH", &self.config.path_env)
            .env("TERM", "dumb")
            .env("NO_COLOR", "1")
            .env("HOME", "/tmp/agent-sidecar-home");

        if let Some(codex_home) = &self.config.codex_home {
            command.env("CODEX_HOME", codex_home);
        }

        for (key, value) in &self.extra_env {
            command.env(key, value);
        }

        command
    }
}

fn parse_provider_output<T>(stdout: &str) -> Result<T, serde_json::Error>
where
    T: DeserializeOwned,
{
    let trimmed = stdout.trim();

    serde_json::from_str(trimmed).or_else(|_| {
        let object = last_json_object(trimmed).unwrap_or(trimmed);
        serde_json::from_str(object)
    })
}

fn last_json_object(value: &str) -> Option<&str> {
    let end = value.rfind('}')?;
    let start = value[..=end].rfind('{')?;
    value.get(start..=end)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    };

    use serde::Deserialize;
    use tempfile::TempDir;

    use crate::{config::CodexConfig, secret_filter::SecretFilter};

    use super::{CodexRunner, ProviderError};

    #[derive(Debug, Deserialize)]
    struct TestOutput {
        answer: String,
    }

    #[tokio::test]
    async fn returns_structured_provider_answer() {
        let temp = TempDir::new().expect("tempdir");
        write_fake_provider(
            temp.path(),
            r#"#!/bin/sh
cat >/dev/null
printf '{"answer":"hello from fake codex"}'
"#,
        );

        let runner = fake_runner(temp.path(), "fake-codex", Vec::new());
        let result = runner
            .run_json::<TestOutput>("hello")
            .await
            .expect("provider result");

        assert_eq!(result.output.answer, "hello from fake codex");
        assert!(result.stats.stdout_bytes > 0);
    }

    #[tokio::test]
    async fn blocks_secret_in_provider_output() {
        let temp = TempDir::new().expect("tempdir");
        write_fake_provider(
            temp.path(),
            r#"#!/bin/sh
cat >/dev/null
printf '{"answer":"leaked test-db-password-123456"}'
"#,
        );

        let runner = fake_runner(temp.path(), "fake-codex", Vec::new());
        let error = runner
            .run_json::<TestOutput>("hello")
            .await
            .expect_err("secret detection");

        assert!(matches!(error, ProviderError::SecretDetected { .. }));
    }

    #[tokio::test]
    async fn rejects_invalid_provider_json() {
        let temp = TempDir::new().expect("tempdir");
        write_fake_provider(
            temp.path(),
            r#"#!/bin/sh
cat >/dev/null
printf 'not json'
"#,
        );

        let runner = fake_runner(temp.path(), "fake-codex", Vec::new());
        let error = runner
            .run_json::<TestOutput>("hello")
            .await
            .expect_err("invalid json");

        assert!(matches!(error, ProviderError::InvalidOutput(_)));
    }

    #[tokio::test]
    async fn passes_only_configured_extra_env_to_provider() {
        let temp = TempDir::new().expect("tempdir");
        write_fake_provider(
            temp.path(),
            r#"#!/bin/sh
cat >/dev/null
if [ "$PGPASSWORD" = "test-db-password-123456" ]; then
  printf '{"answer":"db-env-present"}'
else
  printf '{"answer":"db-env-missing"}'
fi
"#,
        );

        let runner = fake_runner(
            temp.path(),
            "fake-codex",
            vec![(
                "PGPASSWORD".to_owned(),
                "test-db-password-123456".to_owned(),
            )],
        );
        let result = runner
            .run_json::<TestOutput>("hello")
            .await
            .expect("provider result");

        assert_eq!(result.output.answer, "db-env-present");
    }

    fn fake_runner(
        temp_path: &Path,
        command_name: &str,
        extra_env: Vec<(String, String)>,
    ) -> CodexRunner {
        let policy_workspace = temp_path.join("policy");
        fs::create_dir_all(&policy_workspace).expect("policy workspace");
        fs::write(policy_workspace.join("response.schema.json"), "{}").expect("schema");
        let filter = SecretFilter::new([(
            "postgres.password".to_owned(),
            "test-db-password-123456".to_owned(),
        )])
        .expect("filter");

        CodexRunner::new(
            CodexConfig {
                command: temp_path.join(command_name).display().to_string(),
                model: None,
                timeout: Duration::from_secs(5),
                policy_workspace,
                response_schema_path: temp_path.join("policy/response.schema.json"),
                codex_home: None,
                sandbox: "read-only".to_owned(),
                path_env: "/usr/bin:/bin".to_owned(),
            },
            Arc::new(filter),
            extra_env,
        )
    }

    fn write_fake_provider(temp_path: &Path, script: &str) -> PathBuf {
        let path = temp_path.join("fake-codex");
        fs::write(&path, script).expect("fake provider");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("permissions");
        path
    }
}

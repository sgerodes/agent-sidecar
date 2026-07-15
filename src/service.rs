use std::sync::Arc;

use thiserror::Error;

use crate::{
    codex::{ProviderError, ProviderRunStats},
    executor::ExecutorAi,
    models::ChatRequest,
    security::{SecurityAi, SecurityDecision},
};

#[derive(Debug, Clone)]
pub struct ChatService {
    executor: Arc<ExecutorAi>,
    security_ai: Option<Arc<SecurityAi>>,
}

#[derive(Debug, Clone)]
pub struct ChatServiceResult {
    pub answer: String,
    pub executor_stats: ProviderRunStats,
    pub security_ai_result: Option<SecurityAuditResult>,
    pub executor_db_access_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct SecurityAuditResult {
    pub decision: SecurityDecision,
    pub reason: String,
    pub stats: ProviderRunStats,
}

#[derive(Debug, Error)]
pub enum ChatServiceError {
    #[error("security AI blocked request")]
    SecurityBlocked { reason: String },

    #[error("security AI failed")]
    SecurityAiFailed(#[source] ProviderError),

    #[error("executor AI failed")]
    ExecutorFailed(#[source] ProviderError),
}

impl ChatService {
    pub fn new(executor: Arc<ExecutorAi>, security_ai: Option<Arc<SecurityAi>>) -> Self {
        Self {
            executor,
            security_ai,
        }
    }

    pub fn security_ai_enabled(&self) -> bool {
        self.security_ai.is_some()
    }

    pub async fn handle(
        &self,
        request: &ChatRequest,
    ) -> Result<ChatServiceResult, ChatServiceError> {
        let security_ai_result = if let Some(security_ai) = &self.security_ai {
            let check = security_ai
                .check(&request.user_message)
                .await
                .map_err(ChatServiceError::SecurityAiFailed)?;

            if check.decision != SecurityDecision::Allow {
                return Err(ChatServiceError::SecurityBlocked {
                    reason: normalize_reason(check.reason),
                });
            }

            Some(SecurityAuditResult {
                decision: check.decision,
                reason: normalize_reason(check.reason),
                stats: check.stats,
            })
        } else {
            None
        };

        let executor_result = self
            .executor
            .run(request)
            .await
            .map_err(ChatServiceError::ExecutorFailed)?;

        Ok(ChatServiceResult {
            answer: executor_result.answer,
            executor_stats: executor_result.stats,
            security_ai_result,
            executor_db_access_enabled: self.executor.database_access_enabled(),
        })
    }
}

fn normalize_reason(reason: String) -> String {
    let reason = reason.trim();
    if reason.is_empty() {
        "no reason provided".to_owned()
    } else {
        reason.chars().take(240).collect()
    }
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

    use tempfile::TempDir;

    use crate::{
        codex::CodexRunner, config::CodexConfig, executor::ExecutorAi, models::ChatRequest,
        secret_filter::SecretFilter, security::SecurityAi,
    };

    use super::{ChatService, ChatServiceError};

    #[tokio::test]
    async fn security_block_stops_before_executor() {
        let temp = TempDir::new().expect("tempdir");
        let filter = Arc::new(SecretFilter::new([]).expect("filter"));
        let security_script = write_fake_provider(
            temp.path(),
            "security-codex",
            r#"#!/bin/sh
cat >/dev/null
printf '{"decision":"block","reason":"secret request"}'
"#,
        );
        let missing_executor = temp.path().join("missing-executor");
        let service = build_service(temp.path(), filter, missing_executor, security_script, true);

        let error = service
            .handle(&sample_request())
            .await
            .expect_err("blocked request");

        assert!(matches!(error, ChatServiceError::SecurityBlocked { .. }));
    }

    #[tokio::test]
    async fn security_allow_runs_executor() {
        let temp = TempDir::new().expect("tempdir");
        let filter = Arc::new(SecretFilter::new([]).expect("filter"));
        let security_script = write_fake_provider(
            temp.path(),
            "security-codex",
            r#"#!/bin/sh
cat >/dev/null
printf '{"decision":"allow","reason":"safe_request"}'
"#,
        );
        let executor_script = write_fake_provider(
            temp.path(),
            "executor-codex",
            r#"#!/bin/sh
cat >/dev/null
printf '{"answer":"executor answer"}'
"#,
        );
        let service = build_service(temp.path(), filter, executor_script, security_script, true);

        let result = service.handle(&sample_request()).await.expect("result");

        assert_eq!(result.answer, "executor answer");
        assert_eq!(
            result.security_ai_result.expect("security audit").reason,
            "safe_request"
        );
    }

    fn build_service(
        temp_path: &Path,
        filter: Arc<SecretFilter>,
        executor_command: PathBuf,
        security_command: PathBuf,
        security_enabled: bool,
    ) -> ChatService {
        let policy_workspace = temp_path.join("policy");
        fs::create_dir_all(&policy_workspace).expect("policy workspace");
        fs::write(policy_workspace.join("response.schema.json"), "{}").expect("schema");
        fs::write(policy_workspace.join("security-response.schema.json"), "{}")
            .expect("security schema");
        let executor_prompt = policy_workspace.join("executor.md");
        let security_prompt = policy_workspace.join("security.md");
        fs::write(&executor_prompt, "Executor prompt.").expect("executor prompt");
        fs::write(&security_prompt, "Security prompt.").expect("security prompt");

        let executor = Arc::new(
            ExecutorAi::from_prompt_file(
                CodexRunner::new(
                    codex_config(&policy_workspace, executor_command),
                    filter.clone(),
                    Vec::new(),
                ),
                &executor_prompt,
                false,
            )
            .expect("executor"),
        );
        let security_ai = security_enabled.then(|| {
            Arc::new(
                SecurityAi::from_prompt_file(
                    CodexRunner::new(
                        codex_config(&policy_workspace, security_command),
                        filter,
                        Vec::new(),
                    ),
                    &security_prompt,
                )
                .expect("security ai"),
            )
        });

        ChatService::new(executor, security_ai)
    }

    fn codex_config(policy_workspace: &Path, command: PathBuf) -> CodexConfig {
        CodexConfig {
            command: command.display().to_string(),
            model: None,
            timeout: Duration::from_secs(5),
            policy_workspace: policy_workspace.to_path_buf(),
            response_schema_path: policy_workspace.join("response.schema.json"),
            codex_home: None,
            sandbox: "read-only".to_owned(),
            path_env: "/usr/bin:/bin".to_owned(),
        }
    }

    fn sample_request() -> ChatRequest {
        ChatRequest {
            conversation_id: None,
            conversation: Vec::new(),
            user_message: "hello".to_owned(),
            metadata: Default::default(),
        }
    }

    fn write_fake_provider(temp_path: &Path, name: &str, script: &str) -> PathBuf {
        let path = temp_path.join(name);
        fs::write(&path, script).expect("fake provider");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("permissions");
        path
    }
}

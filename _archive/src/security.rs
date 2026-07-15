use std::{fs, path::Path, sync::Arc};

use serde::Deserialize;

use crate::{
    codex::{CodexRunner, ProviderError, ProviderRunStats},
    prompt::build_security_prompt,
};

#[derive(Debug, Clone)]
pub struct SecurityAi {
    runner: CodexRunner,
    base_prompt: Arc<str>,
}

#[derive(Debug, Clone)]
pub struct SecurityCheckResult {
    pub decision: SecurityDecision,
    pub reason: String,
    pub stats: ProviderRunStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityDecision {
    Allow,
    Block,
}

#[derive(Debug, Deserialize)]
struct SecurityStructuredOutput {
    decision: SecurityDecision,
    reason: String,
}

impl SecurityAi {
    pub fn from_prompt_file(
        runner: CodexRunner,
        prompt_path: &Path,
    ) -> Result<Self, std::io::Error> {
        Ok(Self {
            runner,
            base_prompt: fs::read_to_string(prompt_path)?.into(),
        })
    }

    pub async fn check(&self, user_message: &str) -> Result<SecurityCheckResult, ProviderError> {
        let prompt = build_security_prompt(&self.base_prompt, user_message);
        let result = self
            .runner
            .run_json::<SecurityStructuredOutput>(&prompt)
            .await?;

        Ok(SecurityCheckResult {
            decision: result.output.decision,
            reason: result.output.reason,
            stats: result.stats,
        })
    }
}

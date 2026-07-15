use std::{fs, path::Path, sync::Arc};

use crate::{
    codex::{CodexRunner, ProviderError, ProviderRunStats},
    models::{ChatRequest, ProviderStructuredOutput},
    prompt::build_executor_prompt,
};

#[derive(Debug, Clone)]
pub struct ExecutorAi {
    runner: CodexRunner,
    base_prompt: Arc<str>,
    database_access_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct ExecutorRunResult {
    pub answer: String,
    pub stats: ProviderRunStats,
}

impl ExecutorAi {
    pub fn from_prompt_file(
        runner: CodexRunner,
        prompt_path: &Path,
        database_access_enabled: bool,
    ) -> Result<Self, std::io::Error> {
        Ok(Self {
            runner,
            base_prompt: fs::read_to_string(prompt_path)?.into(),
            database_access_enabled,
        })
    }

    pub fn database_access_enabled(&self) -> bool {
        self.database_access_enabled
    }

    pub async fn run(&self, request: &ChatRequest) -> Result<ExecutorRunResult, ProviderError> {
        let prompt =
            build_executor_prompt(&self.base_prompt, request, self.database_access_enabled)
                .map_err(ProviderError::Prompt)?;
        let result = self
            .runner
            .run_json::<ProviderStructuredOutput>(&prompt)
            .await?;

        Ok(ExecutorRunResult {
            answer: result.output.answer,
            stats: result.stats,
        })
    }
}

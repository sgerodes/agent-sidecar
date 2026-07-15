use crate::ai::error::AiError;
use crate::ai::types::{AiProvider, PromptRequest, PromptResponse};

pub struct CodexAiProvider {
    // Codex-specific client/config
}

impl AiProvider for CodexAiProvider {
    fn prompt(
        &self,
        request: PromptRequest,
    ) -> Result<PromptResponse, AiError> {
        tracing::info!(request = request.prompt, "Request received");
        Ok(PromptResponse{
            content: "Not implemented yet".to_string(),
        })
    }
}
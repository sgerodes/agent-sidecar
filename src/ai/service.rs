use std::sync::Arc;
use crate::ai::error::AiError;
use crate::ai::providers::codex::CodexAiProvider;
use crate::ai::types::{AiProvider, PromptRequest, PromptResponse};
use crate::config::ai::AiProviderKind;

pub struct AiService {
    provider: Arc<dyn AiProvider>,
}

impl AiService {
    pub fn new(provider: Arc<dyn AiProvider>) -> Self {
        Self { provider }
    }
    pub fn prompt(
        &self,
        request: PromptRequest,
    ) -> Result<PromptResponse, AiError> {
        self.provider.prompt(request)
    }
}


pub fn prompt(request: String) -> Result<PromptResponse, AiError> {
    let config = crate::config::app::get();

    let provider: Arc<dyn AiProvider> = match config.ai_config.ai_provider {
        AiProviderKind::Codex => Arc::new(CodexAiProvider {}),
        _ => todo!("Other providers are not implemented yet"),
    };

    let ai_service = AiService::new(provider);

    ai_service.prompt(PromptRequest{
        prompt: request.to_string(),
        system_prompt: None,
    })
}
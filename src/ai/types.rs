// #[async_trait::async_trait]
// pub trait AiProvider: Send + Sync {
//     async fn prompt(&self, request: PromptRequest)
//                     -> Result<PromptResponse, AiError>;
// }

use crate::ai::error::AiError;

pub trait AiProvider {
    fn prompt(&self, request: PromptRequest)
                    -> Result<PromptResponse, AiError>;
}

pub struct PromptRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
}

pub struct PromptResponse {
    pub content: String,
}
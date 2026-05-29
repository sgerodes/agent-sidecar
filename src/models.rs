use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct ChatRequest {
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub conversation: Vec<ConversationMessage>,
    pub user_message: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConversationMessage {
    pub role: ConversationRole,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatResponse {
    pub request_id: Uuid,
    pub status: ResponseStatus,
    pub answer: String,
    pub provider: String,
    pub diagnostics: ResponseDiagnostics,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Completed,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseDiagnostics {
    pub duration_ms: u128,
    pub secret_filter_checked: bool,
    pub provider_stdout_bytes: usize,
    pub provider_stderr_bytes: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderStructuredOutput {
    pub answer: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub request_id: Option<Uuid>,
    pub status: ErrorStatus,
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorStatus {
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
}

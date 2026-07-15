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
    pub executor_stdout_bytes: usize,
    pub executor_stderr_bytes: usize,
    pub executor_db_access_enabled: bool,
    pub security_ai_enabled: bool,
    pub security_ai_checked: bool,
    pub security_ai_decision: Option<String>,
    pub security_ai_reason: Option<String>,
    pub security_ai_duration_ms: Option<u128>,
    pub security_ai_stdout_bytes: Option<usize>,
    pub security_ai_stderr_bytes: Option<usize>,
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

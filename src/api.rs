use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    codex::ProviderError,
    database::DatabaseReadiness,
    models::{
        ChatRequest, ChatResponse, ErrorBody, ErrorResponse, ErrorStatus, ResponseDiagnostics,
        ResponseStatus,
    },
    security::SecurityDecision,
    service::{ChatService, ChatServiceError},
};

#[derive(Clone)]
pub struct AppState {
    pub chat_service: Arc<ChatService>,
    pub database: Option<DatabaseReadiness>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/chat", post(chat))
        .with_state(state)
}

async fn healthz() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn readyz(State(state): State<AppState>) -> Response {
    let Some(database) = &state.database else {
        return Json(json!({ "status": "ready", "database": "disabled" })).into_response();
    };

    match database.check().await {
        Ok(()) => Json(json!({ "status": "ready", "database": "ready" })).into_response(),
        Err(error) => {
            tracing::warn!(error = %error, "database readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "not_ready", "database": "not_ready" })),
            )
                .into_response()
        }
    }
}

async fn chat(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, ApiError> {
    let request_id = Uuid::new_v4();

    if request.user_message.trim().is_empty() {
        return Err(ApiError::bad_request(
            request_id,
            "empty_user_message",
            "user_message must not be empty",
        ));
    }

    let result = state
        .chat_service
        .handle(&request)
        .await
        .map_err(|error| ApiError::from_service(request_id, error))?;
    let security = result.security_ai_result.as_ref();

    Ok(Json(ChatResponse {
        request_id,
        status: ResponseStatus::Completed,
        answer: result.answer,
        provider: "codex".to_owned(),
        diagnostics: ResponseDiagnostics {
            duration_ms: result.executor_stats.duration_ms,
            secret_filter_checked: true,
            executor_stdout_bytes: result.executor_stats.stdout_bytes,
            executor_stderr_bytes: result.executor_stats.stderr_bytes,
            executor_db_access_enabled: result.executor_db_access_enabled,
            security_ai_enabled: state.chat_service.security_ai_enabled(),
            security_ai_checked: security.is_some(),
            security_ai_decision: security.map(|audit| decision_name(audit.decision).to_owned()),
            security_ai_reason: security.map(|audit| audit.reason.clone()),
            security_ai_duration_ms: security.map(|audit| audit.stats.duration_ms),
            security_ai_stdout_bytes: security.map(|audit| audit.stats.stdout_bytes),
            security_ai_stderr_bytes: security.map(|audit| audit.stats.stderr_bytes),
        },
    }))
}

#[derive(Debug)]
pub struct ApiError {
    request_id: Option<Uuid>,
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn bad_request(request_id: Uuid, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            request_id: Some(request_id),
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }

    fn from_service(request_id: Uuid, error: ChatServiceError) -> Self {
        match error {
            ChatServiceError::SecurityBlocked { reason } => {
                tracing::warn!(
                    request_id = %request_id,
                    reason,
                    "security AI blocked request"
                );

                Self {
                    request_id: Some(request_id),
                    status: StatusCode::FORBIDDEN,
                    code: "security_ai_blocked",
                    message: format!("request blocked by security AI: {reason}"),
                }
            }
            ChatServiceError::SecurityAiFailed(error) => {
                Self::from_provider_error(request_id, "security_ai", error)
            }
            ChatServiceError::ExecutorFailed(error) => {
                Self::from_provider_error(request_id, "executor_ai", error)
            }
        }
    }

    fn from_provider_error(request_id: Uuid, stage: &'static str, error: ProviderError) -> Self {
        match error {
            ProviderError::SecretDetected { stream, detection } => {
                tracing::warn!(
                    request_id = %request_id,
                    stage,
                    stream,
                    rule = detection.label,
                    "provider output blocked by egress secret filter"
                );

                Self {
                    request_id: Some(request_id),
                    status: StatusCode::BAD_GATEWAY,
                    code: stage_code(stage, "egress_secret_detected"),
                    message: format!("{stage} output failed secret safety checks"),
                }
            }
            ProviderError::Timeout => Self {
                request_id: Some(request_id),
                status: StatusCode::GATEWAY_TIMEOUT,
                code: stage_code(stage, "timeout"),
                message: format!("{stage} timed out"),
            },
            ProviderError::ProcessFailed {
                code,
                stdout_bytes,
                stderr_bytes,
            } => {
                tracing::warn!(
                    request_id = %request_id,
                    stage,
                    exit_code = ?code,
                    stdout_bytes,
                    stderr_bytes,
                    "provider process failed"
                );

                Self {
                    request_id: Some(request_id),
                    status: StatusCode::BAD_GATEWAY,
                    code: stage_code(stage, "process_failed"),
                    message: format!("{stage} process failed"),
                }
            }
            ProviderError::InvalidOutput(_) => Self {
                request_id: Some(request_id),
                status: StatusCode::BAD_GATEWAY,
                code: stage_code(stage, "invalid_output"),
                message: format!("{stage} returned invalid JSON output"),
            },
            ProviderError::Prompt(_) | ProviderError::Spawn(_) | ProviderError::WritePrompt(_) => {
                tracing::error!(
                    request_id = %request_id,
                    stage,
                    error = %error,
                    "provider execution failed"
                );

                Self {
                    request_id: Some(request_id),
                    status: StatusCode::BAD_GATEWAY,
                    code: stage_code(stage, "execution_failed"),
                    message: format!("{stage} execution failed"),
                }
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorResponse {
            request_id: self.request_id,
            status: ErrorStatus::Failed,
            error: ErrorBody {
                code: self.code,
                message: self.message,
            },
        };

        (self.status, Json(body)).into_response()
    }
}

fn decision_name(decision: SecurityDecision) -> &'static str {
    match decision {
        SecurityDecision::Allow => "allow",
        SecurityDecision::Block => "block",
    }
}

fn stage_code(stage: &'static str, suffix: &'static str) -> &'static str {
    match (stage, suffix) {
        ("security_ai", "egress_secret_detected") => "security_ai_egress_secret_detected",
        ("security_ai", "timeout") => "security_ai_timeout",
        ("security_ai", "process_failed") => "security_ai_process_failed",
        ("security_ai", "invalid_output") => "security_ai_invalid_output",
        ("security_ai", "execution_failed") => "security_ai_execution_failed",
        ("executor_ai", "egress_secret_detected") => "executor_ai_egress_secret_detected",
        ("executor_ai", "timeout") => "executor_ai_timeout",
        ("executor_ai", "process_failed") => "executor_ai_process_failed",
        ("executor_ai", "invalid_output") => "executor_ai_invalid_output",
        ("executor_ai", "execution_failed") => "executor_ai_execution_failed",
        _ => "provider_execution_failed",
    }
}

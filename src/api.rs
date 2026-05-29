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
    codex::{CodexRunner, ProviderError},
    database::DatabaseReadiness,
    models::{
        ChatRequest, ChatResponse, ErrorBody, ErrorResponse, ErrorStatus, ResponseDiagnostics,
        ResponseStatus,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<CodexRunner>,
    pub database: DatabaseReadiness,
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
    match state.database.check().await {
        Ok(()) => Json(json!({ "status": "ready" })).into_response(),
        Err(error) => {
            tracing::warn!(error = %error, "database readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "not_ready" })),
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

    let provider_result = state
        .provider
        .run(&request)
        .await
        .map_err(|error| ApiError::from_provider(request_id, error))?;

    Ok(Json(ChatResponse {
        request_id,
        status: ResponseStatus::Completed,
        answer: provider_result.answer,
        provider: "codex".to_owned(),
        diagnostics: ResponseDiagnostics {
            duration_ms: provider_result.duration_ms,
            secret_filter_checked: true,
            provider_stdout_bytes: provider_result.stdout_bytes,
            provider_stderr_bytes: provider_result.stderr_bytes,
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

    fn from_provider(request_id: Uuid, error: ProviderError) -> Self {
        match error {
            ProviderError::SecretDetected { stream, detection } => {
                tracing::warn!(
                    request_id = %request_id,
                    stream,
                    rule = detection.label,
                    "provider output blocked by egress secret filter"
                );

                Self {
                    request_id: Some(request_id),
                    status: StatusCode::BAD_GATEWAY,
                    code: "egress_secret_detected",
                    message: "provider output failed safety checks".to_owned(),
                }
            }
            ProviderError::Timeout => Self {
                request_id: Some(request_id),
                status: StatusCode::GATEWAY_TIMEOUT,
                code: "provider_timeout",
                message: "provider run timed out".to_owned(),
            },
            ProviderError::ProcessFailed {
                code,
                stdout_bytes,
                stderr_bytes,
            } => {
                tracing::warn!(
                    request_id = %request_id,
                    exit_code = ?code,
                    stdout_bytes,
                    stderr_bytes,
                    "provider process failed"
                );

                Self {
                    request_id: Some(request_id),
                    status: StatusCode::BAD_GATEWAY,
                    code: "provider_process_failed",
                    message: "provider process failed".to_owned(),
                }
            }
            ProviderError::InvalidOutput(_) => Self {
                request_id: Some(request_id),
                status: StatusCode::BAD_GATEWAY,
                code: "provider_invalid_output",
                message: "provider returned invalid structured output".to_owned(),
            },
            ProviderError::Prompt(_) | ProviderError::Spawn(_) | ProviderError::WritePrompt(_) => {
                tracing::error!(request_id = %request_id, error = %error, "provider execution failed");

                Self {
                    request_id: Some(request_id),
                    status: StatusCode::BAD_GATEWAY,
                    code: "provider_execution_failed",
                    message: "provider execution failed".to_owned(),
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

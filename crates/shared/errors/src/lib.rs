//! Unified error handling for the CBTC platform.
//!
//! Every service uses `AppError` as its top-level error type so that
//! error → HTTP response mapping, logging, and audit-trail hooks stay
//! consistent across all fifteen services. Service-specific error enums
//! should implement `From<ServiceError> for AppError` rather than
//! constructing `axum::response::Response` directly.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

/// Canonical error type returned by every handler in every service.
///
/// Each variant carries enough context to produce both a safe client-facing
/// message and a detailed server-side log line. The `error_id` is generated
/// at construction time so operators can correlate a client-visible error
/// with the corresponding tracing span / audit log entry.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("validation failed: {message}")]
    Validation { message: String },

    #[error("resource not found: {resource} ({id})")]
    NotFound { resource: &'static str, id: String },

    #[error("resource conflict: {message}")]
    Conflict { message: String },

    #[error("unauthorized: {message}")]
    Unauthorized { message: String },

    #[error("forbidden: {message}")]
    Forbidden { message: String },

    #[error("rate limit exceeded")]
    RateLimited,

    #[error("database error")]
    Database(#[from] sqlx::Error),

    #[error("message broker error: {0}")]
    MessageBroker(String),

    #[error("cache error: {0}")]
    Cache(String),

    #[error("upstream service error: {service} returned {status}")]
    UpstreamService { service: &'static str, status: u16 },

    #[error("safety invariant violated: {message}")]
    SafetyViolation { message: String },

    #[error("serialization error")]
    Serialization(#[from] serde_json::Error),

    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

/// Machine-readable error body returned to API clients.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error_id: Uuid,
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    /// Stable machine-readable error code, independent of the Display message.
    /// Used by client SDKs and the dashboard to branch on error type.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Validation { .. } => "VALIDATION_ERROR",
            AppError::NotFound { .. } => "NOT_FOUND",
            AppError::Conflict { .. } => "CONFLICT",
            AppError::Unauthorized { .. } => "UNAUTHORIZED",
            AppError::Forbidden { .. } => "FORBIDDEN",
            AppError::RateLimited => "RATE_LIMITED",
            AppError::Database(_) => "DATABASE_ERROR",
            AppError::MessageBroker(_) => "MESSAGE_BROKER_ERROR",
            AppError::Cache(_) => "CACHE_ERROR",
            AppError::UpstreamService { .. } => "UPSTREAM_SERVICE_ERROR",
            AppError::SafetyViolation { .. } => "SAFETY_VIOLATION",
            AppError::Serialization(_) => "SERIALIZATION_ERROR",
            AppError::Internal(_) => "INTERNAL_ERROR",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            AppError::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::NotFound { .. } => StatusCode::NOT_FOUND,
            AppError::Conflict { .. } => StatusCode::CONFLICT,
            AppError::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            AppError::Forbidden { .. } => StatusCode::FORBIDDEN,
            AppError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::MessageBroker(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Cache(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::UpstreamService { .. } => StatusCode::BAD_GATEWAY,
            AppError::SafetyViolation { .. } => StatusCode::CONFLICT,
            AppError::Serialization(_) => StatusCode::BAD_REQUEST,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Message safe to return to an API client. Internal errors are
    /// deliberately generic so we never leak DB/internal details.
    fn client_message(&self) -> String {
        match self {
            AppError::Database(_) => "an internal error occurred".to_string(),
            AppError::MessageBroker(_) => "an internal error occurred".to_string(),
            AppError::Cache(_) => "an internal error occurred".to_string(),
            AppError::Internal(_) => "an internal error occurred".to_string(),
            other => other.to_string(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        AppError::Validation {
            message: message.into(),
        }
    }

    pub fn not_found(resource: &'static str, id: impl Into<String>) -> Self {
        AppError::NotFound {
            resource,
            id: id.into(),
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        AppError::Conflict {
            message: message.into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        AppError::Unauthorized {
            message: message.into(),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        AppError::Forbidden {
            message: message.into(),
        }
    }

    pub fn safety_violation(message: impl Into<String>) -> Self {
        AppError::SafetyViolation {
            message: message.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let error_id = Uuid::new_v4();
        let status = self.status();
        let code = self.code();
        let client_message = self.client_message();

        // Server-side detail always gets logged at the correct severity,
        // even though the client only sees the sanitized message above.
        // SafetyViolation is logged at error! regardless of build profile
        // because it indicates a potential vital-logic breach.
        match &self {
            AppError::SafetyViolation { .. } => {
                tracing::error!(error_id = %error_id, code, error = %self, "safety violation");
            }
            AppError::Database(_) | AppError::Internal(_) | AppError::MessageBroker(_) => {
                tracing::error!(error_id = %error_id, code, error = %self, "internal error");
            }
            AppError::UpstreamService { .. } => {
                tracing::warn!(error_id = %error_id, code, error = %self, "upstream error");
            }
            _ => {
                tracing::debug!(error_id = %error_id, code, error = %self, "client error");
            }
        }

        let body = ErrorBody {
            error_id,
            code,
            message: client_message,
        };
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;

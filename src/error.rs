//! Stable public error taxonomy and internal error type.

use serde::Serialize;
use thiserror::Error;

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, DarialError>;

/// Stable public error codes (wire + OpenAI-compatible mapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Malformed or invalid client request.
    InvalidRequest,
    /// Missing or bad credentials.
    Unauthenticated,
    /// Authenticated but not permitted.
    Unauthorized,
    /// Policy / allowlist / DLP denial.
    PolicyDenied,
    /// Capability not supported by route/provider.
    UnsupportedCapability,
    /// Rate or quota exceeded.
    RateLimited,
    /// Server overloaded / load shed.
    Overloaded,
    /// Upstream timed out.
    UpstreamTimeout,
    /// Upstream rejected the request.
    UpstreamRejected,
    /// Upstream returned malformed data.
    UpstreamMalformed,
    /// Transport failure to upstream or peer.
    TransportFailure,
    /// Privacy/key negotiation failure.
    PrivacyNegotiationFailure,
    /// Unexpected internal failure.
    InternalError,
}

impl ErrorCode {
    /// Snake-case wire string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Unauthenticated => "unauthenticated",
            Self::Unauthorized => "unauthorized",
            Self::PolicyDenied => "policy_denied",
            Self::UnsupportedCapability => "unsupported_capability",
            Self::RateLimited => "rate_limited",
            Self::Overloaded => "overloaded",
            Self::UpstreamTimeout => "upstream_timeout",
            Self::UpstreamRejected => "upstream_rejected",
            Self::UpstreamMalformed => "upstream_malformed",
            Self::TransportFailure => "transport_failure",
            Self::PrivacyNegotiationFailure => "privacy_negotiation_failure",
            Self::InternalError => "internal_error",
        }
    }

    /// Suggested HTTP status.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::InvalidRequest | Self::UnsupportedCapability => 400,
            Self::Unauthenticated => 401,
            Self::Unauthorized | Self::PolicyDenied => 403,
            Self::RateLimited => 429,
            Self::Overloaded => 503,
            Self::UpstreamTimeout => 504,
            Self::UpstreamRejected => 502,
            Self::UpstreamMalformed
            | Self::TransportFailure
            | Self::PrivacyNegotiationFailure
            | Self::InternalError => 500,
        }
    }
}

/// Internal error. Display messages MUST remain safe for clients (no secrets/prompts).
#[derive(Debug, Error)]
pub enum DarialError {
    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),
    /// Authentication failure.
    #[error("unauthenticated")]
    Unauthenticated,
    /// Authorization failure.
    #[error("unauthorized")]
    Unauthorized,
    /// Policy denial with safe reason.
    #[error("policy denied: {0}")]
    PolicyDenied(String),
    /// Invalid request with safe reason.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// Unsupported capability.
    #[error("unsupported capability: {0}")]
    UnsupportedCapability(String),
    /// Rate limited.
    #[error("rate limited")]
    RateLimited,
    /// Overloaded.
    #[error("overloaded")]
    Overloaded,
    /// Upstream timeout.
    #[error("upstream timeout")]
    UpstreamTimeout,
    /// Upstream rejected.
    #[error("upstream rejected: {0}")]
    UpstreamRejected(String),
    /// Upstream malformed.
    #[error("upstream malformed")]
    UpstreamMalformed,
    /// Transport failure.
    #[error("transport failure")]
    TransportFailure,
    /// Privacy negotiation failure.
    #[error("privacy negotiation failure: {0}")]
    PrivacyNegotiationFailure(String),
    /// Internal error.
    #[error("internal error")]
    Internal,
    /// Serialization.
    #[error("serialization error")]
    Serde(#[from] serde_json::Error),
    /// I/O.
    #[error("io error")]
    Io(#[from] std::io::Error),
}

impl DarialError {
    /// Map to stable public code.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Config(_) | Self::Internal | Self::Serde(_) | Self::Io(_) => {
                ErrorCode::InternalError
            }
            Self::Unauthenticated => ErrorCode::Unauthenticated,
            Self::Unauthorized => ErrorCode::Unauthorized,
            Self::PolicyDenied(_) => ErrorCode::PolicyDenied,
            Self::InvalidRequest(_) => ErrorCode::InvalidRequest,
            Self::UnsupportedCapability(_) => ErrorCode::UnsupportedCapability,
            Self::RateLimited => ErrorCode::RateLimited,
            Self::Overloaded => ErrorCode::Overloaded,
            Self::UpstreamTimeout => ErrorCode::UpstreamTimeout,
            Self::UpstreamRejected(_) => ErrorCode::UpstreamRejected,
            Self::UpstreamMalformed => ErrorCode::UpstreamMalformed,
            Self::TransportFailure => ErrorCode::TransportFailure,
            Self::PrivacyNegotiationFailure(_) => ErrorCode::PrivacyNegotiationFailure,
        }
    }

    /// Safe client-facing message (never includes secrets).
    #[must_use]
    pub fn client_message(&self) -> String {
        self.to_string()
    }
}

/// OpenAI-compatible error body.
#[derive(Debug, Serialize)]
pub struct PublicErrorBody {
    /// Nested error object.
    pub error: PublicError,
}

/// Nested error fields.
#[derive(Debug, Serialize)]
pub struct PublicError {
    /// Stable code.
    pub code: String,
    /// Safe message.
    pub message: String,
    /// Request id when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// OpenAI-style type.
    #[serde(rename = "type")]
    pub kind: String,
}

impl PublicErrorBody {
    /// Build from Darial error.
    #[must_use]
    pub fn from_error(err: &DarialError, request_id: Option<String>) -> Self {
        let code = err.code();
        Self {
            error: PublicError {
                code: code.as_str().to_string(),
                message: err.client_message(),
                request_id,
                kind: code.as_str().to_string(),
            },
        }
    }
}

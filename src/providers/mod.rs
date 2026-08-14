//! Provider adapter trait and shared types.

pub mod anthropic;
pub mod gemini;
pub mod mock;
pub mod openai;

use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::canonical_ir::{ChatRequest, ChatResponse, ModelCapabilities, StreamEvent};
use crate::error::Result;

/// Shared provider context (keys never leave gateway).
#[derive(Debug, Clone)]
pub struct ProviderContext {
    /// Upstream model id.
    pub model: String,
    /// Base URL from admin config.
    pub base_url: String,
    /// API key (empty for mock).
    pub api_key: String,
}

/// Async provider adapter.
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// Adapter name.
    fn name(&self) -> &'static str;

    /// Capabilities for this adapter.
    fn capabilities(&self) -> ModelCapabilities;

    /// Non-streaming generation.
    async fn generate(&self, ctx: &ProviderContext, request: &ChatRequest) -> Result<ChatResponse>;

    /// Streaming generation.
    async fn stream(
        &self,
        ctx: &ProviderContext,
        request: &ChatRequest,
    ) -> Result<BoxStream<'static, Result<StreamEvent>>>;
}

/// Build HTTP client with safe defaults (no auto cross-host redirects).
pub fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(120))
        .user_agent("darial-gateway/0.1")
        .build()
        .map_err(|_| crate::error::DarialError::Internal)
}

/// Map HTTP status to Darial errors with sanitized messages.
pub fn map_http_status(status: u16, _body: &str) -> crate::error::DarialError {
    use crate::error::DarialError;
    match status {
        401 | 403 => DarialError::UpstreamRejected("provider auth/policy".into()),
        404 => DarialError::UpstreamRejected("not found".into()),
        429 => DarialError::RateLimited,
        408 | 504 => DarialError::UpstreamTimeout,
        500..=599 => DarialError::UpstreamRejected("provider 5xx".into()),
        _ => DarialError::UpstreamRejected(format!("status {status}")),
    }
}

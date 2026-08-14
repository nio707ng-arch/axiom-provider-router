//! Content-blind relay: forwards protected envelopes to a configured gateway.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;
use tokio::net::TcpListener;

use crate::config::Config;
use crate::crypto_envelope::Envelope;
use crate::error::{DarialError, PublicErrorBody, Result};
use crate::protocol::{
    parse_protocol_version, CONTENT_TYPE_ENVELOPE, HEADER_PROTOCOL, PROTOCOL_VERSION,
};

/// Relay state.
pub struct RelayState {
    /// Config.
    pub config: Config,
    /// Client.
    pub client: reqwest::Client,
}

/// Serve relay.
pub async fn serve(config: Config) -> Result<()> {
    config.validate("relay")?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(
            config.limits.request_timeout_secs,
        ))
        .build()
        .map_err(|_| DarialError::Internal)?;
    let state = Arc::new(RelayState { config, client });
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/darial/relay", post(forward))
        .with_state(state.clone());

    let addr: SocketAddr = state
        .config
        .listen
        .bind
        .parse()
        .map_err(|e| DarialError::Config(format!("bind: {e}")))?;
    tracing::info!(%addr, "darial relay listening (content-blind)");
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| DarialError::Config(format!("bind failed: {e}")))?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| DarialError::Internal)?;
    Ok(())
}

async fn forward(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Json(env): Json<Envelope>,
) -> Response {
    match do_forward(&state, &headers, env).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            let status = StatusCode::from_u16(e.code().http_status())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(PublicErrorBody::from_error(&e, None))).into_response()
        }
    }
}

async fn do_forward(state: &RelayState, headers: &HeaderMap, env: Envelope) -> Result<Value> {
    let v = headers
        .get(HEADER_PROTOCOL)
        .or_else(|| headers.get("Darial-Protocol"))
        .and_then(|v| v.to_str().ok());
    parse_protocol_version(v).map_err(DarialError::PrivacyNegotiationFailure)?;

    // Relay MUST NOT attempt decryption. Log only metadata.
    tracing::info!(
        kid = %env.kid,
        ciphertext_len = env.ciphertext_b64.len(),
        "relay forward"
    );

    let gateway = state
        .config
        .relay
        .gateway_url
        .as_deref()
        .ok_or_else(|| DarialError::Config("gateway_url".into()))?;
    let url = format!("{}/v1/darial/private/chat", gateway.trim_end_matches('/'));

    // Do not forward client IP or X-Forwarded-For.
    let resp = state
        .client
        .post(&url)
        .header(HEADER_PROTOCOL, PROTOCOL_VERSION.to_string())
        .header(axum::http::header::CONTENT_TYPE, CONTENT_TYPE_ENVELOPE)
        .json(&env)
        .send()
        .await
        .map_err(|_| DarialError::TransportFailure)?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|_| DarialError::TransportFailure)?;
    if !(200..300).contains(&status) {
        return Err(DarialError::UpstreamRejected(format!("gateway {status}")));
    }
    serde_json::from_str(&text).map_err(|_| DarialError::UpstreamMalformed)
}

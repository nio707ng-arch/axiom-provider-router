//! Local loopback OpenAI-compatible agent.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::stream::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use crate::auth::{extract_bearer, EnvSecretStore, SecretStore};
use crate::canonical_ir::{
    validate_request, ChatRequest, ContentPart, GenerationSettings, Message, Role, StreamEvent,
    ToolChoice, IR_VERSION,
};
use crate::config::{Config, PrivacyProfile};
use crate::crypto_envelope::{Envelope, GatewayKeypair, KeySet};
use crate::dlp::DlpEngine;
use crate::error::{DarialError, PublicErrorBody, Result};
use crate::protocol::{Direction, CONTENT_TYPE_ENVELOPE, HEADER_PROTOCOL, PROTOCOL_VERSION};

/// Shared agent state.
pub struct AgentState {
    /// Config.
    pub config: Config,
    /// DLP engine.
    pub dlp: DlpEngine,
    /// HTTP client to gateway/relay.
    pub client: reqwest::Client,
    /// Secrets.
    pub secrets: EnvSecretStore,
}

/// Serve the local agent.
pub async fn serve(config: Config) -> Result<()> {
    config.validate("agent")?;
    let dlp = DlpEngine::from_config(&config.dlp)?;
    let client = config.http_client()?;
    let state = Arc::new(AgentState {
        config: config.clone(),
        dlp,
        client,
        secrets: EnvSecretStore,
    });
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .layer(CorsLayer::new()) // deny permissive CORS by default (empty)
        .with_state(state);

    let addr: SocketAddr = config
        .listen
        .bind
        .parse()
        .map_err(|e| DarialError::Config(format!("bind: {e}")))?;
    tracing::info!(%addr, "darial agent listening (loopback-oriented)");
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| DarialError::Config(format!("bind failed: {e}")))?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|_| DarialError::Internal)?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn list_models(State(state): State<Arc<AgentState>>) -> impl IntoResponse {
    // Agent asks gateway for models when possible; otherwise return configured aliases.
    let data: Vec<Value> = state
        .config
        .model_aliases
        .keys()
        .map(|id| json!({"id": id, "object": "model", "owned_by": "darial"}))
        .collect();
    // If no local aliases, try gateway.
    if data.is_empty() {
        if let Ok(models) = fetch_gateway_models(&state).await {
            return Json(json!({"object":"list","data": models}));
        }
    }
    Json(json!({"object":"list","data": data}))
}

async fn fetch_gateway_models(state: &AgentState) -> Result<Vec<Value>> {
    let url = format!(
        "{}/v1/darial/models",
        state.config.gateway.url.trim_end_matches('/')
    );
    let mut req = state
        .client
        .get(&url)
        .header(HEADER_PROTOCOL, PROTOCOL_VERSION.to_string());
    if let Some(token) = gateway_token(state)? {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .await
        .map_err(|_| DarialError::TransportFailure)?;
    let v: Value = resp
        .json()
        .await
        .map_err(|_| DarialError::UpstreamMalformed)?;
    Ok(v["data"].as_array().cloned().unwrap_or_default())
}

fn gateway_token(state: &AgentState) -> Result<Option<String>> {
    if let Some(env) = &state.config.gateway.auth_token_env {
        return state.secrets.get(env);
    }
    Ok(None)
}

async fn chat_completions(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
    Json(body): Json<OpenAiChatRequest>,
) -> Response {
    match handle_chat(&state, &headers, body).await {
        Ok(resp) => resp,
        Err(err) => error_response(err, None),
    }
}

async fn handle_chat(
    state: &AgentState,
    headers: &HeaderMap,
    body: OpenAiChatRequest,
) -> Result<Response> {
    // Local auth
    if state.config.auth.require_local_auth {
        let auth = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());
        let expected_env = state
            .config
            .auth
            .local_token_env
            .as_deref()
            .unwrap_or("DARIAL_LOCAL_TOKEN");
        let expected = state
            .secrets
            .get(expected_env)?
            .ok_or(DarialError::Unauthenticated)?;
        let presented = extract_bearer(auth).ok_or(DarialError::Unauthenticated)?;
        if presented != expected {
            return Err(DarialError::Unauthenticated);
        }
    }

    let mut ir = openai_to_ir(body)?;
    ir.privacy_profile = state.config.gateway.privacy_profile;
    ir.tenant_id = state.config.auth.default_tenant.clone();
    validate_request(
        &ir,
        state.config.limits.max_messages,
        state.config.limits.max_tools,
    )?;

    let outcome = state.dlp.apply(ir)?;
    let ir = outcome.request;
    let stream = ir.stream;

    if state.config.gateway.privacy_profile == PrivacyProfile::PrivateBuffered {
        let resp = forward_private(state, &ir).await?;
        return Ok(Json(ir_to_openai(&resp)).into_response());
    }

    if stream {
        let events = forward_direct_stream(state, &ir).await?;
        let sse = Sse::new(events.map(|ev| {
            let data = match ev {
                Ok(e) => openai_sse_chunk(&e),
                Err(err) => json!({
                    "error": {"message": err.client_message(), "code": err.code().as_str()}
                })
                .to_string(),
            };
            Ok::<_, std::convert::Infallible>(Event::default().data(data))
        }))
        .keep_alive(KeepAlive::default());
        return Ok(sse.into_response());
    }

    let resp = forward_direct(state, &ir).await?;
    Ok(Json(ir_to_openai(&resp)).into_response())
}

async fn forward_direct(
    state: &AgentState,
    ir: &ChatRequest,
) -> Result<crate::canonical_ir::ChatResponse> {
    let url = format!(
        "{}/v1/darial/chat",
        state.config.gateway.url.trim_end_matches('/')
    );
    let mut req = state
        .client
        .post(&url)
        .header(HEADER_PROTOCOL, PROTOCOL_VERSION.to_string())
        .json(ir);
    if let Some(token) = gateway_token(state)? {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .await
        .map_err(|_| DarialError::TransportFailure)?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|_| DarialError::TransportFailure)?;
    if !(200..300).contains(&status) {
        return Err(map_agent_upstream(status, &text));
    }
    serde_json::from_str(&text).map_err(|_| DarialError::UpstreamMalformed)
}

async fn forward_direct_stream(
    state: &AgentState,
    ir: &ChatRequest,
) -> Result<impl futures::Stream<Item = Result<StreamEvent>> + Send + 'static> {
    let url = format!(
        "{}/v1/darial/chat/stream",
        state.config.gateway.url.trim_end_matches('/')
    );
    let mut req = state
        .client
        .post(&url)
        .header(HEADER_PROTOCOL, PROTOCOL_VERSION.to_string())
        .json(ir);
    if let Some(token) = gateway_token(state)? {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .await
        .map_err(|_| DarialError::TransportFailure)?;
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(map_agent_upstream(status, &text));
    }
    let byte_stream = resp.bytes_stream();
    Ok(async_stream::stream! {
        let mut buffer = String::new();
        futures::pin_mut!(byte_stream);
        use futures::StreamExt;
        while let Some(chunk) = byte_stream.next().await {
            let Ok(bytes) = chunk else {
                yield Err(DarialError::TransportFailure);
                return;
            };
            buffer.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(idx) = buffer.find('\n') {
                let line = buffer[..idx].trim_end().to_string();
                buffer = buffer[idx + 1..].to_string();
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        return;
                    }
                    match serde_json::from_str::<StreamEvent>(data) {
                        Ok(ev) => yield Ok(ev),
                        Err(_) => yield Err(DarialError::UpstreamMalformed),
                    }
                }
            }
        }
    })
}

async fn forward_private(
    state: &AgentState,
    ir: &ChatRequest,
) -> Result<crate::canonical_ir::ChatResponse> {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;

    let relay = state
        .config
        .relay
        .url
        .as_deref()
        .ok_or_else(|| DarialError::Config("relay.url required".into()))?;

    let origin =
        std::env::var("DARIAL_GATEWAY_ORIGIN").unwrap_or_else(|_| state.config.gateway.url.clone());

    let keys_url = format!(
        "{}/v1/darial/ohttp-keys",
        state.config.gateway.url.trim_end_matches('/')
    );
    let keyset: KeySet = state
        .client
        .get(&keys_url)
        .send()
        .await
        .map_err(|_| DarialError::PrivacyNegotiationFailure("key fetch".into()))?
        .json()
        .await
        .map_err(|_| DarialError::PrivacyNegotiationFailure("key parse".into()))?;
    let key = keyset
        .keys
        .first()
        .ok_or_else(|| DarialError::PrivacyNegotiationFailure("no keys".into()))?;
    let gateway_public = B64
        .decode(&key.public_key_b64)
        .map_err(|_| DarialError::PrivacyNegotiationFailure("key b64".into()))?;

    // Ephemeral client keypair for response sealing.
    let response_kp = GatewayKeypair::generate(format!("resp-{}", uuid::Uuid::new_v4()))?;
    let auth = gateway_token(state)?;
    let payload = serde_json::json!({
        "auth": auth,
        "response_public_b64": B64.encode(&response_kp.public),
        "response_kid": response_kp.kid,
        "request": ir,
    });
    let env = GatewayKeypair::seal_with_public(
        &key.kid,
        &gateway_public,
        &serde_json::to_vec(&payload).map_err(DarialError::from)?,
        Direction::Request,
        &origin,
    )?;

    let url = format!("{}/v1/darial/relay", relay.trim_end_matches('/'));
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
        return Err(map_agent_upstream(status, &text));
    }
    let resp_env: Envelope =
        serde_json::from_str(&text).map_err(|_| DarialError::UpstreamMalformed)?;
    let pt = response_kp.open(&resp_env, Direction::Response, &origin)?;
    serde_json::from_slice(&pt).map_err(|_| DarialError::UpstreamMalformed)
}

fn map_agent_upstream(status: u16, body: &str) -> DarialError {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        if let Some(code) = v["error"]["code"].as_str() {
            return match code {
                "unauthenticated" => DarialError::Unauthenticated,
                "unauthorized" | "policy_denied" => DarialError::PolicyDenied(
                    v["error"]["message"].as_str().unwrap_or("denied").into(),
                ),
                "rate_limited" => DarialError::RateLimited,
                _ => DarialError::UpstreamRejected(code.into()),
            };
        }
    }
    DarialError::UpstreamRejected(format!("status {status}"))
}

fn error_response(err: DarialError, request_id: Option<String>) -> Response {
    let status =
        StatusCode::from_u16(err.code().http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = PublicErrorBody::from_error(&err, request_id);
    (status, Json(body)).into_response()
}

/// OpenAI chat request subset.
#[derive(Debug, Deserialize)]
pub struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(default)]
    stream: bool,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    #[serde(default)]
    tools: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: OpenAiContent,
    tool_call_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OpenAiContent {
    Text(String),
    Parts(Vec<Value>),
}

fn openai_to_ir(body: OpenAiChatRequest) -> Result<ChatRequest> {
    let messages = body
        .messages
        .into_iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "system" => Role::System,
                "developer" => Role::Developer,
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "tool" => Role::Tool,
                _ => Role::User,
            };
            let content = match m.content {
                OpenAiContent::Text(t) => vec![ContentPart::Text { text: t }],
                OpenAiContent::Parts(parts) => parts
                    .into_iter()
                    .filter_map(|p| {
                        let ty = p.get("type")?.as_str()?;
                        match ty {
                            "text" => Some(ContentPart::Text {
                                text: p.get("text")?.as_str()?.to_string(),
                            }),
                            "image_url" => {
                                let url = p.pointer("/image_url/url")?.as_str()?;
                                if let Some(rest) = url.strip_prefix("data:") {
                                    let (mt, b64) = rest.split_once(";base64,")?;
                                    Some(ContentPart::ImageInline {
                                        media_type: mt.to_string(),
                                        data_base64: b64.to_string(),
                                    })
                                } else {
                                    // MVP: do not fetch arbitrary URLs
                                    Some(ContentPart::Text {
                                        text: "[image_url omitted: remote fetch not allowed]"
                                            .into(),
                                    })
                                }
                            }
                            _ => None,
                        }
                    })
                    .collect(),
            };
            Message {
                role,
                content,
                tool_call_id: m.tool_call_id,
                name: None,
            }
        })
        .collect();
    let tools = body
        .tools
        .into_iter()
        .filter_map(|t| {
            let f = t.get("function")?;
            Some(crate::canonical_ir::ToolDefinition {
                name: f.get("name")?.as_str()?.to_string(),
                description: f
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string(),
                parameters: f.get("parameters").cloned().unwrap_or(json!({})),
            })
        })
        .collect();
    Ok(ChatRequest {
        ir_version: IR_VERSION,
        request_id: uuid::Uuid::new_v4(),
        idempotency_key: uuid::Uuid::new_v4().to_string(),
        created_at: chrono::Utc::now(),
        privacy_profile: PrivacyProfile::Direct,
        tenant_id: "default".into(),
        model: body.model,
        messages,
        tools,
        tool_choice: ToolChoice::Auto,
        settings: GenerationSettings {
            temperature: body.temperature,
            max_output_tokens: body.max_tokens,
            ..Default::default()
        },
        stream: body.stream,
        deadline: None,
    })
}

fn ir_to_openai(resp: &crate::canonical_ir::ChatResponse) -> Value {
    let choices: Vec<Value> = resp
        .choices
        .iter()
        .map(|c| {
            let text = c
                .message
                .content
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");
            json!({
                "index": c.index,
                "message": {
                    "role": "assistant",
                    "content": text,
                    "tool_calls": c.message.tool_calls.iter().map(|t| json!({
                        "id": t.id,
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "arguments": t.arguments.to_string(),
                        }
                    })).collect::<Vec<_>>(),
                },
                "finish_reason": match c.finish_reason {
                    crate::canonical_ir::FinishReason::Stop => "stop",
                    crate::canonical_ir::FinishReason::Length => "length",
                    crate::canonical_ir::FinishReason::ToolCalls => "tool_calls",
                    crate::canonical_ir::FinishReason::ContentFilter => "content_filter",
                    crate::canonical_ir::FinishReason::Other => "stop",
                }
            })
        })
        .collect();
    json!({
        "id": format!("chatcmpl-{}", resp.request_id),
        "object": "chat.completion",
        "model": resp.model,
        "choices": choices,
        "usage": {
            "prompt_tokens": resp.usage.input_tokens,
            "completion_tokens": resp.usage.output_tokens,
            "total_tokens": resp.usage.input_tokens + resp.usage.output_tokens,
        }
    })
}

fn openai_sse_chunk(ev: &StreamEvent) -> String {
    match ev {
        StreamEvent::ResponseStart { request_id, model } => json!({
            "id": format!("chatcmpl-{request_id}"),
            "object": "chat.completion.chunk",
            "model": model,
            "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": null}]
        })
        .to_string(),
        StreamEvent::ContentDelta { text, .. } => json!({
            "id": "chatcmpl-stream",
            "object": "chat.completion.chunk",
            "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}]
        })
        .to_string(),
        StreamEvent::ResponseEnd { finish_reason, .. } => {
            let fr = match finish_reason {
                crate::canonical_ir::FinishReason::Stop => "stop",
                crate::canonical_ir::FinishReason::Length => "length",
                crate::canonical_ir::FinishReason::ToolCalls => "tool_calls",
                crate::canonical_ir::FinishReason::ContentFilter => "content_filter",
                crate::canonical_ir::FinishReason::Other => "stop",
            };
            format!(
                "{}\n\ndata: [DONE]",
                json!({
                    "id": "chatcmpl-stream",
                    "object": "chat.completion.chunk",
                    "choices": [{"index": 0, "delta": {}, "finish_reason": fr}]
                })
            )
        }
        StreamEvent::Error { code, message } => json!({
            "error": {"code": code, "message": message}
        })
        .to_string(),
        _ => json!({"id":"chatcmpl-stream","object":"chat.completion.chunk","choices":[]})
            .to_string(),
    }
}

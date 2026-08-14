//! Gateway: auth, policy, provider routing.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use crate::auth::{verify_bearer, EnvSecretStore, SecretStore};
use crate::canonical_ir::{validate_request, ChatRequest, StreamEvent};
use crate::config::Config;
use crate::crypto_envelope::{Envelope, GatewayKeypair, KeyDocument, KeySet};
use crate::error::{DarialError, PublicErrorBody, Result};
use crate::policy::{authorize_and_route, RateLimiter, ReplayCache};
use crate::protocol::{parse_protocol_version, Direction, HEADER_PROTOCOL, PROTOCOL_VERSION};
use crate::providers::anthropic::AnthropicProvider;
use crate::providers::gemini::GeminiProvider;
use crate::providers::mock::MockProvider;
use crate::providers::openai::OpenAiProvider;
use crate::providers::{ProviderAdapter, ProviderContext};

/// Gateway runtime state.
pub struct GatewayState {
    /// Config.
    pub config: Config,
    /// Adapters by provider name.
    pub adapters: HashMap<String, Arc<dyn ProviderAdapter>>,
    /// Rate limiter.
    pub rate: RateLimiter,
    /// Replay cache.
    pub replay: ReplayCache,
    /// Concurrency semaphore.
    pub concurrency: Arc<Semaphore>,
    /// Secrets.
    pub secrets: EnvSecretStore,
    /// HPKE keypair (optional until generated).
    pub hpke: Option<Arc<GatewayKeypair>>,
    /// Origin string for AAD.
    pub origin: String,
}

/// Serve gateway.
pub async fn serve(config: Config, hpke: Option<GatewayKeypair>) -> Result<()> {
    config.validate("gateway")?;
    let adapters = build_adapters(&config);
    let mut origin = std::env::var("DARIAL_GATEWAY_ORIGIN")
        .unwrap_or_else(|_| format!("http://{}", config.listen.bind));
    // Prefer configured public URL when it looks like a real gateway URL.
    if !config.gateway.url.is_empty() && config.gateway.url.starts_with("http") {
        // For AAD, agents and gateway must agree; examples set DARIAL_GATEWAY_ORIGIN.
        let _ = &mut origin;
    }
    let state = Arc::new(GatewayState {
        concurrency: Arc::new(Semaphore::new(config.limits.max_concurrency)),
        rate: RateLimiter::default(),
        replay: ReplayCache::new(Duration::from_secs(120), 10_000),
        adapters,
        secrets: EnvSecretStore,
        hpke: hpke.map(Arc::new),
        origin,
        config,
    });

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(readyz))
        .route("/v1/darial/models", get(models))
        .route("/v1/darial/chat", post(chat))
        .route("/v1/darial/chat/stream", post(chat_stream))
        .route("/v1/darial/ohttp-keys", get(ohttp_keys))
        .route("/v1/darial/private/chat", post(private_chat))
        .with_state(state.clone());

    let addr: SocketAddr = state
        .config
        .listen
        .bind
        .parse()
        .map_err(|e| DarialError::Config(format!("bind: {e}")))?;
    tracing::info!(%addr, origin = %state.origin, "darial gateway listening");
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

fn build_adapters(cfg: &Config) -> HashMap<String, Arc<dyn ProviderAdapter>> {
    let mut map: HashMap<String, Arc<dyn ProviderAdapter>> = HashMap::new();
    for (name, p) in &cfg.providers {
        if !p.enabled {
            continue;
        }
        let adapter: Arc<dyn ProviderAdapter> = match p.kind.as_str() {
            "mock" => Arc::new(MockProvider),
            "openai" => Arc::new(OpenAiProvider),
            "anthropic" => Arc::new(AnthropicProvider),
            "gemini" => Arc::new(GeminiProvider),
            _ => continue,
        };
        map.insert(name.clone(), adapter);
    }
    map
}

async fn readyz(State(state): State<Arc<GatewayState>>) -> impl IntoResponse {
    if state.adapters.is_empty() {
        (StatusCode::SERVICE_UNAVAILABLE, "no adapters")
    } else {
        (StatusCode::OK, "ready")
    }
}

async fn models(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if let Err(e) = authenticate(&state, &headers) {
        return error_response(e, None);
    }
    if let Err(e) = require_protocol(&headers) {
        return error_response(e, None);
    }
    let data: Vec<Value> = state
        .config
        .model_aliases
        .keys()
        .map(|id| {
            json!({
                "id": id,
                "object": "model",
                "owned_by": "darial",
            })
        })
        .collect();
    Json(json!({"object":"list","data": data})).into_response()
}

async fn ohttp_keys(State(state): State<Arc<GatewayState>>) -> Response {
    let Some(kp) = &state.hpke else {
        return error_response(
            DarialError::PrivacyNegotiationFailure("keys not configured".into()),
            None,
        );
    };
    Json(KeySet {
        keys: vec![kp.public_document()],
    })
    .into_response()
}

async fn chat(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    match handle_chat(&state, &headers, req, false).await {
        Ok(ChatOutcome::Json(v)) => Json(v).into_response(),
        Ok(ChatOutcome::Stream(_)) => error_response(DarialError::Internal, None),
        Err(e) => error_response(e, None),
    }
}

async fn chat_stream(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    Json(mut req): Json<ChatRequest>,
) -> Response {
    req.stream = true;
    match handle_chat(&state, &headers, req, true).await {
        Ok(ChatOutcome::Stream(sse)) => sse,
        Ok(ChatOutcome::Json(_)) => error_response(DarialError::Internal, None),
        Err(e) => error_response(e, None),
    }
}

enum ChatOutcome {
    Json(Value),
    Stream(Response),
}

async fn handle_chat(
    state: &GatewayState,
    headers: &HeaderMap,
    req: ChatRequest,
    stream: bool,
) -> Result<ChatOutcome> {
    authenticate(state, headers)?;
    require_protocol(headers)?;
    let request_id = req.request_id;
    validate_request(
        &req,
        state.config.limits.max_messages,
        state.config.limits.max_tools,
    )?;
    let tenant = if req.tenant_id.is_empty() {
        state.config.auth.default_tenant.clone()
    } else {
        req.tenant_id.clone()
    };
    state
        .rate
        .check(&tenant, state.config.limits.requests_per_minute)?;
    let idem = format!("{tenant}:{}", req.idempotency_key);
    state.replay.check_and_insert(&idem)?;
    let decision = authorize_and_route(&state.config, &req, &tenant)?;
    let _permit = state
        .concurrency
        .clone()
        .try_acquire_owned()
        .map_err(|_| DarialError::Overloaded)?;

    let adapter = state
        .adapters
        .get(&decision.provider)
        .ok_or_else(|| DarialError::PolicyDenied("adapter missing".into()))?
        .clone();
    let pcfg = state
        .config
        .providers
        .get(&decision.provider)
        .ok_or(DarialError::Internal)?;
    let api_key = if let Some(env) = &pcfg.api_key_env {
        state.secrets.get(env)?.unwrap_or_default()
    } else {
        String::new()
    };
    let ctx = ProviderContext {
        model: decision.upstream_model,
        base_url: pcfg
            .base_url
            .clone()
            .unwrap_or_else(|| "http://127.0.0.1".into()),
        api_key,
    };

    tracing::info!(
        request_id = %request_id,
        model_alias = %req.model,
        provider = %decision.provider,
        stream,
        "gateway chat"
    );

    if stream {
        let events = adapter.stream(&ctx, &req).await?;
        let sse = Sse::new(events.map(|ev| {
            let data = match ev {
                Ok(e) => serde_json::to_string(&e).unwrap_or_else(|_| {
                    json!({"event":"error","code":"internal_error","message":"serialize"})
                        .to_string()
                }),
                Err(err) => serde_json::to_string(&StreamEvent::Error {
                    code: err.code().as_str().to_string(),
                    message: err.client_message(),
                })
                .unwrap_or_default(),
            };
            Ok::<_, std::convert::Infallible>(Event::default().data(data))
        }))
        .keep_alive(KeepAlive::default());
        return Ok(ChatOutcome::Stream(sse.into_response()));
    }

    let resp = adapter.generate(&ctx, &req).await?;
    Ok(ChatOutcome::Json(
        serde_json::to_value(resp).map_err(DarialError::from)?,
    ))
}

async fn private_chat(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    Json(env): Json<Envelope>,
) -> Response {
    match handle_private(&state, &headers, env).await {
        Ok(out) => Json(out).into_response(),
        Err(e) => error_response(e, None),
    }
}

#[derive(Debug, Deserialize)]
struct PrivatePayload {
    auth: Option<String>,
    response_public_b64: String,
    response_kid: String,
    request: ChatRequest,
}

async fn handle_private(
    state: &GatewayState,
    headers: &HeaderMap,
    env: Envelope,
) -> Result<Envelope> {
    require_protocol(headers)?;
    let kp = state
        .hpke
        .as_ref()
        .ok_or_else(|| DarialError::PrivacyNegotiationFailure("no hpke key".into()))?;
    let plaintext = kp.open(&env, Direction::Request, &state.origin)?;
    let payload: PrivatePayload = serde_json::from_slice(&plaintext)
        .map_err(|_| DarialError::InvalidRequest("private payload".into()))?;

    let mut fake = HeaderMap::new();
    fake.insert(
        HEADER_PROTOCOL,
        PROTOCOL_VERSION
            .to_string()
            .parse()
            .map_err(|_| DarialError::Internal)?,
    );
    if let Some(auth) = &payload.auth {
        fake.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {auth}")
                .parse()
                .map_err(|_| DarialError::Unauthenticated)?,
        );
    }
    authenticate(state, &fake)?;

    let mut req = payload.request;
    req.stream = false;
    // Already authenticated via in-envelope token; process without re-checking
    // protocol on outer headers (relay does not forward client auth).
    let outcome = handle_chat(state, &fake, req, false).await?;
    let ChatOutcome::Json(v) = outcome else {
        return Err(DarialError::Internal);
    };
    let pt = serde_json::to_vec(&v).map_err(DarialError::from)?;
    let public = B64
        .decode(&payload.response_public_b64)
        .map_err(|_| DarialError::PrivacyNegotiationFailure("response key".into()))?;
    GatewayKeypair::seal_with_public(
        &payload.response_kid,
        &public,
        &pt,
        Direction::Response,
        &state.origin,
    )
}

fn authenticate(state: &GatewayState, headers: &HeaderMap) -> Result<()> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    if state.config.auth.token_sha256.is_empty() && state.config.auth.token_env.is_none() {
        if state.config.profile == crate::config::Profile::Dev {
            return Ok(());
        }
        return Err(DarialError::Unauthenticated);
    }
    verify_bearer(
        auth,
        &state.config.auth.token_sha256,
        state.config.auth.token_env.as_deref(),
        &state.secrets,
    )
}

fn require_protocol(headers: &HeaderMap) -> Result<()> {
    let v = headers
        .get(HEADER_PROTOCOL)
        .or_else(|| headers.get("Darial-Protocol"))
        .and_then(|v| v.to_str().ok());
    parse_protocol_version(v)
        .map(|_| ())
        .map_err(DarialError::PrivacyNegotiationFailure)
}

fn error_response(err: DarialError, request_id: Option<String>) -> Response {
    let status =
        StatusCode::from_u16(err.code().http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = PublicErrorBody::from_error(&err, request_id);
    (status, Json(body)).into_response()
}

/// Generate HPKE key material to a directory.
pub fn generate_keys(output: &std::path::Path, force: bool) -> Result<KeyDocument> {
    std::fs::create_dir_all(output).map_err(DarialError::from)?;
    let path = output.join("hpke_keypair.json");
    if path.exists() && !force {
        return Err(DarialError::Config(
            "key files exist; pass --force to overwrite".into(),
        ));
    }
    let kp = GatewayKeypair::generate(format!("kid-{}", uuid::Uuid::new_v4()))?;
    let doc = kp.public_document();
    save_keypair(output, &kp)?;
    std::fs::write(
        output.join("hpke_public.json"),
        serde_json::to_vec_pretty(&doc).unwrap(),
    )
    .map_err(DarialError::from)?;
    Ok(doc)
}

fn save_keypair(dir: &std::path::Path, kp: &GatewayKeypair) -> Result<()> {
    let path = dir.join("hpke_keypair.json");
    let v = json!({
        "kid": kp.kid,
        "public_key_b64": B64.encode(&kp.public),
        "private_key_b64": B64.encode(kp.private_bytes()),
    });
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .map_err(DarialError::from)?;
        f.write_all(serde_json::to_vec_pretty(&v).unwrap().as_slice())
            .map_err(DarialError::from)?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, serde_json::to_vec_pretty(&v).unwrap()).map_err(DarialError::from)?;
    }
    Ok(())
}

/// Load keypair from directory.
pub fn load_keypair(dir: &std::path::Path) -> Result<GatewayKeypair> {
    let path = dir.join("hpke_keypair.json");
    let text = std::fs::read_to_string(&path).map_err(DarialError::from)?;
    let v: Value = serde_json::from_str(&text).map_err(DarialError::from)?;
    let kid = v["kid"].as_str().unwrap_or("k").to_string();
    let public = B64
        .decode(v["public_key_b64"].as_str().unwrap_or(""))
        .map_err(|_| DarialError::Config("bad public key".into()))?;
    let private = B64
        .decode(v["private_key_b64"].as_str().unwrap_or(""))
        .map_err(|_| DarialError::Config("bad private key".into()))?;
    Ok(GatewayKeypair::from_parts(kid, public, private))
}

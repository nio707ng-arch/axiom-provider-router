//! Anthropic Messages API adapter.

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use serde_json::{json, Value};

use crate::canonical_ir::{
    AssistantMessage, ChatRequest, ChatResponse, Choice, ContentPart, FinishReason,
    ModelCapabilities, Role, StreamEvent, Usage,
};
use crate::error::{DarialError, Result};
use crate::providers::{http_client, map_http_status, ProviderAdapter, ProviderContext};

/// Anthropic adapter.
#[derive(Debug, Default, Clone)]
pub struct AnthropicProvider;

#[async_trait]
impl ProviderAdapter for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            streaming: true,
            tools: true,
            vision: true,
            json_mode: false,
            seed: false,
        }
    }

    async fn generate(&self, ctx: &ProviderContext, request: &ChatRequest) -> Result<ChatResponse> {
        let client = http_client()?;
        let body = translate(ctx, request, false)?;
        let url = format!("{}/v1/messages", ctx.base_url.trim_end_matches('/'));
        let resp = client
            .post(&url)
            .header("x-api-key", &ctx.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|_| DarialError::TransportFailure)?;
        let status = resp.status().as_u16();
        let text = resp
            .text()
            .await
            .map_err(|_| DarialError::TransportFailure)?;
        if !(200..300).contains(&status) {
            return Err(map_http_status(status, &text));
        }
        parse_message(&text, request)
    }

    async fn stream(
        &self,
        ctx: &ProviderContext,
        request: &ChatRequest,
    ) -> Result<BoxStream<'static, Result<StreamEvent>>> {
        let client = http_client()?;
        let body = translate(ctx, request, true)?;
        let url = format!("{}/v1/messages", ctx.base_url.trim_end_matches('/'));
        let resp = client
            .post(&url)
            .header("x-api-key", &ctx.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|_| DarialError::TransportFailure)?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            let text = resp.text().await.unwrap_or_default();
            return Err(map_http_status(status, &text));
        }
        let request_id = request.request_id;
        let model = ctx.model.clone();
        let byte_stream = resp
            .bytes_stream()
            .map_err(|_| DarialError::TransportFailure);
        Ok(crate::providers::openai::sse_content_stream(
            byte_stream,
            request_id,
            model,
            "anthropic",
        )
        .boxed())
    }
}

fn translate(ctx: &ProviderContext, req: &ChatRequest, stream: bool) -> Result<Value> {
    let mut system = String::new();
    let mut messages = Vec::new();
    for m in &req.messages {
        match m.role {
            Role::System | Role::Developer => {
                for p in &m.content {
                    if let ContentPart::Text { text } = p {
                        if !system.is_empty() {
                            system.push('\n');
                        }
                        system.push_str(text);
                    }
                }
            }
            Role::User | Role::Assistant => {
                let role = if m.role == Role::User {
                    "user"
                } else {
                    "assistant"
                };
                let content: Vec<Value> = m
                    .content
                    .iter()
                    .map(|p| match p {
                        ContentPart::Text { text } => json!({"type":"text","text": text}),
                        ContentPart::ImageInline {
                            media_type,
                            data_base64,
                        } => json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": media_type,
                                "data": data_base64,
                            }
                        }),
                        ContentPart::AttachmentRef { id } => {
                            json!({"type":"text","text": format!("[attachment:{id}]")})
                        }
                    })
                    .collect();
                messages.push(json!({"role": role, "content": content}));
            }
            Role::Tool => {
                // Anthropic tool results use user role with tool_result blocks; simplified.
                let text = m
                    .content
                    .iter()
                    .filter_map(|p| match p {
                        ContentPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                messages.push(json!({"role": "user", "content": text}));
            }
        }
    }
    let mut body = json!({
        "model": ctx.model,
        "messages": messages,
        "max_tokens": req.settings.max_output_tokens.unwrap_or(1024),
        "stream": stream,
    });
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    if let Some(t) = req.settings.temperature {
        body["temperature"] = json!(t);
    }
    Ok(body)
}

fn parse_message(text: &str, request: &ChatRequest) -> Result<ChatResponse> {
    let v: Value = serde_json::from_str(text).map_err(|_| DarialError::UpstreamMalformed)?;
    let mut content = String::new();
    if let Some(arr) = v["content"].as_array() {
        for block in arr {
            if block["type"] == "text" {
                if let Some(t) = block["text"].as_str() {
                    content.push_str(t);
                }
            }
        }
    }
    Ok(ChatResponse {
        request_id: request.request_id,
        provider_request_id: v["id"].as_str().map(str::to_string),
        model: v["model"].as_str().unwrap_or(&request.model).to_string(),
        choices: vec![Choice {
            index: 0,
            message: AssistantMessage {
                role: Role::Assistant,
                content: vec![ContentPart::Text { text: content }],
                tool_calls: vec![],
            },
            finish_reason: FinishReason::Stop,
        }],
        usage: Usage {
            input_tokens: v["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: v["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
        },
    })
}

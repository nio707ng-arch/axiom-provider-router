//! OpenAI Chat Completions adapter.

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::canonical_ir::{
    AssistantMessage, ChatRequest, ChatResponse, Choice, ContentPart, FinishReason,
    ModelCapabilities, Role, StreamEvent, ToolCall, Usage,
};
use crate::error::{DarialError, Result};
use crate::providers::{http_client, map_http_status, ProviderAdapter, ProviderContext};

/// OpenAI-compatible HTTP adapter.
#[derive(Debug, Default, Clone)]
pub struct OpenAiProvider;

#[async_trait]
impl ProviderAdapter for OpenAiProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            streaming: true,
            tools: true,
            vision: true,
            json_mode: true,
            seed: true,
        }
    }

    async fn generate(&self, ctx: &ProviderContext, request: &ChatRequest) -> Result<ChatResponse> {
        let client = http_client()?;
        let body = translate_request(ctx, request, false)?;
        let url = format!("{}/chat/completions", ctx.base_url.trim_end_matches('/'));
        let resp = client
            .post(&url)
            .bearer_auth(&ctx.api_key)
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
        parse_completion(&text, request)
    }

    async fn stream(
        &self,
        ctx: &ProviderContext,
        request: &ChatRequest,
    ) -> Result<BoxStream<'static, Result<StreamEvent>>> {
        let client = http_client()?;
        let body = translate_request(ctx, request, true)?;
        let url = format!("{}/chat/completions", ctx.base_url.trim_end_matches('/'));
        let resp = client
            .post(&url)
            .bearer_auth(&ctx.api_key)
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
        Ok(sse_content_stream(byte_stream, request_id, model, "openai").boxed())
    }
}

/// Shared SSE parser for OpenAI-like and Anthropic-like content deltas.
pub fn sse_content_stream<S>(
    stream: S,
    request_id: Uuid,
    model: String,
    flavor: &'static str,
) -> impl Stream<Item = Result<StreamEvent>> + Send
where
    S: Stream<Item = Result<Bytes>> + Send + 'static,
{
    async_stream::stream! {
        yield Ok(StreamEvent::ResponseStart { request_id, model });
        let mut buffer = String::new();
        futures::pin_mut!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    buffer.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(idx) = buffer.find('\n') {
                        let line = buffer[..idx].trim_end().to_string();
                        buffer = buffer[idx + 1..].to_string();
                        if let Some(ev) = parse_sse_line(&line, flavor) {
                            let done = matches!(ev, StreamEvent::ResponseEnd { .. });
                            yield Ok(ev);
                            if done {
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    yield Err(e);
                    return;
                }
            }
        }
        yield Ok(StreamEvent::ResponseEnd {
            finish_reason: FinishReason::Stop,
            usage: Usage::default(),
        });
    }
}

fn parse_sse_line(line: &str, flavor: &str) -> Option<StreamEvent> {
    if !line.starts_with("data:") {
        return None;
    }
    let data = line.trim_start_matches("data:").trim();
    if data == "[DONE]" {
        return Some(StreamEvent::ResponseEnd {
            finish_reason: FinishReason::Stop,
            usage: Usage::default(),
        });
    }
    let v: Value = serde_json::from_str(data).ok()?;
    if flavor == "openai" {
        if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
            return Some(StreamEvent::ContentDelta {
                index: 0,
                text: delta.to_string(),
            });
        }
    } else if flavor == "anthropic" || flavor == "delta" {
        // Anthropic content_block_delta
        if v["type"] == "content_block_delta" {
            if let Some(delta) = v["delta"]["text"].as_str() {
                return Some(StreamEvent::ContentDelta {
                    index: 0,
                    text: delta.to_string(),
                });
            }
        }
        if v["type"] == "message_stop" {
            return Some(StreamEvent::ResponseEnd {
                finish_reason: FinishReason::Stop,
                usage: Usage::default(),
            });
        }
    }
    None
}

fn translate_request(ctx: &ProviderContext, req: &ChatRequest, stream: bool) -> Result<Value> {
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::System | Role::Developer => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            };
            let content = parts_to_openai(&m.content);
            let mut obj = json!({"role": role, "content": content});
            if let Some(id) = &m.tool_call_id {
                obj["tool_call_id"] = json!(id);
            }
            obj
        })
        .collect();
    let mut body = json!({
        "model": ctx.model,
        "messages": messages,
        "stream": stream,
    });
    if let Some(t) = req.settings.temperature {
        body["temperature"] = json!(t);
    }
    if let Some(m) = req.settings.max_output_tokens {
        body["max_tokens"] = json!(m);
    }
    if !req.tools.is_empty() {
        body["tools"] = json!(req
            .tools
            .iter()
            .map(|t| json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            }))
            .collect::<Vec<_>>());
    }
    Ok(body)
}

fn parts_to_openai(parts: &[ContentPart]) -> Value {
    if parts.len() == 1 {
        if let ContentPart::Text { text } = &parts[0] {
            return json!(text);
        }
    }
    let arr: Vec<Value> = parts
        .iter()
        .map(|p| match p {
            ContentPart::Text { text } => json!({"type":"text","text": text}),
            ContentPart::ImageInline {
                media_type,
                data_base64,
            } => json!({
                "type": "image_url",
                "image_url": {"url": format!("data:{media_type};base64,{data_base64}")}
            }),
            ContentPart::AttachmentRef { id } => {
                json!({"type":"text","text": format!("[attachment:{id}]")})
            }
        })
        .collect();
    json!(arr)
}

fn parse_completion(text: &str, request: &ChatRequest) -> Result<ChatResponse> {
    let v: Value = serde_json::from_str(text).map_err(|_| DarialError::UpstreamMalformed)?;
    let choice = &v["choices"][0];
    let content = choice["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let tool_calls = parse_tool_calls(&choice["message"]["tool_calls"]);
    let finish = map_finish(choice["finish_reason"].as_str());
    Ok(ChatResponse {
        request_id: request.request_id,
        provider_request_id: v["id"].as_str().map(str::to_string),
        model: v["model"].as_str().unwrap_or(&request.model).to_string(),
        choices: vec![Choice {
            index: 0,
            message: AssistantMessage {
                role: Role::Assistant,
                content: vec![ContentPart::Text { text: content }],
                tool_calls,
            },
            finish_reason: finish,
        }],
        usage: Usage {
            input_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
        },
    })
}

fn parse_tool_calls(v: &Value) -> Vec<ToolCall> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let id = t["id"].as_str()?.to_string();
                    let name = t["function"]["name"].as_str()?.to_string();
                    let args_str = t["function"]["arguments"].as_str().unwrap_or("{}");
                    let arguments = serde_json::from_str(args_str).unwrap_or(json!({}));
                    Some(ToolCall {
                        id,
                        name,
                        arguments,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn map_finish(s: Option<&str>) -> FinishReason {
    match s {
        Some("stop") => FinishReason::Stop,
        Some("length") => FinishReason::Length,
        Some("tool_calls") => FinishReason::ToolCalls,
        Some("content_filter") => FinishReason::ContentFilter,
        _ => FinishReason::Other,
    }
}

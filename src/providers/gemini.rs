//! Google Gemini generateContent adapter.

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde_json::{json, Value};

use crate::canonical_ir::{
    AssistantMessage, ChatRequest, ChatResponse, Choice, ContentPart, FinishReason,
    ModelCapabilities, Role, StreamEvent, Usage,
};
use crate::error::{DarialError, Result};
use crate::providers::{http_client, map_http_status, ProviderAdapter, ProviderContext};

/// Gemini adapter.
#[derive(Debug, Default, Clone)]
pub struct GeminiProvider;

#[async_trait]
impl ProviderAdapter for GeminiProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            streaming: true,
            tools: true,
            vision: true,
            json_mode: true,
            seed: false,
        }
    }

    async fn generate(&self, ctx: &ProviderContext, request: &ChatRequest) -> Result<ChatResponse> {
        let client = http_client()?;
        let body = translate(request)?;
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            ctx.base_url.trim_end_matches('/'),
            ctx.model,
            urlencoding_minimal(&ctx.api_key)
        );
        let resp = client
            .post(&url)
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
        parse_response(&text, request, &ctx.model)
    }

    async fn stream(
        &self,
        ctx: &ProviderContext,
        request: &ChatRequest,
    ) -> Result<BoxStream<'static, Result<StreamEvent>>> {
        // Gemini streaming uses streamGenerateContent; for MVP we buffer generate then emit deltas.
        let response = self.generate(ctx, request).await?;
        let text = response
            .choices
            .first()
            .and_then(|c| {
                c.message.content.iter().find_map(|p| match p {
                    ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
            })
            .unwrap_or_default();
        let request_id = request.request_id;
        let model = ctx.model.clone();
        let usage = response.usage;
        let events = vec![
            Ok(StreamEvent::ResponseStart { request_id, model }),
            Ok(StreamEvent::ContentDelta { index: 0, text }),
            Ok(StreamEvent::ResponseEnd {
                finish_reason: FinishReason::Stop,
                usage,
            }),
        ];
        Ok(futures::stream::iter(events).boxed())
    }
}

fn urlencoding_minimal(s: &str) -> String {
    // Keys are typically URL-safe; avoid pulling another dep for MVP.
    s.replace(' ', "%20")
}

fn translate(req: &ChatRequest) -> Result<Value> {
    let mut contents = Vec::new();
    let mut system_instruction = None;
    for m in &req.messages {
        match m.role {
            Role::System | Role::Developer => {
                let text = m
                    .content
                    .iter()
                    .filter_map(|p| match p {
                        ContentPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                system_instruction = Some(json!({"parts": [{"text": text}]}));
            }
            Role::User | Role::Assistant | Role::Tool => {
                let role = if m.role == Role::Assistant {
                    "model"
                } else {
                    "user"
                };
                let parts: Vec<Value> = m
                    .content
                    .iter()
                    .map(|p| match p {
                        ContentPart::Text { text } => json!({"text": text}),
                        ContentPart::ImageInline {
                            media_type,
                            data_base64,
                        } => json!({
                            "inline_data": {
                                "mime_type": media_type,
                                "data": data_base64,
                            }
                        }),
                        ContentPart::AttachmentRef { id } => {
                            json!({"text": format!("[attachment:{id}]")})
                        }
                    })
                    .collect();
                contents.push(json!({"role": role, "parts": parts}));
            }
        }
    }
    let mut body = json!({ "contents": contents });
    if let Some(sys) = system_instruction {
        body["system_instruction"] = sys;
    }
    if let Some(t) = req.settings.temperature {
        body["generationConfig"] = json!({"temperature": t});
    }
    Ok(body)
}

fn parse_response(text: &str, request: &ChatRequest, model: &str) -> Result<ChatResponse> {
    let v: Value = serde_json::from_str(text).map_err(|_| DarialError::UpstreamMalformed)?;
    let mut content = String::new();
    if let Some(parts) = v["candidates"][0]["content"]["parts"].as_array() {
        for p in parts {
            if let Some(t) = p["text"].as_str() {
                content.push_str(t);
            }
        }
    }
    Ok(ChatResponse {
        request_id: request.request_id,
        provider_request_id: None,
        model: model.to_string(),
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
            input_tokens: v["usageMetadata"]["promptTokenCount"].as_u64().unwrap_or(0) as u32,
            output_tokens: v["usageMetadata"]["candidatesTokenCount"]
                .as_u64()
                .unwrap_or(0) as u32,
        },
    })
}

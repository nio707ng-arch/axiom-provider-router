//! Built-in mock provider for offline demos and tests.

use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use futures::StreamExt;

use crate::canonical_ir::{
    AssistantMessage, ChatRequest, ChatResponse, Choice, ContentPart, FinishReason,
    ModelCapabilities, Role, StreamEvent, Usage,
};
use crate::error::Result;
use crate::providers::{ProviderAdapter, ProviderContext};

/// Deterministic echo-style mock.
#[derive(Debug, Default, Clone)]
pub struct MockProvider;

#[async_trait]
impl ProviderAdapter for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
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
        let user_text = last_user_text(request).unwrap_or_else(|| "(empty)".into());
        let reply = format!("mock[{}]: {user_text}", ctx.model);
        Ok(ChatResponse {
            request_id: request.request_id,
            provider_request_id: Some("mock-req".into()),
            model: ctx.model.clone(),
            choices: vec![Choice {
                index: 0,
                message: AssistantMessage {
                    role: Role::Assistant,
                    content: vec![ContentPart::Text { text: reply }],
                    tool_calls: vec![],
                },
                finish_reason: FinishReason::Stop,
            }],
            usage: Usage {
                input_tokens: 8,
                output_tokens: 8,
            },
        })
    }

    async fn stream(
        &self,
        ctx: &ProviderContext,
        request: &ChatRequest,
    ) -> Result<BoxStream<'static, Result<StreamEvent>>> {
        let user_text = last_user_text(request).unwrap_or_else(|| "(empty)".into());
        let reply = format!("mock[{}]: {user_text}", ctx.model);
        let request_id = request.request_id;
        let model = ctx.model.clone();
        let events = vec![
            Ok(StreamEvent::ResponseStart { request_id, model }),
            Ok(StreamEvent::ContentDelta {
                index: 0,
                text: reply,
            }),
            Ok(StreamEvent::ResponseEnd {
                finish_reason: FinishReason::Stop,
                usage: Usage {
                    input_tokens: 8,
                    output_tokens: 8,
                },
            }),
        ];
        Ok(stream::iter(events).boxed())
    }
}

fn last_user_text(req: &ChatRequest) -> Option<String> {
    for msg in req.messages.iter().rev() {
        if msg.role == Role::User {
            for part in &msg.content {
                if let ContentPart::Text { text } = part {
                    return Some(text.clone());
                }
            }
        }
    }
    None
}

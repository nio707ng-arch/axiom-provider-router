//! Provider-neutral canonical intermediate representation (IR v1).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::PrivacyProfile;
use crate::error::{DarialError, Result};

/// Current IR major version.
pub const IR_VERSION: u32 = 1;

/// Canonical chat / generation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// IR version.
    pub ir_version: u32,
    /// Request id.
    pub request_id: Uuid,
    /// Idempotency key.
    pub idempotency_key: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Privacy profile.
    #[serde(default)]
    pub privacy_profile: PrivacyProfile,
    /// Pseudonymous tenant / client id.
    #[serde(default)]
    pub tenant_id: String,
    /// Logical model alias.
    pub model: String,
    /// Messages.
    pub messages: Vec<Message>,
    /// Tools.
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    /// Tool choice.
    #[serde(default)]
    pub tool_choice: ToolChoice,
    /// Generation settings.
    #[serde(default)]
    pub settings: GenerationSettings,
    /// Stream preference.
    #[serde(default)]
    pub stream: bool,
    /// Deadline.
    pub deadline: Option<DateTime<Utc>>,
}

/// Message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role.
    pub role: Role,
    /// Content parts.
    pub content: Vec<ContentPart>,
    /// Tool call id when role is tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Name for tool messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// System.
    System,
    /// Developer (mapped like system for most providers).
    Developer,
    /// User.
    User,
    /// Assistant.
    Assistant,
    /// Tool result.
    Tool,
}

/// Content part.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Text.
    Text {
        /// Text body.
        text: String,
    },
    /// Inline image (bounded base64).
    ImageInline {
        /// Media type.
        media_type: String,
        /// Base64 data.
        data_base64: String,
    },
    /// Opaque attachment reference (no arbitrary URL fetch in MVP).
    AttachmentRef {
        /// Opaque id.
        id: String,
    },
}

/// Tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,
    /// Description.
    #[serde(default)]
    pub description: String,
    /// JSON Schema parameters object.
    pub parameters: serde_json::Value,
}

/// Tool choice policy.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// Auto.
    #[default]
    Auto,
    /// None.
    None,
    /// Required.
    Required,
    /// Named tool.
    Named {
        /// Name.
        name: String,
    },
}

/// Generation settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerationSettings {
    /// Temperature.
    pub temperature: Option<f32>,
    /// Top-p.
    pub top_p: Option<f32>,
    /// Max output tokens.
    pub max_output_tokens: Option<u32>,
    /// Stop sequences.
    #[serde(default)]
    pub stop: Vec<String>,
    /// Seed.
    pub seed: Option<i64>,
    /// Response format hint.
    pub response_format: Option<String>,
}

/// Canonical response (non-streaming).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Request id.
    pub request_id: Uuid,
    /// Provider request id if any.
    pub provider_request_id: Option<String>,
    /// Model used.
    pub model: String,
    /// Outputs.
    pub choices: Vec<Choice>,
    /// Usage.
    pub usage: Usage,
}

/// One choice / output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    /// Index.
    pub index: u32,
    /// Message.
    pub message: AssistantMessage,
    /// Finish reason.
    pub finish_reason: FinishReason,
}

/// Assistant message body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    /// Role always assistant.
    pub role: Role,
    /// Content parts.
    pub content: Vec<ContentPart>,
    /// Tool calls.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

/// Tool call from model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Stable id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Arguments JSON object (complete).
    pub arguments: serde_json::Value,
}

/// Finish reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Natural stop.
    Stop,
    /// Length.
    Length,
    /// Tool calls.
    ToolCalls,
    /// Content filter / refusal.
    ContentFilter,
    /// Other.
    Other,
}

/// Token usage.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    /// Input tokens.
    pub input_tokens: u32,
    /// Output tokens.
    pub output_tokens: u32,
}

/// Streaming event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Stream started.
    ResponseStart {
        /// Request id.
        request_id: Uuid,
        /// Model.
        model: String,
    },
    /// Text delta.
    ContentDelta {
        /// Index.
        index: u32,
        /// Text.
        text: String,
    },
    /// Tool call start.
    ToolCallStart {
        /// Index.
        index: u32,
        /// Id.
        id: String,
        /// Name.
        name: String,
    },
    /// Tool call arguments delta (raw JSON fragment).
    ToolCallDelta {
        /// Index.
        index: u32,
        /// Id.
        id: String,
        /// Arguments fragment.
        arguments_delta: String,
    },
    /// Tool call end.
    ToolCallEnd {
        /// Index.
        index: u32,
        /// Id.
        id: String,
    },
    /// Usage update.
    UsageUpdate {
        /// Usage.
        usage: Usage,
    },
    /// Heartbeat (no sensitive content).
    Heartbeat,
    /// Refusal.
    Refusal {
        /// Safe message.
        message: String,
    },
    /// Error terminal.
    Error {
        /// Code.
        code: String,
        /// Message.
        message: String,
    },
    /// Successful end.
    ResponseEnd {
        /// Finish reason.
        finish_reason: FinishReason,
        /// Usage.
        usage: Usage,
    },
}

/// Capability advertisement for a model route.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCapabilities {
    /// Streaming.
    pub streaming: bool,
    /// Tools.
    pub tools: bool,
    /// Vision / images.
    pub vision: bool,
    /// JSON mode.
    pub json_mode: bool,
    /// Seed.
    pub seed: bool,
}

/// Validate request against limits.
pub fn validate_request(req: &ChatRequest, max_messages: usize, max_tools: usize) -> Result<()> {
    if req.ir_version != IR_VERSION {
        return Err(DarialError::InvalidRequest(format!(
            "unsupported ir_version {}",
            req.ir_version
        )));
    }
    if req.idempotency_key.is_empty() || req.idempotency_key.len() > 128 {
        return Err(DarialError::InvalidRequest(
            "idempotency_key length invalid".into(),
        ));
    }
    if req.messages.is_empty() {
        return Err(DarialError::InvalidRequest("messages empty".into()));
    }
    if req.messages.len() > max_messages {
        return Err(DarialError::InvalidRequest("too many messages".into()));
    }
    if req.tools.len() > max_tools {
        return Err(DarialError::InvalidRequest("too many tools".into()));
    }
    if req.model.is_empty() {
        return Err(DarialError::InvalidRequest("model required".into()));
    }
    for msg in &req.messages {
        for part in &msg.content {
            if let ContentPart::Text { text } = part {
                if text.len() > 256 * 1024 {
                    return Err(DarialError::InvalidRequest("text part too large".into()));
                }
            }
            if let ContentPart::ImageInline { data_base64, .. } = part {
                if data_base64.len() > 14 * 1024 * 1024 {
                    return Err(DarialError::InvalidRequest("image too large".into()));
                }
            }
        }
    }
    for tool in &req.tools {
        validate_schema_depth(&tool.parameters, 0, 8)?;
    }
    Ok(())
}

fn validate_schema_depth(value: &serde_json::Value, depth: usize, max: usize) -> Result<()> {
    if depth > max {
        return Err(DarialError::InvalidRequest(
            "tool schema too deeply nested".into(),
        ));
    }
    match value {
        serde_json::Value::Object(map) => {
            for v in map.values() {
                validate_schema_depth(v, depth + 1, max)?;
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                validate_schema_depth(v, depth + 1, max)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Build a request shell with fresh ids.
#[must_use]
pub fn new_request_shell(model: impl Into<String>, messages: Vec<Message>) -> ChatRequest {
    ChatRequest {
        ir_version: IR_VERSION,
        request_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4().to_string(),
        created_at: Utc::now(),
        privacy_profile: PrivacyProfile::Direct,
        tenant_id: "default".into(),
        model: model.into(),
        messages,
        tools: vec![],
        tool_choice: ToolChoice::Auto,
        settings: GenerationSettings::default(),
        stream: false,
        deadline: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_empty_messages() {
        let mut req = new_request_shell("m", vec![]);
        req.messages.clear();
        assert!(validate_request(&req, 128, 32).is_err());
    }

    #[test]
    fn round_trip_json() {
        let req = new_request_shell(
            "darial-mock",
            vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text { text: "hi".into() }],
                tool_call_id: None,
                name: None,
            }],
        );
        let s = serde_json::to_string(&req).unwrap();
        let back: ChatRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.model, "darial-mock");
    }
}

//! Privacy-safe telemetry helpers.
//!
//! Content fields (prompts, completions, tool args, attachments, raw envelopes,
//! Authorization headers) MUST NEVER be recorded through these helpers.

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Field classification for redaction discipline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldClass {
    /// Safe to log (ids, status classes, durations).
    Allowed,
    /// Must be redacted or omitted.
    Forbidden,
}

/// Classify a logical field name.
#[must_use]
pub fn classify_field(name: &str) -> FieldClass {
    let lowered = name.to_ascii_lowercase();
    const FORBIDDEN: &[&str] = &[
        "prompt",
        "completion",
        "content",
        "message",
        "messages",
        "tool_arguments",
        "tool_result",
        "attachment",
        "filename",
        "authorization",
        "api_key",
        "private_key",
        "cookie",
        "password",
        "token",
        "envelope",
        "ciphertext",
        "user_agent",
        "query",
        "body",
    ];
    if FORBIDDEN.iter().any(|f| lowered.contains(f)) {
        FieldClass::Forbidden
    } else {
        FieldClass::Allowed
    }
}

/// Initialize tracing from `DARIAL_LOG` / `RUST_LOG`.
pub fn init_tracing(json: bool) {
    let filter = EnvFilter::try_from_env("DARIAL_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"));

    if json {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().json().flatten_event(true))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().compact())
            .init();
    }
}

/// Redact a string if it might contain secrets (conservative).
#[must_use]
pub fn redact_secret_like(value: &str) -> String {
    if value.len() > 8 {
        format!("{}…[redacted]", &value[..4])
    } else {
        "[redacted]".to_string()
    }
}

/// Assert that a log buffer does not contain a canary secret (for tests).
pub fn assert_no_canary(log: &str, canary: &str) {
    assert!(
        !log.contains(canary),
        "canary secret leaked into telemetry output"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_forbidden_fields() {
        assert_eq!(classify_field("prompt_text"), FieldClass::Forbidden);
        assert_eq!(classify_field("authorization"), FieldClass::Forbidden);
        assert_eq!(classify_field("request_id"), FieldClass::Allowed);
        assert_eq!(classify_field("model_alias"), FieldClass::Allowed);
    }

    #[test]
    fn canary_detection() {
        assert_no_canary(
            "status=200 model=darial-mock",
            "sk-CANARY-SECRET-DO-NOT-LOG",
        );
    }

    #[test]
    #[should_panic]
    fn canary_panics_on_leak() {
        assert_no_canary(
            "key=sk-CANARY-SECRET-DO-NOT-LOG",
            "sk-CANARY-SECRET-DO-NOT-LOG",
        );
    }
}

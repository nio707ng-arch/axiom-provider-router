//! Deterministic local DLP: detect and redact/block sensitive patterns.

use regex::Regex;
use serde::Serialize;

use crate::canonical_ir::{ChatRequest, ContentPart};
use crate::config::DlpConfig;
use crate::error::{DarialError, Result};

/// DLP action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlpAction {
    /// Allow.
    Allow,
    /// Warn (allow but record finding category).
    Warn,
    /// Redact matched spans.
    Redact,
    /// Block request.
    Block,
}

impl DlpAction {
    fn parse(s: &str) -> Self {
        match s {
            "allow" => Self::Allow,
            "warn" => Self::Warn,
            "redact" => Self::Redact,
            _ => Self::Block,
        }
    }
}

/// Finding without including the secret itself.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// Category.
    pub category: String,
    /// Message index.
    pub message_index: usize,
    /// Part index.
    pub part_index: usize,
    /// Start offset in text.
    pub start: usize,
    /// End offset.
    pub end: usize,
}

/// Pipeline result.
#[derive(Debug)]
pub struct DlpOutcome {
    /// Possibly modified request.
    pub request: ChatRequest,
    /// Findings (locations only).
    pub findings: Vec<Finding>,
    /// Effective action taken.
    pub action: DlpAction,
}

struct Rule {
    re: Regex,
    category: String,
    action: DlpAction,
}

/// Compiled DLP engine.
pub struct DlpEngine {
    rules: Vec<Rule>,
    default_action: DlpAction,
    enabled: bool,
}

impl DlpEngine {
    /// Build from config.
    pub fn from_config(cfg: &DlpConfig) -> Result<Self> {
        let mut rules = builtin_rules();
        for c in &cfg.custom_rules {
            let re = Regex::new(&c.pattern)
                .map_err(|e| DarialError::Config(format!("dlp regex: {e}")))?;
            rules.push(Rule {
                re,
                category: c.category.clone(),
                action: DlpAction::parse(&c.action),
            });
        }
        Ok(Self {
            rules,
            default_action: DlpAction::parse(&cfg.default_action),
            enabled: cfg.enabled,
        })
    }

    /// Scan and apply policy.
    pub fn apply(&self, mut request: ChatRequest) -> Result<DlpOutcome> {
        if !self.enabled {
            return Ok(DlpOutcome {
                request,
                findings: vec![],
                action: DlpAction::Allow,
            });
        }
        let mut findings = Vec::new();
        let mut strongest = DlpAction::Allow;
        for (mi, msg) in request.messages.iter_mut().enumerate() {
            for (pi, part) in msg.content.iter_mut().enumerate() {
                if let ContentPart::Text { text } = part {
                    for rule in &self.rules {
                        for mat in rule.re.find_iter(text) {
                            findings.push(Finding {
                                category: rule.category.clone(),
                                message_index: mi,
                                part_index: pi,
                                start: mat.start(),
                                end: mat.end(),
                            });
                            strongest = stronger(strongest, rule.action);
                        }
                    }
                    if strongest == DlpAction::Redact || self.default_action == DlpAction::Redact {
                        let mut redacted = text.clone();
                        for rule in &self.rules {
                            redacted = rule.re.replace_all(&redacted, "[REDACTED]").into_owned();
                        }
                        *text = redacted;
                        if strongest == DlpAction::Allow {
                            strongest = DlpAction::Redact;
                        }
                    }
                }
            }
        }
        if !findings.is_empty() && strongest == DlpAction::Allow {
            strongest = self.default_action;
        }
        if strongest == DlpAction::Block {
            // Do not include matched secret in error.
            return Err(DarialError::PolicyDenied(format!(
                "dlp blocked {} finding(s)",
                findings.len()
            )));
        }
        Ok(DlpOutcome {
            request,
            findings,
            action: strongest,
        })
    }
}

fn stronger(a: DlpAction, b: DlpAction) -> DlpAction {
    use DlpAction::*;
    let rank = |x: DlpAction| match x {
        Allow => 0,
        Warn => 1,
        Redact => 2,
        Block => 3,
    };
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

fn builtin_rules() -> Vec<Rule> {
    let patterns = [
        (
            r"(?i)\b(sk-[A-Za-z0-9_\-]{20,})\b",
            "api_token",
            DlpAction::Block,
        ),
        (
            r"(?i)\b(ghp_[A-Za-z0-9]{20,})\b",
            "github_token",
            DlpAction::Block,
        ),
        (
            r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
            "private_key",
            DlpAction::Block,
        ),
        (
            r"(?i)\b[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}\b",
            "email",
            DlpAction::Warn,
        ),
        (
            r"(?i)password\s*=\s*\S+",
            "password_assignment",
            DlpAction::Block,
        ),
    ];
    patterns
        .into_iter()
        .filter_map(|(p, cat, action)| {
            Regex::new(p).ok().map(|re| Rule {
                re,
                category: cat.into(),
                action,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical_ir::{new_request_shell, Message, Role};

    #[test]
    fn blocks_api_token_without_echoing_secret() {
        let eng = DlpEngine::from_config(&DlpConfig::default()).unwrap();
        let secret = "sk-CANARY-SECRET-DO-NOT-LOG-abcdefgh";
        let req = new_request_shell(
            "m",
            vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text {
                    text: format!("my key is {secret}"),
                }],
                tool_call_id: None,
                name: None,
            }],
        );
        let err = eng.apply(req).unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains(secret));
        assert!(matches!(err, DarialError::PolicyDenied(_)));
    }
}

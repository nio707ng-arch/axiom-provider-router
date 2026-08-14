//! Authorization, routing, limits, and DLP decision helpers.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::canonical_ir::ChatRequest;
use crate::config::{Config, ModelAlias};
use crate::error::{DarialError, Result};

/// Outcome of policy evaluation before provider dispatch.
#[derive(Debug, Clone)]
pub struct PolicyDecision {
    /// Resolved provider name.
    pub provider: String,
    /// Upstream model id.
    pub upstream_model: String,
    /// Tenant id.
    pub tenant_id: String,
}

/// Simple in-memory rate limiter (per-process MVP).
#[derive(Debug, Default)]
pub struct RateLimiter {
    inner: Mutex<HashMap<String, Window>>,
}

#[derive(Debug)]
struct Window {
    start: Instant,
    count: u32,
}

impl RateLimiter {
    /// Check and increment; errors with RateLimited.
    pub fn check(&self, key: &str, limit_per_minute: u32) -> Result<()> {
        let mut guard = self.inner.lock().map_err(|_| DarialError::Internal)?;
        let now = Instant::now();
        let entry = guard.entry(key.to_string()).or_insert_with(|| Window {
            start: now,
            count: 0,
        });
        if now.duration_since(entry.start) > Duration::from_secs(60) {
            entry.start = now;
            entry.count = 0;
        }
        if entry.count >= limit_per_minute {
            return Err(DarialError::RateLimited);
        }
        entry.count += 1;
        Ok(())
    }
}

/// Bounded replay cache for idempotency keys.
#[derive(Debug, Default)]
pub struct ReplayCache {
    inner: Mutex<HashMap<String, Instant>>,
    ttl: Duration,
    max_entries: usize,
}

impl ReplayCache {
    /// Create with TTL and capacity.
    #[must_use]
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            ttl,
            max_entries,
        }
    }

    /// Insert key; Err if duplicate still valid.
    pub fn check_and_insert(&self, key: &str) -> Result<()> {
        let mut guard = self.inner.lock().map_err(|_| DarialError::Internal)?;
        let now = Instant::now();
        guard.retain(|_, t| now.duration_since(*t) < self.ttl);
        if guard.contains_key(key) {
            return Err(DarialError::InvalidRequest("replay detected".into()));
        }
        if guard.len() >= self.max_entries {
            // Load shed oldest by clearing half (simple MVP).
            let keys: Vec<_> = guard.keys().take(self.max_entries / 2).cloned().collect();
            for k in keys {
                guard.remove(&k);
            }
        }
        guard.insert(key.to_string(), now);
        Ok(())
    }
}

/// Resolve model alias and apply basic request policy.
pub fn authorize_and_route(
    cfg: &Config,
    req: &ChatRequest,
    tenant_id: &str,
) -> Result<PolicyDecision> {
    let alias: &ModelAlias = cfg
        .model_aliases
        .get(&req.model)
        .ok_or_else(|| DarialError::PolicyDenied(format!("model {} not allowed", req.model)))?;
    let provider = cfg
        .providers
        .get(&alias.provider)
        .ok_or_else(|| DarialError::PolicyDenied("provider missing".into()))?;
    if !provider.enabled {
        return Err(DarialError::PolicyDenied("provider disabled".into()));
    }
    if !provider.models.is_empty() && !provider.models.contains(&alias.model) {
        return Err(DarialError::PolicyDenied(
            "upstream model not allowlisted".into(),
        ));
    }
    // Clock skew / freshness for wire requests.
    let skew = chrono::Utc::now() - req.created_at;
    if skew.num_seconds().abs() > 120 {
        return Err(DarialError::InvalidRequest(
            "created_at outside validity window".into(),
        ));
    }
    Ok(PolicyDecision {
        provider: alias.provider.clone(),
        upstream_model: alias.model.clone(),
        tenant_id: tenant_id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical_ir::{new_request_shell, ContentPart, Message, Role};
    use crate::config::{ModelAlias, ProviderConfig};

    fn cfg_with_mock() -> Config {
        let mut cfg = Config::parse_toml("role = \"gateway\"").unwrap();
        cfg.providers.insert(
            "mock".into(),
            ProviderConfig {
                enabled: true,
                kind: "mock".into(),
                base_url: None,
                api_key_env: None,
                models: vec!["mock-1".into()],
            },
        );
        cfg.model_aliases.insert(
            "darial-mock".into(),
            ModelAlias {
                provider: "mock".into(),
                model: "mock-1".into(),
            },
        );
        cfg
    }

    #[test]
    fn routes_known_model() {
        let cfg = cfg_with_mock();
        let req = new_request_shell(
            "darial-mock",
            vec![Message {
                role: Role::User,
                content: vec![ContentPart::Text { text: "hi".into() }],
                tool_call_id: None,
                name: None,
            }],
        );
        let d = authorize_and_route(&cfg, &req, "t1").unwrap();
        assert_eq!(d.provider, "mock");
        assert_eq!(d.upstream_model, "mock-1");
    }

    #[test]
    fn replay_cache_detects_duplicate() {
        let c = ReplayCache::new(Duration::from_secs(60), 100);
        c.check_and_insert("k").unwrap();
        assert!(c.check_and_insert("k").is_err());
    }
}

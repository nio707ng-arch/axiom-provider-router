//! Typed layered configuration with validation and fail-closed defaults.

use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{DarialError, Result};

/// Deployment profile affecting validation strictness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    /// Local development; allows mock provider and optional loopback auth skip.
    #[default]
    Dev,
    /// Production; refuses unsafe settings.
    Production,
}

/// Privacy profile selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyProfile {
    /// Agent talks directly to gateway.
    #[default]
    Direct,
    /// Agent → relay → gateway with HPKE envelopes.
    PrivateBuffered,
}

/// Role-specific root config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Deployment profile.
    #[serde(default)]
    pub profile: Profile,
    /// Role hint for validation (`agent`, `gateway`, `relay`).
    #[serde(default)]
    pub role: String,
    /// Listen settings.
    #[serde(default)]
    pub listen: ListenConfig,
    /// Auth settings.
    #[serde(default)]
    pub auth: AuthConfig,
    /// Upstream gateway (agent).
    #[serde(default)]
    pub gateway: GatewayClientConfig,
    /// Relay settings (agent private mode / relay role).
    #[serde(default)]
    pub relay: RelayConfig,
    /// Provider map (gateway).
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    /// Model aliases logical → provider/model.
    #[serde(default)]
    pub model_aliases: HashMap<String, ModelAlias>,
    /// Limits.
    #[serde(default)]
    pub limits: LimitsConfig,
    /// DLP.
    #[serde(default)]
    pub dlp: DlpConfig,
    /// Telemetry.
    #[serde(default)]
    pub telemetry: TelemetryConfig,
    /// Privacy / HPKE.
    #[serde(default)]
    pub privacy: PrivacyConfig,
    /// TLS trust (additional enterprise or Caddy local CA roots).
    #[serde(default)]
    pub tls: TlsConfig,
}

/// Listen address configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListenConfig {
    /// Bind address.
    #[serde(default = "default_agent_bind")]
    pub bind: String,
    /// Allow non-loopback binds for the agent (high friction).
    #[serde(default)]
    pub allow_non_loopback: bool,
    /// TLS cert path.
    pub tls_cert: Option<PathBuf>,
    /// TLS key path.
    pub tls_key: Option<PathBuf>,
}

fn default_agent_bind() -> String {
    "127.0.0.1:8080".into()
}

impl Default for ListenConfig {
    fn default() -> Self {
        Self {
            bind: default_agent_bind(),
            allow_non_loopback: false,
            tls_cert: None,
            tls_key: None,
        }
    }
}

/// Authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AuthConfig {
    /// Require bearer token on local agent (recommended).
    #[serde(default = "default_true")]
    pub require_local_auth: bool,
    /// Env var containing local/agent bearer token (plaintext at rest avoided; env for MVP).
    pub local_token_env: Option<String>,
    /// SHA-256 hex hashes of accepted gateway tokens (gateway role).
    #[serde(default)]
    pub token_sha256: Vec<String>,
    /// Env var whose value is hashed and compared (dev convenience).
    pub token_env: Option<String>,
    /// Default tenant id for single-tenant deploys.
    #[serde(default = "default_tenant")]
    pub default_tenant: String,
}

fn default_true() -> bool {
    true
}

fn default_tenant() -> String {
    "default".into()
}

/// Agent → gateway client settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayClientConfig {
    /// Gateway base URL.
    #[serde(default = "default_gateway_url")]
    pub url: String,
    /// Env var for bearer token.
    pub auth_token_env: Option<String>,
    /// Privacy profile.
    #[serde(default)]
    pub privacy_profile: PrivacyProfile,
}

fn default_gateway_url() -> String {
    "http://127.0.0.1:8443".into()
}

impl Default for GatewayClientConfig {
    fn default() -> Self {
        Self {
            url: default_gateway_url(),
            auth_token_env: None,
            privacy_profile: PrivacyProfile::Direct,
        }
    }
}

/// Relay configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayConfig {
    /// Relay URL for agent private mode.
    pub url: Option<String>,
    /// Gateway URL the relay forwards to (relay role) — admin configured only.
    pub gateway_url: Option<String>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            url: None,
            gateway_url: None,
        }
    }
}

/// Per-provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    /// Enable this provider.
    #[serde(default)]
    pub enabled: bool,
    /// Kind: mock | openai | anthropic | gemini.
    pub kind: String,
    /// Administrator-configured base URL.
    pub base_url: Option<String>,
    /// Env var holding API key.
    pub api_key_env: Option<String>,
    /// Allowlisted provider model ids.
    #[serde(default)]
    pub models: Vec<String>,
}

/// Logical model alias.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelAlias {
    /// Provider name in `providers` map.
    pub provider: String,
    /// Upstream model id.
    pub model: String,
}

/// Resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitsConfig {
    /// Max request body bytes.
    #[serde(default = "default_max_body")]
    pub max_body_bytes: usize,
    /// Max messages per request.
    #[serde(default = "default_max_messages")]
    pub max_messages: usize,
    /// Max tools.
    #[serde(default = "default_max_tools")]
    pub max_tools: usize,
    /// Max concurrent requests per tenant.
    #[serde(default = "default_max_conc")]
    pub max_concurrency: usize,
    /// Request timeout seconds.
    #[serde(default = "default_timeout")]
    pub request_timeout_secs: u64,
    /// Max stream duration seconds.
    #[serde(default = "default_stream_secs")]
    pub max_stream_secs: u64,
    /// Requests per minute per tenant.
    #[serde(default = "default_rpm")]
    pub requests_per_minute: u32,
    /// Max attachment bytes.
    #[serde(default = "default_max_attach")]
    pub max_attachment_bytes: usize,
}

fn default_max_body() -> usize {
    4 * 1024 * 1024
}
fn default_max_messages() -> usize {
    128
}
fn default_max_tools() -> usize {
    32
}
fn default_max_conc() -> usize {
    32
}
fn default_timeout() -> u64 {
    120
}
fn default_stream_secs() -> u64 {
    300
}
fn default_rpm() -> u32 {
    120
}
fn default_max_attach() -> usize {
    10 * 1024 * 1024
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: default_max_body(),
            max_messages: default_max_messages(),
            max_tools: default_max_tools(),
            max_concurrency: default_max_conc(),
            request_timeout_secs: default_timeout(),
            max_stream_secs: default_stream_secs(),
            requests_per_minute: default_rpm(),
            max_attachment_bytes: default_max_attach(),
        }
    }
}

/// DLP configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DlpConfig {
    /// Enable DLP pipeline.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Default action: allow | warn | redact | block.
    #[serde(default = "default_dlp_action")]
    pub default_action: String,
    /// Extra regex rules (pattern, category, action).
    #[serde(default)]
    pub custom_rules: Vec<CustomDlpRule>,
}

fn default_dlp_action() -> String {
    "block".into()
}

impl Default for DlpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_action: default_dlp_action(),
            custom_rules: Vec::new(),
        }
    }
}

/// Custom DLP rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomDlpRule {
    /// Regex pattern.
    pub pattern: String,
    /// Category label.
    pub category: String,
    /// Action override.
    pub action: String,
}

/// Telemetry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryConfig {
    /// JSON logs.
    #[serde(default)]
    pub json_logs: bool,
    /// Optional Prometheus listen address.
    pub metrics_bind: Option<String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            json_logs: false,
            metrics_bind: None,
        }
    }
}

/// TLS client trust configuration. Certificate verification is never disabled.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TlsConfig {
    /// Extra PEM root CA file (corporate PKI or Caddy `tls internal`).
    pub additional_roots: Option<PathBuf>,
}

/// Privacy / HPKE key configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PrivacyConfig {
    /// Directory containing HPKE keys.
    pub key_dir: Option<PathBuf>,
    /// Enable experimental private streaming (must also be compiled in).
    #[serde(default)]
    pub experimental_private_streaming: bool,
    /// Max padding bytes (0 = disabled).
    #[serde(default)]
    pub max_padding_bytes: usize,
}

impl Config {
    /// Load from TOML file.
    pub fn load_file(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .map_err(|e| DarialError::Config(format!("read {}: {e}", path.display())))?;
        Self::parse_toml(&text)
    }

    /// Parse TOML.
    pub fn parse_toml(text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|e| DarialError::Config(format!("toml: {e}")))
    }

    /// Validate for a role.
    pub fn validate(&self, role: &str) -> Result<()> {
        match role {
            "agent" => self.validate_agent(),
            "gateway" => self.validate_gateway(),
            "relay" => self.validate_relay(),
            other => Err(DarialError::Config(format!("unknown role {other}"))),
        }
    }

    fn validate_agent(&self) -> Result<()> {
        let addr: SocketAddr = self
            .listen
            .bind
            .parse()
            .map_err(|e| DarialError::Config(format!("invalid bind: {e}")))?;
        if !addr.ip().is_loopback() {
            if !self.listen.allow_non_loopback {
                return Err(DarialError::Config(
                    "agent bind is not loopback; set listen.allow_non_loopback = true to opt in (dangerous)"
                        .into(),
                ));
            }
            if self.profile == Profile::Production {
                return Err(DarialError::Config(
                    "production profile refuses non-loopback agent bind".into(),
                ));
            }
            tracing::warn!(
                "SECURITY WARNING: agent binding to non-loopback address {}; this exposes the local API beyond this host",
                addr
            );
        }
        if self.profile == Profile::Production && !self.auth.require_local_auth {
            return Err(DarialError::Config(
                "production requires auth.require_local_auth = true".into(),
            ));
        }
        if self.gateway.privacy_profile == PrivacyProfile::PrivateBuffered
            && self.relay.url.is_none()
        {
            return Err(DarialError::Config(
                "private_buffered requires relay.url".into(),
            ));
        }
        if self.privacy.experimental_private_streaming {
            return Err(DarialError::Config(
                "experimental_private_streaming is not enabled in this build".into(),
            ));
        }
        Ok(())
    }

    fn validate_gateway(&self) -> Result<()> {
        if self.providers.is_empty() {
            return Err(DarialError::Config(
                "gateway requires at least one provider".into(),
            ));
        }
        let any_enabled = self.providers.values().any(|p| p.enabled);
        if !any_enabled {
            return Err(DarialError::Config("no provider enabled".into()));
        }
        for (name, p) in &self.providers {
            if p.enabled && p.kind != "mock" && p.base_url.is_none() {
                return Err(DarialError::Config(format!(
                    "provider {name} missing base_url"
                )));
            }
            if let Some(url) = &p.base_url {
                validate_provider_url(url, self.profile)?;
            }
        }
        if self.model_aliases.is_empty() {
            return Err(DarialError::Config("gateway requires model_aliases".into()));
        }
        for (alias, target) in &self.model_aliases {
            if !self.providers.contains_key(&target.provider) {
                return Err(DarialError::Config(format!(
                    "alias {alias} references unknown provider {}",
                    target.provider
                )));
            }
        }
        if self.profile == Profile::Production
            && self.auth.token_sha256.is_empty()
            && self.auth.token_env.is_none()
        {
            return Err(DarialError::Config(
                "production gateway requires auth tokens".into(),
            ));
        }
        Ok(())
    }

    fn validate_relay(&self) -> Result<()> {
        let Some(url) = &self.relay.gateway_url else {
            return Err(DarialError::Config(
                "relay requires relay.gateway_url (administrator configured)".into(),
            ));
        };
        if !(url.starts_with("https://")
            || (self.profile == Profile::Dev && url.starts_with("http://127.0.0.1")))
        {
            return Err(DarialError::Config(
                "relay.gateway_url must be https:// (or http://127.0.0.1 in dev)".into(),
            ));
        }
        Ok(())
    }

    /// HTTP client with safe defaults and optional extra TLS roots.
    ///
    /// Never disables certificate verification.
    pub fn http_client(&self) -> Result<reqwest::Client> {
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(
                self.limits.request_timeout_secs,
            ))
            .user_agent("darial/0.1");
        if let Some(path) = &self.tls.additional_roots {
            let pem = fs::read(path).map_err(|e| {
                DarialError::Config(format!("tls.additional_roots {}: {e}", path.display()))
            })?;
            let cert = reqwest::Certificate::from_pem(&pem)
                .map_err(|e| DarialError::Config(format!("tls.additional_roots parse: {e}")))?;
            builder = builder.add_root_certificate(cert);
        }
        builder.build().map_err(|_| DarialError::Internal)
    }

    /// Sanitized JSON for `--print-effective-config` (secrets redacted).
    pub fn effective_sanitized(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self).unwrap_or_default();
        if let Some(obj) = v.as_object_mut() {
            if let Some(auth) = obj.get_mut("auth").and_then(|a| a.as_object_mut()) {
                if auth.contains_key("token_sha256") {
                    auth.insert(
                        "token_sha256".into(),
                        serde_json::json!(["[redacted-hashes]"]),
                    );
                }
            }
        }
        v
    }
}

fn validate_provider_url(url: &str, profile: Profile) -> Result<()> {
    let parsed =
        reqwest::Url::parse(url).map_err(|e| DarialError::Config(format!("provider url: {e}")))?;
    match parsed.scheme() {
        "https" => {}
        "http" if profile == Profile::Dev => {
            let host = parsed.host_str().unwrap_or("");
            if host != "127.0.0.1" && host != "localhost" {
                return Err(DarialError::Config(
                    "http provider URLs only allowed to loopback in dev".into(),
                ));
            }
        }
        _ => {
            return Err(DarialError::Config(
                "provider base_url must use https".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_loopback_agent_without_opt_in() {
        let mut cfg = Config::parse_toml(
            r#"
            role = "agent"
            [listen]
            bind = "0.0.0.0:8080"
            "#,
        )
        .unwrap();
        cfg.role = "agent".into();
        assert!(cfg.validate("agent").is_err());
    }

    #[test]
    fn accepts_loopback_agent() {
        let cfg = Config::parse_toml(
            r#"
            role = "agent"
            [listen]
            bind = "127.0.0.1:8080"
            [gateway]
            url = "http://127.0.0.1:8443"
            "#,
        )
        .unwrap();
        cfg.validate("agent").unwrap();
    }

    #[test]
    fn parses_additional_tls_roots() {
        let cfg = Config::parse_toml(
            r#"
            role = "agent"
            [listen]
            bind = "0.0.0.0:8080"
            allow_non_loopback = true
            [gateway]
            url = "https://gateway.localhost"
            [tls]
            additional_roots = "/data/caddy/pki/authorities/local/root.crt"
            "#,
        )
        .unwrap();
        cfg.validate("agent").unwrap();
        assert!(cfg.tls.additional_roots.is_some());
    }
}

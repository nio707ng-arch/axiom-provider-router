# Configuration Reference

## Precedence (highest last wins)

1. Secure compiled defaults
2. Configuration file (`--config`)
3. Environment variables (`DARIAL_*`)
4. Command-line flags

Production profile MUST refuse contradictory or unsafe settings (e.g., non-loopback agent bind without `allow_non_loopback = true`, TLS verify disabled, empty provider allowlist with live routing enabled).

## Commands

```text
darial agent serve --config agent.toml
darial agent check-config --config agent.toml
darial agent print-effective-config --config agent.toml
darial gateway serve --config gateway.toml
darial gateway check-config --config gateway.toml
darial gateway key generate --output ./keys
darial gateway key publish --config gateway.toml
darial relay serve --config relay.toml
darial relay check-config --config relay.toml
```

`--print-effective-config` MUST redact secrets (tokens, keys, passwords).

## Common sections

### `[listen]`

| Key | Default | Notes |
|-----|---------|-------|
| `bind` | agent: `127.0.0.1:8080`; gateway/relay: `0.0.0.0:8443` | Agent non-loopback requires `allow_non_loopback` |
| `allow_non_loopback` | `false` | High-friction |
| `tls_cert` / `tls_key` | unset | Unused in 0.1 when Caddy (or another proxy) terminates TLS |

### `[tls]` (agent)

| Key | Notes |
|-----|-------|
| `additional_roots` | Optional PEM file added to the default WebPKI roots (Caddy local CA, corporate PKI). Never disables verification. |

### `[gateway]` / `[relay]` (agent)

| Key | Notes |
|-----|-------|
| `url` | HTTPS URL of gateway or relay |
| `auth_token_env` | Env var holding bearer token |
| `privacy_profile` | `direct` \| `private_buffered` |

### `[providers.<name>]` (gateway)

| Key | Notes |
|-----|-------|
| `enabled` | bool |
| `base_url` | Administrator-configured only |
| `api_key_env` | Env var name |
| `models` | Allowlisted model IDs |
| `aliases` | Logical name → provider model |

### `[auth]`

Token hashes, OIDC issuer/audience (enterprise), scopes.

### `[limits]`

Body size, concurrency, rate, stream duration, attachments.

### `[dlp]` (agent)

Rules, actions (`allow`/`warn`/`redact`/`block`), custom regex.

### `[telemetry]`

Metrics bind address, log format (`json`/`pretty`), privacy mode always on for content.

### `[privacy]`

Key paths, padding, experimental streaming flag (default false).

## Environment variables

| Variable | Purpose |
|----------|---------|
| `DARIAL_CONFIG` | Default config path |
| `DARIAL_LOG` | `tracing` filter |
| `DARIAL_GATEWAY_TOKEN` | Example token env |
| `DARIAL_OPENAI_API_KEY` | Provider key |
| `DARIAL_ANTHROPIC_API_KEY` | Provider key |
| `DARIAL_GEMINI_API_KEY` | Provider key |

## Validation

`check-config` loads, resolves, and validates without binding sockets. Exit codes: `0` ok, `2` config error, `3` auth/secret error, `1` internal.

# Architecture

## Mission

Darial provides approved applications with a provider-neutral LLM API over ordinary HTTPS while minimizing disclosure of user identity, prompt content, credentials, and operational metadata to parties that do not need them. It is an authorized gateway, not a circumvention tool.

## Components

Darial is a **single Cargo package** and a **single executable** (`darial`) that selects its role via subcommands.

| Component | Module | Responsibility |
|-----------|--------|----------------|
| Agent | `agent/` | Loopback OpenAI-compatible HTTP API; local DLP/policy; outbound to gateway or relay |
| Relay | `relay/` | Content-blind HTTPS forwarder of protected envelopes; learns client IP and sizes, not plaintext |
| Gateway | `gateway/` | Authentication, authorization, policy, IR orchestration, provider routing |
| Providers | `providers/` | Adapters for OpenAI, Anthropic, Gemini behind a stable trait |
| Protocol | `protocol.rs` | Versioned wire messages shared by agent/relay/gateway |
| Canonical IR | `canonical_ir.rs` | Provider-neutral domain model |
| Policy | `policy.rs` | Routing, limits, DLP decisions |
| Crypto envelope | `crypto_envelope.rs` | HPKE/OHTTP-inspired protected-message abstraction |
| Auth | `auth/` | Tokens, scopes, secret store |
| DLP | `dlp/` | Deterministic local detection/redaction |
| Config | `config.rs` | Typed layered configuration |
| Telemetry | `telemetry.rs` | Privacy-safe metrics and tracing |

## Data flows

### Direct mode

```text
Application --HTTP loopback--> Agent --TLS HTTPS--> Caddy (or other TLS terminator)
    --HTTP internal--> Gateway --TLS HTTPS--> Provider
```

The gateway process itself listens for HTTP. Production HTTPS is provided by Caddy (see `deploy/compose/`).

Trust assumptions:

- Application trusts the local agent process and its config.
- Agent trusts gateway authenticity via TLS + configured hostname + auth credentials.
- Gateway trusts its own provider allowlist and secrets store.
- Network observers see connections to the gateway hostname.

Visible plaintext: agent, gateway (in memory), provider. Not persisted by default.

### Private buffered mode

```text
Application --> Agent --protected envelope--> Relay --protected envelope--> Gateway --> Provider
```

Trust assumptions (non-collusion):

- Relay is honest-but-curious: follows protocol, may log metadata, must not obtain gateway private keys.
- Gateway is honest-but-curious: may see plaintext for inference, must not receive originating client IP from the relay path.
- Collusion between relay and gateway defeats unlinkability; this is an **explicit residual risk**, not a claimed protection.

### Trust boundaries

1. **Application ↔ Agent** — loopback only by default; optional local bearer token; no permissive CORS.
2. **Agent policy/DLP** — last chance to redact/block before egress.
3. **Agent ↔ Gateway (direct)** — mutual auth via TLS + bearer/OIDC; content visible to gateway.
4. **Agent ↔ Relay ↔ Gateway (private)** — relay sees ciphertext; gateway decrypts; IP separation.
5. **Gateway ↔ Provider** — allowlisted HTTPS only; provider keys never leave gateway.
6. **Config / secrets / telemetry / updates** — separate admin plane; fail closed on indeterminate policy.

## Deployment profiles

| Profile | Auth | Privacy | Typical use |
|---------|------|---------|-------------|
| Personal | Scoped gateway token + gateway-held provider keys | Direct or private buffered | Individual workstation |
| Enterprise | OIDC/workload identity, tenant isolation | Admin may disable private mode | Corporate deployment |
| Dev/mock | Local tokens, mock provider | Direct only | Offline CI and demos |

## Explicit non-goals (architecture)

- Transparent OS-wide proxy or VPN
- Arbitrary Internet tunneling
- Encrypted inference hiding content from the LLM provider
- Custom cryptographic primitives
- Multi-provider failover after streaming has begun

## Versioning

- Wire protocol: `Darial-Protocol` header / envelope version field; major bumps are incompatible.
- Canonical IR: `ir_version` field; see [protocol.md](protocol.md).
- Compatibility policy: gateways MUST reject unsupported major versions (fail closed / downgrade prevention).

## Related ADRs

- [0001-single-package.md](adr/0001-single-package.md)
- [0002-canonical-ir.md](adr/0002-canonical-ir.md)
- [0003-loopback-default.md](adr/0003-loopback-default.md)
- [0004-private-mode.md](adr/0004-private-mode.md)
- [0005-mvp-scope.md](adr/0005-mvp-scope.md)

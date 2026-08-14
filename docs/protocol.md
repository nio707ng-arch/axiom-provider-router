# Wire Protocol (Normative Skeleton)

Language: **MUST**, **SHOULD**, **MAY** as in RFC 2119.

Status: **Draft for Darial 0.1**. Private-mode framing details align with RFC 9180 (HPKE) and are inspired by RFC 9458 (Oblivious HTTP). Classic OHTTP encapsulation libraries may be substituted when interoperability vectors are verified (see ADR 0004).

## 1. Version negotiation

- Clients MUST set `Darial-Protocol: 1` on requests to gateway/relay.
- Servers MUST reject unknown major versions with error code `privacy_negotiation_failure` or `invalid_request`.
- Servers MUST NOT silently downgrade. A missing version header on production profiles MUST be rejected.
- Minor/patch extensions MAY be negotiated via `Darial-Capabilities` without changing the major version.

## 2. Content types

| Mode | Content-Type | Body |
|------|--------------|------|
| Direct | `application/json` | Canonical IR JSON (`ChatRequest` / streaming events) |
| Private | `application/darial-envelope+json` or binary encapsulations documented below | Encrypted envelope |

JSON is used for MVP interchange. Where cryptographic binding is required, associated authenticated data (AAD) MUST be constructed from a fixed field order defined in §6.

Maximum request body: configurable, default **4 MiB** (excluding attachments handled via separate upload).

## 3. Identifiers, time, replay

- Every request MUST include `request_id` (UUID v4) and `idempotency_key` (client-chosen opaque string ≤ 128 chars).
- Every authenticated request MUST include `created_at` (RFC 3339 UTC) within a validity window of **±120 seconds** clock skew by default.
- Nonces or HPKE ephemeral material MUST NOT be reused.
- Gateways MUST maintain a bounded replay cache keyed by `(tenant, idempotency_key)` and/or cryptographic request hash for the validity window.
- Clock-skew beyond the window MUST produce `invalid_request`.

## 4. Authentication binding

- Direct mode: `Authorization: Bearer <token>` bound to tenant scopes.
- Private mode: authorization material MUST be carried such that the relay cannot use it to obtain plaintext, and the gateway SHOULD avoid a stable long-lived client identifier that re-links IP (relay) with content (gateway). MVP uses short-lived gateway tokens presented only inside the protected envelope (weaker unlinkability; documented in privacy.md).
- Scopes cover models, providers, tools, attachments, privacy modes, and admin actions.

## 5. Endpoints

### Direct gateway

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/v1/darial/chat` | Non-streaming or buffered chat |
| POST | `/v1/darial/chat/stream` | SSE stream of canonical events |
| GET | `/v1/darial/models` | Logical models and capabilities |
| GET | `/healthz` | Liveness |
| GET | `/readyz` | Readiness |

### Relay (private)

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/v1/darial/relay` | Forward protected envelope to configured gateway |
| GET | `/healthz` | Liveness |

Relay MUST forward only to the administrator-configured gateway URL. Relay MUST NOT decrypt. Relay MUST strip hop-by-hop headers and MUST NOT forward arbitrary `Host` overrides from clients.

### Key discovery (private)

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/v1/darial/ohttp-keys` | Gateway HPKE public keys (JWKS-like document) |

Keys include `kid`, algorithm suite, `not_before`, `not_after`, and cache directives. Clients MUST respect cache TTLs and MUST fail closed if no valid key is available.

## 6. Private envelope (buffered)

Inspired by Oblivious HTTP request/response pairing with HPKE (RFC 9180):

- KEM: X25519 HKDF-SHA256
- KDF: HKDF-SHA256
- AEAD: AES-128-GCM (or ChaCha20-Poly1305 if configured)

AAD MUST bind: protocol version, direction (`request`|`response`), key id, gateway origin, and media type. Direction separation prevents response ciphertext from being accepted as a request.

Relay-visible metadata MUST be limited to: ciphertext length, content-type, key id (optional), and transport headers required for TLS. Prompt text, tools, attachments, and provider credentials MUST NOT appear in plaintext outside agent and gateway.

## 7. Streaming events (canonical)

SSE `data:` lines carry JSON events. Ordering MUST follow:

`response_start` → (`content_delta` | `tool_call_*` | `usage_update` | heartbeat)* → exactly one of (`response_end` | `error` | `refusal`)

Heartbeats MUST NOT carry sensitive content. After visible output begins, failover to another provider MUST NOT occur silently.

## 8. Errors

Public error object:

```json
{
  "error": {
    "code": "policy_denied",
    "message": "Model not permitted for this tenant",
    "request_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

Stable codes: `invalid_request`, `unauthenticated`, `unauthorized`, `policy_denied`, `unsupported_capability`, `rate_limited`, `overloaded`, `upstream_timeout`, `upstream_rejected`, `upstream_malformed`, `transport_failure`, `privacy_negotiation_failure`, `internal_error`.

Errors MUST NOT include secrets, prompts, stack traces, private URLs, or raw provider bodies.

## 9. Limits (defaults)

| Limit | Default |
|-------|---------|
| Max messages | 128 |
| Max tools | 32 |
| Max tool schema depth | 8 |
| Max string field | 256 KiB |
| Max attachment | 10 MiB |
| Max stream duration | 300 s |
| Max concurrent per tenant | 32 |

## 10. Schemas

Machine-readable schemas live under `schemas/`. CI verifies drift against Rust types where generation is enabled.

## 11. Example (direct chat request)

```json
{
  "ir_version": 1,
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "idempotency_key": "client-abc-1",
  "created_at": "2026-08-13T23:00:00Z",
  "privacy_profile": "direct",
  "model": "darial-mock",
  "messages": [
    {"role": "user", "content": [{"type": "text", "text": "Hello"}]}
  ],
  "stream": false
}
```

## Compatibility and deprecation

Breaking wire changes require a new major protocol version. Deprecated fields remain accepted for at least one minor release with warnings in operator telemetry (never in content logs).

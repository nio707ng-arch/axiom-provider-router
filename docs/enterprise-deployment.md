# Enterprise Deployment

## Authorization first

Before deploying Darial, obtain organizational approval. Ask network administrators to **allowlist** the Darial gateway (and relay, if used) hostnames. Darial is not a tool for bypassing firewalls, TLS inspection, or acceptable-use policy.

## Approved HTTPS

- Expose gateway/relay on port **443** with valid publicly or enterprise-trusted certificates.
- Use identifiable DNS names (example placeholders: `gateway.darial.example`, `relay.darial.example`).
- Support standard enterprise HTTP(S) proxy configuration via conventional environment variables (`HTTPS_PROXY`, `NO_PROXY`) when authorized.
- **Never** silently disable certificate verification. An explicit, high-friction insecure mode for lab use only MUST log continuous warnings and MUST be refused in `production` profile.

## Egress inventory (machine-readable)

See [deploy/egress-inventory.json](../deploy/egress-inventory.json). Summary:

| Destination | Port | Purpose |
|-------------|------|---------|
| Gateway hostname | 443 | Agent → gateway (direct) |
| Relay hostname | 443 | Agent → relay (private) |
| Relay → gateway | 443 | Protected envelope forward |
| `api.openai.com` | 443 | OpenAI provider (if enabled) |
| `api.anthropic.com` | 443 | Anthropic provider (if enabled) |
| `generativelanguage.googleapis.com` | 443 | Gemini provider (if enabled) |

Exact provider hostnames are those configured in gateway config — never client-supplied.

## Administrator controls

- Disable private mode
- Restrict models, providers, tools, attachment MIME types
- Quotas, rate limits, spend caps
- Credential rotation and revocation
- Retention: default **no prompt retention**
- Audit security metadata without content

## Identity

- Personal: scoped gateway tokens
- Enterprise: OIDC/OAuth at an authorized component; map roles to Darial scopes
- Separate data-plane and admin-plane credentials

## Logging and retention

Configure telemetry exporters that never receive prompt bodies. Define retention for metadata (default suggestion: 30 days) and document deletion runbooks in [operations.md](operations.md).

## Health checks

- `GET /healthz` — process up
- `GET /readyz` — config loaded, secret store reachable, optional upstream probe without leaking secrets

## TLS and proxies

Document corporate root CAs via the system trust store or `tls.additional_roots` (PEM file). Darial never disables certificate verification.

### Caddy in front of the gateway

The supported 0.1 HTTPS layout is:

```text
agent  ->  Caddy :443 (TLS)  ->  darial gateway :8443 (HTTP, internal)
```

See [deploy/compose/README.md](../deploy/compose/README.md) and [containers.md](containers.md). Ask administrators to allowlist the Caddy hostname on port 443. Do not expose the gateway port publicly.

Proxy use must remain ordinary HTTPS CONNECT/forward proxy behavior — no alternate covert transports.

# Threat Model

## Assets

| Asset | Sensitivity |
|-------|-------------|
| Prompt / completion / tool args / attachments | High — content confidentiality |
| Provider API keys | Critical — gateway-only |
| Client/gateway auth tokens | High |
| HPKE private keys | Critical — gateway-only |
| Tenant identity mapping | Medium–High |
| Policy / allowlists | Medium — integrity |
| Telemetry backends | Medium — metadata aggregation |

## Actors

1. Passive/active corporate network observer
2. Honest-but-curious relay operator
3. Honest-but-curious gateway operator
4. Malicious or compromised client
5. Malicious tenant (credential theft, SSRF, exhaustion, policy bypass)
6. Compromised workstation, gateway, relay, provider account, CI, registry, dependency, or telemetry
7. LLM provider (retention, abuse monitoring)
8. Global passive adversary (traffic analysis) — **out of scope for anonymity claims**

## Trust assumptions

- Administrators correctly configure allowlists, TLS trust, and auth issuers.
- Loopback agent is not exposed to other network hosts unless explicitly opted in.
- Relay and gateway do not collude when private-mode unlinkability is claimed.
- Provider must see plaintext to perform inference.
- OS and allocator memory zeroization is best-effort, not a hard guarantee.

## Attack surfaces and mitigations

| Threat | Mitigation | Effect | Tests |
|--------|------------|--------|-------|
| Arbitrary proxying / SSRF | Hard-coded allowlists; URLs from config only; deny private/link-local/metadata IPs for custom endpoints | Prevents | SSRF integration tests |
| Non-loopback agent exposure | Default bind `127.0.0.1`; high-friction opt-in + warning | Limits | Config validation tests |
| Prompt logging | Field classification; no body logging; canary tests | Prevents (default) | Secret-canary tests |
| Token theft in logs | Redaction; hashed token storage; SecretStore | Limits | Canary + unit |
| Replay | Timestamp window + bounded replay cache | Prevents | Replay tests |
| Downgrade | Mandatory protocol version; reject missing/unknown | Prevents | Negotiation tests |
| Relay reads content | HPKE envelope; relay has no private key | Prevents | Decrypt-negative tests |
| Gateway learns client IP (private) | Relay strips client IP; no forwarding of `X-Forwarded-For` from client | Limits | Context assertion tests |
| Cross-tenant leakage | Tenant-scoped caches/quotas; no shared content buffers | Prevents/Limits | Isolation tests |
| Deserialization bombs | Size/depth limits before heavy parse | Prevents | Oversized payload tests |
| DNS rebinding on agent | Loopback + Host validation | Limits | Host validation tests |
| Tool execution on gateway | Tools are transported, never executed by gateway | Prevents | Architecture invariant |
| Provider key exfiltration | Keys only in gateway SecretStore; never sent to agent/relay | Prevents | Adapter contract tests |
| Streaming failover duplicates | No silent failover after visible output | Prevents | Stream state tests |
| Rate abuse | Quotas, concurrency, load shedding | Limits | Rate limit tests |
| Colluding relay+gateway | Documented residual risk | Documents | N/A |
| Traffic analysis | Optional bounded padding (no elimination claim) | Limits | Documented |
| Supply chain | Lockfile, CI audit, SBOM, pinned actions | Detects/Limits | CI |

## Explicit exclusions

- Anonymity against a globally observing adversary
- Hiding prompts from the selected LLM provider
- Protection if the workstation agent is compromised
- Circumvention of enterprise firewalls or TLS inspection

## Residual risks (disclose plainly)

1. Size and timing metadata remain visible on the wire.
2. Identifying content in prompts can deanonymize users to the provider/gateway.
3. Memory remnants may survive after `zeroize` due to allocator/kernel behavior.
4. MVP private-mode auth uses in-envelope tokens (weaker unlinkability than Privacy Pass).
5. Experimental protected streaming is disabled until independent review.

## Mapping to tests

See `tests/` for integration and canary coverage. CI MUST fail if canary secrets appear in captured logs.

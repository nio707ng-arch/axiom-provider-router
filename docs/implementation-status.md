# Implementation Status

Updated: 2026-08-13

## Milestone progress

| Milestone | Status | Notes |
|-----------|--------|-------|
| 0 Design & foundation | **Complete** | Docs, ADRs, package, CI, config/telemetry |
| 1 Canonical IR & mock vertical slice | **Complete** | Agent → gateway → mock provider |
| 2 Streaming & provider adapters | **Complete** | SSE, OpenAI/Anthropic/Gemini adapters |
| 3 Tools, DLP, images | **Complete** | Deterministic DLP; bounded images; tools transported not executed |
| 4 Private buffered mode | **Complete (review pending)** | HPKE envelopes; relay content-blind; crypto review before 1.0 |
| 5 Enterprise hardening | **Partial** | Docker Compose + Apple Container helpers; OIDC still out of scope |
| 6 Protected streaming research | **Deferred** | Experimental flag present; disabled by default |

## Residual risks

See [threat-model.md](threat-model.md) and [privacy.md](privacy.md). Private-mode unlinkability is weaker than Privacy Pass. External crypto review pending.

## Trust assumptions (concrete)

1. Administrators configure allowlists and TLS correctly.
2. Relay and gateway do not collude when IP/content separation is claimed.
3. Provider sees plaintext for inference.
4. Workstation compromise defeats local DLP and agent protections.
5. Memory zeroization is best-effort.

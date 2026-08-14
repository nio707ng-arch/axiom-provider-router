# Privacy

## Purpose

State honest, observer-by-observer guarantees. Darial's privacy layer protects data from **designated intermediaries**. It does **not** authorize bypassing network policy.

## Profiles

| Profile | Name | Status |
|---------|------|--------|
| `direct` | Direct mode | Stable |
| `private_buffered` | Private buffered request/response | Stable intent; crypto review pending for 1.0 |
| `private_streaming` | Protected streaming | **Experimental** — compile-time + runtime gated; disabled by default |

## Observer matrix

| Observer | Direct | Private buffered |
|----------|--------|------------------|
| Corporate network | Sees gateway hostname, SNI, sizes, timing | Sees relay (and/or gateway) hostname, sizes, timing |
| Relay | N/A | Client IP, sizes, timing, ciphertext; **not** plaintext, tools, attachments, provider keys |
| Gateway | Client IP (from agent connection), plaintext, model, usage | Relay IP (not client IP), plaintext after decrypt, model, usage |
| Provider | Content, model params, gateway identity | Same |
| Telemetry backend | Aggregates only; no prompts by default | Same |

## Authentication and unlinkability

Ideal private mode uses unlinkable authorization (e.g., Privacy Pass). Until a mature integration is ready, MVP private mode places short-lived bearer credentials **inside** the encrypted envelope so the relay cannot see them. The gateway can still link successive requests by token/tenant for quotas and revocation. This is a **weaker** privacy profile: it hides content from the relay and originating IP from the gateway, but does not provide strong cross-request unlinkability at the gateway.

## Metadata leakage

Even with encryption, observers may infer:

- Approximate prompt/response size (unless bounded padding is enabled)
- Timing and cadence
- Model choice (if exposed outside the envelope — Darial keeps logical model inside the envelope in private mode)
- Error classes visible at the relay (transport-level)

Optional padding is administrator-configurable and **bounded**. Darial does **not** claim padding eliminates traffic analysis.

## Logging rules

Forbidden by default in logs/metrics/traces:

- Prompt/completion text
- Tool arguments/results
- Attachment bodies or filenames
- Raw encrypted envelopes
- Authorization headers, cookies, API keys, private keys
- Full user-agent strings, query strings
- Client IPs at the gateway in private mode
- Unsanitized provider error bodies

Allowed: random request IDs, component, coarse model alias, status class, canonical error code, duration, byte/token buckets, queue depth, retry count.

## Provider visibility

The selected LLM provider **must** receive plaintext content for inference. Private mode does not change that. Do not describe Darial as "E2E encrypted chat with the model."

## Enterprise controls

Administrators MAY:

- Disable private mode entirely
- Restrict models/providers/tools
- Set retention and size policies
- Rotate/revoke credentials
- Audit security-relevant metadata without storing prompts

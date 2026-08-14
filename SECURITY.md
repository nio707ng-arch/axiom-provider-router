# Security Policy

## Reporting

Report security issues privately to the maintainers via the repository's private vulnerability reporting channel (GitHub Security Advisories when enabled) or email listed in the project settings. Do not file public issues for exploitable vulnerabilities.

Please include:

- Darial version / commit
- Component (agent, relay, gateway, provider adapter)
- Reproduction steps **without** real secrets or personal data
- Impact assessment

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x | Yes (pre-release hardening) |
| < 0.1 | No |

## Embargo

Coordinated disclosure with a reasonable embargo (typically 90 days) is preferred for high-impact issues.

## Cryptography review status

- Direct mode TLS: standard Rustls via reqwest/axum stacks
- Private buffered mode: HPKE via the `hpke` crate (RFC 9180); OHTTP-inspired framing — **independent external review pending** before 1.0 "stable privacy" claim
- Protected streaming: **experimental / disabled by default**

## Safety boundary

Darial must not be used to bypass enterprise security controls. See README and ACCEPTABLE_USE.md.

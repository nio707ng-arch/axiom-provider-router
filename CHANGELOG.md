# Changelog

All notable changes to Darial are documented in this file.

## [0.1.0] - 2026-08-14

### Added

- Initial single-package `darial` executable with `agent`, `relay`, and `gateway` roles
- Canonical IR v1, wire protocol skeleton, policy and DLP foundations
- Direct-mode loopback OpenAI-compatible `/v1/chat/completions` and `/v1/models`
- Streaming SSE with backpressure and cancellation
- Mock, OpenAI, Anthropic, and Gemini provider adapters
- Private buffered mode using HPKE (RFC 9180) with OHTTP-inspired framing
- Documentation: architecture, protocol, threat model, privacy, enterprise deployment
- CI: fmt, clippy, tests, dependency audit, secret scan hooks
- Docker Compose (gateway behind Caddy) and Apple Container helpers
- Homebrew tap with `brew services` for agent and gateway
- crates.io package (`cargo install darial`)

### Security

- Fail-closed auth/policy/crypto
- Loopback-default agent bind
- No prompt logging by default; secret-canary tests
- Independent cryptography review for private mode still pending before 1.0 stable claim

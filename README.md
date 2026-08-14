# Darial

**Darial** is a privacy-preserving, provider-neutral LLM gateway written in Rust. Approved applications talk to a local OpenAI-compatible API on loopback; Darial authenticates authorized use, applies policy, and routes only to administrator-configured LLM providers.

## Safety and compliance boundary

Darial is an **authorized LLM gateway**, not a firewall circumvention tool or general-purpose proxy.

- It does **not** forward arbitrary TCP, UDP, HTTP, DNS, WebSocket, CONNECT, SOCKS, or user-selected destination traffic.
- Upstream destinations come **only** from administrator-configured provider allowlists. Clients cannot supply arbitrary upstream URLs.
- The local agent binds to `127.0.0.1` / `::1` by default. Non-loopback binding requires an explicit high-friction opt-in and prints a prominent warning.
- Gateway and relay endpoints use ordinary, identifiable HTTPS hostnames and standard authentication. Darial does **not** implement domain fronting, traffic mimicry, covert channels, protocol obfuscation, certificate pinning intended to defeat enterprise inspection, or fallback transports around blocked HTTPS.
- A privacy layer can protect content from designated intermediaries (for example, a content-blind relay). **It does not grant permission to violate network policy.** Obtain authorization and ask administrators to allowlist the deployment hostname before use.
- Darial does **not** claim end-to-end secrecy from the destination LLM provider: providers normally receive plaintext content to perform inference.

See [SECURITY.md](SECURITY.md), [docs/threat-model.md](docs/threat-model.md), [docs/privacy.md](docs/privacy.md), and [ACCEPTABLE_USE.md](ACCEPTABLE_USE.md).

## What it does

| Role | Subcommand | Purpose |
|------|------------|---------|
| Local agent | `darial agent` | OpenAI-compatible API on loopback; DLP before egress |
| Relay | `darial relay` | Content-blind forwarder for private mode (ciphertext only) |
| Gateway | `darial gateway` | Auth, policy, provider routing |

Modes:

- **Direct mode** — agent → gateway → approved provider over TLS 1.3 HTTPS.
- **Private buffered mode** — agent → relay → gateway with HPKE-protected request/response objects so the relay cannot read prompts (see privacy docs for honest guarantees and limitations).

## Quick start

**crates.io**

```bash
cargo install darial --locked --version 0.1.0
```

**macOS / Linux Homebrew** (agent + gateway as services):

```bash
brew tap latentmeta/darial "$(pwd)"   # or https://github.com/latentmeta/darial.git
brew install darial darial-agent darial-gateway
cp "$(brew --prefix)/etc/darial/env.example" "$(brew --prefix)/etc/darial/env"
# edit tokens in that env file
brew services start darial-gateway
brew services start darial-agent
```

Native install (Windows, Linux, macOS): [docs/installation.md](docs/installation.md).

Docker and Apple Container: [docs/containers.md](docs/containers.md).

```bash
# Build
cargo build --release

# Validate sample configs
./target/release/darial agent check-config --config examples/agent.toml
./target/release/darial gateway check-config --config examples/gateway.toml

# Run gateway with the built-in mock provider (no live API keys)
./target/release/darial gateway serve --config examples/gateway.toml

# In another terminal, run the local agent
./target/release/darial agent serve --config examples/agent.toml

# Call the loopback OpenAI-compatible API
curl -s http://127.0.0.1:8080/v1/models
curl -s http://127.0.0.1:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer local-dev-token" \
  -d '{"model":"darial-mock","messages":[{"role":"user","content":"Hello"}]}'
```

Placeholder domains and dummy keys appear in examples only. Replace them for real deployments. Never disable TLS verification silently.

## Docker (agent + gateway behind Caddy)

See [docs/containers.md](docs/containers.md). Short version:

```bash
cd deploy/compose
cp .env.example .env
docker compose up --build
```

Caddy serves HTTPS on port 443; the gateway is not published. The agent is on `http://127.0.0.1:8080`. Compose notes: [deploy/compose/README.md](deploy/compose/README.md).

Apple silicon / macOS 26+: [Apple Container](docs/containers.md#3-apple-container-install) (`deploy/apple-container/`).
## Architecture (summary)

```text
Approved application
    -> OpenAI-compatible HTTP on loopback (agent)
    -> authenticated TLS 1.3 HTTPS
Gateway (or Relay then Gateway in private mode)
    -> provider-native TLS HTTPS
Approved LLM provider (allowlisted)
```

Full detail: [docs/architecture.md](docs/architecture.md).

## Privacy (honest summary)

| Observer | What they can learn (typical) |
|----------|-------------------------------|
| Enterprise network | Connections to identifiable gateway/relay hostnames; sizes/timing |
| Relay (private mode) | Client network identity, coarse timing, ciphertext size — **not** plaintext |
| Gateway | Content needed for routing/inference; in private mode, not originating client IP |
| Provider | Request content, model parameters, gateway identity |

No claim of anonymity against collusion between relay and gateway, global traffic analysis, endpoint compromise, a malicious provider, or identifying prompt content. Details: [docs/privacy.md](docs/privacy.md).

## Compatibility status

Initial release targets a useful OpenAI Chat Completions subset (`/v1/chat/completions`, `/v1/models`, streaming, tools, bounded images). Full provider feature parity is a non-goal. See [docs/provider-adapters.md](docs/provider-adapters.md).

## Documentation

| Document | Contents |
|----------|----------|
| [docs/installation.md](docs/installation.md) | Native, Homebrew, and crates.io |
| [docs/releasing.md](docs/releasing.md) | crates.io + Homebrew publish steps |
| [docs/containers.md](docs/containers.md) | Docker and Apple Container |
| [docs/architecture.md](docs/architecture.md) | Components, trust boundaries, deployment profiles |
| [docs/protocol.md](docs/protocol.md) | Normative wire protocol |
| [docs/threat-model.md](docs/threat-model.md) | Threat analysis |
| [docs/privacy.md](docs/privacy.md) | Observer-by-observer guarantees |
| [docs/enterprise-deployment.md](docs/enterprise-deployment.md) | Allowlisting and egress |
| [docs/configuration.md](docs/configuration.md) | Configuration reference |
| [docs/operations.md](docs/operations.md) | Runbooks |
| [docs/development.md](docs/development.md) | Development guide |

## License

Apache License 2.0. See [LICENSE](LICENSE).

## Implementation status

See [docs/implementation-status.md](docs/implementation-status.md) for milestone progress and residual risks.

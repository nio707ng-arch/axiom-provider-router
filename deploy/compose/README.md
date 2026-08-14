# Docker Compose: agent + gateway behind Caddy

Darial’s gateway speaks HTTP on loopback inside the compose network. **Caddy** terminates ordinary HTTPS on port 443. The local agent is published only on the host loopback interface.

```text
OpenCode / curl
    -> 127.0.0.1:8080   (agent container, host-published loopback)
    -> https://gateway.localhost  (Caddy :443)
    -> gateway:8443     (not published)
    -> allowlisted provider
```

Ask administrators to allowlist the gateway hostname on port 443 before a real deployment. This stack does not bypass firewalls.

Native binaries: [docs/installation.md](../../docs/installation.md). Docker Desktop / Engine / Apple Container: [docs/containers.md](../../docs/containers.md).

## Quick start (local)

```bash
cd deploy/compose
cp .env.example .env
# edit dummy tokens in .env

docker compose up --build
```

Then on the host:

```bash
export DARIAL_LOCAL_TOKEN=local-dev-token-change-me   # match .env

curl -s http://127.0.0.1:8080/v1/models \
  -H "Authorization: Bearer $DARIAL_LOCAL_TOKEN"

curl -s http://127.0.0.1:8080/v1/chat/completions \
  -H "Authorization: Bearer $DARIAL_LOCAL_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"darial-mock","messages":[{"role":"user","content":"Hello"}]}'
```

Point OpenCode / Tidewave at `http://127.0.0.1:8080` with that bearer token.

Local HTTPS uses Caddy’s internal CA for `gateway.localhost`. The agent trusts it via `tls.additional_roots` (verification stays on; it is not `insecure_skip_verify`).

## Production (public hostname)

1. Put a real DNS name on the host (or load balancer) that Caddy binds.
2. Set in `.env`:
   - `DARIAL_GATEWAY_HOST=gateway.example.com`
   - `CADDY_EMAIL=you@example.com`
3. Change `[gateway] url` in `config/agent.toml` to `https://gateway.example.com`.
4. Remove the `[tls]` additional_roots section (public Let’s Encrypt / enterprise CAs are already trusted).
5. Unset `DARIAL_WAIT_FOR_CA` in compose, or leave it unused.
6. Ask IT to allowlist `gateway.example.com:443`.
7. Enable OpenAI (or another provider) in `config/gateway.toml` and set `DARIAL_OPENAI_API_KEY` in `.env`.

Do not publish `gateway:8443` to the internet. Only Caddy should be reachable on 80/443.

## OpenAI

In `config/gateway.toml`:

```toml
[providers.openai]
enabled = true
```

Set `DARIAL_OPENAI_API_KEY` in `.env`. Use model `gpt-4o-mini` (or add aliases). Keep the mock provider only if you still want offline tests.

## Security notes

- Agent port mapping is `127.0.0.1:8080:8080` so the OpenAI-compatible API is not on the LAN.
- Inside the container the agent binds `0.0.0.0` with `allow_non_loopback = true` (required for Docker NAT). That is a high-friction opt-in; do not publish `8080` on `0.0.0.0` on the host.
- Caddy access logs must not include request bodies.
- Rotate `DARIAL_GATEWAY_TOKEN` and `DARIAL_LOCAL_TOKEN` if they leak; restart the stack.

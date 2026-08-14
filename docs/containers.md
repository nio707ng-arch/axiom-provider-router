# Containers

This page covers **installing**, **running**, and **operating** Darial in containers.

Two runtimes are supported:

| Runtime | Where | Typical use |
|---------|--------|-------------|
| [Docker](https://docs.docker.com/) / Docker Compose | Linux, macOS, Windows | Gateway (and optional agent) with Caddy in front |
| [Apple Container](https://github.com/apple/container) (`container` CLI) | Apple silicon Mac, macOS 26+ | Same images, no Docker Desktop |

Images are OCI images built from `deploy/docker/Dockerfile`. Apple Container understands that Dockerfile.

The gateway must sit behind ordinary HTTPS (Caddy in the Compose file). Ask administrators to allowlist the gateway hostname on port **443**. Containers are not a way around enterprise policy.

Recommended split:

- **Agent** on the workstation (native), loopback only — best for OpenCode / Tidewave.
- **Gateway** in a container, unpublished, with **Caddy** publishing 443.

Compose also runs the agent in a container published as `127.0.0.1:8080` if you want everything in containers.

---

## 1. Docker: install the engine

### Linux

Install Docker Engine from Docker’s distro instructions, or:

```bash
# Debian/Ubuntu example — follow https://docs.docker.com/engine/install/ for current steps
sudo apt-get update
sudo apt-get install -y docker.io docker-compose-v2
sudo usermod -aG docker "$USER"
# log out and back in
docker version
docker compose version
```

### macOS

Install [Docker Desktop for Mac](https://docs.docker.com/desktop/setup/install/mac-install/) (Apple silicon or Intel). Confirm:

```bash
docker version
docker compose version
```

### Windows

Install [Docker Desktop for Windows](https://docs.docker.com/desktop/setup/install/windows-install/) with the WSL 2 backend. In PowerShell:

```powershell
docker version
docker compose version
```

---

## 2. Docker: run the gateway (and Caddy)

From a clone of this repository:

```bash
cd deploy/compose
cp .env.example .env
# edit tokens in .env — never commit .env
```

### Full stack (agent + gateway + Caddy)

```bash
docker compose up --build
```

| Service | Reachable at | Notes |
|---------|----------------|-------|
| Agent | `http://127.0.0.1:8080` on the host | Published loopback-only |
| Caddy | `https://gateway.localhost:443` | TLS terminator |
| Gateway | *not published* | Only Caddy can reach `:8443` |

```bash
export DARIAL_LOCAL_TOKEN=local-dev-token-change-me   # match .env
curl -s http://127.0.0.1:8080/healthz
curl -s http://127.0.0.1:8080/v1/models \
  -H "Authorization: Bearer $DARIAL_LOCAL_TOKEN"
```

Windows PowerShell: `$env:DARIAL_LOCAL_TOKEN = "..."`.

Local TLS uses Caddy’s internal CA for `*.localhost`. The agent container trusts it with `tls.additional_roots` (verification stays **on**).

### Gateway + Caddy only (native agent)

If the agent already runs on the host (`darial agent serve`):

```bash
docker compose up --build gateway caddy
```

Point the native agent `[gateway] url` at `https://gateway.localhost` (or your public hostname). You must trust Caddy’s local CA, or use a publicly trusted certificate (below).

### Production hostname

1. DNS A/AAAA record for `gateway.example.com` → this host.
2. In `.env`: `DARIAL_GATEWAY_HOST=gateway.example.com` and `CADDY_EMAIL=you@example.com`.
3. Agent URL: `https://gateway.example.com`.
4. Remove `[tls] additional_roots` from the agent config when using Let’s Encrypt / enterprise CAs.
5. Allowlist `gateway.example.com:443` with IT.
6. Enable OpenAI in `config/gateway.toml` and set `DARIAL_OPENAI_API_KEY`.

Do **not** publish `8443` on `0.0.0.0`.

More Compose detail: [deploy/compose/README.md](../deploy/compose/README.md).

### Gateway image without Compose

```bash
docker build -t darial:0.1.0 -f deploy/docker/Dockerfile .

docker run --rm --name darial-gateway \
  --read-only --tmpfs /tmp \
  --cap-drop ALL --security-opt no-new-privileges \
  -e DARIAL_GATEWAY_TOKEN --env-file deploy/compose/.env \
  -v "$(pwd)/deploy/compose/config/gateway.toml:/config/gateway.toml:ro" \
  -p 127.0.0.1:8443:8443 \
  darial:0.1.0 gateway serve --config /config/gateway.toml
```

Put Caddy (or another proxy) on 443 in front of `127.0.0.1:8443`. Publishing `8443` on all interfaces is not recommended.

---

## 3. Apple Container: install

[Apple Container](https://github.com/apple/container) (`container`) runs Linux OCI containers as lightweight VMs on a Mac. It is **not** Docker Desktop.

Requirements:

- Mac with **Apple silicon**
- **macOS 26** (Tahoe) or later — this is Apple’s supported baseline
- Administrator rights for the first install

Intel Macs are not supported by `container`. Use Docker Desktop or a native binary instead.

### Install the CLI

Signed package (preferred):

1. Download the `.pkg` from [apple/container releases](https://github.com/apple/container/releases).
2. Open it and follow the installer (files go under `/usr/local`).

Or Homebrew:

```bash
brew install container
```

### Start the system service

```bash
container system start
```

The first start may download a Linux kernel. Confirm:

```bash
container --version
container ls
```

Optional: enable DNS names so containers can reach each other as `name.test` (see [Apple networking docs](https://github.com/apple/container/blob/main/docs/networking.md)):

```bash
mkdir -p ~/.config/container
# add to ~/.config/container/config.toml:
# [dns]
# domain = "test"
container system stop
container system start
sudo container system dns create test
```

Stop later with `container system stop`.

---

## 4. Apple Container: build and run Darial

Helper scripts live in `deploy/apple-container/`. Run them from the **repository root**.

### Build the image

```bash
./deploy/apple-container/build.sh
# equivalent:
# container build --tag darial:0.1.0 --file deploy/docker/Dockerfile .
```

### Run the gateway (typical Mac setup)

Keep the **agent native** on macOS. Run only the gateway in Apple Container, published on loopback:

```bash
cp deploy/compose/.env.example deploy/compose/.env   # if needed
./deploy/apple-container/run-gateway.sh
```

This starts `darial-gateway` with `127.0.0.1:8443` forwarded to container port 8443.

Native agent (`examples/agent.toml`):

```toml
[gateway]
url = "http://127.0.0.1:8443"
auth_token_env = "DARIAL_GATEWAY_TOKEN"
privacy_profile = "direct"
```

```bash
export DARIAL_LOCAL_TOKEN DARIAL_GATEWAY_TOKEN
darial agent serve --config examples/agent.toml
```

For HTTPS, put **Caddy on the Mac** (`brew install caddy`) in front of `127.0.0.1:8443`, or run the Caddy container:

```bash
./deploy/apple-container/run-caddy.sh
```

`run-caddy.sh` publishes `127.0.0.1:80` and `127.0.0.1:443` and reverse-proxies to the gateway container. If you enabled `*.test` DNS, the proxy target is `darial-gateway.test:8443`; otherwise the script uses the gateway’s IP from `container inspect`.

Logs and process list:

```bash
container ls
container logs darial-gateway
container logs darial-caddy
```

Stop:

```bash
./deploy/apple-container/stop.sh
```

### Optional: agent in Apple Container

```bash
container run --name darial-agent --detach \
  --publish 127.0.0.1:8080:8080 \
  --env-file deploy/compose/.env \
  --volume "$(pwd)/deploy/compose/config/agent.toml:/config/agent.toml" \
  darial:0.1.0 agent serve --config /config/agent.toml
```

The sample Compose agent config talks to `https://gateway.localhost` and expects Caddy’s local CA. For Apple Container, prefer a dedicated agent toml whose `[gateway] url` is `http://127.0.0.1:8443` **from the Mac**, or `http://darial-gateway.test:8443` **from another container** after DNS setup. Do not publish `8080` on all host interfaces.

---

## 5. Day-to-day container operations

| Task | Docker | Apple Container |
|------|--------|-----------------|
| Build | `docker compose build` / `docker build` | `./deploy/apple-container/build.sh` |
| Start | `docker compose up -d` | `run-gateway.sh` (+ `run-caddy.sh`) |
| Logs | `docker compose logs -f gateway` | `container logs darial-gateway` |
| Health | `curl http://127.0.0.1:8080/healthz` | same, if agent published; or `curl http://127.0.0.1:8443/healthz` |
| Stop | `docker compose down` | `./deploy/apple-container/stop.sh` |
| Rebuild | `docker compose up --build` | rebuild image, `stop.sh`, run again |

Rotate leaked tokens by changing `.env` (or the process environment) and restarting. There is no separate revoke list in 0.1.

Do not enable Caddy/Docker request-body logging. Prompts must not land in container logs.

### Choosing a runtime

- **Linux server:** Docker Engine + Compose, Caddy on 443, native or omitted agent.
- **Windows workstation:** Docker Desktop for the gateway; native `darial.exe` agent on loopback is usually simpler than a Windows container.
- **Mac (Intel):** Docker Desktop or native binaries. Apple Container is Apple silicon only.
- **Mac (Apple silicon, macOS 26+):** Apple Container *or* Docker Desktop. Apple Container avoids Docker Desktop if you already use `container`.

## Safety reminder

Upstream destinations still come only from gateway config. Running in Docker or Apple Container does not allow arbitrary proxying, CONNECT tunnels, or unapproved hostnames.

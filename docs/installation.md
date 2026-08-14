# Installation

Darial is a single binary named `darial`. The same binary runs the **agent** (loopback OpenAI-compatible API) and the **gateway** (policy + provider routing). Choose a role with a subcommand.

Obtain authorization before pointing the agent at a remote gateway. Ask administrators to allowlist the gateway hostname on port 443. Darial is not a firewall bypass.

Minimum Rust version: **1.75**. A current stable toolchain is recommended.

For containers, see [containers.md](containers.md) (Docker and Apple Container).

## Homebrew (macOS and Linux)

Preferred install when you have [Homebrew](https://brew.sh). This repository is the tap (`Formula/`).

```bash
# From a clone of this repo:
brew tap latentmeta/darial "$(pwd)"

# Or from GitHub:
brew tap latentmeta/darial https://github.com/latentmeta/darial.git

brew install darial darial-agent darial-gateway
```

On first install, copy the env file and set tokens (do not leave `change-me` in production):

```bash
cp "$(brew --prefix)/etc/darial/env.example" "$(brew --prefix)/etc/darial/env"
${EDITOR:-nano} "$(brew --prefix)/etc/darial/env"
```

Configs are in `$(brew --prefix)/etc/darial/agent.toml` and `gateway.toml` (not overwritten on upgrade if you already edited them).

### Start the gateway and agent as services

```bash
brew services start darial-gateway
brew services start darial-agent
brew services list | grep darial
```

| Service | Bind | Logs |
|---------|------|------|
| `darial-agent` | `http://127.0.0.1:8080` | `$(brew --prefix)/var/log/darial-agent.log` |
| `darial-gateway` | `http://127.0.0.1:8443` | `$(brew --prefix)/var/log/darial-gateway.log` |

On macOS this uses **launchd** (user services; do not `sudo brew services` for the agent). On Linux Homebrew it uses **systemd --user**.

```bash
curl -s http://127.0.0.1:8080/healthz
curl -s http://127.0.0.1:8443/healthz
```

Point OpenCode / Tidewave at `http://127.0.0.1:8080` with `Authorization: Bearer` matching `DARIAL_LOCAL_TOKEN`.

Stop / restart:

```bash
brew services restart darial-agent
brew services restart darial-gateway
brew services stop darial-agent
brew services stop darial-gateway
```

Put Caddy on port 443 in front of `127.0.0.1:8443` for a real deployment. Ask IT to allowlist that hostname.

### Formula-only (no services)

```bash
brew install darial
darial agent serve --config "$(brew --prefix)/etc/darial/agent.toml"
darial gateway serve --config "$(brew --prefix)/etc/darial/gateway.toml"
```

## crates.io

```bash
cargo install darial --locked --version 0.1.0
darial --version
```

Requires a Rust toolchain (MSRV 1.75). This installs only the `darial` binary, not the Homebrew service wrappers.

---

## Install the binary from source

### Linux

```bash
# Build dependencies (Debian/Ubuntu)
sudo apt-get update
sudo apt-get install -y build-essential pkg-config curl git

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

git clone https://github.com/latentmeta/darial.git
cd darial
cargo build --release
sudo install -m 0755 target/release/darial /usr/local/bin/darial
darial --version
```

Fedora/RHEL: install `gcc`, `pkgconf-pkg-config`, `curl`, and `git` instead of `apt-get`.

### macOS (without Homebrew)

If you did not use the [Homebrew](#homebrew-macos-and-linux) path:

```bash
xcode-select --install   # if needed
# Optional: brew install rustup-init && rustup-init

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

git clone https://github.com/latentmeta/darial.git
cd darial
cargo build --release
sudo install -m 0755 target/release/darial /usr/local/bin/darial
darial --version
```

### Windows

1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the **Desktop development with C++** workload (required by native TLS/crypto crates).
2. Install Rust from [https://rustup.rs](https://rustup.rs) (`x86_64-pc-windows-msvc`).
3. Open **x64 Native Tools Command Prompt** or PowerShell:

```powershell
git clone https://github.com/latentmeta/darial.git
cd darial
cargo build --release
```

The binary is `target\release\darial.exe`. Copy it somewhere on your `PATH`, for example:

```powershell
New-Item -ItemType Directory -Force $env:USERPROFILE\bin | Out-Null
Copy-Item target\release\darial.exe $env:USERPROFILE\bin\
# Add %USERPROFILE%\bin to your user PATH, then reopen the terminal
darial --version
```

---

## 1. Agent (Windows, Linux, macOS)

The agent should stay on the **workstation**, bound to loopback. Applications such as OpenCode or Tidewave call `http://127.0.0.1:8080`.

### Configure

Copy `examples/agent.toml` and set the gateway URL:

```toml
profile = "dev"
role = "agent"

[listen]
bind = "127.0.0.1:8080"
allow_non_loopback = false

[auth]
require_local_auth = true
local_token_env = "DARIAL_LOCAL_TOKEN"

[gateway]
url = "https://gateway.example.com"   # or http://127.0.0.1:8443 for local gateway
auth_token_env = "DARIAL_GATEWAY_TOKEN"
privacy_profile = "direct"

[dlp]
enabled = true
default_action = "block"
```

Never bind the agent to a LAN address unless you understand the risk. Non-loopback bind requires `allow_non_loopback = true` and is refused in `profile = "production"`.

### Linux and macOS

```bash
export DARIAL_LOCAL_TOKEN='choose-a-long-random-local-token'
export DARIAL_GATEWAY_TOKEN='same-token-the-gateway-expects'

darial agent check-config --config agent.toml
darial agent serve --config agent.toml
```

Leave that terminal open. Health check:

```bash
curl -s http://127.0.0.1:8080/healthz
```

Optional user service on Linux (`~/.config/systemd/user/darial-agent.service`):

```ini
[Unit]
Description=Darial local agent
After=network.target

[Service]
ExecStart=/usr/local/bin/darial agent serve --config %h/.config/darial/agent.toml
Environment=DARIAL_LOCAL_TOKEN=replace-me
Environment=DARIAL_GATEWAY_TOKEN=replace-me
Restart=on-failure

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now darial-agent
```

On macOS you can keep `darial agent serve` in a terminal, or wrap the same command in a LaunchAgent if you want it at login. Do not expose port 8080 in the firewall.

### Windows

PowerShell:

```powershell
$env:DARIAL_LOCAL_TOKEN = "choose-a-long-random-local-token"
$env:DARIAL_GATEWAY_TOKEN = "same-token-the-gateway-expects"

darial agent check-config --config examples\agent.toml
darial agent serve --config examples\agent.toml
```

In a second PowerShell:

```powershell
curl.exe -s http://127.0.0.1:8080/healthz
```

Point OpenCode / Tidewave at `http://127.0.0.1:8080` with header `Authorization: Bearer <DARIAL_LOCAL_TOKEN>`.

---

## 2. Gateway (Windows, Linux, macOS)

The gateway holds provider API keys and must not be an open proxy. In production put **Caddy (or another TLS terminator) on port 443** in front of it and bind the gateway to localhost.

### Configure

Start from `examples/gateway.toml` (local mock) or `deploy/compose/config/gateway.toml` (container/production-style).

Minimal local file:

```toml
profile = "dev"
role = "gateway"

[listen]
bind = "127.0.0.1:8443"

[auth]
token_env = "DARIAL_GATEWAY_TOKEN"

[providers.mock]
enabled = true
kind = "mock"
models = ["mock-1"]

[model_aliases.darial-mock]
provider = "mock"
model = "mock-1"
```

To call OpenAI, enable the provider and set `DARIAL_OPENAI_API_KEY`. Keep `base_url` as the official hostname; clients cannot supply an upstream URL.

### Linux

```bash
export DARIAL_GATEWAY_TOKEN='choose-a-long-random-gateway-token'
# export DARIAL_OPENAI_API_KEY='sk-...'   # only if OpenAI is enabled

darial gateway check-config --config gateway.toml
darial gateway serve --config gateway.toml
```

Production-style systemd (after installing the binary and config):

```bash
sudo useradd --system --home /var/lib/darial --shell /usr/sbin/nologin darial
sudo mkdir -p /etc/darial /var/lib/darial
sudo cp gateway.toml /etc/darial/gateway.toml
sudo cp deploy/systemd/darial-gateway.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now darial-gateway
```

Put Caddy or nginx on `:443` proxying to `127.0.0.1:8443`. Ask IT to allowlist that public hostname. See [enterprise-deployment.md](enterprise-deployment.md).

### macOS

```bash
export DARIAL_GATEWAY_TOKEN='choose-a-long-random-gateway-token'
darial gateway check-config --config gateway.toml
darial gateway serve --config gateway.toml
```

For HTTPS on the Mac, install Caddy (`brew install caddy`) and reverse-proxy to `127.0.0.1:8443`. Example Caddyfile:

```caddy
gateway.example.com {
    reverse_proxy 127.0.0.1:8443 {
        flush_interval -1
        transport http {
            read_timeout 5m
            write_timeout 5m
        }
    }
}
```

`flush_interval -1` is required for SSE streaming.

### Windows

```powershell
$env:DARIAL_GATEWAY_TOKEN = "choose-a-long-random-gateway-token"
darial gateway check-config --config examples\gateway.toml
darial gateway serve --config examples\gateway.toml
```

Bind remains `127.0.0.1:8443` in the sample. For a reachable HTTPS endpoint, run Caddy for Windows (or IIS/nginx) on port 443 proxying to that address. Do not expose `8443` on the public interface.

---

## First request

With both processes running and tokens set:

```bash
curl -s http://127.0.0.1:8080/v1/chat/completions \
  -H "Authorization: Bearer $DARIAL_LOCAL_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"darial-mock","messages":[{"role":"user","content":"Hello"}]}'
```

Windows: use `curl.exe` and `$env:DARIAL_LOCAL_TOKEN`.

If DLP is on, do not paste live API keys into prompts; the default action is **block**.

## Configuration checks

```text
darial agent check-config --config agent.toml
darial gateway check-config --config gateway.toml
darial agent print-effective-config --config agent.toml
```

`print-effective-config` redacts secrets. Full option list: [configuration.md](configuration.md).

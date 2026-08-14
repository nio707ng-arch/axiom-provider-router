//! End-to-end: agent → gateway → mock provider (direct mode).

use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::Value;

fn wait_http(url: &str, attempts: u32) {
    for _ in 0..attempts {
        if let Ok(resp) = reqwest::blocking::get(url) {
            if resp.status().is_success() {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("service not ready: {url}");
}

struct Proc(Child);

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

#[test]
fn direct_chat_completions_mock() {
    let gateway_port = free_port();
    let agent_port = free_port();
    let bin = env!("CARGO_BIN_EXE_darial");

    let gw_cfg = format!(
        r#"
profile = "dev"
role = "gateway"
[listen]
bind = "127.0.0.1:{gateway_port}"
[auth]
token_env = "DARIAL_GATEWAY_TOKEN"
[providers.mock]
enabled = true
kind = "mock"
models = ["mock-1"]
[model_aliases.darial-mock]
provider = "mock"
model = "mock-1"
"#
    );
    let ag_cfg = format!(
        r#"
profile = "dev"
role = "agent"
[listen]
bind = "127.0.0.1:{agent_port}"
[auth]
require_local_auth = true
local_token_env = "DARIAL_LOCAL_TOKEN"
[gateway]
url = "http://127.0.0.1:{gateway_port}"
auth_token_env = "DARIAL_GATEWAY_TOKEN"
privacy_profile = "direct"
[dlp]
enabled = true
default_action = "block"
"#
    );

    let dir = tempfile::tempdir().unwrap();
    let gw_path = dir.path().join("gateway.toml");
    let ag_path = dir.path().join("agent.toml");
    std::fs::write(&gw_path, gw_cfg).unwrap();
    std::fs::write(&ag_path, ag_cfg).unwrap();

    let gw = Proc(
        Command::new(bin)
            .args(["gateway", "serve", "--config"])
            .arg(&gw_path)
            .env("DARIAL_GATEWAY_TOKEN", "gw-token")
            .env("DARIAL_LOG", "error")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_http(&format!("http://127.0.0.1:{gateway_port}/healthz"), 100);

    let _ag = Proc(
        Command::new(bin)
            .args(["agent", "serve", "--config"])
            .arg(&ag_path)
            .env("DARIAL_LOCAL_TOKEN", "local-dev-token")
            .env("DARIAL_GATEWAY_TOKEN", "gw-token")
            .env("DARIAL_LOG", "error")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_http(&format!("http://127.0.0.1:{agent_port}/healthz"), 100);

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{agent_port}/v1/chat/completions"))
        .bearer_auth("local-dev-token")
        .json(&serde_json::json!({
            "model": "darial-mock",
            "messages": [{"role":"user","content":"Hello from test"}]
        }))
        .send()
        .unwrap();
    assert!(resp.status().is_success(), "status {}", resp.status());
    let body: Value = resp.json().unwrap();
    let content = body["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(content.contains("Hello from test"));

    // DLP should block canary token without echoing it in error
    let secret = "sk-CANARY-SECRET-DO-NOT-LOG-abcdefgh";
    let resp = client
        .post(format!("http://127.0.0.1:{agent_port}/v1/chat/completions"))
        .bearer_auth("local-dev-token")
        .json(&serde_json::json!({
            "model": "darial-mock",
            "messages": [{"role":"user","content": format!("key {secret}")}]
        }))
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 403);
    let err_text = resp.text().unwrap();
    assert!(!err_text.contains(secret));

    drop(gw);
}

#[test]
fn private_buffered_through_relay() {
    let gateway_port = free_port();
    let relay_port = free_port();
    let agent_port = free_port();
    let bin = env!("CARGO_BIN_EXE_darial");
    let dir = tempfile::tempdir().unwrap();
    let key_dir = dir.path().join("keys");

    // Generate keys
    let status = Command::new(bin)
        .args(["gateway", "key", "generate", "--output"])
        .arg(&key_dir)
        .status()
        .unwrap();
    assert!(status.success());

    let origin = format!("http://127.0.0.1:{gateway_port}");
    let gw_cfg = format!(
        r#"
profile = "dev"
role = "gateway"
[listen]
bind = "127.0.0.1:{gateway_port}"
[auth]
token_env = "DARIAL_GATEWAY_TOKEN"
[privacy]
key_dir = "{key_dir}"
[providers.mock]
enabled = true
kind = "mock"
models = ["mock-1"]
[model_aliases.darial-mock]
provider = "mock"
model = "mock-1"
"#,
        key_dir = key_dir.display()
    );
    let relay_cfg = format!(
        r#"
profile = "dev"
role = "relay"
[listen]
bind = "127.0.0.1:{relay_port}"
[relay]
gateway_url = "http://127.0.0.1:{gateway_port}"
"#
    );
    let ag_cfg = format!(
        r#"
profile = "dev"
role = "agent"
[listen]
bind = "127.0.0.1:{agent_port}"
[auth]
require_local_auth = true
local_token_env = "DARIAL_LOCAL_TOKEN"
[gateway]
url = "{origin}"
auth_token_env = "DARIAL_GATEWAY_TOKEN"
privacy_profile = "private_buffered"
[relay]
url = "http://127.0.0.1:{relay_port}"
[dlp]
enabled = false
"#
    );

    let gw_path = dir.path().join("gateway.toml");
    let relay_path = dir.path().join("relay.toml");
    let ag_path = dir.path().join("agent.toml");
    std::fs::write(&gw_path, gw_cfg).unwrap();
    std::fs::write(&relay_path, relay_cfg).unwrap();
    std::fs::write(&ag_path, ag_cfg).unwrap();

    let _gw = Proc(
        Command::new(bin)
            .args(["gateway", "serve", "--config"])
            .arg(&gw_path)
            .env("DARIAL_GATEWAY_TOKEN", "gw-token")
            .env("DARIAL_GATEWAY_ORIGIN", &origin)
            .env("DARIAL_LOG", "error")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_http(&format!("http://127.0.0.1:{gateway_port}/healthz"), 100);

    let _relay = Proc(
        Command::new(bin)
            .args(["relay", "serve", "--config"])
            .arg(&relay_path)
            .env("DARIAL_LOG", "error")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_http(&format!("http://127.0.0.1:{relay_port}/healthz"), 100);

    let _ag = Proc(
        Command::new(bin)
            .args(["agent", "serve", "--config"])
            .arg(&ag_path)
            .env("DARIAL_LOCAL_TOKEN", "local-dev-token")
            .env("DARIAL_GATEWAY_TOKEN", "gw-token")
            .env("DARIAL_GATEWAY_ORIGIN", &origin)
            .env("DARIAL_LOG", "error")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_http(&format!("http://127.0.0.1:{agent_port}/healthz"), 100);

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{agent_port}/v1/chat/completions"))
        .bearer_auth("local-dev-token")
        .json(&serde_json::json!({
            "model": "darial-mock",
            "messages": [{"role":"user","content":"private hello"}]
        }))
        .send()
        .unwrap();
    assert!(
        resp.status().is_success(),
        "status {} body {}",
        resp.status(),
        resp.text().unwrap_or_default()
    );
}

#[allow(dead_code)]
fn _addr(port: u16) -> SocketAddr {
    format!("127.0.0.1:{port}").parse().unwrap()
}

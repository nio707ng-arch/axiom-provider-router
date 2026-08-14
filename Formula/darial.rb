# frozen_string_literal: true

class Darial < Formula
  desc "Privacy-preserving, provider-neutral LLM gateway"
  homepage "https://github.com/latentmeta/darial"
  license "Apache-2.0"
  version "0.1.0"
  url "https://static.crates.io/crates/darial/darial-0.1.0.crate"
  sha256 "7a44e4ed8bc4ecb55ab63d69d8632f49e6b0d44bfaf74e1f5f933ddaa67c7d7b"
  head "https://github.com/latentmeta/darial.git", branch: "main"

  livecheck do
    url "https://crates.io/api/v1/crates/darial"
    regex(/"newest_version"\s*:\s*"(\d+\.\d+\.\d+)"/)
  end

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--locked"
    bin.install "target/release/darial"

    (bin/"darial-agent").write <<~SH
      #!/bin/bash
      set -euo pipefail
      env_file="#{etc}/darial/env"
      if [[ -f "${env_file}" ]]; then
        set -a
        # shellcheck disable=SC1090
        source "${env_file}"
        set +a
      fi
      exec "#{opt_bin}/darial" agent "$@"
    SH

    (bin/"darial-gateway").write <<~SH
      #!/bin/bash
      set -euo pipefail
      env_file="#{etc}/darial/env"
      if [[ -f "${env_file}" ]]; then
        set -a
        # shellcheck disable=SC1090
        source "${env_file}"
        set +a
      fi
      exec "#{opt_bin}/darial" gateway "$@"
    SH

    chmod 0755, bin/"darial-agent", bin/"darial-gateway"

    pkgetc = etc/"darial"
    pkgetc.mkpath
    pkgetc.install "examples/agent.toml"
    pkgetc.install "examples/gateway.toml"
    pkgetc.install "examples/brew.env.example" => "env.example"
  end

  def post_install
    (var/"darial").mkpath
    (var/"log").mkpath
  end

  def caveats
    <<~EOS
      Config:  #{etc}/darial/{agent,gateway}.toml
      Secrets: copy #{etc}/darial/env.example to #{etc}/darial/env and edit tokens.

      Start as background services (Homebrew services / launchd on macOS,
      systemd --user on Linux Homebrew):

        brew install darial-agent darial-gateway
        brew services start darial-gateway
        brew services start darial-agent

      Agent loopback API: http://127.0.0.1:8080
      Gateway (put Caddy on :443 in front for production): http://127.0.0.1:8443

      Obtain authorization before using a remote gateway. Ask administrators
      to allowlist the gateway hostname. Darial is not a firewall bypass.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/darial --version")
    assert_match "agent", shell_output("#{bin}/darial-agent --help")
    assert_match "gateway", shell_output("#{bin}/darial-gateway --help")
  end
end

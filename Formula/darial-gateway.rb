# frozen_string_literal: true

class DarialGateway < Formula
  desc "Brew services unit for the Darial gateway"
  homepage "https://github.com/latentmeta/darial"
  license "Apache-2.0"
  version "0.1.0"
  url "https://static.crates.io/crates/darial/darial-0.1.0.crate"
  sha256 "7a44e4ed8bc4ecb55ab63d69d8632f49e6b0d44bfaf74e1f5f933ddaa67c7d7b"
  head "https://github.com/latentmeta/darial.git", branch: "main"

  depends_on "darial"

  def install
    (prefix/"README.service").write <<~EOS
      Darial gateway brew services wrapper.
      Config: #{Formula["darial"].etc}/darial/gateway.toml
      Start:  brew services start darial-gateway
    EOS
  end

  def post_install
    (var/"darial").mkpath
    (var/"log").mkpath
  end

  service do
    run [
      Formula["darial"].opt_bin/"darial-gateway",
      "serve",
      "--config",
      Formula["darial"].etc/"darial/gateway.toml",
    ]
    keep_alive true
    working_dir var/"darial"
    log_path var/"log/darial-gateway.log"
    error_log_path var/"log/darial-gateway.log"
    environment_variables PATH: std_service_path_env, DARIAL_LOG: "info"
  end

  def caveats
    <<~EOS
      Bind: 127.0.0.1:8443. Put Caddy (or another TLS terminator) on port 443
      in front for production. Ask IT to allowlist that hostname.

      Put tokens in #{Formula["darial"].etc}/darial/env then:
        brew services start darial-gateway

      Logs: #{var}/log/darial-gateway.log
    EOS
  end

  test do
    assert_predicate Formula["darial"].opt_bin/"darial-gateway", :executable?
  end
end

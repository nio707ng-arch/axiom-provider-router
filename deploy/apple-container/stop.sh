#!/bin/sh
set -eu
container stop darial-caddy darial-gateway darial-agent >/dev/null 2>&1 || true
container rm -f darial-caddy darial-gateway darial-agent >/dev/null 2>&1 || true
echo "stopped darial apple-container workloads"

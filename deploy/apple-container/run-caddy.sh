#!/bin/sh
# Run Caddy in Apple Container in front of darial-gateway (loopback HTTPS).
#
# Default upstream is darial-gateway.test:8443 (Apple container DNS).
# If you have not configured *.test DNS, set:
#   DARIAL_UPSTREAM=<gateway-ip>:8443
# Get the IP with: container inspect darial-gateway
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CADDYFILE="${ROOT}/deploy/apple-container/Caddyfile"
UPSTREAM="${DARIAL_UPSTREAM:-darial-gateway.test:8443}"

if ! container ls | grep -q darial-gateway; then
    echo "error: darial-gateway is not running (start run-gateway.sh first)" >&2
    exit 2
fi

container stop darial-caddy >/dev/null 2>&1 || true
container rm darial-caddy >/dev/null 2>&1 || true

container run --name darial-caddy --detach \
    --publish 127.0.0.1:80:80 \
    --publish 127.0.0.1:443:443 \
    --env "DARIAL_UPSTREAM=${UPSTREAM}" \
    --volume "${CADDYFILE}:/etc/caddy/Caddyfile" \
    caddy:2-alpine

echo "caddy on https://127.0.0.1 (loopback) -> ${UPSTREAM}"
echo "if proxy fails, set DARIAL_UPSTREAM to the IP from: container inspect darial-gateway"

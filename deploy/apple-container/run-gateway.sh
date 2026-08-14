#!/bin/sh
# Run the Darial gateway in Apple Container, published on loopback only.
# Usage: from repository root, ./deploy/apple-container/run-gateway.sh
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ENV_FILE="${ROOT}/deploy/compose/.env"
CFG="${ROOT}/deploy/compose/config/gateway.toml"

if [ ! -f "${ENV_FILE}" ]; then
    echo "error: missing ${ENV_FILE} (copy deploy/compose/.env.example)" >&2
    exit 2
fi

"${ROOT}/deploy/apple-container/build.sh"

container stop darial-gateway >/dev/null 2>&1 || true
container rm darial-gateway >/dev/null 2>&1 || true

container run --name darial-gateway --detach \
    --publish 127.0.0.1:8443:8443 \
    --env-file "${ENV_FILE}" \
    --volume "${CFG}:/config/gateway.toml" \
    darial:0.1.0 gateway serve --config /config/gateway.toml

echo "gateway listening on http://127.0.0.1:8443 (loopback)"
echo "put Caddy on 443 in front for production HTTPS (see run-caddy.sh)"

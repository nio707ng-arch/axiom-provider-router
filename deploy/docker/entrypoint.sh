#!/bin/sh
set -eu

# Optional: wait for a PEM CA (Caddy local PKI) before the agent starts TLS.
if [ -n "${DARIAL_WAIT_FOR_CA:-}" ]; then
    i=0
    while [ ! -s "${DARIAL_WAIT_FOR_CA}" ]; do
        i=$((i + 1))
        if [ "${i}" -gt 60 ]; then
            echo "error: timed out waiting for TLS CA at ${DARIAL_WAIT_FOR_CA}" >&2
            exit 2
        fi
        sleep 1
    done
fi

exec /usr/local/bin/darial "$@"

# Operations

## Scaling

- Horizontally scale stateless gateway replicas behind TLS-terminating load balancers.
- Replay caches and rate limiters SHOULD use shared stores in multi-replica deployments (MVP: in-memory per process with documented limitation).
- Bound concurrency and queue depth; shed load with `overloaded`.

## Alerts (metadata only)

- Elevated `5xx` / `upstream_timeout` rates
- Auth failure spikes
- Queue depth / load shedding
- Certificate expiry
- Key `not_after` approaching

Never alert on prompt content.

## Key rotation

1. `darial gateway key generate --output ./keys-new`
2. Add new public key to published set with overlapping validity
3. Deploy gateway with both private keys during overlap
4. Clients refresh `/v1/darial/ohttp-keys`
5. Retire old key after overlap + max request validity window

Refuse to overwrite existing key files without `--force`.

## Incident response

1. Revoke compromised tokens/tenants
2. Rotate gateway HPKE and provider keys as needed
3. Emergency shutdown: stop gateway/relay units; agent fails closed
4. Preserve metadata audits; do not scramble to collect prompts unless a separate consented workflow exists (omitted from first release)

## Retention

Default: no prompt/completion persistence. Metadata retention per enterprise policy. Deletion runbook: purge metrics TSDB / log indices by time range; verify canary absence.

## Backups

Prefer regenerable config in version control (no secrets). Secrets live in a secret manager. Key material: offline encrypted backup with dual control if required by policy.

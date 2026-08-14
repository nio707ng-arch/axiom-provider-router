# ADR 0003: Loopback-Default Local Agent

## Status

Accepted

## Context

Exposing an unauthenticated or weakly authenticated LLM proxy on a LAN is a common footgun (DNS rebinding, lateral movement).

## Decision

The agent binds to `127.0.0.1` and/or `::1` by default. Non-loopback binding requires `allow_non_loopback = true`, refused in production profile without it, and emits a prominent warning. Optional local bearer auth is recommended even on loopback; unauthenticated loopback is only for an explicit `dev` profile.

## Consequences

- Safer defaults
- Slightly more config for remote-agent edge cases (out of MVP scope)

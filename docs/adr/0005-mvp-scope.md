# ADR 0005: MVP Scope and Deferrals

## Status

Accepted

## Context

The full prompt spans many milestones. Shipping requires explicit deferrals.

## Decision — MVP includes

- Direct mode text + streaming
- Mock + OpenAI + Anthropic + Gemini adapters (core chat)
- Local DLP allow/warn/redact/block
- Tools and bounded images through IR
- Private buffered mode with documented limitations
- Loopback agent OpenAI-compatible subset

## Explicitly deferred

- Privacy Pass / strong unlinkable auth
- Protected streaming (experimental gate only)
- Shared Redis rate-limit store
- Full OIDC admin UI
- Malware scanning implementations (interface only)
- Multi-provider failover after stream start
- Fetching client-provided attachment URLs

## Consequences

- Honest README compatibility claims
- Faster path to a secure runnable system

# ADR 0002: Canonical Intermediate Representation

## Status

Accepted

## Context

Multiple providers expose incompatible APIs. The gateway needs a stable internal model for policy, logging classification, and adapters.

## Decision

Define a versioned canonical IR in `canonical_ir.rs` with explicit capability negotiation. Provider-specific extensions live only in namespaced fields and cannot override routing or security fields. Unsupported features fail with `unsupported_capability` unless an explicit compatibility policy says otherwise.

## Consequences

- Predictable policy enforcement
- Adapters stay thin
- Not every provider quirk is papered over

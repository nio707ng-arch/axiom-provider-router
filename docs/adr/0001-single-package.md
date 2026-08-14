# ADR 0001: Single Cargo Package

## Status

Accepted

## Context

The implementation prompt requires one Rust package, one executable `darial`, and role selection via subcommands. Splitting into many crates can improve reuse but increases versioning and audit surface.

## Decision

Keep a single package with modules for agent, relay, gateway, providers, etc. Do not publish reusable SDK crates.

## Consequences

- Simpler dependency graph and release unit
- Clearer security boundary for review
- Less hypothetical abstraction

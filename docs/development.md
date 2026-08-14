# Development

## Package layout

Single package `darial`, binary `darial`. Modules under `src/` as described in the architecture doc. Do **not** split into a workspace without an explicit project decision.

## Toolchain

- Rust MSRV: **1.75** (CI may use newer stable)
- `cargo fmt`, `cargo clippy -- -D warnings` (pedantic allows listed in `Cargo.toml`)
- `unsafe_code` denied at package level

Install and run locally: [installation.md](installation.md) (includes Homebrew services). Containers: [containers.md](containers.md).

## Testing

```bash
cargo test
cargo test --features "" # default offline
DARIAL_LIVE_TESTS=1 cargo test --test live_providers -- --ignored
```

Add fixtures under `tests/fixtures/`. Mock providers with `wiremock`.

## Adding a provider adapter

1. Implement `ProviderAdapter` in `src/providers/<name>.rs`
2. Register in `providers/mod.rs` and config allowlist
3. Add capability matrix row and contract tests
4. Update egress inventory

## Protocol compatibility

- Never break IR major version without a migration guide
- Prefer rejecting unsupported capabilities over silent drop
- Update `schemas/` and CI drift checks

## Security hygiene

- No prompt content in tests logs
- Use dummy keys only
- Do not add circumvention features or docs

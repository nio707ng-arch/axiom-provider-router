# Contributing

Thanks for contributing to Darial.

## Rules of engagement

1. Safety and compliance first — no circumvention features or docs.
2. Do not log prompts, secrets, or attachment bodies in tests or examples.
3. Prefer failing closed on auth, policy, and crypto errors.
4. Do not add `unsafe` without isolation, justification, and tests (package denies unsafe by default).
5. Do not invent cryptographic primitives.

## Workflow

1. Open an issue or discussion for protocol/security-sensitive changes.
2. Fork and create a feature branch.
3. Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`.
4. Update docs and schemas when changing wire formats.
5. Submit a PR with a clear threat/impact note for security-relevant changes.

## Code of conduct

Be respectful. Harassment and bad-faith security bypass advocacy are not welcome. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

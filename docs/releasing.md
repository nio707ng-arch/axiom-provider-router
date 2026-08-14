# Releasing Darial

Publish the **same** crate version to crates.io, then point Homebrew at that tarball.

The crate owner on crates.io is **[thanos](https://crates.io/users/thanos)** (same account as [oratos](https://crates.io/crates/oratos)). The GitHub repo stays `latentmeta/darial`. Workflow: [`.github/workflows/publish.yml`](../.github/workflows/publish.yml).

## Automated (preferred)

1. Bump `version` in `Cargo.toml` and the three Homebrew formulae (run `./scripts/update-brew-sha256.sh` after `cargo package`).
2. Commit, tag `vX.Y.Z` (must match `Cargo.toml`), push the tag.
3. The **publish** workflow runs tests, packs the crate, and uploads to crates.io.

### Credentials (one-time)

**First publish** (crate not yet on crates.io): log in as crates.io user **thanos**, create an API token, and add it as repository secret `CARGO_REGISTRY_TOKEN`. The first `cargo publish` must use that account so the crate is owned by `thanos`, not an org or CI identity. Trusted Publishing cannot be configured until the crate exists.

**Later releases:** on [crates.io](https://crates.io) → crate **darial** → Settings → Trusted Publishing, add:

| Field | Value |
|-------|--------|
| Repository owner | `latentmeta` |
| Repository name | `darial` |
| Workflow filename | `publish.yml` |
| Environment | `crates-io` |

After that, OIDC short-lived tokens are enough; you can delete `CARGO_REGISTRY_TOKEN`.

GitHub environment **crates-io** can require reviewers before the job publishes.

Manual dry-run (no upload): Actions → **publish** → Run workflow → leave dry_run checked.

## Manual fallback

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets --locked
cargo package --locked
./scripts/update-brew-sha256.sh
git tag v0.1.0
git push origin v0.1.0
# or: cargo publish --locked
```

## Homebrew after crates.io

Index updates take a minute. Then:

```bash
brew tap latentmeta/darial https://github.com/latentmeta/darial.git
brew install darial darial-agent darial-gateway
```

## Notes

- Do not publish from pull requests or forks. The workflow is tag-only and restricted to `latentmeta/darial`.
- Bump `version` in `Cargo.toml` and all three formulae together.
- Private-mode cryptography is still pending external review; do not market it as 1.0-stable.

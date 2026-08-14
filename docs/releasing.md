# Releasing Darial 0.1.0

Publish the **same** `0.1.0` artifact to crates.io, then point Homebrew at it.

You need a [crates.io](https://crates.io) account and `cargo login`. GitHub repo: `latentmeta/darial`.

## 1. Checks

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo package --list
```

## 2. Pack and pin Homebrew sha256

```bash
cargo package --allow-dirty   # omit --allow-dirty on a clean tagged tree
./scripts/update-brew-sha256.sh
git add Formula Cargo.toml
```

`update-brew-sha256.sh` writes the SHA-256 of `target/package/darial-0.1.0.crate` into `Formula/*.rb`. That hash must match crates.io after publish.

## 3. Commit, tag, push

```bash
git add -A
git status   # no secrets, no .env
git commit -m "Release v0.1.0"
git tag v0.1.0
git push origin main
git push origin v0.1.0
```

## 4. crates.io

```bash
cargo publish
```

Install check (after index updates, often ~1 minute):

```bash
cargo install darial --version 0.1.0
darial --version
```

## 5. Homebrew

From a machine with the tap:

```bash
brew tap latentmeta/darial https://github.com/latentmeta/darial.git
brew update
brew install darial darial-agent darial-gateway
```

If you already tapped, `brew update` / `brew reinstall darial`.

`brew services start darial-gateway` and `brew services start darial-agent` after copying `$(brew --prefix)/etc/darial/env.example` to `env`.

## Notes

- Do not publish from a dirty tree that contains `.env` or real API keys.
- Bump `version` in `Cargo.toml` and all three formulae together for later releases.
- Private-mode cryptography is still pending external review; do not market it as 1.0-stable.

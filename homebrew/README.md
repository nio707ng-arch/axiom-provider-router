# Homebrew tap

This repository is a [Homebrew](https://brew.sh) tap. Formulae live in [`Formula/`](../Formula/).

Stable installs download the crate from crates.io (same artifact as `cargo install darial`). Full instructions: **[docs/installation.md](../docs/installation.md#homebrew-macos-and-linux)**.

```bash
brew tap latentmeta/darial "$(pwd)"          # local clone
# brew tap latentmeta/darial https://github.com/latentmeta/darial.git

brew install darial darial-agent darial-gateway
cp "$(brew --prefix)/etc/darial/env.example" "$(brew --prefix)/etc/darial/env"
# edit tokens in that env file

brew services start darial-gateway
brew services start darial-agent
```

After publishing a new crates.io version, run `./scripts/update-brew-sha256.sh` so `sha256` matches the crate tarball. Use `brew install --HEAD darial` to build the git tip.

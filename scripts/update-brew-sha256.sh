#!/bin/sh
# Write crates.io tarball sha256 into Homebrew formulae.
# Run after: cargo package
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "${ROOT}/Cargo.toml" | head -1)"
CRATE="${ROOT}/target/package/darial-${VERSION}.crate"

if [ ! -f "${CRATE}" ]; then
    echo "error: missing ${CRATE} — run cargo package first" >&2
    exit 2
fi

SHA="$(shasum -a 256 "${CRATE}" | awk '{print $1}')"
echo "darial-${VERSION}.crate sha256 ${SHA}"

for f in "${ROOT}/Formula/darial.rb" \
         "${ROOT}/Formula/darial-agent.rb" \
         "${ROOT}/Formula/darial-gateway.rb"; do
    # portable in-place replace of sha256 line
    tmp="$(mktemp)"
    awk -v sha="${SHA}" -v ver="${VERSION}" '
        /^  version / { print "  version \"" ver "\""; next }
        /^  url / {
            print "  url \"https://static.crates.io/crates/darial/darial-" ver ".crate\""
            next
        }
        /^  sha256 / { print "  sha256 \"" sha "\""; next }
        { print }
    ' "${f}" > "${tmp}"
    mv "${tmp}" "${f}"
done

echo "updated Formula/*.rb"

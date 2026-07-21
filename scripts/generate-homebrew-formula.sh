#!/usr/bin/env bash
# Fill packaging/homebrew/sub-maker.rb.tmpl with version + SHA256 of release assets.
#
# Usage:
#   scripts/generate-homebrew-formula.sh <version> [dist-dir] [output-path]
#
# Reads binaries named:
#   sub-maker-linux-amd64, sub-maker-linux-arm64,
#   sub-maker-darwin-amd64, sub-maker-darwin-arm64
# from dist-dir (default: .)
#
# Default output: dist/sub-maker.rb (for release assets + homebrew-tap publish)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${1:?version required (e.g. 0.2.0 or v0.2.0)}"
VERSION="${VERSION#v}"
DIST="${2:-$ROOT}"
OUT="${3:-$DIST/sub-maker.rb}"
TMPL="$ROOT/packaging/homebrew/sub-maker.rb.tmpl"

sha_of() {
  local f="$1"
  if [[ ! -f "$f" ]]; then
    echo "MISSING"
    return 0
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" | awk '{print $1}'
  else
    shasum -a 256 "$f" | awk '{print $1}'
  fi
}

LINUX_AMD64="$(sha_of "$DIST/sub-maker-linux-amd64")"
LINUX_ARM64="$(sha_of "$DIST/sub-maker-linux-arm64")"
DARWIN_AMD64="$(sha_of "$DIST/sub-maker-darwin-amd64")"
DARWIN_ARM64="$(sha_of "$DIST/sub-maker-darwin-arm64")"

for label in LINUX_AMD64 LINUX_ARM64 DARWIN_AMD64 DARWIN_ARM64; do
  val="${!label}"
  if [[ "$val" == "MISSING" ]]; then
    echo "warning: missing asset for $label — formula will need a rebuild when available" >&2
    # Homebrew rejects empty sha; use a placeholder that fails install loudly
    eval "$label=0000000000000000000000000000000000000000000000000000000000000000"
  fi
done

mkdir -p "$(dirname "$OUT")"
sed \
  -e "s|__VERSION__|${VERSION}|g" \
  -e "s|__SHA_LINUX_AMD64__|${LINUX_AMD64}|g" \
  -e "s|__SHA_LINUX_ARM64__|${LINUX_ARM64}|g" \
  -e "s|__SHA_DARWIN_AMD64__|${DARWIN_AMD64}|g" \
  -e "s|__SHA_DARWIN_ARM64__|${DARWIN_ARM64}|g" \
  "$TMPL" >"$OUT"

echo "Wrote $OUT (version $VERSION)"

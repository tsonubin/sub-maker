#!/usr/bin/env bash
# Build .deb and .rpm packages from prebuilt linux binaries using nfpm.
#
# Usage:
#   scripts/package-linux.sh <version> [dist-dir]
#
# Expects in dist-dir (default: .):
#   sub-maker-linux-amd64
#   sub-maker-linux-arm64
#
# Writes:
#   sub-maker_<ver>_amd64.deb
#   sub-maker_<ver>_arm64.deb
#   sub-maker-<ver>-1.x86_64.rpm
#   sub-maker-<ver>-1.aarch64.rpm

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${1:?version required (e.g. 0.2.0 or v0.2.0)}"
VERSION="${VERSION#v}"
DIST="${2:-$ROOT}"
TMPL="$ROOT/packaging/nfpm.yaml.tmpl"
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

if ! command -v nfpm >/dev/null 2>&1; then
  echo "nfpm not found; installing via go install..." >&2
  if ! command -v go >/dev/null 2>&1; then
    echo "error: need nfpm or go to install it" >&2
    exit 1
  fi
  GOBIN="$WORKDIR/bin" go install github.com/goreleaser/nfpm/v2/cmd/nfpm@latest
  export PATH="$WORKDIR/bin:$PATH"
fi

package_one() {
  local asset="$1"   # path to binary
  local deb_arch="$2"
  local rpm_arch="$3"
  local bin_copy="$WORKDIR/sub-maker-${deb_arch}"

  if [[ ! -f "$asset" ]]; then
    echo "skip missing asset: $asset" >&2
    return 0
  fi

  cp "$asset" "$bin_copy"
  chmod 755 "$bin_copy"

  local cfg="$WORKDIR/nfpm-${deb_arch}.yaml"
  sed \
    -e "s|__NAME__|sub-maker|g" \
    -e "s|__VERSION__|${VERSION}|g" \
    -e "s|__ARCH__|${deb_arch}|g" \
    -e "s|__BIN__|${bin_copy}|g" \
    "$TMPL" >"$cfg"

  # nfpm maps arch for rpm; pass target arch in config (already deb_arch style).
  # For rpm, nfpm converts amd64→x86_64 and arm64→aarch64 when packaging.
  echo "==> deb (${deb_arch})"
  nfpm package --config "$cfg" --packager deb --target "$DIST"

  # rpm arch names in config: use rpm_arch
  sed \
    -e "s|__NAME__|sub-maker|g" \
    -e "s|__VERSION__|${VERSION}|g" \
    -e "s|__ARCH__|${rpm_arch}|g" \
    -e "s|__BIN__|${bin_copy}|g" \
    "$TMPL" >"$cfg"
  echo "==> rpm (${rpm_arch})"
  nfpm package --config "$cfg" --packager rpm --target "$DIST"
}

package_one "$DIST/sub-maker-linux-amd64" "amd64" "x86_64"
package_one "$DIST/sub-maker-linux-arm64" "arm64" "aarch64"

echo "==> packages in $DIST"
ls -la "$DIST"/sub-maker*.deb "$DIST"/sub-maker*.rpm 2>/dev/null || ls -la "$DIST"

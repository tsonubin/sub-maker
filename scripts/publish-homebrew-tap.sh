#!/usr/bin/env bash
# Publish a generated formula to tsonubin/homebrew-tap.
#
# Usage:
#   scripts/publish-homebrew-tap.sh <version> <path-to-sub-maker.rb>
#
# Auth (first match wins):
#   HOMEBREW_TAP_TOKEN  — classic PAT with repo scope (required in GitHub Actions)
#   GH_TOKEN / GITHUB_TOKEN — used by `gh` when pushing locally if permitted
#
# Env:
#   HOMEBREW_TAP_REPO   — default: tsonubin/homebrew-tap

set -euo pipefail

VERSION="${1:?version required (e.g. 0.2.1)}"
VERSION="${VERSION#v}"
FORMULA_SRC="${2:?path to generated sub-maker.rb required}"
TAP_REPO="${HOMEBREW_TAP_REPO:-tsonubin/homebrew-tap}"

if [[ ! -f "$FORMULA_SRC" ]]; then
  echo "error: formula not found: $FORMULA_SRC" >&2
  exit 1
fi

TOKEN="${HOMEBREW_TAP_TOKEN:-${GH_TOKEN:-${GITHUB_TOKEN:-}}}"
if [[ -z "$TOKEN" ]]; then
  echo "error: set HOMEBREW_TAP_TOKEN (or GH_TOKEN) to push to ${TAP_REPO}" >&2
  exit 1
fi

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

export GIT_TERMINAL_PROMPT=0
git clone --depth 1 "https://x-access-token:${TOKEN}@github.com/${TAP_REPO}.git" "$WORKDIR/tap"
mkdir -p "$WORKDIR/tap/Formula"
cp "$FORMULA_SRC" "$WORKDIR/tap/Formula/sub-maker.rb"

cd "$WORKDIR/tap"
git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"

if git diff --quiet Formula/sub-maker.rb; then
  echo "Formula unchanged in ${TAP_REPO}"
  exit 0
fi

git add Formula/sub-maker.rb
git commit -m "sub-maker ${VERSION}"
git push origin HEAD:main
echo "Published Formula/sub-maker.rb → https://github.com/${TAP_REPO}"

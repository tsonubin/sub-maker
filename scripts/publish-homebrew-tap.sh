#!/usr/bin/env bash
# Publish a generated formula to tsonubin/homebrew-tap.
#
# Usage:
#   scripts/publish-homebrew-tap.sh <version> <path-to-sub-maker.rb>
#
# Auth (first match wins):
#   HOMEBREW_TAP_SSH_KEY — private key of a write deploy key on the tap (preferred in CI)
#   HOMEBREW_TAP_TOKEN / GH_TOKEN / GITHUB_TOKEN — classic PAT or gh token with repo access
#
# Env:
#   HOMEBREW_TAP_REPO — default: tsonubin/homebrew-tap

set -euo pipefail

VERSION="${1:?version required (e.g. 0.2.1)}"
VERSION="${VERSION#v}"
FORMULA_SRC="${2:?path to generated sub-maker.rb required}"
TAP_REPO="${HOMEBREW_TAP_REPO:-tsonubin/homebrew-tap}"

if [[ ! -f "$FORMULA_SRC" ]]; then
  echo "error: formula not found: $FORMULA_SRC" >&2
  exit 1
fi

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

export GIT_TERMINAL_PROMPT=0
git config --global --add safe.directory "$WORKDIR/tap" 2>/dev/null || true

clone_tap() {
  if [[ -n "${HOMEBREW_TAP_SSH_KEY:-}" ]]; then
    # Support literal \n in Actions secrets, or real multiline PEMs.
    local keyfile="$WORKDIR/deploy_key"
    printf '%s\n' "${HOMEBREW_TAP_SSH_KEY}" | sed 's/\r$//' >"$keyfile"
    # If the secret stored escaped newlines, expand them.
    if ! grep -q "BEGIN" "$keyfile" 2>/dev/null; then
      printf '%b\n' "${HOMEBREW_TAP_SSH_KEY}" >"$keyfile"
    fi
    chmod 600 "$keyfile"
    export GIT_SSH_COMMAND="ssh -i ${keyfile} -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new"
    git clone --depth 1 "git@github.com:${TAP_REPO}.git" "$WORKDIR/tap"
    return
  fi

  local token="${HOMEBREW_TAP_TOKEN:-${GH_TOKEN:-${GITHUB_TOKEN:-}}}"
  if [[ -z "$token" ]]; then
    echo "error: set HOMEBREW_TAP_SSH_KEY (deploy key) or HOMEBREW_TAP_TOKEN to push to ${TAP_REPO}" >&2
    exit 1
  fi
  git clone --depth 1 "https://x-access-token:${token}@github.com/${TAP_REPO}.git" "$WORKDIR/tap"
}

clone_tap
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

#!/usr/bin/env bash
# sub-maker installer
# Usage (recommended, uses GitHub Releases for prebuilt binary):
#   curl -fsSL https://raw.githubusercontent.com/tsonubin/sub-maker/main/install.sh | sudo bash
# or after cloning: sudo ./install.sh
#
# The script will auto-detect amd64 or arm64 and download the matching
# sub-maker-linux-{arch} from the latest GitHub Release.

# See LICENSE and DISCLAIMER.md (strict terms apply; forking/private use requires permission).

set -euo pipefail

INSTALL_DIR="/usr/local/bin"
BINARY_NAME="sub-maker"
REPO="tsonubin/sub-maker"

echo "==> sub-maker installer"
echo "This will download the latest sub-maker binary and place it in ${INSTALL_DIR}."

if [ "$(id -u)" -ne 0 ]; then
  echo "Please run as root (sudo) or the script will use sudo for the final install step."
  SUDO="sudo"
else
  SUDO=""
fi

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)
    GOARCH="amd64"
    ;;
  aarch64|arm64)
    GOARCH="arm64"
    ;;
  *)
    echo "Unsupported architecture: $ARCH"
    echo "Falling back to building from source..."
    GOARCH=""
    ;;
esac

if [ -n "$GOARCH" ]; then
  BIN_URL="https://github.com/${REPO}/releases/latest/download/sub-maker-linux-${GOARCH}"
  echo "==> Attempting to download pre-built binary for linux-${GOARCH} from GitHub Releases..."
  if curl -fL "$BIN_URL" -o "/tmp/${BINARY_NAME}"; then
    echo "Downloaded pre-built binary."

    # Best-effort checksum verification (non-fatal)
    CHECKSUM_URL="https://github.com/${REPO}/releases/latest/download/checksums.txt"
    if curl -fsL "$CHECKSUM_URL" -o /tmp/checksums.txt 2>/dev/null; then
      expected=$(grep "sub-maker-linux-${GOARCH}" /tmp/checksums.txt 2>/dev/null | awk '{print $1}' | head -1 || true)
      if [ -n "$expected" ]; then
        actual=$(sha256sum "/tmp/${BINARY_NAME}" | awk '{print $1}')
        if [ "$expected" = "$actual" ]; then
          echo "Checksum verified OK."
        else
          echo "WARNING: Checksum mismatch (expected $expected, got $actual). The binary may be corrupted or the release is in progress."
          echo "You can verify manually later with: sha256sum /usr/local/bin/sub-maker"
        fi
      else
        echo "Note: No matching checksum entry found for this arch (proceeding anyway)."
      fi
    else
      echo "Note: Could not fetch checksums.txt (proceeding without verification)."
    fi
  else
    echo "Download failed (no matching release asset or network issue)."
    GOARCH=""
  fi
fi

if [ -z "$GOARCH" ]; then
  echo "==> Falling back to building from source..."
  if ! command -v go >/dev/null 2>&1; then
    echo "Go is not installed. Installing Go (this may take a minute)..."
    curl -fsSL https://go.dev/dl/go1.23.0.linux-amd64.tar.gz -o /tmp/go.tar.gz
    rm -rf /usr/local/go
    tar -C /usr/local -xzf /tmp/go.tar.gz
    export PATH=$PATH:/usr/local/go/bin
    echo 'export PATH=$PATH:/usr/local/go/bin' > /etc/profile.d/go.sh
  fi

  echo "Cloning repository and building..."
  rm -rf /tmp/sub-maker-src
  git clone --depth 1 "https://github.com/${REPO}.git" /tmp/sub-maker-src
  cd /tmp/sub-maker-src
  go mod tidy
  go build -o "/tmp/${BINARY_NAME}" .
fi

echo "==> Installing to ${INSTALL_DIR}/${BINARY_NAME}"
$SUDO mkdir -p "$INSTALL_DIR"
$SUDO cp "/tmp/${BINARY_NAME}" "$INSTALL_DIR/${BINARY_NAME}"
$SUDO chmod +x "$INSTALL_DIR/${BINARY_NAME}"

echo "==> Done!"
echo "You can now run: sudo ${BINARY_NAME} --help"
echo ""
echo "After setup, start the services with:"
echo "  sudo systemctl daemon-reload"
echo "  sudo systemctl enable --now sing-box subconverter sub-maker-sub"
echo ""
echo "Your Clash subscription will be at:"
echo "  http://YOUR-IP:8964/sub?token=YOUR-TOKEN"
echo ""
echo "See README.md and GUIDE.md for full documentation."

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
TARGET_PATH="${INSTALL_DIR}/${BINARY_NAME}"
SUB_MAKER_SERVICE="sub-maker-sub"

echo "==> sub-maker installer"
echo "This will download the latest sub-maker binary and place it in ${INSTALL_DIR}."

if [ "$(id -u)" -ne 0 ]; then
  echo "Please run as root (sudo) or the script will use sudo for the final install step."
  SUDO="sudo"
else
  SUDO=""
fi

get_running_sub_maker_pids() {
  target="$1"
  if [ ! -e "$target" ]; then
    return 0
  fi

  if command -v fuser >/dev/null 2>&1; then
    $SUDO fuser "$target" 2>/dev/null | tr ' ' '\n' | sed '/^$/d; s/[^0-9].*$//' | sed '/^$/d' | sort -u
  elif command -v pgrep >/dev/null 2>&1; then
    pgrep -f "$target" 2>/dev/null | sort -u || true
  fi
}

confirm_reinstall_running_binary() {
  target="$1"
  pids="$2"

  echo "WARNING: ${target} appears to be running."
  echo "Running PID(s): ${pids}"
  echo "Replacing a running executable can fail with 'Text file busy'."

  if [ "${SUB_MAKER_INSTALL_REINSTALL:-}" = "1" ]; then
    echo "SUB_MAKER_INSTALL_REINSTALL=1 set; stopping the running instance and reinstalling."
    return 0
  fi

  if [ ! -r /dev/tty ]; then
    echo "Abort: cannot prompt because no interactive TTY is available."
    echo "Stop ${SUB_MAKER_SERVICE} first, then rerun the installer:"
    echo "  sudo systemctl stop ${SUB_MAKER_SERVICE}"
    echo "Or rerun non-interactively with:"
    echo "  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | sudo env SUB_MAKER_INSTALL_REINSTALL=1 bash"
    exit 1
  fi

  printf "Stop the running sub-maker instance and reinstall? [y/N] " > /dev/tty
  read -r answer < /dev/tty
  case "$answer" in
    y|Y|yes|YES)
      return 0
      ;;
    *)
      echo "Abort: leaving the existing installation untouched."
      exit 1
      ;;
  esac
}

stop_running_sub_maker() {
  target="$1"
  restart_service=0

  if command -v systemctl >/dev/null 2>&1 && systemctl is-active --quiet "$SUB_MAKER_SERVICE" 2>/dev/null; then
    echo "==> Stopping ${SUB_MAKER_SERVICE} service before reinstall..."
    $SUDO systemctl stop "$SUB_MAKER_SERVICE"
    restart_service=1
  fi

  pids=$(get_running_sub_maker_pids "$target" || true)
  if [ -n "$pids" ]; then
    echo "==> Stopping remaining sub-maker process(es): ${pids}"
    # shellcheck disable=SC2086
    $SUDO kill $pids 2>/dev/null || true
    for _ in 1 2 3 4 5; do
      sleep 1
      pids=$(get_running_sub_maker_pids "$target" || true)
      [ -z "$pids" ] && break
    done
  fi

  pids=$(get_running_sub_maker_pids "$target" || true)
  if [ -n "$pids" ]; then
    echo "Abort: sub-maker is still running as PID(s): ${pids}"
    echo "Stop it manually and rerun the installer."
    exit 1
  fi

  if [ "$restart_service" -eq 1 ]; then
    RESTART_SUB_MAKER_SERVICE=1
  fi
}

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
    GO_VERSION="1.23.0"
    GO_DL_ARCH=$(uname -m)
    case "$GO_DL_ARCH" in
      x86_64) GO_DL_ARCH="amd64" ;;
      aarch64|arm64) GO_DL_ARCH="arm64" ;;
      *) echo "Unsupported architecture for Go: $GO_DL_ARCH"; exit 1 ;;
    esac
    curl -fsSL "https://go.dev/dl/go${GO_VERSION}.linux-${GO_DL_ARCH}.tar.gz" -o /tmp/go.tar.gz
    rm -rf /usr/local/go
    tar -C /usr/local -xzf /tmp/go.tar.gz
    export PATH=$PATH:/usr/local/go/bin
    echo 'export PATH=$PATH:/usr/local/go/bin' > /etc/profile.d/go.sh
  fi

  if ! command -v git >/dev/null 2>&1; then
    echo "git is not installed. Installing git..."
    if command -v apt-get >/dev/null 2>&1; then
      apt-get update -qq || true
      apt-get install -y -qq git
    elif command -v yum >/dev/null 2>&1; then
      yum install -y git
    elif command -v apk >/dev/null 2>&1; then
      apk add --no-cache git
    else
      echo "Unable to install git automatically. Please install git and re-run."
      exit 1
    fi
  fi

  echo "Cloning repository and building..."
  rm -rf /tmp/sub-maker-src
  git clone --depth 1 "https://github.com/${REPO}.git" /tmp/sub-maker-src
  cd /tmp/sub-maker-src
  go mod tidy
  VERSION=$(git describe --tags --always --dirty 2>/dev/null || echo dev)
  CGO_ENABLED=0 go build -ldflags "-X main.version=${VERSION} -s -w" -o "/tmp/${BINARY_NAME}" .
fi

if [ -e "$TARGET_PATH" ]; then
  running_pids=$(get_running_sub_maker_pids "$TARGET_PATH" || true)
  if [ -n "$running_pids" ]; then
    confirm_reinstall_running_binary "$TARGET_PATH" "$running_pids"
    stop_running_sub_maker "$TARGET_PATH"
  fi
fi

echo "==> Installing to ${TARGET_PATH}"
$SUDO mkdir -p "$INSTALL_DIR"
tmp_target=$($SUDO mktemp "${INSTALL_DIR}/.${BINARY_NAME}.XXXXXX")
$SUDO install -m 0755 "/tmp/${BINARY_NAME}" "$tmp_target"
$SUDO mv -f "$tmp_target" "$TARGET_PATH"

if [ "${RESTART_SUB_MAKER_SERVICE:-}" = "1" ]; then
  echo "==> Restarting ${SUB_MAKER_SERVICE} service..."
  if ! $SUDO systemctl start "$SUB_MAKER_SERVICE"; then
    echo "WARNING: Installed successfully, but ${SUB_MAKER_SERVICE} did not restart."
    echo "Check it with: sudo systemctl status ${SUB_MAKER_SERVICE}"
  fi
fi

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

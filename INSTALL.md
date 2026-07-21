# Install sub-maker

Install paths for **Linux VPS** (production) and **Homebrew** on Linux or macOS (dev / demo).

See [LICENSE](LICENSE) and [DISCLAIMER.md](DISCLAIMER.md) for license terms.

For full product docs and operations, see [README.md](README.md) and [GUIDE.md](GUIDE.md). Packaging internals: [packaging/README.md](packaging/README.md).

---

## Choose a path

| Goal | Method |
|------|--------|
| Fresh Ubuntu/Debian VPS | [One-line `install.sh`](#1-one-line-install-recommended) or [`.deb`](#3-debian--ubuntu-deb) |
| Fedora / RHEL-like VPS | [One-line `install.sh`](#1-one-line-install-recommended) or [`.rpm`](#4-fedora--rhel-like-rpm) |
| Laptop (macOS or Linux) | [Homebrew](#5-homebrew-macos--linux) |
| No package manager | [Manual release binary](#2-install-from-a-release-asset-manual) |
| Contribute / hack | [Build from source](#6-build-from-source) |

Production setup (`sudo sub-maker --setup`) expects **Linux + systemd**. Homebrew on macOS is fine for the binary and `SUB_MAKER_DEMO=1` testing, not for full system install.

---

## Requirements

| Item | Notes |
|------|--------|
| **OS (production)** | Linux with systemd (Ubuntu 22.04 / 24.04 or Debian 12 recommended) |
| **Arch** | `amd64` (x86_64) or `arm64` (aarch64); macOS via Homebrew too |
| **Privileges** | Root/sudo for production install (writes `/usr/local/bin`, `/etc`, systemd) |
| **Network** | Outbound HTTPS (GitHub releases, ACME, downloads) |
| **Tools** | `curl` (required for one-liner). `tar` / `gzip` usually present |

**Recommended for production**

- A domain (or subdomain) with an `A`/`AAAA` record pointing at this VPS
- Firewall / security-group access for the ports you enable
- A free terminal (SSH) so the ratatui setup wizard can run

Minimum VPS: about **1 vCPU, 512 MB–1 GB RAM, 10 GB disk**.

---

## 1. One-line install (recommended)

Downloads the latest **pre-built** release binary for your architecture into `/usr/local/bin/sub-maker`. No Rust toolchain required.

```bash
curl -fsSL https://raw.githubusercontent.com/tsonubin/sub-maker/main/install.sh | sudo bash
```

What the script does:

1. Detects `amd64` or `arm64`
2. Fetches the matching asset from the latest [GitHub Release](https://github.com/tsonubin/sub-maker/releases)
3. Optionally verifies against `checksums.txt` when available
4. Installs to `/usr/local/bin/sub-maker`
5. If a release download is unavailable, falls back to **building from source** (installs Rust via rustup if needed)

Verify:

```bash
sub-maker --version
sub-maker --help
```

### Reinstall while services are running

If `sub-maker` is already running (e.g. `sub-maker-sub.service`), replacing the binary can fail with “Text file busy”. The installer will prompt, or you can force stop-and-reinstall:

```bash
curl -fsSL https://raw.githubusercontent.com/tsonubin/sub-maker/main/install.sh \
  | sudo env SUB_MAKER_INSTALL_REINSTALL=1 bash
```

Or stop the service first:

```bash
sudo systemctl stop sub-maker-sub
# then re-run install.sh
```

---

## 2. Install from a release asset (manual)

Useful if you prefer not to pipe a script to bash.

1. Open [Releases](https://github.com/tsonubin/sub-maker/releases) and download the binary for your machine:
   - `sub-maker-linux-amd64`
   - `sub-maker-linux-arm64`
2. (Optional) Download `checksums.txt` and verify:

   ```bash
   sha256sum -c checksums.txt --ignore-missing
   ```

3. Install:

   ```bash
   chmod +x sub-maker-linux-amd64   # or arm64
   sudo mv sub-maker-linux-amd64 /usr/local/bin/sub-maker
   sub-maker --version
   ```

---

## 3. Debian / Ubuntu (`.deb`)

Each release publishes architecture-specific packages (names include the version):

- `sub-maker_<VERSION>_amd64.deb`
- `sub-maker_<VERSION>_arm64.deb`

Example for the latest version on amd64 (replace `VERSION` and arch as needed — check [Releases](https://github.com/tsonubin/sub-maker/releases)):

```bash
ARCH=amd64   # or arm64
# Discover latest tag (optional)
TAG=$(curl -fsSL https://api.github.com/repos/tsonubin/sub-maker/releases/latest | sed -n 's/.*"tag_name": "\(.*\)".*/\1/p' | head -1)
VERSION=${TAG#v}

curl -fLO "https://github.com/tsonubin/sub-maker/releases/download/${TAG}/sub-maker_${VERSION}_${ARCH}.deb"
curl -fLO "https://github.com/tsonubin/sub-maker/releases/download/${TAG}/checksums.txt"
sha256sum -c checksums.txt --ignore-missing

sudo dpkg -i "sub-maker_${VERSION}_${ARCH}.deb"
sub-maker --version
```

There is **no hosted apt repository** yet; install the `.deb` directly (or use `install.sh`). A future apt repo can serve these same packages.

---

## 4. Fedora / RHEL-like (`.rpm`)

Release assets:

- `sub-maker-<VERSION>-1.x86_64.rpm`
- `sub-maker-<VERSION>-1.aarch64.rpm`

```bash
# Example for x86_64 — set TAG/VERSION from the Releases page
TAG=v0.2.1
VERSION=0.2.1

curl -fLO "https://github.com/tsonubin/sub-maker/releases/download/${TAG}/sub-maker-${VERSION}-1.x86_64.rpm"
sudo dnf install "./sub-maker-${VERSION}-1.x86_64.rpm"
# or: sudo rpm -Uvh ./sub-maker-*.rpm
sub-maker --version
```

---

## 5. Homebrew (macOS + Linux)

Formulae live in the personal tap **[tsonubin/homebrew-tap](https://github.com/tsonubin/homebrew-tap)** (`brew tap tsonubin/tap`). Prebuilt release binaries for macOS and Linux.

```bash
brew tap tsonubin/tap
brew install sub-maker
sub-maker --version
```

Upgrade:

```bash
brew update
brew upgrade sub-maker
```

(Optional) install the formula file directly from a release asset:

```bash
brew install --formula \
  https://github.com/tsonubin/sub-maker/releases/latest/download/sub-maker.rb
```

On macOS, prefer demo mode — production paths and systemd are Linux-oriented:

```bash
SUB_MAKER_DEMO=1 sub-maker --setup
```

---

## 6. Build from source

Needs a Rust toolchain and a C linker (`build-essential` / `gcc` on Debian/Ubuntu).

### Debian / Ubuntu

```bash
sudo apt update
sudo apt install -y git curl build-essential pkg-config

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

git clone https://github.com/tsonubin/sub-maker.git
cd sub-maker
cargo build --release
sudo install -m 755 target/release/sub-maker /usr/local/bin/sub-maker
sub-maker --version
```

### Fedora / RHEL-like

```bash
sudo dnf install -y git curl gcc
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
# then clone + cargo build --release as above
```

### Alpine

```bash
sudo apk add --no-cache git curl build-base
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
# then clone + cargo build --release as above
```

Using the Makefile on a checkout:

```bash
make build
sudo make install   # installs ./sub-maker to /usr/local/bin
```

---

## 7. Run production setup

On the VPS (interactive TTY required for the wizard):

```bash
sudo sub-maker --setup
# or: sudo sub-maker setup
```

The ratatui wizard walks through:

1. Production domain vs IP-only mode  
2. Server address, domain, email, subscription token, SSH link passcode, sub port  
3. Protocols (Reality, Hysteria2, TUIC, AnyTLS, SS2022)  
4. Certificate strategy (if TLS protocols are enabled)  
5. Per-protocol ports and credentials  
6. Review → apply  

Apply will:

- Download **sing-box** and **subconverter** when needed  
- Issue or place certificates  
- Write `/etc/sing-box/config.json`, `/etc/sub-maker/*`, systemd units  
- Enable and start services, then verify `http://127.0.0.1:<port>/sub?token=...`  

### Firewall (example with ufw)

Open only what you enabled (defaults shown):

```bash
sudo ufw allow OpenSSH
sudo ufw allow 8964/tcp    # subscription
sudo ufw allow 443/tcp     # Reality
sudo ufw allow 8443/udp    # Hysteria2
sudo ufw allow 9443/udp    # TUIC
sudo ufw allow 7443/tcp    # AnyTLS
sudo ufw allow 8388/tcp    # SS2022
sudo ufw allow 8388/udp    # SS2022
# temporarily, for Certbot/acme HTTP-01:
# sudo ufw allow 80/tcp
sudo ufw enable
sudo ufw status
```

Also open the same ports in your cloud provider’s security group if applicable.

### After setup

```bash
sudo sub-maker links      # subscription + raw URLs
sudo sub-maker link       # subscription URL (passcode-protected)
sudo sub-maker doctor     # health checks
sudo sub-maker status     # systemd units
```

Add the printed **Subscription** URL as a remote config in Clash / Clash Meta / Stash / similar.

---

## 8. Demo install (no root)

For local testing without touching system paths:

```bash
# binary already on PATH, or: cargo run --
SUB_MAKER_DEMO=1 sub-maker --setup
```

- Configs → `/tmp/sub-maker-demo-etc/...`  
- sing-box → `~/.local/bin/sing-box`  
- subconverter → `~/.local/subconverter`  
- systemd is not enabled  

Serve the subscription manually if needed:

```bash
export SUB_MAKER_DEMO=1
export SUB_MAKER_TOKEN=demo-token-123
export SUB_MAKER_NODES_FILE=/tmp/sub-maker-demo-etc/sub-maker/nodes.txt
export SUB_MAKER_PORT=8964
sub-maker --serve
```

---

## 9. Upgrade

**sub-maker binary** (pick the channel you used)

```bash
# install.sh
curl -fsSL https://raw.githubusercontent.com/tsonubin/sub-maker/main/install.sh | sudo bash
# or: sudo env SUB_MAKER_INSTALL_REINSTALL=1 bash when the service holds the binary

# Homebrew
brew upgrade sub-maker

# deb / rpm — download the new package from Releases and dpkg/dnf install again
```

Then restart the subscription unit if it is running:

```bash
sudo systemctl restart sub-maker-sub
```

**sing-box / subconverter only**

```bash
sudo sub-maker --update
sudo sub-maker restart
```

**Reconfigure after first setup**

```bash
sudo sub-maker config
```

---

## 10. Uninstall (manual)

There is no automated uninstaller. To remove a production install:

```bash
sudo systemctl disable --now sing-box sub-maker-sub subconverter 2>/dev/null || true
sudo rm -f /usr/local/bin/sub-maker /usr/local/bin/sing-box
sudo rm -f /etc/systemd/system/sing-box.service \
           /etc/systemd/system/sub-maker-sub.service \
           /etc/systemd/system/subconverter.service
sudo systemctl daemon-reload
# optional: remove data (configs, nodes, certs)
# sudo rm -rf /etc/sub-maker /etc/sing-box /opt/subconverter
```

---

## Troubleshooting install

| Problem | What to try |
|---------|-------------|
| `curl: command not found` | `sudo apt install curl` (or distro equivalent) |
| Release download fails | Check network / GitHub access; use [manual asset](#2-install-from-a-release-asset-manual) or [build from source](#3-build-from-source) |
| `Text file busy` | Stop `sub-maker-sub` or use `SUB_MAKER_INSTALL_REINSTALL=1` |
| `sub-maker: command not found` | Confirm `/usr/local/bin` is on `PATH`; re-run install |
| Wizard exits / garbled UI | Use a real TTY (`ssh` interactive session), not a non-interactive pipe |
| Permission denied during setup | Re-run with `sudo` for production paths |
| Source build fails | Install `build-essential`/`gcc`, ensure `cargo` is on `PATH` after rustup |

```bash
# useful diagnostics after a partial setup
sudo sub-maker doctor
sudo journalctl -u sing-box -u sub-maker-sub -xe --no-pager | tail -100
```

---

## Next steps

- Day-to-day commands and architecture: [README.md](README.md)  
- Longer tutorial (DNS, certs, clients, hardening): [GUIDE.md](GUIDE.md)  

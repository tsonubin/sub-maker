# sub-maker

**sub-maker** is a TUI tool to set up a proxy server on a VPS using sing-box. It supports multiple protocols, verifies domain DNS, handles certificates via Certbot or acme.sh, starts services, and provides a Clash subscription endpoint on port 8964.

See [LICENSE](LICENSE) for terms. This is a restricted license: viewing source for personal research/education only; forking or any use/deployment by others requires prior written permission.

## Quick Start

```bash
curl -fsSL https://raw.githubusercontent.com/tsonubin/sub-maker/main/install.sh | sudo bash
sudo sub-maker --setup
```

Production setup finishes by starting services, verifying the local subscription endpoint, and printing your subscription URL. If DNS or certificates are not ready, setup stops with the exact record or cert action needed.

See the full usage below and GUIDE.md for details.

## What it does

- Interactive setup wizard for server configuration.
- Supports VLESS+Reality, Hysteria2, TUICv5, AnyTLS, and Shadowsocks 2022 via sing-box.
- DNS checks for domain-based setup.
- Certificate handling through Certbot HTTP-01, acme.sh HTTP-01/DNS-01, existing cert files, or self-signed fallback.
- Built-in Clash subscription server on 8964 with rulesets.
- Systemd service generation for easy deployment.
- Operational commands: `doctor`, `links`, `status`, `restart`, `start`, `stop`.
- Demo mode for testing without root.

See below for commands and GUIDE.md for full tutorial.

## Prerequisites

- A Linux VPS (Ubuntu 22.04 / 24.04 or Debian 12 recommended; other distros may work with manual adjustments).
- **Root / sudo privileges are required for a full real installation** on a production server. The tool performs system-level operations that need elevated privileges:
  - Installs sing-box binary to `/usr/local/bin/sing-box`.
  - Installs subconverter to `/opt/subconverter`.
  - Writes systemd unit files to `/etc/systemd/system/`.
  - Writes persistent configs to `/etc/sub-maker/`.
  - May install/run Certbot or acme.sh for certificates.
- **You do *not* need sudo for testing/demo/CI**:
  - Use `SUB_MAKER_DEMO=1 ./sub-maker --setup` (or in TUI flows). This redirects configs to `/tmp/sub-maker-demo-etc/...` (user-writable), installs sing-box to `~/.local/bin/sing-box` and subconverter to `~/.local/subconverter` (fully user-writable, no root needed), skips real service enabling.
  - You can then run `./sub-maker --serve` as a regular user (using env vars like `SUB_MAKER_NODES_FILE=...` and `SUB_MAKER_PORT=...` to point to demo paths).
  - The `sub-maker` Go binary itself (the TUI + server) does **not** require special privileges to *run* — only the *setup* in real (non-DEMO) mode does for system paths.
- Outbound internet access from the VPS (to download binaries, obtain certificates, etc.).
- (Recommended) A domain name pointed at the VPS IP (for real TLS certificates via ACME). You can use a subdomain (e.g. `proxy.example.com`).
- For Cloudflare DNS-01: a Cloudflare API token with DNS edit permissions for the zone.
- Open inbound ports on your firewall / VPS provider security group for the protocols you enable (default suggestions: 443 for Reality, 8443 for Hysteria2, 9443 for TUIC, 7443 for AnyTLS, 8388 for SS2022, plus 8964 for the subscription, and temporarily 80 for HTTP-01 ACME if used).
- `curl`, `tar`, `gzip` (usually present).

The tool will attempt to install Certbot or acme.sh automatically when needed.

## Quick Start

```bash
# 1. Install the binary (recommended: one-liner pulls pre-built release binary)
curl -fsSL https://raw.githubusercontent.com/tsonubin/sub-maker/main/install.sh | sudo bash

# 2. Run the interactive setup wizard
# - For real production install (writes system files): use sudo
sudo sub-maker --setup
# - For testing/demo (no sudo needed, everything in /tmp):
# SUB_MAKER_DEMO=1 sub-maker --setup

# Follow the TUI prompts:
# - Production domain setup or IP-only / advanced setup
# - Server public IP (auto-detected when possible)
# - Domain for DNS, certificates, and domain-based subscription
# - Email for certificate issuance
# - Subscription token (leave blank to auto-generate)
# - Subscription port (default 8964)
# - Select which of the 5 protocols to enable
# - Per-protocol ports, credentials, SNI, Reality shortId, etc.
# - Certificate mode (certbot-http, acme-http, acme-dns-cf, existing, or self-signed)

# 3. Open the necessary ports in your firewall/provider security group
# Example with ufw:
sudo ufw allow 8964/tcp
sudo ufw allow 443/tcp     # Reality
sudo ufw allow 8443/udp    # Hysteria2
# ... add the other protocol ports you enabled

# 4. Inspect final links and health
sudo sub-maker links
sudo sub-maker doctor
```

In Clash, Clash Meta, Stash, etc., add the URL above as a subscription source. It will contain all your enabled nodes plus a rich set of routing rules.

You can also view the raw node list at `/raw?token=YOUR-TOKEN`.

## Releases and Automated Install

We use GitHub Actions to automatically build and publish release binaries whenever a new version tag (e.g. `v1.0.0`) is pushed.

- Binaries for `linux/amd64` and `linux/arm64` are attached to each release.
- A `checksums.txt` is also provided.

The `install.sh` script now prefers downloading the pre-built binary matching your architecture from the latest release:

```bash
curl -fsSL https://raw.githubusercontent.com/tsonubin/sub-maker/main/install.sh | sudo bash
```

This will place `sub-maker` in `/usr/local/bin` without needing Go installed on the target machine (unless falling back).

The repository is public so that release binaries can be downloaded anonymously by `install.sh` (and `curl`) on any server without requiring `gh auth` or tokens. This is required for the "install on a fresh/random server" use case.

To create a release:
1. `git tag vX.Y.Z`
2. `git push origin vX.Y.Z`
3. The workflow will build, create the GitHub Release, and attach the assets.

See `.github/workflows/release.yml` for details.

## Detailed Usage

### Commands

| Command / Flag | Description | Example |
|---|---|---|
| `setup` / `--setup` | Launch the production-first TUI wizard and apply configuration | `sudo sub-maker setup` |
| `serve` / `--serve` | Run the subscription HTTP server | `sub-maker serve` |
| `nodes` / `--nodes` | Print generated node URIs from `/etc/sub-maker/nodes.txt` | `sudo sub-maker nodes` |
| `links` | Print subscription, raw node, and IP fallback URLs | `sudo sub-maker links` |
| `doctor` | Check config, nodes, DNS, certs, services, and local subscription endpoint | `sudo sub-maker doctor` |
| `status` | Show systemd status for sing-box and sub-maker-sub | `sudo sub-maker status` |
| `restart` | Restart sing-box and sub-maker-sub | `sudo sub-maker restart` |
| `start` / `stop` | Start or stop managed services | `sudo sub-maker start` |
| `--update` | (Planned) Update sing-box and subconverter binaries | `sub-maker --update` |
| `--version` | Print version | `sub-maker --version` |

Environment variables (useful for scripting / containers):

- `SUB_MAKER_DEMO=1` — Run `--setup` in non-interactive demo mode (writes to `/tmp/sub-maker-demo-etc/...`).
- `SUB_MAKER_TOKEN=xxx` — Override the token used by the server (for `--serve`).
- `SUB_MAKER_PORT=8964` — Override the subscription listen port.
- `SUB_MAKER_NODES_FILE=/path/to/nodes.txt` — Override the nodes file used by the server (useful for testing or custom locations).

### Post-Setup Checks

After a successful real setup, services are already started and the local subscription endpoint has been verified. Useful commands:

```bash
sudo sub-maker links
sudo sub-maker doctor
sudo sub-maker status
sudo journalctl -u sub-maker-sub -f
```

**Important**: Re-run `sudo ./sub-maker --setup` if you want to change protocols, ports, credentials, or re-obtain certificates. It is safe and will back up / overwrite as needed.

### DNS And Certificate Notes

- Production mode requires a domain whose DNS points to the detected server IP. If it does not, setup prints the exact `A` or `AAAA` record to create.
- Hysteria2, TUIC, and AnyTLS require readable certificate files.
- Reality does not require a local server certificate, but uses a separate camouflage target domain.
- Shadowsocks 2022 does not require certificates.
- Certbot HTTP-01 is the recommended certificate strategy. It requires DNS to be correct and port 80 to be free during issuance.
- acme.sh HTTP-01 and Cloudflare DNS-01 remain available.
- Existing cert mode validates cert/key files and domain coverage before continuing.
- Certificates are copied or generated at `/etc/sub-maker/certs/{fullchain.pem,privkey.pem}` and referenced by sing-box.

### Using the Raw Nodes

The `/raw` endpoint is useful if you want to:

- Feed the nodes into your own subconverter instance (with custom rules/pref).
- Use with other subscription converters or clients that prefer URI lists.

Example: `http://your-server:8964/raw?token=YOUR-TOKEN`

### Updating

- Re-run `./sub-maker --setup` (it will re-download binaries if you use `--update` once implemented, or you can manually upgrade).
- For sing-box / subconverter updates: the `--update` flag is planned; until then, you can replace the binaries in `/usr/local/bin` and `/opt/subconverter` and restart the units.

## Architecture Overview

- **sing-box** runs as the actual proxy server with multiple inbounds (one per enabled protocol).
- **subconverter** (the fork) is prepared with your nodes.txt + a rich rules config (ACL4SSR_Online_Full by default). It can be used standalone on port 25500 if you want.
- The Go **subscription server** (sub-maker) serves the Clash YAML on 8964. It reads `nodes.txt` and constructs (or can proxy to) the final config containing both your nodes and the common rulesets.
- Configuration is stored in `/etc/sub-maker/config.yaml`.
- Nodes (share links) are in `/etc/sub-maker/nodes.txt`.
- sing-box config: `/etc/sing-box/config.json`.
- Certificates: `/etc/sub-maker/certs/`.
- Systemd units: `/etc/systemd/system/{sing-box,subconverter,sub-maker-sub}.service`.

## Troubleshooting

- **No nodes found / empty subscription**: Make sure you ran `--setup` successfully and the token is correct. Check `/etc/sub-maker/nodes.txt`.
- **Certificate errors**: For HTTP-01, ensure DNS points to this server and nothing else is listening on port 80 during issuance. Use DNS-01 if port 80 is problematic. Check Certbot output or acme.sh logs in `~/.acme.sh/`.
- **Connection refused on 8964**: The sub-maker-sub unit may not be running. Check `systemctl status sub-maker-sub`.
- **Clients can't connect to protocols**: Verify firewall / security group rules, that the correct ports are in the sing-box config, and that the client is using the exact share links from `/raw` or the subscription.
- **Hysteria2 / TUIC / AnyTLS performance issues**: These benefit from a domain + real certificate. Reality works well even without.
- **Demo mode vs real**: In demo the paths are under `/tmp/...` and services are not really started. Use real mode on a VPS for production.
- **sing-box fails to start**: Check `journalctl -u sing-box -xe`. Common causes: port conflicts, missing cert files for TLS protocols, or firewall/provider port blocks. Setup now generates Reality keypairs automatically.

## Development

```bash
git clone https://github.com/tsonubin/sub-maker.git
cd sub-maker
go mod tidy
go build -o sub-maker .
./sub-maker --help

# Quick demo (no root, no real services)
SUB_MAKER_DEMO=1 ./sub-maker --setup
./sub-maker --nodes
SUB_MAKER_DEMO=1 SUB_MAKER_TOKEN=demo-token-123 ./sub-maker --serve &
curl 'http://127.0.0.1:8964/sub?token=demo-token-123' | head -c 800
```

See `Makefile` for cross-build targets (`make build-linux`).

The project follows the design documented in the session `plan.md`.

Contributions are welcome! Please follow the architecture (TUI collects → Apply does the heavy lifting using templates → server is a thin dynamic provider).

## License

**This is a highly restrictive source-available license, not a standard open source license.**

The full terms are in the [LICENSE](LICENSE) file (incorporated with DISCLAIMER.md).

In summary:
- Only browser-based viewing of the source for personal research/education is permitted.
- **Any forking, use, deployment (private or otherwise), modification, or redistribution requires prior express written permission.**

Unauthorized exercise of copyright rights is prohibited. See the LICENSE for complete legal terms, liability limitations, indemnification, and termination provisions.

## Acknowledgments

- [SagerNet/sing-box](https://github.com/SagerNet/sing-box) — the excellent universal proxy platform.
- [tindy2013/subconverter](https://github.com/tindy2013/subconverter) and community forks (especially asdlokj1qpi233) for subscription conversion and rich rule support.
- ACL4SSR project for the excellent common rule sets.
- acme.sh for easy certificate automation.
- The many researchers and developers working on censorship circumvention (Reality, Hysteria2, TUIC, etc.).

---

For a step-by-step tutorial with more explanations, screenshots descriptions, firewall examples, client configuration, and advanced usage, see **GUIDE.md**.

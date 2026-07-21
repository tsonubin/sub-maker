# sub-maker

**sub-maker** is a Rust TUI tool to set up a proxy server on a Linux VPS with [sing-box](https://github.com/SagerNet/sing-box). It walks you through multi-protocol setup, DNS and certificates, systemd services, and a Clash subscription endpoint (default port **8964**).

See [LICENSE](LICENSE) for terms. Restricted license: viewing source for personal research/education only; forking or any use/deployment by others requires prior written permission.

## Install

**Linux VPS (production):**

```bash
curl -fsSL https://raw.githubusercontent.com/tsonubin/sub-maker/main/install.sh | sudo bash
sudo sub-maker --setup
```

**Homebrew (macOS or Linux — great for dev/demo):**

```bash
brew tap tsonubin/sub-maker https://github.com/tsonubin/sub-maker
brew install sub-maker
```

**Debian/Ubuntu `.deb` / Fedora `.rpm`:** download the package from [Releases](https://github.com/tsonubin/sub-maker/releases) (`sub-maker_*.deb` / `sub-maker-*.rpm`) and install with `dpkg` / `dnf`. Details in **[INSTALL.md](INSTALL.md)**.

Also covered there: manual binaries, build from source, firewall, upgrade, uninstall.

Demo without root:

```bash
SUB_MAKER_DEMO=1 sub-maker --setup
```

## What it does

- Full-screen **ratatui** setup / config wizard (step sidebar, forms, review)
- Protocols via sing-box: **VLESS+Reality**, **Hysteria2**, **TUIC v5**, **AnyTLS**, **Shadowsocks 2022**
- DNS checks for domain-based (production) setup
- Certificates: Certbot HTTP-01, acme.sh HTTP-01 / Cloudflare DNS-01, existing files, or self-signed
- Clash subscription server with common rulesets; raw URI list at `/raw`
- Systemd units for sing-box, subconverter, and the subscription service
- Ops commands: `config`, `doctor`, `link`, `links`, `status`, `restart`, `start`, `stop`, `--update`

Longer tutorial: **[GUIDE.md](GUIDE.md)**.

## Prerequisites (summary)

| | Production | Demo (`SUB_MAKER_DEMO=1`) |
|--|------------|---------------------------|
| OS | Linux + systemd | Same or any host for smoke tests |
| Privileges | **sudo/root** | Regular user |
| Paths | `/etc/sub-maker`, `/etc/sing-box`, `/usr/local/bin`, `/opt/subconverter` | `/tmp/sub-maker-demo-etc`, `~/.local/...` |
| Domain | Strongly recommended (ACME + client URLs) | Optional |

Also needed: outbound HTTPS, open firewall ports for enabled protocols, and tools like `curl` / `tar` for install and downloads.

Details: [INSTALL.md](INSTALL.md#requirements).

## After setup

```bash
sudo sub-maker links      # subscription + raw (+ IP fallback)
sudo sub-maker link       # subscription URL only (passcode)
sudo sub-maker doctor
sudo sub-maker status
```

Example firewall (adjust to what you enabled):

```bash
sudo ufw allow 8964/tcp
sudo ufw allow 443/tcp      # Reality
sudo ufw allow 8443/udp     # Hysteria2
# … TUIC 9443/udp, AnyTLS 7443/tcp, SS2022 8388/tcp+udp
```

In Clash / Clash Meta / Stash / similar, add the printed **Subscription** URL. Raw nodes: `http://HOST:PORT/raw?token=YOUR-TOKEN`.

Remote URL only over SSH:

```bash
ssh root@YOUR-SERVER 'sub-maker link'
printf '%s\n' 'YOUR-LINK-PASSCODE' | ssh root@YOUR-SERVER 'sub-maker link --passcode-stdin'
```

Reconfigure later (keeps secrets when fields are left blank):

```bash
sudo sub-maker config
```

## Commands

| Command / flag | Description |
|----------------|-------------|
| `setup` / `--setup` | Interactive production-first wizard and apply |
| `config` / `configure` | Edit existing setup, reapply |
| `serve` / `--serve` | Run subscription HTTP server (used by systemd) |
| `nodes` / `--nodes` | Print node URIs from `nodes.txt` |
| `link` | Subscription URL after passcode check |
| `links` | Subscription, raw, and IP fallback URLs |
| `doctor` | Config, DNS, certs, services, local `/sub` |
| `status` / `start` / `stop` / `restart` | Managed systemd units |
| `--update` | Refresh sing-box + subconverter binaries |
| `--version` | Print version |

**Environment**

| Variable | Purpose |
|----------|---------|
| `SUB_MAKER_DEMO=1` | Non-interactive demo paths; no real service enable |
| `SUB_MAKER_TOKEN` | Token for `--serve` |
| `SUB_MAKER_PORT` | Listen port for `--serve` (default 8964) |
| `SUB_MAKER_NODES_FILE` | Path to `nodes.txt` for `--serve` |

## DNS and certificates

- **Production** mode expects a domain whose DNS points at this server; mismatch shows the exact `A`/`AAAA` record (you can continue anyway).
- **Hysteria2 / TUIC / AnyTLS** need readable cert/key files; **Reality** uses a camouflage target (no local server cert); **SS2022** needs none.
- Preferred cert path: **Certbot HTTP-01** (correct DNS + free port 80). Alternatives: acme.sh HTTP-01, Cloudflare DNS-01, existing files, self-signed.
- Managed files: `/etc/sub-maker/certs/{fullchain.pem,privkey.pem}`.

## Releases

GitHub Actions builds on tags (`v*`) and attaches:

| Asset | Platform |
|-------|----------|
| `sub-maker-linux-amd64` / `arm64` | Linux raw binaries |
| `sub-maker-darwin-amd64` / `arm64` | macOS (Homebrew) |
| `sub-maker_*_{amd64,arm64}.deb` | Debian / Ubuntu |
| `sub-maker-*-1.{x86_64,aarch64}.rpm` | Fedora / RHEL-like |
| `sub-maker.rb` | Homebrew formula (pinned checksums) |
| `checksums.txt` | SHA-256 for the above |

`install.sh` and package managers consume these assets. To cut a release: tag `vX.Y.Z` and push the tag; see `.github/workflows/release.yml` and [packaging/README.md](packaging/README.md).

## Architecture

| Component | Role |
|-----------|------|
| **sing-box** | Proxy inbounds (one per enabled protocol) |
| **subconverter** | Optional converter stack + ACL rules under `/opt/subconverter` |
| **sub-maker** | Rust TUI + subscription server (`/sub`, `/raw`) |
| **config** | `/etc/sub-maker/config.yaml` |
| **nodes** | `/etc/sub-maker/nodes.txt` |
| **sing-box config** | `/etc/sing-box/config.json` |
| **units** | `sing-box`, `subconverter`, `sub-maker-sub` under `/etc/systemd/system/` |

## Updating

```bash
# sub-maker binary
curl -fsSL https://raw.githubusercontent.com/tsonubin/sub-maker/main/install.sh | sudo bash
sudo systemctl restart sub-maker-sub

# proxy stack binaries only
sudo sub-maker --update
sudo sub-maker restart
```

See [INSTALL.md § Upgrade](INSTALL.md#9-upgrade).

## Troubleshooting

| Symptom | Check |
|---------|--------|
| Empty subscription | Setup completed? Token correct? `/etc/sub-maker/nodes.txt` |
| Cert / ACME failures | DNS to this host; port 80 free for HTTP-01; or use DNS-01 |
| Nothing on 8964 | `systemctl status sub-maker-sub`; `journalctl -u sub-maker-sub` |
| Clients fail | Firewall / SG, ports in config, URIs from `links` / `/raw` |
| sing-box won’t start | `journalctl -u sing-box -xe` — ports, missing certs |
| Demo vs prod | Demo under `/tmp/...`, no real systemd enable |

`sudo sub-maker doctor` runs a combined health pass.

## Development

```bash
git clone https://github.com/tsonubin/sub-maker.git
cd sub-maker
cargo build --release
./target/release/sub-maker --help

SUB_MAKER_DEMO=1 cargo run -- --setup
cargo test
make build-linux   # cross targets when configured
```

Stack: Rust, **ratatui** + crossterm TUI, clap CLI. Layout: collect in TUI → `setup::apply` (templates + downloads) → thin HTTP subscription server.

## License

**Highly restrictive source-available terms — not a standard open-source license.**

Full text: [LICENSE](LICENSE) (with [DISCLAIMER.md](DISCLAIMER.md)).

In short: browser viewing of source for personal research/education only. **Forking, use, deployment, modification, or redistribution requires prior written permission.**

## Acknowledgments

- [SagerNet/sing-box](https://github.com/SagerNet/sing-box)
- [tindy2013/subconverter](https://github.com/tindy2013/subconverter) and community forks (e.g. asdlokj1qpi233)
- ACL4SSR rule sets
- acme.sh
- Work on Reality, Hysteria2, TUIC, and related protocols

---

| Doc | Contents |
|-----|----------|
| **[INSTALL.md](INSTALL.md)** | Linux install, firewall, upgrade, uninstall |
| **[GUIDE.md](GUIDE.md)** | Extended tutorial and operations |
| **[DISCLAIMER.md](DISCLAIMER.md)** | Legal disclaimer |

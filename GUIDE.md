# sub-maker Installation Guide & Tutorial

> **STRICT LICENSE — SEE LICENSE FILE FIRST**
>
> This project is subject to a highly restrictive license. See [LICENSE](LICENSE).
>
> Only reading the source on GitHub for research/education is permitted. Forking, cloning to build or run, installing, deploying, or using the software (even privately) is prohibited without prior express written permission from the copyright holder.
>
> If you do not have written permission, do not use this guide or the software.

This guide is provided only for individuals who have received explicit prior written authorization from the copyright holder.

See [LICENSE](LICENSE) and [DISCLAIMER.md](DISCLAIMER.md) for the complete legal terms.

## Table of Contents

1. [Overview and Why Use This Tool](#overview-and-why-use-this-tool)
2. [Prerequisites](#prerequisites)
3. [VPS Preparation](#vps-preparation)
4. [Obtaining the Binary](#obtaining-the-binary)
5. [Running the Setup Wizard](#running-the-setup-wizard)
6. [Certificate Setup (ACME)](#certificate-setup-acme)
7. [Starting and Managing Services](#starting-and-managing-services)
8. [Firewall and Network Configuration](#firewall-and-network-configuration)
9. [Using the Subscription in Clients](#using-the-subscription-in-clients)
10. [Daily Operations and Maintenance](#daily-operations-and-maintenance)
11. [Troubleshooting](#troubleshooting)
12. [Advanced Topics](#advanced-topics)
13. [Security Recommendations](#security-recommendations)

## Overview and Why Use This Tool

sub-maker automates the tedious parts of running a modern proxy server:

- Choosing and configuring multiple strong protocols (the current "best of breed" for different use cases).
- Generating consistent share links.
- Obtaining and renewing real TLS certificates.
- Preparing a high-quality Clash subscription that includes not just nodes but also battle-tested routing rules (China direct, streaming services, AI tools, ad blocking, etc.).
- Running everything under systemd with sensible defaults.

After a single `sudo sub-maker --setup` you get a production-ready server and a subscription URL that you can share with your devices or friends.

The default protocols are:
- VLESS + Reality-Vision (best stealth)
- Hysteria2 (best on bad networks)
- TUIC v5 (great all-rounder QUIC)
- AnyTLS (modern flexible TLS obfuscation)
- Shadowsocks 2022 (lightweight & fast)

All run inside sing-box. The subscription endpoint (port 8964 by default) is powered by a small Go server that can leverage subconverter for even richer output.

## Prerequisites

- A Linux VPS with a public IP.
  - Recommended: Ubuntu 22.04 LTS or 24.04 LTS, or Debian 12.
  - Minimum: 1 vCPU, 512 MB–1 GB RAM, 10+ GB disk (very lightweight).
- **Root or sudo access is required for real installations**. 
  - The setup performs privileged operations: installing binaries system-wide (`/usr/local/bin`, `/opt`), writing systemd units (`/etc/systemd/system`), configs (`/etc/sub-maker`), firewall rules, and certificate management.
  - **You do *not* need sudo privileges just to experiment or test**:
    - Run with `SUB_MAKER_DEMO=1 ./sub-maker --setup` (or set the env before running the binary). This uses user-writable `/tmp/sub-maker-demo-etc/...` paths for configs, installs sing-box to `~/.local/bin/sing-box` and subconverter to `~/.local/subconverter` (fully user-writable, no root at all), skips real `systemctl enable`.
    - The `sub-maker` executable (the Go program providing the TUI and the subscription server) can be run as a regular user.
    - For the server: use env vars like `SUB_MAKER_NODES_FILE=/tmp/.../nodes.txt SUB_MAKER_PORT=8964 SUB_MAKER_TOKEN=xxx ./sub-maker --serve`.
  - If you see permission errors during `--setup` without sudo, that's expected for real mode — just re-run with `sudo`.
- The VPS must be able to make outbound connections (to GitHub for downloads, Let's Encrypt / Cloudflare for certificates, etc.).
- (Strongly recommended) A domain name with DNS pointing to the VPS (A/AAAA record). You can use a cheap domain or a subdomain you already own.
- For the best experience with TLS-based protocols (Hysteria2, TUIC, AnyTLS, SS if you add TLS), a real certificate is very helpful.
- Basic familiarity with SSH and a text editor is helpful but not required (the TUI guides you).

## VPS Preparation

1. **Create the VPS**
   - Choose a provider with good international routing (many users prefer providers with CN2 GIA, or locations in Japan, Singapore, Hong Kong, US West, etc. depending on your location).
   - Use a clean OS image (Ubuntu 24.04 recommended).

2. **Initial hardening (optional but good practice)**
   ```bash
   sudo apt update && sudo apt upgrade -y
   sudo apt install ufw curl wget -y
   sudo ufw default deny incoming
   sudo ufw default allow outgoing
   sudo ufw allow ssh
   sudo ufw --force enable
   ```

3. **Open the ports you will use**
   You can do this after running the wizard (it will print the ports). Typical defaults:
   - 443/tcp (Reality + often also used for other things)
   - 8443/udp (Hysteria2)
   - 9443/udp (TUIC)
   - 7443/tcp (AnyTLS)
   - 8388/tcp and 8388/udp (SS2022)
   - 8964/tcp (the subscription endpoint — you can change this)
   - 80/tcp (temporary, only during ACME HTTP-01 if you choose that method)

   Example:
   ```bash
   sudo ufw allow 8964/tcp
   sudo ufw allow 443/tcp
   sudo ufw allow 8443/udp
   # ... etc
   sudo ufw reload
   ```

   If you use a cloud provider with its own firewall (AWS, GCP, DigitalOcean, etc.), also open the ports in the cloud console / security group.

4. **(Optional) Point a domain at the VPS**
   - Create an A record (and AAAA if you have IPv6) pointing e.g. `proxy.yourdomain.com` → your VPS IP.
   - Wait for DNS propagation (you can test with `dig proxy.yourdomain.com`).

## Obtaining the Binary

The easiest way is to use the provided `install.sh` script, which downloads a pre-built release binary (no Go required on the target server).

### Recommended: One-command install (pre-built from GitHub Releases)

```bash
curl -fsSL https://raw.githubusercontent.com/tsonubin/sub-maker/main/install.sh | sudo bash
```

This auto-detects your architecture (amd64 or arm64), downloads the matching `sub-maker-linux-*` from the latest release, verifies basic integrity where possible, and installs to `/usr/local/bin/sub-maker`.

After it finishes:
```bash
sub-maker --version
sub-maker --help
```

### Alternative: Manual pre-built binary

```bash
ARCH=$(uname -m)
case "$ARCH" in
  x86_64) ARCH=amd64 ;;
  aarch64|arm64) ARCH=arm64 ;;
esac
sudo curl -fL "https://github.com/tsonubin/sub-maker/releases/latest/download/sub-maker-linux-${ARCH}" \
  -o /usr/local/bin/sub-maker
sudo chmod +x /usr/local/bin/sub-maker
sub-maker --version
```

### For development or building from source

Only needed if you want to modify the code or the releases are temporarily unavailable:

```bash
sudo apt update
sudo apt install git golang-go -y   # or a newer Go from https://go.dev/dl

git clone https://github.com/tsonubin/sub-maker.git
cd sub-maker
go mod tidy
go build -o sub-maker .
sudo mv sub-maker /usr/local/bin/
sudo chmod +x /usr/local/bin/sub-maker
```

You can always verify with:
```bash
sub-maker --version
sub-maker --help
```

## Running the Setup Wizard

This is the heart of the tool.

```bash
sudo /usr/local/bin/sub-maker --setup
```

The TUI will guide you through:

1. **Welcome screen** with a short description.
2. **Global settings**
   - Server public address (IP or domain — used in the share links)
   - Domain for ACME certificates (highly recommended)
   - ACME contact email
   - Subscription token (auto-generated if left blank — treat this like a password)
   - Subscription port (default 8964)

3. **Protocol selection**
   - Multi-select the protocols you want (all five are pre-selected).

4. **Per-protocol configuration**
   - Listen port for each protocol (sensible defaults are suggested and validated for conflicts).
   - Remark / display name that will appear in clients.
   - SNI / target domain (important for TLS camouflage and Reality).
   - Passwords / UUIDs (auto-generated or you can paste your own).
   - For Reality: camouflage target domain and shortId. The keypair is generated automatically during setup if you do not provide one.

5. **Certificate strategy**
   - `certbot-http` — recommended if DNS points to this VPS and port 80 is free.
   - `acme-http` — alternate HTTP-01 path through acme.sh.
   - `acme-dns-cf` — best if you use Cloudflare for DNS (you will be asked for a CF API token with DNS edit rights).
   - `existing` — validate and use an existing cert/key pair.
   - `self-signed` — quick but less ideal for TLS protocols (Reality does not need a server cert).

After you confirm, the tool will:

- Download the latest sing-box and a compatible subconverter.
- Generate all node URIs and write `/etc/sub-maker/nodes.txt`.
- Render and write a clean sing-box configuration.
- Check DNS and obtain or validate certificates when selected protocols require them.
- Download a rich ACL4SSR rules file.
- Write systemd unit files.
- Start sing-box and the subscription server.
- Verify the local subscription endpoint.
- Print a summary with the exact subscription URL and operational commands.

**Demo mode (for testing without root or on your laptop)**

```bash
SUB_MAKER_DEMO=1 ./sub-maker --setup
```

This uses `/tmp/sub-maker-demo-etc/...` paths and does not start real services. Very useful for verifying the flow or for CI.

## Certificate Setup

### Certbot HTTP-01 (Recommended)

- Requires that port 80 is not occupied by another service during issuance.
- Requires the domain's DNS record to point at the VPS public IP.
- The tool runs `certbot certonly --standalone` and copies certs to `/etc/sub-maker/certs/`.

### acme.sh HTTP-01

- Alternate HTTP-01 method using `acme.sh --standalone`.
- Also requires DNS to point at the VPS and port 80 to be free.
- After successful issuance, certificates are copied to `/etc/sub-maker/certs/`.

If you later add more subdomains or need to renew, you can re-run the wizard or run Certbot/acme.sh commands manually.

### Cloudflare DNS-01

1. In the Cloudflare dashboard, go to the domain → API Tokens → Create Token → "Edit zone DNS" template.
2. Restrict it to the specific zone (your domain) and give it DNS:Edit permission.
3. Copy the token (it is shown only once).
4. In the TUI, choose `acme-dns-cf` and paste the token when prompted.

This method does not require port 80 to be free and works even behind CDNs / reverse proxies.

**Important**: Never commit the Cloudflare token to git. The TUI stores it (base64-ish) inside the config, but you should protect the config file (`chmod 600 /etc/sub-maker/config.yaml`).

After certificates are obtained, the sing-box config for the TLS protocols (Hysteria2, TUIC, AnyTLS) will reference them. Reality does not use these certs.

## Starting and Managing Services

After real `--setup` finishes successfully, services are already started and the subscription endpoint has been verified.

Useful commands:

```bash
sudo sub-maker links
sudo sub-maker doctor
sudo sub-maker status
sudo sub-maker restart
```

Raw systemd commands still work:

```bash
sudo systemctl status sing-box sub-maker-sub
sudo journalctl -u sub-maker-sub -f
sudo journalctl -u sing-box -f
```

The subconverter unit runs on localhost:25500 by default (not exposed to the internet). You can use it directly if you want to experiment with custom subconverter configurations.

## Firewall and Network Configuration

Minimal example with `ufw` (adjust ports to what you actually enabled):

```bash
sudo ufw allow 22/tcp          # SSH - do not forget this!
sudo ufw allow 8964/tcp        # Subscription
sudo ufw allow 443/tcp         # Reality (and often a good general port)
sudo ufw allow 8443/udp        # Hysteria2
sudo ufw allow 9443/udp        # TUIC
sudo ufw allow 7443/tcp        # AnyTLS
sudo ufw allow 8388/tcp        # SS2022
sudo ufw allow 8388/udp        # SS2022 UDP
sudo ufw --force enable
sudo ufw status
```

On cloud providers, also update the cloud firewall / security groups / VPC firewall rules.

If you use a CDN or reverse proxy in front (Cloudflare, etc.), be aware that some protocols (especially UDP-based ones like Hysteria2 and TUIC) do not work well behind many CDNs. Reality and SS2022 are more tolerant.

## Using the Subscription in Clients

### Clash / Clash Meta / Stash / etc.

1. Add a new subscription.
2. Paste: `http://your-server-ip-or-domain:8964/sub?token=YOUR-TOKEN`
3. (Optional) Give it a name like "My VPS Nodes".
4. Update / download the profile.

You should see 5 (or fewer, if you disabled some) proxies plus several policy groups and a large set of rules.

### Other Clients

- Use the `/raw` URL with any tool that can consume a list of `vless://`, `hysteria2://`, `tuic://`, etc. URIs.
- Many third-party subscription converters also accept the `/raw` URL as input.

### Example Manual Node (for testing)

After running `--nodes` or looking at `/raw`, you will see lines like:

```
vless://uuid@your-ip:443?security=reality&flow=xtls-rprx-vision&fp=chrome&sni=www.microsoft.com&pbk=YOURPUBKEY&sid=shortid#VLESS-Reality
hysteria2://password@your-ip:8443/?sni=your.domain.com#Hysteria2
...
```

Import them individually in clients that support the URI scheme.

## Daily Operations and Maintenance

- **Reconfiguring**: Run `sudo sub-maker setup` again. The setup flow rewrites configs and restarts/verifies services.
- **Health checks**: Run `sudo sub-maker doctor`.
- **Viewing links**: Run `sudo sub-maker links`.
- **Renewing certificates**: Certbot usually installs a timer; acme.sh usually installs a cron job. You can also renew manually and then run `sudo sub-maker restart`.
- **Viewing current nodes**: `sudo sub-maker nodes` or `curl 'http://127.0.0.1:8964/raw?token=...'`.
- **Updating binaries**: Re-run setup or (when implemented) use `--update`. You can also manually download newer sing-box / subconverter releases and replace the files in `/usr/local/bin` and `/opt/subconverter`, then restart units.
- **Backup**: The important files are under `/etc/sub-maker/` (especially `config.yaml` and `nodes.txt`) and your sing-box config. Back them up before major changes.

## Troubleshooting

### "No nodes were found" or empty subscription

- Check that `/etc/sub-maker/nodes.txt` exists and is not empty.
- Verify the token.
- Make sure the sub-maker-sub service is running.

### Certificate issuance fails

- For HTTP-01: nothing else should be listening on port 80. Temporarily stop any web server or use a different port for testing.
- Check the output of Certbot or acme.sh (the tool prints it).
- DNS-01 requires a correct Cloudflare token with zone edit rights.
- After manual fixes, re-run `--setup`.

### sing-box fails to start after setup

Common causes:
- Missing certificate files for TLS protocols → use Certbot/acme.sh, existing cert mode, or self-signed mode and rerun setup.
- Port already in use (another service or previous sing-box instance).
- Bad Reality configuration (wrong private key format, etc.). Setup now generates Reality keypairs automatically; rerun setup if generated config and node links drift.

Check logs: `journalctl -u sing-box -xe`

### Clients connect but everything is slow or blocked

- Try different protocols (Reality for stealth, Hysteria2 when the network is lossy).
- Make sure you are using the subscription (or fresh links) — old links may have been blocked.
- Check whether your VPS IP range is being actively probed (some providers are heavily targeted).
- Consider using a domain + real certificate + uTLS fingerprinting for better camouflage.

### Permission errors when running without sudo

Most operations that write system files require root. Use `sudo`.

## Advanced Topics

### Using a Custom subconverter Configuration

1. Run the server normally.
2. You can also reach the subconverter instance directly on `http://127.0.0.1:25500/sub?...` (if the unit is running).
3. Or point an external subconverter (or sub-web frontend) at your `/raw?token=...` URL as the subscription source and apply your own `config=` / rules.

### Running Behind Cloudflare / a CDN

- UDP protocols (Hysteria2, TUIC) generally do **not** work reliably.
- Reality + TCP-based fallbacks are more CDN-friendly.
- You may need to use Cloudflare Spectrum (paid) or Argo Tunnel / Cloudflare Tunnel for some protocols.

### Multiple Users / Different Tokens

Currently the tool is designed for a single subscription token. You can manually edit the nodes or run multiple instances if you need strict per-user isolation.

### Monitoring

- Use `systemctl status` + journalctl.
- You can add Prometheus exporters for sing-box (it has a built-in Clash API compatible endpoint) or simple health checks against the subscription URL.

### Custom Rules

Edit `/etc/sub-maker/acl4ssr.ini` (or replace it with your own) and restart the sub-maker-sub service (or the subconverter unit if you are using it directly). Many people maintain their own rule sets on GitHub and reference them by URL in subconverter configs.

## Security Recommendations

- Keep the subscription token secret (treat it like a password).
- Use a strong, unique token (the wizard generates one).
- Restrict SSH to key-only auth and (ideally) to specific IPs or use fail2ban.
- Only open the ports you actually need.
- Keep the system and the binaries updated.
- Consider running the subscription server on a non-standard high port and using a reverse proxy with additional authentication if you expose it publicly.
- For Reality, never reuse shortIds across unrelated deployments.
- Review the generated sing-box config and units before enabling them in production.

## Getting Help

- Check the logs first (`journalctl`).
- Re-run `--setup` — it is the quickest way to regenerate consistent configuration.
- Open an issue on the repository with the output of `--version`, the relevant logs, and a description of what you were trying to do.

---

You now have a fully functional, well-documented proxy server with excellent obfuscation and a high-quality subscription. Enjoy your uncensored internet!

If you have suggestions to improve this guide or the tool itself, contributions are very welcome.

# Packaging

Thin wrappers around **GitHub Release** binaries. The CLI itself still performs production setup (`sing-box`, certs, systemd).

## Artifacts (per `v*` release)

| Asset | Use |
|-------|-----|
| `sub-maker-linux-amd64` / `arm64` | Raw binary / `install.sh` |
| `sub-maker-darwin-amd64` / `arm64` | Homebrew on macOS |
| `sub-maker_*_amd64.deb` / `arm64.deb` | `apt` / `dpkg` |
| `sub-maker-*-1.x86_64.rpm` / `*.aarch64.rpm` | `dnf` / `rpm` |
| `checksums.txt` | SHA-256 of the above |
| `Formula` published as `sub-maker.rb` | Optional downloadable Homebrew formula |

## Local package build

```bash
# After make build-linux (and optional darwin builds):
./scripts/package-linux.sh 0.2.0 .
./scripts/generate-homebrew-formula.sh 0.2.0 .
```

Requires [nfpm](https://nfpm.goreleaser.com/) (`go install github.com/goreleaser/nfpm/v2/cmd/nfpm@latest`).

## Homebrew

Personal tap: **[tsonubin/homebrew-tap](https://github.com/tsonubin/homebrew-tap)**

```bash
brew tap tsonubin/tap
brew install sub-maker
```

On each `v*` release, CI:

1. Generates `sub-maker.rb` (version + SHA256s) into the release assets
2. Publishes it to `tsonubin/homebrew-tap` via `scripts/publish-homebrew-tap.sh`

**CI auth (sub-maker repo secrets)**

| Secret | Purpose |
|--------|---------|
| `HOMEBREW_TAP_SSH_KEY` | **Preferred.** Private half of a **write** deploy key on `tsonubin/homebrew-tap` |
| `HOMEBREW_TAP_TOKEN` | Optional fallback classic PAT with `repo` scope |

Without either secret, the formula is still attached to the GitHub Release as `sub-maker.rb`; the tap is not updated automatically.

## apt / dnf (direct packages)

No third-party apt/dnf **repository** is published yet. Install the package asset from the release:

```bash
# Debian / Ubuntu
curl -fLO https://github.com/tsonubin/sub-maker/releases/latest/download/sub-maker_VERSION_amd64.deb
sudo dpkg -i sub-maker_VERSION_amd64.deb

# Fedora / RHEL-like
curl -fLO https://github.com/tsonubin/sub-maker/releases/latest/download/sub-maker-VERSION-1.x86_64.rpm
sudo dnf install ./sub-maker-VERSION-1.x86_64.rpm
```

Replace `VERSION` and arch as appropriate. A hosted apt/yum repo can be added later on top of these same packages.

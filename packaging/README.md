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

Formula lives at [`Formula/sub-maker.rb`](../Formula/sub-maker.rb) so this repo can be tapped:

```bash
brew tap tsonubin/sub-maker https://github.com/tsonubin/sub-maker
brew install sub-maker
```

Or install the formula file from a release:

```bash
brew install --formula https://github.com/tsonubin/sub-maker/releases/latest/download/sub-maker.rb
```

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

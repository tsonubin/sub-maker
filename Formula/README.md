# Formulae live in the personal tap

Homebrew formulae for this project are maintained in:

**https://github.com/tsonubin/homebrew-tap**

```bash
brew tap tsonubin/tap
brew install sub-maker
```

Release CI generates `sub-maker.rb` (with version + SHA256s) and publishes it to that tap when `HOMEBREW_TAP_SSH_KEY` (or `HOMEBREW_TAP_TOKEN`) is configured on this repo.

Template and generator: `packaging/homebrew/`, `scripts/generate-homebrew-formula.sh`.

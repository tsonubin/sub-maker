# Formulae live in the personal tap

Homebrew formulae for this project are maintained in:

**https://github.com/tsonubin/homebrew-tap**

```bash
brew tap tsonubin/tap
brew install sub-maker
```

Release CI generates `sub-maker.rb` (with version + SHA256s) and publishes it to that tap when the `HOMEBREW_TAP_TOKEN` secret is configured.

Template and generator: `packaging/homebrew/`, `scripts/generate-homebrew-formula.sh`.

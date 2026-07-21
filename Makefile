# sub-maker Makefile (Rust cross build + packages)
CARGO := cargo
BINARY := sub-maker
# Strip leading v from tags for package versions
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null | sed 's/^v//' || echo 0.0.0-dev)
PKG_VERSION := $(shell echo $(VERSION) | sed 's/-.*//')

.PHONY: build test clean install demo build-linux build-darwin package formula

build:
	$(CARGO) build --release
	cp target/release/$(BINARY) ./$(BINARY)

build-linux:
	$(CARGO) build --release --target x86_64-unknown-linux-gnu
	$(CARGO) build --release --target aarch64-unknown-linux-gnu
	cp target/x86_64-unknown-linux-gnu/release/$(BINARY) $(BINARY)-linux-amd64
	cp target/aarch64-unknown-linux-gnu/release/$(BINARY) $(BINARY)-linux-arm64
	chmod 755 $(BINARY)-linux-amd64 $(BINARY)-linux-arm64

# Requires macOS host (or osxcross). Builds arm64 + amd64.
build-darwin:
	$(CARGO) build --release --target aarch64-apple-darwin
	$(CARGO) build --release --target x86_64-apple-darwin
	cp target/aarch64-apple-darwin/release/$(BINARY) $(BINARY)-darwin-arm64
	cp target/x86_64-apple-darwin/release/$(BINARY) $(BINARY)-darwin-amd64
	chmod 755 $(BINARY)-darwin-arm64 $(BINARY)-darwin-amd64

# Needs nfpm on PATH. Uses existing *-linux-* binaries in cwd.
package:
	./scripts/package-linux.sh $(PKG_VERSION) .

formula:
	./scripts/generate-homebrew-formula.sh $(PKG_VERSION) . dist/sub-maker.rb

test:
	$(CARGO) test

demo:
	SUB_MAKER_DEMO=1 $(CARGO) run -- --setup

clean:
	$(CARGO) clean
	rm -f $(BINARY) $(BINARY)-* $(BINARY)_*.deb $(BINARY)-*.rpm
	rm -f checksums.txt

install: build
	install -m 755 $(BINARY) /usr/local/bin/$(BINARY)

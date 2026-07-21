# sub-maker Makefile (Rust cross build + test)
CARGO := cargo
BINARY := sub-maker
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo dev)
export CARGO_PKG_VERSION_OVERRIDE := $(VERSION)

.PHONY: build test clean install demo build-linux

build:
	$(CARGO) build --release
	cp target/release/$(BINARY) ./$(BINARY)

build-linux:
	$(CARGO) build --release --target x86_64-unknown-linux-gnu
	$(CARGO) build --release --target aarch64-unknown-linux-gnu
	cp target/x86_64-unknown-linux-gnu/release/$(BINARY) $(BINARY)-linux-amd64
	cp target/aarch64-unknown-linux-gnu/release/$(BINARY) $(BINARY)-linux-arm64

test:
	$(CARGO) test

demo:
	SUB_MAKER_DEMO=1 $(CARGO) run -- --setup

clean:
	$(CARGO) clean
	rm -f $(BINARY) $(BINARY)-*

install: build
	install -m 755 $(BINARY) /usr/local/bin/$(BINARY)

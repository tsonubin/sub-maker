# sub-maker Makefile (simple cross build + test)
GO := go
BINARY := sub-maker
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo dev)
LDFLAGS := -ldflags "-X main.version=$(VERSION) -s -w"

.PHONY: build test clean install demo

build:
	$(GO) build $(LDFLAGS) -o $(BINARY) .

build-linux:
	CGO_ENABLED=0 GOOS=linux GOARCH=amd64 $(GO) build $(LDFLAGS) -o $(BINARY)-linux-amd64 .
	CGO_ENABLED=0 GOOS=linux GOARCH=arm64 $(GO) build $(LDFLAGS) -o $(BINARY)-linux-arm64 .

test:
	$(GO) test ./internal/generator -v
	$(GO) test ./... 2>/dev/null || true

demo:
	SUB_MAKER_DEMO=1 $(GO) run . --setup

clean:
	rm -f $(BINARY) $(BINARY)-*

install: build
	install -m 755 $(BINARY) /usr/local/bin/$(BINARY)

# Example: make build-linux ; ls -l *linux*

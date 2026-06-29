.PHONY: all build clean test install help dist icons auth-proxy-build

APP_NAME = yandex-messenger
CARGO_BIN ?= $(shell command -v cargo || echo $(HOME)/.cargo/bin/cargo)

# Ensure cargo is on PATH for parallel builds
export PATH := $(HOME)/.cargo/bin:$(PATH)
export CARGO_HOME := $(HOME)/.cargo

all: build

build:
	@echo "Building $(APP_NAME)..."
	$(CARGO_BIN) build --release
	@echo "Build complete: target/release/$(APP_NAME)"

icons:
	./scripts/prepare-icons.sh

dist:
	./scripts/build-dist.sh

auth-proxy-build:
	~/.cargo/bin/cargo build --release --manifest-path auth-proxy/Cargo.toml

run:
	$(CARGO_BIN) run

run-release:
	$(CARGO_BIN) run --release

test:
	$(CARGO_BIN) test

check:
	$(CARGO_BIN) clippy -- -D warnings
	$(CARGO_BIN) fmt --check

fmt:
	$(CARGO_BIN) fmt

clean:
	@$(HOME)/.cargo/bin/cargo clean

install: build
	install -d /usr/local/bin
	install -m 755 target/release/$(APP_NAME) /usr/local/bin/
	@echo "Installed to /usr/local/bin/$(APP_NAME)"

uninstall:
	rm -f /usr/local/bin/$(APP_NAME)

help:
	@echo "Available targets:"
	@echo "  build        - Build in release mode"
	@echo "  run          - Run in debug mode"
	@echo "  run-release  - Run in release mode"
	@echo "  test         - Run tests"
	@echo "  check        - Run clippy and format check"
	@echo "  fmt          - Format code"
	@echo "  clean        - Clean build artifacts"
	@echo "  install      - Install to /usr/local/bin/"
	@echo "  uninstall    - Remove installed binary"
	@echo "  icons        - Generate hicolor icon set"
	@echo "  dist         - Build distributable artifacts"
	@echo "  auth-proxy-build - Build optional auth-proxy service"

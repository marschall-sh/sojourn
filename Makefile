.PHONY: help build release clean test fmt clippy doc install uninstall dev

help:
	@echo "sojourn Makefile"
	@echo ""
	@echo "Available targets:"
	@echo "  make dev        - Build debug version and run"
	@echo "  make build      - Build debug version"
	@echo "  make release    - Build optimized release version"
	@echo "  make install    - Install globally"
	@echo "  make uninstall  - Remove global installation"
	@echo "  make clean      - Clean build artifacts"
	@echo "  make test       - Run tests"
	@echo "  make fmt        - Format code with rustfmt"
	@echo "  make clippy     - Run linter (clippy)"
	@echo "  make doc        - Generate and open documentation"

dev: build
	./target/debug/sojourn

build:
	cargo build

release:
	cargo build --release

install: release
	cargo install --path .

uninstall:
	cargo uninstall sojourn

clean:
	cargo clean

test:
	cargo test

fmt:
	cargo fmt

clippy:
	cargo clippy -- -D warnings

doc:
	cargo doc --open

all: fmt clippy test release
	@echo "Build complete! Binary at ./target/release/sojourn"

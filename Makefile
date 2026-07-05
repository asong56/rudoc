# Rudoc — Makefile
# Convenience targets for building, testing, and cross-compiling.
#
# Requirements:
#   Local:  Rust stable, musl-tools (Linux), cargo-zigbuild (optional)
#   Cross:  `cargo install cross` + Docker running

.PHONY: all build release test lint fmt clean install
.PHONY: build-linux build-linux-arm build-windows build-macos
.PHONY: dist dist-checksums

BINARY  := rudoc
VERSION := $(shell grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
DIST    := dist

# ── Default: debug build ────────────────────────────────────────────────────
all: build

build:
	cargo build --no-default-features

build-pdf:
	cargo build --features pdf

release:
	cargo build --release --no-default-features

release-pdf:
	cargo build --release --features pdf

# ── Tests ───────────────────────────────────────────────────────────────────
test:
	cargo test --no-default-features
	cargo test --features pdf

test-verbose:
	cargo test --no-default-features -- --nocapture
	cargo test --features pdf -- --nocapture

# ── Lint / format ───────────────────────────────────────────────────────────
lint:
	cargo clippy --no-default-features -- -D warnings
	cargo clippy --features pdf -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

# ── Install locally ─────────────────────────────────────────────────────────
install: release
	install -m 755 target/release/$(BINARY) /usr/local/bin/$(BINARY)

# ── Cross-platform builds (using `cross` + Docker) ──────────────────────────
# Install cross: cargo install cross
# Requires Docker to be running.

build-linux: $(DIST)
	# x86_64 musl (fully static, runs on any Linux ≥ kernel 2.6.32)
	CC_x86_64_unknown_linux_musl=musl-gcc \
	  cargo build --release --features pdf \
	  --target x86_64-unknown-linux-musl
	cp target/x86_64-unknown-linux-musl/release/$(BINARY) \
	   $(DIST)/$(BINARY)-linux-x86_64
	strip $(DIST)/$(BINARY)-linux-x86_64

build-linux-arm: $(DIST)
	# aarch64 musl via cross (needs Docker)
	cross build --release --features pdf \
	  --target aarch64-unknown-linux-musl
	cp target/aarch64-unknown-linux-musl/release/$(BINARY) \
	   $(DIST)/$(BINARY)-linux-aarch64
	-aarch64-linux-gnu-strip $(DIST)/$(BINARY)-linux-aarch64 2>/dev/null || true

build-windows: $(DIST)
	# Windows x86_64 via cross (needs Docker) or native Windows runner
	cross build --release --features pdf \
	  --target x86_64-pc-windows-gnu
	cp target/x86_64-pc-windows-gnu/release/$(BINARY).exe \
	   $(DIST)/$(BINARY)-windows-x86_64.exe

build-macos: $(DIST)
	# macOS — must be run on a macOS host (not cross-compilable without osxcross)
	cargo build --release --features pdf \
	  --target aarch64-apple-darwin
	cp target/aarch64-apple-darwin/release/$(BINARY) \
	   $(DIST)/$(BINARY)-macos-aarch64
	strip $(DIST)/$(BINARY)-macos-aarch64
	@echo "For Intel Mac:"
	@echo "  cargo build --release --features pdf --target x86_64-apple-darwin"

# ── Distribution archive ─────────────────────────────────────────────────────
$(DIST):
	mkdir -p $(DIST)

dist-linux: build-linux build-linux-arm
	cd $(DIST) && tar -czf $(BINARY)-$(VERSION)-linux-x86_64.tar.gz $(BINARY)-linux-x86_64
	cd $(DIST) && tar -czf $(BINARY)-$(VERSION)-linux-aarch64.tar.gz $(BINARY)-linux-aarch64

dist-windows: build-windows
	cd $(DIST) && zip $(BINARY)-$(VERSION)-windows-x86_64.zip $(BINARY)-windows-x86_64.exe

dist-macos: build-macos
	cd $(DIST) && tar -czf $(BINARY)-$(VERSION)-macos-aarch64.tar.gz $(BINARY)-macos-aarch64

dist-checksums: $(DIST)
	cd $(DIST) && sha256sum *.tar.gz *.zip 2>/dev/null | tee SHA256SUMS

dist: dist-linux dist-windows dist-checksums
	@echo ""
	@echo "Distribution packages in $(DIST)/:"
	@ls -lh $(DIST)/

# ── Clean ───────────────────────────────────────────────────────────────────
clean:
	cargo clean
	rm -rf $(DIST)

# ── Quick smoke test of the binary ──────────────────────────────────────────
smoke:
	@echo "Version:"
	@./target/release/$(BINARY) --version
	@echo ""
	@echo "md → html:"
	@printf '# Test\n**bold**\n- item\n' | ./target/release/$(BINARY) -f md -t html
	@echo ""
	@echo "json → xml:"
	@printf '{"x":1}' | ./target/release/$(BINARY) -f json -t xml

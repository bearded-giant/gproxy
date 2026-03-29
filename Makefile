.PHONY: build build-release lint test check run install install-app clean kill

# dev build (debug)
build:
	cargo build --workspace

# release build (all crates + tauri app bundle)
build-release:
	cargo build --release --workspace
	rm -rf target/release/build/giant-proxy-ui-*/out/tauri-codegen-assets/
	rm -f crates/giant-proxy-ui/src-tauri/binaries/giantd-* crates/giant-proxy-ui/src-tauri/binaries/giant-proxy-*
	cd crates/giant-proxy-ui && pnpm tauri build

# format + clippy
lint:
	cargo fmt --check
	cargo clippy --workspace -- -D warnings

# format in-place
fmt:
	cargo fmt --all

# run all tests
test:
	cargo test --workspace

# lint + test (same as CI)
check: lint test

# run daemon in foreground (dev build)
run:
	cargo run -p giantd -- --foreground

# install CLI binaries to ~/.cargo/bin
install:
	cargo install --path crates/giantd
	cargo install --path crates/giant-proxy

# full release build, kill running processes, install app + binaries
install-app: build-release
	-pkill -f giant-proxy-ui
	-pkill -f giantd
	cp -r target/release/bundle/macos/Giant\ Proxy.app /Applications/
	cp target/release/giantd /opt/homebrew/bin/
	cp target/release/giant-proxy /opt/homebrew/bin/
	@echo "installed Giant Proxy.app to /Applications and binaries to /opt/homebrew/bin"

# kill all running processes
kill:
	-pkill -f giant-proxy-ui
	-pkill -f giantd

# remove build artifacts
clean:
	cargo clean

# Dev Workflow

## Getting Started

```bash
git clone https://github.com/bearded-giant/gproxy.git && cd gproxy
```

Install the daemon and CLI (builds from source, puts binaries in `~/.cargo/bin/`):

```bash
cargo install --path crates/giantd && cargo install --path crates/giant-proxy
```

Initialize config dir and install the CA cert (will prompt for password):

```bash
giant-proxy init
```

Import your Proxyman rules (if you have them):

```bash
giant-proxy profile import-proxyman
```

Start proxying:

```bash
giant-proxy on
```

## Running Tests

```bash
cargo test --workspace
```

Lint + format check (same as CI):

```bash
cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo test --workspace
```

## Kill, Rebuild, Relaunch the Tauri App

```bash
# kill running instance
pkill -9 -f giant-proxy-ui

# rebuild (from repo root)
cd crates/giant-proxy-ui && pnpm tauri build

# launch
/Users/bryan/dev/gproxy/target/release/bundle/macos/Giant\ Proxy.app/Contents/MacOS/giant-proxy-ui &
```

One-liner:

```bash
pkill -9 -f giant-proxy-ui; cd /Users/bryan/dev/gproxy/crates/giant-proxy-ui && pnpm tauri build && /Users/bryan/dev/gproxy/target/release/bundle/macos/Giant\ Proxy.app/Contents/MacOS/giant-proxy-ui &
```

## Build CLI Only

```bash
cargo build -p giant-proxy-cli
```

## Build Daemon Only

```bash
cargo build -p giantd
```

## Import Proxyman Config

```bash
cargo run -p giant-proxy-cli -- import /path/to/proxyman_map_remote_rules.config --all
```

## Check Status

```bash
cargo run -p giant-proxy-cli -- status
```

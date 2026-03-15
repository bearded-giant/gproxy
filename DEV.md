# Dev Workflow

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

#!/bin/bash
set -euo pipefail

# read current base version from the UI crate's Cargo.toml
BASE=$(grep '^version' crates/giant-proxy-ui/src-tauri/Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

# increment dev build counter
COUNTER_FILE=".dev-build-number"
if [ -f "$COUNTER_FILE" ]; then
    N=$(cat "$COUNTER_FILE")
    N=$((N + 1))
else
    N=1
fi
echo "$N" > "$COUNTER_FILE"

DEV_VERSION="${BASE}-dev.${N}"
echo "Building dev version: $DEV_VERSION"

# stamp dev version into tauri.conf.json only (Cargo.toml stays at base)
sed -i '' "s/\"version\": \".*\"/\"version\": \"$DEV_VERSION\"/" crates/giant-proxy-ui/src-tauri/tauri.conf.json

# build workspace then Tauri app
cargo build --workspace --release
cd crates/giant-proxy-ui && pnpm tauri build 2>&1
cd ../..

# restore tauri.conf.json
sed -i '' "s/\"version\": \".*\"/\"version\": \"$BASE\"/" crates/giant-proxy-ui/src-tauri/tauri.conf.json

echo ""
echo "DMG: crates/giant-proxy-ui/src-tauri/target/release/bundle/dmg/Giant Proxy_${DEV_VERSION}_aarch64.dmg"

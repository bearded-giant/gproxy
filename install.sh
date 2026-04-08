#!/usr/bin/env bash
set -euo pipefail

REPO="bearded-giant/gproxy"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin) OS="apple-darwin" ;;
  linux) OS="unknown-linux-gnu" ;;
  *) echo "unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) echo "unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"

if [ -z "${VERSION:-}" ]; then
  VERSION=$(curl -sSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
fi

echo "installing gproxy $VERSION for $TARGET"

URL="https://github.com/$REPO/releases/download/$VERSION/gproxy-$VERSION-$TARGET.tar.gz"

mkdir -p "$INSTALL_DIR"
curl -sSL "$URL" | tar xz -C "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/giantd" "$INSTALL_DIR/gproxy"

echo "installed to $INSTALL_DIR"

if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
  echo ""
  echo "add to your shell profile:"
  echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo ""
echo "run: gproxy init"

#!/usr/bin/env bash
# Install the latest `mreview` binary (Manual Reviewer CLI) to ~/.local/bin
# Usage: bash <(curl -fsSL https://raw.githubusercontent.com/jel1yspot/manual-reviewer-zed/main/scripts/install.sh)

set -euo pipefail

REPO="${MREVIEW_REPO:-jel1yspot/manual-reviewer-zed}"
BIN_DIR="${MREVIEW_BIN_DIR:-$HOME/.local/bin}"

uname_s="$(uname -s)"
uname_m="$(uname -m)"
case "$uname_s/$uname_m" in
  Darwin/arm64)   target="aarch64-apple-darwin" ;;
  Darwin/x86_64)  target="x86_64-apple-darwin" ;;
  Linux/x86_64)   target="x86_64-unknown-linux-gnu" ;;
  Linux/aarch64)  target="aarch64-unknown-linux-gnu" ;;
  *)              echo "Unsupported platform: $uname_s/$uname_m" >&2; exit 1 ;;
esac

api_url="https://api.github.com/repos/$REPO/releases/latest"
echo "Looking up latest release of $REPO..."
asset_url=$(curl -fsSL "$api_url" \
  | grep -oE "https://[^\"]*mreview-[^\"]*${target}\\.tar\\.gz" \
  | head -n1 || true)
if [ -z "$asset_url" ]; then
  echo "No release asset matching target $target found." >&2
  echo "Tip: build from source with: cd manual-reviewer-zed && cargo install --path crates/manual-reviewer-cli" >&2
  exit 1
fi

mkdir -p "$BIN_DIR"
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

echo "Downloading $asset_url"
curl -fsSL -o "$tmpdir/release.tar.gz" "$asset_url"
tar -xzf "$tmpdir/release.tar.gz" -C "$tmpdir"
install -m 0755 "$tmpdir/mreview" "$BIN_DIR/mreview"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "Note: $BIN_DIR is not in your PATH. Add this to your shell rc:"
     echo "  export PATH=\"$BIN_DIR:\$PATH\""
     ;;
esac

echo "Installed: $BIN_DIR/mreview"
"$BIN_DIR/mreview" --version || true

# Merge Zed config (tasks.json + keymap.json) into ~/.config/zed/
echo
echo "Merging Zed task + keymap into ~/.config/zed/ ..."
"$BIN_DIR/mreview" config-zed || {
  echo "Note: config-zed failed; you can run it later with: mreview config-zed" >&2
}

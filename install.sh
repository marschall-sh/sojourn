#!/usr/bin/env bash
# sojourn installer
# Usage: curl -fsSL https://raw.githubusercontent.com/marschall-sh/sojourn/main/install.sh | bash
set -euo pipefail

REPO="marschall-sh/sojourn"
BINARY="sojourn"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# ── Detect OS + architecture ──────────────────────────────────────────────────
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin)
    case "$ARCH" in
      arm64)   ARTIFACT="sojourn-macos-arm64" ;;
      *)       echo "✗ Unsupported Mac architecture: $ARCH (only Apple Silicon is supported)"; exit 1 ;;
    esac
    ;;
  linux)
    case "$ARCH" in
      x86_64)          ARTIFACT="sojourn-linux-amd64" ;;
      aarch64|arm64)   ARTIFACT="sojourn-linux-arm64" ;;
      *)       echo "✗ Unsupported Linux architecture: $ARCH (x86-64 and arm64 are supported)"; exit 1 ;;
    esac
    ;;
  *)
    echo "✗ Unsupported OS: $OS"
    exit 1
    ;;
esac

# ── Resolve latest release URL ────────────────────────────────────────────────
TARBALL="${ARTIFACT}.tar.gz"
BASE_URL="https://github.com/${REPO}/releases/latest/download"
DOWNLOAD_URL="${BASE_URL}/${TARBALL}"

echo "  sojourn installer"
echo "  ─────────────────────────────────────"
echo "  Platform : $OS / $ARCH"
echo "  Artifact : $ARTIFACT"
echo "  Install  : $INSTALL_DIR/$BINARY"
echo ""

# ── Download + extract ────────────────────────────────────────────────────────
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "→ Downloading $TARBALL..."
if ! curl -fsSL "$DOWNLOAD_URL" -o "$TMP/$TARBALL"; then
  echo "✗ Download failed. Check that $REPO has a published release."
  exit 1
fi

echo "→ Extracting..."
tar xzf "$TMP/$TARBALL" -C "$TMP"

# ── Install ───────────────────────────────────────────────────────────────────
mkdir -p "$INSTALL_DIR"
mv "$TMP/$BINARY" "$INSTALL_DIR/$BINARY"
chmod +x "$INSTALL_DIR/$BINARY"

echo ""
echo "✓ sojourn $("$INSTALL_DIR/$BINARY" --version 2>/dev/null || echo "(installed)") → $INSTALL_DIR/$BINARY"

# ── PATH hint if needed ───────────────────────────────────────────────────────
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
  SHELL_NAME=$(basename "${SHELL:-bash}")
  case "$SHELL_NAME" in
    zsh)   PROFILE="~/.zshrc" ;;
    fish)  PROFILE="~/.config/fish/config.fish" ;;
    *)     PROFILE="~/.bashrc" ;;
  esac
  echo ""
  echo "  ⚠  $INSTALL_DIR is not in your PATH."
  echo "     Add this line to $PROFILE:"
  echo ""
  echo "       export PATH=\"\$HOME/.local/bin:\$PATH\""
  echo ""
fi

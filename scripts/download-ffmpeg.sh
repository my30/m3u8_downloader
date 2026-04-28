#!/usr/bin/env bash
# scripts/download-ffmpeg.sh
# Downloads static ffmpeg binaries for all supported platforms into src-tauri/binaries/
# Usage:
#   ./scripts/download-ffmpeg.sh              # downloads for current platform only
#   ./scripts/download-ffmpeg.sh --all        # downloads for all platforms (macOS + Windows)
#
# Requires: curl, unzip, tar (all standard on macOS/Linux)

set -euo pipefail

BINARIES_DIR="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/binaries"
mkdir -p "$BINARIES_DIR"

# Get the Rust target triple for the current machine
CURRENT_TRIPLE="$(rustc -vV 2>/dev/null | awk '/host:/{print $2}')"
echo "Current target triple: $CURRENT_TRIPLE"

download_macos_arm64() {
  echo "Downloading ffmpeg for macOS arm64 (aarch64-apple-darwin)..."
  local url="https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip"
  local zip="$BINARIES_DIR/ffmpeg-mac-arm64.zip"
  curl -L "$url" -o "$zip"
  unzip -jo "$zip" "ffmpeg" -d "$BINARIES_DIR"
  mv "$BINARIES_DIR/ffmpeg" "$BINARIES_DIR/ffmpeg-aarch64-apple-darwin"
  chmod +x "$BINARIES_DIR/ffmpeg-aarch64-apple-darwin"
  rm "$zip"
  echo "  ✓ ffmpeg-aarch64-apple-darwin"
}

download_macos_x86() {
  echo "Downloading ffmpeg for macOS x86_64 (x86_64-apple-darwin)..."
  # evermeet.cx only builds for macOS (universal or arm64); use the same binary for x86
  # For a proper x86 build, BtbN builds can be used in CI
  local url="https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip"
  local zip="$BINARIES_DIR/ffmpeg-mac-x86.zip"
  curl -L "$url" -o "$zip"
  unzip -jo "$zip" "ffmpeg" -d "$BINARIES_DIR"
  mv "$BINARIES_DIR/ffmpeg" "$BINARIES_DIR/ffmpeg-x86_64-apple-darwin"
  chmod +x "$BINARIES_DIR/ffmpeg-x86_64-apple-darwin"
  rm "$zip"
  echo "  ✓ ffmpeg-x86_64-apple-darwin"
}

download_windows_x86() {
  echo "Downloading ffmpeg for Windows x64 (x86_64-pc-windows-msvc)..."
  # BtbN builds: latest master lgpl-essentials build
  local url="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-lgpl-essentials.zip"
  local zip="$BINARIES_DIR/ffmpeg-win64.zip"
  curl -L "$url" -o "$zip"
  # The zip contains a directory like ffmpeg-master-latest-win64-lgpl-essentials/bin/ffmpeg.exe
  unzip -jo "$zip" "*/bin/ffmpeg.exe" -d "$BINARIES_DIR"
  mv "$BINARIES_DIR/ffmpeg.exe" "$BINARIES_DIR/ffmpeg-x86_64-pc-windows-msvc.exe"
  rm "$zip"
  echo "  ✓ ffmpeg-x86_64-pc-windows-msvc.exe"
}

if [[ "${1:-}" == "--all" ]]; then
  download_macos_arm64
  download_macos_x86
  download_windows_x86
else
  case "$CURRENT_TRIPLE" in
    aarch64-apple-darwin) download_macos_arm64 ;;
    x86_64-apple-darwin)  download_macos_x86 ;;
    x86_64-pc-windows-*)  download_windows_x86 ;;
    *)
      echo "Unknown triple '$CURRENT_TRIPLE', downloading macOS arm64 as fallback..."
      download_macos_arm64
      ;;
  esac
fi

echo ""
echo "Done. Binaries in: $BINARIES_DIR"
ls -lh "$BINARIES_DIR"/ffmpeg* 2>/dev/null || true

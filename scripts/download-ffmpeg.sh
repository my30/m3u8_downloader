#!/usr/bin/env bash
# scripts/download-ffmpeg.sh
# Downloads a static ffmpeg binary as a Tauri sidecar into src-tauri/binaries/ffmpeg-<target-triple>[.exe]
#
# Usage:
#   ./scripts/download-ffmpeg.sh                       # current host triple
#   ./scripts/download-ffmpeg.sh aarch64-apple-darwin  # specific triple(s)
#   ./scripts/download-ffmpeg.sh --all                 # all release triples
#
# Sources (both native per-architecture builds):
#   macOS   : Martin Riedl's build server (https://ffmpeg.martin-riedl.de) — GPL build
#   Windows : BtbN/FFmpeg-Builds (https://github.com/BtbN/FFmpeg-Builds) — LGPL build
#
# Integrity:
#   - Windows zips are verified against BtbN's published checksums.sha256.
#   - Any download can be pinned with FFMPEG_SHA256_<TRIPLE> (dashes -> underscores), e.g.
#       FFMPEG_SHA256_aarch64_apple_darwin=<sha256> ./scripts/download-ffmpeg.sh aarch64-apple-darwin
#   - macOS binaries are checked to be Mach-O for the expected architecture.
#
# Requires: curl, and unzip or 7z.

set -euo pipefail

BINARIES_DIR="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/binaries"
mkdir -p "$BINARIES_DIR"

ALL_TRIPLES=(aarch64-apple-darwin x86_64-apple-darwin x86_64-pc-windows-msvc)

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}

verify_pin() { # <file> <triple>
  local var="FFMPEG_SHA256_${2//-/_}" actual
  actual="$(sha256 "$1")"
  echo "  sha256: $actual"
  if [[ -n "${!var:-}" && "${!var}" != "$actual" ]]; then
    echo "ERROR: checksum mismatch for $2 (expected ${!var})" >&2
    exit 1
  fi
}

extract_zip() { # <zip> <dest-dir>
  mkdir -p "$2"
  if command -v unzip >/dev/null 2>&1; then unzip -qo "$1" -d "$2"
  elif command -v 7z >/dev/null 2>&1; then 7z x -y -o"$2" "$1" >/dev/null
  else echo "ERROR: need unzip or 7z" >&2; exit 1; fi
}

download_macos() { # <triple> <arch: arm64|amd64>
  local triple="$1" arch="$2"
  local url="https://ffmpeg.martin-riedl.de/redirect/latest/macos/${arch}/release/ffmpeg.zip"
  local tmp; tmp="$(mktemp -d)"
  echo "Downloading ffmpeg for $triple from $url"
  curl -fL --retry 3 -o "$tmp/ffmpeg.zip" "$url"
  verify_pin "$tmp/ffmpeg.zip" "$triple"
  extract_zip "$tmp/ffmpeg.zip" "$tmp/x"
  local bin; bin="$(find "$tmp/x" -type f -name ffmpeg | head -1)"
  [[ -n "$bin" ]] || { echo "ERROR: ffmpeg not found in archive" >&2; exit 1; }
  if command -v lipo >/dev/null 2>&1; then
    local want; want=$([[ $arch == arm64 ]] && echo arm64 || echo x86_64)
    lipo -archs "$bin" | grep -qw "$want" || { echo "ERROR: $bin is not $want ($(lipo -archs "$bin"))" >&2; exit 1; }
  fi
  install -m 755 "$bin" "$BINARIES_DIR/ffmpeg-$triple"
  rm -rf "$tmp"
  echo "  ✓ ffmpeg-$triple"
}

download_windows() { # <triple>
  local triple="$1"
  local base="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest"
  local name="ffmpeg-master-latest-win64-lgpl.zip"
  local tmp; tmp="$(mktemp -d)"
  echo "Downloading ffmpeg for $triple from $base/$name"
  curl -fL --retry 3 -o "$tmp/$name" "$base/$name"
  curl -fL --retry 3 -o "$tmp/checksums.sha256" "$base/checksums.sha256"
  local expected; expected="$(awk -v n="$name" '$2==n {print $1}' "$tmp/checksums.sha256")"
  [[ -n "$expected" ]] || { echo "ERROR: $name not listed in checksums.sha256" >&2; exit 1; }
  [[ "$(sha256 "$tmp/$name")" == "$expected" ]] || { echo "ERROR: checksum mismatch for $name" >&2; exit 1; }
  echo "  checksum OK (published by BtbN)"
  verify_pin "$tmp/$name" "$triple"
  extract_zip "$tmp/$name" "$tmp/x"
  local bin; bin="$(find "$tmp/x" -type f -name ffmpeg.exe | head -1)"
  [[ -n "$bin" ]] || { echo "ERROR: ffmpeg.exe not found in archive" >&2; exit 1; }
  cp "$bin" "$BINARIES_DIR/ffmpeg-$triple.exe"
  rm -rf "$tmp"
  echo "  ✓ ffmpeg-$triple.exe"
}

download() {
  case "$1" in
    aarch64-apple-darwin)   download_macos "$1" arm64 ;;
    x86_64-apple-darwin)    download_macos "$1" amd64 ;;
    x86_64-pc-windows-msvc) download_windows "$1" ;;
    *) echo "ERROR: unsupported target '$1' (supported: ${ALL_TRIPLES[*]})" >&2; exit 1 ;;
  esac
}

if [[ $# -eq 0 ]]; then
  host="$(rustc -vV | awk '/host:/{print $2}')"
  set -- "$host"
elif [[ "$1" == "--all" ]]; then
  set -- "${ALL_TRIPLES[@]}"
fi

for t in "$@"; do download "$t"; done

echo ""
echo "Done. Binaries in: $BINARIES_DIR"
ls -lh "$BINARIES_DIR"/ffmpeg* 2>/dev/null || true

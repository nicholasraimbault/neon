#!/bin/bash
# Downloads WidevineCdm directly from Google's servers (via Mozilla's manifest).
# Usage: download-widevine.sh [output_dir] [--force]
# No Chrome installation needed.

set -euo pipefail

FORCE=false
OUTPUT_DIR="$HOME/.local/share/WidevineCdm"

for arg in "$@"; do
  case "$arg" in
    --force) FORCE=true ;;
    *) OUTPUT_DIR="$arg" ;;
  esac
done

# Skip if already cached (unless --force)
if [ "$FORCE" = false ] && [ -d "$OUTPUT_DIR" ] && [ -f "$OUTPUT_DIR/manifest.json" ]; then
  echo "WidevineCdm already cached at $OUTPUT_DIR (use --force to re-download)."
  exit 0
fi

OS=$(uname -s)
ARCH=$(uname -m)

if [ "$OS" = "Darwin" ]; then
  if [ "$ARCH" = "arm64" ] || [ "$ARCH" = "aarch64" ]; then
    PLATFORM_KEY="Darwin_aarch64-gcc3"
  elif [ "$ARCH" = "x86_64" ]; then
    PLATFORM_KEY="Darwin_x86_64-gcc3-u-i386-x86_64"
  else
    echo "Error: Unsupported architecture: $ARCH"
    exit 1
  fi
elif [ "$OS" = "Linux" ]; then
  if [ "$ARCH" = "x86_64" ]; then
    PLATFORM_KEY="Linux_x86_64-gcc3"
  elif [ "$ARCH" = "aarch64" ]; then
    PLATFORM_KEY="LACROS_ARM64"
  else
    echo "Error: Unsupported architecture: $ARCH"
    exit 1
  fi
else
  echo "Error: Unsupported OS: $OS"
  exit 1
fi

# --- ARM64 Linux: extract from ChromeOS LaCrOS ---

if [ "${PLATFORM_KEY:-}" = "LACROS_ARM64" ]; then
  echo "ARM64 Linux: extracting WidevineCdm from ChromeOS LaCrOS image..."

  if ! command -v unsquashfs >/dev/null 2>&1; then
    echo "Error: unsquashfs is required for ARM64. Install squashfs-tools:"
    echo "  sudo apt install squashfs-tools   # Debian/Ubuntu"
    echo "  sudo pacman -S squashfs-tools     # Arch"
    exit 1
  fi

  TMP_DIR=$(mktemp -d)
  trap 'rm -rf "$TMP_DIR"' EXIT

  # Fetch the latest LaCrOS version URL from the update manifest
  echo "Fetching LaCrOS version..."
  LACROS_URL=$(curl -sL "https://chromiumdash.appspot.com/fetch/milestones" | python3 -c "
import json, sys
ms = json.loads(sys.stdin.read())
latest = max(ms, key=lambda m: m['milestone'])
v = latest['chromeos_branch_base_position']
# LaCrOS squashfs URL pattern
print(f'https://commondatastorage.googleapis.com/chromeos-localmirror/distfiles/chromeos-lacros-arm64-squash-zstd-{latest[\"milestone\"]}.0.0.0')
" 2>/dev/null || true)

  # Fall back to a known recovery image approach if the above fails
  if [ -z "$LACROS_URL" ] || ! curl -sI "$LACROS_URL" | grep -q "200"; then
    echo "Fetching LaCrOS recovery image metadata..."
    LACROS_URL=$(curl -sL "https://chromiumdash.appspot.com/cros/fetch_serving_builds?deviceCategory=ChromeOS" | python3 -c "
import json, sys
data = json.loads(sys.stdin.read())
# Find any ARM64 board's LaCrOS component
for board in data.get('builds', {}).values():
    for build in board if isinstance(board, list) else [board]:
        lacros = build.get('lacros', {})
        if 'uri' in lacros:
            print(lacros['uri'])
            sys.exit(0)
print('')
" 2>/dev/null || echo "")
  fi

  if [ -z "$LACROS_URL" ]; then
    echo "Error: Could not find LaCrOS download URL."
    echo "You can try the Asahi Linux widevine-installer as an alternative:"
    echo "  https://github.com/AsahiLinux/widevine-installer"
    rm -rf "$TMP_DIR"
    exit 1
  fi

  echo "Downloading LaCrOS image..."
  curl -sL -o "$TMP_DIR/lacros.squash" "$LACROS_URL"

  echo "Extracting WidevineCdm from LaCrOS..."
  unsquashfs -f -d "$TMP_DIR/lacros" "$TMP_DIR/lacros.squash" \
    'WidevineCdm/*' '**/libwidevinecdm.so' '**/manifest.json' 2>/dev/null || true

  # Find the extracted WidevineCdm
  CDM_DIR=$(find "$TMP_DIR/lacros" -type d -name "WidevineCdm" 2>/dev/null | head -1)
  if [ -z "$CDM_DIR" ] || [ ! -f "$CDM_DIR/manifest.json" ]; then
    echo "Error: Could not find WidevineCdm in LaCrOS image."
    rm -rf "$TMP_DIR"
    exit 1
  fi

  # Restructure: cros_arm64 -> linux_arm64
  mkdir -p "$OUTPUT_DIR"
  rm -rf "$OUTPUT_DIR"/*
  cp "$CDM_DIR/manifest.json" "$OUTPUT_DIR/"
  [ -f "$CDM_DIR/LICENSE" ] && cp "$CDM_DIR/LICENSE" "$OUTPUT_DIR/"
  [ -f "$CDM_DIR/LICENSE.txt" ] && cp "$CDM_DIR/LICENSE.txt" "$OUTPUT_DIR/"

  mkdir -p "$OUTPUT_DIR/_platform_specific/linux_arm64"
  # Copy from whichever platform dir exists (cros_arm64 or linux_arm64)
  for src_dir in "$CDM_DIR/_platform_specific/cros_arm64" "$CDM_DIR/_platform_specific/linux_arm64"; do
    if [ -d "$src_dir" ]; then
      cp "$src_dir"/libwidevinecdm.so "$OUTPUT_DIR/_platform_specific/linux_arm64/"
      break
    fi
  done

  if [ ! -f "$OUTPUT_DIR/_platform_specific/linux_arm64/libwidevinecdm.so" ]; then
    # Try finding it anywhere in the extraction
    SO_FILE=$(find "$TMP_DIR/lacros" -name "libwidevinecdm.so" 2>/dev/null | head -1)
    if [ -n "$SO_FILE" ]; then
      cp "$SO_FILE" "$OUTPUT_DIR/_platform_specific/linux_arm64/"
    else
      echo "Error: libwidevinecdm.so not found in LaCrOS image."
      rm -rf "$TMP_DIR"
      exit 1
    fi
  fi

  chmod -R 755 "$OUTPUT_DIR"
  rm -rf "$TMP_DIR"
  echo "WidevineCdm (ARM64) saved to $OUTPUT_DIR"
  exit 0
fi

# --- Standard download (x86_64 and macOS) ---

echo "Fetching latest WidevineCdm version info..."
JSON=$(curl -sL "https://raw.githubusercontent.com/mozilla-firefox/firefox/refs/heads/main/toolkit/content/gmp-sources/widevinecdm.json")

read -r VERSION URL HASH SIZE <<< $(python3 -c "
import json, sys
d = json.loads(sys.stdin.read())
v = d['vendors']['gmp-widevinecdm']
p = v['platforms']['$PLATFORM_KEY']
print(v['version'], p['fileUrl'], p['hashValue'], p['filesize'])
" <<< "$JSON")

echo "Version: $VERSION ($ARCH)"
echo "Downloading..."

TMP_DIR=$(mktemp -d)
CRX3_FILE="$TMP_DIR/widevine.crx3"
curl -sL -o "$CRX3_FILE" "$URL"

# Verify SHA-512
if command -v sha512sum >/dev/null 2>&1; then
  ACTUAL_HASH=$(sha512sum "$CRX3_FILE" | awk '{print $1}')
else
  ACTUAL_HASH=$(shasum -a 512 "$CRX3_FILE" | awk '{print $1}')
fi
if [ "$ACTUAL_HASH" != "$HASH" ]; then
  echo "Error: SHA-512 hash mismatch!"
  rm -rf "$TMP_DIR"
  exit 1
fi
echo "SHA-512 verified."

# Extract CRX3 (skip magic + version + header length + header -> ZIP)
echo "Extracting..."
HEADER_LEN=$(python3 -c "
import struct, sys
with open(sys.argv[1], 'rb') as f:
    f.seek(8)
    print(12 + struct.unpack('<I', f.read(4))[0])
" "$CRX3_FILE")

dd if="$CRX3_FILE" bs=1 skip="$HEADER_LEN" of="$TMP_DIR/widevine.zip" 2>/dev/null
mkdir -p "$OUTPUT_DIR"
rm -rf "$OUTPUT_DIR"/*
unzip -o "$TMP_DIR/widevine.zip" -d "$OUTPUT_DIR" > /dev/null

rm -rf "$TMP_DIR"
echo "WidevineCdm $VERSION saved to $OUTPUT_DIR"

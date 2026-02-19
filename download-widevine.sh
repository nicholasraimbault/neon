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

ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ] || [ "$ARCH" = "aarch64" ]; then
  PLATFORM_KEY="Darwin_aarch64-gcc3"
elif [ "$ARCH" = "x86_64" ]; then
  PLATFORM_KEY="Darwin_x86_64-gcc3-u-i386-x86_64"
else
  echo "Error: Unsupported architecture: $ARCH"
  exit 1
fi

echo "Fetching latest WidevineCdm version info..."
JSON=$(curl -sL "https://hg.mozilla.org/mozilla-central/raw-file/tip/toolkit/content/gmp-sources/widevinecdm.json")

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
ACTUAL_HASH=$(shasum -a 512 "$CRX3_FILE" | awk '{print $1}')
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

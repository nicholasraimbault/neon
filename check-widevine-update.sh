#!/bin/bash
# Checks if a newer WidevineCdm version is available.
# Compares the cached version against Mozilla's manifest.
# Exit codes: 0 = up to date, 1 = error, 3 = update available

set -euo pipefail

CACHE_DIR="$HOME/.local/share/WidevineCdm"

# Get cached version
if [ -f "$CACHE_DIR/manifest.json" ]; then
  CACHED_VERSION=$(python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    print(json.load(f).get('version', 'unknown'))
" "$CACHE_DIR/manifest.json" 2>/dev/null || echo "unknown")
else
  CACHED_VERSION="none"
fi

# Get latest version from Mozilla's manifest
LATEST_VERSION=$(curl -sL "https://hg.mozilla.org/mozilla-central/raw-file/tip/toolkit/content/gmp-sources/widevinecdm.json" | \
  python3 -c "
import json, sys
d = json.loads(sys.stdin.read())
print(d['vendors']['gmp-widevinecdm']['version'])
" 2>/dev/null || echo "error")

if [ "$LATEST_VERSION" = "error" ]; then
  echo "Error: Could not fetch latest version."
  exit 1
fi

echo "Cached:  $CACHED_VERSION"
echo "Latest:  $LATEST_VERSION"

if [ "$CACHED_VERSION" = "$LATEST_VERSION" ]; then
  echo "WidevineCdm is up to date."
  exit 0
else
  echo "Update available! Run: neon-update-widevine --force"
  exit 3
fi

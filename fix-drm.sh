#!/bin/bash
# Patches WidevineCdm into Chromium-based browsers for DRM playback.
# Supports: Helium, Thorium, ungoogled-chromium (Chromium.app)
# Downloads WidevineCdm if not cached. Runs standalone or via Neon.app.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CACHE_DIR="$HOME/.local/share/WidevineCdm"
DL_SCRIPT="$SCRIPT_DIR/download-widevine.sh"

# Browser configs: "AppName|FrameworkName"
BROWSERS=(
  "Helium|Helium Framework"
  "Thorium|Thorium Framework"
  "Chromium|Chromium Framework"
)

patched=0
skipped=0
failed=0

for entry in "${BROWSERS[@]}"; do
  IFS='|' read -r app_name fw_name <<< "$entry"
  app_path="/Applications/${app_name}.app"
  fw_path="$app_path/Contents/Frameworks/${fw_name}.framework/Versions"

  if [ ! -d "$app_path" ]; then
    continue
  fi

  ver=$(ls "$fw_path" 2>/dev/null | grep -E '^[0-9]+\.' | head -1)
  if [ -z "$ver" ]; then
    echo "$app_name: no version directory found — skipping."
    ((failed++)) || true
    continue
  fi

  dest="$fw_path/$ver/Libraries/WidevineCdm"

  if [ -d "$dest" ] && [ "${1:-}" != "--force" ]; then
    echo "$app_name $ver: already patched."
    ((skipped++)) || true
    continue
  fi

  # Download WidevineCdm if not cached
  if [ ! -d "$CACHE_DIR" ] || [ ! -f "$CACHE_DIR/manifest.json" ]; then
    if [ -x "$DL_SCRIPT" ]; then
      echo "WidevineCdm not cached. Downloading..."
      bash "$DL_SCRIPT" "$CACHE_DIR"
    else
      echo "Error: No cached WidevineCdm and download script not found at $DL_SCRIPT"
      exit 1
    fi
  fi

  echo "Patching WidevineCdm into $app_name $ver..."

  tmp="/tmp/${app_name}-drm-fix.app"
  rm -rf "$tmp"
  cp -R "$app_path" "$tmp"

  tmp_dest="$tmp/Contents/Frameworks/${fw_name}.framework/Versions/$ver/Libraries/WidevineCdm"
  cp -R "$CACHE_DIR" "$tmp_dest"
  xattr -cr "$tmp"

  rm -rf "$app_path"
  mv "$tmp" "$app_path"
  codesign --force --deep -s - "$app_path"

  echo "$app_name $ver: patched."
  ((patched++)) || true
done

if [ $patched -eq 0 ] && [ $skipped -eq 0 ]; then
  echo "No supported browsers found in /Applications."
  exit 1
fi

echo "Done. Patched: $patched, already patched: $skipped."

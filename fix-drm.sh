#!/bin/bash
# Patches WidevineCdm into Chromium-based browsers for DRM playback.
# macOS: Helium, Thorium, ungoogled-chromium
# Linux: Helium, Thorium, ungoogled-chromium, Chromium
# Downloads WidevineCdm if not cached. Runs standalone or via Neon app.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CACHE_DIR="$HOME/.local/share/WidevineCdm"
DL_SCRIPT="$SCRIPT_DIR/download-widevine.sh"
OS=$(uname -s)

# --- Browser configs per OS ---

# macOS: "AppName|FrameworkName"
DARWIN_BROWSERS=(
  "Helium|Helium Framework"
  "Thorium|Thorium Framework"
  "Chromium|Chromium Framework"
)

# Linux: "DisplayName|InstallPath"
LINUX_BROWSERS=(
  "Helium|/opt/helium-browser-bin"
  "Thorium|/opt/chromium.org/thorium"
  "Thorium|/opt/thorium-browser"
  "ungoogled-chromium|/usr/lib/chromium"
  "ungoogled-chromium|/usr/lib64/chromium"
  "Chromium|/usr/lib/chromium-browser"
)

# --- Auto-discovery ---

# Scan for additional Chromium-based browsers not in the hardcoded list.
# On macOS: look for .app bundles with a Framework.framework/Versions structure.
# On Linux: look for directories containing chrome-sandbox (Chromium marker).

discover_darwin_browsers() {
  local known_apps=""
  for entry in "${DARWIN_BROWSERS[@]}"; do
    known_apps="$known_apps|$(echo "$entry" | cut -d'|' -f1)"
  done

  for app in /Applications/*.app; do
    [ -d "$app" ] || continue
    app_name=$(basename "$app" .app)

    # Skip already-known browsers
    echo "$known_apps" | grep -q "|$app_name" && continue

    # Look for a Chromium framework structure
    for fw_dir in "$app/Contents/Frameworks/"*".framework/Versions"; do
      [ -d "$fw_dir" ] || continue
      # Check for version-numbered subdirectory
      if ls "$fw_dir" 2>/dev/null | grep -qE '^[0-9]+\.'; then
        fw_name=$(basename "$(dirname "$fw_dir")" .framework)
        DARWIN_BROWSERS+=("$app_name|$fw_name")
        break
      fi
    done
  done
}

discover_linux_browsers() {
  local known_paths=""
  for entry in "${LINUX_BROWSERS[@]}"; do
    known_paths="$known_paths|$(echo "$entry" | cut -d'|' -f2)"
  done

  # Scan common install locations for Chromium-based browsers
  local scan_dirs=(/opt /usr/lib /usr/lib64 /usr/local/lib)
  for scan_dir in "${scan_dirs[@]}"; do
    [ -d "$scan_dir" ] || continue
    for dir in "$scan_dir"/*/; do
      [ -d "$dir" ] || continue
      dir="${dir%/}"

      # Skip already-known paths
      echo "$known_paths" | grep -q "|$dir" && continue

      # Check for chrome-sandbox (present in all Chromium-based browsers)
      if [ -f "$dir/chrome-sandbox" ] || [ -f "$dir/chromium-sandbox" ]; then
        browser_name=$(basename "$dir")
        LINUX_BROWSERS+=("$browser_name (discovered)|$dir")
      fi
    done
  done
}

# --- Shared: ensure WidevineCdm is cached ---

ensure_widevine() {
  if [ ! -d "$CACHE_DIR" ] || [ ! -f "$CACHE_DIR/manifest.json" ]; then
    if [ -x "$DL_SCRIPT" ]; then
      echo "WidevineCdm not cached. Downloading..."
      bash "$DL_SCRIPT" "$CACHE_DIR"
    else
      echo "Error: No cached WidevineCdm and download script not found at $DL_SCRIPT"
      exit 1
    fi
  fi
}

# --- macOS patching ---

patch_darwin() {
  local patched=0 skipped=0 failed=0

  for entry in "${DARWIN_BROWSERS[@]}"; do
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

    ensure_widevine

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
}

# --- Linux patching ---

patch_linux() {
  local patched=0 skipped=0

  for entry in "${LINUX_BROWSERS[@]}"; do
    IFS='|' read -r display_name browser_path <<< "$entry"

    if [ ! -d "$browser_path" ]; then
      continue
    fi

    dest="$browser_path/WidevineCdm"

    if [ -d "$dest" ] && [ "${1:-}" != "--force" ]; then
      echo "$display_name ($browser_path): already patched."
      ((skipped++)) || true
      continue
    fi

    ensure_widevine

    # Check write permissions
    if [ ! -w "$browser_path" ]; then
      echo "Error: No write permission to $browser_path"
      echo "Re-run with sudo: sudo bash $0 ${1:-}"
      exit 1
    fi

    echo "Patching WidevineCdm into $display_name ($browser_path)..."

    rm -rf "$dest"
    cp -R "$CACHE_DIR" "$dest"
    chmod -R 755 "$dest"

    echo "$display_name ($browser_path): patched."
    ((patched++)) || true
  done

  if [ $patched -eq 0 ] && [ $skipped -eq 0 ]; then
    echo "No supported browsers found."
    echo "Searched: ${LINUX_BROWSERS[*]%%|*}"
    exit 1
  fi

  echo "Done. Patched: $patched, already patched: $skipped."
}

# --- Main ---

case "$OS" in
  Darwin)
    discover_darwin_browsers
    patch_darwin "${1:-}"
    ;;
  Linux)
    discover_linux_browsers
    patch_linux "${1:-}"
    ;;
  *)
    echo "Error: Unsupported OS: $OS"
    exit 1
    ;;
esac

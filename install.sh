#!/bin/bash
# Neon installer: downloads WidevineCdm, patches browsers, sets up auto-patching.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Neon Installer ==="
echo ""

# Step 1: Download WidevineCdm
echo "[1/3] Downloading WidevineCdm..."
bash "$SCRIPT_DIR/download-widevine.sh"
echo ""

# Step 2: Patch browsers (requires admin for /Applications)
echo "[2/3] Patching browsers..."
osascript -e "do shell script \"bash '$SCRIPT_DIR/fix-drm.sh' --force\" with administrator privileges"
echo ""

# Step 3: Set up LaunchDaemon for auto-patching on app updates
echo "[3/3] Setting up auto-patch daemon..."

cat > /tmp/com.neon.fix-drm.plist << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.neon.fix-drm</string>
    <key>ProgramArguments</key>
    <array>
        <string>$SCRIPT_DIR/fix-drm.sh</string>
    </array>
    <key>WatchPaths</key>
    <array>
        <string>/Applications/Helium.app</string>
        <string>/Applications/Thorium.app</string>
        <string>/Applications/Chromium.app</string>
    </array>
    <key>StandardOutPath</key>
    <string>/tmp/neon-fix-drm.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/neon-fix-drm.log</string>
</dict>
</plist>
PLIST

osascript -e "do shell script \"cp /tmp/com.neon.fix-drm.plist /Library/LaunchDaemons/ && launchctl load /Library/LaunchDaemons/com.neon.fix-drm.plist\" with administrator privileges"

if [ $? -eq 0 ]; then
  rm -f /tmp/com.neon.fix-drm.plist
  echo ""
  echo "Done! Neon is installed."
  echo "  - DRM is patched in all detected browsers"
  echo "  - Auto-patch daemon is active (triggers on app updates)"
  echo ""
  echo "To uninstall: bash $SCRIPT_DIR/uninstall.sh"
else
  echo "Setup cancelled."
  exit 1
fi

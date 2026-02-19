#!/bin/bash
# Neon uninstaller: removes the LaunchDaemon and cached WidevineCdm.

set -euo pipefail

echo "=== Neon Uninstaller ==="
echo ""

# Remove LaunchDaemon
PLIST="/Library/LaunchDaemons/com.neon.fix-drm.plist"
if [ -f "$PLIST" ]; then
  echo "Removing auto-patch daemon..."
  osascript -e "do shell script \"launchctl unload '$PLIST' 2>/dev/null; rm -f '$PLIST'\" with administrator privileges"
  echo "Daemon removed."
else
  echo "No daemon found — skipping."
fi

# Also remove old daemon if present
OLD_PLIST="/Library/LaunchDaemons/com.local.fix-helium-drm.plist"
if [ -f "$OLD_PLIST" ]; then
  echo "Removing legacy daemon..."
  osascript -e "do shell script \"launchctl unload '$OLD_PLIST' 2>/dev/null; rm -f '$OLD_PLIST'\" with administrator privileges"
  echo "Legacy daemon removed."
fi

# Remove cached WidevineCdm
CACHE_DIR="$HOME/.local/share/WidevineCdm"
if [ -d "$CACHE_DIR" ]; then
  echo "Removing cached WidevineCdm..."
  rm -rf "$CACHE_DIR"
  echo "Cache removed."
fi

# Remove LaunchAgent (if Neon.app set one up)
AGENT_PLIST="$HOME/Library/LaunchAgents/com.neon.app.plist"
if [ -f "$AGENT_PLIST" ]; then
  launchctl unload "$AGENT_PLIST" 2>/dev/null || true
  rm -f "$AGENT_PLIST"
  echo "Launch agent removed."
fi

echo ""
echo "Neon uninstalled. Browser patches remain in place until the browser updates."

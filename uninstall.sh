#!/bin/bash
# Neon uninstaller: removes the auto-patch daemon and cached WidevineCdm.

set -euo pipefail

OS=$(uname -s)

echo "=== Neon Uninstaller ==="
echo ""

# --- macOS ---

uninstall_darwin() {
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

  # Remove LaunchAgent (if Neon.app set one up)
  AGENT_PLIST="$HOME/Library/LaunchAgents/com.neon.app.plist"
  if [ -f "$AGENT_PLIST" ]; then
    launchctl unload "$AGENT_PLIST" 2>/dev/null || true
    rm -f "$AGENT_PLIST"
    echo "Launch agent removed."
  fi
}

# --- Linux ---

uninstall_linux() {
  # Remove systemd units
  if command -v systemctl >/dev/null 2>&1; then
    if [ -f /etc/systemd/system/neon-fix-drm.path ]; then
      echo "Removing auto-patch daemon..."
      sudo systemctl disable --now neon-fix-drm.path 2>/dev/null || true
      sudo rm -f /etc/systemd/system/neon-fix-drm.path
      sudo rm -f /etc/systemd/system/neon-fix-drm.service
      sudo systemctl daemon-reload
      echo "Daemon removed."
    else
      echo "No daemon found — skipping."
    fi
  fi

  # Remove autostart entry (if tray app set one up)
  AUTOSTART="$HOME/.config/autostart/neon.desktop"
  if [ -f "$AUTOSTART" ]; then
    rm -f "$AUTOSTART"
    echo "Autostart entry removed."
  fi
}

# --- Shared ---

case "$OS" in
  Darwin) uninstall_darwin ;;
  Linux)  uninstall_linux ;;
  *)
    echo "Error: Unsupported OS: $OS"
    exit 1
    ;;
esac

# Remove cached WidevineCdm
CACHE_DIR="$HOME/.local/share/WidevineCdm"
if [ -d "$CACHE_DIR" ]; then
  echo "Removing cached WidevineCdm..."
  rm -rf "$CACHE_DIR"
  echo "Cache removed."
fi

echo ""
echo "Neon uninstalled. Browser patches remain in place until the browser updates."

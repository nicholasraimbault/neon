#!/bin/bash
# Builds a .deb package for Neon.
# Requires: dpkg-deb, go (for tray app)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR/../.."
VERSION="1.0.0"
PKG="neon-drm_${VERSION}_amd64"
BUILD="$SCRIPT_DIR/$PKG"

echo "Building Neon .deb package..."

# Clean previous build
rm -rf "$BUILD"

# Build tray app
echo "Compiling tray app..."
cd "$PROJECT_DIR/linux-app"
go build -o "$SCRIPT_DIR/neon-tray" .
cd "$SCRIPT_DIR"

# Create directory structure
mkdir -p "$BUILD/DEBIAN"
mkdir -p "$BUILD/usr/lib/neon"
mkdir -p "$BUILD/usr/bin"
mkdir -p "$BUILD/usr/lib/systemd/system"
mkdir -p "$BUILD/usr/share/polkit-1/actions"
mkdir -p "$BUILD/usr/share/applications"

# DEBIAN control files
cp debian/control "$BUILD/DEBIAN/control"
cp debian/postinst "$BUILD/DEBIAN/postinst"
cp debian/prerm "$BUILD/DEBIAN/prerm"
chmod 755 "$BUILD/DEBIAN/postinst" "$BUILD/DEBIAN/prerm"

# Core scripts
install -m 755 "$PROJECT_DIR/fix-drm.sh" "$BUILD/usr/lib/neon/"
install -m 755 "$PROJECT_DIR/download-widevine.sh" "$BUILD/usr/lib/neon/"
install -m 755 "$PROJECT_DIR/install.sh" "$BUILD/usr/lib/neon/"
install -m 755 "$PROJECT_DIR/uninstall.sh" "$BUILD/usr/lib/neon/"

# Tray app binary
install -m 755 neon-tray "$BUILD/usr/bin/neon-tray"
rm -f neon-tray

# CLI wrappers
for cmd in install uninstall; do
  cat > "$BUILD/usr/bin/neon-$cmd" << EOF
#!/bin/bash
exec /usr/lib/neon/$cmd.sh "\$@"
EOF
  chmod 755 "$BUILD/usr/bin/neon-$cmd"
done

cat > "$BUILD/usr/bin/neon-patch" << 'EOF'
#!/bin/bash
exec /usr/lib/neon/fix-drm.sh "$@"
EOF
chmod 755 "$BUILD/usr/bin/neon-patch"

cat > "$BUILD/usr/bin/neon-update-widevine" << 'EOF'
#!/bin/bash
exec /usr/lib/neon/download-widevine.sh "$@"
EOF
chmod 755 "$BUILD/usr/bin/neon-update-widevine"

# Desktop entry
install -m 644 "$PROJECT_DIR/linux-app/neon.desktop" "$BUILD/usr/share/applications/"

# Polkit policy
install -m 644 "$PROJECT_DIR/linux-app/com.neon.fix-drm.policy" "$BUILD/usr/share/polkit-1/actions/"

# systemd units
cat > "$BUILD/usr/lib/systemd/system/neon-fix-drm.path" << 'EOF'
[Unit]
Description=Neon: Watch browsers for updates

[Path]
PathChanged=/opt/helium-browser-bin
PathChanged=/opt/chromium.org/thorium
PathChanged=/opt/thorium-browser
PathChanged=/usr/lib/chromium
PathChanged=/usr/lib64/chromium
PathChanged=/usr/lib/chromium-browser

[Install]
WantedBy=multi-user.target
EOF

cat > "$BUILD/usr/lib/systemd/system/neon-fix-drm.service" << 'EOF'
[Unit]
Description=Neon: Patch WidevineCdm into browsers

[Service]
Type=oneshot
ExecStartPre=/bin/sleep 5
ExecStart=/usr/lib/neon/fix-drm.sh
StandardOutput=journal
StandardError=journal
EOF

# Build the .deb
dpkg-deb --build "$BUILD"

# Clean up
rm -rf "$BUILD"

echo "Built: $SCRIPT_DIR/${PKG}.deb"

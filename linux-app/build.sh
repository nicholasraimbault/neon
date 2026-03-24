#!/bin/bash
# Builds the Neon Linux tray app.
# Requires: go 1.21+, libayatana-appindicator3-dev (or libappindicator3-dev)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$SCRIPT_DIR/../build"

echo "Building Neon Linux tray app..."

mkdir -p "$BUILD_DIR"

cd "$SCRIPT_DIR"
go build -o "$BUILD_DIR/neon-tray" .

echo "Built: $BUILD_DIR/neon-tray"

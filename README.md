# Neon

Fix DRM playback in Chromium-based browsers on macOS and Linux.

Neon patches [WidevineCdm](https://www.widevine.com/) into browsers that ship without it, enabling Netflix, Spotify, Disney+, and other DRM-protected content. It auto-patches when your browser updates so you never have to think about it again.

## Features

- **Multi-browser**: Supports Helium, Thorium, ungoogled-chromium, and Chromium out of the box.
- **Auto-discovery**: Scans for additional Chromium-based browsers beyond the hardcoded list.
- **Auto-patching**: Re-patches automatically when a browser updates (LaunchDaemon on macOS, systemd on Linux).
- **Tray/menu bar app**: Per-browser status, one-click patching, launch at login.
- **Update check**: See if a newer WidevineCdm is available from Google.
- **ARM64 Linux**: Extracts WidevineCdm from ChromeOS LaCrOS images for aarch64 systems (Asahi Linux, Raspberry Pi).

## Supported browsers

### macOS

| Browser | Path |
|---------|------|
| [Helium](https://helium.build) | `/Applications/Helium.app` |
| [Thorium](https://thorium.rocks) | `/Applications/Thorium.app` |
| [ungoogled-chromium](https://ungoogled-software.github.io/ungoogled-chromium-binaries/) | `/Applications/Chromium.app` |

Plus any Chromium-based `.app` in `/Applications` (auto-discovered).

### Linux

| Browser | Path(s) |
|---------|---------|
| [Helium](https://helium.build) | `/opt/helium-browser-bin` |
| [Thorium](https://thorium.rocks) | `/opt/chromium.org/thorium`, `/opt/thorium-browser` |
| [ungoogled-chromium](https://ungoogled-software.github.io/ungoogled-chromium-binaries/) | `/usr/lib/chromium`, `/usr/lib64/chromium` |
| [Chromium](https://www.chromium.org/) | `/usr/lib/chromium-browser` |

Plus any Chromium-based browser in `/opt`, `/usr/lib`, `/usr/lib64` (auto-discovered).

## Requirements

- Python 3, curl, unzip
- macOS or Linux (x86_64 or ARM64)
- squashfs-tools (ARM64 Linux only)
- Go 1.21+ and `libayatana-appindicator3-dev` (only if building the Linux tray app)
- systemd (optional, for auto-patching on Linux)

## Install

### macOS — Homebrew

```
brew install nicholasraimbault/neon/neon
neon-install
```

macOS will prompt for your password to patch apps in `/Applications` and install the auto-patch daemon.

### macOS — Menu bar app

Download **Neon.dmg** from [Releases](https://github.com/nicholasraimbault/neon/releases) and drag to Applications.

Neon lives in your menu bar:

- Per-browser patch status
- **Patch Now** — patch all detected browsers
- **Update Widevine** — re-download the latest WidevineCdm
- **Launch at Login** — start Neon on boot
- Auto-patches when a browser updates (no daemon needed)

### Linux — Arch (PKGBUILD)

```
git clone https://github.com/nicholasraimbault/neon.git
cd neon/packaging/aur
makepkg -si
neon-install
```

### Linux — .deb (Debian/Ubuntu)

```
git clone https://github.com/nicholasraimbault/neon.git
cd neon
bash packaging/deb/build-deb.sh
sudo dpkg -i packaging/deb/neon-drm_*.deb
neon-install
```

Requires Go to build the tray app binary.

### Manual (macOS or Linux)

```
git clone https://github.com/nicholasraimbault/neon.git
cd neon
bash install.sh
```

This downloads WidevineCdm, patches all detected browsers, and sets up auto-patching. The scripts can be run directly afterward:

```
bash fix-drm.sh              # Patch browsers
bash fix-drm.sh --force      # Re-patch even if already patched
bash download-widevine.sh     # Download latest WidevineCdm
bash check-widevine-update.sh # Check for newer version
bash uninstall.sh             # Remove daemon and cache
```

### CLI wrapper commands

Homebrew, PKGBUILD, and .deb installs add these to your PATH:

| Command | Description |
|---------|-------------|
| `neon-install` | Download WidevineCdm, patch browsers, install auto-patch daemon |
| `neon-patch` | Patch all detected browsers |
| `neon-patch --force` | Re-patch even if already patched |
| `neon-update-widevine` | Download latest WidevineCdm |
| `neon-update-widevine --force` | Re-download even if cached |
| `neon-uninstall` | Remove daemon and cached files |
| `neon-check-update` | Check if a newer WidevineCdm version is available |

## How it works

1. Neon downloads the latest WidevineCdm from Google (via Mozilla's version manifest) and verifies its SHA-512 hash.

2. It copies WidevineCdm into each browser's install directory:
   - **macOS**: patches the `.app` bundle, clears extended attributes, and ad-hoc codesigns
   - **Linux**: copies into the browser directory alongside the binary

3. A background watcher monitors your browsers for updates and re-patches automatically:
   - **macOS Homebrew/manual**: LaunchDaemon with WatchPaths
   - **macOS menu bar app**: built-in file watcher (no root daemon needed)
   - **Linux**: systemd `.path` unit with PathChanged
   - **Linux tray app**: built-in inotify watcher

## Build from source

### macOS app

```
bash app/build.sh
```

Produces `build/Neon.app` and `build/Neon.dmg`. Requires Xcode Command Line Tools.

### Linux tray app

```
cd linux-app
go mod tidy
go build -o neon-tray .
```

Requires Go 1.21+ and `libayatana-appindicator3-dev`.

### Linux packages

```
# .deb
bash packaging/deb/build-deb.sh

# Arch (test locally)
cd packaging/aur && makepkg -si
```

## Uninstall

```
# macOS — Homebrew
neon-uninstall && brew uninstall neon

# macOS — Manual
bash uninstall.sh

# Linux — Arch
neon-uninstall && sudo pacman -R neon-drm

# Linux — .deb
neon-uninstall && sudo dpkg -r neon-drm

# Linux — Manual
bash uninstall.sh
```

## Known limitations

- **Linux support is untested.** The CLI, systemd daemon, and tray app have been built and pass CI, but have not been verified on a real Linux system yet. Bug reports welcome.
- **Flatpak browsers**: Flatpak sandboxing prevents writing to the browser's install directory.

## License

MIT

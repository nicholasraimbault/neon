# Neon

Fix DRM playback in Chromium-based browsers on macOS and Linux.

Neon patches [WidevineCdm](https://www.widevine.com/) into browsers that ship without it, enabling Netflix, Spotify, Disney+, and other DRM-protected content. It auto-patches when your browser updates so you never have to think about it again.

## Supported browsers

### macOS

| Browser | Path |
|---------|------|
| [Helium](https://helium.build) | `/Applications/Helium.app` |
| [Thorium](https://thorium.rocks) | `/Applications/Thorium.app` |
| [ungoogled-chromium](https://ungoogled-software.github.io/ungoogled-chromium-binaries/) | `/Applications/Chromium.app` |

### Linux

| Browser | Path(s) |
|---------|---------|
| [Helium](https://helium.build) | `/opt/helium-browser-bin` |
| [Thorium](https://thorium.rocks) | `/opt/chromium.org/thorium`, `/opt/thorium-browser` |
| [ungoogled-chromium](https://ungoogled-software.github.io/ungoogled-chromium-binaries/) | `/usr/lib/chromium`, `/usr/lib64/chromium` |
| Chromium | `/usr/lib/chromium-browser` |

## Requirements

- Python 3
- curl, unzip
- macOS or Linux (x86_64)
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

Download `neon-drm_1.0.0_amd64.deb` from [Releases](https://github.com/nicholasraimbault/neon/releases).

```
sudo dpkg -i neon-drm_1.0.0_amd64.deb
neon-install
```

### Linux — Tray app

The `neon-tray` binary provides a system tray app with the same functionality as the macOS menu bar app. It's included in the AUR and .deb packages, or build from source:

```
cd linux-app
go build -o neon-tray .
```

### Manual (macOS or Linux)

```
git clone https://github.com/nicholasraimbault/neon.git
cd neon
bash install.sh
```

## CLI commands

Available after installing via Homebrew, AUR, .deb, or manual install. The tray/menu bar apps have the same functionality built in.

| Command | Description |
|---------|-------------|
| `neon-install` | Download WidevineCdm, patch browsers, install auto-patch daemon |
| `neon-patch` | Patch all detected browsers |
| `neon-patch --force` | Re-patch even if already patched |
| `neon-update-widevine` | Download latest WidevineCdm |
| `neon-update-widevine --force` | Re-download even if cached |
| `neon-uninstall` | Remove daemon and cached files |

## How it works

1. Neon downloads the latest WidevineCdm from Google (via Mozilla's version manifest) and verifies its SHA-512 hash.

2. It copies WidevineCdm into each browser's framework directory, then:
   - **macOS**: clears extended attributes and ad-hoc codesigns the bundle
   - **Linux**: sets file permissions (no codesigning needed)

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

Produces `build/Neon.app` and `build/Neon-1.0.0.dmg`. Requires Xcode Command Line Tools.

### Linux tray app

```
cd linux-app
go build -o neon-tray .
```

Requires Go 1.21+ and `libayatana-appindicator3-dev`.

### Linux packages

```
# .deb
bash packaging/deb/build-deb.sh

# AUR (test locally)
cd packaging/aur && makepkg -si
```

## Uninstall

```
# macOS — Homebrew
neon-uninstall && brew uninstall neon

# macOS — Manual
bash uninstall.sh

# Linux — Arch (PKGBUILD)
neon-uninstall && sudo pacman -R neon-drm

# Linux — .deb
neon-uninstall && sudo dpkg -r neon-drm

# Linux — Manual
bash uninstall.sh
```

## Known limitations

- **ARM64 Linux**: Google does not distribute WidevineCdm for Linux ARM64. Support is planned via ChromeOS LaCrOS extraction.
- **Flatpak browsers**: Flatpak sandboxing prevents writing to the browser's install directory. Flatpak support is not yet implemented.

## License

MIT

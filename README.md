# Neon

Fix DRM playback in Chromium-based browsers on macOS.

Neon patches [WidevineCdm](https://www.widevine.com/) into browsers that ship without it, enabling Netflix, Spotify, Disney+, and other DRM-protected content. It auto-patches when your browser updates so you never have to think about it again.

## Supported browsers

| Browser | Path |
|---------|------|
| [Helium](https://helium.build) | `/Applications/Helium.app` |
| [Thorium](https://thorium.rocks) | `/Applications/Thorium.app` |
| [ungoogled-chromium](https://ungoogled-software.github.io/ungoogled-chromium-binaries/) | `/Applications/Chromium.app` |

## Requirements

- macOS
- Python 3 (ships with Xcode Command Line Tools)

## Install

### Homebrew

```
brew install nicholasraimbault/neon/neon
neon-install
```

macOS will prompt for your password to patch apps in `/Applications` and install the auto-patch daemon.

### Menu bar app

Download **Neon.dmg** from [Releases](https://github.com/nicholasraimbault/neon/releases) and drag to Applications.

Neon lives in your menu bar:

- Per-browser patch status
- **Patch Now** — patch all detected browsers
- **Update Widevine** — re-download the latest WidevineCdm
- **Launch at Login** — start Neon on boot
- Auto-patches when a browser updates (no daemon needed)

### Manual

```
git clone https://github.com/nicholasraimbault/neon.git
cd neon
bash install.sh
```

## CLI commands

Available after Homebrew or manual install. The menu bar app has the same functionality built in.

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

2. It copies WidevineCdm into each browser's framework directory, clears extended attributes, and ad-hoc codesigns the bundle.

3. A background watcher monitors your browsers for updates and re-patches automatically. The Homebrew/manual install uses a LaunchDaemon for this; the menu bar app uses a built-in file watcher instead (no root daemon needed).

## Build from source

```
bash app/build.sh
```

Produces `build/Neon.app` and `build/Neon-1.0.0.dmg`.

Requires Xcode Command Line Tools (`xcode-select --install`).

## Uninstall

```
# Homebrew
neon-uninstall && brew uninstall neon

# Manual
bash uninstall.sh
```

## License

MIT

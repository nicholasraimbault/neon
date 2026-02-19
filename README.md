# Neon

Fix DRM playback in Chromium-based browsers on macOS.

Neon patches [WidevineCdm](https://www.widevine.com/) into browsers that ship without it, enabling Netflix, Spotify, Disney+, and other DRM-protected content. It auto-patches when your browser updates so you never have to think about it again.

## Supported browsers

| Browser | Path |
|---------|------|
| [Helium](https://helium.build) | `/Applications/Helium.app` |
| [Thorium](https://thorium.rocks) | `/Applications/Thorium.app` |
| [ungoogled-chromium](https://ungoogled-software.github.io/ungoogled-chromium-binaries/) | `/Applications/Chromium.app` |

## Install

### Homebrew

```
brew install nicholasraimbault/neon/neon
neon-install
```

macOS will prompt for your password to patch apps in `/Applications` and install the auto-patch daemon.

### Menu bar app

Download **Neon.dmg** from [Releases](https://github.com/nicholasraimbault/neon/releases) and drag to Applications.

Neon lives in your menu bar with a neon tube icon:

- Per-browser patch status
- **Patch Now** — patch all detected browsers
- **Update Widevine** — re-download the latest WidevineCdm
- **Launch at Login** — start Neon on boot
- Auto-patches when a browser updates (file watcher, no daemon needed)

### Manual

```
git clone https://github.com/nicholasraimbault/neon.git
cd neon
bash install.sh
```

## Commands

| Command | Description |
|---------|-------------|
| `neon-install` | Download WidevineCdm, patch browsers, install auto-patch daemon |
| `neon-patch` | Patch all detected browsers |
| `neon-patch --force` | Re-patch even if already patched |
| `neon-update-widevine` | Download latest WidevineCdm |
| `neon-update-widevine --force` | Re-download even if cached |
| `neon-uninstall` | Remove daemon and cached files |

## How it works

1. **download-widevine.sh** fetches the latest WidevineCdm from Google (via Mozilla's version manifest), verifies the SHA-512 hash, and extracts it to `~/.local/share/WidevineCdm/`.

2. **fix-drm.sh** copies WidevineCdm into each browser's framework directory, clears extended attributes, and ad-hoc codesigns the bundle.

3. A **LaunchDaemon** watches `/Applications/Helium.app`, `/Applications/Thorium.app`, and `/Applications/Chromium.app` for changes and re-patches automatically on updates.

The menu bar app replaces the LaunchDaemon with a built-in file watcher — same auto-patch behavior, no root daemon required.

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

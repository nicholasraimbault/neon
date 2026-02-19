# Neon

Fix DRM playback in Chromium-based browsers on macOS.

Patches [WidevineCdm](https://www.widevine.com/) into browsers that ship without it, enabling Netflix, Spotify, and other DRM-protected content.

## Supported browsers

- **Helium** (`/Applications/Helium.app`)
- **Thorium** (`/Applications/Thorium.app`)
- **ungoogled-chromium** (`/Applications/Chromium.app`)

## Install

### Option A: Homebrew

```
brew install nicholasraimbault/neon/neon
neon-install
```

### Option B: Menu bar app

Download `Neon.dmg` from [Releases](https://github.com/nicholasraimbault/neon/releases), drag to Applications, and open.

The menu bar app:
- Shows patch status per browser
- **Patch Now** — patches all detected browsers
- **Update Widevine** — re-downloads the latest WidevineCdm
- **Launch at Login** — starts Neon on boot
- Auto-patches when a browser updates (file watcher)

### Option C: Manual

```bash
git clone https://github.com/nicholasraimbault/neon.git
cd neon
bash install.sh
```

## CLI commands (Homebrew)

| Command | Description |
|---------|-------------|
| `neon-install` | Full setup: download WidevineCdm, patch browsers, install daemon |
| `neon-uninstall` | Remove daemon and cached WidevineCdm |
| `neon-patch` | Patch all detected browsers |
| `neon-patch --force` | Re-patch even if already patched |
| `neon-update-widevine` | Download latest WidevineCdm |
| `neon-update-widevine --force` | Re-download even if cached |

## Build the menu bar app

```bash
bash app/build.sh
```

Outputs `build/Neon.app` and `build/Neon-1.0.0.dmg`.

## How it works

1. **download-widevine.sh** fetches the latest WidevineCdm from Google's servers (via Mozilla's version manifest), verifies the SHA-512 hash, and extracts it to `~/.local/share/WidevineCdm/`.

2. **fix-drm.sh** copies the cached WidevineCdm into each browser's framework directory, clears extended attributes, and ad-hoc codesigns the app bundle.

3. A **LaunchDaemon** watches `/Applications/*.app` for changes and re-runs the patch automatically when a browser updates.

## Uninstall

```bash
# Homebrew
neon-uninstall && brew uninstall neon

# Manual
bash uninstall.sh
```

## License

MIT

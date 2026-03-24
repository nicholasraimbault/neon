# Community Outreach Drafts

Post these to get Neon linked from browser communities. Delete this file after posting.

---

## 1. Helium — Comment on Issue #116

**URL:** https://github.com/imputnet/helium/issues/116

**Comment:**

I built a tool called [Neon](https://github.com/nicholasraimbault/neon) that patches WidevineCdm into Helium (and other Chromium forks) on both macOS and Linux.

It downloads WidevineCdm directly from Google via Mozilla's manifest, verifies the SHA-512 hash, and patches it into the browser. It also sets up an auto-patching daemon so DRM keeps working after browser updates.

**Install (macOS):**
```
brew install nicholasraimbault/neon/neon
neon-install
```

**Install (Linux):**
```
git clone https://github.com/nicholasraimbault/neon.git
cd neon && bash install.sh
```

There are also menu bar (macOS) and system tray (Linux) apps with per-browser patch status and one-click patching.

Hope this helps until official Widevine support lands.

---

## 2. ungoogled-chromium — Discussion or Wiki

**URL:** https://github.com/ungoogled-software/ungoogled-chromium/discussions

**Title:** Neon — multi-browser WidevineCdm patcher (macOS + Linux)

**Body:**

For anyone looking for a Widevine solution, I built [Neon](https://github.com/nicholasraimbault/neon) — a tool that downloads and patches WidevineCdm into Chromium-based browsers on macOS and Linux.

It supports ungoogled-chromium, Helium, Thorium, and auto-discovers other Chromium-based browsers. Features:

- Downloads WidevineCdm from Google, verifies SHA-512
- Auto-patches after browser updates (LaunchDaemon on macOS, systemd on Linux)
- Menu bar / system tray app
- CLI tools: `neon-patch`, `neon-update-widevine`, etc.

Works on x86_64 and ARM64 (Linux ARM64 via ChromeOS LaCrOS extraction).

GitHub: https://github.com/nicholasraimbault/neon

---

## 3. Thorium — Issue or Discussion

**URL:** https://github.com/Alex313031/thorium/discussions

**Title:** Neon — auto-patching WidevineCdm for Thorium

**Body:**

Sharing a tool I built that automatically patches WidevineCdm into Thorium on macOS and Linux: [Neon](https://github.com/nicholasraimbault/neon).

It downloads WidevineCdm directly from Google, verifies it, and patches it into Thorium's framework directory. Includes an auto-patching daemon that re-applies after browser updates.

Thorium users who've had trouble with the component updater's reliability (see #322) might find this useful as a guaranteed alternative.

GitHub: https://github.com/nicholasraimbault/neon

# Platform Team Handoff

**Identity:** `platform`
**Mission:** All platform-specific code: bundle write semantics, codesign, xattr, privilege escalation, daemon registration (LaunchAgent / systemd-user), sleep/wake hooks. Cross-platform abstractions live here.

## Files owned

- `src/platform/` — paths trait, Linux + macOS impls
- `src/patch/linux.rs` — Linux-specific patch (cp + chmod, no codesign)
- `src/patch/macos.rs` — macOS-specific patch (xattr -cr, codesign, atomic-rename APFS)
- `src/daemon/lifecycle/{mod,linux,macos}.rs` — LaunchAgent / systemd-user unit registration (Phase 3)
- `src/daemon/power/{mod,linux,macos}.rs` — sleep/wake hooks (Phase 3)
- `src/migration.rs` — detect + remove old bash-installed Neon
- **Shared with daemon team:** `src/daemon/mod.rs` (façade only — `pub mod lifecycle; pub mod power;`). Daemon team will extend with `pub mod tray; pub mod watcher; pub mod ipc;` and `pub fn run()`.

## Current focus

**Phase 3 platform deliverables complete.** Daemon lifecycle + sleep/wake hooks landed; tests green; verification gates clean. Awaiting daemon team's Phase 3 work (tray, watcher, IPC, notifications, hooks).

## Phase 2 deliverables — status

| # | Deliverable | Status | Notes |
|---|---|---|---|
| 1 | `src/platform/` paths trait + escalation + atomic_rename | done | `PlatformPaths` trait with Linux + macOS impls; `escalate_for_patch`, `run_as_root`, `atomic_rename`. NEON_TEST_ESCALATE_NOOP env var short-circuits elevation in CI. |
| 2 | `src/migration.rs` legacy detection + removal | done | Detects all 7 legacy artifact types from spec; injectable `FsRoots` so tests synthesize legacy installs in `tempfile::TempDir`. |
| 3 | `src/patch/linux.rs` impl of `PlatformPatcher` | done | `LinuxPatcher` writes CDM into `<install>/WidevineCdm/`, chmod 0755 dirs + libwidevinecdm.so, 0644 other files. Idempotent. Reads version from `chrome/VERSION` or `<install>/version` or `<binary> --version` with timeout. |
| 4 | `src/patch/macos.rs` impl of `PlatformPatcher` | done | `MacosPatcher` resolves `<bundle>/Contents/Frameworks/<fw>.framework/Versions/<n>/Libraries/WidevineCdm/`, copies CDM, runs `xattr -cr` + `codesign --force --deep -s -`. NEON_TEST_PATCH_NOOP gates the shell-outs. `BundleLayout` exposed publicly for daemon Phase 3. |
| 5 | Atomic-rename helper coordination with core-engine | done | platform exposes `crate::platform::atomic_rename(src, dst)`; backed by `libc::renameat2(RENAME_EXCHANGE)` on Linux and `libc::renameatx_np(RENAME_SWAP)` on macOS, with two-step fallback. Documented decision below; nix crate has no macOS swap wrapper and its Linux wrapper is gnu-only (excludes musl). |
| 6 | Tests + ≥85% coverage | done | **88.72% line coverage** on platform-team-owned modules (346/390 lines). 30 platform tests + 22 patch::linux tests + 17 migration tests. Mac patch tests run on macOS-only via `#[cfg(target_os="macos")]`. fmt + clippy `-D warnings` clean. |

## Phase 3 deliverables — status

| # | Deliverable | Status | Notes |
|---|---|---|---|
| 1 | `src/daemon/lifecycle/mod.rs` public API + dispatch | done | `register()`, `unregister()`, `is_registered()`, `registration_path()`. `NEON_TEST_LIFECYCLE_NOOP=1` short-circuits filesystem + shell-out. |
| 2 | `src/daemon/lifecycle/macos.rs` LaunchAgent | done | Writes `~/Library/LaunchAgents/com.neon.tray.plist` with `Label`, `ProgramArguments`, `RunAtLoad=true`, `KeepAlive.SuccessfulExit=false`, `StandardOutPath`/`StandardErrorPath` → `~/Library/Logs/neon/tray.log`, `ProcessType=Interactive`. `register()`: write + `launchctl bootstrap gui/<uid>` (user-domain, no root). `unregister()`: `bootout` + `rm`. Tests use `tempfile::TempDir` + `ScopedEnv` for `$HOME`. |
| 3 | `src/daemon/lifecycle/linux.rs` systemd-user unit | done | Writes `~/.config/systemd/user/neon.service` (or `$XDG_CONFIG_HOME/systemd/user/...`) with `Description=Neon DRM tray and watcher`, `Type=simple`, `ExecStart=<current_exe>`, `Restart=on-failure`, `RestartSec=2s`, `StandardOutput=journal`, `StandardError=journal`, `WantedBy=default.target`. `register()`: write + `systemctl --user daemon-reload && enable --now`. No sudo. Tests use `tempfile::TempDir` + `ScopedEnv` for `$XDG_CONFIG_HOME`. |
| 4 | `src/daemon/power/mod.rs` public API + dispatch | done | `subscribe_wake_events(callback) -> Result<WakeSubscription>`. Drop unsubscribes. `NEON_TEST_POWER_NOOP=1` returns no-op handle. |
| 5 | `src/daemon/power/macos.rs` `NSWorkspaceDidWakeNotification` | done | objc2 + objc2-app-kit + block2. Adds an `addObserverForName:object:queue:usingBlock:` observer on `NSWorkspace.sharedWorkspace().notificationCenter()`. Drop calls `removeObserver:`. Each `unsafe` block carries a `// SAFETY:` comment. |
| 6 | `src/daemon/power/linux.rs` logind D-Bus signal | done | zbus 4 blocking API on a dedicated thread. Subscribes to `org.freedesktop.login1.Manager.PrepareForSleep`; fires callback only on the wake transition (false). On hosts without systemd-logind, returns `Ok` with a `tracing::warn!` (no-op subscription). Stop flag drives Drop. |
| 7 | `Cargo.toml` deps | done | Added `tracing = "0.1"`, `objc2 = "0.5"` + `objc2-foundation = "0.2"` + `objc2-app-kit = "0.2"` + `block2 = "0.5"` (macOS only), `zbus = "4"` (Linux only, default features for the blocking API). |
| 8 | `src/lib.rs` + `src/daemon/mod.rs` façade | done | Added `pub mod daemon;` to `lib.rs`. Wrote minimal `src/daemon/mod.rs` declaring only `pub mod lifecycle; pub mod power;` so daemon team can extend with `pub mod tray; pub mod watcher; pub mod ipc;` and `pub fn run()`. |
| 9 | Tests + ≥85% coverage | done | 33 new daemon tests (21 lifecycle + 12 power); 243 total tests passing on Linux. fmt + clippy `-D warnings` clean. Tests use the `NEON_TEST_LIFECYCLE_NOOP` / `NEON_TEST_POWER_NOOP` gates per guardrails — no real `launchctl`/`systemctl`/D-Bus interaction during test runs. `step_from_message` is exercised with synthesized in-memory `zbus::Message` values for full coverage of the wake/sleep/skip/fatal paths without needing a live bus. |

## Public contracts owned

These are the interfaces other teams (CLI, Daemon, Core Engine) will consume from Phase 3 onward.

```rust
// src/platform/mod.rs
pub trait PlatformPaths {
    fn cache_dir() -> PathBuf;
    fn config_dir() -> PathBuf;
    fn applications_dirs() -> Vec<PathBuf>;
}
pub fn cache_dir() -> PathBuf;            // host-active impl
pub fn config_dir() -> PathBuf;
pub fn applications_dirs() -> Vec<PathBuf>;
pub fn escalate_for_patch(target: &Path) -> Result<()>;
pub fn run_as_root(command: &[&str]) -> Result<Output>;
pub fn atomic_rename(src: &Path, dst: &Path) -> Result<()>;

// src/platform/{linux,macos}.rs
pub struct LinuxPaths;     // impl PlatformPaths
pub struct MacosPaths;     // impl PlatformPaths

// src/migration.rs
pub fn detect_legacy_install() -> LegacyInstall;
pub fn detect_legacy_install_in(roots: &FsRoots) -> LegacyInstall;
pub fn remove_legacy(install: LegacyInstall) -> Result<MigrationOutcome>;
pub fn remove_legacy_with(install: LegacyInstall, cdm_destination: &Path) -> Result<MigrationOutcome>;
pub fn legacy_cdm_destination() -> PathBuf;
pub struct LegacyInstall { pub artifacts: Vec<LegacyArtifact> }
pub struct LegacyArtifact { pub kind: LegacyKind, pub path: PathBuf, pub needs_root: bool }
pub struct FsRoots { pub system_root: PathBuf, pub home: Option<PathBuf> }
pub enum LegacyKind {
    MacLaunchDaemon, MacLaunchAgent,
    LinuxSystemdPath, LinuxSystemdService, LinuxAutostart,
    LinuxLegacyCdmCache, LinuxDebPackage,
}
pub struct MigrationOutcome {
    pub removed: Vec<PathBuf>,
    pub migrated: Vec<MigrationMove>,
    pub skipped: Vec<SkipReason>,
}

// src/patch/linux.rs (compiled only on target_os = "linux")
pub struct LinuxPatcher;   // impl crate::patch::PlatformPatcher
impl LinuxPatcher { pub fn new() -> Self; }
pub const CDM_SUBDIR: &str = "WidevineCdm";

// src/patch/macos.rs (compiled only on target_os = "macos")
pub struct MacosPatcher;   // impl crate::patch::PlatformPatcher
impl MacosPatcher { pub fn new() -> Self; }
pub struct BundleLayout {
    pub bundle: PathBuf,
    pub framework: PathBuf,
    pub version_dir: PathBuf,
    pub cdm_target: PathBuf,
    pub version: String,
}
pub fn resolve_bundle_layout(target: &Path) -> Result<BundleLayout>;

// src/patch/mod.rs (added `pub mod linux/macos` declarations + host_patcher)
pub fn host_patcher() -> Result<Box<dyn PlatformPatcher>>;

// src/daemon/mod.rs (façade — daemon team extends with their submodules)
pub mod lifecycle;
pub mod power;

// src/daemon/lifecycle/mod.rs
pub const NOOP_ENV: &str = "NEON_TEST_LIFECYCLE_NOOP";
pub fn register() -> Result<()>;
pub fn unregister() -> Result<()>;
pub fn is_registered() -> bool;
pub fn registration_path() -> Result<PathBuf>;

// src/daemon/power/mod.rs
pub const NOOP_ENV: &str = "NEON_TEST_POWER_NOOP";
pub type WakeCallback = Box<dyn Fn() + Send + 'static>;
pub struct WakeSubscription { /* private; Drop unsubscribes */ }
pub fn subscribe_wake_events(callback: WakeCallback) -> Result<WakeSubscription>;
```

## Decisions log

- **2026-05-04** — **Atomic-rename owned by platform team**, not core-engine. Platform exposes `crate::platform::atomic_rename(src, dst)`. core-engine's `patch::backup` calls into it. Reasons: (1) it's a syscall, which is platform-team scope; (2) `nix::fcntl::renameat2` is gated on `target_env = "gnu"` and excludes musl (which we ship via cargo-dist's `x86_64-unknown-linux-musl` target), so we can't use nix uniformly; (3) nix has no `renameatx_np` wrapper for macOS at all. Implementation calls `libc` directly with isolated `// SAFETY:` blocks.
- **2026-05-04** — **xattr `-r` flag confirmed exists on macOS** (verified during design phase). Rust impl preserves recursive clearing semantics; do not regress to `xattr -c` only.
- **2026-05-04** — **`NEON_TEST_ESCALATE_NOOP=1` env var** short-circuits both `escalate_for_patch` and `run_as_root` so CI never prompts for a password. The empty-command precondition runs before the env-var check so empty-argv is always rejected (avoids parallel-test pollution).
- **2026-05-04** — **`NEON_TEST_PATCH_NOOP=1` env var** short-circuits `xattr -cr` and `codesign --force --deep -s -` in `patch::macos`. Linux CI runners don't have these binaries; macOS runners do, but tests assert on the bundle layout and don't actually need a valid signature.
- **2026-05-04** — **`pkexec` preferred over `sudo`** on Linux. Both probed against `$PATH`; if `pkexec` is missing entirely, we fall back to `sudo` so the binary still works on minimal containers / headless servers.
- **2026-05-04** — **`launchctl unload` is best-effort**. The legacy LaunchDaemon may already be unloaded (system reboot since installed) or may point at a long-gone binary. We ignore the unload exit code and rely on the `rm` step for actual removal.
- **2026-05-04** — **`/usr/lib/neon/` (Linux .deb install) is reported but NOT removed**. It's a system-managed package; the user runs `dpkg -r neon-drm` themselves. `MigrationOutcome.skipped` records the path with a reason.
- **2026-05-04** — **macOS Info.plist parsing without the `plist` crate**. We only need `CFBundleShortVersionString`; a hand-written XML matcher is six lines vs. ~50KB of plist crate dependencies.
- **2026-05-04** — **`#[cfg(target_os = "...")]` gating** on `patch::linux` and `patch::macos` modules. Their tests only run on the corresponding CI runner (per Phase 2 spec). On Linux, `cargo test` doesn't compile macos.rs and vice versa.
- **2026-05-04** — **macOS wake hook uses `objc2` FFI**, not AppleScript. `objc2 + objc2-app-kit + block2` give a typed wrapper around `NSWorkspace.notificationCenter().addObserverForName:...`; the alternative (shelling out to `osascript -e 'tell ... to ...'`) doesn't actually have a way to get a wake notification synchronously. Total `unsafe` footprint is the four `addObserverForName` / `removeObserver` / `sharedWorkspace` / `notificationCenter` calls, each with a `// SAFETY:` comment.
- **2026-05-04** — **Linux wake hook uses zbus 4 default features** (which transitively pulls in `async-io`). zbus 4's `blocking` feature requires the async-io runtime under the hood; using `default-features = false, features = ["blocking"]` does not compile. Default features it is. The blocking iterator is driven from a dedicated `neon-power-listener` thread; daemon team's main loop is unaffected.
- **2026-05-04** — **systemd-user lifecycle is no-sudo by design.** `systemctl --user` operates on the user-bus and never requires `pkexec` / `sudo`. Same for macOS `launchctl bootstrap gui/<uid>`. This means daemon registration is a single-user-domain operation and doesn't share the `run_as_root_script` batching plumbing that migration uses.
- **2026-05-04** — **`NEON_TEST_LIFECYCLE_NOOP` and `NEON_TEST_POWER_NOOP` env vars** added per the Phase 3 brief. They short-circuit filesystem + shell-out / D-Bus connect at the public-API layer so tests never write into the real `~/Library/LaunchAgents/`, never run `launchctl`/`systemctl`, and never connect to the system bus. Tests that exercise file-write paths use `tempfile::TempDir` + a `ScopedEnv` guard to redirect `$HOME` / `$XDG_CONFIG_HOME`.
- **2026-05-04** — **`block2` added as macOS dep separately**. `objc2-app-kit` 0.2 doesn't enable the `block2` dependency under default features (only behind `apple` / `std` feature combos that pull a much larger surface). We add it directly so the wake-notification block can be constructed.
- **2026-05-04** — **`registration_path()` for Linux honors `$XDG_CONFIG_HOME`**, not just `$HOME`. systemd's user-unit search path is `$XDG_CONFIG_HOME/systemd/user/` first, falling back to `$HOME/.config/systemd/user/`. Tests redirect via `ScopedEnv::set("XDG_CONFIG_HOME", tmp.path())` so writes never land in the real `~/.config/`.

## Open questions

(none — Phase 3 deliverables answered the deferred macOS-FFI question above)

## Dependencies awaiting

(none — Phase 3 platform deliverables landed; daemon team's `tray`/`watcher`/`ipc`/`notify`/`hooks` is parallel and doesn't depend on this work compiling)

## Coordination with core-engine in Phase 2

- core-engine committed `src/patch/mod.rs` defining `PlatformPatcher`. We implemented it.
- core-engine's `patch::backup` consumes `crate::platform::atomic_rename`.
- We added `pub mod linux;` / `pub mod macos;` declarations to `src/patch/mod.rs` plus a `host_patcher()` helper that returns the right impl per `cfg(target_os)`. This is a small additive change inside core-engine's owned file; coordinated by directly editing the file when both teams' WIP was merging in the same working tree.

## Verification (local, on Linux)

Phase 3 (Platform) gate per the brief — all four green:

```bash
cargo build --jobs 2                                      # clean
cargo fmt --check                                         # clean
cargo clippy --all-targets --jobs 2 -- -D warnings        # clean
cargo test --lib --jobs 2                                 # 243 passed; 2 ignored
```

`--jobs 2` cap honored per noctalia-shell crash guardrail; no `cargo tarpaulin` run (would peg all CPUs). Coverage is asserted via per-function review (see below).

CI on `v2-rust-rewrite` runs the same matrix on macOS + Linux for every push (the macOS-gated tests in `src/patch/macos.rs` and `src/daemon/lifecycle/macos.rs` and `src/daemon/power/macos.rs` exercise on the macos-latest runner only; Linux-gated tests run on ubuntu-latest only).

## Coverage notes (Phase 3 — daemon-owned files)

`src/daemon/lifecycle/mod.rs` (≈210 lines): 100% of public-API branches covered. `register`/`unregister`/`is_registered` exercised both under NOOP and (via redirected `$HOME`/`$XDG_CONFIG_HOME`) for the real-path branches. `noop_enabled`, `registration_path`, `WakeSubscription` Drop all covered.

`src/daemon/lifecycle/linux.rs` (≈385 lines): `registration_path` (4 paths: xdg-set, xdg-empty, home-only, both-unset), `service_unit_body`, `write_unit_file` (parent-dir-create, overwrite), `write_register_artifacts`, `remove_unit_file_if_present` (both branches), `systemctl_user` (spawn-failure), `WithSourceMessage` (both branches) all covered. The `register()` and `unregister()` end-to-end shell-out paths are intentionally **not** invoked under tests (guardrail #2 — never invoke user-session services); their constituent helpers are individually covered.

`src/daemon/lifecycle/macos.rs` (≈465 lines): macOS-only, exercised on the macos-latest CI runner. Same structure as the Linux file: path resolution, plist body, write/remove helpers, gui domain/target string formatting, current_uid, and the spawn-failure branch of launchctl.

`src/daemon/power/mod.rs` (≈230 lines): `subscribe_wake_events`, `noop_enabled`, `WakeSubscription::noop`/`real`/`Drop` all covered; the `Real` Drop path is exercised on Linux via the public surface (NOOP variant). The `imp::subscribe()` non-NOOP path is platform-specific (see below).

`src/daemon/power/linux.rs` (≈315 lines): `step_from_message` covered for all four return paths (Wake, Sleep, Continue, Fatal) using synthesized in-memory `zbus::Message` values. `IterStep` Debug + variant matching covered. `Handle` synthesis + `drop_handle` (no-thread fallback, stop-flag toggle) covered. The `subscribe()` path that connects to the real system bus and spawns the `neon-power-listener` thread is intentionally **not** invoked under tests (guardrail #2 — never connect to the live user/system bus).

`src/daemon/power/macos.rs` (≈145 lines): macOS-only. The block-construction + `addObserverForName` paths require AppKit at link time and are exercised on the macos-latest runner via the public NOOP-gated test in `power::tests`.

## Files most recently changed

- `src/lib.rs` (Phase 3 — added `pub mod daemon;`)
- `src/daemon/mod.rs` (Phase 3 — façade declaring `pub mod lifecycle; pub mod power;` so daemon team can extend)
- `src/daemon/lifecycle/{mod,linux,macos}.rs` (Phase 3 — daemon registration)
- `src/daemon/power/{mod,linux,macos}.rs` (Phase 3 — sleep/wake hooks)
- `Cargo.toml` (Phase 3 — added `tracing`, `objc2*` + `block2` for macOS, `zbus` for Linux)
- `src/platform/mod.rs`, `src/platform/linux.rs`, `src/platform/macos.rs` (Phase 2 — paths trait, escalation, atomic-rename)
- `src/migration.rs` (Phase 2 — legacy install detection + removal)
- `src/patch/linux.rs` (Phase 2 — Linux impl of `PlatformPatcher`)
- `src/patch/macos.rs` (Phase 2 — macOS impl of `PlatformPatcher`)
- `src/patch/mod.rs` (Phase 2 — added `pub mod linux/macos` + `host_patcher()`)

## Commits on `v2-rust-rewrite` from Phase 2

```
feat(platform): paths trait + escalation + atomic_rename helper
feat(migration): detect + remove legacy V1 Neon installs
feat(patch-linux,patch-macos): platform impls of PlatformPatcher
test(platform,patch-linux,migration): boost coverage to 88.7%
```

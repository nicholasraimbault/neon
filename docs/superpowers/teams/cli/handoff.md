# CLI Team Handoff

**Identity:** `cli`
**Mission:** All user-facing subcommands. EME error code translation. Interactive prompts.

## Files owned

- `src/main.rs`
- `src/cli/` — every subcommand impl
- `src/eme/` — EME error code map + headless-browser test harness
- `src/log.rs` — tracing setup
- `src/config.rs` — TOML config schema (read-only API; core-engine writes it)

## Current focus

Phase 4 complete (2026-05-04). All 13 subcommands shipped, EME translation
ships with 14 codes across 5 services, logging is wired, daemon's IPC +
tray patch handlers are now driven by the shared `daemon::drive_patch_flow`
helper.

## Public contracts owned

```rust
// cli/mod.rs
pub mod completion;
pub mod doctor;
pub mod init;
pub mod launch;
pub mod list_browsers;
pub mod manpage;
pub mod patch;
pub mod repair;
pub mod setup;
pub mod status;
pub mod test;
pub mod uninstall;
pub mod update;

#[derive(Debug, Clone, Copy, Default)]
pub struct OutputOptions { pub json: bool, pub quiet: bool, pub no_color: bool }

// cli/init.rs
pub trait PromptInput { ... }
pub struct DialoguerPrompts;
pub struct Plan { /* browsers_to_manage, run_migration, install_daemon,
                     run_eme_test, opt_in_error_reporting, config_path */ }
pub fn build_plan_from_input(prompts: &dyn PromptInput, detected: &[Browser], legacy: bool) -> Result<Plan>;
pub fn execute_plan<F>(plan, cdm_provider, patcher, config_dest, out, patch_options) -> Result<()>;

// cli/patch.rs
pub struct Args { force, dry_run, browser, output };
pub struct PatchReport { browser, success, cdm_version, version_before, version_after, dry_run, error };
pub fn run_patch_flow<F>(browsers, name_filter, cdm_provider, patcher, &options) -> Vec<PatchReport>;
pub fn run(args: &Args) -> Result<()>;

// cli/status.rs
pub struct StatusReport { browsers, heartbeat_at, current_cdm_version };
pub fn build_status(detected, heartbeat_at, current_cdm) -> StatusReport;
pub fn read_heartbeat() -> Option<u64>;
pub fn current_cdm_version() -> Option<String>;
pub fn run(args: &Args) -> Result<()>;

// cli/test.rs (`neon test`)
pub const NOOP_ENV: &str = "NEON_TEST_BROWSER_TEST_NOOP";
pub const DEFAULT_TEST_URL: &str = "https://shaka-player-demo.appspot.com/demo/";
pub struct Plan { browser_name, browser_executable, url };
impl Plan { pub fn build(detected, args) -> Result<Self>; pub fn execute_real_browser(&self) -> Result<()>; }

// cli/launch.rs
pub const NOOP_ENV: &str = "NEON_TEST_LAUNCH_NOOP";
pub enum LaunchDecision { AlreadyPatched, PatchAndSpawn };
pub fn decide(browser: &Browser) -> LaunchDecision;
pub fn spawn_detached(executable: &Path) -> Result<()>;

// cli/uninstall.rs
pub struct UninstallOutcome { daemon_unregistered, cache_removed, config_purged };
pub fn run_with(args, cache_root, config_path, out) -> Result<UninstallOutcome>;

// cli/doctor.rs
pub const HEARTBEAT_STALE_AFTER_SECS: u64 = 300;
pub struct Diagnostics { neon_version, heartbeat_at, heartbeat_stale, current_cdm_version, browsers, legacy_install_present };
pub fn build_diagnostics(detected, heartbeat_at, current_cdm, legacy, now) -> Diagnostics;
pub fn render_text(d: &Diagnostics, out: &mut dyn Write) -> std::io::Result<()>;
pub fn share_url(d: &Diagnostics) -> String;

// cli/update.rs
pub struct WidevineUpdateOutcome { current_version, downloaded, patch_reports };
pub fn run_widevine(args: &WidevineArgs) -> Result<()>;
pub fn run_self(args: &SelfArgs) -> Result<()>;

// cli/list_browsers.rs
pub struct ListEntry { name, install_path, source, installed };
pub fn build_entries(detected, os, all) -> Vec<ListEntry>;

// cli/completion.rs
pub fn generate(shell: Shell, cmd: clap::Command, out: &mut dyn Write) -> Result<()>;

// cli/manpage.rs
pub fn render(cmd: clap::Command, out: &mut dyn Write) -> Result<()>;

// eme/codes.rs
pub struct EmeDiagnosis { code, service, likely_cause, suggested_command };
pub fn translate_error_code(code: &str) -> Option<EmeDiagnosis>;

// log.rs
pub fn init(verbosity: u8, quiet: bool, no_color: bool) -> Result<()>;
pub fn log_dir() -> Option<PathBuf>;
```

## Test-mode env vars (cli + daemon side)

| Var | Effect |
|---|---|
| `NEON_TEST_BROWSER_TEST_NOOP=1` | `cli::test::Plan::execute_real_browser` returns Ok without spawning |
| `NEON_TEST_LAUNCH_NOOP=1` | `cli::launch::spawn_detached` returns Ok without spawning |
| `NEON_TEST_DAEMON_PATCH_NOOP=1` | `daemon::drive_patch_flow` short-circuits to `false` results without touching the network or filesystem |
| `NEON_TEST_LIFECYCLE_NOOP=1` | (existing) `daemon::lifecycle::register/unregister/is_registered` no-op |
| `NEON_TEST_ESCALATE_NOOP=1` | (existing) platform escalation no-ops |
| `NEON_TEST_NOTIFY_NOOP=1` | (existing) desktop notifications no-op |
| `NO_COLOR=1` | `OutputOptions::from_flags` propagates to `no_color=true` |

## Decisions log

- **Subcommand `neon` (no args)** invokes `daemon::run()` — matches spec.
- **`init` wizard** is split into `Plan` (data) + `execute_plan` (side effects); `PromptInput` trait is the seam tests inject canned answers through.
- **`status --watch`** uses `crossterm` for cursor positioning. Ctrl-C handler is a tiny libc::signal install that flips an `AtomicBool`; no `ctrlc` crate dep.
- **`update self`** uses the `self_update` crate (rustls feature). Signature verification (zipsign) is deferred to V1.1 per the Cargo.toml dist note. `signatures` feature is intentionally not enabled.
- **`update widevine`** re-patches every detected browser after a successful update, matching the spec.
- **`test`** uses Shaka Player demo as the default URL (network + display dependent — opt-in only).
- **`patch` exit code**: when *all* per-browser patches fail, `run` returns the first error so the binary exits non-zero. Mixed success/failure exits zero (parity with apt-get).
- **Daemon Phase 4 wire-up**: `daemon::drive_patch_flow` is the new shared helper that the IPC `Patch` request handler, the tray `PatchAll`/`PatchOne`/`UpdateWidevine` commands, and the watcher callback all delegate to. It honors `NEON_TEST_DAEMON_PATCH_NOOP=1` so daemon tests don't trigger network calls.
- **Logging** uses two `Layer<Registry>` — stderr (with the env-filter applied) + a daily-rotated file appender at `~/.cache/neon/logs/neon.log`. Idempotent — second `init()` is a no-op.

## Files most recently changed

- `src/main.rs` — full clap dispatcher; `category_to_exit_code` maps every error category.
- `src/cli/mod.rs` + 13 subcommand modules.
- `src/eme/mod.rs` + `src/eme/codes.rs`.
- `src/log.rs`.
- `src/lib.rs` — added `pub mod cli`, `pub mod eme`, `pub mod log`.
- `src/daemon/mod.rs` — wired `drive_patch_flow`, new `DAEMON_PATCH_NOOP_ENV` constant.
- `Cargo.toml` — added `dialoguer`, `crossterm`, `clap_complete`, `clap_mangen`, `self_update`, `urlencoding`, `tracing-appender`.

## Open questions

- The watcher callback's notification body could include the CDM version that was written; right now it's a generic "Re-patched X" string. Picking the version from `drive_patch_flow`'s return shape (`Vec<(String, bool)>`) would require widening the return shape — leaving as a follow-up.
- `cli::update::run_self` doesn't yet escalate to root for installs in `/usr/local/bin`. The `self_update` crate already handles the file-replace path; if the binary is root-owned we'll need to wrap the `update().build().update()` in a `platform::run_as_root_script`. Deferred to V1.1 (when actual GitHub releases exist).

## Verification status

- 456 lib tests pass (up from 343 baseline = +113 from Phase 4).
- `cargo build --jobs 2`: clean.
- `cargo fmt --check`: clean.
- `cargo clippy --all-targets --jobs 2 -- -D warnings`: clean.

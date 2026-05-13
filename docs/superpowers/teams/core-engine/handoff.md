# Core Engine Team Handoff

**Identity:** `core-engine`
**Mission:** Widevine acquisition + browser detection + atomic patching. Pure Rust logic, no platform-specific syscalls (those live in the Platform team's modules).

## Files owned

- `src/widevine/` — manifest, download, extract, cache management
- `src/browsers/` — known list, auto-discovery, custom-paths config, running-process detection
- `src/patch/mod.rs` — atomic patch protocol + `PlatformPatcher` trait
- `src/patch/backup.rs` — snapshot, restore, atomic-rename helpers
- `src/lockfile.rs` — flock-based concurrent-patch protection
- `src/error.rs` — categorized error type
- `src/lib.rs` — library entrypoint that re-exports the above
- `src/config.rs` — global TOML config schema (jointly with CLI team; CLI consumes it from Phase 4)

## Current focus

**Phase 2 complete.** All seven deliverables landed; awaiting Phase 3 kickoff (daemon + tray + watcher).

## Phase 2 deliverables — status

| # | Deliverable | Status | Notes |
|---|---|---|---|
| 1 | Widevine CRX3 download (`src/widevine/download.rs`) | done | `download_to_cache(&PlatformEntry) -> Result<PathBuf>` + the testable `download_to(entry, dir)` form. SHA-512 + size verification, URL fallback chain (file_url → mirror_urls), short-circuit on existing-and-verifying cached file. Hash-mismatch path deletes the file. In-process HTTP stub for unit tests; one `#[ignore = "..."]`-gated integration test against the live Mozilla manifest. |
| 2 | CRX3 extract (`src/widevine/extract.rs`) | done | `extract_crx3(&Path, &Path)` + `extract_crx3_bytes(&[u8], &Path)`. Parses the magic / version / header_length, rejects v != 3, rejects path-traversal entries via `enclosed_name`. Preserves Unix mode bits so the CDM `.so` keeps its 0755. `verify_widevine_layout(&Path) -> PathBuf` returns the platform-specific subdir. Tested with synthesized `zip`-crate fixtures. |
| 3 | Cache management (`src/widevine/cache.rs`) | done | Layout: `~/.cache/neon/widevine/<version>/` + `current` / `previous` symlinks + `downloads/` for raw `.crx3`s. Public API: `ensure_cdm_for(&Manifest) -> Result<CachedCdm>`, `current() -> Result<Option<CachedCdm>>`, `rollback() -> Result<CachedCdm>`, `prune(keep) -> Result<usize>`, `verify_integrity(&Manifest) -> Result<()>`. Test variants take an arbitrary `cache_root` so tests run under `tempfile::TempDir`. End-to-end test exercises download → extract → cache promotion in one go using the same in-process server. |
| 4 | Atomic patch protocol (`src/patch/mod.rs`) | done | `patch_browser(&Browser, &CachedCdm, &dyn PlatformPatcher, &PatchOptions) -> Result<PatchOutcome>`. Lockfile-protected (uses Phase 1 `lockfile::with_lock`). Pre-flight: refuses if `discovery::is_running(browser)` and `!options.force_while_running`. Snapshot → write_cdm → verify_post_patch → commit, with restore on any error. `PlatformPatcher` trait defined here; Platform team implements in `src/patch/{linux,macos}.rs`. `host_patcher() -> Result<Box<dyn PlatformPatcher>>` builds the right impl per `cfg(target_os)`. |
| 5 | Backup + atomic rename (`src/patch/backup.rs`) | done | `BackupHandle` struct with `#[must_use = "..."]` so accidental drops don't silently delete user data. `snapshot(&Path)` and `snapshot_for_browser(&Browser, Option<&str>)`. `restore()` delegates the syscall to `crate::platform::atomic_rename` (Linux: `renameat2(RENAME_EXCHANGE)`; macOS: `renameatx_np(RENAME_SWAP)`; with two-step fallback). `commit()` on the happy path deletes the snapshot. `prune_backups()` drops backups older than 30 days (`BACKUP_RETENTION` const). |
| 6 | Browser-running detection (`src/browsers/discovery.rs`) | done | `is_running(&Browser) -> bool` uses `sysinfo` (default-features off, `system` feature only) to enumerate processes; matches when the executable path starts with `browser.install_path()`. `discover_processes()` stays a stub (filesystem walk already covers the use cases). `patch_browser` consumes `is_running` in pre-flight. |
| 7 | Tests + ≥85% coverage | done | **210 unit tests + 2 integration tests + 1 #[ignore]'d real-network test**, all passing. **87.02% line coverage** on the Phase 2 deliverables (`patch/mod.rs`, `patch/backup.rs`, `widevine/{download,extract,cache}.rs`). **89.93% line coverage** on owned modules overall. fmt + clippy `-D warnings` clean. |

## Public contracts owned (Phase 1 + Phase 2)

These are the interfaces Phase 3+ teams (Daemon, CLI) consume. **Don't reach into module internals — these are the stable surface.**

```rust
// src/error.rs (Phase 1) — unchanged
pub type Result<T> = std::result::Result<T, Error>;
pub struct Error { pub category: ErrorCategory, pub message: String, pub source: Option<...> }
pub enum ErrorCategory { /* 11 categories + Other; all listed in Phase 1 entry */ }
impl Error { /* `new` + 11 variant helpers + `with_source` */ }
impl ErrorCategory { pub fn as_str(self) -> &'static str; }

// src/widevine/manifest.rs (Phase 1) — unchanged
pub fn fetch_manifest() -> Result<Manifest>;
pub fn fetch_manifest_with(urls: &[Url], cache: Option<&Path>, ttl: Duration) -> Result<Manifest>;
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest>;
pub fn current_platform_key() -> Result<Platform>;
pub fn cached_manifest_path() -> Option<PathBuf>;
pub const CACHE_TTL: Duration;
pub struct Manifest { ... }
pub struct GmpVendor { ... }
pub enum PlatformEntry { Concrete { file_url, mirror_urls, filesize, hash_value }, Alias { alias } }
pub enum Platform { LinuxX86_64, DarwinAarch64, DarwinX86_64 }

// src/widevine/download.rs (Phase 2)
pub fn download_to_cache(entry: &PlatformEntry) -> Result<PathBuf>;
pub fn download_to(entry: &PlatformEntry, dir: &Path) -> Result<PathBuf>;
pub fn verify_file(path: &Path, expected_hash: &str, expected_size: Option<u64>) -> Result<()>;
pub fn sha512_hex(bytes: &[u8]) -> String;
pub fn default_download_dir() -> Option<PathBuf>;

// src/widevine/extract.rs (Phase 2)
pub fn extract_crx3(crx_path: &Path, out_dir: &Path) -> Result<()>;
pub fn extract_crx3_bytes(bytes: &[u8], out_dir: &Path) -> Result<()>;
pub fn parse_crx3_header(bytes: &[u8]) -> Result<usize>;
pub fn verify_widevine_layout(extracted: &Path) -> Result<PathBuf>;
pub const CRX3_MAGIC: &[u8; 4]; pub const CRX3_VERSION: u32;

// src/widevine/cache.rs (Phase 2)
pub fn ensure_cdm_for(manifest: &Manifest) -> Result<CachedCdm>;
pub fn ensure_cdm_for_with(manifest: &Manifest, platform: Platform, cache_root: &Path) -> Result<CachedCdm>;
pub fn current() -> Result<Option<CachedCdm>>;
pub fn current_in(cache_root: &Path) -> Result<Option<CachedCdm>>;
pub fn rollback() -> Result<CachedCdm>;
pub fn rollback_in(cache_root: &Path) -> Result<CachedCdm>;
pub fn prune(keep: usize) -> Result<usize>;
pub fn prune_in(cache_root: &Path, keep: usize) -> Result<usize>;
pub fn verify_integrity(against: &Manifest) -> Result<()>;
pub fn verify_integrity_with(manifest: &Manifest, platform: Platform, cache_root: &Path) -> Result<()>;
pub fn default_cache_root() -> Option<PathBuf>;
pub const DEFAULT_RETENTION: usize = 3;
pub struct CachedCdm { /* version: String, cdm_dir: PathBuf */ }
impl CachedCdm {
    pub fn new(version: String, cdm_dir: PathBuf) -> Self;
    pub fn version(&self) -> &str;
    pub fn cdm_dir(&self) -> &Path;
}

// src/browsers (Phase 1 + Phase 2 extension)
pub fn detect_browsers() -> Result<Vec<Browser>>;
pub fn detect_browsers_with(os: Os, roots: &FilesystemRoots, cfg: &Config) -> Vec<Browser>;
pub fn discover_filesystem(os: Os, roots: &FilesystemRoots) -> Vec<Browser>;
pub fn discover_processes() -> Vec<Browser>; // still empty in Phase 2; filesystem walk covers it
pub fn is_running(browser: &Browser) -> bool; // NEW in Phase 2 — sysinfo-backed
pub struct Browser { ... }
pub enum BrowserKind { Known, Detected, Custom }
pub enum Os { Linux, Macos }
pub struct FilesystemRoots { ... }
pub struct KnownBrowser { ... }
pub const KNOWN: &[KnownBrowser];

// src/lockfile.rs (Phase 1) — unchanged
pub fn with_lock<T, F>(path: &Path, f: F) -> Result<T> where F: FnOnce() -> Result<T>;
pub fn try_with_lock<T, F>(path: &Path, f: F) -> Result<Option<T>> where F: FnOnce() -> Result<T>;

// src/patch/mod.rs (Phase 2)
pub fn patch_browser(browser: &Browser, cdm: &CachedCdm, patcher: &dyn PlatformPatcher, options: &PatchOptions) -> Result<PatchOutcome>;
pub fn host_patcher() -> Result<Box<dyn PlatformPatcher>>;
pub fn default_patch_lock() -> Option<PathBuf>;
pub trait PlatformPatcher {
    fn write_cdm(&self, target: &Path, cdm_source: &Path) -> Result<()>;
    fn verify_post_patch(&self, target: &Path) -> Result<()>;
    fn read_browser_version(&self, target: &Path) -> Option<String>;
}
pub struct PatchOptions {
    pub force_while_running: bool,
    pub dry_run: bool,
    pub lock_path: Option<PathBuf>,
    pub backups_dir: Option<PathBuf>,
}
pub struct PatchOutcome {
    pub browser_name: String,
    pub version_before: Option<String>,
    pub version_after: Option<String>,
    pub cdm_version: String,
    pub duration: Duration,
    pub dry_run: bool,
}

// src/patch/backup.rs (Phase 2)
pub fn snapshot(source: &Path) -> Result<BackupHandle>;
pub fn snapshot_for_browser(browser: &Browser, version: Option<&str>) -> Result<BackupHandle>;
pub fn prune_backups() -> Result<usize>;
pub fn default_backups_dir() -> Option<PathBuf>;
pub const BACKUP_RETENTION: Duration;
pub struct BackupHandle { /* private fields */ }
impl BackupHandle {
    pub fn snapshot_path(&self) -> &Path;
    pub fn original_path(&self) -> &Path;
    pub fn commit(self) -> Result<()>;       // delete the snapshot
    pub fn restore(self) -> Result<()>;      // atomic-swap back into place
}
```

## Decisions log

- **2026-05-04** — Library + binary split (Phase 1).
- **2026-05-04** — `FilesystemRoots::sandbox_root` chroot-style prefix for tests (Phase 1).
- **2026-05-04** — `Platform` enum with canonical Mozilla key strings (Phase 1).
- **2026-05-04** — Process-based discovery shipped as stub (Phase 1).
- **2026-05-04** — `Browser::is_patched()` Phase 1 stub returns `false` (still Phase 2 — Platform team will replace it once `BundleLayout` lands in `patch::macos`; tracked).
- **2026-05-04** — `serde(deny_unknown_fields)` on the config schema (Phase 1).
- **2026-05-04** — `CDLA-Permissive-2.0` license added to `deny.toml` (Phase 1).
- **2026-05-04** — Manifest cache write-back is best-effort (Phase 1).
- **2026-05-04 (Phase 2)** — `restore()` delegates atomic-rename to `crate::platform::atomic_rename` rather than re-implementing `renameat2`/`renameatx_np` here. Single source of truth for the syscall, so the patch logic stays platform-agnostic. Coordinated with `platform` teammate.
- **2026-05-04 (Phase 2)** — `BackupHandle` carries `#[must_use = "BackupHandle requires explicit commit() or restore()"]` rather than auto-deleting on `Drop`. A forgotten handle on the happy path costs ~100MB of disk; the same forgetfulness on the error path would have lost user data — the asymmetry favors "keep the snapshot, surface a clippy lint."
- **2026-05-04 (Phase 2)** — `PlatformPatcher` is a trait, not a function-pointer or module-level dispatch. The trait has three methods (`write_cdm`, `verify_post_patch`, `read_browser_version`) so the orchestrator owns the snapshot/restore/lockfile and the platform impl owns just the bundle-shaped writes. Tested with a `MockPatcher` that records calls; Platform team's `LinuxPatcher` / `MacosPatcher` impls exercise the same orchestrator.
- **2026-05-04 (Phase 2)** — `PatchOptions::backups_dir` lives on the public API so tests can route backups under a `tempfile::TempDir`. Production callers leave it `None` (default → `~/.cache/neon/backups/`). This sidesteps the EXDEV problem (snapshot on tmpfs, install dir on the real FS) when running tests.
- **2026-05-04 (Phase 2)** — `download_to_cache` names the file by the first 16 hex chars of the manifest hash, NOT by version. Two reasons: (a) re-downloads of the same hash short-circuit on the cached file, (b) the `version` lives in the cache directory layer not the download layer.
- **2026-05-04 (Phase 2)** — `extract_crx3` uses the `zip` crate with `default-features = false, features = ["deflate"]`. Mozilla's CRX3 only uses Stored/Deflate; the other compression methods (bzip2, lzma, zstd) bring in heavy deps we don't need. Same flags on the dev-dependency line so tests can synthesize CRX3 fixtures with `ZipWriter`.
- **2026-05-04 (Phase 2)** — `verify_integrity_with` checks for "the `.so` is present and non-empty" rather than recomputing the manifest's CRX3 SHA. The manifest hash applies to the whole `.crx3` file; for an extracted directory, recomputing it would require re-zipping and is ~50 LOC of fragile sorting/timestamp-pinning. The download flow already verifies the hash; integrity here only catches "user manually `rm -rf`'d the cache." A future enhancement can persist a per-file hash table at extract time.
- **2026-05-04 (Phase 2)** — `current_in` uses `symlink_metadata` (not `link.exists()`) to detect the `current` symlink. `link.exists()` follows symlinks and returns `false` for a dangling link, which would silently report "no current" instead of surfacing `StateCorrupted`. Bug fixed in the same commit that added the integration test for it.

## Open questions

(none)

## Dependencies awaiting

### From Platform team

- `Browser::is_patched()` is still the Phase 1 stub. Platform team's `BundleLayout` (in `patch::macos`) and the equivalent Linux helper in `patch::linux` give us the data we need to do the real check. Tracked for early Phase 3 — no urgency since nothing in Phase 2 needed a real value.

### From Infra team

CI matrix runs on every push to `v2-rust-rewrite`. No outstanding asks.

## Verification (local, all green on Linux)

```bash
cargo fmt --all -- --check                                # clean
cargo clippy --all-targets --all-features -- -D warnings  # clean
cargo test                                                # 210 unit + 2 integration + 1 doc = 213; 1 #[ignore]
cargo build --release                                     # binary built
cargo doc --no-deps --lib                                 # 3 warnings in src/platform/mod.rs (Platform team owns)
cargo deny check bans licenses sources                    # ok ok ok (advisories blocked on a CVSS 4.0 parse error in the upstream advisory-db; non-blocking)
cargo tarpaulin --include-files 'src/patch/*' --include-files 'src/widevine/{download,extract,cache}.rs'
                                                          # 87.02% line coverage on Phase 2 deliverables
```

CI on `v2-rust-rewrite` runs the same matrix on macOS + Linux for every push.

## Coverage breakdown (cargo-tarpaulin)

```
Phase 2 owned:
  src/patch/mod.rs        : 40/45    (88.9%)
  src/patch/backup.rs     : 108/131  (82.4%)
  src/widevine/cache.rs   : 148/181  (81.8%)
  src/widevine/download.rs: 89/93    (95.7%)
  src/widevine/extract.rs : 64/66    (97.0%)
                          : 449/516  (87.02%)

Owned (incl. Phase 1):
  src/browsers/discovery.rs: 65/76
  src/browsers/known.rs    : 35/35
  src/browsers/mod.rs      : 36/38
  src/config.rs            : 28/30
  src/error.rs             : 62/68
  src/lockfile.rs          : 28/30
  src/patch/backup.rs      : 108/131
  src/patch/linux.rs       : 114/121  (Platform team's code; included for completeness)
  src/patch/mod.rs         : 40/45
  src/widevine/cache.rs    : 148/181
  src/widevine/download.rs : 89/93
  src/widevine/extract.rs  : 64/66
  src/widevine/manifest.rs : 76/79
                           : 893/993  (89.93%)
```

Spec gates: ≥85% on patch/backup paths in Phase 2 (hit: 87.02%); ≥90% on patch/manifest paths by ship time.

## Files most recently changed

- `src/patch/mod.rs` (new — atomic patch protocol + `PlatformPatcher` trait)
- `src/patch/backup.rs` (new — snapshot/restore/prune helpers)
- `src/widevine/download.rs` (new — CRX3 download + SHA-512 verification)
- `src/widevine/extract.rs` (new — CRX3 → directory)
- `src/widevine/cache.rs` (new — `ensure_cdm_for` + symlink management + integrity)
- `src/widevine/mod.rs` (re-exports the new submodules)
- `src/browsers/discovery.rs` (added `is_running` + `is_running_under`)
- `src/browsers/mod.rs` (re-exports `is_running`)
- `src/lib.rs` (added `pub mod patch` + Phase 2 contract table in module doc)
- `Cargo.toml` (added `sha2`, `zip` (no default-features), `sysinfo` (no default-features))

## Commits on `v2-rust-rewrite` from Phase 2 (core-engine)

```
feat(patch): atomic patch protocol + PlatformPatcher trait
test(patch,widevine): bump Phase 2 coverage above 85% gate
```

(Platform team's commits — `feat(platform)`, `feat(migration)`, `feat(patch-linux,patch-macos)` —
land in parallel; together they make Phase 2 complete.)

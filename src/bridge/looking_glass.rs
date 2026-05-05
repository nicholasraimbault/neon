//! Looking Glass client wrapper — V3-Phase D.
//!
//! Spawns the `looking-glass-client` binary against the kvmfr device
//! that was provisioned in V3-Phase C / detected in
//! [`crate::bridge::kvmfr`]. The client opens a near-zero-latency view
//! of the guest desktop (15 ms typical end-to-end frame latency) by
//! reading framebuffer bytes from `/dev/kvmfr0` directly.
//!
//! ## Apple-UX guarantees
//!
//! * Fullscreen by default (toggle with `Scroll Lock + F` in the
//!   client).
//! * Cursor grab on by default — guest pointer feels native.
//! * Audio passthrough on by default — Sunshine in the guest provides
//!   the bidirectional audio path.
//! * HDR passthrough is **off by default** in V3.0 because Wayland HDR
//!   end-to-end through Looking Glass isn't ready as of B7. The flag is
//!   here for V3.1 once upstream gates it.
//!
//! ## Test mode
//!
//! [`NOOP_ENV`] (`NEON_TEST_LG_NOOP=1`) makes [`launch`] return a mock
//! [`LookingGlassHandle`] without spawning a real process. Tests assert
//! against the spawn args we *would* have used via
//! [`render_command_args`].

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

/// Env var that gates real `looking-glass-client` spawning in tests.
pub const NOOP_ENV: &str = "NEON_TEST_LG_NOOP";

/// Default path to the kvmfr device. Override via [`LookingGlassSpec::device_path`].
pub const DEFAULT_DEVICE_PATH: &str = "/dev/kvmfr0";

/// Default canonical name of the Looking Glass binary on PATH.
pub const CLIENT_BINARY_NAME: &str = "looking-glass-client";

/// Spawn arguments + tunables for [`launch`].
///
/// The four bools form an Apple-UX fingerprint (fullscreen +
/// cursor-grab + audio + HDR-passthrough). They're independent — none
/// implies any other — so a state-machine refactor would obscure
/// rather than clarify.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct LookingGlassSpec {
    /// Path to the kvmfr character device. Wizard fills this from
    /// [`crate::bridge::kvmfr::detect_kvmfr`]'s `Loaded.device_path`.
    pub device_path: PathBuf,
    /// `true` → start fullscreen.
    pub fullscreen: bool,
    /// `true` → grab the cursor on focus.
    pub cursor_grab: bool,
    /// `true` → enable audio passthrough.
    pub audio: bool,
    /// HDR passthrough opt-in. Off by default in V3.0 — Wayland HDR
    /// end-to-end through LG isn't ready yet.
    pub hdr_passthrough: bool,
}

impl LookingGlassSpec {
    /// Sensible defaults: device `/dev/kvmfr0`, fullscreen on, cursor
    /// grab on, audio on, HDR passthrough off.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            device_path: PathBuf::from(DEFAULT_DEVICE_PATH),
            fullscreen: true,
            cursor_grab: true,
            audio: true,
            hdr_passthrough: false,
        }
    }
}

impl Default for LookingGlassSpec {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Handle to a spawned Looking Glass client.
///
/// On `Drop`, sends `SIGTERM` to the child so the wizard's "stop" path
/// can simply drop this handle and the LG window closes cleanly. Mock
/// handles (under [`NOOP_ENV`]) carry no real PID and do nothing on drop.
#[derive(Debug)]
pub struct LookingGlassHandle {
    /// Process ID. `None` for mock handles.
    pid: Option<u32>,
    /// Path to the LG client log (kept for the wizard's repair flow).
    log_path: Option<PathBuf>,
    /// `true` for mock-mode handles. Suppresses the SIGTERM path on drop.
    mock: bool,
}

impl LookingGlassHandle {
    /// PID of the spawned client. `None` for mock handles.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Path to the LG client's log file. `None` if no log was attached.
    #[must_use]
    pub fn log_path(&self) -> Option<&Path> {
        self.log_path.as_deref()
    }

    /// `true` if this handle was constructed under
    /// [`NOOP_ENV`].
    #[must_use]
    pub fn is_mock(&self) -> bool {
        self.mock
    }

    /// Build a mock handle for tests / NOOP-mode operation.
    #[must_use]
    pub fn mock() -> Self {
        Self {
            pid: None,
            log_path: None,
            mock: true,
        }
    }
}

impl Drop for LookingGlassHandle {
    fn drop(&mut self) {
        if self.mock {
            return;
        }
        if let Some(pid) = self.pid {
            // SIGTERM — graceful shutdown. The LG client closes its
            // ivshmem mapping and exits within ~50ms.
            //
            // We use libc::kill rather than `Child::kill` because the
            // `Child` was forgotten (std `Command::spawn` returns a
            // `Child` we don't keep, since we want detached lifecycle).
            // SAFETY: `kill(pid, SIGTERM)` on a non-existent PID returns
            // ESRCH and is safe; we ignore the return value.
            //
            // The cast `u32 -> i32` wraps for PIDs ≥ 2^31, which Linux
            // never assigns (PID_MAX is 4 million by default; 2^22).
            // `cast_possible_wrap` lint disabled for that reason.
            unsafe {
                #[cfg(unix)]
                {
                    #[allow(clippy::cast_possible_wrap)]
                    let pid_t = pid as libc::pid_t;
                    libc::kill(pid_t, libc::SIGTERM);
                }
                #[cfg(not(unix))]
                {
                    let _ = pid;
                }
            }
        }
    }
}

/// Spawn `looking-glass-client` per the spec.
///
/// Returns a [`LookingGlassHandle`] whose `Drop` impl sends `SIGTERM` to
/// the child. The wizard typically holds the handle for the lifetime of
/// the streaming session and lets `Drop` clean up.
///
/// Honors [`NOOP_ENV`]: when set, returns
/// [`LookingGlassHandle::mock`] without invoking `Command::spawn`.
///
/// # Errors
///
/// * [`crate::ErrorCategory::Other`] — `looking-glass-client` not on
///   PATH (suggests `pacman -S looking-glass`).
/// * [`crate::ErrorCategory::Other`] — `kvmfr` device file doesn't
///   exist.
/// * [`crate::ErrorCategory::Other`] — spawn failed (e.g. binary
///   exists but exec'd-as-root somehow).
pub fn launch(spec: &LookingGlassSpec) -> Result<LookingGlassHandle> {
    if std::env::var_os(NOOP_ENV).is_some() {
        return Ok(LookingGlassHandle::mock());
    }
    if !spec.device_path.exists() {
        return Err(Error::other(format!(
            "Looking Glass device path {} does not exist. \
             Run `{}` (sudo) and retry.",
            spec.device_path.display(),
            crate::bridge::kvmfr::load_module_command()
        )));
    }
    let binary = detect_client_binary().ok_or_else(|| {
        Error::other(
            "looking-glass-client not found on PATH. \
             Install via your package manager (Arch: \
             `sudo pacman -S looking-glass`; \
             Debian/Ubuntu: `sudo apt install looking-glass-client`) \
             and retry.",
        )
    })?;

    let args = render_command_args(spec);
    let mut cmd = Command::new(&binary);
    cmd.args(&args);

    // Detach: stdin from /dev/null, stdout/stderr to a log file.
    let log_path = log_file_path();
    if let Some(parent) = log_path.as_ref().and_then(|p| p.parent()) {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Some(p) = &log_path {
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
        {
            let dup = file.try_clone().ok();
            cmd.stdout(file);
            if let Some(d) = dup {
                cmd.stderr(d);
            }
        }
    }
    cmd.stdin(std::process::Stdio::null());

    let child = cmd
        .spawn()
        .map_err(|e| Error::other(format!("looking-glass-client spawn: {e}")))?;
    let pid = child.id();
    // We deliberately drop `child` here without waiting — the LG window
    // is a long-lived process tied to the session. Drop sends SIGTERM
    // via libc.
    std::mem::forget(child);
    Ok(LookingGlassHandle {
        pid: Some(pid),
        log_path,
        mock: false,
    })
}

/// Search `$PATH` for `looking-glass-client`. Returns the first match.
#[must_use]
pub fn detect_client_binary() -> Option<PathBuf> {
    detect_binary_named(CLIENT_BINARY_NAME)
}

/// Search `$PATH` for a binary by name. Used by [`detect_client_binary`]
/// and exposed `pub(crate)` so other modules don't pull in `which`.
pub(crate) fn detect_binary_named(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Best-effort: `true` if `path` points at an executable file.
fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Render the command-line argument vector for `looking-glass-client`
/// from a [`LookingGlassSpec`].
///
/// Pure function — tests assert against this without spawning anything.
#[must_use]
pub fn render_command_args(spec: &LookingGlassSpec) -> Vec<String> {
    // Looking Glass client uses `app:option=value` notation (or
    // command-line `-` flags). We use the long form for clarity and
    // because it survives across LG B6 → B7 → master more reliably.
    let mut args = Vec::new();
    args.push(format!("app:shmFile={}", spec.device_path.display()));
    if spec.fullscreen {
        args.push("win:fullScreen=yes".to_string());
    } else {
        args.push("win:fullScreen=no".to_string());
    }
    if spec.cursor_grab {
        args.push("input:grabKeyboard=yes".to_string());
        args.push("input:grabKeyboardOnFocus=yes".to_string());
    }
    if spec.audio {
        args.push("audio:micDefault=allow".to_string());
    } else {
        args.push("audio:micDefault=deny".to_string());
    }
    if spec.hdr_passthrough {
        args.push("win:hdr=yes".to_string());
    }
    args
}

/// Path to `~/.cache/neon/logs/looking-glass.log` (best-effort). When
/// the cache dir can't be resolved, returns `None` and the spawn
/// suppresses logging.
fn log_file_path() -> Option<PathBuf> {
    Some(
        dirs::cache_dir()?
            .join("neon")
            .join("logs")
            .join("looking-glass.log"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_match_apple_ux_choices() {
        let s = LookingGlassSpec::defaults();
        assert_eq!(s.device_path, PathBuf::from(DEFAULT_DEVICE_PATH));
        assert!(s.fullscreen);
        assert!(s.cursor_grab);
        assert!(s.audio);
        assert!(!s.hdr_passthrough, "HDR off-by-default per V3 plan");
    }

    #[test]
    fn render_args_includes_shm_file_path() {
        let spec = LookingGlassSpec {
            device_path: PathBuf::from("/dev/kvmfr0"),
            ..LookingGlassSpec::defaults()
        };
        let args = render_command_args(&spec);
        assert!(args.iter().any(|a| a.contains("shmFile=/dev/kvmfr0")));
    }

    #[test]
    fn render_args_emits_fullscreen_yes_no() {
        let on = render_command_args(&LookingGlassSpec {
            fullscreen: true,
            ..LookingGlassSpec::defaults()
        });
        let off = render_command_args(&LookingGlassSpec {
            fullscreen: false,
            ..LookingGlassSpec::defaults()
        });
        assert!(on.iter().any(|a| a.contains("fullScreen=yes")));
        assert!(off.iter().any(|a| a.contains("fullScreen=no")));
    }

    #[test]
    fn render_args_omits_hdr_flag_when_off() {
        let off = render_command_args(&LookingGlassSpec {
            hdr_passthrough: false,
            ..LookingGlassSpec::defaults()
        });
        assert!(!off.iter().any(|a| a.contains("hdr=")));
    }

    #[test]
    fn render_args_includes_hdr_flag_when_on() {
        let on = render_command_args(&LookingGlassSpec {
            hdr_passthrough: true,
            ..LookingGlassSpec::defaults()
        });
        assert!(on.iter().any(|a| a.contains("hdr=yes")));
    }

    #[test]
    fn render_args_audio_default_allow_when_on() {
        let on = render_command_args(&LookingGlassSpec {
            audio: true,
            ..LookingGlassSpec::defaults()
        });
        assert!(on.iter().any(|a| a.contains("micDefault=allow")));
        let off = render_command_args(&LookingGlassSpec {
            audio: false,
            ..LookingGlassSpec::defaults()
        });
        assert!(off.iter().any(|a| a.contains("micDefault=deny")));
    }

    #[test]
    fn render_args_grab_keyboard_only_when_cursor_grab_on() {
        let on = render_command_args(&LookingGlassSpec {
            cursor_grab: true,
            ..LookingGlassSpec::defaults()
        });
        assert!(on.iter().any(|a| a.contains("grabKeyboard=yes")));
        let off = render_command_args(&LookingGlassSpec {
            cursor_grab: false,
            ..LookingGlassSpec::defaults()
        });
        assert!(!off.iter().any(|a| a.contains("grabKeyboard=")));
    }

    #[test]
    fn launch_under_noop_returns_mock_handle() {
        let _g = crate::test_support::env_lock();
        // SAFETY: env behind env_lock.
        unsafe { std::env::set_var(NOOP_ENV, "1") };
        let h = launch(&LookingGlassSpec::defaults()).expect("noop launch");
        assert!(h.is_mock());
        assert_eq!(h.pid(), None);
        unsafe { std::env::remove_var(NOOP_ENV) };
    }

    #[test]
    fn launch_without_device_returns_error() {
        let _g = crate::test_support::env_lock();
        // Not under NOOP — expect launch to error on missing device.
        // SAFETY: env behind env_lock.
        unsafe { std::env::remove_var(NOOP_ENV) };
        let tmp = TempDir::new().expect("tempdir");
        let bogus = tmp.path().join("no-such-kvmfr");
        let spec = LookingGlassSpec {
            device_path: bogus,
            ..LookingGlassSpec::defaults()
        };
        let err = launch(&spec).expect_err("missing device");
        assert!(err.to_string().to_lowercase().contains("device"));
    }

    #[test]
    fn detect_client_binary_uses_path_lookup() {
        let _g = crate::test_support::env_lock();
        // Synthesize a fake bin in a tempdir and add it to PATH.
        let tmp = TempDir::new().expect("tempdir");
        let bin = tmp.path().join(CLIENT_BINARY_NAME);
        std::fs::write(&bin, "#!/bin/sh\necho fake\n").expect("write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755))
                .expect("chmod +x");
        }
        let saved = std::env::var_os("PATH");
        // SAFETY: env behind env_lock.
        unsafe { std::env::set_var("PATH", tmp.path().as_os_str()) };
        let found = detect_client_binary();
        unsafe {
            if let Some(p) = saved {
                std::env::set_var("PATH", p);
            } else {
                std::env::remove_var("PATH");
            }
        }
        assert_eq!(found.as_deref(), Some(bin.as_path()));
    }

    #[test]
    fn detect_client_binary_returns_none_when_absent() {
        let _g = crate::test_support::env_lock();
        let tmp = TempDir::new().expect("tempdir");
        // Empty tempdir on PATH.
        let saved = std::env::var_os("PATH");
        // SAFETY: env behind env_lock.
        unsafe { std::env::set_var("PATH", tmp.path().as_os_str()) };
        let found = detect_client_binary();
        unsafe {
            if let Some(p) = saved {
                std::env::set_var("PATH", p);
            } else {
                std::env::remove_var("PATH");
            }
        }
        assert_eq!(found, None);
    }

    #[test]
    fn handle_mock_drop_does_not_panic() {
        let h = LookingGlassHandle::mock();
        assert!(h.is_mock());
        assert!(h.log_path().is_none());
        drop(h);
    }

    #[test]
    fn handle_pid_and_log_path_accessors_round_trip() {
        let h = LookingGlassHandle {
            pid: Some(1234),
            log_path: Some(PathBuf::from("/var/log/x")),
            mock: false,
        };
        assert_eq!(h.pid(), Some(1234));
        assert_eq!(h.log_path(), Some(Path::new("/var/log/x")));
        // Don't let Drop run for real (we don't have PID 1234).
        std::mem::forget(h);
    }

    #[test]
    fn is_executable_handles_nonexistent_path() {
        assert!(!is_executable(Path::new("/nope/not/here")));
    }

    #[test]
    fn is_executable_returns_false_for_directory() {
        let tmp = TempDir::new().expect("tempdir");
        assert!(!is_executable(tmp.path()));
    }
}

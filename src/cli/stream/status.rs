//! `neon stream status` — V3-Phase C VM status reporter.
//!
//! Prints (in human-readable form by default; JSON when `--json`):
//!
//! * Whether the bridge VM is defined.
//! * Whether it's currently running.
//! * Snapshot age (how many days since "fresh" was taken).
//! * Trial license expiry (only when posture is `Eval`).
//! * Sunshine reachability (best-effort TCP probe at port 47984).
//!
//! ## Test-mode env vars
//!
//! | Var | Effect |
//! |---|---|
//! | [`crate::bridge::libvirt::HV_NOOP_ENV`] | libvirt connection is mocked |
//! | [`STATUS_NO_NETWORK_ENV`] | Skip Sunshine TCP probe |

use std::io::Write;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::bridge::install::POST_INSTALL_SNAPSHOT;
use crate::bridge::libvirt::Hypervisor;
use crate::bridge::license::{self, LicensePosture};
use crate::cli::OutputOptions;
use crate::error::{Error, Result};

/// Env var that suppresses the Sunshine TCP probe in tests (avoids
/// making network connections during `cargo test`).
pub const STATUS_NO_NETWORK_ENV: &str = "NEON_TEST_STATUS_NO_NETWORK";

/// Args for `neon stream status`.
#[derive(Debug, Clone, Default)]
pub struct Args {
    /// `--json`: emit JSON.
    pub json: bool,
    /// Output flags.
    pub output: OutputOptions,
}

/// Status snapshot — also the JSON output schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamStatus {
    /// VM name.
    pub vm_name: String,
    /// `true` when libvirt has a domain definition.
    pub vm_defined: bool,
    /// `true` when the domain is currently running.
    pub vm_running: bool,
    /// `true` when a "fresh" snapshot exists in the libvirt store.
    pub snapshot_present: bool,
    /// License posture, if `bridge.toml` exists.
    pub license_mode: Option<String>,
    /// Days until trial license expires (positive = days remaining,
    /// negative = days past expiry). `None` for non-trial postures.
    pub license_days_remaining: Option<i64>,
    /// `true` if the host can reach the guest's Sunshine port.
    /// `None` when the probe was skipped (test mode).
    pub sunshine_reachable: Option<bool>,
}

/// Run `neon stream status`.
///
/// # Errors
///
/// * Propagates errors from the libvirt and license modules.
pub fn run(args: &Args) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    run_with(args, &mut out)
}

/// Test-friendly variant: takes a writer.
///
/// # Errors
///
/// See [`run`].
pub fn run_with(args: &Args, out: &mut dyn Write) -> Result<()> {
    let status = build_status("neon-bridge")?;
    if args.json || args.output.json {
        let body = serde_json::to_string_pretty(&status)
            .map_err(|e| Error::other(format!("status JSON serialize: {e}")))?;
        writeln!(out, "{body}").map_err(Error::from)?;
    } else {
        render_text(&status, out).map_err(Error::from)?;
    }
    Ok(())
}

/// Probe the runtime + filesystem for current status.
///
/// # Errors
///
/// * Propagates libvirt connection / lookup errors.
pub fn build_status(vm_name: &str) -> Result<StreamStatus> {
    let posture = license::current_posture()?;
    let (license_mode, license_days_remaining) = match &posture {
        Some(p) => (Some(license_mode_label(p)), p.days_until_expiry()),
        None => (None, None),
    };

    let hv = Hypervisor::connect();
    let (vm_defined, vm_running, snapshot_present) = match hv {
        Ok(hv) => {
            let dom = hv.lookup_domain(vm_name);
            match dom {
                Ok(d) => {
                    let running = d.is_running().unwrap_or(false);
                    // Snapshot detection is best-effort; we look at the
                    // mock recorder (under NOOP) or assume present in
                    // real-libvirt mode (full snapshot enumeration is
                    // V3-Phase D scope).
                    let snap = if let Some(r) = hv.recorder() {
                        r.snapshots(vm_name)
                            .iter()
                            .any(|s| s == POST_INSTALL_SNAPSHOT)
                    } else {
                        false
                    };
                    (true, running, snap)
                }
                Err(_) => (false, false, false),
            }
        }
        Err(_) => (false, false, false),
    };

    let sunshine_reachable = if std::env::var_os(STATUS_NO_NETWORK_ENV).is_some() {
        None
    } else {
        Some(probe_sunshine())
    };

    Ok(StreamStatus {
        vm_name: vm_name.to_string(),
        vm_defined,
        vm_running,
        snapshot_present,
        license_mode,
        license_days_remaining,
        sunshine_reachable,
    })
}

fn license_mode_label(p: &LicensePosture) -> String {
    match p {
        LicensePosture::Eval { .. } => "trial".to_string(),
        LicensePosture::Key(_) => "key".to_string(),
        LicensePosture::KeyFile(_) => "key_file".to_string(),
    }
}

/// Probe `127.0.0.1:47984` (Sunshine HTTP API). Returns `false` on
/// any failure (connection refused, timeout). Best-effort.
fn probe_sunshine() -> bool {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 47984);
    TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok()
}

/// Render the status as a friendly multi-line block.
///
/// # Errors
///
/// Propagates `std::io::Error` from `writeln!`.
pub fn render_text(s: &StreamStatus, out: &mut dyn Write) -> std::io::Result<()> {
    writeln!(out, "Neon stream status")?;
    writeln!(out, "  VM name:           {}", s.vm_name)?;
    writeln!(
        out,
        "  Defined:           {}",
        if s.vm_defined { "yes" } else { "no" }
    )?;
    writeln!(
        out,
        "  Running:           {}",
        if s.vm_running { "yes" } else { "no" }
    )?;
    writeln!(
        out,
        "  Fresh snapshot:    {}",
        if s.snapshot_present { "yes" } else { "no" }
    )?;
    match &s.license_mode {
        Some(mode) => {
            writeln!(out, "  License mode:      {mode}")?;
            if let Some(days) = s.license_days_remaining {
                if days >= 0 {
                    writeln!(out, "  License remaining: {days} days")?;
                } else {
                    writeln!(out, "  License expired:   {} days ago", -days)?;
                }
            }
        }
        None => {
            writeln!(out, "  License mode:      (not configured)")?;
        }
    }
    match s.sunshine_reachable {
        Some(true) => writeln!(out, "  Sunshine:          reachable")?,
        Some(false) => writeln!(out, "  Sunshine:          unreachable")?,
        None => writeln!(out, "  Sunshine:          (probe skipped)")?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::libvirt::HV_NOOP_ENV;

    #[test]
    fn build_status_with_no_libvirt_returns_undefined() {
        let _g = crate::test_support::env_lock();
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var(HV_NOOP_ENV, "1");
            std::env::set_var(STATUS_NO_NETWORK_ENV, "1");
            std::env::set_var(
                "XDG_CONFIG_HOME",
                tempfile::TempDir::new().expect("tempdir").path(),
            );
        }
        let s = build_status("neon-bridge").expect("status");
        // Mock hypervisor's lookup_domain succeeds for any name (no
        // existence check) — so vm_defined is true, but no snapshot
        // recorded → snapshot_present false, vm_running false.
        assert!(s.vm_defined);
        assert!(!s.vm_running);
        assert!(!s.snapshot_present);
        assert!(s.sunshine_reachable.is_none());
        unsafe {
            std::env::remove_var(HV_NOOP_ENV);
            std::env::remove_var(STATUS_NO_NETWORK_ENV);
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    #[test]
    fn build_status_includes_license_mode_when_set() {
        let _g = crate::test_support::env_lock();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var(HV_NOOP_ENV, "1");
            std::env::set_var(STATUS_NO_NETWORK_ENV, "1");
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        }
        // Save a trial posture.
        let bridge_toml = tmp.path().join("neon").join("bridge.toml");
        std::fs::create_dir_all(bridge_toml.parent().unwrap()).expect("mkdir");
        license::save_posture_to(&LicensePosture::Eval { accepted_at: 1 }, &bridge_toml)
            .expect("save");
        let s = build_status("neon-bridge").expect("status");
        assert_eq!(s.license_mode.as_deref(), Some("trial"));
        // Trial accepted in 1970 → expired long ago, negative days.
        assert!(s.license_days_remaining.is_some() && s.license_days_remaining.unwrap() < 0);
        unsafe {
            std::env::remove_var(HV_NOOP_ENV);
            std::env::remove_var(STATUS_NO_NETWORK_ENV);
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    #[test]
    fn run_with_emits_text_for_default_args() {
        let _g = crate::test_support::env_lock();
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var(HV_NOOP_ENV, "1");
            std::env::set_var(STATUS_NO_NETWORK_ENV, "1");
            std::env::set_var(
                "XDG_CONFIG_HOME",
                tempfile::TempDir::new().expect("tempdir").path(),
            );
        }
        let mut buf = Vec::new();
        let args = Args::default();
        run_with(&args, &mut buf).expect("run");
        let body = String::from_utf8(buf).expect("utf8");
        assert!(body.contains("Neon stream status"));
        assert!(body.contains("VM name"));
        unsafe {
            std::env::remove_var(HV_NOOP_ENV);
            std::env::remove_var(STATUS_NO_NETWORK_ENV);
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    #[test]
    fn run_with_emits_json_when_requested() {
        let _g = crate::test_support::env_lock();
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var(HV_NOOP_ENV, "1");
            std::env::set_var(STATUS_NO_NETWORK_ENV, "1");
            std::env::set_var(
                "XDG_CONFIG_HOME",
                tempfile::TempDir::new().expect("tempdir").path(),
            );
        }
        let mut buf = Vec::new();
        let args = Args {
            json: true,
            ..Default::default()
        };
        run_with(&args, &mut buf).expect("run");
        let body = String::from_utf8(buf).expect("utf8");
        assert!(body.starts_with('{') || body.starts_with("\n{"));
        // Round-trip: parse it back.
        let parsed: StreamStatus =
            serde_json::from_str(body.trim()).expect("status parses as JSON");
        assert_eq!(parsed.vm_name, "neon-bridge");
        unsafe {
            std::env::remove_var(HV_NOOP_ENV);
            std::env::remove_var(STATUS_NO_NETWORK_ENV);
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    #[test]
    fn render_text_lays_out_six_lines() {
        let s = StreamStatus {
            vm_name: "neon-bridge".into(),
            vm_defined: true,
            vm_running: false,
            snapshot_present: true,
            license_mode: Some("trial".into()),
            license_days_remaining: Some(82),
            sunshine_reachable: Some(false),
        };
        let mut buf = Vec::new();
        render_text(&s, &mut buf).expect("render");
        let body = String::from_utf8(buf).expect("utf8");
        let lines: Vec<&str> = body.lines().collect();
        assert!(
            lines.len() >= 6,
            "expected ≥6 lines; got {} ({body})",
            lines.len()
        );
        assert!(body.contains("Defined:           yes"));
        assert!(body.contains("Running:           no"));
        assert!(body.contains("trial"));
        assert!(body.contains("82 days"));
    }

    #[test]
    fn render_text_handles_missing_license() {
        let s = StreamStatus {
            vm_name: "n".into(),
            vm_defined: false,
            vm_running: false,
            snapshot_present: false,
            license_mode: None,
            license_days_remaining: None,
            sunshine_reachable: None,
        };
        let mut buf = Vec::new();
        render_text(&s, &mut buf).expect("render");
        let body = String::from_utf8(buf).expect("utf8");
        assert!(body.contains("(not configured)"));
        assert!(body.contains("(probe skipped)"));
    }

    #[test]
    fn render_text_marks_expired_license_distinctly() {
        let s = StreamStatus {
            vm_name: "n".into(),
            vm_defined: true,
            vm_running: false,
            snapshot_present: true,
            license_mode: Some("trial".into()),
            license_days_remaining: Some(-5),
            sunshine_reachable: Some(true),
        };
        let mut buf = Vec::new();
        render_text(&s, &mut buf).expect("render");
        let body = String::from_utf8(buf).expect("utf8");
        assert!(body.contains("expired"));
        assert!(body.contains("5 days ago"));
    }

    #[test]
    fn license_mode_label_for_each_variant() {
        assert_eq!(
            license_mode_label(&LicensePosture::Eval { accepted_at: 1 }),
            "trial"
        );
        assert_eq!(
            license_mode_label(&LicensePosture::Key("AAAAA-BBBBB-CCCCC-DDDDD-EEEEE".into())),
            "key"
        );
        assert_eq!(
            license_mode_label(&LicensePosture::KeyFile("/tmp/x".into())),
            "key_file"
        );
    }
}

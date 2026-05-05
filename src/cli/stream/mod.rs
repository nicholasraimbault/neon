//! `neon stream <subcommand>` — V3 experimental localhost-bridge.
//!
//! Only compiled when the `experimental-bridge` Cargo feature is on.
//! V3-Phase C shipped `init` + `status`. V3-Phase D added `start` +
//! `stop`. V3-Phase F adds `repair`, `uninstall`, `license`, plus
//! "no subcommand" auto-dispatch (init when not provisioned, status
//! otherwise).

use std::io::Write;

use crate::cli::OutputOptions;
use crate::error::Result;

pub mod init;
pub mod license;
pub mod repair;
pub mod start;
pub mod status;
pub mod stop;
pub mod uninstall;

/// `neon stream init` Args (top-level CLI subcommand).
pub use init::Args as InitArgs;
/// `neon stream license` Args (V3-Phase F).
pub use license::Args as LicenseArgs;
/// `neon stream repair` Args (V3-Phase F).
pub use repair::Args as RepairArgs;
/// `neon stream start` Args (V3-Phase D).
pub use start::Args as StartArgs;
/// `neon stream status` Args.
pub use status::Args as StatusArgs;
/// `neon stream stop` Args (V3-Phase D).
pub use stop::Args as StopArgs;
/// `neon stream uninstall` Args (V3-Phase F).
pub use uninstall::Args as UninstallArgs;

/// Subcommand variants under `neon stream`. Mapped 1:1 from
/// the `StreamSubcommand` enum in `src/main.rs`.
#[derive(Debug)]
pub enum Subcommand {
    /// `neon stream` (no arguments) — auto-dispatch: run `init` when no
    /// `bridge.toml` exists, otherwise show `status`.
    Default(OutputOptions),
    /// `neon stream init [--accept-eval | --license-key K | --license-file P]`.
    Init(InitArgs),
    /// `neon stream status [--json]`.
    Status(StatusArgs),
    /// `neon stream start [URL]` — V3-Phase D.
    Start(StartArgs),
    /// `neon stream stop` — V3-Phase D.
    Stop(StopArgs),
    /// `neon stream repair` — V3-Phase F.
    Repair(RepairArgs),
    /// `neon stream uninstall` — V3-Phase F.
    Uninstall(UninstallArgs),
    /// `neon stream license <show|set|rearm>` — V3-Phase F.
    License(LicenseArgs),
}

/// Dispatcher from `main.rs`'s `Stream` variant.
///
/// # Errors
///
/// * Propagates errors from each subcommand.
pub fn run(sub: Subcommand) -> Result<()> {
    match sub {
        Subcommand::Default(output) => dispatch_no_args(output),
        Subcommand::Init(args) => init::run(&args),
        Subcommand::Status(args) => status::run(&args),
        Subcommand::Start(args) => start::run(&args),
        Subcommand::Stop(args) => stop::run(&args),
        Subcommand::Repair(args) => repair::run(&args),
        Subcommand::Uninstall(args) => uninstall::run(&args),
        Subcommand::License(args) => license::run(&args),
    }
}

/// Apple-UX: `neon stream` with no arguments figures out what the user
/// most likely wants. If `bridge.toml` is missing → run init. If
/// present → show status.
fn dispatch_no_args(output: OutputOptions) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    dispatch_no_args_with(output, &mut out)
}

fn dispatch_no_args_with(output: OutputOptions, out: &mut dyn Write) -> Result<()> {
    let posture = crate::bridge::license::current_posture().ok().flatten();
    if posture.is_none() {
        if !output.quiet {
            let _ = writeln!(
                out,
                "Bridge not yet provisioned. Running `neon stream init --accept-eval` \
                 (interactive)."
            );
        }
        // Defer to init wizard with --accept-eval defaulting to false.
        // The wizard surfaces a clear error if no posture flag is set.
        let args = InitArgs {
            output,
            ..Default::default()
        };
        init::run(&args)
    } else {
        let args = StatusArgs {
            output,
            ..Default::default()
        };
        status::run(&args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::libvirt::HV_NOOP_ENV;
    use crate::bridge::license::{self, LicensePosture};

    #[test]
    fn no_args_with_no_posture_runs_init_path() {
        let _g = crate::test_support::env_lock();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        }
        let mut buf = Vec::new();
        let output = OutputOptions::default();
        let err = dispatch_no_args_with(output, &mut buf).expect_err("init without flags fails");
        // The wizard fails because either capability gate fails (most
        // hosts in CI lack TPM/IOMMU) OR no license posture is supplied.
        // Both are valid states for "fresh user runs `neon stream` with
        // no args"; we just assert the dispatch happened (suggestion
        // line appeared) and the error is non-empty.
        let body = String::from_utf8(buf).expect("utf8");
        assert!(body.contains("Bridge not yet provisioned"));
        assert!(!err.to_string().is_empty(), "expected non-empty error");
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    #[test]
    fn no_args_with_existing_posture_runs_status_path() {
        let _g = crate::test_support::env_lock();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let bridge_toml = tmp.path().join("neon").join("bridge.toml");
        std::fs::create_dir_all(bridge_toml.parent().unwrap()).expect("mkdir");
        license::save_posture_to(&LicensePosture::eval_now(), &bridge_toml).expect("save");
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
            std::env::set_var(HV_NOOP_ENV, "1");
            std::env::set_var(crate::cli::stream::status::STATUS_NO_NETWORK_ENV, "1");
        }
        let mut buf = Vec::new();
        let output = OutputOptions::default();
        dispatch_no_args_with(output, &mut buf).expect("status path");
        // Status was rendered (production calls `status::run` which writes
        // to stdout — we don't capture stdout here, but we confirm no panic).
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var(HV_NOOP_ENV);
            std::env::remove_var(crate::cli::stream::status::STATUS_NO_NETWORK_ENV);
        }
    }

    #[test]
    fn run_dispatches_repair_subcommand() {
        let _g = crate::test_support::env_lock();
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var(HV_NOOP_ENV, "1");
            #[cfg(target_os = "linux")]
            std::env::set_var(crate::bridge::kvmfr::NOOP_ENV, "1");
        }
        let args = RepairArgs {
            output: OutputOptions {
                quiet: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let result = run(Subcommand::Repair(args));
        assert!(result.is_ok());
        unsafe {
            std::env::remove_var(HV_NOOP_ENV);
            #[cfg(target_os = "linux")]
            std::env::remove_var(crate::bridge::kvmfr::NOOP_ENV);
        }
    }

    #[test]
    fn run_dispatches_uninstall_subcommand() {
        let _g = crate::test_support::env_lock();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
            std::env::set_var(HV_NOOP_ENV, "1");
        }
        let args = UninstallArgs {
            output: OutputOptions {
                quiet: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let result = run(Subcommand::Uninstall(args));
        assert!(result.is_ok());
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var(HV_NOOP_ENV);
        }
    }

    #[test]
    fn run_dispatches_license_show_subcommand() {
        let _g = crate::test_support::env_lock();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        }
        let args = LicenseArgs {
            action: crate::cli::stream::license::Action::Show,
            output: OutputOptions {
                quiet: true,
                ..Default::default()
            },
        };
        run(Subcommand::License(args)).expect("show");
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }
}

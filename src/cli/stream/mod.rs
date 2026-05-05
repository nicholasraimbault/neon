//! `neon stream <subcommand>` — V3 experimental localhost-bridge.
//!
//! Only compiled when the `experimental-bridge` Cargo feature is on.
//! V3-Phase C ships:
//!
//! * [`init`] — provisions the bridge VM (the heavy phase).
//! * [`status`] — surfaces VM state, snapshot age, license expiry.
//!
//! Future V3 phases extend this surface with `start`, `stop`, `repair`,
//! `uninstall`, `license`. Until those land, those subcommands return
//! a "queued for V3-Phase D/F" stub error pointing at ROADMAP.md.

use crate::cli::OutputOptions;
use crate::error::{Error, Result};

pub mod init;
pub mod status;

/// `neon stream init` Args (top-level CLI subcommand).
pub use init::Args as InitArgs;
/// `neon stream status` Args.
pub use status::Args as StatusArgs;

/// Subcommand variants under `neon stream`. Mapped 1:1 from
/// the `StreamSubcommand` enum in `src/main.rs`.
#[derive(Debug)]
pub enum Subcommand {
    /// `neon stream init [--accept-eval | --license-key K | --license-file P]`.
    Init(InitArgs),
    /// `neon stream status [--json]`.
    Status(StatusArgs),
    /// `neon stream start <url>` — V3-Phase D (stubbed).
    Start {
        /// URL to open in the bridged browser.
        url: String,
        /// Output flags.
        output: OutputOptions,
    },
    /// `neon stream stop` — V3-Phase D (stubbed).
    Stop {
        /// Output flags.
        output: OutputOptions,
    },
    /// `neon stream repair` — V3-Phase F (stubbed).
    Repair {
        /// Output flags.
        output: OutputOptions,
    },
    /// `neon stream uninstall` — V3-Phase F (stubbed).
    Uninstall {
        /// `--purge`: also remove `~/.config/neon/bridge.toml`.
        purge: bool,
        /// Output flags.
        output: OutputOptions,
    },
    /// `neon stream license` — V3-Phase F (stubbed).
    License {
        /// Output flags.
        output: OutputOptions,
    },
}

/// Dispatcher from `main.rs`'s `Stream` variant.
///
/// # Errors
///
/// * Propagates errors from each subcommand.
/// * V3-Phase C-stubbed subcommands return `Error::other("queued for
///   V3-Phase D/F")` pointing at ROADMAP.md.
pub fn run(sub: Subcommand) -> Result<()> {
    match sub {
        Subcommand::Init(args) => init::run(&args),
        Subcommand::Status(args) => status::run(&args),
        Subcommand::Start { .. } => Err(Error::other(
            "neon stream start is queued for V3-Phase D. \
             Track ROADMAP.md and the V3 orchestration plan.",
        )),
        Subcommand::Stop { .. } => Err(Error::other(
            "neon stream stop is queued for V3-Phase D. \
             Track ROADMAP.md and the V3 orchestration plan.",
        )),
        Subcommand::Repair { .. } => Err(Error::other(
            "neon stream repair is queued for V3-Phase F. \
             Track ROADMAP.md and the V3 orchestration plan.",
        )),
        Subcommand::Uninstall { .. } => Err(Error::other(
            "neon stream uninstall is queued for V3-Phase F. \
             Track ROADMAP.md and the V3 orchestration plan.",
        )),
        Subcommand::License { .. } => Err(Error::other(
            "neon stream license is queued for V3-Phase F. \
             Track ROADMAP.md and the V3 orchestration plan.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_returns_phase_d_stub() {
        let err = run(Subcommand::Start {
            url: "https://example.com".into(),
            output: OutputOptions::default(),
        })
        .expect_err("stub");
        assert_eq!(err.category, crate::ErrorCategory::Other);
        assert!(err.to_string().contains("V3-Phase D"));
    }

    #[test]
    fn stop_returns_phase_d_stub() {
        let err = run(Subcommand::Stop {
            output: OutputOptions::default(),
        })
        .expect_err("stub");
        assert!(err.to_string().contains("V3-Phase D"));
    }

    #[test]
    fn repair_returns_phase_f_stub() {
        let err = run(Subcommand::Repair {
            output: OutputOptions::default(),
        })
        .expect_err("stub");
        assert!(err.to_string().contains("V3-Phase F"));
    }

    #[test]
    fn uninstall_returns_phase_f_stub() {
        let err = run(Subcommand::Uninstall {
            purge: false,
            output: OutputOptions::default(),
        })
        .expect_err("stub");
        assert!(err.to_string().contains("V3-Phase F"));
    }

    #[test]
    fn license_returns_phase_f_stub() {
        let err = run(Subcommand::License {
            output: OutputOptions::default(),
        })
        .expect_err("stub");
        assert!(err.to_string().contains("V3-Phase F"));
    }
}

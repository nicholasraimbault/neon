//! Neon — single-binary cross-platform DRM (Widevine) helper for Chromium-family browsers.
//!
//! Phase 0 skeleton: subcommands are placeholders that exit with "not implemented".
//! See `docs/superpowers/specs/2026-05-04-neon-rust-rewrite-design.md` for the full design.

use clap::{Parser, Subcommand};
use std::process::ExitCode;

/// Neon — patches Chromium-family browsers to play Widevine-protected content.
#[derive(Debug, Parser)]
#[command(
    name = "neon",
    version,
    about,
    long_about = None,
    propagate_version = true
)]
struct Cli {
    /// Increase log verbosity (repeat for more detail: -v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Silence non-error output.
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Disable colored output (NO_COLOR environment variable also honored).
    #[arg(long, global = true)]
    no_color: bool,

    /// Emit structured JSON output where supported.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Interactive first-run setup wizard.
    Init,

    /// Non-interactive install (for scripts and CI).
    Setup,

    /// Patch one or more browsers with the Widevine CDM.
    Patch {
        /// Patch even if the browser appears to already be patched.
        #[arg(long)]
        force: bool,

        /// Show what would be done without making changes.
        #[arg(long)]
        dry_run: bool,

        /// Optional: specific browser name to patch (e.g. "Helium").
        browser: Option<String>,
    },

    /// Show patch state for all known browsers.
    Status {
        /// Continuously refresh status output.
        #[arg(long)]
        watch: bool,
    },

    /// Enumerate known + auto-discovered browsers.
    ListBrowsers {
        /// Include auto-discovered browsers and custom-config entries.
        #[arg(long)]
        all: bool,
    },

    /// Run diagnostics; optionally translate an EME error code.
    Doctor {
        /// Output an issue-template URL prefilled with diagnostics.
        #[arg(long)]
        share: bool,

        /// EME error code to translate (e.g. Netflix N8156-6013).
        error_code: Option<String>,
    },

    /// Run an EME health check against a known test page.
    Test,

    /// Update the Widevine CDM or self-update the Neon binary.
    Update {
        #[command(subcommand)]
        target: UpdateTarget,
    },

    /// Combination uninstall + setup; preserves user config.
    Repair,

    /// Verify a browser is patched, then launch it.
    Launch {
        /// Browser name (e.g. "Helium", "Thorium").
        browser: String,
    },

    /// Remove the Neon daemon and cache (browsers stay patched until they auto-update).
    Uninstall,

    /// Generate shell completion scripts.
    Completion {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: clap_complete_shell::Shell,
    },

    /// Generate the man page in roff format.
    Manpage,
}

#[derive(Debug, Subcommand)]
enum UpdateTarget {
    /// Update the Widevine CDM (the bundled DRM module).
    Widevine {
        /// Roll back to the previous Widevine version.
        #[arg(long)]
        rollback: bool,

        /// Override the Mozilla manifest URL with a custom CRX3 source.
        #[arg(long)]
        cdm_source: Option<String>,
    },

    /// Self-update the Neon binary from GitHub Releases.
    #[command(name = "self")]
    SelfUpdate,
}

// Stand-in shell enum so clap can type-check `completion <shell>`.
// Replaced by `clap_complete::Shell` once the completion subcommand is implemented.
mod clap_complete_shell {
    use clap::ValueEnum;

    #[derive(Debug, Clone, ValueEnum)]
    #[allow(clippy::enum_variant_names)] // PowerShell is canonical naming.
    pub enum Shell {
        Bash,
        Zsh,
        Fish,
        PowerShell,
        Elvish,
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        // No subcommand → run the tray daemon (default).
        None => {
            eprintln!("neon: tray daemon not yet implemented (Phase 3)");
            ExitCode::from(2)
        }
        Some(cmd) => {
            let name = match &cmd {
                Command::Init => "init",
                Command::Setup => "setup",
                Command::Patch { .. } => "patch",
                Command::Status { .. } => "status",
                Command::ListBrowsers { .. } => "list-browsers",
                Command::Doctor { .. } => "doctor",
                Command::Test => "test",
                Command::Update { .. } => "update",
                Command::Repair => "repair",
                Command::Launch { .. } => "launch",
                Command::Uninstall => "uninstall",
                Command::Completion { .. } => "completion",
                Command::Manpage => "manpage",
            };
            eprintln!("neon: subcommand `{name}` not yet implemented");
            ExitCode::from(2)
        }
    }
}

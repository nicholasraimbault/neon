//! V3 localhost-bridge — experimental.
//!
//! Automates the QEMU/KVM + Win11 IoT LTSC + Looking Glass + GPU/TPM
//! passthrough setup that delivers premium 4K HDR streaming
//! (Netflix, Disney+, etc.) on Linux Chromium-family browsers.
//!
//! See `docs/superpowers/specs/2026-05-04-neon-v3-localhost-bridge-scaffolding-plan.md`
//! and `docs/superpowers/plans/2026-05-04-neon-v3-orchestration-plan.md`
//! for the gap analysis and architecture.
//!
//! This module is **only compiled when the `experimental-bridge`
//! feature is enabled**. Default builds of `neon` do not include any of
//! this code; users opt in via:
//!
//! ```sh
//! cargo install neon --features experimental-bridge
//! ```
//!
//! ## Status: stub-only
//!
//! V3-Phase A (this module's scaffolding phase) ships only:
//!
//! * The [`stream`] entry point that returns an
//!   [`crate::ErrorCategory::Other`] error pointing at ROADMAP.md.
//! * The [`HardwareCapabilities`] type stub (V3-Phase B will fill it
//!   with real TPM 2.0 / IOMMU / GPU / RAM / disk detection).
//!
//! The actual VM provisioning, libvirt domain XML generation, Looking
//! Glass integration, and CDM forwarding all land in V3-Phase C / D /
//! E / F. None of that code exists yet.

use crate::error::{Error, Result};

/// Top-level entry from `cli::stream::run`. Provisions the bridge VM
/// (idempotent), boots Edge in the guest pointed at `target_url`, and
/// connects the Linux host's Looking Glass client.
///
/// V3-Phase A scaffolding: this is a stub that returns an error
/// pointing the user at ROADMAP.md. The real V3 implementation lands
/// after V1.0 ships and stabilizes.
///
/// # Errors
///
/// Always returns [`crate::ErrorCategory::Other`] in V2 — the
/// localhost-bridge feature is not yet implemented.
pub fn stream(_target_url: &str) -> Result<()> {
    Err(Error::other(
        "neon stream is queued for V3; current build is a stub. \
         Track ROADMAP.md and the localhost-bridge scaffolding plan \
         (docs/superpowers/specs/2026-05-04-neon-v3-localhost-bridge-scaffolding-plan.md).",
    ))
}

/// Hardware capability snapshot consumed by the V3 bridge wizard.
///
/// **Stub.** V3-Phase B fills in TPM 2.0 presence, IOMMU enablement,
/// CPU virtualization extensions, GPU model + IOMMU-grouping, RAM,
/// available disk, HDR-capable display, etc. All detection is feature-
/// gated to keep the V2 binary lean.
#[derive(Debug, Clone)]
pub struct HardwareCapabilities;

impl HardwareCapabilities {
    /// Detect host capabilities. V3-Phase A stub returns an empty
    /// struct; V3-Phase B replaces this with real probing of `/dev/tpm0`,
    /// `dmesg | grep -i iommu`, `/proc/cpuinfo`, `/sys/class/drm`,
    /// `lspci -vk`, etc.
    #[must_use]
    pub fn detect() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `stream` returns the stub error pointing at ROADMAP.md.
    #[test]
    fn stream_returns_stub_error() {
        let err = stream("https://example.com").expect_err("must error");
        assert_eq!(err.category, crate::ErrorCategory::Other);
        assert!(err.to_string().contains("V3"));
        assert!(err.to_string().contains("ROADMAP"));
    }

    /// `HardwareCapabilities::detect` constructs an empty stub.
    #[test]
    fn hardware_capabilities_detect_constructs_stub() {
        let _caps = HardwareCapabilities::detect();
    }
}

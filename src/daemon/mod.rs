//! Long-running tray daemon (entry point + tray + watcher + IPC).
//!
//! Phase 3 platform-team-owned submodules:
//!   - lifecycle: `LaunchAgent` / systemd-user registration
//!   - power: sleep/wake hook subscription
//!
//! Daemon team will extend this module with tray/watcher/IPC code.
pub mod lifecycle;
pub mod power;

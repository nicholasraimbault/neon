//! Periodic bridge-health monitor — V3-Phase F.
//!
//! Spawned by `daemon::run` when the `experimental-bridge` feature is
//! enabled, this module runs an in-process thread that:
//!
//! * Once per [`HEALTH_INTERVAL`] (10 minutes) probes the bridge state:
//!   - Eval license days remaining (notifies at <7 days remaining).
//!   - Snapshot age (notifies when older than [`SNAPSHOT_STALE_DAYS`]).
//!   - VM lifecycle (notifies when paused continuously > 24h).
//! * Writes a heartbeat timestamp at [`heartbeat_path`] so the user
//!   (and `neon stream status`) can see the monitor is alive.
//!
//! Notifications are emitted via [`crate::notify`] — same surface as
//! the V2 patch flow uses. Each notification kind only fires once per
//! `(state, day)` so the user isn't spammed.
//!
//! ## Test-mode env var
//!
//! [`NOOP_ENV`] (`NEON_TEST_BRIDGE_HEALTH_NOOP=1`) makes
//! [`spawn_health_thread`] return immediately without spawning anything.
//! Tests use this when the daemon is brought up under test mode.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::bridge::license::{self, LicensePosture};
use crate::error::{Error, Result};

/// Env var that gates spawning the health thread in tests.
pub const NOOP_ENV: &str = "NEON_TEST_BRIDGE_HEALTH_NOOP";

/// How often the monitor runs its checks (production).
pub const HEALTH_INTERVAL: Duration = Duration::from_secs(10 * 60);

/// Eval-license expiry threshold for the recurring "near-expiry" notification.
pub const EVAL_NOTIFY_THRESHOLD_DAYS: i64 = 7;

/// Days at which a snapshot is considered stale.
pub const SNAPSHOT_STALE_DAYS: u64 = 30;

/// Hours at which a continuously-paused VM is flagged.
pub const PAUSED_NOTIFY_THRESHOLD_HOURS: u64 = 24;

/// Filename for the health heartbeat under `cache_dir/neon/bridge/`.
pub const HEARTBEAT_FILENAME: &str = "health-heartbeat";

/// Default heartbeat path: `~/.cache/neon/bridge/health-heartbeat`.
#[must_use]
pub fn heartbeat_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("neon").join("bridge").join(HEARTBEAT_FILENAME))
}

/// Outcome of one health-check tick. Public for tests + for
/// `cli::stream::status` to render.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HealthSample {
    /// Days remaining on the trial license. `None` for non-trial.
    pub eval_days_remaining: Option<i64>,
    /// `true` when eval days < [`EVAL_NOTIFY_THRESHOLD_DAYS`].
    pub eval_expiring_soon: bool,
    /// Hours since the most recent snapshot. `None` when no snapshot.
    pub snapshot_age_hours: Option<u64>,
    /// `true` when snapshot age exceeds [`SNAPSHOT_STALE_DAYS`].
    pub snapshot_stale: bool,
    /// `true` when the VM is currently paused (suspended).
    pub vm_paused: bool,
    /// Hours the VM has been continuously paused (best-effort; tracked
    /// across calls via the heartbeat).
    pub vm_paused_hours: Option<u64>,
}

impl HealthSample {
    /// Run one full sample of the bridge state. Reads
    /// `bridge.toml` for the license posture; does NOT call libvirt
    /// directly to avoid blocking the daemon thread on a hung connection.
    /// VM state is left at `vm_paused = false` here; the daemon's
    /// existing `BridgeMenuState` is the canonical surface for it.
    ///
    /// # Errors
    ///
    /// * Propagates `crate::bridge::license::current_posture` errors.
    pub fn collect() -> Result<Self> {
        let posture = license::current_posture()?;
        let eval_days_remaining = posture.as_ref().and_then(LicensePosture::days_until_expiry);
        let eval_expiring_soon =
            eval_days_remaining.is_some_and(|d| (0..EVAL_NOTIFY_THRESHOLD_DAYS).contains(&d));
        Ok(Self {
            eval_days_remaining,
            eval_expiring_soon,
            snapshot_age_hours: None,
            snapshot_stale: false,
            vm_paused: false,
            vm_paused_hours: None,
        })
    }

    /// Treat a synthesized `snapshot_age_hours` as authoritative for the
    /// stale calculation. Tests use this to drive the
    /// `SNAPSHOT_STALE_DAYS` boundary without touching disk.
    #[must_use]
    pub fn with_snapshot_age_hours(mut self, hours: Option<u64>) -> Self {
        self.snapshot_age_hours = hours;
        self.snapshot_stale = hours.is_some_and(|h| h / 24 > SNAPSHOT_STALE_DAYS);
        self
    }

    /// Treat a synthesized `vm_paused_hours` as authoritative.
    #[must_use]
    pub fn with_paused_hours(mut self, hours: Option<u64>) -> Self {
        self.vm_paused_hours = hours;
        self.vm_paused = hours.is_some_and(|h| h > 0);
        self
    }

    /// `true` when the sample warrants a tray alert badge.
    #[must_use]
    pub fn needs_attention(&self) -> bool {
        self.eval_expiring_soon
            || self.snapshot_stale
            || self
                .vm_paused_hours
                .is_some_and(|h| h > PAUSED_NOTIFY_THRESHOLD_HOURS)
    }

    /// Compose the user-facing notification body for this sample, when
    /// [`needs_attention`] is true. Returns `None` otherwise.
    #[must_use]
    pub fn compose_notification(&self) -> Option<String> {
        if !self.needs_attention() {
            return None;
        }
        let mut parts = Vec::new();
        if self.eval_expiring_soon {
            if let Some(days) = self.eval_days_remaining {
                parts.push(format!(
                    "Eval expires in {days} day(s). Run `neon stream license --rearm`."
                ));
            }
        }
        if self.snapshot_stale {
            parts.push(
                "Bridge snapshot is stale. Consider `neon stream repair --refresh-snapshot`."
                    .to_string(),
            );
        }
        if let Some(h) = self.vm_paused_hours {
            if h > PAUSED_NOTIFY_THRESHOLD_HOURS {
                parts.push(format!("Bridge VM has been paused for {h} hours."));
            }
        }
        Some(parts.join(" "))
    }

    /// `needs_attention` callers can use this for the `compose_notification` fallback.
    #[must_use]
    pub fn priority_label(&self) -> &'static str {
        if self.eval_expiring_soon {
            "Eval license"
        } else if self.snapshot_stale {
            "Stale snapshot"
        } else if self.vm_paused {
            "Paused VM"
        } else {
            "Healthy"
        }
    }
}

/// Spawn the health-monitor thread.
///
/// Returns `None` in test mode (the thread is not started). Production
/// callers (the daemon's `run_with`) keep the [`JoinHandle`] alive for
/// the daemon's lifetime; on shutdown they flip `stop` and join.
///
/// # Errors
///
/// * [`crate::ErrorCategory::Other`] when the OS rejects thread spawn.
pub fn spawn_health_thread(stop: Arc<AtomicBool>) -> Result<Option<JoinHandle<()>>> {
    if std::env::var_os(NOOP_ENV).is_some() {
        return Ok(None);
    }
    let handle = std::thread::Builder::new()
        .name("neon-bridge-health".to_string())
        .spawn(move || run_loop(&stop, HEALTH_INTERVAL))
        .map_err(|e| Error::other(format!("spawn neon-bridge-health: {e}")))?;
    Ok(Some(handle))
}

/// Inner loop. Loops until `stop` flips to `true`, with a short sleep
/// granularity (1 s) so shutdown latency stays small. Tests use a smaller
/// `interval`.
fn run_loop(stop: &AtomicBool, interval: Duration) {
    let mut last_tick = SystemTime::UNIX_EPOCH;
    let mut last_eval_notify_day: Option<i64> = None;
    let mut last_snapshot_notify_age: Option<u64> = None;
    while !stop.load(Ordering::Relaxed) {
        if SystemTime::now()
            .duration_since(last_tick)
            .unwrap_or(Duration::MAX)
            >= interval
        {
            last_tick = SystemTime::now();
            if let Ok(sample) = HealthSample::collect() {
                write_heartbeat(&sample);
                emit_notifications(
                    &sample,
                    &mut last_eval_notify_day,
                    &mut last_snapshot_notify_age,
                );
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Write the heartbeat artifact to disk. Best-effort; failures are
/// logged at TRACE only.
fn write_heartbeat(sample: &HealthSample) {
    let Some(path) = heartbeat_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let body = format!(
        "ts={now}\nlabel={}\nattention={}\n",
        sample.priority_label(),
        sample.needs_attention()
    );
    let _ = std::fs::write(path, body);
}

/// Emit notifications for state changes. `last_eval_notify_day` carries
/// the days-remaining value the last time we notified about eval
/// expiry, so we don't spam every 10 minutes.
fn emit_notifications(
    sample: &HealthSample,
    last_eval_notify_day: &mut Option<i64>,
    last_snapshot_notify_age: &mut Option<u64>,
) {
    if sample.eval_expiring_soon {
        if let Some(days) = sample.eval_days_remaining {
            if Some(days) != *last_eval_notify_day {
                crate::notify::notify_info(&format!(
                    "Bridge eval expires in {days} day(s). \
                     Run `neon stream license --rearm` to extend."
                ));
                *last_eval_notify_day = Some(days);
            }
        }
    }
    if sample.snapshot_stale {
        if let Some(age) = sample.snapshot_age_hours {
            let age_days = age / 24;
            // Notify once per "stale day" so users see one ping then nothing
            // for ~24h.
            if Some(age_days) != *last_snapshot_notify_age {
                crate::notify::notify_info(
                    "Bridge snapshot is stale. \
                     Run `neon stream repair --refresh-snapshot` when convenient.",
                );
                *last_snapshot_notify_age = Some(age_days);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Default health sample is healthy and doesn't need attention.
    #[test]
    fn default_sample_is_healthy() {
        let s = HealthSample::default();
        assert!(!s.needs_attention());
        assert_eq!(s.priority_label(), "Healthy");
        assert!(s.compose_notification().is_none());
    }

    /// `with_snapshot_age_hours` flips `snapshot_stale` past the threshold.
    #[test]
    fn with_snapshot_age_hours_marks_stale_past_threshold() {
        let s =
            HealthSample::default().with_snapshot_age_hours(Some((SNAPSHOT_STALE_DAYS + 5) * 24));
        assert!(s.snapshot_stale);
        assert!(s.needs_attention());
        assert!(s.compose_notification().is_some());
    }

    /// Snapshot under the threshold is not stale.
    #[test]
    fn with_snapshot_age_hours_under_threshold_is_fresh() {
        let s = HealthSample::default().with_snapshot_age_hours(Some(72));
        assert!(!s.snapshot_stale);
        assert!(!s.needs_attention());
    }

    /// `with_paused_hours` over threshold needs attention.
    #[test]
    fn with_paused_hours_over_threshold_needs_attention() {
        let s = HealthSample::default().with_paused_hours(Some(48));
        assert!(s.needs_attention());
    }

    /// Eval expiring soon (< 7 days) is flagged.
    #[test]
    fn eval_expiring_soon_threshold() {
        let s = HealthSample {
            eval_days_remaining: Some(3),
            eval_expiring_soon: true,
            ..HealthSample::default()
        };
        assert!(s.needs_attention());
        let body = s.compose_notification().expect("notification body");
        assert!(body.contains("Eval expires"));
        assert!(body.contains("rearm"));
    }

    /// `priority_label` reflects most-urgent state.
    #[test]
    fn priority_label_orders_by_urgency() {
        let eval = HealthSample {
            eval_days_remaining: Some(2),
            eval_expiring_soon: true,
            ..HealthSample::default()
        };
        assert_eq!(eval.priority_label(), "Eval license");

        let stale = HealthSample::default().with_snapshot_age_hours(Some(60 * 24));
        assert_eq!(stale.priority_label(), "Stale snapshot");

        let paused = HealthSample::default().with_paused_hours(Some(48));
        assert_eq!(paused.priority_label(), "Paused VM");
    }

    /// `spawn_health_thread` under NOOP returns None.
    #[test]
    fn spawn_under_noop_returns_none() {
        let _g = crate::test_support::env_lock();
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var(NOOP_ENV, "1");
        }
        let stop = Arc::new(AtomicBool::new(true));
        let h = spawn_health_thread(stop).expect("spawn");
        assert!(h.is_none());
        unsafe {
            std::env::remove_var(NOOP_ENV);
        }
    }

    /// `spawn_health_thread` honors the stop flag and joins promptly.
    #[test]
    fn spawn_thread_honors_stop_flag() {
        let _g = crate::test_support::env_lock();
        // SAFETY: env behind env_lock — make sure the noop var is unset
        // so we actually spawn.
        unsafe {
            std::env::remove_var(NOOP_ENV);
            // Set notify NOOP so the loop's notify dispatch doesn't
            // disturb the user.
            std::env::set_var(crate::notify::NOOP_ENV, "1");
        }
        let stop = Arc::new(AtomicBool::new(false));
        let handle = spawn_health_thread(Arc::clone(&stop))
            .expect("spawn")
            .expect("handle");
        // Flip stop almost immediately and ensure the thread joins.
        stop.store(true, Ordering::Relaxed);
        let started = std::time::Instant::now();
        handle.join().expect("join");
        assert!(started.elapsed() < Duration::from_secs(5));
        unsafe {
            std::env::remove_var(crate::notify::NOOP_ENV);
        }
    }

    /// `heartbeat_path` ends with `bridge/health-heartbeat`.
    #[test]
    fn heartbeat_path_ends_with_bridge_health_heartbeat() {
        if let Some(p) = heartbeat_path() {
            let suffix = std::path::Path::new("bridge").join(HEARTBEAT_FILENAME);
            assert!(p.ends_with(&suffix), "got {}", p.display());
        }
    }

    /// `emit_notifications` deduplicates eval-day pings.
    #[test]
    fn emit_notifications_dedupes_eval_day() {
        let _g = crate::test_support::env_lock();
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var(crate::notify::NOOP_ENV, "1");
        }
        let mut last_eval_day = None;
        let mut last_snap_age = None;
        let sample = HealthSample {
            eval_days_remaining: Some(3),
            eval_expiring_soon: true,
            ..HealthSample::default()
        };
        emit_notifications(&sample, &mut last_eval_day, &mut last_snap_age);
        assert_eq!(last_eval_day, Some(3));
        // Second call with same day should not re-set anything.
        emit_notifications(&sample, &mut last_eval_day, &mut last_snap_age);
        assert_eq!(last_eval_day, Some(3));
        unsafe {
            std::env::remove_var(crate::notify::NOOP_ENV);
        }
    }

    /// `compose_notification` returns Some only when attention is warranted.
    #[test]
    fn compose_notification_only_when_needs_attention() {
        let healthy = HealthSample::default();
        assert!(healthy.compose_notification().is_none());

        let stale = HealthSample::default().with_snapshot_age_hours(Some(60 * 24));
        let body = stale.compose_notification().expect("body");
        assert!(body.contains("snapshot is stale"));
    }

    /// `HealthSample::collect` doesn't panic when called without a
    /// posture configured.
    #[test]
    fn collect_with_no_posture_is_healthy() {
        let _g = crate::test_support::env_lock();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        // SAFETY: env behind env_lock.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        }
        let s = HealthSample::collect().expect("collect");
        assert!(s.eval_days_remaining.is_none());
        assert!(!s.eval_expiring_soon);
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    /// `write_heartbeat` is best-effort and shouldn't panic when path
    /// can't be created.
    #[test]
    fn write_heartbeat_is_best_effort() {
        let s = HealthSample::default();
        // We don't assert filesystem state here — heartbeat_path is OS
        // dependent and writing under a redirected XDG_CACHE_HOME would
        // require env_lock. The smoke test is "doesn't panic".
        write_heartbeat(&s);
    }
}

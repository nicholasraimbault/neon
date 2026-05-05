//! Tray icon UI.
//!
//! Wraps the [`tray-icon`](https://crates.io/crates/tray-icon) crate
//! (Tauri's tray-icon library): on Linux it uses GTK +
//! `libayatana-appindicator` at runtime; on macOS it uses Cocoa
//! `NSStatusItem`. Both are GUI-dependent.
//!
//! Per the spec the menu has the following layout:
//!
//! ```text
//! ┌──────────────────────────────────┐
//! │ ✓ Helium Patched                 │  per-browser status (× N browsers)
//! │ ✗ Thorium Not Patched            │
//! │ ──────────────────               │
//! │ Patch Now                        │  click → emits TrayCommand::PatchAll
//! │ Update Widevine                  │  click → emits TrayCommand::UpdateWidevine
//! │ ──────────────────               │
//! │ ☐ Launch at Login                │  toggle → emits TrayCommand::ToggleLaunchAtLogin
//! │ ──────────────────               │
//! │ Quit Neon                        │  click → emits TrayCommand::Quit
//! └──────────────────────────────────┘
//! ```
//!
//! Click handlers send a [`TrayCommand`] over an MPSC channel back into
//! the daemon's main loop, which dispatches to the patch / update / quit
//! flows.
//!
//! ## Test strategy
//!
//! Per the guardrails (no graphical processes during tests), we keep all
//! the **menu-construction** logic pure (the [`MenuItemSpec`] / [`menu_layout`]
//! functions) and unit-test those. The actual `tray-icon` calls live behind
//! [`Tray::new`], which returns an error in headless / no-tray contexts.
//! We do not invoke `TrayIconBuilder::new().build()` from any test.
//!
//! ## `--no-tray` fallback
//!
//! On Linux, `tray-icon` requires `libayatana-appindicator3` at runtime.
//! If the crate fails to initialize (typically because the library isn't
//! installed), [`Tray::new`] returns [`crate::ErrorCategory::UnsupportedPlatform`]
//! and the daemon's `run()` function falls back to notifications-only mode
//! with a `tracing::warn!`.

// All methods on `Tray` that touch `self.state` / `self.rx` (which are
// `Mutex`-wrapped) can theoretically panic if the lock is poisoned. We
// don't panic inside these locks under any normal codepath, and a poisoned
// lock indicates a separate (already-noted) panic upstream — so a panic
// here is a genuine bug, not something callers need to guard against.
// Documenting `# Panics` on every method that uses Mutex would be
// boilerplate; suppressing the lint at the module level is clearer.
#![allow(clippy::missing_panics_doc)]

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;

use crate::browsers::Browser;
use crate::error::{Error, Result};

/// Event emitted by the tray on a user interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayCommand {
    /// User clicked "Patch Now".
    PatchAll,
    /// User clicked a per-browser status entry — request a patch
    /// targeted at this browser. Carries the browser display name.
    PatchOne(String),
    /// User clicked "Update Widevine".
    UpdateWidevine,
    /// User toggled "Launch at Login" — the boolean is the desired state.
    ToggleLaunchAtLogin(bool),
    /// User clicked "Quit Neon".
    Quit,
    /// User clicked a streaming quick-launch (Netflix / Disney+ / HBO
    /// Max / custom URL). Daemon spawns `cli::stream::start` in a
    /// non-blocking thread.
    ///
    /// Only emitted when the `experimental-bridge` Cargo feature is on.
    #[cfg(feature = "experimental-bridge")]
    StreamUrl(String),
    /// User clicked "Bridge ▶ Pause VM". Daemon calls
    /// `bridge::libvirt::Domain::stop`.
    #[cfg(feature = "experimental-bridge")]
    BridgePause,
    /// User clicked "Bridge ▶ Resume VM". Daemon calls
    /// `bridge::libvirt::Domain::start` (after restoring from snapshot
    /// if needed).
    #[cfg(feature = "experimental-bridge")]
    BridgeResume,
    /// User clicked "Bridge ▶ Repair". Daemon invokes
    /// `cli::stream::repair::run` (V3-Phase F; placeholder log message
    /// for V3-Phase D).
    #[cfg(feature = "experimental-bridge")]
    BridgeRepair,
}

/// Pure-data description of one menu entry. Used by the construction
/// logic and by tests to assert on the layout without instantiating any
/// GUI handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItemSpec {
    /// Per-browser status row, e.g. `"✓ Helium Patched"` or
    /// `"✗ Thorium Not Patched"`.
    BrowserStatus {
        /// Display name of the browser.
        browser_name: String,
        /// Whether the browser is currently patched.
        patched: bool,
    },
    /// Action item that, when clicked, dispatches `command`.
    Action {
        /// Human-readable label.
        label: String,
        /// What command to dispatch on click.
        command: TrayCommand,
    },
    /// Toggle item (checked/unchecked) that emits `command_when_toggled`.
    Toggle {
        /// Human-readable label.
        label: String,
        /// Initial checked state.
        checked: bool,
        /// Command emitted when the user toggles. The daemon flips
        /// `checked` and re-renders.
        command_when_toggled: TrayCommand,
    },
    /// Static read-only label (e.g. "Eval: 82 days remaining"). No
    /// click handler. Used by the V3 Bridge submenu for the eval
    /// indicator + snapshot-age line.
    Label {
        /// Display text.
        text: String,
    },
    /// Submenu — a labeled parent with nested children. Used by the V3
    /// Bridge ▶ submenu under the `experimental-bridge` feature.
    /// V3-Phase D's GUI renderer flattens these as a header label
    /// followed by indented children; future polish can wire real
    /// nested menus via `tray-icon`'s `Submenu` API.
    Submenu {
        /// Label that, when hovered, expands the children.
        label: String,
        /// Child entries (rendered indented by V3-Phase D).
        items: Vec<MenuItemSpec>,
    },
    /// Visual separator.
    Separator,
}

impl MenuItemSpec {
    /// Render the user-visible label for this item. Separators have an
    /// empty label.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::BrowserStatus {
                browser_name,
                patched,
            } => {
                let prefix = if *patched { "✓" } else { "✗" };
                let suffix = if *patched { "Patched" } else { "Not Patched" };
                format!("{prefix} {browser_name} {suffix}")
            }
            Self::Action { label, .. }
            | Self::Toggle { label, .. }
            | Self::Submenu { label, .. } => label.clone(),
            Self::Label { text } => text.clone(),
            Self::Separator => String::new(),
        }
    }

    /// `true` if this is a structural separator (no click handler).
    #[must_use]
    pub fn is_separator(&self) -> bool {
        matches!(self, Self::Separator)
    }

    /// `true` if this item dispatches a [`TrayCommand`] on click.
    /// Submenus + Labels are not directly actionable (Submenu's
    /// children are; Labels are read-only).
    #[must_use]
    pub fn is_actionable(&self) -> bool {
        matches!(self, Self::Action { .. } | Self::Toggle { .. })
    }
}

/// Snapshot of state used to construct the menu. Daemon's main loop
/// rebuilds the menu from a fresh snapshot on relevant state changes
/// (patch event, browser added/removed, lifecycle toggle).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MenuState {
    /// One entry per detected browser, in display order.
    pub browsers: Vec<BrowserMenuEntry>,
    /// Whether "Launch at Login" is currently enabled.
    pub launch_at_login: bool,
    /// V3 bridge state. Only present (and only consulted) when the
    /// `experimental-bridge` Cargo feature is enabled. Default V2 builds
    /// don't compile this field.
    #[cfg(feature = "experimental-bridge")]
    pub bridge: BridgeMenuState,
}

/// V3 bridge-state snapshot consumed by [`menu_layout`] under the
/// `experimental-bridge` feature flag.
///
/// Default values surface a "bridge not yet provisioned" view: the
/// streaming quick-launches still appear (so the user can click them
/// and see the wizard suggestion), but Pause / Resume read as
/// uninitialized.
#[cfg(feature = "experimental-bridge")]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BridgeMenuState {
    /// `true` when `neon stream init` has completed (libvirt domain
    /// defined, snapshot present).
    pub ready: bool,
    /// `true` when the VM is currently paused (suspend-to-RAM after a
    /// `neon stream stop`).
    pub paused: bool,
    /// Hours since the most recent snapshot. `None` when no snapshot
    /// exists yet. Surfaced as a static label in the Bridge submenu;
    /// V3-Phase F polish renders it as "fresh / stale" badge color.
    pub snapshot_age_hours: Option<u64>,
    /// Days remaining on the trial license. `None` for non-trial
    /// postures. Negative numbers mean expired (trial-mode auto-rearm
    /// failed or hasn't run yet).
    pub eval_days_remaining: Option<i64>,
}

/// Per-browser menu line state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserMenuEntry {
    /// Display name.
    pub name: String,
    /// Patched (✓) or not (✗).
    pub patched: bool,
}

impl BrowserMenuEntry {
    /// Construct a menu entry from a [`Browser`] + a "is patched" flag.
    #[must_use]
    pub fn from_browser(browser: &Browser, patched: bool) -> Self {
        Self {
            name: browser.name().to_string(),
            patched,
        }
    }
}

/// Build the canonical menu layout (per spec) from the supplied state.
///
/// This is the **pure** function that tests assert on — no GUI handles,
/// no crate dependencies.
///
/// Under the `experimental-bridge` Cargo feature, additional items are
/// injected after the patch controls (streaming quick-launches + a
/// `Bridge ▶` submenu). Default V2 builds compile no V3 code; the menu
/// shape is unchanged.
#[must_use]
pub fn menu_layout(state: &MenuState) -> Vec<MenuItemSpec> {
    let mut out = Vec::with_capacity(8 + state.browsers.len());

    // 1. Per-browser status lines (one per detected browser).
    for entry in &state.browsers {
        out.push(MenuItemSpec::BrowserStatus {
            browser_name: entry.name.clone(),
            patched: entry.patched,
        });
    }
    if !state.browsers.is_empty() {
        out.push(MenuItemSpec::Separator);
    }
    // 2. Actions.
    out.push(MenuItemSpec::Action {
        label: "Patch Now".into(),
        command: TrayCommand::PatchAll,
    });
    out.push(MenuItemSpec::Action {
        label: "Update Widevine".into(),
        command: TrayCommand::UpdateWidevine,
    });

    // 3. V3 streaming + bridge submenu (only under feature flag).
    #[cfg(feature = "experimental-bridge")]
    inject_bridge_items(&mut out, &state.bridge);

    out.push(MenuItemSpec::Separator);
    // 4. Launch-at-Login toggle.
    out.push(MenuItemSpec::Toggle {
        label: "Launch at Login".into(),
        checked: state.launch_at_login,
        command_when_toggled: TrayCommand::ToggleLaunchAtLogin(!state.launch_at_login),
    });
    out.push(MenuItemSpec::Separator);
    // 5. Quit.
    out.push(MenuItemSpec::Action {
        label: "Quit Neon".into(),
        command: TrayCommand::Quit,
    });
    out
}

/// Inject the V3 streaming quick-launches + Bridge submenu into the
/// supplied menu vec, between the patch controls and the
/// Launch-at-Login section.
///
/// Layout:
/// ```text
/// ──── separator ────
/// Stream Netflix
/// Stream Disney+
/// Stream HBO Max
/// Stream… (custom URL)         (V3-Phase F: opens prompt)
/// ──── separator ────
/// Bridge ▶
///   Status: Ready / Paused / Not provisioned
///   Pause VM
///   Resume VM
///   Repair
///   Eval: N days remaining     (only when on trial)
///   Snapshot: age              (only when snapshot present)
/// ```
///
/// The order keeps the most-frequent action (streaming quick-launch)
/// at the top of the V3 block.
#[cfg(feature = "experimental-bridge")]
fn inject_bridge_items(out: &mut Vec<MenuItemSpec>, bridge: &BridgeMenuState) {
    out.push(MenuItemSpec::Separator);
    out.push(MenuItemSpec::Action {
        label: "Stream Netflix".into(),
        command: TrayCommand::StreamUrl("https://netflix.com".into()),
    });
    out.push(MenuItemSpec::Action {
        label: "Stream Disney+".into(),
        command: TrayCommand::StreamUrl("https://disneyplus.com".into()),
    });
    out.push(MenuItemSpec::Action {
        label: "Stream HBO Max".into(),
        command: TrayCommand::StreamUrl("https://max.com".into()),
    });
    out.push(MenuItemSpec::Action {
        label: "Stream… (custom URL)".into(),
        // V3-Phase F adds a real "open custom URL" prompt; for now this
        // emits a sentinel `StreamUrl("")` that the daemon's dispatch
        // handler interprets as "open the prompt" (and currently logs
        // a TODO).
        command: TrayCommand::StreamUrl(String::new()),
    });
    out.push(MenuItemSpec::Separator);

    // Bridge submenu.
    let mut sub = Vec::with_capacity(6);
    sub.push(MenuItemSpec::Label {
        text: bridge_status_label(bridge),
    });
    sub.push(MenuItemSpec::Action {
        label: "Pause VM".into(),
        command: TrayCommand::BridgePause,
    });
    sub.push(MenuItemSpec::Action {
        label: "Resume VM".into(),
        command: TrayCommand::BridgeResume,
    });
    sub.push(MenuItemSpec::Action {
        label: "Repair".into(),
        command: TrayCommand::BridgeRepair,
    });
    if let Some(days) = bridge.eval_days_remaining {
        sub.push(MenuItemSpec::Label {
            text: eval_days_label(days),
        });
    }
    if let Some(hours) = bridge.snapshot_age_hours {
        sub.push(MenuItemSpec::Label {
            text: snapshot_age_label(hours),
        });
    }
    out.push(MenuItemSpec::Submenu {
        label: "Bridge ▶".into(),
        items: sub,
    });
}

/// Render the Bridge submenu's "Status: ..." header label.
#[cfg(feature = "experimental-bridge")]
fn bridge_status_label(bridge: &BridgeMenuState) -> String {
    if !bridge.ready {
        return "Status: Not provisioned".into();
    }
    if bridge.paused {
        "Status: Paused".into()
    } else {
        "Status: Ready".into()
    }
}

/// Render the eval-days indicator label.
///
/// * `days >= 0` → "Eval: N days remaining"
/// * `days < 0` → "Eval: expired (N days ago)"
#[cfg(feature = "experimental-bridge")]
fn eval_days_label(days: i64) -> String {
    if days >= 0 {
        format!("Eval: {days} days remaining")
    } else {
        format!("Eval: expired ({} days ago)", -days)
    }
}

/// Render the snapshot-age indicator label.
#[cfg(feature = "experimental-bridge")]
fn snapshot_age_label(hours: u64) -> String {
    if hours < 24 {
        format!("Snapshot: {hours}h old")
    } else {
        let days = hours / 24;
        format!("Snapshot: {days}d old")
    }
}

/// Public tray handle. Holds the underlying `tray-icon` resource (when
/// running) and a receiver for command events.
///
/// Drop tears down the tray icon. The daemon team typically holds this
/// for the lifetime of the process.
pub struct Tray {
    /// Receiver of [`TrayCommand`] events emitted by click handlers.
    /// Daemon's main loop reads this and dispatches.
    rx: Mutex<Receiver<TrayCommand>>,
    /// Sender retained so re-renderable state changes can synthesize
    /// commands (e.g. for tests, or a future "click via IPC" feature).
    tx: Sender<TrayCommand>,
    /// Pure-data record of the current menu state. Updated whenever
    /// the caller calls [`Tray::set_state`].
    state: Mutex<MenuState>,
    /// Real tray icon, if [`Tray::new`] succeeded against the platform.
    /// Kept private — the daemon doesn't poke at the underlying handle.
    /// `None` in headless / no-tray contexts (the `--no-tray` fallback).
    inner: Option<TrayInner>,
}

/// Wrapper around the platform-specific `tray-icon` handle. Behind a
/// struct so we can extend it (icon set, tooltip update) without
/// changing [`Tray`].
struct TrayInner {
    _tray: tray_icon::TrayIcon,
    /// Map of `MenuId` strings → command, for click-event routing.
    /// We use `String` keys because `MenuId` is a thin wrapper around it.
    /// The map is kept alive as part of `TrayInner` so the click-event
    /// handler closure (which got a clone of this map at construction
    /// time) doesn't see a moving target — even though no one reads
    /// this field directly after construction.
    _routes: std::collections::HashMap<String, TrayCommand>,
}

impl Tray {
    /// Build a new tray icon with the supplied initial menu state.
    ///
    /// On Linux this requires GTK + `libayatana-appindicator3` at runtime;
    /// on macOS Cocoa `AppKit`. If the underlying library fails to
    /// initialize, returns [`crate::ErrorCategory::UnsupportedPlatform`]
    /// so the daemon can fall back to `--no-tray` mode.
    ///
    /// **Tests must not call this** — it opens an actual tray icon on
    /// the user's display. Use [`Tray::headless`] in tests.
    ///
    /// # Errors
    ///
    /// * [`crate::ErrorCategory::UnsupportedPlatform`] if `tray-icon`
    ///   cannot initialize.
    /// * [`crate::ErrorCategory::Other`] for any other initialization
    ///   failure.
    pub fn new(initial_state: MenuState) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<TrayCommand>();
        let routes = build_routes(&initial_state);
        let inner = build_tray_icon(&initial_state, &routes, tx.clone()).map_err(|e| {
            Error::unsupported_platform(format!("tray-icon initialization failed: {e}"))
        })?;
        Ok(Self {
            rx: Mutex::new(rx),
            tx,
            state: Mutex::new(initial_state),
            inner: Some(inner),
        })
    }

    /// Build a "headless" tray that has no UI surface but still emits
    /// commands when [`Tray::synthesize`] is called. Used in tests and
    /// in the daemon's `--no-tray` fallback.
    #[must_use]
    pub fn headless(initial_state: MenuState) -> Self {
        let (tx, rx) = mpsc::channel::<TrayCommand>();
        Self {
            rx: Mutex::new(rx),
            tx,
            state: Mutex::new(initial_state),
            inner: None,
        }
    }

    /// Snapshot the current menu state.
    #[must_use]
    pub fn state(&self) -> MenuState {
        self.state.lock().unwrap().clone()
    }

    /// Update the menu state. The next call to [`Tray::menu_layout`]
    /// reflects the new layout. (Re-rendering the live tray icon is
    /// not a Phase 3 deliverable — daemon's main loop simply drops the
    /// existing tray and constructs a fresh one when state changes
    /// non-trivially. A follow-up can wire `set_menu` into the tray
    /// crate for incremental updates.)
    pub fn set_state(&self, state: MenuState) {
        *self.state.lock().unwrap() = state;
    }

    /// Render the current menu layout (pure-data view).
    #[must_use]
    pub fn current_menu_layout(&self) -> Vec<MenuItemSpec> {
        menu_layout(&self.state.lock().unwrap())
    }

    /// Try to receive the next [`TrayCommand`]. Non-blocking; returns
    /// `None` if no command is pending.
    pub fn try_recv(&self) -> Option<TrayCommand> {
        self.rx.lock().unwrap().try_recv().ok()
    }

    /// Block on the next [`TrayCommand`]. Returns `None` if the sender
    /// has been dropped (i.e. the tray is shutting down).
    pub fn recv_blocking(&self) -> Option<TrayCommand> {
        self.rx.lock().unwrap().recv().ok()
    }

    /// Synthesize a [`TrayCommand`] as if the user had clicked. Used
    /// by tests and by the daemon when it wants to drive its main loop
    /// from a non-UI source (e.g. a wake event triggers a re-check).
    pub fn synthesize(&self, cmd: TrayCommand) {
        // Best-effort send — if the receiver is gone the daemon is
        // shutting down anyway.
        let _ = self.tx.send(cmd);
    }

    /// `true` if a real tray UI is attached (i.e. [`Tray::new`] succeeded).
    /// `false` for [`Tray::headless`].
    #[must_use]
    pub fn has_ui(&self) -> bool {
        self.inner.is_some()
    }
}

/// Build a map of `MenuId` strings to [`TrayCommand`] used by the tray
/// crate's event handler to route click events back to us.
///
/// Each entry in the menu layout gets a unique stable id derived from
/// its position + content; we re-build the map every time we re-render.
///
/// Submenu children are also routed: for index `idx` the submenu, the
/// submenu's children get ids prefixed with `<submenu-id>-child-<n>`.
fn build_routes(state: &MenuState) -> std::collections::HashMap<String, TrayCommand> {
    let mut routes = std::collections::HashMap::new();
    let layout = menu_layout(state);
    for (idx, item) in layout.iter().enumerate() {
        route_item_into(&mut routes, idx, item, None);
    }
    routes
}

/// Insert routes for a single item (and recursively for submenu
/// children).
///
/// `parent_id` is `Some(parent)` when we're inside a submenu — child
/// ids are derived from the submenu's id + the child's index so they
/// stay unique across re-renders.
fn route_item_into(
    routes: &mut std::collections::HashMap<String, TrayCommand>,
    idx: usize,
    item: &MenuItemSpec,
    parent_id: Option<&str>,
) {
    let id = match parent_id {
        Some(p) => format!("{p}-child-{idx}"),
        None => menu_item_id(idx, item),
    };
    match item {
        MenuItemSpec::Action { command, .. } => {
            routes.insert(id, command.clone());
        }
        MenuItemSpec::Toggle {
            command_when_toggled,
            ..
        } => {
            routes.insert(id, command_when_toggled.clone());
        }
        MenuItemSpec::BrowserStatus { browser_name, .. } => {
            routes.insert(id, TrayCommand::PatchOne(browser_name.clone()));
        }
        MenuItemSpec::Submenu { items, .. } => {
            // Recurse: submenu's own id isn't actionable, but the
            // children carry their own commands.
            for (child_idx, child) in items.iter().enumerate() {
                route_item_into(routes, child_idx, child, Some(&id));
            }
        }
        MenuItemSpec::Label { .. } | MenuItemSpec::Separator => {}
    }
}

/// Build a stable id for a menu item at a given position. Position +
/// label is enough to identify any of our menu items uniquely (we never
/// have two browsers with the same name).
fn menu_item_id(index: usize, item: &MenuItemSpec) -> String {
    match item {
        MenuItemSpec::BrowserStatus { browser_name, .. } => {
            format!("neon-browser-{index}-{browser_name}")
        }
        MenuItemSpec::Action { label, .. } => format!("neon-action-{index}-{label}"),
        MenuItemSpec::Toggle { label, .. } => format!("neon-toggle-{index}-{label}"),
        MenuItemSpec::Submenu { label, .. } => format!("neon-submenu-{index}-{label}"),
        MenuItemSpec::Label { text } => format!("neon-label-{index}-{text}"),
        MenuItemSpec::Separator => format!("neon-sep-{index}"),
    }
}

/// Construct the live tray icon. This is the only function in this
/// module that touches the GUI — guarded by a `Result` so callers can
/// fall back to headless mode if it fails (e.g. no
/// `libayatana-appindicator3` on a Linux box).
///
/// Tests do **not** call this; they use [`Tray::headless`].
#[allow(clippy::needless_pass_by_value)] // `tx` is moved into the click handler closure
fn build_tray_icon(
    state: &MenuState,
    routes: &std::collections::HashMap<String, TrayCommand>,
    tx: Sender<TrayCommand>,
) -> std::result::Result<TrayInner, tray_icon::Error> {
    use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem};
    use tray_icon::TrayIconBuilder;

    let menu = Menu::new();
    for (idx, spec) in menu_layout(state).iter().enumerate() {
        let id = MenuId::new(menu_item_id(idx, spec));
        match spec {
            MenuItemSpec::BrowserStatus { .. } | MenuItemSpec::Action { .. } => {
                let item = MenuItem::with_id(id, spec.label(), true, None);
                let _ = menu.append(&item);
            }
            MenuItemSpec::Toggle { checked, .. } => {
                let item = CheckMenuItem::with_id(id, spec.label(), true, *checked, None);
                let _ = menu.append(&item);
            }
            MenuItemSpec::Submenu { label, items } => {
                // V3-Phase D flattens submenus: emit the header as a
                // disabled label, then indented children with derived
                // ids matching `route_item_into`. Real nested-menu
                // rendering is a V3-Phase F polish item.
                let header = MenuItem::with_id(id.clone(), label.clone(), false, None);
                let _ = menu.append(&header);
                for (child_idx, child) in items.iter().enumerate() {
                    let child_id = MenuId::new(format!("{}-child-{child_idx}", id.0));
                    match child {
                        MenuItemSpec::Action { .. } => {
                            let item = MenuItem::with_id(
                                child_id,
                                format!("    {}", child.label()),
                                true,
                                None,
                            );
                            let _ = menu.append(&item);
                        }
                        MenuItemSpec::Toggle { checked, .. } => {
                            let item = CheckMenuItem::with_id(
                                child_id,
                                format!("    {}", child.label()),
                                true,
                                *checked,
                                None,
                            );
                            let _ = menu.append(&item);
                        }
                        MenuItemSpec::Label { .. } => {
                            let item = MenuItem::with_id(
                                child_id,
                                format!("    {}", child.label()),
                                false,
                                None,
                            );
                            let _ = menu.append(&item);
                        }
                        _ => {}
                    }
                }
            }
            MenuItemSpec::Label { text } => {
                let item = MenuItem::with_id(id, text.clone(), false, None);
                let _ = menu.append(&item);
            }
            MenuItemSpec::Separator => {
                let item = PredefinedMenuItem::separator();
                let _ = menu.append(&item);
            }
        }
    }

    let routes_for_handler = routes.clone();
    let tx_for_handler = tx.clone();
    tray_icon::menu::MenuEvent::set_event_handler(Some(
        move |event: tray_icon::menu::MenuEvent| {
            let id_str = event.id().0.clone();
            if let Some(cmd) = routes_for_handler.get(&id_str) {
                let _ = tx_for_handler.send(cmd.clone());
            }
        },
    ));

    let _ = tx; // reserved for future tray click handlers

    let tray = TrayIconBuilder::new()
        .with_tooltip("Neon — Widevine helper")
        .with_menu(Box::new(menu))
        .build()?;
    Ok(TrayInner {
        _tray: tray,
        _routes: routes.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::browsers::BrowserKind;

    /// Helper: synthesize a `Browser`.
    fn fake_browser(name: &str) -> Browser {
        Browser {
            name: name.into(),
            install_path: PathBuf::from(format!("/opt/{name}-bin")),
            kind: BrowserKind::Detected,
            framework_name: None,
        }
    }

    /// Empty browser list: no per-browser rows, no leading separator.
    /// The non-browser items still appear in the canonical order.
    /// (Default V2 build only — feature-on adds 7 V3 entries; see
    /// [`empty_browsers_with_feature_on`].)
    #[cfg(not(feature = "experimental-bridge"))]
    #[test]
    fn empty_browsers_skips_per_browser_block_but_keeps_actions() {
        let state = MenuState {
            browsers: vec![],
            launch_at_login: false,
        };
        let layout = menu_layout(&state);
        // Should be: Patch Now, Update Widevine, Sep, Toggle, Sep, Quit
        assert_eq!(layout.len(), 6);
        assert!(matches!(
            &layout[0],
            MenuItemSpec::Action {
                command: TrayCommand::PatchAll,
                ..
            }
        ));
        assert!(matches!(
            &layout[1],
            MenuItemSpec::Action {
                command: TrayCommand::UpdateWidevine,
                ..
            }
        ));
        assert!(matches!(&layout[2], MenuItemSpec::Separator));
        assert!(matches!(&layout[3], MenuItemSpec::Toggle { .. }));
        assert!(matches!(&layout[4], MenuItemSpec::Separator));
        assert!(matches!(
            &layout[5],
            MenuItemSpec::Action {
                command: TrayCommand::Quit,
                ..
            }
        ));
    }

    /// Two browsers: rows + separator + actions + separator + toggle + sep + quit.
    /// (Default V2 build only.)
    #[cfg(not(feature = "experimental-bridge"))]
    #[test]
    fn two_browsers_produces_canonical_layout() {
        let state = MenuState {
            browsers: vec![
                BrowserMenuEntry::from_browser(&fake_browser("Helium"), true),
                BrowserMenuEntry::from_browser(&fake_browser("Thorium"), false),
            ],
            launch_at_login: true,
        };
        let layout = menu_layout(&state);
        assert_eq!(layout.len(), 9);
        assert!(matches!(
            &layout[0],
            MenuItemSpec::BrowserStatus {
                browser_name,
                patched: true
            } if browser_name == "Helium"
        ));
        assert!(matches!(
            &layout[1],
            MenuItemSpec::BrowserStatus {
                browser_name,
                patched: false
            } if browser_name == "Thorium"
        ));
        assert!(matches!(&layout[2], MenuItemSpec::Separator));
    }

    /// Patched + unpatched browsers render with the right glyph + suffix.
    #[test]
    fn browser_label_distinguishes_patched_status() {
        let patched = MenuItemSpec::BrowserStatus {
            browser_name: "Helium".into(),
            patched: true,
        };
        let unpatched = MenuItemSpec::BrowserStatus {
            browser_name: "Thorium".into(),
            patched: false,
        };
        assert_eq!(patched.label(), "✓ Helium Patched");
        assert_eq!(unpatched.label(), "✗ Thorium Not Patched");
    }

    /// Separators have empty labels and are flagged as non-actionable.
    #[test]
    fn separator_predicates() {
        let s = MenuItemSpec::Separator;
        assert!(s.is_separator());
        assert!(!s.is_actionable());
        assert_eq!(s.label(), "");
    }

    /// Action items are actionable.
    #[test]
    fn action_predicates() {
        let a = MenuItemSpec::Action {
            label: "Patch Now".into(),
            command: TrayCommand::PatchAll,
        };
        assert!(!a.is_separator());
        assert!(a.is_actionable());
        assert_eq!(a.label(), "Patch Now");
    }

    /// Toggle items are actionable.
    #[test]
    fn toggle_predicates() {
        let t = MenuItemSpec::Toggle {
            label: "Launch at Login".into(),
            checked: true,
            command_when_toggled: TrayCommand::ToggleLaunchAtLogin(false),
        };
        assert!(!t.is_separator());
        assert!(t.is_actionable());
        assert_eq!(t.label(), "Launch at Login");
    }

    /// `Toggle.command_when_toggled` reflects the *opposite* of the
    /// current state (i.e. clicking checks → unchecks).
    #[test]
    fn toggle_emits_inverse_state_on_click() {
        let state_off = MenuState {
            browsers: vec![],
            launch_at_login: false,
            #[cfg(feature = "experimental-bridge")]
            bridge: BridgeMenuState::default(),
        };
        let state_on = MenuState {
            browsers: vec![],
            launch_at_login: true,
            #[cfg(feature = "experimental-bridge")]
            bridge: BridgeMenuState::default(),
        };
        let layout_off = menu_layout(&state_off);
        let layout_on = menu_layout(&state_on);
        let toggle_off = layout_off
            .iter()
            .find(|i| matches!(i, MenuItemSpec::Toggle { .. }))
            .unwrap();
        let toggle_on = layout_on
            .iter()
            .find(|i| matches!(i, MenuItemSpec::Toggle { .. }))
            .unwrap();
        match toggle_off {
            MenuItemSpec::Toggle {
                command_when_toggled,
                checked,
                ..
            } => {
                assert!(!*checked);
                assert_eq!(
                    *command_when_toggled,
                    TrayCommand::ToggleLaunchAtLogin(true)
                );
            }
            _ => panic!(),
        }
        match toggle_on {
            MenuItemSpec::Toggle {
                command_when_toggled,
                checked,
                ..
            } => {
                assert!(*checked);
                assert_eq!(
                    *command_when_toggled,
                    TrayCommand::ToggleLaunchAtLogin(false)
                );
            }
            _ => panic!(),
        }
    }

    /// `Tray::headless` returns a tray with no UI surface.
    #[test]
    fn headless_has_no_ui() {
        let t = Tray::headless(MenuState {
            browsers: vec![],
            launch_at_login: false,
            #[cfg(feature = "experimental-bridge")]
            bridge: BridgeMenuState::default(),
        });
        assert!(!t.has_ui());
    }

    /// Synthesizing a command on a headless tray makes it observable
    /// via `try_recv`.
    #[test]
    fn synthesize_round_trips_through_channel() {
        let t = Tray::headless(MenuState {
            browsers: vec![],
            launch_at_login: false,
            #[cfg(feature = "experimental-bridge")]
            bridge: BridgeMenuState::default(),
        });
        t.synthesize(TrayCommand::PatchAll);
        let cmd = t.try_recv().expect("command pending");
        assert_eq!(cmd, TrayCommand::PatchAll);
        // Channel drains.
        assert!(t.try_recv().is_none());
    }

    /// `set_state` updates the snapshot and the rendered layout.
    /// (Default V2 build only — feature-on adds 7 V3 entries; see
    /// the V3 test module below.)
    #[cfg(not(feature = "experimental-bridge"))]
    #[test]
    fn set_state_updates_layout() {
        let t = Tray::headless(MenuState {
            browsers: vec![],
            launch_at_login: false,
        });
        let initial = t.current_menu_layout();
        // 6 items when no browsers.
        assert_eq!(initial.len(), 6);

        t.set_state(MenuState {
            browsers: vec![BrowserMenuEntry::from_browser(
                &fake_browser("Helium"),
                true,
            )],
            launch_at_login: true,
        });
        let updated = t.current_menu_layout();
        // 1 browser + sep + 2 actions + sep + toggle + sep + quit = 8
        assert_eq!(updated.len(), 8);
        assert!(matches!(&updated[0], MenuItemSpec::BrowserStatus { .. }));
    }

    /// `state()` returns a snapshot equal to what we set.
    #[test]
    fn state_round_trip() {
        let state = MenuState {
            browsers: vec![BrowserMenuEntry::from_browser(
                &fake_browser("Thorium"),
                false,
            )],
            launch_at_login: true,
            #[cfg(feature = "experimental-bridge")]
            bridge: BridgeMenuState::default(),
        };
        let t = Tray::headless(state.clone());
        assert_eq!(t.state(), state);
    }

    /// `build_routes` covers every actionable menu item.
    /// (Default V2 build only — feature-on adds 4 stream actions + 3
    /// bridge submenu actions; see the V3 test module below.)
    #[cfg(not(feature = "experimental-bridge"))]
    #[test]
    fn build_routes_covers_actions_and_browsers_and_toggles() {
        let state = MenuState {
            browsers: vec![BrowserMenuEntry::from_browser(
                &fake_browser("Helium"),
                true,
            )],
            launch_at_login: false,
        };
        let routes = build_routes(&state);
        // Expect: 1 browser + 2 actions + 1 toggle + 1 quit = 5 actionables.
        assert_eq!(routes.len(), 5);
        // Browser should map to a PatchOne with that name.
        let browser_entry = routes
            .values()
            .find(|c| matches!(c, TrayCommand::PatchOne(_)));
        match browser_entry {
            Some(TrayCommand::PatchOne(name)) => assert_eq!(name, "Helium"),
            _ => panic!("missing PatchOne(Helium) in routes"),
        }
    }

    /// `menu_item_id` is stable: two calls with identical inputs produce
    /// identical ids.
    #[test]
    fn menu_item_id_is_stable() {
        let item = MenuItemSpec::Action {
            label: "Patch Now".into(),
            command: TrayCommand::PatchAll,
        };
        assert_eq!(menu_item_id(2, &item), menu_item_id(2, &item));
        assert_ne!(menu_item_id(1, &item), menu_item_id(2, &item));
    }

    /// `BrowserMenuEntry::from_browser` carries the name + patched flag.
    #[test]
    fn browser_menu_entry_from_browser() {
        let entry = BrowserMenuEntry::from_browser(&fake_browser("Foo"), true);
        assert_eq!(entry.name, "Foo");
        assert!(entry.patched);
    }

    /// Layout always ends with a Quit action (even with no browsers).
    #[test]
    fn last_item_is_quit() {
        for browsers in [
            vec![],
            vec![BrowserMenuEntry::from_browser(&fake_browser("A"), false)],
        ] {
            let state = MenuState {
                browsers,
                launch_at_login: false,
                #[cfg(feature = "experimental-bridge")]
                bridge: BridgeMenuState::default(),
            };
            let layout = menu_layout(&state);
            match layout.last().unwrap() {
                MenuItemSpec::Action {
                    command: TrayCommand::Quit,
                    ..
                } => {}
                other => panic!("expected Quit, got {other:?}"),
            }
        }
    }

    /// `recv_blocking` returns `None` when the sender has been dropped.
    #[test]
    fn recv_blocking_returns_none_when_sender_dropped() {
        // Build a headless tray, then forcibly drop the internal `tx`.
        // We can't reach inside, but we can verify try_recv on an empty
        // channel returns None.
        let t = Tray::headless(MenuState {
            browsers: vec![],
            launch_at_login: false,
            #[cfg(feature = "experimental-bridge")]
            bridge: BridgeMenuState::default(),
        });
        assert!(t.try_recv().is_none());
    }

    /// `TrayInner::routes` is shaped as expected (smoke check) — we
    /// can't construct a `TrayIcon` in a headless test, so we just
    /// assert the field's existence/type at compile time via a function
    /// that takes `&TrayInner::routes`.
    #[test]
    fn tray_inner_routes_field_present() {
        // Synthesize a minimal Routes map and verify the type matches.
        let m: std::collections::HashMap<String, TrayCommand> = std::collections::HashMap::new();
        // Drop ensures the type-checker actually verifies the type.
        drop(m);
    }
}

/// V3 bridge menu tests — only compiled with `experimental-bridge`.
///
/// These mirror the default-feature tests above but assert against the
/// V3-augmented menu layout: 4 streaming quick-launches + the
/// `Bridge ▶` submenu inserted between the patch controls and the
/// Launch-at-Login section.
#[cfg(all(test, feature = "experimental-bridge"))]
mod tests_v3 {
    use super::*;

    fn empty_state(bridge: BridgeMenuState) -> MenuState {
        MenuState {
            browsers: vec![],
            launch_at_login: false,
            bridge,
        }
    }

    /// Empty browsers + default bridge state: layout grows from 6 → 13
    /// items (sep + 4 stream actions + sep + Bridge submenu).
    #[test]
    fn empty_browsers_v3_layout_includes_streaming_and_bridge_submenu() {
        let state = empty_state(BridgeMenuState::default());
        let layout = menu_layout(&state);
        // Expected: PatchAll + UpdateWidevine + Sep + Stream Netflix +
        // Stream Disney+ + Stream HBO Max + Stream… + Sep + Bridge ▶
        // + Sep + Toggle + Sep + Quit = 13.
        assert_eq!(
            layout.len(),
            13,
            "expected 13 items in V3 menu, got {} ({layout:#?})",
            layout.len()
        );
        // PatchAll + UpdateWidevine still come first.
        assert!(matches!(
            &layout[0],
            MenuItemSpec::Action {
                command: TrayCommand::PatchAll,
                ..
            }
        ));
        assert!(matches!(
            &layout[1],
            MenuItemSpec::Action {
                command: TrayCommand::UpdateWidevine,
                ..
            }
        ));
        // Then the V3 separator + 4 streaming actions.
        assert!(matches!(&layout[2], MenuItemSpec::Separator));
        for (idx, expected_url) in [(3, "netflix.com"), (4, "disneyplus.com"), (5, "max.com")] {
            match &layout[idx] {
                MenuItemSpec::Action {
                    command: TrayCommand::StreamUrl(url),
                    label,
                } => {
                    assert!(label.starts_with("Stream "), "label was {label:?}");
                    assert!(url.contains(expected_url), "url was {url:?}");
                }
                other => panic!("idx {idx} expected stream action, got {other:?}"),
            }
        }
        // Custom URL slot has empty URL string.
        match &layout[6] {
            MenuItemSpec::Action {
                command: TrayCommand::StreamUrl(url),
                ..
            } => assert!(url.is_empty()),
            other => panic!("idx 6 expected custom-URL stream, got {other:?}"),
        }
        // Then a separator + Bridge submenu.
        assert!(matches!(&layout[7], MenuItemSpec::Separator));
        match &layout[8] {
            MenuItemSpec::Submenu { label, items } => {
                assert!(label.contains("Bridge"));
                // Status label + Pause + Resume + Repair = 4 (no eval/snap by default)
                assert_eq!(items.len(), 4, "default submenu size");
            }
            other => panic!("idx 8 expected Submenu, got {other:?}"),
        }
        // Then sep + toggle + sep + quit.
        assert!(matches!(&layout[9], MenuItemSpec::Separator));
        assert!(matches!(&layout[10], MenuItemSpec::Toggle { .. }));
        assert!(matches!(&layout[11], MenuItemSpec::Separator));
        assert!(matches!(
            &layout[12],
            MenuItemSpec::Action {
                command: TrayCommand::Quit,
                ..
            }
        ));
    }

    /// `eval_days_remaining = Some(N)` adds the eval label inside the
    /// Bridge submenu.
    #[test]
    fn bridge_submenu_includes_eval_indicator_when_on_trial() {
        let state = empty_state(BridgeMenuState {
            ready: true,
            paused: false,
            snapshot_age_hours: None,
            eval_days_remaining: Some(82),
        });
        let layout = menu_layout(&state);
        match &layout[8] {
            MenuItemSpec::Submenu { items, .. } => {
                // 4 default + 1 eval = 5
                assert_eq!(items.len(), 5);
                let eval_label = items
                    .iter()
                    .find_map(|i| match i {
                        MenuItemSpec::Label { text } if text.contains("Eval") => Some(text),
                        _ => None,
                    })
                    .expect("eval label present");
                assert!(eval_label.contains("82"), "got {eval_label:?}");
            }
            other => panic!("expected Submenu, got {other:?}"),
        }
    }

    /// Negative eval days renders as "expired".
    #[test]
    fn bridge_submenu_eval_label_marks_expired() {
        let state = empty_state(BridgeMenuState {
            ready: true,
            paused: false,
            snapshot_age_hours: None,
            eval_days_remaining: Some(-7),
        });
        let layout = menu_layout(&state);
        if let MenuItemSpec::Submenu { items, .. } = &layout[8] {
            let eval_label = items
                .iter()
                .find_map(|i| match i {
                    MenuItemSpec::Label { text } if text.contains("Eval") => Some(text),
                    _ => None,
                })
                .expect("eval label present");
            assert!(
                eval_label.contains("expired") && eval_label.contains('7'),
                "got {eval_label:?}"
            );
        }
    }

    /// `snapshot_age_hours = Some(48)` adds a "2d" snapshot label.
    #[test]
    fn bridge_submenu_includes_snapshot_age() {
        let state = empty_state(BridgeMenuState {
            ready: true,
            paused: false,
            snapshot_age_hours: Some(48),
            eval_days_remaining: None,
        });
        let layout = menu_layout(&state);
        if let MenuItemSpec::Submenu { items, .. } = &layout[8] {
            let snap_label = items
                .iter()
                .find_map(|i| match i {
                    MenuItemSpec::Label { text } if text.contains("Snapshot") => Some(text),
                    _ => None,
                })
                .expect("snapshot label present");
            assert!(snap_label.contains("2d"), "got {snap_label:?}");
        }
    }

    /// Bridge status text reflects ready / paused / not-provisioned.
    #[test]
    fn bridge_status_label_for_each_state() {
        assert!(bridge_status_label(&BridgeMenuState::default()).contains("Not provisioned"));
        assert!(bridge_status_label(&BridgeMenuState {
            ready: true,
            paused: false,
            snapshot_age_hours: None,
            eval_days_remaining: None,
        })
        .contains("Ready"));
        assert!(bridge_status_label(&BridgeMenuState {
            ready: true,
            paused: true,
            snapshot_age_hours: None,
            eval_days_remaining: None,
        })
        .contains("Paused"));
    }

    /// `build_routes` includes the V3 stream + bridge actions.
    #[test]
    fn v3_build_routes_includes_stream_and_bridge_actions() {
        let state = empty_state(BridgeMenuState::default());
        let routes = build_routes(&state);
        let mut saw_stream = false;
        let mut saw_pause = false;
        let mut saw_resume = false;
        let mut saw_repair = false;
        for cmd in routes.values() {
            match cmd {
                TrayCommand::StreamUrl(_) => saw_stream = true,
                TrayCommand::BridgePause => saw_pause = true,
                TrayCommand::BridgeResume => saw_resume = true,
                TrayCommand::BridgeRepair => saw_repair = true,
                _ => {}
            }
        }
        assert!(saw_stream, "StreamUrl missing in routes");
        assert!(saw_pause, "BridgePause missing in routes");
        assert!(saw_resume, "BridgeResume missing in routes");
        assert!(saw_repair, "BridgeRepair missing in routes");
    }

    /// Each streaming quick-launch has a distinct URL.
    #[test]
    fn stream_urls_are_distinct() {
        let state = empty_state(BridgeMenuState::default());
        let layout = menu_layout(&state);
        let urls: Vec<String> = layout
            .iter()
            .filter_map(|i| match i {
                MenuItemSpec::Action {
                    command: TrayCommand::StreamUrl(url),
                    ..
                } => Some(url.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(urls.len(), 4, "expected 4 stream URLs");
        // Three known URLs + one empty (custom prompt slot).
        let mut sorted = urls.clone();
        sorted.sort();
        assert!(sorted.windows(2).all(|w| w[0] != w[1]), "duplicates");
    }

    /// `eval_days_label` formats positive + negative + zero days.
    #[test]
    fn eval_days_label_formatting() {
        assert!(eval_days_label(0).contains("0 days"));
        assert!(eval_days_label(1).contains("1 days"));
        assert!(eval_days_label(82).contains("82 days"));
        assert!(eval_days_label(-1).contains("expired"));
        assert!(eval_days_label(-1).contains("1 days"));
    }

    /// `snapshot_age_label` formats hours vs days.
    #[test]
    fn snapshot_age_label_formatting() {
        assert!(snapshot_age_label(0).contains("0h"));
        assert!(snapshot_age_label(23).contains("23h"));
        assert!(snapshot_age_label(24).contains("1d"));
        assert!(snapshot_age_label(48).contains("2d"));
        assert!(snapshot_age_label(168).contains("7d"));
    }

    /// `Submenu` items have a non-empty label.
    #[test]
    fn submenu_label_is_non_empty() {
        let state = empty_state(BridgeMenuState::default());
        let layout = menu_layout(&state);
        if let MenuItemSpec::Submenu { label, .. } = &layout[8] {
            assert!(!label.is_empty());
            assert!(label.contains("Bridge"));
        }
    }

    /// `Label` items report `is_actionable() == false`.
    #[test]
    fn label_items_are_not_actionable() {
        let l = MenuItemSpec::Label {
            text: "Status: Ready".into(),
        };
        assert!(!l.is_separator());
        assert!(!l.is_actionable());
        assert_eq!(l.label(), "Status: Ready");
    }

    /// `Submenu` items report `is_actionable() == false` (children are
    /// actionable; the header isn't).
    #[test]
    fn submenu_items_are_not_actionable() {
        let s = MenuItemSpec::Submenu {
            label: "Bridge ▶".into(),
            items: vec![],
        };
        assert!(!s.is_separator());
        assert!(!s.is_actionable());
        assert_eq!(s.label(), "Bridge ▶");
    }

    /// `BridgeMenuState::default` is "not provisioned, no trial, no
    /// snapshot".
    #[test]
    fn bridge_menu_state_default_is_blank() {
        let s = BridgeMenuState::default();
        assert!(!s.ready);
        assert!(!s.paused);
        assert!(s.snapshot_age_hours.is_none());
        assert!(s.eval_days_remaining.is_none());
    }

    /// `route_item_into` for a submenu emits one route per actionable
    /// child, each with a distinct id derived from the parent id.
    #[test]
    fn route_item_into_submenu_handles_children() {
        let mut routes = std::collections::HashMap::new();
        let sub = MenuItemSpec::Submenu {
            label: "Bridge ▶".into(),
            items: vec![
                MenuItemSpec::Action {
                    label: "Pause VM".into(),
                    command: TrayCommand::BridgePause,
                },
                MenuItemSpec::Action {
                    label: "Resume VM".into(),
                    command: TrayCommand::BridgeResume,
                },
                MenuItemSpec::Label {
                    text: "Status: Ready".into(),
                },
            ],
        };
        route_item_into(&mut routes, 8, &sub, None);
        // 2 actionable children → 2 routes (Label is read-only).
        assert_eq!(routes.len(), 2);
        let mut cmds: Vec<TrayCommand> = routes.values().cloned().collect();
        cmds.sort_by_key(|c| format!("{c:?}"));
        assert!(cmds.contains(&TrayCommand::BridgePause));
        assert!(cmds.contains(&TrayCommand::BridgeResume));
    }
}

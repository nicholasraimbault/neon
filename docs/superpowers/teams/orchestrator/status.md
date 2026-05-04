# Orchestrator Status

**Lead:** Claude (main session)
**Team:** `neon-v2`
**Active phase:** Phase 3 done — Phase 4 (CLI completion) next

## Current focus

Phase 3 complete. 343 tests passing (210 → 243 after platform → 343 after daemon), all four verification gates green. Both teams committed; working tree clean.

Phase 3 ran serially (platform → daemon) per guardrails after the noctalia incident — no further desktop disruption. All cargo invocations used `--jobs 2` cap; `cargo tarpaulin` never invoked.

Phase 4 spawns the **cli team** (next): wires up all 13 CLI subcommands (`init`, `setup`, `patch`, `status`, `list-browsers`, `doctor`, `test`, `update`, `repair`, `launch`, `uninstall`, `completion`, `manpage`) to call into the now-complete core-engine + platform + daemon modules. CLI team also implements EME error-code translation and the interactive first-run wizard. Phase 4 is a single-team phase per the orchestration plan.

## Decisions made (recorded for handoff)

- 2026-05-04: Cloudflare Worker lives as `cloudflare-worker/` subdirectory in main `neon` repo (not separate repo)
- 2026-05-04: Beta tester recruitment via pinned GitHub issue first; subreddits considered in Phase 6
- 2026-05-04: `homebrew-neon` tap archival happens 30 days after V2 ships (grace period)
- 2026-05-04: Orchestrator → user check-ins at end of each phase + on blockers (not per-task)
- 2026-05-04: Phase 3 spawned serially (platform → daemon) after parallel agent activity correlated with noctalia-shell crash
- 2026-05-04: `neon localhost-bridge` queued as **V3 stretch goal** behind Cargo feature flag `experimental-bridge`. Recipe: Win11 IoT LTSC (BYO license) + Looking Glass B7 + GPU/TPM passthrough + HEVC (free in IoT LTSC). Verified gap: WinBoat (21k⭐) abandoned Looking Glass; cloud SaaS bans VOD streaming; 50-200k addressable audience. Three blockers documented: license grey-area (mitigated by BYO posture), Looking Glass IDD paused (mitigated by $5 dummy HDMI plug), niche pricing (free / part of Neon). Build after V2.0 ships.

## Phase status

| Phase | Status | Notes |
|---|---|---|
| 0 — Foundation | **Done** | 6 commits; infra agent reports complete; verified locally (build + fmt + clippy green) |
| 1 — Core skeleton | **Done** | 8 commits; manifest, browsers, config, error, lockfile shipped; 95.38% coverage on owned modules |
| 2 — Widevine + patching | **Done** | core-engine 87% / platform 88.7% coverage; 210 tests passing |
| 2.x — Sudo batching fix | **Done** | migration's 5+ prompts → 1 prompt via `run_as_root_script` |
| 3 — Daemon + tray + watcher | **Done** | platform: lifecycle + power; daemon: tray + watcher + IPC + notify + hooks + run(); 343 tests; serial spawn worked, no desktop disruption |
| 4 — CLI completion | Pending | cli team (single-team phase) |
| 3 — Daemon | Pending | daemon + platform (parallel) |
| 4 — CLI completion | Pending | cli sequential |
| 5 — Distribution + docs | Pending | infra + platform |
| 6 — Beta + release | Pending | All teams on standby for fixes |

## Active blockers

**Pending Nick action items (non-blocking for code work, blocking for full V2 launch):**
1. Branch protection rules on `master` and `v2-rust-rewrite` — `gh api` commands ready in `docs/superpowers/teams/infra/handoff.md`
2. Cloudflare Worker deployment — runbook in `cloudflare-worker/README.md`; needs `wrangler login` + D1 setup
3. (Optional) Re-enable GitHub Issues on the repo; set `CODECOV_TOKEN` secret

## Decision log

(empty)

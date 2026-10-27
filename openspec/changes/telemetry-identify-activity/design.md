# Design: richer telemetry — identify, instances, activity

## Context

`src/apps/kt-app/src/telemetry.rs` (change telemetry-posthog) sends `app_started` on the first
frame and `app_closed` from `on_exit`, through one sender thread and a bounded channel, with a
seven-key allow-list and `$process_person_profile: false`. One `Telemetry` per process
(`install`/`global`); the harness sets its own with a recording sink. The first-start message
is a bell entry plus a status-line headline; the View menu holds the switch. The key reaches
the Linux and the Windows release builds (`src/scripts/release-build.sh` passes it to both
targets). The owner's request and answers: `docs/user-feedback.md`, "Richer telemetry"
(2026-10-28). The neighbouring change `update-check` adds a second checkbox next to ours in
View and a second network path; it does not touch telemetry.

## Goals / Non-Goals

**Goals:** a PostHog person per install with current properties; instances told apart and
their activity visible; time-in-app also for crashed sessions; where it crashes; what machines
and FFmpeg setups people have; how projects and exports look — all without names or paths,
inside the free tier, and with calm texts.

**Non-Goals:** session replay, autocapture, feature flags or surveys; per-click events; crash
backtraces or messages; any identifier beyond the random install id; re-asking consent; a
telemetry window; telemetry from `kt-cli`.

## Decisions

### D1. One schema table per event
A `const SCHEMA: &[(&str /*event*/, &[&str] /*keys*/)]` plus the common keys. `event_json`
takes a map of properties and asserts (debug) and filters (release) against the row, so an
unlisted key cannot leave the machine even if a hook adds it by mistake. t837 compares every
event against its row; the README and the bell details are written from the same table by hand
and checked by t840 for the plain-words groups.

### D2. Identify through `$set` on the start, not a separate `$identify`
PostHog creates/updates a person from `$set`/`$set_once` on any event once person processing
is on. The install id is already the distinct id and never changes, so there is nothing to
merge — a separate `$identify` event would add one event per session for the same effect.
`$set` repeats on every start (versions and hardware change), `$set_once` keeps the first
version and OS. **Reconsideration** of telemetry-posthog D3 (`$process_person_profile: false`):
reversed by the owner's request; `$geoip_disable`, the discarded IP and the random id stay.
Cost: person-processed events are priced higher above the free tier; the expected volume
(D6) stays inside it.

### D3. Standard PostHog keys where they exist
`$os`, `$os_version`, `$app_version`, `$app_build` feed PostHog's built-in breakdowns.
`$app_version` is the short `ktYY.MM` (stable for grouping), `$app_build` the full
`kt26.10.28-hash` (for "which exact build crashed"). The old `os`/`app_version` keys are
dropped — the project is days old, nothing depends on them.

### D4. Instances: session UUID v7, a sequence file, kernel locks
`$session_id` becomes UUID v7 (48-bit ms time + random; built from `getrandom` and
`SystemTime` — the one wall-clock read, outside offline paths, invariant 11 untouched). PostHog
treats only v7 ids as session ids for its Sessions views. `instance_seq` is a counter file
`<data_dir>/telemetry-seq` (read-increment-write; a race between two starts can give the same
number — accepted, it is a statistic). `instances_running`: each process creates
`<data_dir>/instances/<session>.lock` and holds `File::try_lock` for its life — the same kernel
lock `project_lock.rs` uses (`flock` / `LockFileEx`, released by the kernel on any death); at
start it tries to lock every other file there: held → a live instance, free → stale, removed.
Rejected: listing processes by name (platform code, sees renamed binaries wrongly); a pid file
(pids are reused, a crash leaves it).

### D5. Counters: atomics bumped where the person acts, timer on the sender thread
`struct Counters` of `AtomicU64` plus two small `Mutex<BTreeMap<&'static str, u64>>`
(`media_imported` by kind, `fx_added` by registry name, `exports` by outcome) — the map lock is
held only for an increment or a snapshot. Bump sites: the `KtApp` apply/undo/redo wrappers
(`edits`, `undos`, `redos`, and `fx_added` / `clips_added` by matching the command), the media
add paths (`add_media_clip_at`, the bin import, generators), `save`, project open/import/
recovery, the render dialog's end of job, and `draw_all` for two transitions: playback
started/stopped (`playback_starts`, `playback_s` from `Instant`s) and "this frame had input
events" (marks the current 10-second slot active → `active_s`). Each is one branch and an
atomic add — no allocation in the draw path (invariant 12). The heartbeat: the sender loop uses
`recv_timeout(until next 5-minute mark)`; on timeout it snapshots the counters, compares with
the last sent snapshot and sends `app_heartbeat` only if they differ. The UI clock and repaint
are never involved, so an idle program stays idle. A fake clock is injected for t879.

### D6. Volume
Per session: `app_started`, `app_closed`, ≤ 12 heartbeats per ACTIVE hour, plus milestones
(projects opened, exports) and rarely `app_crashed`. A heavy user at 4 active hours a day is
~60 events/day ≈ 1 800/month; 1M/month covers ~500 such users or several thousand casual
ones. If it grows, the heartbeat period is one constant.

### D7. Milestones and buckets
`project_opened` and `export_finished` (spec) use one bucket function for counts and one for
durations, so dashboards can group without leaking exact sizes. `speed_x` rounds to one
decimal. `export_finished` is emitted where the render dialog clears `render_job`
(`render_dialog.rs`), the only place that knows the outcome, settings and the backend.

### D8. Crash: the place, not the message
The panic hook (`journal::install_panic_hook`) already writes `kt-app-crash.txt`; it gains a
`session:` line. At the next start `take_crash_report` (already called once to tell the person)
hands the report's text to `telemetry::crash_from_report`, which parses `kinetren <version>`,
`thread:` and the `panicked at <file>:<line>:<col>` location, cuts the path to the part after
`src/` and DROPS everything after the location. Rejected: sending a backtrace (addresses of a
stripped binary are useless, symbols may name paths); the message (often holds the person's
paths).

### D9. Environment probe, fail-soft, after the first frame
`probe_env(&EnvSources)` takes its inputs as data (os-release text, adapter info, RAM, cores,
scale, monitor size, FFmpeg state) so t886 runs on samples; the production wrapper gathers
them: `/etc/os-release` (Linux); `RtlGetVersion` from ntdll through a small `extern "system"`
declaration (Windows — `GetVersionEx` lies without a manifest); `/proc/meminfo` /
`GlobalMemoryStatusEx`; `available_parallelism`; the wgpu adapter info that `ui()` already
reads for the software-render check; `ctx.pixels_per_point()` and the viewport's monitor size;
the FFmpeg state from the loader (`ffmpeg_setup.rs` knows system / chosen folder / downloaded /
missing). Missing values are omitted. Because the adapter is known only on the first `ui()`
frame, `app_started` is built there — as today.

### D10. Texts: bell only, one calm line, details under a spoiler
`Notice` gains `details: Option<String>`; the notices window draws such an entry as the summary
line plus a collapsed `CollapsingHeader` ("Details") with the details — the first collapsible
notice, a general facility. The first-start code stops writing `self.status`. The not-re-shown
rule needs nothing new: a stored `on` already means "seen". Catalog keys: `telemetry.summary`,
`telemetry.details_title`, `telemetry.sent` (plain-words groups), `telemetry.not_sent`,
`telemetry.how_off` — each ≤ 160 characters as the catalog requires, Arabic punctuation rules
kept; `headline` is removed. The View hint becomes `summary` + `not_sent`. README "Privacy"
keeps one friendly paragraph and puts the lists in `<details>`. The live-scenario dump already
flattens line breaks (t843); the details part is not dumped.

## Risks / Trade-offs

- [Identify makes events pseudonymous rather than anonymous] → the person is a random id with
  coarse properties; GeoIP off, IP discarded; texts still say "anonymous" in the everyday sense
  (not tied to you) — the details say "a random install number".
- [Hardware + screen + OS version narrow down a machine (fingerprinting)] → coarse values only
  (RAM in GB, sizes rounded, no serials, no host name); nothing joins it to a person outside
  PostHog.
- [Counter hooks scattered over app.rs] → each hook is one call `tel::bump(...)` that is a no-op
  without telemetry; t884 drives the real UI paths, so a lost hook turns red.
- [Wall clock for UUID v7] → used only for the id's time prefix; a skewed clock affects
  PostHog's session ordering, nothing in the program.
- [Two concurrent starts race on `telemetry-seq`] → accepted.
- [`update-check` edits the same View menu block] → only additive lines; whichever lands second
  rebases its two lines.

## Migration Plan

No data migration. Old builds keep sending the old keys until updated; dashboards built on
`os`/`app_version` move to `$os`/`$app_version`. The `instances/` directory and
`telemetry-seq` appear on the first start.

## Open Questions

None that change what gets built.

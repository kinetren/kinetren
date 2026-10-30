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

### D5. One door: `telemetry::signal(Signal)`; counts are derived inside the module
The owner asked that telemetry not smear across the code (2026-10-28). The application reports
FACTS, never counts or events:

```rust
pub enum Signal<'a> {
    Commands(&'a [Command]),        // apply / apply_batch / apply_coalesced — 3 lines
    Undo, Redo,                     // undo(), redo() — 2 lines
    Saved,                          // the save path — 1 line
    ProjectOpened { how: OpenHow }, // KtApp::open / new_empty / recovery — ≤ 3 lines
    ExportEnded(&'a ExportFacts),   // render_dialog.rs where render_job ends — 1 line
    Frame { playing: bool, had_input: bool }, // draw_all — replaces today's t.frame(lang)
    CrashReport(&'a str),           // next to take_crash_report — 1 line
    Exit,                           // on_exit — replaces today's t.closed(..)
}
```

`telemetry::signal` is a free function: `global()` absent or consent off → return. Inside
the module: `Commands` is classified once per batch (`AddMedia` → kind from the path's
extension or `gen:` prefix; `AddClip` → `clips_added`; the FX-adding commands → `fx_added`
by registry name; every batch → `edits`); `Frame` compares `playing` with the previous frame
(start/stop → `playback_starts`, `playback_s` from `Instant`s) and marks the current 10-second
slot on input. Counters are `AtomicU64` plus three small `Mutex<BTreeMap<&'static str, u64>>`
(`media_imported`, `fx_added`, `exports`) touched only by non-frame signals and the sender's
snapshot. `Frame` is two atomic loads/stores and at most one `Instant::now()` — no lock, no
allocation (invariant 12); `Commands` iterates a slice the wrapper already has. About 14 call
sites in total, each one line; t887 counts them so a new door is a deliberate edit.
Rejected: hooks at every feature (the "smear" the owner rejected); deriving activity by diffing
model snapshots at heartbeat time (cannot see undo/redo, effect kinds or playback, and copies
the project); a generic `track(name, props)` API (moves the allow-list out of the module).

The heartbeat: the sender loop uses `recv_timeout(until next 5-minute mark)`; on timeout it
snapshots the counters, compares with the last sent snapshot and sends `app_heartbeat` only if
they differ. The UI clock and repaint are never involved, so an idle program stays idle. A fake
clock is injected for t879.

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

### D11. A local PostHog stand-in, in tests and in the live run
A real self-hosted PostHog (ClickHouse, Kafka, Postgres, Redis, several GB of RAM) is not a
test fixture. Instead `telemetry::testing::FakePostHog` — a `TcpListener` on `127.0.0.1:0` with
a minimal HTTP/1.1 reader (the one t841 already has, moved here and reused) — answers
`200 {"status":1}` and validates what the ingestion validates (spec). t888 drives the harness
app through it with the real `HttpSink`. For the real binary, debug builds read
`KT_TELEMETRY_KEY` next to `KT_TELEMETRY_URL` (both required; release ignores both, as it
already ignores the URL), and the live scenario `src/scripts/live/telemetry.py` runs a Python
stand-in with the same checks — it goes into `live-check.sh` with the others, so `finish.sh`
proves on every run that the real program still reaches PostHog in the right shape.

### D12. What implementation changed in the plan (2026-10-30)
- **Signal variants.** The model rides with the commands (`Commands(&cmds, &model)`): an
  effect chain is replaced whole (`SetFx`), so "which effect was added" is the registry name
  the new chain has more of than the model's current one — the door runs BEFORE the apply.
  `apply_coalesced` gets its own `Gesture(cmd, id, model)`: a drag sends a command per frame,
  one gesture is one edit. `Gpu(&AdapterInfo, software)` comes from the software-render check
  `ui()` already does; `Frame` carries the `egui::Context` (input activity, and on the first
  frame the scale and monitor size). `CrashReport` takes the report's path; the module reads it.
- **The harness reaches the door through a thread-local binding** (`testing::bind`, test
  builds only): the harness installs no `GLOBAL`, and a per-app value would bring back the
  second `app_started` per document that `install`/`global` exist to prevent.
- **Milestones before the first frame** (the project opened at start, the crash report) wait
  in the module and follow `app_started`; the probe and `app_started` are built on the sender
  thread (they read files).
- **`win.details`, not `telemetry.details_title`:** the collapsible notice is a general
  facility. A new `telemetry.where` says where the data goes; `how_off` now names only the
  environment switch, because the summary already names the View item.
- **t884's path is a drop onto the window**, the real gesture that imports and places a clip in
  one go, rather than a bin import plus a drag.
- **The export backend** is a `OnceLock<bool>` on `RenderJob` that the job's thread sets when
  it chooses GPU or CPU; until then `backend` is omitted.

## Risks / Trade-offs

- [Identify makes events pseudonymous rather than anonymous] → the person is a random id with
  coarse properties; GeoIP off, IP discarded; texts still say "anonymous" in the everyday sense
  (not tied to you) — the details say "a random install number".
- [Hardware + screen + OS version narrow down a machine (fingerprinting)] → coarse values only
  (RAM in GB, sizes rounded, no serials, no host name); nothing joins it to a person outside
  PostHog.
- [A door forgotten or a new one added ad hoc] → t887 counts the doors; t884 drives the real
  UI paths, so a lost door turns red.
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

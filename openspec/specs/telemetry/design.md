# Telemetry — why it is built this way

## What was missing and who asked

Before public distribution the owner had no way to know whether anyone runs Kinetren, how
long a session lasts and how often it ends without a clean close (owner, 2026-10-22,
`docs/user-feedback.md`, "Telemetry: launches, time in the program, clean close"). Research
of other editors and tools is in `docs/features/collab/depend/anonymous-usage-telemetry.md`
(F-COL-046). Change `telemetry-posthog`, archived 2026-10-22.

## Owner decisions (do not change without a new request)

- **Opt-out**: on by default, said once on the first start, switched off in View.
- **Only the exit event**: no crash marker on disk; a start without a close is the signal.
- **One region, EU**: the owner asked whether US/EU could follow the time zone; that needs two
  PostHog projects, two keys and the data split in half.
- **No heavy SDK**: the owner feared an uncontrolled dependency; our own capture client is
  one JSON POST per event. On TLS: "whatever is simpler".
- **The notice lists what IS sent and what is NOT sent** — asked after the Audacity finding of
  F-COL-046 (its 2021 telemetry was opt-in and still drew a backlash because people could not
  see what was collected).

## Where it lives

- `src/apps/kt-app/src/telemetry.rs` — consent (`resolve_consent`), install id
  (`<data_dir>/telemetry-id`), the payload allow-list (`event_json`), the sender thread, the
  process-wide `install`/`global`, the notice and hint texts.
- `src/apps/kt-app/src/app.rs` — `run` installs the telemetry before the first `KtApp`;
  `draw_all` sends `app_started` and posts the notice on the first frame; `save` stores the
  choice under `telemetry`; `on_exit` sends `app_closed` after the recovery write.
- `src/apps/kt-app/src/toolbar.rs` — the View menu checkbox next to Language.
- `src/scripts/release-build.sh` + `src/packaging/posthog-key` — the only way the key enters
  a binary.

## Non-obvious details

- **One `Telemetry` per process, not per `KtApp`.** File → New/Open rebuild `KtApp` in place
  at nine `*self = …` sites; a per-app value would be one missed site away from a second
  `app_started` per New. `frame`/`closed` are idempotent; t840 checks that every constructor
  takes `telemetry::global()`.
- **`Drop` sends nothing.** `KtApp::drop` runs on File → New (the session goes on) and on panic
  unwind (not a clean close).
- **The sender thread is spawned lazily**, on the first event: a build without a key or a
  person who switched it off never gets the thread (t839).
- **The environment can only switch telemetry OFF.** `KINETREN_TELEMETRY=1` does not override
  a stored `off`.
- **A debug build honours `KT_TELEMETRY_URL`** (live check against a local capture server);
  a release build ignores it, so a shipped binary cannot be redirected.
- **Arabic catalog strings come back in display order** (`i18n::bidi` reorders words), so a
  test that looks for the checkbox name inside the notice compares word sets, not substrings.
- **The live-scenario dump is hand-built JSON.** The telemetry notice was the first multi-line
  notice and its raw line break made `geom.json` unparsable — every live scenario then waited
  for a frame it could not see. Line breaks become ` | ` (t843).
- **The release binary writes no geometry dump**, so the live-scenario frame wait cannot run
  it; a real end-to-end check starts the binary directly and closes it with WM_DELETE_WINDOW
  (`scripts/ui-probe.py close_window`).
- **The PostHog project must have "Discard client IP data" on** — otherwise the notice's "the
  IP address is not stored" is false. Done by the owner on 2026-10-22.

## Rejected and why

- `posthog-rs`: pulls `reqwest` and tokio for two requests a run.
- Spawning `curl`: an external binary per event, behaviour per distribution.
- `native-tls`: OpenSSL at runtime — a new library in `ldd` and a version matrix for the
  portable tarball.
- dlopen of the system OpenSSL (the FFmpeg pattern): zero crates, but hand-written FFI for TLS
  verification; the owner chose the simpler `ureq` + `rustls`.
- An opt-in dialog: the owner chose opt-out.
- A disk marker / heartbeat for crashes: the pair of events already answers it.
- Aptabase: 20k events a month free, against PostHog's 1M.

## The checkbox moved to File › Preferences… (welcome-window-and-settings, archived 2026-11-02)

The owner's rule of 2026-10-27 was "a per-machine preference lives next to the other ones", and View
was where they were. His request of 2026-11-01 (kkt-26.11.01 item 13) moves the program's
preferences out of View into one window; the rule is kept — the checkbox stands in Preferences next to
Language, with the same disabled-with-a-reason form. The notice and the hint name the new path
("File › Preferences…"). The stored value stays where the process-global object owns it.

## The telemetry journal (telemetry-journal, archived 2026-11-08)

The owner's run of 61414dba (kkt-26.11.07b G6, G10): a load report stamped "N s ago" from its
writing, a Diagnostics log stamped at the next UI frame, a playback black box in memory and no record
of what he did — "я не могу тебе сказать точное время". The mechanism: diagnostic records kept apart,
stamped late or relatively, without the person's action next to them. The walk of every record of
the program and its verdict (folded, mirrored, stays apart and why) is the change's design D1
(`openspec/changes/archive/2026-11-08-telemetry-journal/design.md`).

- `src/apps/kt-app/src/tele_journal.rs` — the journal: the caller stamps, one writer blocked in
  `recv` appends with no buffer, rotation at 2 MiB into `.1`/`.2`, a flush barrier from the panic hook,
  the 15-minute tail; `on_signal` hears `telemetry::signal` whatever the consent; `flight_sink`.
- `load_watch.rs` — busy seconds and the problem notes at their second; the report is the journal's
  export. `window_shell.rs` — windows opened/closed. `widgets::chain_strip` — the effect selected.
  `app.rs` — the playback summary, the mark (`Cmd::MarkMoment`, Ctrl+Alt+M), the Diagnostics pane.
- Not usage telemetry: nothing in `telemetry.rs` reads the journal, and the journal sends nothing.
- English, as the render log: the file is the evidence handed over. Paths become file names.
- Measured on the heavy showcase: 0 wake-ups and 0 bytes at rest (the change's design).

## The structured trace (structured-telemetry, archived 2026-11-09)

The owner, kkt-26.11.09 I5–I8: the journal said THAT work ran, never which frame asked for it or why it
waited, so every complaint became a reproduction on our machine — the bench and the version bisect he
refused. The trace records causes with ids and a tool asks the questions. The decisions, the class walk
of every subsystem that waits or keeps, and the measurements are the change's design
(`openspec/changes/archive/2026-11-09-structured-telemetry/design.md`).

- `src/libs/kt-time/src/trace.rs` — the record, the ring, the writer, the files, the reader: std only,
  because every crate that does the work depends on kt-time.
- `src/apps/kt-cli/src/trace_query.rs` — `kt-cli trace`: the questions. A new question goes here, with a
  test on a synthetic run; a new record goes into `Ev` (append only) and into the class walk.
- `tele_journal::install` installs the trace beside the journal (`<data>/trace`); the journal's lines are
  mirrored as text on the trace's clock; the panic hook flushes both.
- Not usage telemetry either: nothing reads it but the person and the tool, nothing is sent.
- Measured on the heavy showcase: a pause and an open effects window add 0 bytes; a UI frame is recorded
  only from 50 ms (a stall), so a pointer over a still place records nothing on a card.
- After `edit-without-rebuild` (merged 2026-11-09) the renderers are built once per project version on
  the player's `engine-build` thread: that build is a `Job` `SHARED_BUILD` record (its version, the
  events it derived, the events its memo holds after it — the rest reused), a composite that waits for
  it records a `FrameWait` `BUILD`, the prefetch thread's taking of a new build an `Invalidate`
  `RENDERERS` with its seq. `AudioBuild` stays in the format for older files; nothing writes it now.

## The rings in the trace (gpu-arbiter, 2026-11-09)

`GpuAdmit` (a span: the wait; class, prediction, flight before it, display flag), `GpuPiece`,
`StreamStop`, and the caller's class in `ComputeJob.id`. `kt-cli trace gpu` answers row 134's question —
did the look-ahead's submissions fall — by class, and the display's present wait. The bench records each
of its presents as a `FrameWait` with `wait::DEVICE`, as the compose thread does.

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

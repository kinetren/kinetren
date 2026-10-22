# Design: anonymous usage telemetry (PostHog Cloud EU)

## Context

Today the tree has no network code at all: no `std::net` use in `src/`, no HTTP or TLS crate
in `src/Cargo.lock` (the fact is stated in rule 13 of `docs/features/README.md`). The
per-machine preferences live in eframe storage (`lang`, `last_project`, written in
`KtApp::save`, `src/apps/kt-app/src/app.rs`); the data directory is
`journal::data_dir()` (`$XDG_DATA_HOME/kinetren`); the exit path is `KtApp::on_exit`, which
first waits up to 5 s for the background autosave, then writes recovery
(`openspec/specs/durability/`). Messages for the person go to the notices window
(`KtApp::warn`, View → Notices). The View menu (`src/apps/kt-app/src/toolbar.rs`) already
holds the per-machine Language choice.

Owner decisions (2026-10-22, `docs/user-feedback.md`): PostHog free tier; opt-out (on by
default, can be switched off); only the exit event, no crash marker. Region: one, EU — the
owner asked whether US/EU could be picked by time zone; that needs two PostHog projects, two
keys and two dashboards with the data split in half, so one region was chosen.

## Goals / Non-Goals

**Goals:** count launches, session length and clean closes; anonymous by construction; zero
effect on UI frame time and at most 2 s on exit; nothing from dev builds or tests.

**Non-Goals:** crash reports or backtraces over the network (the local crash report stays
local, `journal.rs`); feature-usage events; a retry queue; telemetry in `kt-cli`; an opt-in
dialog; a second region.

## Decisions

### D1. PostHog's plain capture endpoint over an SDK
PostHog has no maintained Rust SDK worth a dependency; the ingestion API is one POST of JSON
(`{"api_key","event","distinct_id","properties","timestamp"}`) to
`https://eu.i.posthog.com/i/v0/e/`. Two events per run make batching pointless. Rejected:
`posthog-rs` (unofficial, pulls `reqwest` + tokio — an async runtime for two requests);
Aptabase (20k events/month free is ~10k sessions; PostHog's 1M is enough for the foreseeable
audience and the owner named it).

### D2. `ureq` + `rustls` + `webpki-roots`, in `kt-app` only
Blocking client on our own thread, no async runtime. `rustls` avoids OpenSSL, which would add
a runtime `libssl.so` to `ldd` and a glibc/OpenSSL version matrix to the portable tarball;
bundled `webpki-roots` avoids depending on the system certificate store path, which differs
between distributions. Cost: `ring` builds C/asm (the container already has clang), and the
binary grows by roughly 1–2 MB. Rejected: spawning `curl` (an external binary, behaviour per
distro, and a process per event); `native-tls` (OpenSSL at runtime). The libs/ crates and the
engine core stay network-free.

### D3. Anonymous events: no person profiles, no GeoIP
Each event sets `$process_person_profile: false` (PostHog then stores it as an anonymous event
and creates no person) and `$geoip_disable: true`. PostHog still sees the client IP at the
HTTP layer; the owner enables "Discard client IP data" in the project settings (a step in
tasks, outside the repo). `distinct_id` is a random UUID v4 in `<data_dir>/telemetry-id`,
separate from eframe storage so that resetting UI preferences does not create a new "user".
Counting unique installs uses that id; sessions use `$session_id`.

### D4. Consent: one pure function, three inputs
`resolve_consent(key: Option<&str>, env: &EnvView, stored: Option<&str>) -> Consent { on,
first_start }` — pure, so the whole table is one unit test (t839). Order: no key → off;
`KINETREN_TELEMETRY` in `0|off|false` or `DO_NOT_TRACK=1` → off; stored `off` → off; stored
`on` → on; nothing stored → on + `first_start`. The env cannot turn telemetry ON against a
stored `off` (an override that re-enables what the person disabled would be a dark pattern).
First start posts one notice and stores `on` at the next `save`. The notice lists what is
sent, what is NOT sent (projects, file names and paths, media, names of the person and the
computer, stored IP) and the View menu item (owner, 2026-10-22, after the Audacity finding in
F-COL-046); the checkbox's hover text repeats the two lists from the same catalog entries.

### D5. Thread and exit budget
`Telemetry::start` spawns one thread `kt-telemetry` with a `sync_channel(4)`; `send` uses
`try_send` (full → drop). `app_started` is enqueued on the first `update` (so a start that
fails before a window exists is not a launch). `on_exit` runs its durability steps first,
then enqueues `app_closed`, drops the sender and waits on a `done` channel with
`recv_timeout(2 s)`; the thread is not joined (a detached thread dies with the process).
Requests: connect timeout 1 s, global 2 s. Failures go to `journal::log("telemetry", …)`
once per run. `Drop` for `Telemetry` sends nothing — `Drop` of `KtApp` runs on `*self = app`
(File → New/Open), where the session is not over, and on panic unwind, where it is not clean.
Note: File → New/Open re-create `KtApp` in place, so the `Telemetry` value MUST be moved into
the new app (not re-created), or a second `app_started` would be sent per New.

### D6. Key only at compile time
`option_env!("KINETREN_POSTHOG_KEY")` in `telemetry.rs`; `release-build.sh` reads
`src/packaging/posthog-key` and passes `-e KINETREN_POSTHOG_KEY` into `docker run` (the CI
release workflow goes through `release-build.sh`, so it inherits it). Everything else builds
without the key. A tree guard (t842) refuses a `phc_` literal in `.rs` files so the key cannot
creep into source and from there into every dev build. For a live check, debug builds
(`cfg(debug_assertions)`) honour `KT_TELEMETRY_URL` to point the sender at a local capture
server; release builds ignore it, so the endpoint of a shipped binary cannot be redirected.

## Risks / Trade-offs

- [Opt-out without a dialog is contested under GDPR for EU users] → the data is anonymous by
  D3 (no person, no GeoIP, IP discarded server-side, random id), the first start says so in
  the notices window, and the switch is one click; the owner made this choice explicitly.
- [Notices window is easy to miss] → accepted by the owner's choice of opt-out; the README
  gets a "Privacy" paragraph with the same text and the env switches.
- [First network code; a future change may grow it into feature tracking] → the payload is a
  closed allow-list tested by t837; adding a key is a spec change, not a code edit.
- [Exit can take 2 s longer on a dead network] → bounded and after the recovery write; the
  window is already gone at `on_exit`, so the person does not wait on a visible UI.
- [`ring` build in the release container] → the container has clang; checked by
  `release-build.sh`'s existing delivery checks (no new runtime library in `ldd`).
- [Conflict with `github-ci-docker-build`, which edits `release-build.sh` uncommitted] → the
  release-build task of this change waits until that change's stage with `release-build.sh`
  is committed, then adds one `-e` line.

## Migration Plan

No data or format change. Rollback = remove the module and the dependency; the stored
`telemetry` key and `telemetry-id` file are harmless leftovers.

## Open Questions

- Resolved 2026-10-22: the owner supplied the project key
  (`phc_uenVXmBWNHKszBJcDr6RnNvL589avrpFDjSpKGssGzMk`); it goes into
  `src/packaging/posthog-key` in task 3.1. Region to be confirmed by the first event of the
  live check (4.1): the project must be in EU.
- Resolved 2026-10-22: the owner feared a heavy PostHog library; there is none — our own
  ~100-line capture client (D1). On TLS the owner said "whatever is simpler": `ureq` + `rustls`
  stays (D2), the dlopen-OpenSSL alternative is not taken.

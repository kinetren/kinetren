# Proposal: anonymous usage telemetry (PostHog Cloud EU)

## Why

The owner is about to distribute Kinetren publicly (change `github-ci-docker-build`) and has no
way to know whether anyone runs it, how long a session lasts and how often the program ends
without a clean close. The first numbers needed are three: launches, time spent in the
program, and the share of sessions that closed cleanly (owner, 2026-10-22).

## What Changes

- The GUI (`kt-app`) sends two anonymous events to PostHog Cloud EU (free tier, one project):
  `app_started` after the first frame and `app_closed` from `on_exit` with the session
  length in whole seconds. A session that never sends `app_closed` (crash, kill, power loss)
  is counted as "not closed cleanly" on the PostHog side; there is no crash marker on disk
  (owner's choice).
- Payload is a fixed allow-list: a random install id, a per-run session id, program version,
  OS, CPU architecture, UI language and (on close) the duration. No paths, file names, project
  content, host or user names. Person profiles and GeoIP enrichment are switched off per event.
- Opt-out (owner's choice): on by default; on the first start the program says once in the
  notices window that it sends anonymous statistics and where to switch it off. A checkbox
  "Send anonymous usage statistics" lives in the existing View menu next to Language and is
  remembered. `KINETREN_TELEMETRY=0` or `DO_NOT_TRACK=1` turns it off regardless.
- Only builds that carry the project key send anything. The key comes from the release build
  (`release-build.sh` / the release workflow), not from source, so `cargo run`, `finish.sh`,
  tests, kittest and the live scenarios never report.
- Sending never blocks the UI (background thread) and never holds the exit for more than a
  fixed budget (2 s, after the recovery write). Failures are silent; no retry, no disk queue.
- First HTTP/TLS client in the tree: `ureq` with `rustls` and bundled `webpki-roots`
  (no OpenSSL, no system certificate store, no new runtime library for `ldd`).

## Capabilities

### New Capabilities
- `telemetry`: what the program reports about its own use, when, to whom, the consent model,
  the payload allow-list and the "never block, never report from dev builds" guarantees.

### Modified Capabilities
<!-- none: the exit path keeps its durability contract (recovery written first); the bound on
     the extra wait is stated in the telemetry spec. -->

## Impact

- Code: new `src/apps/kt-app/src/telemetry.rs`; hooks in `src/apps/kt-app/src/app.rs`
  (startup in the `run_native` closure, `update` first frame, `save` storage key, `on_exit`),
  the View menu in `src/apps/kt-app/src/toolbar.rs`, strings in all five catalogs
  `src/apps/kt-app/i18n/*.toml`.
- Dependencies: `ureq` (rustls, webpki-roots) in `kt-app` only; libs/ crates untouched.
- Build: `src/scripts/release-build.sh` passes `KINETREN_POSTHOG_KEY` into the container;
  the key file `src/packaging/posthog-key` (a public, write-only `phc_…` key). Coordinated
  with the in-flight `github-ci-docker-build`, which also edits `release-build.sh`.
- Docs: `docs/user-feedback.md` (owner's words), a feature note in `docs/features/`,
  `docs/features/_services.md` (`posthog`), README privacy paragraph; the README of
  `docs/features/` states "no HTTP crate in Cargo.lock" (rule 13) and must be corrected.
- Owner action outside the repo: create the PostHog EU project, enable "Discard client IP
  data" in its settings, and put the project key into `src/packaging/posthog-key`.

# Proposal

## Why

Kinetren is now published as GitHub Releases (`github.com/kinetren/kinetren`, distribution),
but nothing tells a person who runs an old tarball that a newer one is out — the program
itself never learns about releases (F-RES-008 left the update channel out because there was
no release channel yet). The owner asked (2026-10-27, `docs/user-feedback.md`, "Version
check") for a once-a-day check that reminds through the bell and marks the title briefly,
without any pop-up at start. A stale build is also the known source of "the defect is not
fixed" reports: the owner himself twice ran an old build (kt-version, item 6 round 12).

## What Changes

- Once per calendar day (UTC), after the first frame of a release build, a background thread
  asks the GitHub Releases API for the latest normal release of `kinetren/kinetren`
  (pre-releases and drafts are ignored). The UI thread never waits for it.
- "Newer" is decided from the release's asset names (`kinetren-<YY.MM.DD>-<hash8>-…`), which
  carry the same commit-date version as the binary: newer = a strictly later commit date
  with a different hash. A same-day or older release, or anything unparsable, is "not newer".
- When a newer release exists: ONE info notice in the notices window (the bell counter grows)
  naming our version, the new one and the releases page; the window title — both the OS title
  and our own title row — gets a short translated suffix: `kt26.10 (outdated)`. No dialog, no
  window, no download, no self-update.
- The day's result is cached in `<data_dir>/update-check`: a later start the same day sends no
  request, still shows the title suffix if the cached release is newer than the running build,
  and posts no second notice. A failed check (offline, timeout, HTTP error) is silent (one
  journal line) and does not mark the day as checked.
- View menu: a checkbox "Check for new versions daily", next to the telemetry checkbox, on by
  default, stored in eframe storage under `update_check`. `KINETREN_UPDATE_CHECK=0` turns it
  off. Debug builds never check unless `KT_UPDATE_URL` points at a local test server; a
  release build ignores `KT_UPDATE_URL`.
- Translation catalogs (all 5 languages): the checkbox, its hover text, the notice and the
  title word.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `distribution`: adds the program's side of the release channel — a daily, notice-only check
  for a newer GitHub release, its cache, the title suffix, the View switch and the rule that
  dev builds never ask.

## Impact

- Code: new `src/apps/kt-app/src/update_check.rs` (state file, decision, fetch thread,
  process-wide `install`/`global` like `telemetry.rs`); `src/apps/kt-app/src/app.rs` (start on
  the first frame, poll the result in `draw_all`, notice, title suffix, `save`, `run`);
  `src/apps/kt-app/src/toolbar.rs` (View checkbox; the suffix painted after the project title
  in the title row); `src/apps/kt-app/i18n/*.toml` (new `[update]` section).
- Dependencies: none new — `ureq` (rustls) and `serde_json` are already in kt-app.
- Network: the second network path of the program (after telemetry): one HTTPS GET to
  `api.github.com` per day per machine, carrying only `User-Agent: kinetren/<version>`.
- Docs: `docs/features/resilience/environment/update-available-notice.md` (new note,
  reconnaissance of other editors); `docs/features/README.md` rule 13 prose ("the only network
  path is telemetry") updated; `src/apps/kt-app/CLAUDE.md` first line names `distribution`.
- No project-format change, no engine change, no invariant touched beyond 12 (no repaint loop
  while waiting; one repaint when the result arrives).

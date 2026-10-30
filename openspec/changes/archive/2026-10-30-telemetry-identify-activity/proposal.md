# Proposal: richer telemetry — identify, instances, activity

## Why

Today PostHog receives two anonymous events per run with seven properties, and no person
profiles at all (`$process_person_profile: false`). The owner can count launches and
sessions, but cannot see who runs which version on which OS, whether two instances on one
machine did different things, what a session was spent on, how often FFmpeg is missing, or
where the program crashes (owner, 2026-10-28, `docs/user-feedback.md`, "Richer telemetry").

## What Changes

- **Identify (person per install).** Events stop opting out of person processing; every
  `app_started` carries `$set` (current version, OS, OS version, arch, language, hardware,
  FFmpeg state) and `$set_once` (first seen version and OS), so PostHog's Persons list shows
  one person per install with up-to-date properties. The person is still the random install
  id — no name, no machine name. GeoIP stays disabled; the IP is discarded by the project.
- **PostHog's standard keys.** `$os` (`Linux`/`Windows`), `$os_version` (distribution and
  version, Windows build), `$app_version`, `$app_build` — the built-in breakdowns then work.
- **Instances are told apart.** Every run keeps its own session id, now a UUID v7 (PostHog
  groups sessions only by v7 ids), plus `instance_seq` (the n-th start of this install) and
  `instances_running` (other Kinetren processes alive at start, counted by lock files in the
  data directory).
- **Activity as counters, not per-action events.** A `app_heartbeat` every 5 minutes, sent
  only when something happened since the last one, and `app_closed` carry cumulative
  counters: active time, playback seconds and starts, media imported by kind, clips added,
  edits, undo/redo, project opens and saves, exports by outcome, effects added by kind (names
  from our registry). The heartbeat also gives time-in-app for sessions that later crash.
- **Rare milestones as events:** `project_opened` (size in buckets, how it was opened),
  `export_finished` (container/codec, resolution, fps, duration bucket, outcome, CPU/GPU,
  speed factor), `app_crashed` at the start after a panic (version, thread, panic location
  `file:line` — never the message text, which can hold paths).
- **Environment:** CPU cores, RAM (GB, rounded), GPU vendor/name/type and wgpu backend,
  software render, screen scale and logical size (rounded), FFmpeg major version and how it
  was found (system / chosen folder / downloaded / missing).
- **Texts, calm and short (owner).** The first-start message lives only in the bell (the
  status-line headline goes); it is not shown again to those who saw the old list. The bell
  entry gets a one-line friendly summary with the details under a spoiler; the View hint and
  the README "Privacy" section are updated the same way (README lists under `<details>`).
  "What is NOT sent" stays true and stays in every place.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `telemetry`: the event set (heartbeat, milestones, crash report), person profiles instead
  of anonymous events, the widened allow-list, instance properties, and the first-start
  message (bell only, spoiler, not re-shown).

## Impact

- Code: `src/apps/kt-app/src/telemetry.rs` (event schema, counters, heartbeat timer on the
  sender thread, environment probe, instance locks, UUID v7); hooks in
  `src/apps/kt-app/src/app.rs` (apply wrappers, playback transitions, input activity, open /
  import paths, `on_exit`, crash report hand-off), `render_dialog.rs` (export outcome),
  `ffmpeg_setup.rs` (FFmpeg state), `journal.rs` (panic location for the crash event), the
  notices window (a notice with a collapsible details part), `toolbar.rs` (hint text);
  catalogs `src/apps/kt-app/i18n/*.toml`.
- No new dependency (`ureq`, `serde_json`, `getrandom` are in kt-app). Windows and Linux both
  carry the key already (`release-build.sh` passes it for both targets).
- Docs: README "Privacy"; F-COL-046 is frozen (`сделано`) — this change extends the same
  feature and records its "why" in `openspec/specs/telemetry/design.md` instead of a new note.
- Volume: about 2 + (active hours × 12) + milestones events per session — far inside the
  1M/month free tier for the expected audience.

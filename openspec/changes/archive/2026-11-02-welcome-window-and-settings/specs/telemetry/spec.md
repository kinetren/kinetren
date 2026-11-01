## RENAMED Requirements

- FROM: `### Requirement: On by default, said once, switched off in the View menu`
- TO: `### Requirement: On by default, said once, switched off in Preferences`

## MODIFIED Requirements

### Requirement: On by default, said once, switched off in Preferences
Telemetry SHALL be enabled unless any of these holds: the build has no project key; the
environment has `KINETREN_TELEMETRY=0` (or `off`/`false`) or `DO_NOT_TRACK=1`; the person
turned it off. The choice SHALL be stored in eframe storage under `telemetry` (`on`/`off`)
(the language itself moved to the user store, welcome-window-and-settings D8). On the first start of a build with a key (no stored choice) the program SHALL
post one info entry to the bell (the notices list) and store `on`; it MUST NOT put a headline
into the status line, MUST NOT open any window by itself, and MUST NOT post the entry again —
also not when the list of what is sent grows (owner, 2026-10-28). The entry SHALL read as one
calm line (anonymous statistics help improve Kinetren; how to switch them off: the File › Preferences…
item) with the details under a collapsed spoiler: what IS sent, grouped in plain words
(starts and closes, time in the program, which features are used and how often — as numbers,
version, system and hardware, FFmpeg, crash places), where it goes (PostHog, EU servers), and
what is NOT sent (projects, file and media names and paths, frames or sound, your name, the
computer name; the IP address is not stored). The File › Preferences… window SHALL carry the checkbox "Send
anonymous usage statistics" under Language; its hover text is the same one-line summary
plus the "not sent" sentence. Turning it off during a session SHALL cancel that session's
remaining events. With no key in the build, the checkbox is shown disabled with a hover text
saying this build does not send statistics. Every text comes from the catalogs, in all five
languages.
Why: the owner chose opt-out (2026-10-22) and asked that the person see what is and is NOT
sent (Audacity finding, F-COL-046). On 2026-10-28 he asked to keep it to the bell, not to
show it again, and not to frighten people: details stay one click away under a spoiler.
On 2026-11-01 (kkt-26.11.01 item 13) the program's preferences left View for File ›
Preferences… (design D7 of welcome-window-and-settings).

#### Scenario: Consent resolution
- **WHEN** `resolve_consent(key, env, stored)` is called over the table: no key; key + nothing
  stored; key + `KINETREN_TELEMETRY=0`; key + `DO_NOT_TRACK=1`; key + stored `off`;
  key + stored `on` + `KINETREN_TELEMETRY=1`
- **THEN** the results are: off (no key); on and `first_start`; off; off; off; on, not first
  start — and a `Telemetry` resolved off never spawns the sender thread
- **TEST** t839 (`src/apps/kt-app/src/telemetry.rs`)

#### Scenario: The checkbox is reachable and the notice is said once
- **WHEN** the harness app starts with a key and no stored choice, then opens File › Preferences…, then
  restarts with the stored state
- **THEN** the notices list holds the telemetry entry once after the first start and not after
  the restart; the status line does not show it and no window opened; in every catalog the
  entry's summary names File, Preferences… and the checkbox and its details hold both the "sent" and the "not
  sent" text; the Preferences window holds the checkbox, checked; unchecking it sets the stored value to `off`
  and the recording sink receives nothing more, `app_closed` included
- **PATH** start Kinetren for the first time → the bell shows one unread entry, nothing pops
  up → click the bell → the entry reads one calm line with "Details" collapsed → click
  "Details" → what is sent and what is NOT sent → File › Preferences… → "Send anonymous usage
  statistics" is checked under Language → uncheck it → it stays unchecked after a restart,
  and no entry appears again
- **TEST** t840 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Who saw the old list is not told again
- **WHEN** the app starts with a key and the stored choice `on` left by the previous version
- **THEN** no telemetry entry is posted, and the events carry the new properties
- **TEST** t885 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The multi-line notice keeps the live-scenario dump readable
- **WHEN** a notice with line breaks, quotes and a tab is posted while `KT_GEOM_DUMP` is set
- **THEN** the dump is valid JSON and the notice reads with ` | ` in place of each break.
  Found by the live check of telemetry-posthog: the first multi-line notice in the program
  made `geom.json` unparsable, and every live scenario waited for a frame it could not see
- **TEST** t843 (`src/apps/kt-app/src/layout_tests.rs`)

# Telemetry

## Purpose

What Kinetren reports about its own use: how many times it is started, how long a session
lasts, and whether it closed cleanly. The report is anonymous, goes to one PostHog Cloud EU
project, is on by default with a visible off switch, and never costs the person a frame of
UI or a second of exit.

## Requirements

### Requirement: Two events, and a missing close is the crash signal

The GUI SHALL send `app_started` once per process, after the first frame is drawn, and
`app_closed` from `on_exit` with `duration_s` — whole seconds since process start
(`std::time::Instant`, not wall clock). Both events of one run SHALL carry the same
`$session_id`, a random UUID made at start. The program MUST NOT write a crash or "alive"
marker for telemetry: a session whose `app_closed` never arrived is, by definition, one that
did not close cleanly (owner, 2026-10-22). `kt-cli` SHALL send nothing.
Why: the owner asked for three numbers — launches, time in the program, clean closes — and
chose the exit event alone over a disk marker. Starts without a matching close are countable
on the PostHog side by session id; a disk marker would add a second file in the data
directory and a heartbeat for an answer the pair already gives.

#### Scenario: One run produces exactly the pair, linked by the session id
- **WHEN** a `Telemetry` built with a recording sink is started, the first frame is reported
  twice and the app exits after a simulated 125.6 s
- **THEN** the sink holds exactly `app_started` then `app_closed`, both with the same
  `$session_id`, `app_closed.duration_s == 125`, and a second `on_exit` call sends nothing
- **TEST** t836 (`src/apps/kt-app/src/telemetry.rs`)

#### Scenario: A killed session sends only the start
- **WHEN** the session is started and the `Telemetry` value is dropped without `on_exit`
  (the path of a panic unwind, `*self = app` on File → New, or a kill)
- **THEN** the sink holds only `app_started`; `Drop` sends nothing
- **TEST** t836 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: The payload is a closed allow-list, and it is anonymous

Every event SHALL carry exactly these properties and no others: `$session_id`,
`app_version` (`kt_version::display()`), `os` (`std::env::consts::OS`), `arch`
(`std::env::consts::ARCH`), `lang` (the resolved UI language code), `$process_person_profile:
false`, `$geoip_disable: true`, and on `app_closed` also `duration_s`. `distinct_id` SHALL be
the install id: a random UUID v4 stored as plain text in `<data_dir>/telemetry-id`, created on
first need and reused; an unreadable or malformed file is replaced by a new id. Paths, file
names, project or media data, user, host or machine names MUST NOT appear in any event.
Why: an allow-list is the only form a reviewer can check; "we only send harmless things"
is not checkable and drifts. The install id is random, not derived from the machine, so it
identifies nothing outside PostHog and is gone when the data directory is removed.

#### Scenario: The serialized event holds only allowed keys
- **WHEN** both events are serialized for a project opened from `/home/someone/secret/cut.kt`
  with media under `/media/someone/`
- **THEN** the property keys equal the allow-list exactly, and the JSON text contains neither
  `someone` nor `secret` nor the host name
- **TEST** t837 (`src/apps/kt-app/src/telemetry.rs`)

#### Scenario: The install id survives restarts and heals
- **WHEN** the id is read twice from an empty temp data directory, then the file is replaced
  by `garbage`, then read again
- **THEN** the first two reads return the same UUID and create the file once; after the
  garbage the read returns a new valid UUID and rewrites the file
- **TEST** t838 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: On by default, said once, switched off in the View menu

Telemetry SHALL be enabled unless any of these holds: the build has no project key; the
environment has `KINETREN_TELEMETRY=0` (or `off`/`false`) or `DO_NOT_TRACK=1`; the person
turned it off. The choice SHALL be stored in eframe storage under `telemetry` (`on`/`off`)
next to `lang`. On the first start of a build with a key (no stored choice), the program
SHALL post one notice to the notices window and store `on`; the notice MUST NOT be shown
again. The notice SHALL list, item by item: what IS sent (program start and clean close, session
length, program version, OS and processor type, interface language, a random install number
not tied to the person or the machine) and where it goes (PostHog, EU servers); what is NOT
sent (projects, file names and paths, media, frames or sound, the person's name, the computer
name, the IP address is not stored); and how to switch it off (the View menu item, or
`KINETREN_TELEMETRY=0`). The same lists SHALL be shown as the checkbox's hover text, and both
come from one catalog entry per list, so the notice and the hover text cannot disagree. The View menu SHALL carry a checkbox
"Send anonymous usage statistics" next to Language; turning it off during a session SHALL
cancel that session's `app_closed`. With no key in the build, the checkbox is shown disabled
with a hover text saying this build does not send statistics.
Why: the owner chose opt-out over opt-in (2026-10-22). Research for F-COL-046 showed that
Audacity's 2021 telemetry was opt-in and still drew a backlash, because people could not see
what was collected; so the owner asked (2026-10-22) for the notice to list concretely what is
sent and, to reassure, what is NOT sent. Opt-out is honest only if the person
is told on the first start and the switch is where the other per-machine preferences are
(rule 13 of the kt-app checklist: no new strip for a setting). `DO_NOT_TRACK` is the shared
convention of command-line tools; honouring it costs one line.

#### Scenario: Consent resolution
- **WHEN** `resolve_consent(key, env, stored)` is called over the table: no key; key + nothing
  stored; key + `KINETREN_TELEMETRY=0`; key + `DO_NOT_TRACK=1`; key + stored `off`;
  key + stored `on` + `KINETREN_TELEMETRY=1`
- **THEN** the results are: off (no key); on and `first_start`; off; off; off; on, not first
  start — and a `Telemetry` resolved off never spawns the sender thread
- **TEST** t839 (`src/apps/kt-app/src/telemetry.rs`)

#### Scenario: The checkbox is reachable and the notice is said once
- **WHEN** the harness app starts with a key and no stored choice, then opens View, then
  restarts with the stored state
- **THEN** the notices list holds the telemetry notice once after the first start and not
  after the restart; in every catalog the notice text contains both the "sent" and the "not
  sent" list and the View menu item name, and the checkbox hover text holds the same two lists; View holds the checkbox, checked; unchecking it sets the stored value to
  `off` and the recording sink receives no `app_closed` on exit
- **ПУТЬ** start Kinetren for the first time → a notice lists what is sent, what is NOT sent
  (projects, files, media, names, IP) and points to View → open View → the checkbox "Send anonymous usage statistics" is checked next
  to Language → uncheck it → it stays unchecked after a restart, and no notice appears again
- **TEST** t840 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The multi-line notice keeps the live-scenario dump readable
- **WHEN** a notice with line breaks, quotes and a tab is posted while `KT_GEOM_DUMP` is set
- **THEN** the dump is valid JSON and the notice reads with ` | ` in place of each break.
  Found by the live check of this change: the first multi-line notice in the program made
  `geom.json` unparsable, and every live scenario waited for a frame it could not see
- **TEST** t843 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Sending never blocks the UI and never holds the exit

Events SHALL be sent from one background thread fed by a bounded channel; the UI thread only
enqueues. The HTTP request SHALL use HTTPS to the EU ingestion host
(`https://eu.i.posthog.com/i/v0/e/`) with a connect timeout of 1 s and a total timeout of
2 s. `on_exit` SHALL send `app_closed` only AFTER the recovery write
(durability) and SHALL wait for it at most 2 s in total; a failed or slow request is dropped
silently (one `journal::log` line), with no retry and no on-disk queue. A full channel drops
the event instead of blocking.
Why: invariant 12 (the UI does not wait on anything it does not draw) and the durability
order of `on_exit` (the person's work is written before anything else happens). A telemetry
server that is down must look exactly like no telemetry.

#### Scenario: A black-hole server costs at most the budget
- **WHEN** the sender points at a local `TcpListener` that accepts and never answers, and
  `app_started` plus `app_closed` are sent, then the exit wait runs
- **THEN** enqueuing returns in under 1 ms each, the exit wait returns in under 2.5 s, and the
  process does not panic; against a listener that answers `200` the same pair arrives as two
  POSTs with JSON bodies holding `api_key`, `event` and `distinct_id`
- **TEST** t841 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: Only release builds carry the key

The project key SHALL enter the binary only at compile time through
`option_env!("KINETREN_POSTHOG_KEY")`; the source tree MUST NOT hard-code it in Rust code.
`src/scripts/release-build.sh` (and through it `make` and the release workflow) SHALL read
the committed public key file `src/packaging/posthog-key` and pass it into the container
build. `cargo build`, `cargo test`, `finish.sh`, kittest and the live scenarios build without
it and therefore never send.
Why: the owner's live scenarios and tests start the real `kinetren` dozens of times a day; with
the key in source every one of them would be a "user". The key itself is public (PostHog
project keys only allow writing events), so it may live in the repository — just not in a
build that tests run.

#### Scenario: The tree keeps the key out of dev builds
- **WHEN** the tree is checked
- **THEN** no `.rs` file under `src/` contains a `phc_` literal; `telemetry.rs` reads the key
  only via `option_env!("KINETREN_POSTHOG_KEY")`; `release-build.sh` passes
  `KINETREN_POSTHOG_KEY` into `docker run`; a debug test binary reports `key_present() == false`
- **TEST** t842 (`src/apps/kt-app/src/telemetry.rs`)

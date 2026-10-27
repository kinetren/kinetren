# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Two events, and a missing close is the crash signal`
- TO: `### Requirement: Start, heartbeat and close, and a missing close is the unclean-exit signal`

- FROM: `### Requirement: The payload is a closed allow-list, and it is anonymous`
- TO: `### Requirement: The payload is a closed allow-list per event, keyed by a random install`

## MODIFIED Requirements

### Requirement: Start, heartbeat and close, and a missing close is the unclean-exit signal
The GUI SHALL send `app_started` once per process, after the first frame is drawn, and
`app_closed` from `on_exit` with `duration_s` — whole seconds since process start
(`std::time::Instant`, not wall clock) — and the session's final activity counters. Between
them the sender thread SHALL send `app_heartbeat` every 5 minutes with the cumulative
counters and `uptime_s`, but ONLY when a counter changed since the previous heartbeat (an
idle program sends nothing). All events of one run SHALL carry the same `$session_id`. The
program MUST NOT write an "alive" marker for telemetry: a session whose `app_closed` never
arrived did not close cleanly (owner, 2026-10-22); its last heartbeat bounds how long it ran.
`kt-cli` SHALL send nothing.
Why: the owner asked for launches, time in the program and clean closes (2026-10-22), then
for what a session was spent on (2026-10-28) and chose counters plus a heartbeat over an
event per action: the heartbeat answers "how long and how busy" even for sessions that later
crash, at about twelve events per active hour.

#### Scenario: One run produces exactly the pair, linked by the session id
- **WHEN** a `Telemetry` built with a recording sink is started, the first frame is reported
  twice, nothing is counted, and the app exits after a simulated 125.6 s
- **THEN** the sink holds exactly `app_started` then `app_closed`, both with the same
  `$session_id`, `app_closed.duration_s == 125`, and a second `on_exit` call sends nothing
- **TEST** t836 (`src/apps/kt-app/src/telemetry.rs`)

#### Scenario: A killed session sends only the start
- **WHEN** the session is started and the `Telemetry` value is dropped without `on_exit`
  (the path of a panic unwind, `*self = app` on File → New, or a kill)
- **THEN** the sink holds only `app_started`; `Drop` sends nothing
- **TEST** t836 (`src/apps/kt-app/src/telemetry.rs`)

#### Scenario: The heartbeat speaks only after activity
- **WHEN** with a fake clock the session runs 17 minutes: one import at minute 1, playback of
  40 s at minute 7, nothing after minute 8; then it exits
- **THEN** exactly two `app_heartbeat` events arrive (minutes 5 and 10) with cumulative
  counters (`media_imported.video == 1`, then also `playback_s == 40`), none at minute 15,
  and `app_closed` carries the same final counters; the heartbeat timer lives on the sender
  thread and never requests a repaint
- **TEST** t879 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: The payload is a closed allow-list per event, keyed by a random install
Each event name SHALL have its own closed list of property keys, kept as one table in
`telemetry.rs`, and an event SHALL carry exactly its keys and no others:

- every event: `$session_id`, `$app_version` (`ktYY.MM`), `$app_build` (the full version),
  `$os` (`Linux`/`Windows`), `$os_version`, `arch`, `lang`, `$geoip_disable: true`;
- `app_started`: also `$set`, `$set_once`, `instance_seq`, `instances_running`;
- `app_heartbeat`: also `uptime_s` and the counters;
- `app_closed`: also `duration_s` and the counters;
- `project_opened`, `export_finished`, `app_crashed`: the keys of their requirements below.

`distinct_id` SHALL be the install id: a random UUID v4 stored as plain text in
`<data_dir>/telemetry-id`, created on first need and reused; an unreadable or malformed file
is replaced by a new id. Paths, file names, project, clip, track, marker or media names, media
content, panic message text, user, host or machine names MUST NOT appear in any event.
Why: an allow-list is the only form a reviewer can check; "we only send harmless things" is
not checkable and drifts. The install id is random, not derived from the machine, so it
identifies nothing outside PostHog and is gone when the data directory is removed.

#### Scenario: The serialized event holds only allowed keys
- **WHEN** every event kind is serialized with full counters for a project opened from
  `/home/someone/secret/cut.kt` with media under `/media/someone/`, a clip named `secret clip`
  and a panic whose message names `/home/someone/x.kt`
- **THEN** each event's property keys equal its table row exactly, and no JSON text contains
  `someone`, `secret` or the host name
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
next to `lang`. On the first start of a build with a key (no stored choice) the program SHALL
post one info entry to the bell (the notices list) and store `on`; it MUST NOT put a headline
into the status line, MUST NOT open any window by itself, and MUST NOT post the entry again —
also not when the list of what is sent grows (owner, 2026-10-28). The entry SHALL read as one
calm line (anonymous statistics help improve Kinetren; how to switch them off: the View menu
item) with the details under a collapsed spoiler: what IS sent, grouped in plain words
(starts and closes, time in the program, which features are used and how often — as numbers,
version, system and hardware, FFmpeg, crash places), where it goes (PostHog, EU servers), and
what is NOT sent (projects, file and media names and paths, frames or sound, your name, the
computer name; the IP address is not stored). The View menu SHALL carry the checkbox "Send
anonymous usage statistics" next to Language; its hover text is the same one-line summary
plus the "not sent" sentence. Turning it off during a session SHALL cancel that session's
remaining events. With no key in the build, the checkbox is shown disabled with a hover text
saying this build does not send statistics. Every text comes from the catalogs, in all five
languages.
Why: the owner chose opt-out (2026-10-22) and asked that the person see what is and is NOT
sent (Audacity finding, F-COL-046). On 2026-10-28 he asked to keep it to the bell, not to
show it again, and not to frighten people: details stay one click away under a spoiler.

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
- **THEN** the notices list holds the telemetry entry once after the first start and not after
  the restart; the status line does not show it and no window opened; in every catalog the
  entry's summary names the View menu item and its details hold both the "sent" and the "not
  sent" text; View holds the checkbox, checked; unchecking it sets the stored value to `off`
  and the recording sink receives nothing more, `app_closed` included
- **PATH** start Kinetren for the first time → the bell shows one unread entry, nothing pops
  up → click the bell → the entry reads one calm line with "Details" collapsed → click
  "Details" → what is sent and what is NOT sent → open View → "Send anonymous usage
  statistics" is checked next to Language → uncheck it → it stays unchecked after a restart,
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

### Requirement: Sending never blocks the UI and never holds the exit
Events SHALL be sent from one background thread fed by a bounded channel; the UI thread only
enqueues events and bumps counters, which are atomics (or a map behind a lock taken only by
the bump and by the sender thread, never across I/O). The heartbeat timer SHALL run on the
sender thread (`recv_timeout`), not on the UI clock. The HTTP request SHALL use HTTPS to the EU
ingestion host (`https://eu.i.posthog.com/i/v0/e/`) with a connect timeout of 1 s and a total
timeout of 2 s. `on_exit` SHALL send `app_closed` only AFTER the recovery write (durability)
and SHALL wait for it at most 2 s in total; a failed or slow request is dropped silently (one
`journal::log` line), with no retry and no on-disk queue. A full channel drops the event
instead of blocking.
Why: invariant 12 (the UI does not wait on anything it does not draw, and an idle program
does not wake) and the durability order of `on_exit`. A telemetry server that is down must
look exactly like no telemetry.

#### Scenario: A black-hole server costs at most the budget
- **WHEN** the sender points at a local `TcpListener` that accepts and never answers, and
  `app_started` plus `app_closed` are sent, then the exit wait runs
- **THEN** enqueuing returns in under 1 ms each, the exit wait returns in under 2.5 s, and the
  process does not panic; against a listener that answers `200` the same pair arrives as two
  POSTs with JSON bodies holding `api_key`, `event` and `distinct_id`
- **TEST** t841 (`src/apps/kt-app/src/telemetry.rs`)

## ADDED Requirements

### Requirement: One PostHog person per install, identified on every start
Events SHALL NOT carry `$process_person_profile: false`. `app_started` SHALL carry `$set` with
the install's current properties (`$app_version`, `$os`, `$os_version`, `arch`, `lang`, the
environment keys of the environment requirement) and `$set_once` with `first_app_version`,
`first_os`. The person's distinct id is the install id; no other identifier SHALL be sent.
Why: the owner asked for identify (2026-10-28): the Persons list then shows each install with
its current version, system and hardware, and cohorts ("installs still on kt26.10", "Windows
without FFmpeg") become one filter. The person stays a random number — pseudonymous, not a
person — and GeoIP stays off; the project discards the IP (owner, 2026-10-22).
Reconsideration: telemetry-posthog D3 set `$process_person_profile: false` to keep events
anonymous; the owner's request for identify reverses it, the rest of D3 (no GeoIP, IP
discarded, random id) stands.

#### Scenario: The start identifies the install
- **WHEN** `app_started` is built for an install first seen on `kt26.10` / Linux, now `kt26.11`
- **THEN** it has no `$process_person_profile` key, `$set` holds exactly the person keys with
  `$app_version == "kt26.11"`, and `$set_once` holds `first_app_version` and `first_os`
- **TEST** t880 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: Instances on one machine are told apart
`$session_id` SHALL be a fresh UUID v7 per process (PostHog groups sessions only by v7 ids).
`app_started` SHALL carry `instance_seq` — the n-th start of this install, a counter in
`<data_dir>/telemetry-seq` — and `instances_running` — how many OTHER Kinetren processes are
alive at start, counted by exclusive locks on `<data_dir>/instances/<session>.lock` that each
process holds for its lifetime; a lock file nobody holds is stale and is removed. Activity is
per process: two instances report separate counters under their own session ids.
Why: the owner's example — two instances on one computer, one importing and playing, the other
closed almost at once (2026-10-28). Separate session ids already split the runs; the counters
show what each did, and `instances_running` says that they overlapped.

#### Scenario: Two processes, one install
- **WHEN** in a temp data dir instance A registers, then B registers while A holds its lock,
  then A releases and C registers; and a lock file left without a holder is present
- **THEN** A sees `instances_running == 0`, B sees 1, C sees 1 (B only; the stale file is
  removed), `instance_seq` is 1, 2, 3, and every session id is a valid UUID v7 with a
  non-decreasing time prefix
- **TEST** t881 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: Activity is counted, not narrated
The session SHALL count, without names: `active_s` (10-second slots with input or playback),
`playback_s` and `playback_starts`, `media_imported` by kind (`video`, `audio`, `image`,
`generator` — the kind is derived inside `telemetry.rs` from the media path, the path itself
never leaves the module), `clips_added`, `edits`, `undos`, `redos`, `projects_opened`, `saves`,
`exports` by outcome (`ok`, `failed`, `cancelled`) and `fx_added` by effect kind (the name from
our effect registry, never a user-given name). The counts SHALL be derived inside
`telemetry.rs` from the signals of the one door below — above all from the model COMMANDS the
apply wrappers already hold — not computed at the call sites.
Why: "what was this session spent on" is what the owner asked for; per-action events would
multiply the volume tens of times for the same answer. Deriving the counts from the commands
means one line per apply wrapper instead of a hook in every feature.

#### Scenario: Real UI paths move the counters
- **WHEN** the harness app with a recording telemetry imports a synthetic video and an audio
  file, puts a clip on the timeline, adds a Blur, plays for 2 s of fake time, stops, undoes
  once and saves
- **THEN** the counters read `media_imported {video:1, audio:1}`, `clips_added >= 1`,
  `fx_added {Blur:1}`, `playback_starts == 1`, `playback_s == 2`, `undos == 1`, `saves == 1`,
  `edits >= 3`, `active_s > 0`
- **PATH** import two files into the bin → drag one onto the timeline → add Blur in the event's
  FX window → Space, wait, Space → Ctrl+Z → Ctrl+S; nothing visible changes for the person
- **TEST** t884 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Rare milestones are events
`project_opened` SHALL be sent when a project is opened, imported or recovered, with `how`
(`open`, `import`, `recovery`, `new`), size buckets (`clips`, `tracks`, `media` in
`0`, `1-9`, `10-99`, `100-999`, `1000+`), `duration_bucket` (`<1m`, `1-10m`, `10-60m`, `>60m`)
and flags `has_reels`, `has_morphers`, `has_automation`. `export_finished` SHALL be sent when an
export ends, with `outcome` (`ok`/`failed`/`cancelled`), `container`, `video_codec`,
`audio_codec`, `width`, `height`, `fps`, `duration_bucket`, `backend` (`gpu`/`cpu`) and
`speed_x` (media seconds per wall second, one decimal). No project or file name is sent.
Why: these are rare and each answers a product question by itself (which projects people make,
whether exports succeed and how fast), so they earn their own events.

#### Scenario: Buckets and export properties
- **WHEN** the bucket function gets 0, 1, 9, 10, 999, 1000 and the durations 59 s, 60 s,
  3600 s, 3601 s; and `export_finished` is built from a cancelled GPU export of a 1920×1080
  30 fps H.264/AAC MP4 of 75 s that took 50 s
- **THEN** the buckets are `0`, `1-9`, `1-9`, `10-99`, `100-999`, `1000+` and `<1m`, `1-10m`,
  `10-60m`, `>60m`; the event holds exactly its keys with `outcome == "cancelled"`,
  `backend == "gpu"`, `speed_x == 1.5`
- **TEST** t883 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: A crash is reported at the next start, by place only
When the start finds the previous session's crash report (`journal::take_crash_report`, the
same hand-off that already tells the person), telemetry SHALL send `app_crashed` with
`crashed_build` (the version line of the report), `thread` (the thread name) and `location`
(the panic's `file:line:col` with the path cut to the part after `src/`), and the crashed
session's `crashed_session_id` if the report holds it. The panic message text MUST NOT be sent.
The panic hook SHALL write the session id into the report.
Why: where the program crashes, per version, is the most useful number for fixing it; the
message text often holds the person's paths, the location never does.

#### Scenario: Only the place leaves the machine
- **WHEN** a crash report is produced by the panic hook for
  `panicked at src/apps/kt-app/src/timeline.rs:812:9: cannot open /home/someone/cut.kt` on
  thread `main` in session S, and the next start hands it to telemetry
- **THEN** `app_crashed` holds `location == "apps/kt-app/src/timeline.rs:812:9"`,
  `thread == "main"`, `crashed_session_id == S`, its keys equal the table row, and the JSON
  contains neither `someone` nor `cannot open`; a second start sends no second `app_crashed`
- **TEST** t882 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: The environment is described in coarse terms
The environment properties SHALL be: `$os_version` (Linux: `NAME VERSION_ID` from
`/etc/os-release`; Windows: `Windows <major> (build <n>)`), `cpu_cores`, `ram_gb` (total RAM
rounded to whole GB), `gpu_vendor`, `gpu_name` (the wgpu adapter name), `gpu_type`
(`discrete`/`integrated`/`virtual`/`cpu`/`other`), `gpu_backend` (`vulkan`/`gl`/`dx12`/…),
`software_render`, `screen_scale` (pixels per point, two decimals), `screen_w`/`screen_h`
(the logical size of the first monitor, rounded to 10 points), `ffmpeg_major` and
`ffmpeg_source` (`system`/`folder`/`downloaded`/`missing`). They are read once, after the
first frame (the adapter is known only then), and fail soft: an unreadable value is omitted,
never guessed. The host name, user name, MAC or disk serials MUST NOT be read for telemetry.
Why: "slow on my machine" and "won't open my video" reports become checkable against what the
machines actually are; `ffmpeg_source == missing` on Windows counts people stuck at the first
step.

#### Scenario: The probe reads coarse facts and nothing personal
- **WHEN** the probe runs over an `os-release` sample (`NAME="Ubuntu"`, `VERSION_ID="24.04"`),
  an adapter description (`NVIDIA GeForce RTX 3060`, discrete, Vulkan), 16.4 GB of RAM, 12
  cores, scale 1.25, a 2560×1440 logical monitor and FFmpeg 7 from the system
- **THEN** it yields `$os_version == "Ubuntu 24.04"`, `ram_gb == 16`, `gpu_type ==
  "discrete"`, `gpu_backend == "vulkan"`, `screen_w == 2560`, `ffmpeg_source == "system"`;
  with the `os-release` file missing `$os_version` is absent, not empty; the probe's output
  keys are a subset of the environment row of the table
- **TEST** t886 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: Telemetry enters the application through one door
Code outside `telemetry.rs` SHALL talk to telemetry only through `telemetry::signal(Signal)` —
a free function that is a no-op when telemetry is off or not installed — plus the start-up and
UI plumbing that exists for the switch (`install`, `global`, the notice and hint texts, the View
checkbox). `Signal` is a small enum of FACTS (`Commands(&[Command])`, `Undo`, `Redo`, `Saved`,
`ProjectOpened{how}`, `ExportEnded{..}`, `Frame{playing, had_input}`, `CrashReport(&str)`,
`Exit`); classifying, counting, bucketing, building events and deciding what to send live in
`telemetry.rs` only. Each call site is ONE line, there are at most 16 of them, and the list is
kept by a guard test. `signal` MUST NOT block, perform I/O or allocate on the per-frame path
(`Frame`): it is a branch and atomic adds; other signals may take a short lock on the
counters' maps, never across I/O.
Why: the owner asked (2026-10-28) that telemetry not smear across the code — neat insertions,
handled in one module, without slowing the program. A fixed door plus a counted list of doors
keeps the next feature from adding its own telemetry code where it lives.

#### Scenario: The doors are counted
- **WHEN** the guard scans `src/apps/kt-app/src/**/*.rs` except `telemetry.rs` and the test
  modules for `telemetry::`
- **THEN** every occurrence is `telemetry::signal(`, `telemetry::Signal::`, `telemetry::install(`,
  `telemetry::global()`, a text helper (`notice_*`, `hint_text`, `lists_text`) or a type in the
  checkbox code; the number of `signal(` call sites is at most 16 and equals the number listed in
  the test, so a new site is a deliberate edit of the guard; no file other than `telemetry.rs`
  builds JSON for telemetry or names a PostHog key
- **TEST** t887 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: A local PostHog stand-in checks the whole path
The test suite SHALL carry a local stand-in of PostHog's capture endpoint (an in-process HTTP
server answering `200 {"status":1}` on `POST /i/v0/e/`) that validates each request the way the
ingestion would reject it: JSON body, non-empty `api_key`, `event`, `distinct_id`; `properties`
an object; `$session_id` a UUID v7; `$set`/`$set_once`, when present, objects. A debug build
SHALL send to it when BOTH `KT_TELEMETRY_URL` and `KT_TELEMETRY_KEY` are set (the key variable
is read only by debug builds; a release build ignores both), so a live scenario can drive the
real `kinetren` without the real key and without reaching the internet.
Why: the owner asked for a simple test that the whole thing basically works against a local
PostHog (2026-10-28). A real self-hosted PostHog (ClickHouse, Kafka, Postgres, Redis) is far too
heavy for `cargo test` and the live run; a stand-in that checks what the ingestion checks, plus
the real binary in the live run, covers "it reaches PostHog in the right shape".

#### Scenario: The harness app speaks valid PostHog
- **WHEN** the harness app with an HTTP sink pointed at the stand-in starts, imports a synthetic
  video, plays and stops, and exits
- **THEN** the stand-in accepted, in order, `app_started` (with `$set`), `project_opened` or the
  counters' first heartbeat as applicable, and `app_closed` with `media_imported.video == 1` and
  `playback_starts == 1`; it rejected nothing; all events share one `distinct_id` and one
  `$session_id`
- **TEST** t888 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The real program reaches the stand-in
- **WHEN** the live run starts the real `kinetren` with `KT_TELEMETRY_URL` and `KT_TELEMETRY_KEY`
  pointing at a stand-in, opens a fixture project, plays two seconds, opens the bell and expands
  the telemetry entry, and closes the window
- **THEN** the stand-in holds valid `app_started`, `project_opened` and `app_closed` (counters
  with `playback_starts >= 1`) under one session id; the scenario's screenshots show the entry
  collapsed and expanded
- **PATH** start Kinetren → the bell has one unread entry → click the bell → one calm line with
  "Details" collapsed → click "Details" → the lists of what is and is NOT sent → close the window
- **TEST** live scenario `src/scripts/live/telemetry.py`


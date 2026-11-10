# Telemetry

## Purpose

What Kinetren reports about its own use: how many times it is started, how long a session
lasts, and whether it closed cleanly. The report is anonymous, goes to one PostHog Cloud EU
project, is on by default with a visible off switch, and never costs the person a frame of
UI or a second of exit.

## Requirements

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
- **WHEN** the harness app with a recording telemetry gets a fixture video and an audio file
  dropped onto the window (each lands on the timeline as a clip), adds a Blur in the video
  event's FX window, plays for 2 s of fake time, stops, undoes once and saves
- **THEN** the counters read `media_imported {video:1, audio:1}`, `clips_added >= 1`,
  `fx_added {Blur:1}`, `playback_starts == 1`, `playback_s == 2`, `undos == 1`, `saves == 1`,
  `edits >= 3`, `active_s > 0`
- **PATH** drop a video and an audio file onto the window → both appear on the timeline → open
  the video event's FX window → "+ effect", type "blur", click Blur → the chain shows Blur →
  Esc → Space, the playhead runs; wait; Space, it stops → Ctrl+Z → Ctrl+S, the title loses
  its star; nothing else changes for the person — the counting is invisible
- **TEST** t884 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Rare milestones are events
`project_opened` SHALL be sent when a project is opened, imported or recovered, with `how`
(`open`, `import`, `recovery`, `new`), size buckets (`clips`, `tracks`, `media` in
`0`, `1-9`, `10-99`, `100-999`, `1000+`), `duration_bucket` (`<1m`, `1-10m`, `10-60m`, `>60m`)
and flags `has_reels`, `has_morphers`, `has_automation`. `export_finished` SHALL be sent when an
export ends, with `outcome` (`ok`/`failed`/`cancelled`), `container`, `video_codec`,
`audio_codec`, `width`, `height`, `fps`, `duration_bucket`, `backend` (`gpu`, or `none` when no video device was found and the export failed) and
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
checkbox). `Signal` is a small enum of FACTS (`Commands(&[Command], &model)`, `Command`,
`Gesture` (one edit per gesture), `Undo`, `Redo`, `Saved`, `ProjectOpened(how, &Project)`,
`ExportEnded(&RenderJob)`, `Gpu(&AdapterInfo, software)`, `Frame { ctx, playing }`,
`CrashReport(&Path)`, `Exit`); classifying, counting, bucketing, building events and deciding what to send live in
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
  `telemetry::global()`, a text helper (`notice_*`, `hint_text`), `telemetry::OpenHow` or a
  type in the checkbox code; the number of `signal(` call sites is at most 16 and equals the number listed in
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

### Requirement: The load watch never calls uncounted work rest
A second in which a thread other than the watch's own holds 10 % of a core or more while no work counter
moves SHALL NOT read "at rest": the journal's LOAD record and the report say "NO COUNTER MOVED" and
Diagnostics says that no counter moved. Model edits, project swaps and engine builds SHALL have work
counters (`model_edits`, `project_swaps`, `engine_builds`). The live scenario `idle` SHALL fail a pause
that holds such a thread.
Why: the owner's report read "at rest" at 196 % (its lines 482 and 512) while prefetch and the audio
render rebuilt the engine on every mouse move: the counters did not see that work, and the verdict
believed them. A tool made for proof must not hide what it was made to show.

#### Scenario: A busy thread with no counter
- **WHEN** a second has prefetch at 95 % and the audio render at 92 % of a core and every counter still, and another has only the watch's own tick
- **THEN** the first is not rest and reads "NO COUNTER MOVED", the second is rest
- **TEST** t1880 (`src/apps/kt-app/src/load_watch.rs`)

### Requirement: One local journal, stamped at the moment of the event
The program SHALL keep one journal, `telemetry-journal.log` in the data directory, that never leaves the
machine and that the usage telemetry never reads. Every record SHALL carry the local wall-clock time
with milliseconds and its offset, and the seconds since the process started, both taken when the event
happened by the code that saw it, not when the record is written or read. A record SHALL reach the file
before the next thing happens: one writer thread, blocked while nothing comes, appends each record with
no user-space buffer, so a killed process loses nothing it recorded; a panic flushes it. Past 2 MiB the
journal SHALL rotate into `.1` and `.2`. While nothing happens the journal SHALL cause no wake-up of
any thread. It SHALL hold no media content and no path beyond a file name.
Why: the owner, kkt-26.11.07b G6/G10: "сделал отложенную запись и я не могу тебе сказать точное
время"; "Не догадался сделать общую телеметрию - взять playback journal и втроить в журнал телеметрии,
куда и load-report и playback и действия будут входить"; "телеметрия != трекинг использования в posthog".

#### Scenario: A record carries the time of its event
- **WHEN** an event is recorded at 10:00:00.250 by a fake clock and the clock has moved an hour before the line is written and read
- **THEN** the line says 10:00:00.250 and the monotonic offset of the event
- **TEST** t1960 (`src/apps/kt-app/src/tele_journal.rs`)

#### Scenario: A killed process keeps what it recorded
- **WHEN** a child process records lines, its writer has written them, and the child is killed with SIGKILL
- **THEN** the file holds every line whole, with no explicit flush or close
- **TEST** t1962 (`src/apps/kt-app/src/tele_journal.rs`)

#### Scenario: Rotation bounds the journal
- **WHEN** records pass the rotation size several times
- **THEN** there are at most the journal and `.1`, `.2`, each under the size plus one line, and the newest record is in the journal
- **TEST** t1963 (`src/apps/kt-app/src/tele_journal.rs`)

#### Scenario: At rest the journal costs nothing
- **WHEN** the journal's writer has nothing to write for a second and a half
- **THEN** its thread made no context switch in that time
- **TEST** t1964 (`src/apps/kt-app/src/tele_journal.rs`)

### Requirement: The journal holds the load, the playback, the notes and the person's actions
Into the journal SHALL go: each busy second of the load watch (threads, work counters, "NO COUNTER
MOVED") and the return to rest; every record of the playback black box as it is pushed (play, pause,
seek, loop region, direction, wrap, stall, underrun snapshot), a run of repeats as one record with
its count and the time of its last repeat; a summary at each stop of playback (length, V-fps, new
frames, skipped cells, rollbacks, underruns); the Diagnostics notes (underruns, eviction spikes,
no-read drops, low video hit, offline media) raised by the load watch at the second they happen; every
`kt-app.log` line, notice and export log line; and the person's actions: an edit by command name and
target, undo, redo, a window opened or closed, the effect selected in a chain, a project opened or
saved (by file name), and a mark he sets with Ctrl+Alt+M or the Diagnostics button, with an optional
note. Every existing record of the program SHALL either be folded in or have its reason to stay apart
written in the capability's design.
Why: the owner could not say what he did at "487 s НАВЕРНОЕ"; the black box, the load report and the
Diagnostics log answered three halves of one question.

#### Scenario: An action shows up with its name
- **WHEN** an edit, an undo, a window opened and closed, a playback started and stopped and a mark with a note pass through the app's doors
- **THEN** the journal holds each in order with its name: the command, "undo", the window's title, PLAY and PAUSE with the summary, MARK with the note
- **TEST** t1961 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: The playback black box goes into the journal with its repeats closed
- **WHEN** the flight ring gets a play, a hundred identical wraps and a pause
- **THEN** the sink hands over play, one wrap, the run of a hundred with the time of its last repeat, and pause
- **TEST** t1965 (`src/libs/kt-engine/src/flight.rs`)

#### Scenario: A note is stamped at the second it happened
- **WHEN** the load watch's probe sees two new callback underruns and a no-read drop in one sample, with no UI frame
- **THEN** the journal has NOTE records stamped with that sample's time
- **TEST** t1966 (`src/apps/kt-app/src/load_watch.rs`)

### Requirement: The load report is the journal's last quarter of an hour
`load-report.txt` SHALL be the export of the journal's last 15 minutes, every kind, oldest first, each
record with its clock time, under the machine header. It SHALL be written by itself after three seconds
of a whole core (at most once in five seconds) and by the Diagnostics button; the load watch SHALL write
each busy second into the journal at that second.
Why: the owner's report said "487 s ago" from the moment it was written, so the seconds moved under
him between two reads.

#### Scenario: The report says clock time and holds the actions
- **WHEN** the journal holds a load second, an edit and a mark, and the report is written later
- **THEN** each appears with its own clock time, none says "s ago"
- **TEST** t1967 (`src/apps/kt-app/src/load_watch.rs`)

### Requirement: A busy second records what each GPU pass cost
Every pass of every plan the preview, the offline engine and the export execute, and every dispatch of
the compute host, SHALL be timed on the device by a timestamp before and after it where the adapter
offers `TIMESTAMP_QUERY` (every device the program opens asks for it when the adapter has it, the
preview's eframe device included). The timestamps SHALL come back without a wait: with the frame's own
readback, or with a map a later frame collects. The load watch SHALL take the process-wide sum once a
period and, for a busy second in which anything composed, write one `GPU` record into the telemetry
journal at that second, after its `LOAD` record: the adapter, the frame size and the largest decode size
with the number of sources, the frames by who asked (preview, offline, export), the device time per
frame (the sum of its passes) and the span from the first pass's start to the last pass's end, then each
pass's mean and count named by effect and pass (`vhs_tape:s5-head-read`, `camera_isp:s07-demosaic`,
`composite`, `geometry`, `display`), the largest total first, then the compute host's jobs and
dispatches (`datamosh:search`). Diagnostics SHALL show the same record. Where the adapter has no
timestamps, a frame SHALL be timed from its submit to the device's "done" and the record SHALL say so.
Nothing SHALL be queried or recorded while nothing composes. `kt-cli preview` prints the record of its run.
Why: the owner, kkt-26.11.08 H9: "на моем компьютере всё что могло бы - работало пошустрее...
гораздо.."; his journal showed the UI waiting seconds on the integrated Radeon with no word of what it
ran; found-defects 108: no pass was ever measured on a real card. docs/orchestrator.md: "я не собираюсь
снимать замеры" — a normal run must carry the numbers.

#### Scenario: The passes add up to the frame
- **WHEN** three frames of a plan with VHS Tape over a second layer are read back and one goes through the export's ring
- **THEN** the sum holds four frames (offline 3, export 1) with their size, every tape pass by its name, two composites a frame, and the passes' totals equal the frames' device time to the nanosecond; the line's means times counts equal it within the rounding
- **TEST** t2020 (`src/libs/kt-gpu/tests/pass_time.rs`)

#### Scenario: The journal holds the GPU record of a busy second
- **WHEN** the load watch journals a busy second that carries the device's sum and then one that carries none
- **THEN** the journal holds LOAD, GPU at the same time with the adapter, sizes, frames and each pass's mean and count, then LOAD alone
- **TEST** t2021 (`src/apps/kt-app/src/load_watch.rs`)

#### Scenario: Nothing composes, nothing is recorded
- **WHEN** an engine is open and composes nothing, and again after a taken second
- **THEN** the sum is empty
- **TEST** t2022 (`src/libs/kt-gpu/tests/pass_time.rs`)

### Requirement: A long UI frame names the stage that waited
The journal's note of a UI frame blocked for 100 ms or more SHALL list the frame's stages by their WALL
time (timeline, events, bin, centre, composite, windows; those of a millisecond or more), so a wait —
a lock, a file, the device — is named by its stage even when the thread spent no processor time in
it. The live harness's geometry dump SHALL be written by a thread of its own, so the harness does not
put a file write into the frame it measures.
Why: the owner's journal (kkt-26.11.08 H9) said a frame stood 4.5 s and not where; the per-stage
processor time cannot see a wait. Measuring this change on his project, the note named the timeline
stage, and inside it the harness's own dump: one write stood 10–20 s on a disk busy with the
sidecar's fsyncs.

#### Scenario: A frame whose timeline stage waits
- **WHEN** a frame's timeline stage waits 5 ms without work and its centre stage takes under a millisecond
- **THEN** the frame's wall breakdown names the timeline with at least 5 ms and names neither the centre nor the frame itself
- **TEST** t1846 (`src/apps/kt-app/src/ui_wait_tests.rs`)

### Requirement: One structured trace of the run, typed and joined by ids
The program SHALL record a structured trace beside the text journal: typed fixed-size records (an event,
the time since the process started in nanoseconds, a duration for a span, an id, a cause and the
event's numbers), the names they use written once per file, in bounded ring files under
`<data>/trace` (segments of 8 MiB, all runs within 128 MiB, each segment readable alone). A preview
frame's id SHALL ride with every request it makes: its decode requests, its stream stage's run and its
compute jobs carry it as their cause, and a cache key's id is the same number in the frame's wait, the
request, the decoded entry and the eviction (whose cause is the key that pushed it out). Every
subsystem that waits or keeps SHALL record: the preview frame's life (asked, superseded, planned,
waiting — decode, lock, device —, executed with its kept-composite hit or store, its layers, done,
shown or dropped and why), the "Loading" scrim, playback state changes, the decode queue, cache
evictions, stream stages and their memo, the compute host, the executor's kept composites and pipeline
warm-ups, the audio render and the kept mix's decision, prefetch passes, jobs (thumbnails, effect
thumbnails, reel previews, posters, analysis, export), UI frames of 50 ms or more, edits and what they
invalidated, the load watch's busy seconds; and the machine: processor, cores, memory, OS, and for each
device a role opens its adapter, driver, features and limits. Recording SHALL never wait: a lock-free
ring, no allocation after a thread's first record, a full ring drops and the writer says how many; the
audio callback records nothing. At rest the trace SHALL cause no wake-up. It never leaves the machine.
Why: the owner, kkt-26.11.09 I6/I7: "у тебя в логах срач, очень не структурированные логи";
"телеметрия должна быть такая, чтобы ты мог понять что у тебя происходит, без мышиной возни туда-сюда
версиями или что ты там за бред мне прелагаешь с стендом".

#### Scenario: Records come back whole, in time order, threads and names resolved
- **WHEN** three named threads record requests with a frame as their cause, a span with an interned kernel name, and a journal line
- **THEN** the files read back every record with its fields, in time order, the threads and the name resolved, the span with its length, the journal line and the machine's facts present
- **TEST** t2140 (`src/libs/kt-time/src/trace/tests.rs`)

#### Scenario: The ring files are bounded and each reads alone
- **WHEN** records pass the segment size many times under a small budget
- **THEN** there are several segments within the budget plus one segment, each names its threads and names, and the newest record is kept
- **TEST** t2141 (`src/libs/kt-time/src/trace/tests.rs`)

#### Scenario: At rest the writer sleeps
- **WHEN** nothing is recorded for a second and a half
- **THEN** the writer thread made no context switch; one record then wakes it and is written
- **TEST** t2142 (`src/libs/kt-time/src/trace/tests.rs`)

#### Scenario: Recording never waits and never allocates
- **WHEN** the writer is held and more records are made than the ring holds; and, in a binary of its own, ten thousand records are made after a thread's first
- **THEN** recording returns at once, the lost records are counted and the writer records how many; the records made no allocation
- **TEST** t2143 (`src/libs/kt-time/src/trace/tests.rs`, `src/libs/kt-time/tests/trace_alloc.rs`)

#### Scenario: A frame is joined to the decode it waited for
- **WHEN** the preview asks for a frame whose layer is not decoded, through the real player on model decoders
- **THEN** the frame's wait names a key, the request of that key names the frame as its cause and went out from the compose thread, the media worker decoded the key on its own thread, and the plan, the executor and the answer name the frame
- **TEST** t2144 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: The trace answers questions through a query tool
`kt-cli trace [<dir|file>] [--run N] <question>` SHALL answer from one run's trace, with times in the
journal's seconds since the start: `summary`; `loading [from to]` — each stretch the preview waited for
a picture (the UI's scrim records, else a tick asked until a complete frame of it, 180 ms or more), its
waits ranked by time (the decodes waited for and whether they were evicted again, the stream history,
compute by kernel with device time, the stream memo's evictions, plans, the executor, locks, the device,
warm-ups, builds, prefetch passes), the critical path of its last composite and the journal around it;
`loop` — per lap of a loop region the frames kept and composed, the kept mix's replays, and for every
frame composed again on a later lap why (evicted, its key changed and which layer, a held stream frame,
the slow device, incomplete, never kept); `during <from> <to>`; `slow [n]` with each frame's critical
path; `frame <id>`; `machine`; `dump` as JSON lines; `chrome <out.json>` for Perfetto.
Why: the owner asked for telemetry the orchestrator reads to understand a run; a record nobody can
query is the ASCII log again.

#### Scenario: Why the preview was Loading
- **WHEN** a paused frame waits two seconds: three composites, a decode, a history of 60 positions computed by the prefetch thread with 60 compute jobs
- **THEN** one stretch with the three composites, the stream history ranked first, the compute kernel and the decode next, and the last composite's critical path; with the UI's scrim records, the scrim decides the stretch
- **TEST** t2146 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: Pass by pass over a loop
- **WHEN** three laps over three moments: kept on the first, one evicted and one whose key moved on the second, all kept on the third; the kept mix replays once and misses once in the lap's context
- **THEN** the laps' counts, "evicted from the kept composites", "the frame's key changed", "lap context never kept", and the third lap all kept
- **TEST** t2147 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: The slowest frames carry their critical path
- **WHEN** the slow question is asked of a run whose slowest frame ran three compute jobs
- **THEN** that frame comes first with its length, its stream stage, its compute and its executor lines
- **TEST** t2148 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: A window, a frame, and the exports
- **WHEN** a window, one frame, a dump and a Chrome export are asked
- **THEN** the window names the busy threads and the jobs, the frame tells its decode's story, every dump line is JSON, and the Chrome export parses with its threads named and spans as complete events
- **TEST** t2149 (`src/apps/kt-cli/src/trace_query/tests.rs`)

### Requirement: The trace says what the scheduler ran, paused, cancelled, and what a must-not-wait thread waited for
Every scheduled job SHALL be recorded when it is posted (its class, its lane, who posted it), when
it runs (a span with its time in the queue and its outcome; the job's own records carry it as their
cause) and when it is cancelled (the reason); every pause and resume of the background classes
SHALL be recorded with its reasons and the jobs waiting. The UI thread and the audio render thread
SHALL count every blocking call they make — a file open, an fsync, a thread join, a renderer build,
a sidecar open — and record it with its length. `kt-cli trace during` SHALL name them: a job by its
class and lane, a pause, a resume, a cancel by its lane, a blocking call by its thread and kind.
Why: `docs/engine-audit.md` M3/M10: nothing knew what background work ran, and nothing stopped the
next panel from calling a blocking API on the UI thread — "no type, lint or counter".

#### Scenario: Every job and pause is traced
- **WHEN** a job runs and records something, a queued job is cancelled, and a vote goes up and down
- **THEN** its post, run (done) and its record with it as the cause are in the trace, the cancel names its reason, the pause and the resume are there
- **TEST** t2234 (`src/libs/kt-time/src/sched/tests.rs`)

#### Scenario: Blocking calls are counted on the marked threads only
- **WHEN** an unmarked thread and the UI thread each make blocking calls, and another thread makes one
- **THEN** only the UI thread's are counted, by kind
- **TEST** t2235 (`src/libs/kt-time/src/block.rs`)

#### Scenario: The query names them
- **WHEN** a window holds a visible thumbnail job, a pause, a cancelled analysis job, a resume and an fsync on the UI thread
- **THEN** `during` lists each by class and lane, the pause and the resume, the cancel by lane, and the UI thread's fsync
- **TEST** t2243 (`src/apps/kt-cli/src/trace_query/tests.rs`)

### Requirement: At rest nothing wakes on a clock
No thread of the program SHALL wake on a timer at rest. The scheduler's workers park until a post,
a resume or an exit; the load watch parks after five quiet seconds until a work counter moves (a UI
frame included; the bench keeps sampling); telemetry's sender sleeps on its channel once a heartbeat
finds nothing new, until the next counted activity; the media watch looks at the files on the
opening, a focus change and play or stop, never on a clock and never on a pointer move. A pointer
over a still place SHALL start no job. On a settled pause of the heavy showcase none of the
engine's, the scheduler's or the services' threads makes a context switch.
Why: the owner, 2026-11-07: "ПОЧЕМУ АГЕНТАМ НЕ ПРИКАЗЫВАЕТСЯ ТЕСТИРОВАТЬ ЭТО?!?"; `docs/engine-audit.md`
§2b counted seven timers at rest (rows 122, 174).

#### Scenario: The pool sleeps at rest
- **WHEN** nothing is posted for a second and a half
- **THEN** no worker of the pool makes a context switch
- **TEST** t2233 (`src/libs/kt-time/src/sched/tests.rs`)

#### Scenario: A parked watcher wakes on work only
- **WHEN** a watcher parks until work moves and nothing is counted for 300 ms, then one count
- **THEN** it stays parked, then wakes
- **TEST** t2241 (`src/libs/kt-time/src/work.rs`)

#### Scenario: A settled pause wakes none of our threads
- **WHEN** the heavy showcase project has gone quiet after opening and stands paused for five seconds
- **THEN** the work counters stand still, the trace grows by nothing, and none of the engine's, the scheduler's or the services' threads made a context switch
- **TEST** live `idle` (`src/scripts/live/idle.py`)

#### Scenario: A quiet session's telemetry sleeps
- **WHEN** a heartbeat carried one activity and the next found nothing new; then another activity
- **THEN** the sender sleeps and sends nothing; the activity wakes it and the next beat carries it
- **TEST** t2242 (`src/apps/kt-app/src/telemetry.rs`)

### Requirement: Every ring decision is in the trace
Every admission to a ring SHALL be recorded with its class, the device time it predicted, the time it
waited and the ring; every cut piece and every stopped stream history SHALL be recorded too, with the
frame or job they belong to as the cause. `kt-cli trace gpu` SHALL report, per ring and per class,
submissions and compute jobs per second, admission waits and pieces, and the display's present
latency.
Why: `docs/engine-audit.md` §2b — "every job is counted per class"; row 134 asked whether the
look-ahead's Datamosh submissions fell, which only a count by class answers.

#### Scenario: The query reports the ring by class
- **WHEN** a window holds admissions of classes now and near on one ring, a cut frame of three pieces, a stopped history and three display presents
- **THEN** `gpu` lists per class the admissions, the waits and the submissions per second, the pieces of the cut frame, the stop, and the presents' latency
- **TEST** t2263 (`src/apps/kt-cli/src/trace_query/tests.rs`)

### Requirement: The trace names a stream schedule's compile and the memo's sizing
The structured trace SHALL record each compile of a stream kernel's schedule (`stream_plan`: the
positions compiled and the positions held after it) and each sizing of the preview's stream memo
(`stream_memo_cap`: the bound, the loop's positions, the memory available then).

Why: the owner's run must say when a schedule was compiled and why the memo held what it held
(structured-telemetry; stream-layout-per-build).

#### Scenario: A compile is a record
- **WHEN** a stream layout with a cell is asked about a position, then about an earlier one, then about a later one
- **THEN** the trace holds two `stream_plan` records, the second starting where the first ended; the earlier position compiled nothing
- **TEST** t2277 (`src/plugins/fx-video-codec/tests/schedule.rs`)

#### Scenario: A sizing is a record
- **WHEN** the preview's memo is sized for a loop
- **THEN** one `stream_memo_cap` record carries the bound and the loop's positions
- **TEST** t2275 (`src/libs/kt-engine/tests/stream_memo.rs`)

### Requirement: The trace names the memory ledger's budget and every grant
The structured trace SHALL record each sizing of the memory ledger (`mem_budget`: the growth budget,
what is granted, the memory available then, whether an export claims it) and each grant that changed
(`mem_grant`: the tenant, its want, its new cap, what it holds). An unchanged sizing SHALL write
nothing. `kt-cli trace memory` SHALL print them in order, and each tenant's last cap and bytes.
Why: the owner's run must say why a loop was or was not kept whole — which cache held what and how much
the machine allowed (memory-ledger).

#### Scenario: A sizing is a record and the query reads it
- **WHEN** a trace holds two ledger sizings, the second taking back a stream grant, and a sizing that changed nothing
- **THEN** it holds two `mem_budget` records and the changed `mem_grant` records only; `kt-cli trace memory` prints both sizings and the stream tenant's last cap
- **TEST** t2300 (`src/apps/kt-cli/src/trace_query.rs`)

### Requirement: A long frame says how long it waited for a processor
A UI frame of 100 ms or more SHALL be noted with how much of it the UI thread stood ready with no
processor (Linux: the run-queue wait of `/proc/thread-self/schedstat`), beside its stages' wall time. A
check of "no frame was blocked" SHALL judge the frame's wall time less that wait.
Why: at load 142 the live `ui_wait` saw a frame of 410 ms (centre 408 ms) whose work was 10 ms: the
machine's, not a lock's; nothing in the note told the two apart.

#### Scenario: The run-queue wait is read apart from a sleep
- **WHEN** the thread sleeps 50 ms; then spins with four times as many spinning threads as cores
- **THEN** the sleep adds under 50 ms of run-queue wait; the contention adds some
- **TEST** t2215 (`src/apps/kt-app/src/ui_wait_tests.rs`); live `ui_wait`

### Requirement: A recheck of kept moments asks the interface for no frame
While kept moments presented before an edit are rechecked against the new plan, the timeline SHALL ask
for no frame of its own; the preview's thread wakes the UI after each pass of the check.
Why: the timeline asked for the next frame on every frame while the check was pending, and on a
loaded machine the program ran ~150 frames a second "at rest" for seconds (live `edit_drag`,
`cached_range`, `idle`; found-defects 133, 170).

#### Scenario: A pending recheck
- **WHEN** kept moments are pending a recheck and two frames run
- **THEN** none of the repaint causes of the second frame comes from the timeline
- **TEST** t2217 (`src/apps/kt-app/src/ui_wait_tests.rs`); live `edit_drag`, `cached_range`, `idle`

### Requirement: The trace carries the process's memory and the frames cache's sizing
The structured trace SHALL record each sizing of the preview's decoded-frame cache (`frames_cap`: its
bound, the look-ahead window's bytes, the loop's bytes, the preview quality), and, once per busy second,
the process's resident memory with the ledger's sum and the memory available (`mem_sample`) and each
tenant's bytes and bound (`mem_used`) — only what moved by a megabyte since the last sample. A second at
rest SHALL write none of them. `kt-cli trace memory` SHALL print the sizings, each tenant's bytes at the
last busy second, the process's peak with the caches' part and the rest, and the frames cache's
evictions a second.
Why: found-defects 397 — the owner's 4.7 GB had no record: the ledger accounted 1.4–3.7 GB and nothing
said what the rest was or when it grew (kkt-26.11.10 J4).

#### Scenario: A busy second writes what moved, a still one nothing
- **WHEN** a sample follows a change of the frames' bytes, then a sample with nothing moved, then one after the frames moved again
- **THEN** the first writes the process's record and every tenant's; the second writes nothing; the third writes the process's record and the frames' alone
- **TEST** t2395 (`src/libs/kt-time/src/mem.rs`)

#### Scenario: The query names the frames cap and the whole process
- **WHEN** a run holds a frames cache sizing, two process samples, a tenant's bytes and evictions over two seconds
- **THEN** `trace memory` prints the sizing's bound, window and loop, the tenant's bytes against its bound, the process's peak with the caches' part and the rest, and the evictions with the worst second
- **TEST** t2396 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: The process's resident memory is read
- **WHEN** the process touches 64 MB more
- **THEN** its resident memory, read on Linux and Windows, grows by at least 48 MB
- **TEST** t2399 (`src/libs/kt-gpu/src/memory.rs`)

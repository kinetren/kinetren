## ADDED Requirements

### Requirement: One build per project version, shared by every consumer
A project version (or the same version with new source facts) SHALL be built once, as one audio and
one video renderer, by the player's `engine-build` thread, and published. The audio render thread,
prefetch, the probe (the meters on a pause, the loop region's pinned head) and the preview composer
SHALL all read that build; none SHALL build its own. Only the newest version handed over is built;
the ones in between are never built.
Why: one edit was built 2 audio + 2–3 video times (the UI's video build, the `audio-build` thread,
prefetch's pair, the probe; found-defects rows 110, 111, 150): on the owner's 1974 events a slider
step cost ~197 ms of CPU and 5 renderer builds (release, `kt-cli edit-cost`).

#### Scenario: Five edits while playing
- **WHEN** a playing player of 400 events takes five edits, and after each the meters are read, the loop region is set, and the probe and prefetch take the new build
- **THEN** ten renderers are built in the process, five audio and five video
- **TEST** t2000 (`src/libs/kt-engine/tests/shared_build.rs`)

### Requirement: Nothing on the UI thread builds or waits for a build
Handing a project to the player (`set_project` with the model's shared `Arc<Project>`), a meter
reading, a loop region change, the grid and coverage questions and a preview ask SHALL build no
renderer, derive no event and resolve no reel on the calling thread, and SHALL return while the
builder is held. The opening's first build is the one exception (the player's constructor). The app
SHALL hand the model's shared snapshot of the state (`Model::doc_arc`), not a copy.
Why: invariant 12 and the owner's rule "no interface action waits for a picture" (2026-11-07). The
UI thread spent 88 ms (p50) per slider step on the owner's project: a video build and the generator
sources, which resolve every reel.

#### Scenario: The UI's calls while the build is held
- **WHEN** the builder is held and ten edits are handed over, each followed by a meter reading, a region change, the grid, the decode qualities, the reel coverage and a preview ask
- **THEN** the calling thread counts no renderer build, no derived event and no reel pass, the held build has not landed, and once released the build of the newest version lands
- **TEST** t2001 (`src/libs/kt-engine/tests/shared_build.rs`)

#### Scenario: Edits through the app's doors
- **WHEN** five steps of an effect's parameter go through `apply_coalesced`, a frame drawn after each
- **THEN** each reaches the player, the UI thread builds, derives and resolves nothing, and the player holds the model's snapshot itself
- **PATH** an event with VHS Tape, its effects window open → step the carrier-to-noise field five times: the value moves each time, the picture follows, the window answers at once
- **TEST** t2008 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: A slider dragged in the effects window while a big project plays
- **WHEN** the heavy showcase plays and the VHS Tape dark clip field of the composition's effects window is dragged to and fro four times
- **THEN** there is no callback underrun, at most one shared build (two renderers) is built per edit, and a UI frame of the drag costs the UI thread within twice a hovering frame plus 10 ms
- **PATH** the heavy showcase open → right click the composition → "Effects…" → click the VHS Tape slot: the deck face and its grid → scroll to "Dark clip" → Space: the picture plays → drag the field left and right four times: the value follows the pointer, the picture and the sound go on → Space: stopped, nothing runs
- **TEST** t2007 live scenario `src/scripts/live/edit_drag.py`

### Requirement: A loop region change is not an edit
A change of the loop region SHALL build nothing and move no project version: the region's head is
pinned with the renderer already built, and pinned again by the builder when a new version lands.
Why: found-defects row 117 — a region change built a whole audio renderer on the UI thread (412 ms
on the owner's project, ~12 ms after builder-cost-guard).

#### Scenario: Twenty region changes
- **WHEN** the loop region changes twenty times and is cleared
- **THEN** no renderer is built and the project version stands
- **TEST** t2002 (`src/libs/kt-engine/tests/shared_build.rs`)

### Requirement: A frame is composed by the renderer of its state
A preview composite (and the recheck of the kept moments) SHALL use the build of the newest state
handed over; when that build has not landed, the compose thread SHALL wait for it, never the UI
thread. The UI's readers of the renderer (the grid, the coverage bars) read the last published build,
and the bars' cache is keyed by it.
Why: a composite of the old state under the new version would be shown as the edit's picture, and the
bars would mark a moment "plays without work" by the old plan's key.

#### Scenario: A new frame rate while its build is held
- **WHEN** the builder is held, the project's frame rate changes from 25 to 30 and a frame at 5000 ticks is asked
- **THEN** the ask returns at once, nothing is composed while the build is held, and after the release the frame is composed on the 30 fps grid (3000 ticks)
- **TEST** t2006 (`src/libs/kt-engine/tests/shared_build.rs`)

## MODIFIED Requirements

### Requirement: The UI's own audio renderer is built once per project version
The renderer the UI thread uses (the levels under the cursor on a pause, the pinned head of the loop
region) SHALL be the player's shared build of the version (see "One build per project version, shared
by every consumer"); region changes and meter readings SHALL build nothing, and a new version SHALL be
built once for every consumer.
Why: each change of the loop region built a whole audio renderer on the UI thread, 447 ms on the owner's
1974 events, and the first meter reading after an edit built another.

#### Scenario: Region changes and meter readings on one version
- **WHEN** the loop region changes five times and the level is read five times on one project version, then the project is swapped and the region changes twice
- **THEN** nothing is built for the first five, and the new version is built once (an audio and a video renderer)
- **TEST** t1875 (`src/libs/kt-engine/tests/probe_renderer.rs`)

### Requirement: The UI's own audio renderer of a new version is built off the UI thread
After the first, the renderer the UI thread uses (the meters on a pause, the loop's pinned head) SHALL
come from the player's builder thread, once per version, and its FX states SHALL be synced there (only
the events whose fingerprint changed); the one before SHALL answer meanwhile, and the loop region's head
SHALL be pinned again when the new one lands.
Why: the build takes 385–447 ms on the owner's project (found-defects 111, 117), and these are asked
on the UI thread by every meter reading and every change of the loop region after an edit.

#### Scenario: The meters and the loop region while the build is held
- **WHEN** after an edit the builder is held at its start, and the loop region changes three times and the meters read three times
- **THEN** every call returns, the held build has not landed, and once released the probe of the new version lands
- **TEST** t1844 (`src/libs/kt-engine/tests/probe_async.rs`)

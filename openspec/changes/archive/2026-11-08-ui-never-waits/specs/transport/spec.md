## ADDED Requirements

### Requirement: No interface action waits for a picture
The preview SHALL compose on a thread of its own: the UI thread asks for a frame and goes on, the
newest ask replaces a waiting one, one composite runs at a time, and a frame composed for a project
state the model has since left SHALL NOT land. The UI frame SHALL wait for an answer at most a few
milliseconds, and without one it shows the last picture. A chain whose executor time passes
`SLOW_CHAIN` SHALL compose on the offline device and come back as bytes, so the display's queue never
stands behind it; the first frame of each new project state SHALL be timed there; the plan's own time
(a stream stage's kernel on the compute host's device) SHALL NOT count. Undo, every window and the
transport SHALL answer while a composite takes seconds.
Why: the owner, kkt-26.11.07 E8: an anomalous Datamosh parameter lagged the whole interface until "Я
просто БУКВАЛЬНО НЕ МОГУ ОТМЕНИТЬ СВОЙ ЖЕ ЭФФЕКТ". His release journal (kkt-26.11.08 H9): UI frames of
0.3–4.5 s during playback at a quarter of a core. The composite ran in the UI frame, and a stream stage
decodes the event's history inside the plan, one compute job a frame, each waited for.

#### Scenario: A stuck composite holds nobody
- **WHEN** the executor's composite stands until the test lets it go, and meanwhile the UI asks for a frame, the project is swapped, four more frames are asked and the renderer is queried
- **THEN** every call returns; after the release only the newest ask is answered, and two composites ran, not five
- **TEST** t1840 (`src/libs/kt-engine/tests/compose_queue.rs`)

#### Scenario: A finished frame wakes the UI
- **WHEN** a frame is asked and the executor answers
- **THEN** the wake the UI set is called once and the frame is taken without waiting
- **TEST** t1840b (`src/libs/kt-engine/tests/compose_queue.rs`)

#### Scenario: A slow chain stays off the display's device
- **WHEN** a new project state is composed, then timed fast, then an edit is timed slow twice, then fast
- **THEN** the new state goes to the offline device first, the fast chain to the display, the slow one stays off it until it is fast again
- **TEST** t1841 (`src/libs/kt-engine/src/compose_queue.rs`)

#### Scenario: Undo acts while the kernel is stuck
- **WHEN** an effect is added while the executor's composite never ends, five frames run, and Undo is called
- **THEN** the frames return, Undo removes the effect at the next frame, the frame of the state Undo left is asked for, and its answer lands once the executor is released
- **PATH** add an effect whose frame never finishes → the picture stays as it was; press Ctrl+Z → the effect is gone at once, the picture follows when it is ready
- **TEST** t1842 (`src/apps/kt-app/src/ui_wait_tests.rs`)

### Requirement: The UI's own audio renderer of a new version is built off the UI thread
After the first, the renderer the UI thread uses (the meters on a pause, the loop's pinned head) SHALL
be built for a new project version on a thread of its own, once per version; the one before SHALL
answer meanwhile, and the loop region's head SHALL be pinned again when the new one lands.
Why: the build takes 385–447 ms on the owner's project (found-defects 111, 117), and these are asked
on the UI thread by every meter reading and every change of the loop region after an edit.

#### Scenario: The meters and the loop region while the build is held
- **WHEN** after an edit the probe's builder is held at its start, and the loop region changes three times and the meters read three times
- **THEN** every call returns, the held build has not landed, and once released the probe of the new version lands
- **TEST** t1844 (`src/libs/kt-engine/tests/probe_async.rs`)

### Requirement: The UI thread waits for no lock held through long work
No lock the UI thread takes SHALL be held by another thread through an engine build, a decoder's
join, a file's opening or the executor's composite: the project lock is released before the audio
render thread builds, the source facts are cloned out before the prefetch thread builds, a decoder is
joined after the workers' write guard is gone, the thumbnail worker opens a file with the media map
free, and the display target is published apart from the engine's lock. The render window SHALL find
the largest source on a thread, keyed by the files.
Why: a Rust guard taken inside an expression lives to the end of the statement, so
`AudioRenderer::new(&project.lock().unwrap().clone())` held the project lock through a 412 ms build,
and the UI's next edit (`set_project`) waited for it; the same form held the workers' map through a
decoder's join and the thumbnail map through a file's opening.

#### Scenario: The display target while the engine is held
- **WHEN** a frame is presented, then the engine is held as a composite holds it, and the display target and the glow are asked
- **THEN** both answer at once with the presented target
- **TEST** t1845 (`src/libs/kt-gpu/tests/preview_display.rs`)

## MODIFIED Requirements

### Requirement: The UI's own audio renderer is built once per project version
The renderer the UI thread uses (the levels under the cursor on a pause, the pinned head of the loop
region) SHALL be built once per project version and kept until the next; a later version's is built on
a thread of its own (see "The UI's own audio renderer of a new version is built off the UI thread").
Why: each change of the loop region built a whole audio renderer on the UI thread, 447 ms on the owner's
1974 events, and the first meter reading after an edit built another.

#### Scenario: Region changes and meter readings on one version
- **WHEN** the loop region changes five times and the level is read five times on one project version, then the project is swapped and the region changes twice
- **THEN** one renderer is built for the first five and one for the next two, once that build lands
- **TEST** t1875 (`src/libs/kt-engine/tests/probe_renderer.rs`)

## ADDED Requirements

### Requirement: Opening a project makes no blocking call on the UI thread
Opening a project — File › Open, a recent card or start tile, a dropped project, File › Import
older…, the live rig's open — SHALL read the project file and take its lock on the scheduler's
media lane, and SHALL show the new project's window and timeline as soon as the file is read: its
sources SHALL open on the media lane, the player's first version SHALL be built off the UI thread,
and the project being closed SHALL be released (its threads and decoders joined) off the UI thread.
The preview SHALL say Loading until the new project's first frame is composed; the missing-file
notices and the media lengths SHALL land when the sources answer. From the first frame of the new
project, edits, Undo and Save SHALL act on it: an edit made during the load is built after the
load's version, and a source changed during the load is swapped after its first open. Play pressed
during the load SHALL start only when the version and its sound are there (time stands before the
first frame). On these gestures the UI thread SHALL make no blocking call.
Why: `found-defects.md` row 270 — the new app's constructor opened every source (FFmpeg probe and
sidecar) and built the player's first version on the UI thread, and the closed project's player
joined its decoders there; on the heavy showcase the window stood for seconds. The owner: "No
interface action waits".

#### Scenario: Open and replace block nothing
- **WHEN** a saved project with real media files is opened through the menu's path from a harness app that already holds a project with media
- **THEN** the new project's model is in the window, its sources and lengths land, the old player is released, and the UI thread counted no blocking call; one inline source open turns it red
- **TEST** t2330 (`src/apps/kt-app/src/ui_wait_tests.rs`)

#### Scenario: The player's first version is built off its caller
- **WHEN** a player is made for a project of many events on a thread marked as the UI thread, and Play is pressed before its build lands
- **THEN** the marked thread counted no build, the playhead stands until the version is published, and the first sound after it is the project's
- **TEST** t2331 (`src/libs/kt-engine/tests/player_open.rs`)

#### Scenario: An edit and an Undo during the load land on the loaded project
- **WHEN** a project is opened and, before its sources answer, an event is moved, the move undone and redone, a media file's path changed, and the project saved
- **THEN** the player's last version is the model's, the changed media plays its new file, the file on disk holds the moved event, and no unsaved mark remains
- **TEST** t2332 (`src/apps/kt-app/src/ui_wait_tests.rs`)

#### Scenario: The heavy showcase opens with the window answering
- **PATH** File › Open… → the heavy showcase (`kt-cli showcase --heavy`) → at once the timeline shows its events and the preview says Loading; the menu answers a click while it loads → the preview shows the picture
- **WHEN** the live scenario opens the heavy showcase and clicks the menu during the load
- **THEN** the menu opens within a frame of the click, the UI thread's blocking counter in the dump stays at zero, and the preview's picture arrives
- **TEST** live `open_heavy` (`src/scripts/live/open_heavy.py`)

### Requirement: A save on a busy disk blocks nothing
Ctrl+S while another writer saturates the same disk SHALL leave the UI thread's blocking counter at
zero and its frames under the journal's long-frame threshold: the write and both fsyncs stand on
the save lane, and the asterisk clears when the write lands.
Why: `found-defects.md` row 252 — `formula` under a full live run held a UI frame 2.0–3.6 s per
Ctrl+S on a base before the save lane; proved again on the current tree.

#### Scenario: Ctrl+S under a disk writer
- **WHEN** a background thread writes and fsyncs a large file in a loop in the project's directory while Ctrl+S is pressed in a harness app with zero answer grace
- **THEN** the save call returns without waiting, the UI thread counted no blocking call, and a later frame takes the landed write and clears the asterisk
- **TEST** t2333 (`src/apps/kt-app/src/ui_wait_tests.rs`)

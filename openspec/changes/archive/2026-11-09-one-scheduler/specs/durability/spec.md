## ADDED Requirements

### Requirement: The person's media and save gestures make no blocking call on the UI thread
Ctrl+S SHALL write the project — its text, the write and both fsyncs — on the scheduler's save lane;
the UI SHALL take the outcome within a few milliseconds or in a later frame, and the unsaved mark
SHALL clear only if no edit came while the file was written; a save in flight is finished before
the program exits. A file brought onto the timeline, a card placed from the bin, Undo and Redo of a
media's arrival and a file that came back SHALL open their file, swap or release a decoder on the
media lane, and the gesture lands when it answers. Autosave writes on a background lane that waits
out playback. On these gestures the UI thread SHALL make no blocking call.
Why: `found-defects.md` row 98 (Ctrl+S stood 6.7 s on a busy disk) and row 148 (the UI thread opened
files and joined a decoder on media gestures); the owner: "No interface action waits".

#### Scenario: Media and save gestures block nothing
- **WHEN** a real file is brought onto the timeline, placed again from the bin, its arrival undone twice and redone twice, and the project saved with Ctrl+S
- **THEN** every gesture lands (the clips, the media, the file on disk, no unsaved mark) and the UI thread counted no blocking call; one inline open on the UI thread turns it red
- **TEST** t2236 (`src/apps/kt-app/src/ui_wait_tests.rs`)

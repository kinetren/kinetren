## Purpose

The text of the edit: words recognized locally from a media file's sound, kept in material time so no
edit desyncs them, and a panel to walk, search, select and cut the edit by its text.

## ADDED Requirements

### Requirement: Words are kept in material time
A transcript SHALL belong to a media file and store each word's start and end as material ticks; an
event SHALL show the words of its material window, mapped through its speed and reverse; a split, trim,
move, reverse or speed change SHALL change no stored word. Editing a word's text SHALL keep its times.
Why: the rule of automation keys (R.33) — the representation, not the command, keeps the sync.

#### Scenario: Edits never desync the words
- **WHEN** an event with a transcript is split, its left edge trimmed by 1.3 s, moved, reversed and slowed to 0.5
- **THEN** the stored words are unchanged; each word shown lies under the same material moment it was recognized at (checked against the material sample index ±1 ms); the halves of the split show complementary words
- **TEST** t3460 (`src/libs/kt-model/tests/transcript.rs`)

### Requirement: Recognition runs locally as a downloaded component
Recognition SHALL run only on the person's machine, through a component downloaded on a click and
pinned by version and checksum, as an offline job with its own source at background priority, paused
while playing and cancellable. Re-running it SHALL not replace hand-edited words without confirmation.
No sound or text SHALL leave the machine.
Why: the owner (2026-11-01): local, downloaded on a click, nothing to a cloud.

#### Scenario: A known phrase is recognized with word times
- **WHEN** the recognizer with a pinned tiny model transcribes a fixture of a synthetic voice saying a known sentence with a 1 s pause
- **THEN** the words match the sentence, each word's time lies within 150 ms of the fixture's known word boundaries, the pause lies between two words; no network call is made by the job
- **TEST** t3461 (`src/libs/kt-media/tests/transcribe.rs`)

#### Scenario: Missing component, cancel and pause
- **WHEN** recognition is asked without the component, then with it and cancelled, then running when Play is pressed
- **THEN** the download card opens instead of a job; the cancelled job leaves the transcript unchanged; the running job makes no progress while playing
- **TEST** t3462 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: The Text panel walks, searches and cuts the edit
The Text panel SHALL show the words of the chosen audio track's events in timeline order; the word under
the playhead SHALL be lit and kept in view while playing; a click on a word SHALL move the playhead to
it; a search SHALL count and step through hits and mark them on the ruler; a drag over words SHALL set
the region to their span; Delete with words selected SHALL cut that span with ripple in one undo step.
At rest the panel SHALL cost no work.
Why: B5 — "ходить по тексту, навигация, поиск"; one ripple rule (change `ripple-and-trim-modes`).

#### Scenario: Walk, search and cut by text
- **WHEN** the panel shows an interview of three events; a word is clicked; "budget" is searched and Enter pressed twice; three words are dragged over and Delete pressed; Undo
- **THEN** the playhead jumps to the word; the counter shows hits, the second Enter moves to the second hit and the ruler shows all hits; the region equals the three words' span; Delete removes it and closes the gap, the panel no longer shows the words; Undo restores both
- **PATH** click "Text" beside the bin → the interview text → click a word → playhead jumps → type "budget" in the search → "1 / 3", ticks on the ruler → Enter → "2 / 3" → drag over three words → a region on the timeline → Delete → the words and the gap are gone → Ctrl+Z → back
- **TEST** t3463 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `text_panel` (t3464)

#### Scenario: A long text at rest
- **WHEN** the live `idle` scene opens the panel on a 6 000-word transcript, pauses and moves the mouse over the panel
- **THEN** no work counter moves but the hover frames; the panel's layout is not rebuilt per frame
- **TEST** t3465 (`src/scripts/live/idle.py`)

### Requirement: Fillers are marked, never cut by themselves
Marking fillers SHALL put range markers on words of the language's filler list and SHALL NOT change an event. Pauses are
capability `material-analysis` (change `pause-detection`); the Text panel SHALL open its window.
Why: rule 20.

#### Scenario: Filler markers
- **WHEN** fillers are marked over a transcript where two of twenty words are on the language's list
- **THEN** exactly two range markers cover those words, the events are unchanged, one Undo removes both
- **TEST** t4210 (`src/libs/kt-model/tests/transcript.rs`)

## ADDED Requirements

### Requirement: A UI frame costs what is visible, not the project
A UI frame SHALL do per-item work only for what it shows: the events whose body meets the view
(and the ones being dragged), the bin's cards its scroll shows, the open windows' own objects. What a
frame needs about the whole project — the events by time, the tails past the material, the ids, the
uses of a reel, the bin's card list — SHALL be computed once per state of the model and asked, not
walked. A drawing path SHALL NOT copy the document: it reads `TimelineModel::doc()` (or `doc_arc()`
when it keeps it across its own drawing), one per state. A curve a frame reads many times (the speed
map of the waveform and the composite) SHALL be laid out once, reading the same bits.
Why: the owner, kkt-26.11.07b item 1: "движение мышкой на паузе стоящей на событии при движении мышки
по превью - в htop показывает cpu 25%", and earlier "это зависит от позиции скрола таймлайна". On his
1974 events the bin copied the project four times a frame plus twice per reel card, the tail hatching
built every event's speed map, the waveform laid the speed curve out per pixel column; the effects
window copied the document three times a frame. A frame cost 6 ms over a still preview and 10 ms with
the whole edit in view; with the effects window open, 280 ms.

#### Scenario: The integral laid out once is the same bits
- **WHEN** a speed profile (sorted or not, with keys hidden left of zero, flat or sampled per 5 ms) is read at any progress
- **THEN** the laid-out integral returns exactly what `source_progress` returns
- **TEST** t1910 (`src/libs/kt-model/src/warp.rs`)

#### Scenario: The borrowed document is one per state
- **WHEN** a drawing path asks the document at the root, inside a reel, after an edit, an undo, a place change, a region or a loop switch
- **THEN** at the root it is the canon itself, inside a reel one copy per state, and it always equals `document()`
- **TEST** t1911 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: What is outside the view adds no work
- **WHEN** 2000 events lie past the view's right edge, or 270 files below the bin's fold, and the frames lay the events out
- **THEN** the drawing work of a frame is the same as without them, and the per-state facts still hold every event's tail
- **PATH** open a project with thousands of events → move the mouse over the preview and the timeline → only what is on screen is drawn, and Diagnostics → Performance reads the same "UI frame" as on a small project
- **TEST** t1912 (`src/apps/kt-app/src/frame_cost_tests.rs`)

#### Scenario: A frame's processor time does not follow the project
- **WHEN** ten frames run with 20000 events outside the view, or 1000 files in the bin, against a small project
- **THEN** they cost the UI thread less than one and a half times as much
- **TEST** t1913 (`src/apps/kt-app/src/frame_cost_tests.rs`)

#### Scenario: An open window does not follow the project
- **WHEN** the effects and the project settings windows are open over 20000 events outside the view
- **THEN** ten frames cost the UI thread less than one and a half times what they cost over a small project
- **PATH** double click an event → its effects window opens → move the mouse over it and over the preview → the program draws as fast as without the window
- **TEST** t1914 (`src/apps/kt-app/src/frame_cost_tests.rs`)

#### Scenario: The heavy project, measured live
- **WHEN** the heavy showcase is open and the pointer goes over the preview, over the timeline, over an open effects window of a release, and over the timeline zoomed out to the whole edit
- **THEN** each frame's own processor time stays under its limit, a pointer over the window composes nothing, and Diagnostics → Performance shows the UI frame's cost
- **PATH** open the project → move the mouse over the preview, then along a track → right click the composition under the playhead → Effects… → move over the window; Esc → Ctrl+wheel out until the whole edit shows → move along a track → open Diagnostics → "UI frame … ms" and its stages under Compose
- **TEST** live `ui_frame` (`src/scripts/live/ui_frame.py`)

### Requirement: The UI frame's cost is a number the person can see
The program SHALL measure each UI frame's own processor time on the UI thread, by stage — the frame,
the timeline (and its events), the media bin, the centre (and the preview's composite), the floating
windows — without charging a wait. Diagnostics → Performance SHALL show the last half second's
average as "UI frame N ms" and the stages; the load report SHALL write it per second after the work
(", ui frame N ms"); the bench dump SHALL carry the totals for live scenarios. The measurement steers
nothing (invariant 11).
Why: the owner's report showed a thread at 100 % and the number of frames, never what a frame cost
or where: "АГЕНТЫ НЕ ДУМАЛИ" about the tool he has to bring the proof with (kkt-26.11.07b item 5).

#### Scenario: The stages add up per frame
- **WHEN** two readings of the totals are taken around some frames
- **THEN** the per-frame average of every stage is their difference over the frames, and no frame gives no number
- **TEST** live `ui_frame` (`src/scripts/live/ui_frame.py`)

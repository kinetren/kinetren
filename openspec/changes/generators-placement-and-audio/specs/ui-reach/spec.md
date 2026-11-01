## MODIFIED Requirements

### Requirement: Paired ways of doing one action — one body; an action has one door
An action available through a gesture, a menu item, a button and a key SHALL be executed by ONE
body that accounts for the context identically (region, selection, reel). Publishing a parameter of a
composition SHALL have exactly one door — a right click on the field's caption INSIDE the reel (both in
full entry and in the expanded strip); outside the reel there is no item at all, but the status line
explains where it lives — and only when compositions exist in the document. The handle's name is the author's
text, drawn verbatim, bypassing the catalog.
Why: the reference failure — the M gesture with a region made a range label, while the menu item made
a point one; two entries with two copies — the same "two tracks from one +M".

#### Scenario: A generator is created from the main menu and from the bin with one list
- **WHEN** the "Create a generator" submenu is opened from the Edit menu, from a clip track's right-click menu and from the bin's background menu; the File menu is opened
- **THEN** the list of kinds is built by one body for all three entries (the track's filtered by its kind), creation puts the media and the clip in one history step, the track entry at the clicked track and time; File offers no generator item
- **PATH** click "Edit" → "Create a generator" → the groups; right click on a track → "Create a generator" first; right click on the bin's background → "Create a generator"; click "File" → no such row
- **TEST** t560 (`src/apps/kt-app/src/reach_tests.rs`), t1165 (`src/apps/kt-app/src/gen_audio_tests.rs`)

#### Scenario: The publish door is one and reachable only from inside
- **WHEN** a right click arrives at the parameter's caption inside the reel (full entry and strip), at the same caption outside the reel, and the instance's menu is opened
- **THEN** inside — "Publish…"/"Unpublish"; outside there is no item, and the status line says why; the instance's menu reaches BOTH windows — the consumer's and the author's
- **TEST** t682 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The consumer window draws each type with its own widget
- **WHEN** the instance's window of composition parameters is open with handles of all types
- **THEN** each type is drawn with its own widget, reset sends the removal of the override, a number with a curve places a key in the material of the INSTANCE; the window session is one history step
- **TEST** t683 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The author's name is drawn verbatim and does not move with the language
- **WHEN** a handle is declared with an arbitrary name and the program's language is switched
- **THEN** in the DRAWN galleries the name is present letter for letter in every language (the accessibility tree is not queried — there the reset button's compound name is used)
- **TEST** t684 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Badges speak of overrides, zero is not written
- **WHEN** the instance has overrides, the reel has declared handles, or there are no publications at all
- **THEN** the pin at the tail of the event's caption and the number in the head of the reel card are drawn; at zero — nothing
- **TEST** t685 (`src/apps/kt-app/src/layout_tests.rs`)

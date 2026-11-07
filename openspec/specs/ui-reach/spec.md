# Reachability of the interface

## Purpose

The mechanism "there are no dead ends": everything that is implemented is reachable from the point where the person acts;
the right mouse button is never silent; hints and menu items promise only what the program
does — and all of this breaks `cargo test`, rather than being left to a checklist.

## Requirements

### Requirement: There are no orphaned commands
Every variant of the model's `Command` SHALL be constructed by at least one line of the application's
product code (test code does not count toward reachability). The list of exceptions "the command
is not invoked from the GUI" SHALL start empty; an entry in it MUST carry a reason at least
40 characters long and MUST be removed as soon as the command is wired up. Idioms that blind the scan
(`use …Command::*`, `use kt_model::ops::…`) MUST NOT occur in the application.
Why: four commands of named areas were implemented, covered by unit tests and never once
invoked from the application — the area could be neither removed nor renamed while `cargo test`
stayed green.

#### Scenario: An enum variant is found in the product corpus
- **WHEN** the `Command` enum is parsed from the model source, and for each variant a construction is searched for in the application code and in the bodies of the `kt_model::ops` functions that the application calls
- **THEN** each variant is found on a word boundary (`SetFx` does not count as `SetFxOff`), the parser's self-check finds no fewer than 40 variants and three guard names, and there are no duplicates
- **TEST** t252 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The scan cannot be switched off with one line
- **WHEN** `use …Command::*` or `use kt_model::ops::` appears in a product file of the application
- **THEN** the same test is red, naming the file
- **TEST** t252 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Every timeline entity has a full set of verbs
For every timeline entity (`TlEntity`) it SHALL be recorded how it is created,
renamed, deleted and opened (its contents or properties). The verb table
SHALL be exhaustive over `TlEntity`, and the literal `TlEntity::ALL` — of fixed length:
a new entity MUST NOT compile until its row is filled in. "No verb" is accepted
only with a reason, "it is elsewhere" — only with a reference to a test; the length of the explanation
is checked. Every target of the context menu MUST name its own entity (an exhaustive
`match`).
Why: "you can create it but not control it" repeated for seven rounds in a row; the prose of the checklist
did not break anything.

#### Scenario: The entity menu offers the mandatory verbs from every place of action
- **WHEN** a right click arrives at the entity from EVERY place from which the person acts on it (the row header AND the track itself), in its own scene for each place
- **THEN** the entity exists for hit-testing, its menu contains items for all verbs of the `Menu` kind (the INCREASE in the number of occurrences of the label is counted — the same caption lives in the toolbar), the hint above it does not say "empty place", and for the `Absent`/`Elsewhere` verbs the explanation is not shorter than the threshold
- **TEST** t253 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The verb "open" for nested reels and earlier entities
- **WHEN** the entity is a nested reel, an event, a morpher or a transition
- **THEN** the table row names how the contents or properties are opened (entering the reel, the FX window, the morpher editor, the transition view window), or explains the absence
- **TEST** t253 (`src/apps/kt-app/src/reach_tests.rs`), t457 (`src/apps/kt-app/src/layout_tests.rs`) — four paths into the reel through one body (capability `compositions`)

#### Scenario: Every parameter and flag of the passport is reachable in a window
- **WHEN** the media passport declares a parameter or a flag
- **THEN** the parameter has a caption from the catalog, and the flag has a drawn control; something declared without a window is a dead end
- **TEST** t508, t509 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A click is a click
Pressing and releasing at the CENTER of an element without pointer movement SHALL produce the element's action. `ui.dnd_drag_source` MUST NOT occur in the application: a draggable and clickable thing is drawn, and over its rectangle (extended by the grab zone) goes ONE interaction `Sense::click_and_drag()` — `clicked()` selects, `drag_started()` places the payload.
Why: `dnd_drag_source` in egui 0.36 registers a `Sense::drag()` rectangle OVER the content and after it; `hit_test` returns `click: None`, and "activate parameter input" in the chain strip meant searching for an active pixel.

#### Scenario: A click on a chain slot selects it
- **WHEN** the harness presses and releases at the center of the slot's caption without movement
- **THEN** the slot is selected (the consequence has occurred), rather than "only the strip above the caption works"
- **TEST** t254 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Ban on `dnd_drag_source` across the whole application
- **WHEN** the product sources of the application are scanned without comments
- **THEN** there is not a single occurrence of `dnd_drag_source`
- **TEST** t254b (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A click on a picker gallery cell reaches the model
- **WHEN** the person has opened the effects picker and pressed on a variant cell found by its caption
- **THEN** the event carries an effect of the same kind WITH exactly the variant's parameters; typing in the search field does NOT close the popup (`CloseOnClickOutside`), while selecting closes it explicitly
- **TEST** t547 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The axis of force does not take the click away from the variant
- **WHEN** on a cell the person pressed, moved sideways and released — or pressed and released without moving
- **THEN** in the first case the event carries a mix of neighbouring points of the row (what was under the cursor, quantized); in the second — exactly the named variant; the axis edges clamp, and the centre of the cell is the variant itself
- **TEST** t551, t551b (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Text fits
A long user name (file, source, cell) SHALL be truncated with an ellipsis onto one
line, rather than wrapped letter by letter or stretching the strip; the truncation helper SHALL be
ONE for all places where a name is shown. The test assertion is a comparison of two scenes (a short
name against a long one) by the GALLERIES of the last frame, not by the accessibility tree.
Why: a 196-character source name without spaces produced 11 lines in the chain strip, and the
accessibility label is taken from the source text and never contains the ellipsis.

#### Scenario: A long source name does not grow the chain strip
- **WHEN** a name of several hundred characters stands in the source field and in the strip's header
- **THEN** each glyph gallery of the name is one truncated line, and the strip's height equals its height with a short name; moving the crumb ENTIRELY to the next row is legitimate
- **TEST** t255 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The picker gallery is robust to text
- **WHEN** the picker gallery is open with long variant captions and a typed search
- **THEN** the cell caption is truncated, the category heading is printed once, the gallery's width does not depend on what was typed, and the audio picker has no cells
- **TEST** t548 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A promise in a comment is closed by a scanning guard
A rule declared as "one for the whole project" SHALL have a scanning test that
names every duplicate by name: an assignment to `source_in` from an expression with it MUST
occur only in the rule's home, the piece's rule (`project.rs` of the model). The same device is applied to
every "one home" (finding a source from a clip, the sender to the player, offline rendering of reels,
the time floor of a gesture).
Why: the comment above `clip_slice` promised that "the piece rule is ONE", but the linear formula had been
written out by hand in six more places — fixing `Split` did not fix the trims and punch-out.

#### Scenario: There is no own copy of the piece's formula in either commands or operations
- **WHEN** `model.rs`, `ops.rs` and `project.rs` of the model are scanned without comments
- **THEN** lines with `source_in =`/`source_in +=` outside the rule's home are absent; on the old `ops.rs` the guard names all five duplicates
- **TEST** t273 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The same idiom in the neighbouring "one homes"
- **WHEN** a second media lookup from a clip appears, a second place deciding "what plays", a second offline rendering of a reel, or a bare `.max(0)` over the time of a gesture
- **THEN** the corresponding scanning guard is red, naming the file (the substantive rules belong to the capabilities `compositions` and `ui-timeline`)
- **TEST** t426b, t456b, t554, t602, t637c (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A right click is never silent
An object that answers a hover with a hint SHALL also answer the right mouse button, and the
menu SHALL belong to what was aimed at. Panel targets live in a SECOND registry
(`PanelTarget`/`PanelEntity`), separate from the timeline's targets; the table "panel entity →
what the right button answers" SHALL be exhaustive, and the literal `ALL` — of fixed
length. Timeline and panel targets MUST suppress each other: there is one menu on the screen. The right
button MUST NOT be destructive in itself (a curve key is not deleted by a right click — only
by a menu item).
Why: reconnaissance found twelve places with a hint and no menu; a person learns the right
button once, and silence reads as "there is no object" — so the area was deemed undeletable.

#### Scenario: Every panel object names its target
- **WHEN** a right click arrives in the background of the media bin (by the bin's rectangle, not the column), the meter panel, the preview window, a chain slot, the pinned chain head and a curve key
- **THEN** the program has set exactly this panel target, and the menu is its menu; a stale timeline target does not survive a click in the panel
- **TEST** t394 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The table of panel entities is exhaustive
- **WHEN** all panel entities and all kinds of targets are enumerated
- **THEN** each one states whether and how the right button responds (an explanation of at least 20 characters), and each target belongs to exactly one entity
- **TEST** t395 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: An object with a hint must also respond to the right button
- **WHEN** the object-hint keys of the catalog are scanned
- **THEN** each one is paired with a panel entity from the registry, and the key itself exists in the code (the table does not guard the invented)
- **TEST** t396 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A transition has its own menu, and a row has one menu from two places
- **WHEN** a right click arrives in the transition's window (a hit test over the WHOLE window, not over the ±6 px handles) or in the track itself (not only in its 34-px header)
- **THEN** the menu belongs to the transition (the view in the header, the properties, removal), and the row's menu is the same body as from the header: add morpher/camera here, the row's verbs, beyond the line — paste and region
- **TEST** t253 (`src/apps/kt-app/src/reach_tests.rs`), t397 (`src/scripts/live/rmb.py`)

### Requirement: A hint and a menu item promise only what the program does
A hint over an object SHALL describe a gesture that is actually performed in this zone. A key chord SHALL be passed to the menu item as an ARGUMENT (separator `\t`, as in
system menus) and MUST NOT sit inside a translatable catalog phrase; a chord is promised
only where the key does exactly that for THIS entity.
Why: "double click — 0 dB" was promised for years, while the gesture opened the FX window; "Delete (Del)"
on a named area named a key that the area does not touch.

#### Scenario: A double click on the level line resets the level
- **WHEN** a double click arrives at the level line of an audio or video event (the line is one for both kinds)
- **THEN** audio is at 0 dB, video is at full opacity; the FX window does not open
- **TEST** t392 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A chord in the menu only for the entity where the key works
- **WHEN** the menu of a named area and the menu of a camera are open
- **THEN** in no language does a catalog value contain "(Del"; the area has the delete item without a chord, the camera has it with a chord; for a clip the chord is shown only when there is no region (otherwise Del cuts the region)
- **TEST** t393 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The hint reaches the status line in the same frame
- **WHEN** the pointer hovers over a toolbar button, a window button or the bin
- **THEN** the rendered status line (`status_line_shown`, not an internal field) begins with the hint of this object with the kind "hover"; not a single `egui::Order::Tooltip` layer exists in egui's memory
- **TEST** t83 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The hint of a shared widget inside a window also arrives
- **WHEN** a widget inside `Window::show` (from where the model cannot be reached) sets a hint through the sink
- **THEN** it is shown in the status line, and an unparseable generator address is an error, not silence
- **TEST** t518 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: One hint; an impossible action is an error; there are no layers over the interface
The hint's place SHALL be one — the bottom right of the status line, priority hover → mode
→ message → standby line; there MUST NOT be pop-up tooltips or teaching overlays (`tooltip_delay` is blocking). A refused gesture SHALL be shown in an error style,
overriding the hover: the obstacle is highlighted, and if it is outside the view the
side and the distance are named, and at the edge of the strip a red arrow. A grey hover means "here is what can be done", not
"it cannot be done".
Why: the owner's decision "there are no pop-up hints at all" and the complaint "the hint is a pale faded little thing — everywhere such situations are made as errors"; both are direct requests and do not
change without a new one.

#### Scenario: A blocked move is an error with the address of the obstacle
- **WHEN** the move of an event runs into a neighbour, including one outside the view
- **THEN** the obstacle is known (there is something to highlight), in the status line the kind "error" overrides the hover, and outside the view — where to look
- **TEST** t172 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: There are no tooltips
- **WHEN** two frames pass after hovering
- **THEN** no egui layer has the `Tooltip` order
- **TEST** t83 (`src/apps/kt-app/src/layout_tests.rs`)

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

### Requirement: A test asserts the observable consequence from the point where the person acts; a live scenario for each item
A reachability test SHALL assert what the person sees and does (the rectangle grew,
the click landed, the block moved, the value reached the model/file), rather than the wiring ("the field
was set", "the item is in the menu"). The point of action is part of the assertion: the scene SHALL
return a LIST of places, and a new place is added to it, rather than "checked later". Closing
a UI item SHALL be accompanied by a live scenario in the real window (Xvfb, a real mouse),
which first proves that the gesture HAPPENED, and only then checks the consequence.

**What a live run pays for is decided by a CRITERION, not by the author's taste.** The mouse in the real window
SHALL check: a place that has NEVER been touched live; the hit of a gesture and its repeatability (hit test, edges, window order, modality, drag-and-drop); and a complex
multi-step path named in the owner's complaint. A check that, after a gesture has taken place, asserts ONLY the state of the model or file and repeats a gesture
already verified live in another scenario SHALL be moved to headless (the kt-app harness or a
model test): in a live run such a check costs seconds and adds no knowledge. The move
MUST keep the spec scenario and its `TEST` line with the NEW address — the scenario does not disappear,
only the place of the check changes; deleting a scenario without a replacement remains a violation of the rule
"tests from the specs are not weakened". The move SHALL be recorded MACHINE-READABLY — by a registry
"live scenario → the tests that hold its assertions, and why" — and the guard SHALL
fail if the file of a moved scenario remains on disk, or if the test it names does not
exist: without the registry, "move to non-gui" and "delete" are indistinguishable by the tree. What has never been touched live SHALL be shown by the machine
report of live coverage (the registry windows and fields that no scenario has clicked), and
`finish.sh` SHALL print its total. The number of untouched places SHALL have a DECLARED
LIMIT, and exceeding the limit SHALL be red: the limit is a ratchet, not a target, and a new
place of the program either gets a live step, or the limit is moved deliberately.
Why: everything the owner complained about in the round "you did not check" was green in the harness
and broken in the window — t253 pressed RMB into the 34-px header, where the item had been placed. R.68: 96 % of the time of a
live run is unconditional pauses (`nestparam`: 136 s, of which 130 s is sleep in 591 calls), and the
scenarios partly repeat one another. The owner's decision: "move some of the tests to non-gui,
since the tests partly repeat each other's actions… and click through the complex moments or those
that have never been clicked through even once".

#### Scenario: The row height reaches the event
- **WHEN** an individual height is set on the row
- **THEN** the event's rectangle has grown, and a click BELOW the old border lands on the event (rather than "`row_h` changed")
- **TEST** t274 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Blocks do not hide under one another and are caught by the frame
- **WHEN** "+M" is pressed again on an occupied gap; the selection frame passes over service blocks
- **THEN** each block is caught by the mouse in ITS OWN row (a free row is searched for on the gap, a new one — only if there is no free row); the frame catches morphers and cameras, and Del deletes everything caught in one step
- **TEST** t275, t277 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Live scenarios of the mechanism
- **WHEN** the live check is run (`scripts/live-check.sh`)
- **THEN** the edge of the morph block and the row height reach the file; a right click on the outline of a region changes the screen; a right click on a transition opens its menu; a double click on the level reaches the .otio, and the bin and meters are not silent; the publish path as a whole by mouse — from a right click on the caption to the second release with its own title
- **TEST** t264 (`src/scripts/live/deadend.py`), t397 (`src/scripts/live/rmb.py`), t689 (`src/scripts/live/nestparam.py`), `src/scripts/live/fxpicker.py`, `src/scripts/live/keys.py`

#### Scenario: The live coverage report and moving a check
- **WHEN** live coverage is counted: the `WINDOWS` registry windows and the fields of the geom dump against what the scenarios `src/scripts/live/*.py` click (both by calling a probe by name and by parsing a rectangle from the dump — an aim by coordinate is counted as the same kind of aim); separately, the test address of a spec scenario changes from live to headless; separately, a moved scenario returns to disk; separately, untouched places become more numerous than the declared limit
- **THEN** the report names the windows and fields not touched live EVEN ONCE, their number and the declared limit, as well as the registry of the move "scenario → new addresses"; `finish.sh` prints the total of the report; after the move the spec scenario is intact and its `TEST` line points to an existing test, while the disappearance of a scenario without a replacement, a moved scenario returning, a non-existent new address and exceeding the limit are red
- **PATH** there is no gesture: the guard reads the window registry, the geom dump and the texts of the live scenarios, and prints the places a person has never once reached by mouse in the check
- **TEST** t804 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-coverage.py --check`)

#### Scenario: The track verbs are clicked through from the MENU and from the WINDOW
- **WHEN** `src/scripts/live/trackverbs.py` opens the row's menu with a right click on an empty place of the track and presses in it "Mute", "Solo", "Lock", "Properties…" and "Effects…"; then in the track's window it clicks the checkboxes "Mute"/"Solo"/"Lock", the blend mode "Add" and "Cancel"
- **THEN** each item and each checkbox is reachable by mouse, the consequence is visible IN THE FRAME (under a muted track — the lower one; with solo — only it; add of green and red gives yellow), "Lock" from the menu refuses a drag, "Cancel" returns both the checkbox and the mode in one step and leaves no mode in the file; the program publishes the rectangle of each of these places, and the scenario presses each one
- **PATH** right click on an empty place of the upper video track → "Mute" → in the preview red instead of green → the same item once more → green again → right click on the lower track → "Solo" → in the frame only it → "Solo" once more → "Lock" → a drag of the event does not move it → "Lock" once more → "Properties…" → the track window → the "Mute" checkbox → frame red → the checkbox once more → blend mode "Add" → frame yellow → "Cancel" → frame green again, the window closed → right click on the track → "Effects…" → the same window with the chain strip
- **TEST** t806 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/trackverbs.py` (`./scripts/live-check.sh`)

### Requirement: The settings window and the geometry node are reachable from where the person acts
The project settings window SHALL be reachable from the project menu, by the address in the status line and
by a hotkey; the actions of the leading node (change the rule, reorder, add a second node)
SHALL be reachable from the event's chain editor; the window and the node SHALL have a full set of
verbs, checked by the machine against the paired entities. A live scenario SHALL drop a vertical
file into a 16:9 project and assert the consequence: the preview with letterboxing, the hint with the address, the window at the address, and changing the project's rule changes the picture.
A gesture over the FRAME SHALL be reachable and explained: the toggle for showing controls lives in the preview itself, its state is visible, and a right click on the frame names it in words; the hint of a handle
says what it edits, in the status line (rule 10 — no pop-ups). An event that has no frame at all (audio, a foreign placeholder) SHALL get a NAMED reason, not
silence. The gesture SHALL work repeatedly (rule 14) and in a SEPARATE preview window with the same body
(rule 17).
Why: "Dead ends: you can create it, you cannot control it" repeated for seven rounds; the test must probe from the point where the person acts. R.64: a control that cannot be discovered from the frame would repeat the Premiere complaint "I cannot call up the handles" (reconnaissance F-UI-001).

#### Scenario: Reachability and verbs
- **WHEN** the menus, the status-line address, the hotkeys and the chain editor are scanned
- **THEN** the project settings window and the "Geometry" node are reachable by each of the named paths, and the set of verbs is complete
- **TEST** t720 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A live scenario of a vertical source
- **WHEN** `src/scripts/live/conform.py` drops a vertical file into a 16:9 project, opens the window at the address and changes the project's rule
- **THEN** the screenshots show the letterboxing, the hint, the window, and a different picture after the rule change
- **TEST** `src/scripts/live/conform.py` (live scenario, `./scripts/live-check.sh`)

#### Scenario: Controls over the frame are reachable and explained
- **WHEN** the preview and its menu are scanned; a right click on the frame; hovering over a handle; an audio event is selected; the preview is detached into its own window
- **THEN** the toggle for showing controls is found in the preview and in the right-click menu, its state is visible; the handle's hint is in the status line; for an audio event the reason is named in a word; in the detached window the gesture works with the same body
- **TEST** t766 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A track has the full set of verbs, paired paths share one body
A track SHALL be a timeline entity with a full set of verbs (rename, delete,
insert above/below, reorder, mute, solo, lock, level, blend mode, chain,
properties), each reachable from the badge, the menu and a key where the path is declared; the set
is checked by the machine against the paired entities; a live scenario SHALL assert the consequence
(the picture changes on mute and on solo, the lock refuses a drag and names the track,
a vignette set in the track's window lands ON THE TRACK, not on the event, the render window
lists the muted track). The meters of a silent file would say nothing — the sum with level and mute
is held by t732 on the engine's side.
Why: the tests of R.38 checked the reachability of what was made, not the consequence; the rule of R.40 —
probe from the point where the person acts.

#### Scenario: Verbs and paths
- **WHEN** the badges, menus, keys and the track's properties window are scanned
- **THEN** the set of verbs is complete and matches at the root and inside a reel, and paired paths lead to one command
- **TEST** t741 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A live scenario of a track
- **WHEN** `src/scripts/live/track.py` mutes a track, sets solo, locks and drags an event, hangs a vignette from the track's window, opens the render window
- **THEN** the colour in the preview changes to the track's colour under the muted one and back; a drag over the locked one does not move the event, and the status line names it; the vignette ends up in `kt.tracks`, not on the event; the render window shows the line about muted tracks
- **TEST** `src/scripts/live/track.py` (live scenario, `./scripts/live-check.sh`)

### Requirement: Windows only from the registry; the tool remembers its parameters; name by industry; one widget
A new window (`egui::Window`, `Modal`) SHALL be introduced only with an entry in the window
registry (`src/apps/kt-app/src/reach_tests.rs`, `WINDOWS`): the id, what is in it and WHY it cannot be without a window; the
guard SHALL enumerate all places of window creation in the kt-app sources and fail on a place
outside the registry. Things that already have a place (the chain strip, the track window, the panel, the status line) MUST NOT get their own window. A tool with parameters (render, analysis) SHALL
remember the last entered parameters in the user store and substitute them on the
next call; a tool whose result is an envelope SHALL deliver it as ordinary
automation curves. The name of the item, the tool and the window SHALL be a generally accepted industry
name (Vegas/Premiere/Resolve/FCP), not a description of the mechanics.
An instrument that already exists in the program SHALL show the new function by itself, rather than by a copy
of its fields: ducking is a COMPRESSOR WITH A SIDECHAIN INPUT in the track's chain, shown
by its own widget next to the compressor of the key track. The parameters of such a tool live in the
PROJECT (they are the content itself — the track's chain) and go through undo; a workspace store is not created for them — "remembering what was entered" is ensured here by the project itself.
The setting of a live instrument SHALL be reachable from where the person already stands: the
audio track's window and its chain strip; a refusal (a cycle, a key on itself, a vanished track)
SHALL be named in words, not fade silently.
THE FORM OF A WINDOW SHALL obey a single contract, common to all windows of the registry:
the body of the window scrolls, and the action buttons ("Close", "Cancel", "Apply", "Render",
"Restore") live OUTSIDE the scroll and remain clickable at any screen height —
a window that has outgrown the screen MUST NOT hide its actions. A numeric field SHALL be drawn
by the COMMON row of widgets; there are no own `DragValue`s outside the common widget in windows.
Limits, step and unit SHALL come from the parameter's passport (`ParamSpec`), and for a quantity
without a passport they SHALL be declared in ONE place next to its definition and passed to the row as
an argument; writing them into the drawing code MUST NOT be done.
The six densest forms (the effect chain, morpher, transition, camera, project settings,
render) SHALL be laid out on the model of the "Transform" section: collapsible groups, pairs
on one line, a unit for every number, a slider for a quantity with both limits, reset
in one action. The other windows of the registry are NOT subject to re-layout, but they obey the form contract.
EVERY WINDOW OF THE REGISTRY SHALL be built by ONE window shell, and a window site MUST NOT set
by itself what the shell decides: the id, the title cross, collapsing, resizing, the scroll
policy, the width rule and the meaning of Esc. A window declares its KIND (tool window, dialog,
progress, message) and, where the kind allows it, its own default size; the body scrolls
in the shell's one body scroll, and a scroll of the window itself MUST NOT exist. Esc SHALL close the
window that is ON TOP for the person (the egui layer order), one window per press, before the
non-window steps of the Esc cascade (drag, region and selection, collapse, leaving the reel):
a tool window closes and KEEPS its edits; a dialog takes its action that loses nothing (Cancel,
Later, No). A window whose job must not be stopped by a stray key (export progress, analysis
progress) SHALL declare that Esc does not touch it, with a written reason of at least 25
characters, and Esc then goes on to the next window. The title cross, where the kind has one, and
Esc SHALL take one path with one consequence.
Why: R.63, the owner's words: "I forbid any agents to spawn any additional windows without an obvious purpose (like the window on closing the program)";
"why does the agent… force him to enter all the parameters again"; "duck under a track…
why didn't you simply name it properly"; "meaningless copy-pastes of the same code keep piling up everywhere". R.65: the ducking snapshot was taken together with its own parameter memory — a live instrument has its memory by construction, and a second one (in the store) would diverge from the project. R.66 (the owner's direct request of R.64 "go over this in all the other widgets and windows too"): prose about the look of a form does not work — by that point the scrolling of the body existed in exactly one window out of twenty-one, and it was added only after a test dropped a window with buttons hidden behind the edge. 2026-10-27
(ktprompts 26.10.18 item 6): the camera window did not close on Esc — the Esc cascade was a
hand-kept list, and four windows were missing from it; the owner: "we have lots of duplicated
code, though I asked that all widgets and windows reuse code and have the widest behaviour".
kkt item 2: the morpher window kept the window-level scroll that R.67 removed from the FX and
transition windows, and on another system it grew wider by itself.

#### Scenario: The window registry
- **WHEN** the kt-app sources are scanned for the places where the window shell is called; a window without an entry is temporarily added; the entry of a live window is removed from the registry
- **THEN** each place of window creation is found in the registry with a reason of at least 25 characters; both violations are red; the windows `duck` and `inst_params` are absent from the registry and do not exist in the code
- **TEST** t757 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: No window is built outside the shell
- **WHEN** the kt-app sources outside the shell module (test code cut off) are scanned for `egui::Window::new`, `Modal::new`, a window-level `.vscroll(`/`.scroll(`/`.hscroll(`, `.collapsible(` and `.open(&mut` on a window; separately a window built by hand is temporarily added to a copy of a source
- **THEN** none is found in the program; the added one is red with its file and line; the shell module itself is the only place where these calls stand
- **PATH** there is no gesture: the guard reads the kt-app sources and names the file and line of a window built by hand — it is verified by running the test, not by mouse
- **TEST** t921 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Esc closes every window
- **WHEN** each window of the registry is opened in its own scene (the morpher, transition and camera windows in a scene with a morpher layer, a transition and a camera event; the dialogs from their own gesture) and Esc is pressed once
- **THEN** the window is gone; a tool window's edits made before Esc are still in the model and its session is one undo step; a dialog left no trace (no command, no paste, no rename) and focus is released, so Space starts playback at once; the declared exceptions (export progress, analysis progress) stay open with their job running, and each has a reason of at least 25 characters; the camera window — the owner's case — closes
- **PATH** double-click the camera block on the camera track → the camera window opens over the timeline → change the zoom field → press Esc → the window is gone, the camera block keeps the new zoom, Ctrl+Z returns the old zoom in one step
- **TEST** t919 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Esc closes the top window first
- **WHEN** the FX window of an event is opened, then the camera window over it; Esc; then the FX and morpher windows are opened and the FX window is clicked to bring it to the top; Esc; with no window open and a region and a selection present — Esc
- **THEN** the first Esc closes the camera window and leaves the FX window open; the second pair closes the FX window (the one on top) and leaves the morpher window; with no window open Esc clears the region and the selection as before (the loop stays armed)
- **PATH** double-click an event → its FX window → double-click the camera block → the camera window on top of it → Esc → only the FX window is left → open the morpher window, click the FX window's title → FX is on top → Esc → only the morpher window is left
- **TEST** t920 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The title cross and Esc are one path
- **WHEN** each openable window of the registry that has a title cross is opened twice in the same scene; once it is closed by a click on the cross, once by Esc; the model, the undo stack, the clipboard and the focus are compared
- **THEN** both closings leave the same model, the same undo depth and the same focus state; every window of the kind "tool window" and "dialog" has the cross in the same place of its title row and no collapse arrow, every "progress" window has no cross, as its kind declares
- **PATH** open the project settings window from the toolbar → change the frame rate → click the cross at the right of its title → the window closes, the rate is unchanged; open it again → change the rate → Esc → the window closes, the rate is unchanged
- **TEST** t922 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A tool remembers its parameters and delivers curves
- **WHEN** the workspace (the meters' width, the project view) goes round a loop through the store file, and a file without the section is read again; separately — the track's sidechain compressor: the key and the parameters are edited in the track's window
- **THEN** the workspace settings have returned, a file without the section is still read as before; the sidechain parameters live in the PROJECT (the track's chain), go through undo and are edited by the common parameter widget — there is no second copy of them in the user store
- **TEST** t756b (`src/apps/kt-app/src/user_store.rs`), t775 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Names — by industry
- **WHEN** the catalogs are scanned for words describing the mechanics instead of a name ("under a track", "Russian: "under a track"", "snapshot of" in the names of items)
- **THEN** there are no menu items or windows with such phrases in their names; the items whose names were introduced by R.63 have all five translations
- **TEST** t756c (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: The sidechain is reachable and explains a refusal
- **WHEN** the audio track's window and its chain strip are scanned for the path "assign key"; an attempt is made to choose the track itself and a track that gives a cycle; a live scenario sets ducking by hand and listens to the result
- **THEN** the path is found from both places and leads to one command; an impossible choice is named as a reason in the status line, rather than being silently absent; the live scenario shows that the music dipped under the speech and came back after it
- **TEST** t777 (`src/apps/kt-app/src/reach_tests.rs`), `src/scripts/live/sidechain.py` (live scenario, `./scripts/live-check.sh`)

#### Scenario: The form contract — across all windows of the registry
- **WHEN** each window of the `WINDOWS` registry is opened in a LOW window; separately the kt-app sources are scanned for `egui::DragValue::new` outside `widgets.rs`
- **THEN** for each window the action buttons are visible within the screen, the body lives in a scroll; a window closed by the title's cross, and a window that is short BY CONSTRUCTION, are declared explicitly — with a reason and a measurable row threshold; not a single own `DragValue` outside the common widget is found
- **TEST** t779 (`src/apps/kt-app/src/reach_tests.rs`), `src/scripts/live/forms.py` (live scenario: the buttons are visible and clickable in a real low window)

#### Scenario: Six dense forms are laid out on the model
- **WHEN** the windows of the effect chain, morpher, transition, camera, project settings and render are opened
- **THEN** each has collapsible groups, value pairs on one line, a unit for every number, and a slider for a quantity with both limits; the collapse state is the window's state, not the project's; the limits coincide with the parameter's passport
- **TEST** t780 (`src/apps/kt-app/src/layout_tests.rs`), `src/scripts/live/forms.py` (live scenario: a group collapses and expands)

### Requirement: Project file doors — "Open" only accepts our own format, import through one door, export as a named action
"Open" (the menu, launch with a file, a drop onto the window) SHALL accept only a PROJECT file
(`.kt`). An exchange file `.otio` and the legacy `.ktproj` SHALL be rejected with the kind of file and the name
of the import item in the error text (rule 19), each with its own text; a file of a newer
format version — with the version number and without a partial read. The item "Import older Kinetren
project…" SHALL be ONE door for four inputs — the legacy `.ktproj`, our `.otio` of any
envelope version, a foreign `.otio`, and a `.kt` with the former marker `kinotrain` (before the rename
to Kinetren, 2026-10-21); the result SHALL open as an UNNAMED document of its own
format with the edits flag raised and the hint "save as `<name>.kt` next to
the source", and the source MUST NOT change; a project file of the current marker does not go through this door — the error
names "Open"; "Open" rejects a file with the former marker, naming Kinotrain and
the import item. Export to OTIO SHALL be a separate item `File → Export →
OpenTimelineIO…`, named by industry; it MUST NOT change the project file, its path or its asterisk. A `.kt`, `.otio` and `.ktproj` dropped onto the window SHALL lead to three different outcomes.
The dialog filters SHALL know the `.kt` extension, the import filter — both foreign ones.
Why: the owner's decision (R.70) — "we are moving away from the otio format, we keep otio only for
export"; a document after import has no project file, and writing `.kt` under a foreign name or
quietly putting it alongside cannot be done — the name is chosen by the person in the dialog; the second
item "Import .ktproj…" was an extra door with the same meaning.

#### Scenario: "Open" accepts only its own format and names the door
- **WHEN** "Open" receives a `.kt` file; separately — an exported `.otio`, the legacy `.ktproj`, and a `.kt` with a version higher than the current one
- **THEN** the first opens field by field with the project's path; the second and third are rejected with the name of the import item and the kind of file (the texts differ); the fourth — with the version number and without the import item's name
- **PATH** File → Open → `film.otio` → error window: "an OpenTimelineIO file, not a project — open it via File / Import older Kinetren project…" → File → Open → `film.kt` → timeline with the edit, the project name in the title
- **TEST** t818 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: One import door for three inputs
- **WHEN** through "Import older Kinetren project…" the legacy `.ktproj`, our `.otio` of an old envelope version and a foreign `.otio` with unfamiliar fields are fed in turn; then — a `.kt` file
- **THEN** each of the three opens as a document without a path, with the save hint `<name>.kt` next to the source and an asterisk, the source is byte-for-byte as before, the foreign fields — in passthrough; in the "File" menu there is one door (there is no item "Import .ktproj…"); the `.kt` file is rejected with the name "Open"
- **PATH** File → Import older Kinetren project… → `old.otio` → timeline with events, title "untitled *" → Ctrl+S → the dialog offers `old.kt` next to the source
- **TEST** t819 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/import_old.py`

#### Scenario: A project saved under the former name
- **WHEN** "Open" gets a `.kt` whose root key is the former `kinotrain`; then "Import older Kinetren project…" gets the same file
- **THEN** "Open" refuses it with a text naming Kinotrain as the former name and the import item; the import opens it as an untitled document with the star and the `<name>.kt` save hint, events and tracks equal to the file; the file's bytes are unchanged
- **PATH** File → Open → `old.kt` → error window: «a project of Kinotrain (the former name) — use File → Import older Kinetren project…» → File → Import older Kinetren project… → `old.kt` → timeline with the edit, title «untitled *»
- **TEST** t832 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Export to OTIO from the menu
- **WHEN** the "File" menu → the "Export" submenu → "OpenTimelineIO…" is opened and a file is named; the project, meanwhile, has unsaved edits and a foreign passthrough, and the reel is in two instances
- **THEN** the item is reachable and named "OpenTimelineIO…"; the exchange file is written: `Timeline.1`, two `Stack`s with non-empty bodies, the foreign fields are returned; the project file is byte-for-byte as before, the path and the asterisk are untouched; the notification names the export path; "Open" rejects this file
- **PATH** File → Export → OpenTimelineIO… → save dialog → file name → notification "exported: …", the asterisk in the title as it was
- **TEST** t820 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/export_otio.py`

#### Scenario: Three files on the window — three outcomes
- **WHEN** `film.kt`, `film.otio`, `film.ktproj` are dropped onto the window in turn
- **THEN** the first gives the question "open or insert as a composition", the edit is untouched; the second — a notification with the kind of file (OpenTimelineIO) and the name of the import item, without a question; the third — a notification about the legacy format with the same item; the texts of the second and third differ
- **PATH** drag `film.otio` onto the window → in the bell: «an OpenTimelineIO file, not a project — open it via File / Import older Kinetren project…» → drag `film.kt` → the window "Project dropped: open or insert as a composition?"
- **TEST** t821 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Import of an old-version project — its own door; "Open" rejects it with the address
The "File" menu SHALL have the item "Import older Kinetren project…" (the industry name —
"Import project", as in Premiere/Resolve), which reads an envelope file of the PREVIOUS version,
applying repairs of the old forms, and opens it as an UNNAMED document of the current form
(R.70: of its own `.kt` format) with the "has edits" flag raised and a hint to save it as
`<name>.kt` next to the source (the next record goes in the new form; the source
is not touched). An ordinary "Open" SHALL reject such a file with the file's version and with the name of the
import item in the error text (rule 19: the dialog says where to change the decision), rather than
"metadata.kt is unreadable". The item SHALL be in the verb registry and reachable from the menu and from the
open-error window.
Why: the owner's decision (R.67): "make an item in the File menu for importing old versions, and
fix it precisely in the import so that the reader can read it normally". R.70: the old path
no longer suits the document — it points to the exchange file, while the project is written in its own
format under its own name.

#### Scenario: An old file through import, "Open" names the door
- **WHEN** a v5 file with `targets: null` on the morphers (the form `untitled2_v26.09.26.otio`) is opened via "Open"; then via "Import older Kinetren project…"
- **THEN** "Open" rejects it with a text containing the file's version (5) and the name of the import item; the import opens the project: morphers with `null` act on "everything below", the number of events and morphers equals the file's, the document has no path and carries the `<name>.kt` hint, the title bears the asterisk, saving writes its own format, and the next "Open" already reads the file itself
- **PATH** File → Open → old file → error window: "version 5; use File → Import older Kinetren project…" → File → Import older… → the same file → timeline with events and morphers, title "untitled *"
- **TEST** t783 (`src/apps/kt-app/src/reach_tests.rs`), t783b (`src/libs/kt-otio/tests/convert.rs`), live scenario `src/scripts/live/import_old.py`

### Requirement: User path for a new UI scenario; live scenario screenshots in the report
Every scenario of a UI capability (`ui-timeline`, `ui-reach`) whose test lives in
`src/apps/kt-app/` AND has the number t783 or higher SHALL carry a line `- **PATH**` —
the sequence of the person's actions (where to click, what to drag) and what they SEE after
each step; without it the spec check SHALL be red (the guard is a script run by the
test and by `finish.sh`). A scenario where there is no gesture at all (the spec guard, the production
of the report) carries on the same line a REASON in words — and it is measurable: a reason shorter than
forty characters is not accepted, otherwise the excuse would become cheaper than the description. The threshold is by number, not "for everyone":
the numbering is continuous and monotonic, and the rule applies from the round in which it was introduced — rewriting two hundred earlier scenarios for the sake of form is pointless.
Live scenarios SHALL save a screenshot for each step of `PATH` into `target/live-out/`, and
`finish.sh` SHALL print the list of these files after the version; the agent's report on UI work carries
these screenshots. The run goes on OUR fixture projects, not on the owner's projects.
Why: R.67, the owner: "HOW MANY TIMES DO I HAVE TO REPEAT THAT ALL AGENTS SHOULD THINK ABOUT THE USER FIRST? … THIS IS COORDINATE-LEVEL, IT NEEDS CHANGING!" — and his choice: both mechanisms, "but not on
my projects". The rule in prose (checklist, R.40) held for seven rounds and did not work:
the check "what the person sees" must be part of the text of the contract and part of the report.

#### Scenario: The guard of the user path
- **WHEN** the specs are checked; temporarily the `PATH` line is removed from a new UI scenario; separately — for a scenario before t783
- **THEN** the check is green on the current specs; removal from a new scenario is red with the address of the file and the scenario; for an old scenario — not required (the numbering is monotonic, the threshold is measurable)
- **PATH** there is no gesture: the guard reads the texts of the specs and fails on a scenario without a described path — it is verified by running the script, not by mouse
- **TEST** t799 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/specs-user-path.py --check`)

#### Scenario: Step screenshots in the report
- **WHEN** a live scenario with `PATH` steps is run; then `finish.sh`
- **THEN** `target/live-out/<scenario>/` contains one file per step with the step's number and name; `finish.sh` prints their list after the version line; the list is non-empty for each scenario of the round
- **PATH** there is no gesture: it is verified by the fact that after the run the step files lie in `target/live-out/<scenario>/`, and `finish.sh` prints their list
- **TEST** live scenarios `src/scripts/live/curves.py`, `stills.py`, `import_old.py` (`./scripts/live-check.sh`), `src/scripts/finish.sh`

### Requirement: Live rig: the process is reused, waiting follows the FRAME, not the clock
The live check SHALL keep one kt-app process per worker and open projects in it ONE AFTER ANOTHER, instead of a cold start for each scenario. Opening a project in the bench SHALL
bring the program to a clean state (closed windows, a reset workspace), and the
scenario SHALL receive proof of cleanliness before the first gesture: the bench reports readiness, the number of open windows equals zero, and the open project is the one that was requested. A scenario
that checks the program's OWN start (recovery after a crash, the question about
unsaved work, the project lock, importing a file of an earlier version) SHALL declare "its own
process" and obtain it; the declaration is part of the scenario, not a setting of the run.

The wait after a gesture SHALL be bound to an OBSERVED event of the program — the number of
input events read, the number of the drawn frame, the appearance of a widget in the geometry dump or
a change of a pixel — rather than to a fixed pause. This SHALL apply also to the GESTURE ITSELF:
a click, right click, double click, drag, key, wheel and hover wait until the
program has READ their events, rather than for as long as a frame "usually" takes. The program SHALL
report, next to the geometry dump, AND the number of the drawn frame, AND the number of input events it has read: one frame
is not enough — it could have STARTED before the event arrived from the X server, and "frame drawn" then does not
mean "gesture seen". The wait for "the program has reacted" SHALL be counted by the silence between
frames, and this silence SHALL be measured in OBSERVED frame steps, not in seconds: under
load a frame takes three times as long, and a fixed threshold would declare the program calm
in the middle of drawing. A line of text SHALL
arrive at the program whole, not character by character with a pause for each. The window SHALL be raised
on a CHANGE of window and on the first gesture of a scenario, not before every gesture: raising is a round trip
to the X server and its own pause, while the window between two consecutive clicks does not change.

Fixed pauses SHALL be enumerable THROUGHOUT the live check — in the harness, in the scenarios
and in the mouse and keyboard driver: sleep hidden in the driver is no different from sleep in the
harness, and a rule that does not see it holds less than it promises. Each remaining
pause SHALL carry a REASON in words (a gesture that has no observable consequence, for example
egui's timing threshold of a double click, or a step of a drag between positions), and the guard SHALL fail
on a pause without a reason. The reasons SHALL NOT be the ONLY measure: the total fixed
sleep of the run SHALL be computed by the machine, printed, and have a DECLARED LIMIT, exceeding
which is red. The limit is a ratchet, not a knife: a new pause either fits within it, or the
limit is moved deliberately, in one line and with a reason.

Reading a POINT or a small area of the screen SHALL go by a direct request to the X server, rather than
by a snapshot of the whole screen parsed by an external tool. The screenshot (`xwd`) SHALL
remain the REFERENCE: the program's window has a 32-bit visual, and reading the root window was already
returning BLACK where the picture was — the direct path is accepted only after a cross-check
against the reference ON ONE AND THE SAME frame, and a discrepancy returns the reading to the reference. Snapshots for
artifacts and for the `PATH` steps remain snapshots.

The bench's test channel SHALL be switched on
only by an environment variable and SHALL have no effect in the product.
Why: R.68 measured that 96 % of the live run's time is sleep: `nestparam` 136 s with 130 s of
sleep in 591 calls, `curvesrc` 43 s with 42 s of sleep, `level` 4.7 s with 4.2 s of sleep. The pauses were
tuned to software rendering ("a frame takes ~0.2 s"), that is, the harness waited HOURS for what
the program could have said itself; the same blindness also gives false reds under load.
The owner: "cut down the restarts of kt (you can open projects one after another, after all)".
R.69: the wait on an event reached the probes but did not reach the gestures — 387 s out of a 1582 s
sequential run remained sleep (click 0.78 s, key 0.77 s, drag 2.55 s, typing
a line 0.77 s per character), and 123 s of them lived in the X driver, where the guard did not look. All
265 s in the harness were at the same time "legitimate": each pause had a reason longer than forty
characters. Prose did not hold — the exception must be MEASURABLE (the same lesson as
the window registry of R.66 and the PATH guard of R.67).

#### Scenario: Projects one after another in one process, start where it is needed
- **WHEN** a worker runs three scenarios in a row that did not declare "its own process"; then a scenario that declared it (the recovery check after SIGKILL)
- **THEN** the first three passed in ONE kt-app process: each saw its own project, an empty list of open windows and a workspace with no traces of the previous scenario; the one that declared "its own process" got a new process and checked the start; the verdicts matched a run where each scenario started by itself
- **PATH** the person does not see this: the scene is the run of the check; in the step screenshots of each scenario the first frame shows its own project and not a single open window from the previous one
- **TEST** t805 (`src/apps/kt-app/src/reach_tests.rs`), live scenarios `src/scripts/live/recover.py`, `src/scripts/live/level.py` (`./scripts/live-check.sh`)

#### Scenario: Waiting for the frame, not for the clock
- **WHEN** a scenario clicks, types a line and waits for a widget to appear; separately a fixed pause without a reason is added to the harness
- **THEN** each wait ends on an observed event (the program has read the input, the frame number has grown, the widget has appeared in the dump, the pixel has changed), the line arrived whole in one wait, and a pause without a reason makes the guard red; the total time of the live run on the same scenarios is noticeably less than before
- **PATH** there is no gesture: the guard reads the harness and the scenarios, counts pauses without a reason and checks that the waits are bound to an event of the program
- **TEST** t803 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-sleeps.py --check`), live scenario `src/scripts/live/nestparam.py`

#### Scenario: The gesture itself waits for the read input
- **WHEN** the harness makes a click, right click, double click, drag, key press, wheel and hover; separately — two gestures in a row in ONE window; separately — a gesture after a change of window
- **THEN** each gesture ends by the number of input events read by the program, not by the clock; the window is raised on the first gesture and on a change of window, and between two gestures in the same window it is not raised; the verdicts of all live scenarios match the run on the previous harness
- **PATH** there is no gesture: the scene is the live check run itself; the guard reads the harness and asserts that the gesture has no fixed pause without a declared reason
- **TEST** t807 (`src/apps/kt-app/src/reach_tests.rs`), cross-check run `./scripts/live-check.sh` (all scenarios, verdicts before and after)

#### Scenario: Sleep is enumerable across the WHOLE check and has a limit
- **WHEN** the sleep guard reads the harness, the scenarios AND the mouse and keyboard driver; a pause without a reason is temporarily added to the driver; separately the total sleep of the run is raised above the declared limit
- **THEN** the guard names the pause in the driver and turns red — the scope of the rule includes it on a par with the harness; the total fixed sleep is computed and printed, and exceeding the limit is red, naming the most expensive pause
- **PATH** there is no gesture: the guard reads the sources of the live check and prints the sleep budget; it is verified by running the script, not by mouse
- **TEST** t808 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-sleeps.py --check`)

#### Scenario: A screen point is read from X, and the snapshot remains the reference
- **WHEN** the harness reads the colour of a point of the preview by a direct request to X and, on the same frame, by the `xwd` snapshot; separately the direct path returns a colour that does not match the reference
- **THEN** the colours match, and the scenarios read the point without a snapshot of the whole screen; on a discrepancy the reading returns to the reference and says so, rather than silently returning black
- **PATH** there is no gesture: the scene is the harness reading the screen; it is verified by cross-checking the two paths on one frame
- **TEST** t809 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/trans.py` (`./scripts/live-check.sh`)

### Requirement: The FFmpeg window — state, Download, Locate, Rescan
*File › Preferences… › FFmpeg…* SHALL open a window that shows the search report (capability
`media-index`): the loaded FFmpeg's folder (or SONAME) and version, or "not found", and
every rejected candidate with its reason. On Windows the window SHALL offer **Download
FFmpeg 9.0.2 (gyan.dev, GPLv3)**, **Locate folder…** and **Rescan**. On Linux it offers
**Rescan** and the distribution hint. **Download** stays offered while the current pin's folder is absent, also
when an earlier download (`8.1.2\`) is the loaded set; the window then names that folder. **Locate folder…** opens a folder picker. A folder
holding a complete supported set is saved as the user's choice and loaded at once; any
other folder is refused with the reason, and the previous choice stays. The download shows
its progress, size and **Cancel** in the same window, and after it finishes the window shows
the new state without a restart. When FFmpeg is not loaded at start, the program SHALL show
one notice ("FFmpeg not found — media shows placeholders") with the same buttons, at most
once per session, and the window stays reachable from the Preferences window at any time. The window follows
the window-form contract (scrolling body, action buttons outside it).
Why: the owner's decision to find the user's install and offer the gyan.dev download
(2026-10-22) needs a place where a person sees why nothing plays and fixes it with the
mouse. A notice without buttons would be a dead end ("create, but can't manage", rounds
33–40).

#### Scenario: From the notice to playing media
- **WHEN** the app starts with the FFmpeg search limited to an empty folder and a project with one video clip; the notice's **Locate folder…** gets a folder without DLLs, then a folder with a complete set; then *File › Preferences… › FFmpeg…* is opened. On Windows the complete set is the pinned gyan.dev 9.0.2 build. On Linux the test's search seam reports that folder as complete and loads the system FFmpeg behind it, so the same UI path runs in CI on both OSes
- **THEN** before: the notice is shown once, the clip is a placeholder, the window lists the empty folder as rejected; the folder without DLLs is refused with "no avcodec-*.dll" and the choice is unchanged; after the complete folder: the clip shows frames without a restart, the window shows the folder and the loaded version, and the choice is in the saved settings
- **PATH** start → the FFmpeg window over the timeline: "FFmpeg not found — media shows placeholders", the empty folder under "Not used", the clip drawn as a placeholder → **Locate folder…** → folder picker → a folder without DLLs → red line "… cannot be used: no avcodec-*.dll" → **Locate folder…** → the folder with DLLs → the window's first line becomes "FFmpeg <major> (avcodec <x.y.z>) — <folder>" (on Windows "FFmpeg 9 (avcodec 63.x)"), the clip's preview shows frames → close the window → File › Preferences… → FFmpeg… → the same window with the loaded folder
- **TEST** t850 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Download stays offered over an earlier download
- **WHEN** the window's offer is computed (Windows rules) for: nothing loaded; FFmpeg 8 loaded from the download root's `8.1.2\` with no `9.0.2\`; FFmpeg 9 loaded from `9.0.2\`; FFmpeg 9 loaded from a chosen folder with no `9.0.2\`
- **THEN** **Download FFmpeg 9.0.2** is offered in the first, second and fourth case and not in the third; in the second the window's first line names the `8.1.2` folder; the computation is pure and runs on any OS
- **PATH** (Windows, an 8.1.2 download from an earlier version of Kinetren) start → media plays as before, no notice → File › Preferences… → FFmpeg… → the window's first line "FFmpeg 8 (avcodec 62.x) — …\kinetren\ffmpeg\8.1.2", below it **Download FFmpeg 9.0.2 (gyan.dev, GPLv3)**, **Locate folder…**, **Rescan** → **Download** → progress bar with size and **Cancel** → after it finishes the first line becomes "FFmpeg 9 (avcodec 63.x) — …\kinetren\ffmpeg\9.0.2" only after a restart (a loaded FFmpeg stays for the process), and the Download button is gone
- **TEST** t867 (`src/apps/kt-app/src/ffmpeg_setup.rs`)

### Requirement: UI language on Windows comes from the user's locale
On first start, when no language has been chosen, the UI language SHALL come from
`LC_ALL` → `LC_MESSAGES` → `LANG` when one of them is set and not empty (on any OS), and
otherwise on Windows from the user's default locale name (`ru-RU`, `zh-Hans-CN`, `es-MX`, …)
with the same mapping. The user's choice always wins.
Why: these variables are unset on Windows, so every Windows user started in English.

#### Scenario: Locale name of the OS when no variable is set
- **WHEN** detection runs with injected sources: no variables and OS locale `ru-RU`; no variables and `zh-Hans-CN`; `LANG=es_ES.UTF-8` and OS locale `ru-RU`; no variables and `fr-FR`
- **THEN** the languages are Russian, Chinese, Spanish (the variable wins) and none (English is used)
- **PATH** there is no gesture: the first start of `kinetren.exe` on a Windows account whose display language is Russian (no LANG set) shows the menu bar, toolbar hints and dialogs in Russian at once; after File › Preferences… → Language → English the choice wins on every next start
- **TEST** t861 (`src/apps/kt-app/src/i18n_tests.rs`)

### Requirement: Windows window behaviour matches Linux where the user can see it
On Windows a dropped file SHALL land where the pointer is when it is released over the
timeline (lane and time), as on Linux, and the drop silhouette SHALL follow the pointer.
The release build MUST NOT open a console window next to the program. The executable SHALL
carry the Kinetren icon and version in its resources.
Why: winit does not report the pointer position during an OLE drag on Windows, and the
non-Linux fallback returned "no position", so every drop landed at the playhead. A console
window and a generic exe icon are the first things a Windows user sees.

#### Scenario: Drop position, console and icon on real Windows
- **WHEN** on Windows a video file is dragged from Explorer onto the second video lane at about 10 s; the release `kinetren.exe` is started from Explorer; the exe is viewed in Explorer
- **THEN** the event lands on the second lane at the pointer's time and the silhouette followed the pointer; no console window appears; Explorer shows the Kinetren icon and the version in Properties
- **TEST** manual check on Windows recorded in `tasks.md` (the live harness runs only under Xvfb)

### Requirement: A welcome window at start; one primary tile lands on the most likely action
When the program starts without a project on the command line, without a recovery offer, and with
"Open the welcome window at start" on (the default), it SHALL open the welcome window over an empty
editor (when FFmpeg is not found, the FFmpeg window comes first and the welcome window opens when it
is closed). The window SHALL be a sized, resizable window that never opens larger than the screen, and
its body SHALL be three panes side by side, each with its own scroll that reaches the window's bottom:
Start — the primary tile, "New from template" (the built-in templates 1080p25, 1080p30, 4K30, 9:16
vertical 1080×1920 30, 1:1 square 1080×1080 30, each drawn with its frame's shape, followed by the
person's own templates), the tour tile ("Start the tour", or "Continue the tour — lesson N, step M"
once started) and the five lessons with their progress, each opening its lesson; Recent — the recent
projects, newest first, as poster cards (a placeholder until the poster exists) with name and folder,
as many per row as the pane's width holds; What's new — the notes of `docs/guide/whats-new.md`
compiled into the binary, from the running version's section (`ktYY.MM`, or the newest section with
its label when the running one has none) down to the oldest, drawn by the Guide's renderer. Scrolling
one pane SHALL NOT move another. Enlarging the window SHALL give the room to the panes. Exactly ONE
tile SHALL be primary — drawn larger and focused, so Enter takes it — and it SHALL be chosen by a pure
function of (is this the first start, the tour progress, the recent list with each file's existence):
the tour on the first start; the previous project when it exists; "Continue the tour" when a tour was
left unfinished and no project was opened since; a new project from the last-used template when the
previous project is missing (the missing row stays in the list, says where the file was, and offers to
locate it — it is not silently dropped). A click on a template SHALL create an unnamed project with
that frame size and rate in the same window, without a question unless there is unsaved work. The
What's new pane SHALL be headed by one calm line "Kinetren <version> is out — Releases page" when the
update-check state already on disk says a newer release exists; the window MUST NOT make a request of
its own. The window's action row SHALL hold the Guide, Preferences… and Close; the window SHALL NOT
carry a switch of its own: whether it opens at start is set in File › Preferences…, and Help › Welcome
window… reopens it at any time. Nothing in the window SHALL animate: it repaints on input only.
Why: the owner (2026-10-28) asked for an IDE-style intro window with templates, recent projects with
thumbnails, a tour tile, a primary tile for the most likely action, and What's new with an update
hint. F-HLP-027 had found that the only first-run instruction dies in the status line after 6 s; its
verdict "a start screen as a separate window — no" is revised by that request. kkt-26.11.01 item 1:
"Стартовое окно уродское … не ресайзится и в нем whats new суженный, и при скроле whatsnew скролится
всё подряд"; item 13: the start checkbox sat in the window AND in View (design D1, D6).

#### Scenario: The primary tile for each case
- **WHEN** the choice function is asked for: a first start (empty store); a start after the tour was
  finished with a recent project that exists; the same with the project file deleted; a tour stopped
  at lesson 2 with no project opened since; a tour stopped at lesson 2 and a project opened since; a
  project on the command line; a pending recovery offer; FFmpeg not found at start; "Open the welcome
  window at start" off
- **THEN** in order: the tour; the previous project; a new project from the last-used template (and
  the missing row is flagged, not removed); "Continue the tour — lesson 2"; the previous project; no
  welcome window; no welcome window (recovery first); the FFmpeg window first and the welcome window when
  it closes; no welcome window and the last project opens as before
- **PATH** there is no gesture: the person starts Kinetren and sees the window with the primary tile
  already focused — on the first start "Start the tour", later the previous project's poster tile
- **TEST** t984 (`src/apps/kt-app/src/welcome.rs`)

#### Scenario: The welcome window — templates, recent, What's new, the switch
- **WHEN** the harness starts with a user store holding two recent projects (one with a poster, one
  without) and one user template "Shorts 60"; then clicks the "9:16" tile; then reopens the window from
  Help and clicks the recent project; then turns "Open the welcome window at start" off in File ›
  Preferences… and restarts
- **THEN** the window shows the five built-in tiles, then "Shorts 60", the two recent cards (one
  poster, one placeholder) with names and folders, the tour tile, the Guide and Preferences… buttons and
  What's new for the running version, and no "show at start" checkbox; it lies inside the screen; the
  9:16 click gives an unnamed 1080×1920 30 fps project; the recent click opens that project; after the
  restart no window opens and the last project is open; no frame is drawn while the mouse is still
- **PATH** start Kinetren → the welcome window over an empty editor: on the left "Start" with the
  primary tile and the template rows "1080p25 · 1080p30 · 4K30 · 9:16 · 1:1 · Shorts 60", in the middle
  "Recent" with two cards (a frame of the project and a grey placeholder), on the right "What's new in
  kt26.11" → click "9:16" → the window closes, the preview is tall and narrow, the title says
  "Untitled" → Help › Welcome window… → click the first recent card → that project opens → File ›
  Preferences… → uncheck "Open the welcome window at start" → quit and start again → the last project
  opens directly, no window
- **TEST** t986 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Three panes, each scrolls alone, the window fits and grows
- **WHEN** the harness (1440×860) opens the welcome window over a store with twelve recent projects;
  the wheel turns over the What's new pane, then over the Recent pane; the window's corner is dragged
  toward 1900×1150 on a 1920×1200 screen
- **THEN** the window at its default lies inside the screen and its panes are side by side in the
  order Start, Recent, What's new, each reaching the action row; the wheel over What's new moves only
  What's new (the Start and Recent contents stay where they were), the wheel over Recent moves only
  Recent; after the drag the window's corner is where the pointer let go (the window is placed
  centred once, not anchored), the three panes are wider and taller, What's new is at least 300 points wide,
  and Recent holds more cards per row than before
- **PATH** start Kinetren → the welcome window: "Start" on the left, "Recent" with poster cards in the
  middle, "What's new" on the right → turn the wheel over What's new → only the notes move → turn it
  over Recent → only the cards move → drag the window's corner to the screen's corner → the corner stays under the pointer, the cards take
  the width in more columns, the notes get a wider and taller column
- **TEST** t1290 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

#### Scenario: What's new is the page, drawn by the Guide's renderer
- **WHEN** the welcome window is opened from Help › What's new with an update-check state that names a
  newer release
- **THEN** the What's new pane paints the running version's notes with their headings and items as the
  Guide draws them (a `<kbd>` is a keycap, not angle brackets), older sections follow under their own
  headings, the pane is scrolled to its top, and its first line names the newer release and the
  Releases page
- **PATH** Help › What's new → the welcome window opens with the What's new pane at its top: "Kinetren
  kt26.12 is out — Releases page", then "What's new in kt26.11" and its items → scroll the pane down →
  the "kt26.10" notes follow
- **TEST** t1291 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

#### Scenario: Lessons and templates in the Start pane
- **WHEN** the tour has lesson 1 done and lesson 2 started; the welcome window is open
- **THEN** the Start pane lists five lessons with their titles, lesson 1 marked done and lesson 2
  showing its done steps out of its steps; a click on lesson 3 opens lesson 3's sandbox and its panel;
  each template row paints a frame whose width-to-height ratio is the template's
- **PATH** Help › Welcome window… → in "Start" the lessons "1 · The first edit ✓", "2 · Effects and
  automation 1/3", … → click "3 · Transitions" → the window closes, the lesson project opens and the
  lessons panel reads "Lesson 3 of 5 · Transitions"
- **TEST** t1292 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

#### Scenario: The project name answers where it is drawn, past the left column too
- **WHEN** inside a reel the header's project name (the root crumb) runs past the left column over
  the preview — the menu row now has four items — and the person clicks the part past the edge
- **THEN** the editor leaves to the project's root: the crumbs take their clicks on the title's own
  layer with the screen as the clip, so a click is not clipped to the column (found by the live
  scenario `nestdead` after Help joined the menu row)
- **PATH** enter a reel → the header reads "<project> › Reel", the project name running past the
  left column → click the part of the name past the column's edge → the editor is at the root
- **TEST** t984b (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: The tour is lessons in their own view; a step closes on the project's state
The tour SHALL be a set of lessons shown in a docked side panel of the main window (it narrows the
editor, it does not cover it); a lesson is an ordered list of steps, each a short structured text of
the translation catalogs (paragraphs and list items, `<kbd>` keycaps for what is pressed, bold for
names; the names of the program's own buttons and menu items taken from their catalog keys, never
retyped), its targets, an optional "Show" button and a done predicate. A step SHALL close by itself
when its predicate over the project's state and the program's observable state becomes true (for
example: the project holds a title event; the event at the marker is cut in two; a chain holds a blur;
a loop region is set; the window of a registry id is open) — never by "Next" alone, never by "Show"
and never by a command registry. The predicate SHALL be re-checked whenever anything it reads changes —
the model revision, the region, the loop switch, the open-window set, the render result — in the same
frame, and the check SHALL cost no allocation per frame at rest. Every step SHALL name at least one
target where the person acts: a menu item, an event, a marker, the ruler, a transition, the morpher
block, an item of an event's right-click menu, or a named control of a window or the toolbar; each
target SHALL draw a static accent outline while its step is current. "Show" SHALL point at the target:
bring it into view (scroll the timeline to it, open the menu root, scroll the control into view) and
SHALL NOT perform the step (it never opens the window the step asks the person to open, never makes
the edit). A step whose result shows elsewhere than where the person acted (an effect, a transition, a
morpher batch) SHALL, when it closes, say in one line what changed and move the playhead to where it
shows. Every target named by a lesson SHALL exist in the registry, the menu catalog or the closed list
of named controls, and every named control SHALL be marked by the program's own drawing code; a lesson
whose target is missing SHALL fail the build's guard rather than show a dead step. The lessons SHALL
run on a fresh COPY of the built-in demo project, built from generators only (no media files), placed
in the user data directory and never in the person's own folders; "Start the tour" asks about unsaved
work like any other open. Progress (finished steps per lesson) SHALL live in the user store and SHALL
be reset by Help → Start the tour → "Start over". No layer above the window layer SHALL explain the
interface: the program's layers above `Order::Middle` are a closed, named list, and a lesson adds none.
While the export lesson's render runs, the panel SHALL show a rotating short joke from the catalogs.
Why: the owner's request (2026-10-28) revises F-HLP-028: of the three forms the reconnaissance looked
at, the one it deferred — a lesson that CHECKS the done action, in its own view (Ableton Learn View) —
is taken; the coach mark over the interface stays rejected (invariant 12, the "one hint" rule, and
t83 catches only `Order::Tooltip`). The deferral reason "there is no command registry" (F-UI-002) is
removed by closing steps on the project's state, which the model already exposes. The owner's walk of
2026-11-01 (kkt-26.11.01 items 2–8) found the first form teaching nothing past lesson 1: a step that
counted late, text without keycaps, no outline but one, "Show" that did the step, a lesson promising
a crossfade the program never makes, an effect added with nothing said about it.

#### Scenario: A lesson step closes on the project's state and "Show" reaches the target
- **WHEN** the harness starts the tour from the welcome tile; in lesson 1 the step "Add a title" is
  shown and its "Show" pressed; a title is added through the generator menu → Title; the next step "Cut
  the event at the marker" is done with S at the marker; the panel is closed and the program
  restarted; then "Start over" is used
- **THEN** a demo copy opens from the data directory (generators only, no missing media); "Show"
  opens the menu root and "Create a generator…" carries the static accent outline (the program
  never moves the person's pointer); the step closes by itself when the title event exists,
  without pressing anything in the panel, and the cut step closes when the event at the marker is in
  two pieces; after the restart the welcome window offers "Continue the tour — lesson 1, step 3" and
  the progress is intact; "Start over" clears it and makes a new demo copy
- **PATH** start Kinetren for the first time → the welcome window, "Start the tour" focused → click
  it → the demo project opens and a panel on the right reads "Lesson 1 of 5 · The first edit" with
  step 1 "Add a title" and a "Show" button → click "Show" → the Edit menu opens and "Create a
  generator…" carries an accent outline → Create a generator… → Titles and countdown → Text → a title
  event appears on the timeline and step 1 turns into a check mark, step 2 "Cut the event at the
  marker" appears → put the playhead on the marker and press S → the event is in two pieces and step 2
  is checked → close the panel and quit → start again → the welcome window's primary tile reads
  "Continue the tour — lesson 1, step 3"
- **TEST** t988 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Every lesson target exists and no teaching layer appears
- **WHEN** the guard walks every step of every lesson and checks its targets against the `WINDOWS`
  registry, the menu catalog and the closed list of named controls (each named control must be marked
  somewhere in the program's sources), and its text against the five catalogs; separately, with the
  lessons panel open on each step, the egui layers above `Order::Middle` are listed
- **THEN** every step has a target and every target exists; every text line has five translations
  within the catalog budget; the layers above `Order::Middle` are exactly the closed named list
  (status hint, title, in-place name editors, the overlays that show state), and none belongs to a
  lesson
- **PATH** there is no gesture: the guard runs with the program's own lessons; a person opening any
  lesson never sees a callout, a dimmed screen or a frame drawn over the editor
- **TEST** t988 (`src/apps/kt-app/src/reach_tests.rs`), `reach_tests::no_teaching_layer_over_the_ui` (t988, same file), t1092 (same file)

#### Scenario: The current step's target carries a static outline, drawn by the target itself
- **WHEN** each step of each lesson is made current in turn, with what the step needs on screen (the
  effects window for "+ effect…", the right-click menu for "Transition with previous", the render
  settings for Start)
- **THEN** each step's target draws a static accent outline in its own layer (a menu item inside its
  menu, a control inside its window, an event, a marker, the ruler, a transition or the morpher block
  on the timeline, clipped to the tracks) over the element it names, for as long as the step is
  current; nothing pulses, nothing dims, no arrow is drawn, and no layer is added above
  `Order::Middle`
- **PATH** open lesson 2 → the first video event on the timeline is outlined → double-click it → the
  effects window opens, step 1 is checked and "+ effect…" in the window is outlined → pick Blur → the
  step is checked and "+ curve" is outlined
- **TEST** t1087 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Help → Guide: "I want to … → where", with pages from the repository's guide
The program SHALL have a Help menu with Guide…, Start the tour, Welcome window… and What's new. The
Guide window SHALL hold a search field, an index of entries "I want to … → where / which gesture"
(each entry a catalog key, translated ×5, naming the place — a menu path, a panel, a window — and the
gesture or key), and the guide pages rendered from `docs/guide/*.md` (including `shortcuts.md`), which
SHALL be compiled into the binary from the same files the repository shows — no second copy. Typing
in the search SHALL filter the index entries and the page headings together (a heading is found
by its own words or by the words of its section); clicking an entry SHALL
open the page at its heading. Every index entry SHALL point to an existing heading of a guide page,
and the renderer SHALL accept only the markdown subset the guide uses (headings, paragraphs, lists,
bold, italic, inline code, `<kbd>` key caps, tables, links, fenced code blocks; an image is shown as
its caption); a guide file using anything else
SHALL fail the guard, not render wrongly. The welcome window SHALL link to the Guide. The guide pages
are English (capability `showcase`); the index entries are translated.
Why: the owner — "we have no guideline inside the program explaining what can be done and where"
(2026-10-27); the answer recorded the same day — "a Guide window and a welcome screen". The guide is
already written and checked (`readme-check.py`, t826); F-HLP-023 asked for addresses first, and an
index entry pointing at a heading is that address.

#### Scenario: The Guide finds "where" and opens the page
- **WHEN** the harness opens Help → Guide…, types "loop", clicks the entry "Play a part in a loop",
  then clears the search and opens "Keyboard and mouse"; separately the guard checks every index
  entry's heading and every guide file's markdown against the subset
- **THEN** the index shows "Play a part in a loop → drag on the ruler, then L" and the page heading
  "Markers and the region" from `timeline.md`; the click shows that page scrolled to that heading; the
  shortcuts page is the content of `docs/guide/shortcuts.md`; every entry resolves and every file
  passes the subset check, while a test copy with an HTML block fails it naming the file and line
- **PATH** open Help → Guide… → a window with a search field on top, "I want to…" entries on the left
  and the pages list below them → type "loop" → the entry "Play a part in a loop → drag on the
  ruler, then L" remains, with the headings whose sections speak of loops, "Markers and the region"
  among them → click the first → the right side shows the
  timeline page at "Markers and the region" → clear the search → click "Keyboard and mouse" → the key table
  of the guide
- **TEST** t989 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Selecting segments is reachable by mouse and keyboard and is said
The segment selection of the automation graph SHALL be said where the person acts: the graph's
hover hint over a segment names the gestures (Ctrl+click, Shift+click, a drag over empty space), the
selection row names what it edits (the bounds of one segment or the count), and the segment menu
header names how many segments it touches. The selection row's controls SHALL be reachable by Tab in
the window's order, a typed value SHALL apply on Enter, and the row SHALL carry a button that clears
the selection. The texts SHALL exist in all five catalogs.
Why: kkt-26.11.01 item 10 — the owner did not find where to set one segment's parameters; a selection
nobody can discover is the same "nowhere to write them" (rule 16: a control says what it does).

#### Scenario: The keyboard path through the selection row, and its clear button
- **WHEN** one zigzag segment is selected; the focus is moved by Tab from the "all" row's last control until it lands on the selection row's periods field; 3 is typed and Enter pressed; then the row's clear button is pressed
- **THEN** the periods field is reached by Tab alone, the segment has periods 3 after Enter, the other segments are untouched; after the clear button the row is gone and nothing in the curve changed
- **PATH** right click on a segment → its own shape in the menu → Tab, Tab… → the periods field of "Selected segment" gets the focus frame → type 3, Enter → the segment's zigzag thins → click × at the end of the row → the row disappears, the tint goes off
- **TEST** t1121 (`src/apps/kt-app/src/segfield_tests.rs`)

#### Scenario: The gestures are said in the hint, the row and the menu, in every language
- **WHEN** the pointer hovers a segment of the active curve; a selection of two segments exists; the segment menu is opened on one of them; the catalogs are read
- **THEN** the hover hint names Ctrl+click, Shift+click and the sweep; the row caption names the count; the menu header names the count; every new key exists in en, ru, es, zh-Hans and ar
- **PATH** hover a segment → the status line explains how to select several → Ctrl+click a second segment → the row says "Selected segments: 2" → right click on one → the menu header says 2 segments
- **TEST** t1122 (`src/apps/kt-app/src/segfield_tests.rs`), live scenario `src/scripts/live/segfields.py` (`./scripts/live-check.sh`)

### Requirement: A lesson step is re-checked when what it reads changes
The re-check of the current step SHALL follow every input of its predicate, not only the model
revision: a region drawn on the ruler and the loop switched on SHALL close "Play a part in a loop" in
the frame they happen, with any window open. The key that decides a re-check SHALL be built every
frame without an allocation.
Why: the region and the loop switch are convenience state that does not bump the model revision
(R.43 item 296: bumping it rebuilt both renderers and starved the audio); the first form keyed the
re-check on the revision and the open-window set only, so the step closed only when a window closed
(kkt-26.11.01 item 2).

#### Scenario: The loop step closes while the generator window stays open
- **WHEN** lesson 1 is at "Play a part in a loop" with the text generator's window open; a region is
  dragged on the ruler and L is pressed; nothing else happens
- **THEN** the step is checked within two frames and the window is still open; the re-check key
  holds no heap data
- **PATH** lesson 1, step 4, the title's window still open → drag on the ruler from 0:20 to 0:30 → a
  region appears → press L → the loop button lights up and step 4 turns into a check mark at once,
  the title window stays open
- **TEST** t1085 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Lesson text has paragraphs, lists and keycaps
A step's text SHALL be one or more catalog lines, each a paragraph or a list item, drawn by the
Guide's inline renderer: `<kbd>` as a keycap for every key and every button or menu item pressed,
bold for names. The label of a program control named in a step SHALL be substituted from that
control's own catalog key; a menu path SHALL be built from the step's target. All five catalogs SHALL
give a step the same number of lines, the same kinds of lines and the same number of keycaps; the
parsed text SHALL be kept per language and not rebuilt per frame.
Why: kkt-26.11.01 item 3: "press + effect… and pick" ran together as plain text, while the Guide
already draws `<kbd>`. A label retyped in the lesson drifts from the button (the generator menu moves
out of File in `generators-placement-and-audio`).

#### Scenario: Every step parses into lines with keycaps in five languages
- **WHEN** the text of every step is parsed in each of the five languages and the panel draws lesson
  2, step 2
- **THEN** every step's lines parse in the Guide subset with the same line count, line kinds and
  keycap count across languages; every key and button the English text names is a keycap; the label
  inside "+ effect…"'s keycap equals the button's own catalog text in that language; the panel shows
  the step as separate lines, not one run-on line
- **PATH** open lesson 2, step 2 → the panel reads "Add a blur." and then a list: "Press [+ effect…]
  in the effects window" and "Pick **Blur**", the button name in a keycap
- **TEST** t1086 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: "Show" points at the step's target and never performs the step
Pressing "Show" SHALL bring the current step's first reachable target into view — the timeline
scrolled so the event, marker, transition or morpher block is visible, the menu root opened so the
item is visible, a window's control scrolled into view — and SHALL NOT open a window the step asks the
person to open, make an edit, or close the step. When a target lives inside a window that is closed,
"Show" points at the next target of the step (the element on the timeline whose double-click opens
that window).
Why: kkt-26.11.01 item 5: "Show" did nothing for panels, and in lesson 4 it opened the morpher, which
closed the step and left the person not knowing where the morpher is.

#### Scenario: Show on every step leaves the step open and its target in view
- **WHEN** for every step of every lesson, the timeline scrolled far away from the target, "Show" is
  pressed
- **THEN** no step closes; no window opens that was not open; the outlined target lies inside the
  visible timeline, the open menu or the window; on lesson 4 step 1 the morpher window does not open
  and the BATCH block is outlined in view
- **PATH** lesson 4 → the timeline scrolled to the end → press Show → the timeline scrolls to the
  BATCH morpher block, outlined → double-click it → the morpher window opens and step 1 is checked
- **TEST** t1088 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: The lessons panel leaves the timeline and its toolbar whole
The lessons panel SHALL narrow only the area above the timeline: the timeline and its bottom toolbar
keep the window's full width, so a control a lesson names in that toolbar stays on screen.
Why: found by the live walk (tour-lessons-fixes): drawn before the timeline, the panel clipped the
toolbar row at its edge, and in a 1440 px window the Loop button lesson 1 names was not visible.

#### Scenario: The Loop button stays visible beside the lessons
- **WHEN** lesson 1, step 4 is open in a 1440×860 window
- **THEN** the timeline panel spans the window's width, and the Loop button's outline lies inside it
- **PATH** open lesson 1 in a 1440-wide window, reach step 4 → the panel stands beside the preview,
  the timeline runs to the right edge under it → the Loop button in the toolbar is outlined and
  visible
- **TEST** t1094 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: "Next lesson" keeps the lesson copy without a question
"Next lesson" SHALL save the edited lesson copy (the program's own sandbox) and open the next lesson
without the unsaved-work question; "Start over" SHALL still ask, since it discards the edits.
Why: found by the live walk (tour-lessons-fixes): finishing lesson 1 and pressing "Next lesson"
raised "Unsaved work — Save and continue / Discard / Stay here" about a file the person never chose.

#### Scenario: Next lesson after an edited lesson
- **WHEN** lesson 1 is finished with its copy edited and "Next lesson" is pressed
- **THEN** no question is asked, lesson 2 opens, and the lesson-1 copy on disk holds the edits
- **PATH** finish lesson 1 → the panel reads "Lesson done" → press Next lesson → lesson 2 opens at
  once, no "Unsaved work" window
- **TEST** t1095 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A click on a marker puts the playhead on it
A plain click (no drag) on a marker on the ruler SHALL move the playhead onto the marker, as a click
on the ruler beside it does (it snaps there); a drag still moves the marker and a right click still
offers its menu. The marker's hover hint SHALL name the click.
Why: found by the live walk of the tour (tour-lessons-fixes): the cut lesson says "put the playhead
on the marker", and the marker's hit zone swallowed the click — the playhead stayed where it was and
S cut somewhere else.

#### Scenario: Click the marker, then cut there
- **WHEN** lesson 1, step 2 with the playhead at 0:52; the CUT HERE marker is clicked on the ruler;
  S is pressed over the timeline
- **THEN** the playhead stands at the marker (0:06) and the marker has not moved; the first event is
  in two pieces at the marker and the step is checked
- **PATH** lesson 1, step 2 → click the CUT HERE marker on the ruler → the playhead jumps to 0:06 →
  press S → the first event is in two pieces at the marker and step 2 is checked
- **TEST** t1093 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Lesson 3 teaches the transition the program makes
The transitions lesson SHALL teach "Transition with previous" from an event's right-click menu and
close its first step on a video transition between two events; the item SHALL be outlined in that
menu. Dragging an event onto its neighbour SHALL NOT close the step: an overlap stays a cut (t163,
t464).
Why: kkt-26.11.01 item 6 and the owner's answer 1 of 2026-11-01: the lesson promised a crossfade from
an overlap; the overlap semantics are the owner's decision and stay.

#### Scenario: An overlap does not close the step; Transition with previous does
- **WHEN** lesson 3 is open; the second video event is dragged onto the first so they overlap; then
  the second event is right-clicked and "Transition with previous" → Dissolve picked
- **THEN** after the drag the step is still open and the project holds no transition; with the
  right-click menu open the item is outlined; after the pick step 1 is checked
- **PATH** lesson 3 → drag the second event left over the first → the step stays, the events meet as
  a cut → right-click the second event → "Transition with previous" is outlined → hover it → Dissolve
  → a transition appears between them and step 1 is checked
- **TEST** t1089 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A step that changes the picture says what changed and shows it
When a step whose result shows in the preview closes (an effect added to an event, its curve, a
transition, its kind, an effect added to a morpher's batch), the panel SHALL show one line under the
checked step saying what changed and where, and the playhead SHALL move to a moment where the result
shows: inside the event with the effect, the middle of the transition, inside the morpher's span. The
move happens once, when the step closes, not on later frames.
Why: kkt-26.11.01 item 7: after the effect was added the lesson said nothing, and the person had to
guess where to bring the playhead to see the result.

#### Scenario: The morpher's new effect is announced and the playhead moves into its span
- **WHEN** lesson 4, step 2 is current with the playhead at 0:00, and an effect is added to the
  morpher's batch
- **THEN** the step is checked; under it the panel says the effect now works on the events under the
  BATCH block; the playhead is inside the morpher's span; the same holds for lesson 2's blur (inside
  the event with the blur) and lesson 3's transition (inside the transition)
- **PATH** lesson 4 → double-click BATCH → "+ effect…" → pick any effect → step 2 is checked, the
  line "The effect now works on both events under BATCH…" appears and the playhead jumps to 0:52, the
  preview shows the effect
- **TEST** t1090 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Jokes while the export lesson renders
While the render started from the export lesson runs, the lessons panel SHALL show one short silly
line from a catalog set, changing every few seconds; the set SHALL exist in all five catalogs; the
change SHALL request one repaint at the next switch, not a continuous repaint; nothing SHALL show
once the render finishes.
Why: kkt-26.11.01 item 8: "you could make some silly jokes in the lessons block while the render
runs, it is too boring".

#### Scenario: A joke shows and changes while the render runs, and goes when it is done
- **WHEN** lesson 5, step 2 is current and a render job is running; time advances past the switch
  interval; then the render finishes
- **THEN** one joke of the set is shown, a different one after the interval, every joke has five
  translations; after the render no joke is shown and step 2 is checked
- **PATH** lesson 5 → File › Render to MP4… → Start → while the progress runs the lessons panel shows
  a line like "Teaching the pixels to stand in line…", a few seconds later another → the render ends,
  the joke goes and the step is checked
- **TEST** t1091 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Every window is resizable, its body fills the area, and it remembers its size
Every window of the registry that is not a modal question SHALL be resizable in width and in height,
with a minimum size below which its form stops being usable and no maximum other than the screen.
When the person enlarges a window, its body SHALL take the added room: the body's scroll SHALL reach
down to the action row (or to the window's bottom edge when there is none) and span the window's
width; lists, tables, galleries and previews SHALL grow with it, and text SHALL re-wrap to the new
width. No window body SHALL be capped by a fraction of the screen or a fixed number of points, and
no column inside a window SHALL have a fixed width, unless the guard lists that size with its reason
(a numeric field sized for its longest value, a thumbnail cell). A modal question (the recovery offer,
the question on close) is declared with its reason and is exempt. The size the person gave a window
SHALL be kept across openings: a window opened per object (effects, morpher, transition, camera,
track) SHALL open at the size its kind was last given, and a single window at its own last size, in
the same run and after a restart. A resizable window SHALL still not grow by itself (t918, t923). A window whose content arrives after it opens (the finds of the project search) SHALL open SIZED, with room for that content, not fitted to its empty form: a fitted window fills the height it measured at opening and clips what comes later.
Why: kkt-26.11.01 item 12, the owner: "if you make the Guide window bigger its elements will not
take the whole area, and again you look through a needle's eye on a big window … and so everywhere
… I am tired of reminding"; "some windows grow in width but not in height". The body was capped at
0.62× the screen and shrunk to its content, half of the windows were not resizable at all, and each
earlier fix was of one window — the class came back in the next. One rule in the window shell and one
guard over all windows instead.

#### Scenario: Every window grows when its corner is dragged and its body fills it
- **WHEN** every window the registry can open is opened in a 1920×1200 harness and its bottom-right
  corner is dragged out to make it about 1500×1000 points
- **THEN** the window's frame grew to the dragged size (it is resizable and nothing caps it), and its
  body's scroll reaches within the action row of the window's bottom and spans its width; a modal
  question is listed with its reason (≥ 25 characters) instead
- **PATH** open the effects window of an event → a window of about 580×520 with the chain, the
  parameters and Done/Cancel → drag its bottom-right corner down and right → the window follows the
  pointer, the chain and parameters stretch across, the empty room is inside the scroll above
  Done/Cancel, not around the window's content
- **TEST** t1125 (`src/apps/kt-app/src/window_fill_tests.rs`)

#### Scenario: A window keeps the size the person gave it
- **WHEN** the effects window of one event is enlarged and closed, and the effects window of another
  event is opened; separately the Guide is enlarged, closed and reopened
- **THEN** the second effects window opens at the enlarged size, and the Guide reopens at its own
  enlarged size
- **PATH** open the effects window of event 1 → drag its corner to make it large → close it → open
  the effects window of event 2 → it opens just as large; Help → Guide → enlarge → close → Help →
  Guide → it is as large as it was left
- **TEST** t1126 (`src/apps/kt-app/src/window_fill_tests.rs`)

#### Scenario: A fixed size in a window carries a reason
- **WHEN** the kt-app window code is scanned for screen fractions, fixed scroll heights and fixed
  column widths (`content_rect() … *`, `max_height(<number>)`, `set_width(<number>)`,
  `set_min_width`/`set_max_width(<number>)`, `desired_width(<number>)`)
- **THEN** every match is listed in the guard's table with a reason of at least 25 characters, and
  the table holds no entry the code no longer has; the 0.62× screen cap of the window body is gone
- **PATH** (not a UI scenario: a source scan that keeps fixed sizes out of the windows; the
  consequence the person sees is held by t1125)
- **TEST** t1127 (`src/apps/kt-app/src/window_fill_tests.rs`)

#### Scenario: The project search shows what it finds
- **WHEN** the search window is opened on a project with three markers whose names start with "DR",
  and "dr" is typed after it opened
- **THEN** the finds lie inside the search window's frame (before: the window stayed the height of
  its empty field and the finds were laid out below its edge, unseen)
- **PATH** press Ctrl+F → a window titled "Search the project" with an empty field and room under it
  → type "char" → the media 01-charge.webm and 05-charlotte-by-night.webm and every event that uses
  them are listed under the field, the first one highlighted
- **TEST** t1414 (`src/apps/kt-app/src/layout_tests.rs`), live `readme_shots.py`
  (`src/scripts/live/readme_shots.py`, the search step)

### Requirement: The Guide draws real tables and separate index cards and uses its whole window
A table on a guide page SHALL be drawn as a grid table: a header row in its own shade, a rule between
the columns and between the rows, alternating row stripes, and column widths taken from the content —
a column is never narrower than its longest word, and the width left over is shared in proportion to
how much text each column holds; cells wrap within their column. Every "I want to…" entry SHALL be its
own card: the "what" on its first line, the "where" below it in a weaker style, a frame around the
card and a gap between cards, the whole card clickable. The Guide's two sides SHALL share the window:
the index side a proportion of the width with a floor and a ceiling, the page the rest, and both
lists SHALL reach the window's bottom.
Why: kkt-26.11.01 item 12, the owner: "in the Guide all the tables are crooked too, their first column
is narrow and in the end it looks cheap"; "the text under "I want to…" visually runs together, though
it turns out to be separate sections"; "the Guide window looks unfinished". The table was a
two-column egui grid of wrapping labels (the first column collapsed to its narrowest word and wider
tables lost columns), and the entries were frameless wrapping buttons with one arrow between the two
halves.

#### Scenario: A guide table has columns from its content
- **WHEN** the Guide opens the "Keyboard and mouse" page in a 1600×1000 window, and again in a
  narrow one
- **THEN** each table's columns match its header's count; no column is narrower than its longest
  word; the first column of the keys table is at least as wide as its widest key cap and no row's
  first cell wraps when the window is wide; rules are painted between columns and rows and the header
  row has its own fill; in the narrow window the cells wrap and the table stays within the page
- **PATH** Help → Guide → click "Keyboard and mouse" → each table has a shaded header, the key column
  is as wide as its longest key, the action column takes the rest, lines between the cells → make
  the window narrow → the action texts wrap inside their column, the table does not run past the page
- **TEST** t1128 (`src/apps/kt-app/src/window_fill_tests.rs`)

#### Scenario: "I want to…" entries are separate cards
- **WHEN** the Guide opens with an empty search
- **THEN** every index entry is drawn in its own framed rectangle, the rectangles do not overlap and
  are separated by a gap, the "what" text lies above the "where" text inside each, and a click
  anywhere on a card opens its page at its heading
- **PATH** Help → Guide → the left side lists "I want to…" cards: "Play a part in a loop" with "drag
  on the ruler, then L" under it, each in its own frame → click the card's lower line → the timeline
  page opens at "Markers and the region"
- **TEST** t1129 (`src/apps/kt-app/src/window_fill_tests.rs`)

#### Scenario: The Guide's sides grow with the window
- **WHEN** the Guide is opened at its default size, then its corner is dragged out to about 1500×1000
- **THEN** the index side grew wider within its ceiling, the page grew by the rest, and both scrolls
  reach the window's bottom in both sizes
- **PATH** Help → Guide → drag the corner far out → the cards column widens a little, the page takes
  the rest of the width, both lists run to the bottom of the window
- **TEST** t1130 (`src/apps/kt-app/src/window_fill_tests.rs`)

### Requirement: The effect picker is one gallery that grows with its window
The "+ effect" picker SHALL show its content in ONE scroll: the presets as a row of cells first, then
the effects by category, each effect a card (its name and its variant cells) and the cards flowing
into as many columns as the gallery is wide. Its width SHALL be the width of the window it opens
from (never less than the gallery's floor), so an enlarged effects window gives a wider gallery with
more cards per row, and its height SHALL be the room the screen has below the button, never a fixed
number. What a person types into the search SHALL NOT change the popup's width.
The same screen-bounded height SHALL apply to the media picker's list.
Revision (R.54, `GALLERY_W` "fixed by the gallery of the longest content"): the width is no longer a
constant; it follows the host window, which keeps the R.54 point — nothing jumps while typing.
Why: kkt-26.11.01 item 12, the owner: "the effects window is small by itself, and when you want to add
an fx the choice of fx is split further into two sections, and there too it is a needle's eye". The
picker was two scrolls of 160 and 450 points in a 520-point popup, whatever the window or the screen.

#### Scenario: The picker gallery follows the window and the screen
- **WHEN** the effects window of a video event is opened at its default size and the picker is
  opened; then the window is enlarged to about 1400 points wide and the picker is opened again; a
  query is typed
- **THEN** the picker has one scroll; its width is the host window's content width in both cases
  (the second is wider) and does not change while typing; a row of the wide gallery shows more
  effect cards than a row of the narrow one; the popup's bottom is within the screen and lower than
  500 points below its top when the screen allows
- **PATH** open the effects window of a video event → click "+ effect" → a gallery as wide as the
  window, presets then effects in one list → close it, drag the window's corner to make it wide →
  click "+ effect" → the same gallery, wider, with more effect cards per row, reaching down the screen
- **TEST** t1131 (`src/apps/kt-app/src/window_fill_tests.rs`)

### Requirement: Masks live where a person looks for them — one list widget in three existing places
The masks SHALL be reachable without a new window (rule 22): the event's picture masks in a "Masks"
group of the Geometry node's "Transform" form and from the preview's right-click menu (Mask →
Rectangle / Ellipse / Pen, for the selected event); an effect slot's masks in a "Masks" group under the
slot's parameters in the effects window; a track's masks in a "Masks" section of the track window. All
three SHALL be drawn by ONE widget: the tool buttons (Rectangle, Ellipse, Pen), and per mask a row with
its name, the mode as a row of four words, invert, opacity, feather with falloff, expansion, the path's stopwatch, an edit button
that selects it on the preview, reorder arrows and a delete button; the selected mask's curves below,
in the one automation widget (on a track — no stopwatch and no graph: its masks are static). Every number is a passport
field (limits from the passport, keys only by gesture). The texts SHALL exist in all five catalogs.
Why: kkt-26.11.01 line 55 — "do not throw features anywhere"; the places are those of AE (the layer's
masks), Premiere (each effect's masks) and Resolve (a window on the node); rule 24 — one widget.

#### Scenario: An effect is limited to a mask from the effects window
- **WHEN** the effects window of an event with an effect is opened, the slot selected, its Masks group → Ellipse, a drag on the preview; invert is clicked; the feather field is dragged
- **THEN** the slot has one ellipse mask (the event's picture masks are untouched), inside it the effect, outside the plain event; invert swaps them; the feather reaches the model; the window's Cancel rolls all of it back
- **PATH** double-click the event → the effects window → click the effect's slot → under its parameters "Masks" → Ellipse → drag on the picture → the effect shows only inside the ellipse → tick "Invert" → it shows only outside → drag "Feather" → the edge softens
- **TEST** t1279 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: The picture masks in the Geometry node and the track's masks
- **WHEN** the Geometry node's form is opened and its Masks group used to add a rectangle and an ellipse, the second set to subtract (scrolling the window's body down to it) and moved up; then the track window's Masks section adds a rectangle, and a curve on its feather is sent by command
- **THEN** the event has two masks in the new order with the modes; the track has one mask, its row has no stopwatch and no curve graph; the curve is refused (`TrackCurvesUnsupported`) and the model is untouched
- **PATH** effects window → the Geometry slot → "Masks" → Rectangle, drag → Ellipse, drag → in the ellipse's row the word "Subtract" → ↑ → the hole is cut first → track menu → Properties… → "Masks" → Rectangle, drag → the track is cut; its mask row has the numbers but no stopwatch
- **TEST** t1281 (`src/apps/kt-app/src/mask_tests.rs`), t1280 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: Mask texts in every catalog; the live path
- **WHEN** the catalogs are checked for the mask keys; the live scenario runs on our fixture
- **THEN** every key exists in all five catalogs (the glyphs are held by t100 over every catalog value); the live scenario draws a Bézier mask, feathers it, keys it at two ticks, inverts it and limits an effect to a mask, with a screenshot per step
- **PATH** right click on the picture → Mask → Pen → four points, close → Feather 40 → stopwatch, move the playhead, drag a point → Invert → effects window → the effect's Masks → Ellipse → drag → the effect only inside
- **TEST** t1283 (`src/apps/kt-app/src/mask_tests.rs`), live scenario `src/scripts/live/masks.py`

### Requirement: The preview's right-click menu opens over the transform frame too
The preview's right-click menu (the frame's switch, Mask → Rectangle / Ellipse / Pen, detach) SHALL
open wherever the person right-clicks the picture, including over the R.64 transform frame and over
the masks being edited: one menu body, attached to the picture and to the controls that cover it.
Why: with the frame on by default (R.67) the frame covered the whole picture and took the right click
for itself — the menu, and with it the masks' entry, opened only on the letterbox around the picture.

#### Scenario: Right click on the frame opens the preview menu
- **WHEN** the frame is on over a full-frame event and the picture's centre is right-clicked
- **THEN** the menu with the Mask item opens; Mask → Rectangle arms the tool
- **PATH** select an event → right click in the middle of the picture (on the frame) → the menu shows "Mask" → Mask → Rectangle → the cursor is a cross over the picture
- **TEST** t1276 (`src/apps/kt-app/src/mask_tests.rs`)

### Requirement: File › Preferences… holds the program's preferences; View keeps what is viewed
File › Preferences… SHALL open one registered window with the program's preferences — per machine,
outside any project, reachable with no project open — in sections: Start ("Open the welcome window at
start"), Interface (Language: Auto (system) and the five languages by their own names; Economy UI),
Privacy and updates ("Send anonymous usage statistics", then "Check for new versions daily", each
disabled with its reason in the hover text when the build or the environment blocks it), FFmpeg (the
loaded version or "not found", and FFmpeg… opening the FFmpeg window). Each control SHALL act at once
(no Apply), with its hover text saying what it does. The View menu SHALL hold only what changes what is
viewed: Detach/Dock preview, Diagnostics…, Notices. The window follows the shared window rule
(resizable, its body fills it, Close in the action row).
Why: kkt-26.11.01 item 13 — "пора бы формировать раздел настроек в file, а то у нас в view уже срач из
разного"; answer 3 (2026-11-01): yes. Named by industry (rule 21): Resolve, Premiere, Vegas, Blender say
Preferences; "Settings…" beside "Project settings…" reads as one window twice (design D5).

#### Scenario: The Preferences window holds every preference and each acts
- **WHEN** the harness opens File › Preferences…, switches Economy UI on, picks Русский in Language,
  unchecks "Open the welcome window at start", unchecks the statistics and the daily check, and presses
  FFmpeg…
- **THEN** the window is `kt_prefs` from the registry with the four section titles; Economy UI caps the
  playing repaint at 30 fps; the menu bar reads in Russian on the next frame; the start switch is off in
  the store; both consents are off and stored; the FFmpeg window opens
- **PATH** File › Preferences… → a window "Preferences" with Start, Interface, Privacy and updates,
  FFmpeg → tick "Economy UI" → pick "Русский" → the menus read "Файл / Правка / Вид" → untick "Open the
  welcome window at start" → untick both privacy boxes → press "FFmpeg…" → the FFmpeg window opens over it
- **TEST** t1293 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

#### Scenario: View holds only what is viewed
- **WHEN** the View menu is opened in each of the five languages
- **THEN** it lists exactly Detach preview (or Dock preview), Diagnostics…, Notices — no language, no
  Economy UI, no statistics or update switch, no FFmpeg…, no welcome switch
- **PATH** open View → three items: "Detach preview", "Diagnostics…", "Notices"
- **TEST** t1296 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

### Requirement: One switch, one place
A preference SHALL be changed from exactly one place in the interface. A second checkbox, menu item or
field for the same value elsewhere (a window, a menu, a panel) is a defect, also when its label
differs. The texts that tell where a switch is (the statistics entry, the update hint, the FFmpeg
notice) SHALL name that one place.
Why: kkt-26.11.01 item 13 — "нафига мне галочка welcome window at start И в самом window и еще
продублирована в View?!?!"; the diagnostics change found the same class with Economy UI (a copy in the
perf box). The two start checkboxes had different labels, so the guard watches the writers (design D9).

#### Scenario: Each preference has one writer and the old places hold none
- **WHEN** the kt-app sources outside tests are scanned for the preference writers (the start switch,
  Economy UI, the language, the statistics consent, the update consent), and the harness opens the View
  menu and the welcome window
- **THEN** each writer is called from exactly one site, in the Preferences window's source; neither the
  View menu nor the welcome window paints any preference label; in every catalog the statistics entry,
  the update hint and the FFmpeg notice name File and Preferences…, not View
- **PATH** open View → no switch; Help › Welcome window… → no checkbox, a "Preferences…" button; File ›
  Preferences… → every switch, once
- **TEST** t1294 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

### Requirement: Preferences survive opening another project and a restart
The language choice and Economy UI SHALL be kept with the program's preferences in the user store, not
in the open project's app state: File → New, File → Open, a template, a recent card and a lesson SHALL
keep them, and the next start SHALL apply them. A language chosen by an earlier build (kept in eframe
storage) SHALL be taken once when the store has none.
Why: found by this change — `replace_with` rebuilt the app without the two fields, so after File → New
the stored language became "auto" and the next start came up in the system language.

#### Scenario: File → New keeps the language and Economy UI; the restart applies them
- **WHEN** Russian and Economy UI are chosen in Preferences; File → New; the store is read again as on a
  restart; a store without the language and an eframe value `es` are given to the start
- **THEN** after File → New the language choice is Russian and Economy UI is on; the reread store holds
  both; the start without a stored language resolves Spanish from the former value
- **PATH** File › Preferences… → "Русский", tick "Economy UI" → File › New → the menus stay Russian,
  Preferences still shows both → quit and start → Russian, Economy UI on
- **TEST** t1295 (`src/apps/kt-app/src/welcome_prefs_tests.rs`)

### Requirement: The pitch panel says what happens to the echo before it
The pitch effect's panel SHALL show the choice "Echo of the effects before the pitch" with the two
options "cut at the event's end" and "rings on after the event", in the person's language, with a hint
that says what each does (cut: the joins are seamless, the echo of a reverb before the pitch ends with
the event; rings on: the echo continues after the event as without the pitch, the joins stay as they are). A choice SHALL be one undoable edit of the slot.
Why: the owner asked for both behaviours in his own terms; a switch named in the engine's words would
repeat the unanswerable question.

#### Scenario: The switch is found in the pitch panel and changes the slot
- **WHEN** the FX window of an event with a pitch is drawn, the echo choice is set to "rings on", then undone
- **THEN** the panel shows the label, both options and the hint in every catalog; the slot's row becomes "ring" in one undo step and returns to "cut" on undo
- **PATH** double-click the event → the FX window → click the Pitch Shift slot → above the semitones the line "Echo of the effects before the pitch: cut at the event's end"; hovering it puts the hint in the status line → open it, pick "rings on after the event" → the line shows the new choice → Ctrl+Z → back to "cut"
- **TEST** t1337 (`src/apps/kt-app/src/pitch_tail_tests.rs`)

#### Scenario: Live — the switch in the real program
- **WHEN** the real program opens our fixture project with a pitch on an audio event, the FX window is opened by a double click, the echo choice is switched to "rings on" and the project saved
- **THEN** a screenshot per step shows the window, the choice before and after; the saved `.kt` carries `"echo_before": "ring"` and is format 3
- **PATH** double-click the audio event → the FX window (Delay, Pitch Shift) → click the Pitch Shift slot → the echo line "cut at the event's end" → open it → pick "rings on after the event" → Ctrl+S
- **TEST** t1338 (`src/scripts/live/pitchtail.py`)

### Requirement: The FFmpeg window tells what the download does, how fast, and where it went
Before the click on **Download FFmpeg <pin>**, the FFmpeg window SHALL say what will happen: the
archive's name and size; the source and its mirror; the check against the SHA-256 built
into Kinetren (shortened, the full value in a hover); that only the DLLs and the licence
are unpacked, and into which folder; that nothing else on the computer changes; and that
deleting the folder removes it. During the download the window SHALL show the host in use with
"mirror k of n", the received and total size, the speed and the time left (a rate smoothed
over the last seconds; a dash while nothing arrives), the verifying step, and "unpacking: n
of m files". After a success the window SHALL name the installed folder and the verified hash. After a
failure it lists one line per mirror tried, with that mirror's reason. An **Open folder** button
SHALL open the installed folder, or the folder of the loaded FFmpeg, in the system file
manager. The texts come from the five catalogs. The window repaints only while the
download runs (invariant 12), and the speed and time left are computed by a pure function of the
samples.
Why: `kkt.txt` 2026-10-27: "the FFmpeg download window is not informative — what speed,
where it installs, etc." The person clicked Download and saw only "Connecting…" and
"X of Y MB". The source, the mirror, the folder and the reason of a failure stayed inside
the program.

#### Scenario: Speed and time left are honest numbers
- **WHEN** the rate function is fed byte samples: steady 4 MB/s for 5 s; a burst then a 3 s stall; no announced total (the pinned size is used); the first sample only
- **THEN** steady gives 4.0 MB/s and the time left from the remaining bytes; the stall gives a dash for the speed and no time left (not an infinite or frozen value); without a total the pinned size is the total; one sample gives a dash; the function allocates nothing per frame
- **PATH** View → FFmpeg… → **Download FFmpeg 9.0.2** → the bar reads "12 of 57 MB · 4.0 MB/s · about 11 s left" → the network stalls → the speed shows "—" and the time left disappears instead of growing → bytes arrive again → speed and time left come back
- **TEST** t948 (`src/apps/kt-app/src/ffmpeg_setup.rs`)

#### Scenario: Before the click the window says what will happen
- **WHEN** the harness opens View → FFmpeg… with the Windows rules forced and no pin folder, the download root pointed into the test's own folder
- **THEN** the window shows the archive name `ffmpeg-9.0.2-full_build-shared.7z`, its size in MB, the hosts `www.gyan.dev` and `github.com` (as the mirror), the first 16 hex digits of the pinned SHA-256 (the full value in the hover), the folder `<root>/9.0.2`, and the line about removal; the **Download** and **Open folder** buttons are there; nothing repaints while the window rests
- **PATH** View → FFmpeg… → the window shows the current state, then the block "Download gets ffmpeg-9.0.2-full_build-shared.7z (57 MB) from www.gyan.dev (mirror: github.com), checks SHA-256 4d2060a8b34a940a…, unpacks only the DLLs and the licence into …\kinetren\ffmpeg\9.0.2; nothing else changes; delete the folder to remove it" → hover the hash → the full 64 digits → the buttons **Download FFmpeg 9.0.2**, **Open folder**, **Locate folder…**, **Rescan** are in the row below
- **TEST** t950 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: During the download the window shows source, speed and time left
- **WHEN** the harness starts the download from a local test server pair (the first mirror answers 404, the second serves the t848 archive slowly)
- **THEN** while bytes arrive the window shows "127.0.0.1 — mirror 2 of 2", "X of Y MB · S MB/s · about T left", and under "Tried:" the first mirror with "404"; then "Checking SHA-256…" and "Unpacking: n of m files"; the bar and the text move; **Cancel** is shown instead of **Download**
- **PATH** View → FFmpeg… → **Download FFmpeg 9.0.2** → the bar appears with "127.0.0.1 — mirror 1 of 2, connecting…" → a line "Tried: 127.0.0.1 … — 404" appears and the bar shows "mirror 2 of 2" → "1 of 3 MB · 0.5 MB/s · about 4 s left" grows → "Checking SHA-256…" → "Unpacking: 2 of 3 files" → the bar is gone
- **TEST** t951 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: After the download the window says where it went, or why it did not
- **WHEN** the harness download from the test server succeeds; then a second harness run where both mirrors fail (404, then a hash mismatch); in both runs **Open folder** is pressed
- **THEN** after the success the window shows "Installed into <root>/9.0.2", "SHA-256 … matches", and (with FFmpeg already loaded) the existing "restart to use it" line of ffmpeg-9; **Open folder** asks the file-manager seam to open `<root>/9.0.2`; after the failure the window lists both mirrors, each with its own reason (404; "checksum mismatch: expected …, got …"), the folder has no traces, and **Open folder** opens the download root
- **PATH** (continuing the previous path) the bar is gone → the window shows "Installed into …\kinetren\ffmpeg\9.0.2" and "SHA-256 4d2060a8… matches" → **Open folder** → Explorer opens the folder with the DLLs and the licence → (another run, no network) **Download** → after a while red "FFmpeg was not installed:" with two lines "www.gyan.dev — …" and "github.com — …" → **Open folder** → the download folder, without leftovers
- **TEST** t952 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The new texts are in every catalog and every font
- **WHEN** the catalog and font guards run over the new `[ffmpeg]` keys
- **THEN** all five catalogs have every key with the same positional placeholders, no extra or unused keys, every non-ASCII glyph is in the loaded fonts, and t100, t125, t128 and t171 stay green
- **PATH** View → Language → Arabic (then Chinese, Russian, Spanish) → View → FFmpeg… → the "what happens" block, the progress line and **Open folder** are in that language with no tofu boxes and no English left over
- **TEST** t953 (`src/apps/kt-app/src/i18n_tests.rs`, `src/apps/kt-app/src/font_tests.rs`)

### Requirement: A track is reordered by dragging its header, and the header says so
Pressing a track header's free area (outside its badges and its height edge) and dragging SHALL
reorder the track (one undo step, the same body as the menu's reorder). Hovering that area SHALL show
the grab cursor and the status-line hint with the track's name and "drag — reorder". The test SHALL drag from the places a
person grabs: the name and the empty part above the badges. It SHALL assert the new row order.
Why: the owner, `ktprompts_v26.10` TIMELINE 6: "I would like to drag the order of tracks with the mouse
at the side". The gesture was dead since R.38: the release of a height drag took the drag out of
its own `if let` chain whatever it was, so a reorder never reached its branch. Nothing told the person,
no test held it, and the R.62 badges took most of the header.

#### Scenario: Dragging a header moves the track
- **WHEN** three video tracks; the second track's header is pressed on its name and dragged below the third in three steps; separately — hovered
- **THEN** the order is 1, 3, 2 and one undo restores it; the hover shows the grab cursor and the hint
- **PATH** hover a track's name at the left → the cursor becomes a hand, the status line says "drag — reorder" → drag it down past the next track → a blue line shows the place → release → the track is there
- **TEST** t1637 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Input that changes nothing does no work
A frame of drawing, a pointer moving, a window or panel being open or selected, and a gesture that
holds still SHALL change no model, bump no revision, push no undo step, hand the player no project,
rebuild no renderer, wake no prefetch and order no decode. A number field SHALL write only from a human
gesture (a drag, keyboard input, a click on its slider, a reset), never because the widget clamped or
rounded a value on display, and a value its carrier already holds after its own clamp and cast SHALL
be no edit. A colour button SHALL write only a colour that differs from the one held. A live drag
SHALL hand the player a project only on a frame where the value moved.
Why: the owner, kkt-26.11.07b G5/G7: "открываю эффекты события этого, и у меня 100% херачит prefetch
когда я двигаю мышкой", and with VHS Tape and Camera ISP switched off "любое движение мышки … тоже
приводит к нагрузке". His VHS Tape held `dark_clip = -0.8`, the floor; the row is stored as f32, -0.800000012,
under the floor, so the percent field clamped it on every frame and read "changed", the window wrote it
back and each mouse move rebuilt the whole engine: 447 ms of audio renderer on his 1974 events, on the
audio render thread and again on the prefetch thread. Slowed+Reverb, Chromatic Aberration and Camera
ISP at their ceilings did the same.

#### Scenario: No field writes from a frame
- **WHEN** every number row of every effect, transition, generator and track passport stands at its floor and then its ceiling as the carrier stores it, and the shared grid is drawn for frames with the pointer moving over it
- **THEN** no frame reports a change and no value moves
- **PATH** open any effect's window with a number at the end of its range → the field shows it; move the mouse over the field and the grid → the value stays, nothing is edited, Undo has nothing new
- **TEST** t1870 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: An effects window under the pointer changes nothing
- **WHEN** the effects window is open over an event carrying any effect kind, its numbers at a bound, that slot selected (the VHS Tape deck, the Camera ISP pipeline and the codec's stream panel included), the owner's own VHS values first, and the pointer sweeps the window
- **THEN** the app's and the model's revisions, the undo depth and the player's project version stay as they were
- **PATH** double click an event carrying VHS Tape → the effects window opens on its chain; click the VHS Tape slot → the deck face and its grid show; move the mouse over the window → nothing but the hover changes, the processor stays at rest
- **TEST** t1871 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: Every editing window under the pointer changes nothing
- **WHEN** the transition window (every kind, at both bounds), the track window (video and audio, at both bounds), the camera window and the morpher window are open and the pointer sweeps them
- **THEN** no revision, undo step or player project moves
- **PATH** double click a transition → its window opens; move the mouse over it → nothing changes; the same over a track's window, a camera's and a morpher's
- **TEST** t1872 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: A held level drag hands the player nothing
- **WHEN** an event's level line is grabbed and the pointer then holds still or moves along the line for frames
- **THEN** the player is handed a project only on frames where the level moved
- **PATH** grab the level line of a selected event → the line follows the pointer; hold the mouse still → nothing is rebuilt; move it up → the level follows, once per change
- **TEST** t1876 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: A colour written back is the colour held
- **WHEN** an effect's colour row holds any 8-bit colour and the colour button gives it back through its floating-point round trip
- **THEN** the written value equals the held one, so nothing is sent
- **PATH** open Green Screen's window and click its key colour → the colour popup opens; move the mouse over the popup without pressing → the colour stays as it was, nothing is edited
- **TEST** t1877 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: The VHS deck selected, a number at its floor, the pointer moving, on the heavy project
- **WHEN** the heavy showcase is open, a composition's effects window shows its chain, the VHS Tape slot is selected, the dark clip field is dragged to its floor (-80 %), the program has gone quiet, and the pointer sweeps the deck panel
- **THEN** no edit, project swap, engine build, prefetch pass or decode runs, and the prefetch and audio render threads hold under 5 % of a core each, measured by their processor time
- **PATH** right click the composition → Effects… → the window shows Slowed, VHS Tape and Camera ISP; click VHS Tape → the deck face shows; drag the dark clip number far left → it stops at -80 %; move the mouse over the deck → only the hover changes, the engine stays asleep
- **TEST** live `idle` (`src/scripts/live/idle.py`)

### Requirement: Diagnostics shows the journal at any window height, and the moment can be marked
The Diagnostics window SHALL show the telemetry journal's tail, newest at the bottom, each record with
its clock time, in a pane outside the window's scroll that takes the lower part of the body at every
window size down to the window's minimum, so it is readable without enlarging the window. Its head row
SHALL hold "Mark the moment" with an optional note field (Enter marks too) and "Open folder", which
opens the data directory. Ctrl+Alt+M SHALL mark the moment from anywhere in the main window, in every
key style. Notices and the journal show local time.
Why: the owner, kkt-26.11.07b line 17 (G12, found-defects 109): "ОКАЗЫВАЕТСЯ НАДО ОКНО ПО ВЫСОТЕ
УВЛИЧИТЬ И ТОООГДАДАА ОНО ПОЯВИТСЯ"; and he could only guess when a stall happened.

#### Scenario: The journal is on screen at the default and the minimum height
- **WHEN** the Diagnostics window opens at its default size, and again at its minimum size
- **THEN** the journal pane is inside the window's visible body, shows at least six lines, and its mark button is clickable
- **PATH** View → Diagnostics… → the window opens; at its bottom the journal lists the last records with their times, with "Mark the moment", a note field and "Open folder" above them; drag the window's corner up to its minimum → the journal is still there, the graphs above scroll
- **TEST** t1968 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: A mark from the button and from the key
- **WHEN** the person types a note and presses "Mark the moment", then presses Ctrl+Alt+M over the timeline
- **THEN** the journal gets two MARK records in order, the first with the note, and the note field empties
- **PATH** View → Diagnostics… → type "stall here" in the note field → press "Mark the moment" → a line "MARK stall here" appears at the bottom of the journal with the time; close the window, press Ctrl+Alt+M → the status line says the moment is marked
- **TEST** t1969 (`src/apps/kt-app/src/diag_tests.rs`), live `journal` (`src/scripts/live/journal.py`)

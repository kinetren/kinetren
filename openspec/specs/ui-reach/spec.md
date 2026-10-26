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
- **WHEN** the "Generator" submenu is opened from the main menu and from the bin's background menu
- **THEN** the list of kinds is one for both entries, and creation puts the media and the clip in one history step
- **TEST** t560 (`src/apps/kt-app/src/reach_tests.rs`)

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
Why: R.63, the owner's words: "I forbid any agents to spawn any additional windows without an obvious purpose (like the window on closing the program)";
"why does the agent… force him to enter all the parameters again"; "duck under a track…
why didn't you simply name it properly"; "meaningless copy-pastes of the same code keep piling up everywhere". R.65: the ducking snapshot was taken together with its own parameter memory — a live instrument has its memory by construction, and a second one (in the store) would diverge from the project. R.66 (the owner's direct request of R.64 "go over this in all the other widgets and windows too"): prose about the look of a form does not work — by that point the scrolling of the body existed in exactly one window out of twenty-one, and it was added only after a test dropped a window with buttons hidden behind the edge.

#### Scenario: The window registry
- **WHEN** the kt-app sources are scanned for `egui::Window::new` and `Modal::new`; a window without an entry is temporarily added; the entry of a live window is removed from the registry
- **THEN** each place of window creation is found in the registry with a reason of at least 25 characters; both violations are red; the windows `duck` and `inst_params` are absent from the registry and do not exist in the code
- **TEST** t757 (`src/apps/kt-app/src/reach_tests.rs`)

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
*View → FFmpeg…* SHALL open a window that shows the search report (capability
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
once per session, and the window stays reachable from the View menu at any time. The window follows
the window-form contract (scrolling body, action buttons outside it).
Why: the owner's decision to find the user's install and offer the gyan.dev download
(2026-10-22) needs a place where a person sees why nothing plays and fixes it with the
mouse. A notice without buttons would be a dead end ("create, but can't manage", rounds
33–40).

#### Scenario: From the notice to playing media
- **WHEN** the app starts with the FFmpeg search limited to an empty folder and a project with one video clip; the notice's **Locate folder…** gets a folder without DLLs, then a folder with a complete set; then *View → FFmpeg…* is opened. On Windows the complete set is the pinned gyan.dev 9.0.2 build. On Linux the test's search seam reports that folder as complete and loads the system FFmpeg behind it, so the same UI path runs in CI on both OSes
- **THEN** before: the notice is shown once, the clip is a placeholder, the window lists the empty folder as rejected; the folder without DLLs is refused with "no avcodec-*.dll" and the choice is unchanged; after the complete folder: the clip shows frames without a restart, the window shows the folder and the loaded version, and the choice is in the saved settings
- **PATH** start → the FFmpeg window over the timeline: "FFmpeg not found — media shows placeholders", the empty folder under "Not used", the clip drawn as a placeholder → **Locate folder…** → folder picker → a folder without DLLs → red line "… cannot be used: no avcodec-*.dll" → **Locate folder…** → the folder with DLLs → the window's first line becomes "FFmpeg <major> (avcodec <x.y.z>) — <folder>" (on Windows "FFmpeg 9 (avcodec 63.x)"), the clip's preview shows frames → close the window → View → FFmpeg… → the same window with the loaded folder
- **TEST** t850 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Download stays offered over an earlier download
- **WHEN** the window's offer is computed (Windows rules) for: nothing loaded; FFmpeg 8 loaded from the download root's `8.1.2\` with no `9.0.2\`; FFmpeg 9 loaded from `9.0.2\`; FFmpeg 9 loaded from a chosen folder with no `9.0.2\`
- **THEN** **Download FFmpeg 9.0.2** is offered in the first, second and fourth case and not in the third; in the second the window's first line names the `8.1.2` folder; the computation is pure and runs on any OS
- **PATH** (Windows, an 8.1.2 download from an earlier version of Kinetren) start → media plays as before, no notice → View → FFmpeg… → the window's first line "FFmpeg 8 (avcodec 62.x) — …\kinetren\ffmpeg\8.1.2", below it **Download FFmpeg 9.0.2 (gyan.dev, GPLv3)**, **Locate folder…**, **Rescan** → **Download** → progress bar with size and **Cancel** → after it finishes the first line becomes "FFmpeg 9 (avcodec 63.x) — …\kinetren\ffmpeg\9.0.2" only after a restart (a loaded FFmpeg stays for the process), and the Download button is gone
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
- **PATH** there is no gesture: the first start of `kinetren.exe` on a Windows account whose display language is Russian (no LANG set) shows the menu bar, toolbar hints and dialogs in Russian at once; after View → Language → English the choice wins on every next start
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

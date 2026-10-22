# Spec Delta

## MODIFIED Requirements

### Requirement: Effect windows and input fields: one widget per meaning
A numeric parameter SHALL be drawn only with the common widgets of the parameter row/grid, with the range, step and unit from the model's spec; the captions come from the catalogue.
The chain strip SHALL be one per event and one per morpher (a master checkbox, a checkbox and a cross on the slot, reordering by mouse, copy-paste); for a morpher the strip is one and begins with the pinned morph-slot. The FX window SHALL run the whole link group in one session and one undo step; "Cancel" rolls back all chains. An edit of a parameter under a live curve SHALL be a curve command, not a base edit, and the test SHALL assert the model through the characters typed into the field. The morpher's source field SHALL accept a file from the system, a bin card, a picker with search, and a dialog. Curves SHALL be drawn by ONE automation widget (graph ↔ table); the axis is the material of the event, the value scale is by the keys of the curve; editing time sorts only on release.
Input SHALL be drawn in the same rectangle and font as the value it replaces ("you can't just slam an inputfield in there"); everything that takes focus gives it up on closing.
The focus of an input field SHALL be removed by ONE rule for the whole program: a pointer press outside the rectangle of the focused field (on the timeline, the preview, the toolbar, another window) gives up focus in the same frame, and the hotkeys (Space, M, Del, Ctrl+Z…) act immediately, with any window open. A field MUST NOT take focus back without a new click into it.
Why: Gain was −24…24 in one window and −48…24 in another; slowed drew a graph, pan3d — a table; "I enter a value in the rate field, and it snaps back to the previous value"; Ctrl+L after Enter killed all hotkeys. R.63: egui gives up the focus of a `TextEdit` only on Esc/Tab/another focusable widget; a click on the timeline (`Sense::click_and_drag`) does not touch it — "no matter how I click on the rest of the interface, space does not pause until I close the effect window".

#### Scenario: One chain strip, fields per the specs
- **WHEN** the event window and the morpher window are open
- **THEN** the master checkbox is present in both (the widget is shared); the batch slot's switch removes the effect from the targets without losing it; the field ranges match the spec
- **TEST** t217 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The morpher source field
- **WHEN** a file is dropped into the field, a bin card is dropped, or a file is chosen with the picker
- **THEN** a file that was not there is added and substituted in one undo step; "Cancel" rolls back the session
- **TEST** t218 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: One window per link group
- **WHEN** FX is opened from the partner of a pair
- **THEN** a second window is not created; the window runs both chains in collapsed sections; rollback returns both; for the video instance of the warp the reverb handles are hidden and the frame mode is shown; the reverb handles are visible exactly once
- **TEST** t219, t248c (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Automation in the FX window — one history step
- **WHEN** a curve is added, keys are placed, slots are moved, Cancel is pressed
- **THEN** the curves run in parallel with the chain; the whole session is one undo step; Cancel returns both the chain and the curves
- **TEST** t193 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Automation widget: mapping and stable input
- **WHEN** a keyframe on a forward, a reversed and a sped-up event is converted material ↔ timeline; in the table "t=0.5" is printed after "t=1.0"
- **THEN** the conversion converges in both directions; the rows are not re-sorted until release
- **TEST** t221 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A key value typed into the field reaches the model
- **WHEN** in the live FX window "3.0" is typed into the parameter field with a live curve (the coordinate comes from the real layout; DragValue has no accessibility node)
- **THEN** the curve in the model contains 3.0 (on the old code — 1.0); a keystroke that does not change the value produces no command
- **TEST** t281 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The transition window
- **WHEN** a double click on the transition window; choosing the kind and the audio curve; "Cancel"; the transition dies
- **THEN** the kind and the curve reach the model through the same parameter grid; rollback returns the original kind; the session is one history step; the window of the dead transition closes by itself
- **TEST** t227 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Camera window
- **WHEN** a double click on the camera block; edits; the camera is deleted
- **THEN** the window opens; the edits are one undo step; the window of the dead camera closes
- **TEST** t231b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Morpher window: targets, time, one strip with the morph slot
- **WHEN** the morpher is open in "below" / by-targets mode; "+M" is pressed on the toolbar
- **THEN** the mode and the explanation are visible; the "by time" checkbox is one history step; the frozen list is shown by name; "+M" adds a layer on top in one step; the strip is one, the morph slot is first and cannot be reordered; the effects of both kinds lie in it and are sorted back by kind
- **TEST** t242, t244 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Insert prefix window
- **WHEN** Ctrl+V with a named item in the buffer; Esc; a second Ctrl+V; Enter
- **THEN** without a named item there is no window; Esc leaves no trace and gives up focus; the second Ctrl+V opens the window again; confirmation is one history step with a frozen paste position
- **TEST** t250 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Presets and paired widgets (capability `automation`)
- **WHEN** a chain/preset is pasted into the morpher; the preset name window; the frequency response graph is dragged and scrolled with the wheel
- **THEN** the curves travel with the chain (a silent branch is impossible by type); the name window follows the rules (it grows from input — the buttons are to be probed after the redraw); the graph writes to the same place as the field; the wheel is absorbed only at its own point
- **TEST** t343, t345, t346, t349, t350 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A click past the field releases focus, keys stay alive while a window is open
- **WHEN** in the FX window one clicks into the formula field and types; then clicks on the timeline (an empty spot), on the preview, on the toolbar; presses Space; the same with the name field in the track window and the search field of the picker; again clicks into the field and types
- **THEN** after a click outside, the field no longer has focus; Space starts playback (on the old code — no, while the window is open); the text typed before the click is kept; the field accepts input again after a click into it; the test presses from the person's point and asserts `is_playing()`
- **TEST** t750 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/formula.py`

### Requirement: Shapes, derived graph and formula in the automation widget
The shape of a segment SHALL be edited from the same automation widget as the keys: by a right click on the SEGMENT between two keys (neighbouring segments are not touched), with the shape's parameters shown next to it; the same actions are available by selecting several segments. The target of a right click (a segment or a key) SHALL be fixed AT THE MOMENT of the press and held while the menu is open: moving the pointer toward the menu items MUST NOT close it and MUST NOT change the target; the segment under the menu SHALL be highlighted on the graph, and the menu header SHALL name its boundaries (the times of both keys of the material and the share of the event) — the person sees BETWEEN WHAT they are setting the shape. The graph SHALL draw the shape the way it sounds and looks in the render, with the same value code — there is no second drawing formula. A curve with a DERIVED source (a link or a formula) SHALL be drawn distinguishably from a keyed one and MUST NOT be edited with the mouse: it has no keys, and dragging a point would silently create a third truth about the value; at the same time its VALUES SHALL be drawn — resolved by the same code as the render (`resolve_derived`), not from an empty list of keys. Such a curve SHALL show what drives it — the source address or the formula text — and have a single action "unlink" that turns it into ordinary keys at the current values. The formula field SHALL hold the TEXT BEING TYPED between frames (the text lives in the widget's state, not re-read from the model each frame): an unfinished input is not erased; a parse error is shown with its position and is not applied to the model; the text is applied to the model on confirmation (Enter, loss of focus) — and MUST NOT be applied on the first legal prefix (typing `t*2` does not turn the curve into "t" after the first letter; the keys are not zeroed before confirmation). The shape's default and the command "apply to all" are reachable from the same place as the other actions on the curve. All new controls SHALL have a hint in the status line.
Why: the owner asked for "the resulting graph is drawn too, but not edited with the mouse"; an editable derived graph would have been exactly the "silent dead end" for which the project paid seven rounds. R.63: "I type it in, and it either erases the formula or nothing happens" — the field re-read the model every frame, applied on the first legal letter and zeroed the keys, while the graph drew empty `keys`; "the context menu just disappears, I don't have time to reach it" — the target was computed from `hover_pos()` every frame.

#### Scenario: The shape is edited per section, neighbours intact
- **WHEN** a shape is selected by a right click on a segment, then its parameter is edited, then the shape is applied to all segments of the curve
- **THEN** only the selected segment changes, and only its parameters; the graph is redrawn by the same value function the render computes; "to all" is one undo step
- **TEST** t698 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A derived curve is visible but not edited with the mouse
- **WHEN** the parameter's source is a link; a drag and a double click are made along its line; then "unlink" is pressed
- **THEN** the line is drawn distinguishably and labelled with its source; neither the drag nor the double click changed the model; "unlink" gave ordinary keys at the current values in one step
- **TEST** t706 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The formula field explains the error and does not spoil the model
- **WHEN** into the formula field a text with an unclosed bracket is typed, then one with an unknown name, then a correct one
- **THEN** the first two show an error with its position in the line and do not touch the model; the correct one is applied; the field has a hint in the status line
- **TEST** t709 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The formula is typed in full, the keys stay alive until confirmation, the curve is drawn
- **WHEN** for a curve with three keys, the formula field is typed character by character `sin(t)*0.5` with pauses of several frames, then Enter; then `t*` is typed and focus is moved away; then the text is erased
- **THEN** after each character the field shows what was typed (on the old code `sin(` vanished on the next frame); until Enter the curve's keys are intact and the model is untouched (on the old code `t` was applied with the first letter and zeroed the keys); after Enter the source is the formula, and the graph shows a line with a non-zero number of points, equal to `resolve_derived` at the same ticks; the unfinished `t*` is not applied when focus leaves, and the error is named; empty text returns the keys by "unlinking", not by silence
- **TEST** t749 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/formula.py`

#### Scenario: The section menu stays under the pointer and says what the shape is between
- **WHEN** a right click on a segment between keys at 1.0 s and 2.0 s; the pointer is moved 40 px toward the menu and the shape is clicked; a right click on a key and the same movement toward the item "Delete"
- **THEN** the menu did not close from the movement (on the old code it vanished on the first frame outside the segment); the header names 1.000 and 2.000 s and the shares of the event; the segment is highlighted (a backdrop between the x keys is drawn); the shape landed on this segment; the keyframe menu stays the same way and deletes exactly that key
- **TEST** t751 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/formula.py`

### Requirement: Compositions in the interface (capability `compositions`)
Gestures over reels SHALL obey the common layout rules: what plays is decided by one place; entering and exiting are an identity by view and cursor; Enter is a cascade inward, Esc is a cascade outward; the selection lives in one reel; inside a reel, zero is not a wall for gestures but is a wall for the cursor; the preview of a reel is ordered once per key and does not depend on scrolling; badges and the author's names are drawn verbatim.
The composition parameters of an INSTANCE SHALL be edited in the FX window of that instance — in the pinned head of the chain's strip (with the same machinery as for a generator: `HeadSlot`, `ChainSel::Head`), where the author's handles lie with their widgets by type, the reset, and the same automation section; there MUST NOT be a separate parameter window of the instance; the menu item and the double click on the instance lead to this window with the head selected. The AUTHOR's "Public parameters" window (inside the reel) remains.
Why: the content contract is in the capability `compositions`; only the layout and interactions are fixed here. R.63: "the composition parameters open in a separate window when they could be placed in the same fx-chain" — and the owner's ban on windows without an obvious purpose.

#### Scenario: Entry, exit, cascades and minimap
- **WHEN** they collapse and expand by hand; they enter by four paths and exit with Esc; Enter is pressed twice
- **THEN** the view and cursor return identically (a scene with a SHIFTED instance); Esc exits last; the minimap and the ×N badge are in place; inside a reel the reel plays; entry brings the view to the cursor
- **TEST** t430b, t456, t457, t458, t552, t553, t597 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Expansion strip in place
- **WHEN** they expand in place, edit from the strip, click in another strip, press Esc; they expand inside the full entry; they change context
- **THEN** the document and what plays do not change; the edit reaches the reel; the selection moves with a message; Esc collapses the strip before exiting; the strip opens inside an entry too and survives a change of context
- **TEST** t570, t571, t572, t573, t634, t636 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Zero inside a reel
- **WHEN** an event inside the full entry is dragged left of zero; the view is scrolled to zero in the root and in the reel
- **THEN** the event goes past zero, the cursor never does; the view goes to zero only in the reel; there are no bare `.max(0)` over the gesture time
- **TEST** t637, t637b (`src/apps/kt-app/src/layout_tests.rs`), t637c (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Bin, project drop, insertion by click
- **WHEN** an `.otio` is dropped onto the window; a composition is pasted from the strip to the click point; a track is added under the ceiling
- **THEN** the program asks and does not lose work; pasting a composition is one step; the pair lands at the click point; the new track is the top layer; the reel is a bin object with all the verbs
- **TEST** t576, t577, t578, t594, t596 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Reel preview is a service, not a drawing
- **WHEN** the reel is visible on the timeline, scrolled and zoomed; the project is opened a second time; instances are overridden differently
- **THEN** the request is placed once per key; scrolling orders no work; the second opening — zero renders (the disk tier); the preview key is per instance
- **TEST** t599, t601, t640, t680 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Instance parameters window and badges
- **WHEN** the FX window of an instance is open (double click, menu item "Composition parameters"); the handle's name is set by the author; overrides are zero/N; a number and a colour are edited, and reset is pressed; a curve is created via "Automation: add curve…"
- **THEN** the strip's head is pinned first and shows the author's handles, each type with its own widget; reset sends `ClearOverride`; the name is drawn verbatim (the galley text, not the accessibility tree); the pin and the number appear only when non-zero; the curve is visible with the same automation widget, the keys are in the material of the instance; there is no separate parameter window (`InstParamsWin` removed); the assertions of the earlier t683/t684/t685/t691 hold on the strip's head
- **TEST** t754 (`src/apps/kt-app/src/layout_tests.rs`), t683, t684, t685, t691 (`src/apps/kt-app/src/layout_tests.rs`, moved to the strip's head by the owner's decision in R.63), t693 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Leading node in the chain strip and thumbnails in source proportions
The chain strip of a video event SHALL show the lead node "Geometry" first when created, with the lead flag; the cross on the lead node SHALL give an error with a reason, not be greyed out; the catalogue SHALL contain "Geometry" as an addable node; the conform rule SHALL be localised in all five catalogues. The captions of the lead node's fields SHALL carry the same publication door as the captions of any slot (address `ParamOwner::Geometry`): inside a reel, a right click on the caption of the lead "Geometry"'s zoom, position, crop, rotation or anchor publishes a handle, as the second "Geometry" node in the chain does. Filmstrip thumbnails SHALL be ordered as the source view in the thumbnail's frame and drawn with margins in the colour of the row; scrolling and zooming still do not generate decode requests.
Why: the `Fit/Fill/Stretch` combo box showed `Debug` names and did nothing; the filmstrip of a vertical clip was squashed — the first place where the user could see the defect. R.63: "for what reason can the first geometry not publish its parameters, while the second one can?" — for the lead node the address is the event's field `geometry`, and there was no `ParamOwner` variant for it; the door was closed deliberately (`(!lead_sel).then(...)`).

#### Scenario: The leading node in the editor
- **WHEN** the chain editor of a new video event is open; the cross on the lead node is pressed; the node is dragged after Blur; a second "Geometry" is added
- **THEN** the lead node is first and marked; the cross is an error with a reason; the reordering and the second node are one history step each; the rule's captions come from the catalogue, and the glyphs are in the font
- **TEST** t716b (`src/apps/kt-app/src/layout_tests.rs`), `i18n_tests.rs`, `font_tests.rs`

#### Scenario: Changing the rule on pause rebuilds the preview
- **WHEN** the project is paused; in the settings window the rule is changed from "fit" to "fill"
- **THEN** the preview is rebuilt without any cursor movement: the pixel of the field becomes the pixel of the source; the project version is part of the preview key
- **TEST** t722b (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/conform.py`

#### Scenario: Thumbnails of a vertical clip
- **WHEN** a vertical clip is on a track with a filmstrip; the timeline is scrolled and zoomed
- **THEN** the thumbnail is in the source's proportions with margins on the sides (centred in the cell); the sidecar table `thumb_meta` carries the thumbnail size of the media (old squashed ones are not read); there are no decode orders on scroll/zoom
- **TEST** t719 (`src/apps/kt-app/src/thumbs.rs`, `src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The leading geometry is published by the same door
- **WHEN** inside a reel, a right click on the "Zoom" caption of the lead node and on the "Zoom" caption of the second "Geometry" node; outside a reel — the same
- **THEN** in both cases, inside the reel there is a publication item (on the old code the lead node has none); the handle lands in the reel with address `Geometry`; outside a reel there is no item; the instance's field shows and edits the zoom of the lead geometry, and the preview of the instance changes
- **TEST** t752b (`src/apps/kt-app/src/reach_tests.rs`), t752c (`src/apps/kt-app/src/layout_tests.rs`)

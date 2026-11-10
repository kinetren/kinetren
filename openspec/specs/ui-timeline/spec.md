# Timeline interface

## Purpose

The GUI contract of kt-app (eframe/egui): the cost of a frame and the repaint policy, the video display
clock, the layout and gestures of the timeline, the effect windows and input fields, thumbnails and the preview of
events, localisation — everything the owner demanded in the words "UI/UX is made for the
user" and that is fixed by harness tests.

## Requirements

### Requirement: Repaint only on motion, the cost of a frame is bounded
The UI SHALL request a frame only while something is MOVING: any animated state
MUST converge and release the repaint. At rest NO frame is requested: a background thread that has
something to show wakes the UI (`wake`), and a state on a clock asks for exactly the frame it needs
(`KtApp::rest_wake`). During playback the delay is 16 ms. The preview's frame key SHALL be a tick
aligned to the project's fps grid, and not a raw tick. In the draw paths there MUST NOT be allocations
or clones of structures per frame: snapshots are `Arc` clones of the caches keyed by the model's
revision, the preview's composition buffer is reused. Thousands of painter primitives (the waveform,
the meters' glow) SHALL be drawn as one mesh. UI caches invalidated by the epoch of the burst cache
SHALL be throttled during playback (the coverage bars — no more often than 4 Hz).
Why: the "burning" meters kept 60 fps while idle and heated the whole system, the raw tick
re-uploaded the preview texture 60 times a second, and the epoch changes with every insertion — during playback
this is a storm of polls under the cache's mutex (invariant 12 of the root CLAUDE.md). A tick of 4 Hz
then drew the whole window for nobody: 36 % of a core at rest on the software rasterizer, and the
owner's "ОНО ТАМ ЧЕ, БИТКОИНЫ БЛЯДЬ СЧИТАЕТ ЧТО ЛИ НА ПАУЗЕ".

#### Scenario: When idle the frames are rare, 60 fps during playback
- **WHEN** the application is idle (paused, nothing moves) or is playing
- **THEN** at rest there are no frames: the last one asks for no next frame at all, a status line and unsaved work ask for exactly their moment, and during playback the delay is 16 ms; pausing the player parks the threads
- **PATH** leave the program with nothing moving → the window stands and draws nothing; press Play → the picture and the meters move at full rate; press Pause → the meters fall and stop
- **TEST** t20 (`src/apps/kt-app/src/app.rs`), t1833 (`src/apps/kt-app/src/idle_tests.rs`), t20 (`src/libs/kt-engine/tests/player_dynamic.rs`)

#### Scenario: The meters converge and release the repaint, and go out when paused
- **WHEN** the sound has stopped or the transport is stopped
- **THEN** the meter's "activity" means the movement of the bars, not "the bar is above the minimum"; the decay converges to the floor within the normal fall time and the repaint is released; the measurement under the cursor holds only while the cursor is moved; the glow of a row depends on its own dB row, not on the level along the whole height
- **TEST** t38, t121 (`src/apps/kt-app/src/meters.rs`)

#### Scenario: The preview key is aligned to the grid, V-fps is honest
- **WHEN** the playhead moves within one frame cell
- **THEN** the preview key does not change and the texture is not re-uploaded; the V-fps counter of the performance panel counts the NEW frames shown, not the ui frames
- **TEST** t39 (`src/apps/kt-app/src/app.rs`)

#### Scenario: The meters' glow is one mesh
- **WHEN** the meters with the zonal glow are drawn
- **THEN** the glow is one vertical mesh with stops at the zone boundaries (not hundreds of ellipses), is drawn BEFORE the LED segments of both bars and does not go onto the panel
- **TEST** t40 (`src/apps/kt-app/src/meters.rs`)

### Requirement: Video display clock is interpolated; the playhead lives in audio frames
The DISPLAY position during playback SHALL be interpolated between the steps of the master clock
(the cpal callback, a quantum of up to 42.7 ms) with the smoothed output latency; the raw
playhead of the transport MUST NOT be read for display during playback. Navigation "strictly to the right of the
current position" SHALL work on values aligned upward to audio frames: the transport stores 48 kHz frames, and the round-trip "tick → frame → tick" truncates downward.
Why: without interpolation video physically did not show more than 24 frames/s at
any quality and backend; without alignment the "Forward" button silently got stuck at
one point (capability `transport`).

#### Scenario: Interpolation covers all cells of the grid
- **WHEN** the audio quantum is 2048 samples, the UI is 60 Hz
- **THEN** the display position passes every cell of the frame grid without skips; without interpolation the ceiling is 24 frames/s
- **TEST** t56 (`src/libs/kt-engine/src/player.rs`)

#### Scenario: "Forward" advances the cursor
- **WHEN** the user presses "Forward" at a stop point lying on a tick that is not a multiple of 15
- **THEN** the cursor moves on to the next point, rather than staying a tick to the left of the target; "the end" accounts for the markers and the boundaries of the region to the right of the material
- **TEST** t94 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The timecode shows the display clock and works during playback
- **WHEN** the timecode input mask (Ctrl+L) is opened during playback
- **THEN** the input goes into the same rectangle and monospace font, the digits are typed right to left, the digit events are consumed from the queue (they do not go to the hotkeys), the time shown is the interpolated display clock; to the left of zero the timecode is drawn with a sign
- **TEST** t98 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Numerical stability of UI animations
UI animations SHALL use an analytic solution (an exponential) or
exponential smoothing, rather than an explicit integration scheme: `stable_dt` after an
idle period arrives up to 250 ms. The animation tests SHALL run bad dt values (0.1, 1/30,
alternating). Everything that goes into the sound SHALL be clamped in the engine.
Why: the fader flew to 400 dB in three frames — "painful to the ears".

#### Scenario: The fader does not diverge on bad dt
- **WHEN** the fader's animation receives dt 0.1 s, 1/30 s and their alternation
- **THEN** the value moves monotonically towards the target and does not diverge; the master volume is clamped by the engine independently of the UI
- **TEST** t84 (`src/apps/kt-app/src/meters.rs`), t84 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: Holding the space bar accelerates, and does not toggle
- **WHEN** the space bar is held for longer than 220 ms
- **THEN** the speed multiplier moves towards the ceiling of 4× along an exponential (stable on bad dt), play/pause does not toggle, releasing returns the base speed and the sign of the reverse; a short press is the ordinary play/pause; the speed widget shows the TEMPORARY speed
- **TEST** t120 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Panels stay within bounds, the interface runs to the edges, edges do not steal clicks
A panel MUST NOT grow because of its content (for width `auto_shrink`, the columns are not a `Grid` with Extend, with a `max_size` safeguard); the preview does not collapse when the column's content is long. The interface SHALL be drawn on the full `content_rect` without the default 8 px. The limits set at the owner's request MUST NOT change without a new request: the left column 420 px, the meters panel 90 px, the minimum window 640×640, the ceiling of the timeline's height. The resize edges (8 px for the side, 16 px for the corner) SHALL send `BeginResize` at the press itself, and everything within 16 px of the window's edge (window buttons, menus) SHALL fall into the zones without resizing. After `StartDrag`/`BeginResize` the pointer SHALL be released explicitly (X11 does not send ButtonReleased).
Why: "the preview disappeared without trace", "AGAIN I cannot change the width of the widget" and "the media widget is squeezed in width FOR THE THIRD TIME" — three rounds in a row.

#### Scenario: The panels share the space with the preview and do not grow
- **WHEN** the left column receives long content, the separator is dragged, the edges and corners of the window are pressed
- **THEN** the column stays within its bounds, the preview takes the centre; the separator lives at the ACTUAL edge of the panel and is draggable; the edges give the resize cursor and `BeginResize`; the resize zones do not steal clicks from the buttons; the top zone is given to the preview
- **TEST** t76 (`src/apps/kt-app/src/layout_tests.rs`), t77 (`src/apps/kt-app/src/app.rs`)

#### Scenario: Dragging the window from the top is continuous and takes no space
- **WHEN** the window is dragged by the invisible strip above the preview in the maximised window
- **THEN** the strip works across the full width, including above the preview, and does not take height from the preview; clicks on the preview are accepted below it
- **TEST** t85, t97 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The meters and the minimum window
- **WHEN** the window is shrunk, the meters panel is stretched
- **THEN** the window is not smaller than 640×640, the meters take the full width of their panel up to the edge, the width limit of the meters is 90 px, and an LED segment lights up entirely or not at all and does not cross the zone boundary at any grid step
- **TEST** t86, t97 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The buttons at the window's edges are clicked
- **WHEN** the window button (minimise/close) 16 px from the edge is pressed and released, and also the meters' separator
- **THEN** the window command is sent after an EXPLICIT release; the resize does not intercept the press
- **TEST** t101 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The width of the media bin and of the cards
- **WHEN** the bin's cards are moved onto the timeline, the poster is selected
- **THEN** the bin's layout does not change from the move; the poster is the most informative frame (flat frames are filtered out); the limit of the column's width is not squeezed "by analogy" with its neighbours
- **TEST** t87 (`src/apps/kt-app/src/layout_tests.rs`), t87 (`src/apps/kt-app/src/thumbs.rs`)

#### Scenario: The height of each track is its own, and this is a convenience, not an edit
- **WHEN** the bottom edge of a row in the gutter is dragged, the menu item resets the height
- **THEN** the override beats the computed height and is clamped to [THIN_ROW_MIN, ROW_H_MAX]; the "unsaved" asterisk does not light up — the row's height lives in the workspace, not in the project
- **TEST** t262 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A live probe in the real window, not in the harness
- **WHEN** the panel's limit, a drag, a key or an input that the harness has already shown green are checked
- **THEN** the probe is done in the real kt-app window (X11/XTest, ImageMagick screenshot): the harness catches the layout, the probe catches "what the person sees"; one does not replace the other (the lesson of "you did not check", capability `durability`)
- **TEST** `src/scripts/ui-probe.py` (`geom`/`drag`/`key`/`type`), live scenarios `src/scripts/live-check.sh`

### Requirement: Panel order and one hint in one place
Everything that is filled in during a frame (the hover hint) SHALL be drawn at the END of the frame by a separate non-interactive layer; a panel drawn before the sources MUST NOT read it. The hint SHALL be exactly one — the bottom right of the status line through a single sink, priority hover → mode → message → standby line; there are no pop-up instruction tooltips. The test SHALL check what is DRAWN, not the internal field.
Why: "WHERE HAVE THE BUTTON HINTS GONE?!" — the status bar was a second panel of the frame and always read an empty hint; the tests read the field after the frame and were falsely green.

#### Scenario: The hint reaches the status line
- **WHEN** the cursor is over a button, a zone of the timeline, a combo box, the preview, the meters
- **THEN** the rendered status line contains the contextual hint; the timeline has hints per zone (marker, region border, ruler, track, trim, clip, empty place)
- **TEST** t83 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The hints of shared widgets inside windows also arrive
- **WHEN** a parameter widget inside `Window::show` (where there is no `&mut KtApp`) is hovered
- **THEN** the hint goes through the sink and is shown in the same place of the status line; an unknown kind of generator is announced loudly, not as an empty cell
- **TEST** t518 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Do not waste space; text fits
A new strip, row or column SHALL be introduced only when it is proven that
the content cannot be placed in an already occupied place. The name of a label SHALL be written on
the time caption row, the name of a bin group — in the gap of the frame's top line,
a long track name — as a tag with a backing a row BELOW the clip's name, the name of a transition
kind — inside the transition window. A long name SHALL be truncated by one helper,
rather than stretching the strip. Obvious captions ("zoom:", "FPS:", "(loop)") and
duplicating a highlight with text MUST NOT exist.
Why: the strip for labels stood empty in any project without labels; "do not make a mess out of letters… write the names of the tracks EXACTLY under the names of the clips".

#### Scenario: The track's tag and the cursor's trail
- **WHEN** a track's name is long, the cursor is playing
- **THEN** the tag stands on the row below the clip's name and does not stick out past the track; the full name is in the hint; the cursor's gradient glows from the side of play and does not run onto the cache's stripes
- **TEST** t102 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Bin groups without extra height
- **WHEN** cards are merged into a group, selected with a frame, dragged, searched with Ctrl+F
- **THEN** the frame is the union of the cards, the name is in the gap of the top line, the lock is a group of one member; dragging drags the whole group by one delta; deleting media cleans up the groups and the layout
- **TEST** t99 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Long names are truncated in all languages
- **WHEN** a caption is longer than its budget (menu headings 160 px, rows of the performance panel 190 px) in any of the five languages
- **THEN** the caption is truncated with an ellipsis and does not push the panel; two-column rows are measured as assembled, not as templates
- **TEST** t128, t127 (`src/apps/kt-app/src/layout_tests.rs`), t255 (`src/apps/kt-app/src/reach_tests.rs`, capability `ui-reach`)

### Requirement: A useful default; an action does not switch off a neighbouring state
The useful mode SHALL be on by default, and a modifier turns it OFF (the magnet on a move; Ctrl removes it). Esc SHALL clear the region, but not the loop.
Edits of the CONTENT and edits of CONVENIENCE SHALL live in two baskets: only content raises the asterisk and starts the autosave; convenience (region, loop, zoom,
scroll, the bin's layout, track names) is written in passing and never looks like an edit. A new flag "something has changed" MUST NOT be introduced.
Why: the asterisk lit up from scrolling, Esc switched off the loop, the magnet did not work without the two edges hitting at the same time.

#### Scenario: The move magnet
- **WHEN** a clip is dragged towards a neighbour; the clip's link group stands on the same boundaries; Ctrl is held
- **THEN** the edge sticks to the target (the candidate "without snap" does not take part in the comparison), the partner is excluded from the targets, the yellow line returns together with the result; Ctrl temporarily removes the magnet; on the old formula the test gives 93910 instead of 90000
- **TEST** t95 (`src/apps/kt-app/src/timeline.rs`), t95 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Content dirtiness ≠ convenience, Esc does not switch off the loop
- **WHEN** the user moves the region/zoom/the bin's layout, then presses Esc
- **THEN** the asterisk does not light up, the autosave is not started; the region is cleared, the loop remains armed (the button's third state — "armed without a region")
- **TEST** t96 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Navigation, follow and file DnD
A jump to a stop point SHALL show the cursor even when paused (a deferred reveal where the sizes and zoom are known). The follow mode SHALL restore itself:
playing + the region entirely visible + the cursor inside + ≥ 1.5 s after a manual
scroll; a Shift-vertical does not switch follow off. Shift+A/Shift+D SHALL pull the edge of the
region from the jump. A file drop SHALL land under the mouse with a silhouette: on X11 the position
is read from the X server directly (winit does not give coordinates during DnD), on Wayland —
an honest fallback to the playhead; the hover position lives until the drop itself. A single
click on the preview SHALL wait for the double-click window (300 ms): a single one — play/pause,
a double one — detaches/returns the preview.
Why: Home/End left the window in place; a drop always landed on the playhead, because
the hover was erased by an unconditional `take()` in the same frame.

#### Scenario: Jump and follow
- **WHEN** Home/End while paused; it is playing and the user has turned the wheel
- **THEN** the cursor is shown; follow returns only after the grace period, with the region visible
- **TEST** t88 (`src/apps/kt-app/src/layout_tests.rs`), t72 (`src/apps/kt-app/src/timeline.rs`)

#### Scenario: Shift+A / Shift+D pull the edge of the region
- **WHEN** there is no region and Shift+D is pressed
- **THEN** the right edge goes to the new position, the left one — to the position BEFORE the jump; the cursor is not thrown back to the start of the region; the asterisk does not light up
- **TEST** t119 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The media bin scrolls itself at the edge and by the wheel during a drag
- **WHEN** a card is dragged towards the bin's bottom edge; the wheel is turned over the bin during a drag
- **THEN** the list moves along the auto-scroll curve (bypassing egui's gate that mutes the wheel over a ScrollArea during a drag), the card's position is NOT rolled back; the border of "move onto the timeline" is taken by `clip_rect`, not by `max_rect` inside the ScrollArea
- **TEST** t118 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: File DnD and a click on the preview
- **WHEN** a file is dragged over the timeline and dropped; the preview is clicked once and twice
- **THEN** the hover position is kept until the drop and taken only at the drop; the geometry "x → tick, y → row" — only through the frame's geometry cache; a single click gives play/pause after the window expires, a double one cancels the single and toggles detaching
- **TEST** t47, t74, t51 (`src/apps/kt-app/src/app.rs`)

### Requirement: Timeline row geometry is one and has no divisions by height
Mapping of positions outside drawing SHALL go only through the frame's geometry cache with the rows from `Project::rows()`; there MUST NOT be own formulas "offset(kind)+lane". Rows of DIFFERENT height: the service rows (morph/camera) are three times lower than the clip tracks, a floor of 10 px for the height; the tops of rows are prefix sums inside the cache. Not a single division by the row height: "which row" — by the table, "the share within a track" — by the clip row, the zoom anchor — by the same table. The limits of the vertical layout 30/84/16/120 MUST NOT change.
Why: the formula `(rel / lane_h).floor()` lies on mixed heights — exactly so
the drag of a clip computed the row; "to avoid losing space, make the morpher tracks three times shorter in height".

#### Scenario: A bijection of rows in an arbitrary order
- **WHEN** the rows are shuffled, a clip is dragged across other people's rows
- **THEN** `row_of`/`key_at_y` is a bijection, the drag sticks to the nearest row of ITS OWN kind, the file drop computes the row and the tick with the same geometry
- **TEST** t47, t50 (`src/apps/kt-app/src/timeline.rs`)

#### Scenario: Thin service rows
- **WHEN** the set of rows is mixed (clips + morph/camera), a thin row is clicked with the mouse, zoomed vertically
- **THEN** the height ratio is 3, the floor is 10 px, the prefix sums are correct, `row_at_y` is bijective, the clicks LAND in the thin row (the case "computed correctly, but there is nowhere to click" is caught only by the harness), the zoom anchor does not drift
- **TEST** t245, t246 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A track from the menu is placed under the clicked row
- **WHEN** a right click on the canvas, "add a video/audio/morph track"
- **THEN** the new track is placed RIGHT UNDER the clicked row (not at the end), the track names move on insertion/deletion/reordering, and the order and the insertion are one undo step
- **TEST** t122 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Gestures on the event's top strip: fades and level
The upper strip of an event (35 % of the row's height, at most 12 px) SHALL be a gesture zone: the fade handle at each edge, the level line (for video — opacity, for audio — gain from silence at the bottom to +6.02 dB, 0 dB — the middle). The corner fade is a transition with a zero external offset; the handle sticks to the corner within 3 px, a pushed-in fade disappears entirely. Below the strip everything is as before (the body, the trims, the transition windows). The handles SHALL be visible on the selected and on the hovered event; the grab zones of the handles are a third of the width, the middle is always free. A repeated gesture on a configured transition SHALL edit ONLY the offsets of the window. The preview of the material (waveform/filmstrip) during gestures SHALL shift only on a trim of the left edge.
Why: "in Vegas you can do a fade… by pulling the upper corner"; "fades for audio and video looking the same" (the v1 volume rail stood in the fade zone); a repeated overlap erased the chosen transition kind; "the sound's preview scrolls to the left, although the sound does not change".

#### Scenario: The corner fade and the level
- **WHEN** the upper corner of an event is dragged, the level line is dragged, the edge of a transition window without a neighbour is dragged
- **THEN** the fade appears inside the event (the body and `source_in` are untouched), a pushed-in one disappears; the level is written as `SetOpacity`/`SetGain`; a fade window without a neighbour is dragged as usual (test 157 holds)
- **TEST** t173, t176, t157 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Level is reachable on a narrow event
- **WHEN** an event 20 px wide
- **THEN** the fade handles each take a third of the width, the line is drawn from 6 px, the middle moves the event (U5.3 of R.72 took the level gesture off a narrow event, a later word of the owner than R.40) and the event menu has a number field for the same value
- **TEST** t278 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A repeated gesture does not erase the transition look
- **WHEN** the corner of a configured wipe fade is dragged further; with a Move drag, the pair is again overlapped with a configured look
- **THEN** the look, the parameter curves and the audio curve survive; only the window offsets change
- **TEST** t460 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Material preview moves only when the start is trimmed
- **WHEN** an event is moved deep into its neighbour; the left edge is trimmed
- **THEN** the waveform and the filmstrip stay in place when moving, and when the start is trimmed they shift by the edge delta (without `.max(0)`)
- **TEST** t169 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: An impossible action is an error; paired ways are identical; names say what and where
A gesture's stop (neighbour, occupied place) SHALL be shown as an error: the obstacle
is highlighted in red, a hint in error style overrides hover, and if the obstacle lies
outside the view — the side, the distance and a red arrow at the edge of the row. The obstacle
lives for exactly one frame. An action available both by gesture and by menu item SHALL have one
body and take the context into account the same way. The menu item name SHALL say WHAT and WHERE
will happen, and remain distinguishable in every language.
Why: "the hint is pale and faded… everywhere such situations are made as
errors"; "marker here with a selected region must produce a region marker";
"Remove crossfade… turns out to be the fadeout from the end that removes it". Two paths to
one action SHALL also share one catalog name, with the chord in the chord column, not in the
translated text: the Edit menu's "Split / cut region" promised a cut-out that neither it nor `S`
ever did (owner's decision 2026-10-26, change `split-menu-label`: rename, keep the action).

#### Scenario: Stop against a neighbour outside the view
- **WHEN** an event is moved onto an occupied place, the obstacle is not visible
- **THEN** the hint in error style names the side and the distance, a red arrow at the edge of the row; a visible obstacle is highlighted
- **TEST** t172 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: "Marker here" = gesture M
- **WHEN** a region is selected and the menu item "Marker here" is invoked (including a click on the region edge)
- **THEN** a range marker is placed over the region, as with gesture M; the item name changes with the context
- **TEST** t170 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Transition menu items are distinguishable in all languages
- **WHEN** the labels "remove fade / crossfade" and "fade in / fade out" are read in each catalog
- **THEN** the four actions (fade/crossfade × our start/our end) have different labels, and "at the start / at the end of the event" is present
- **TEST** t171 (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: Overlap transition is reachable by the same gestures
- **WHEN** right-click on the overlap zone of two events; double-click on the window of an overlapping transition
- **THEN** the menu offers "Transition from…" (the neighbour ladder is equal to the joint one), double-click opens the properties
- **TEST** t466 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The Edit menu's split is the event menu's split and the S key
- **WHEN** a region is drawn and the playhead stands inside an event with nothing selected; Edit → "Split at playhead" is clicked; the edit is undone and `S` is pressed instead; the label and chord of the Edit item and of the event menu's split item are read in each of the five catalogs
- **THEN** both paths split the same events at the same tick and leave the region's interior in place; the Edit item shows the event menu's label with `S` in the chord column; no catalog's split label contains a chord; the toolbar scissors button's hint is the split label followed by its key
- **PATH** a region over part of an event, the playhead inside the event → Edit menu → the item reads "Split at playhead" with S on the right → click → the event is cut in two at the playhead, the region still covers both halves → Ctrl+Z → the event is whole again → S → the same cut at the same place
- **TEST** t870 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Multiple selection, marquee, moving a batch, double clicks
The selection SHALL be a set: Ctrl+click adds/removes and does not move
the cursor, a plain click selects one and places the cursor. The frame starts ONLY from
empty space (Ctrl+drag always; a plain drag — in the "Selection
tool" button mode); Ctrl on an event still disables snapping. Moving a batch moves only along
time, mutual shifts do not change, the stop is the first contact of any participant,
the commit is one batch from the ORIGINAL starts with the crossfade handles returned.
Operations that make no sense on a set require exactly one selected item and say so.
Double-click on an event — the FX window; a TRIPLE click on an event, Shift+double-click and the
menu item — the region from the event (one body: the span of the event's link group); double-click
on the ribbon of a range marker — the region along its bounds plus selection of its events.
A triple click SHALL leave nothing of its second click behind: the FX window that this gesture's
double click opened closes again, a reel that it expanded in place collapses again; on the level
line, whose double click resets the level, the third click does nothing more.
COPY (Ctrl+C and the menu item) SHALL take, in this order: (1) when the selection is exactly ONE
event (with its link partners, which travel with it anyway) — that event, whole, whether a region
exists or not; (2) otherwise, when a region exists — what lies in the region on all tracks,
cropped to it; (3) otherwise — the selected events with their link groups. The status line SHALL
name which of them was taken ("the selected event", "the region", "the selected events") and the
count; a region left in place by rule (1) is not cleared. The selection of morpher layers and
cameras does not take part in Copy (unchanged).
Why: "area selection is missing… ctrl… or as in Vegas — a toggle button";
the hint promised "double-click — FX" since R.18, but the gesture set a region.
ktprompts 26.10.18 (2026-10-27), the owner: "4. If I selected one event, copy-paste must take it,
not the region I selected. 5. A triple click on an event selects a region over its bounds." Before
this, the region always won: a loop region kept for listening made a click on one event and
Ctrl+C copy every event of every track inside the loop.

#### Scenario: Ctrl+click and frame
- **WHEN** Ctrl+click on events; Ctrl+drag from empty space; a plain drag from empty space without the button
- **THEN** the set grows and shrinks, the cursor stays; the frame catches ANY intersection and creates no region; without Ctrl a region is created as before
- **TEST** t202 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: "Selection tool" button
- **WHEN** the button is turned on by a click
- **THEN** a plain drag over empty space catches events with the frame; turning it off returns to the region
- **TEST** t203 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Three-step batch move
- **WHEN** two events are selected and one of them is dragged (press → 8 px → target)
- **THEN** both have moved by one shift, the unselected one stays in place, the durations are intact, the stop counts the block as a whole, undo returns it in one step
- **TEST** t204 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The frame becomes a named area
- **WHEN** after the frame, "save as area" is invoked
- **THEN** the area is created along the frame bounds in one history step, the name is made unique, the prefix on insertion is available from the same window
- **TEST** t248 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Operations over the whole selection
- **WHEN** Del, copy-paste, morpher targets on the set
- **THEN** they go over the whole selection in one history step; right-click on a member of the selection does not break the batch
- **TEST** t205 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Double clicks
- **WHEN** double-click on an event, Shift+double-click, double-click on the ribbon of a range marker (harness with a step of 0.02 s, a separate one for each gesture)
- **THEN** the FX window; a region from the event; a region from the marker with the events inside selected; with several overlapping, by flag/name
- **TEST** t206, t207 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: One selected event wins over the region
- **WHEN** a region covers three events on two tracks; one of them, which has an audio partner, is clicked; Ctrl+C; Ctrl+V at a free place
- **THEN** the clipboard holds that event and its partner, whole (not cropped to the region), and the paste puts exactly these two; the region is still in place with the loop state unchanged; the status line names "the selected event" and the count 2; on the code before the change the clipboard held all three events cropped to the region
- **PATH** drag on the ruler → the region over three events → click one event → it is highlighted, the region stays → Ctrl+C → the status line "Copied the selected event: 2" → move the playhead to empty time → Ctrl+V → the event and its audio appear there whole
- **TEST** t924 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Several selected events, nothing selected, no region
- **WHEN** a region exists and two events of different link groups are Ctrl+clicked; Ctrl+C; then Esc and the region is set again with nothing selected; Ctrl+C; then with no region two events are selected; Ctrl+C
- **THEN** the first copy is the region block (every crossing event, cropped) and the status line says "the region"; the second is the region block too; the third is the two events with their groups and the status line says "the selected events"
- **PATH** drag on the ruler → region → Ctrl+click two events → Ctrl+C → "Copied the region: N" → Esc → drag the region again → Ctrl+C → "Copied the region: N" → Esc twice → Ctrl+click two events → Ctrl+C → "Copied the selected events: 2"
- **TEST** t925 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Copy after a double-click on a range marker
- **WHEN** a range marker crosses three events and is double-clicked; Ctrl+C; separately a range marker that crosses exactly one event is double-clicked; Ctrl+C
- **THEN** with three events the region block is copied (cropped to the marker's range, the marker travels with it), as before the change; with one event that event is copied whole, because it is the one selected event, and the status line says "the selected event"
- **PATH** double-click the ribbon of a range marker over three events → region over it, three events highlighted → Ctrl+C → "Copied the region: 3" → double-click a range marker over one event → one event highlighted → Ctrl+C → "Copied the selected event: 1"
- **TEST** t926 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Triple click on an event sets the region over it
- **WHEN** an event with an audio partner and no FX window open is triple-clicked on its body (harness step 0.02 s between clicks); separately with the FX window of another event already open
- **THEN** the region equals the span of the event and its partner, the event is selected, no FX window of this event is open after the third click; the already open FX window of the other event stays open; one triple click adds no history step
- **PATH** triple-click the body of an event → the FX window flashes open on the second click and is gone after the third → the region band over exactly the event and its audio → press L → the loop plays exactly the event
- **TEST** t927 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Triple click on a reel and on the level line
- **WHEN** a reel instance is triple-clicked on its body; separately the level line of an audio event set to −6 dB is triple-clicked
- **THEN** on the reel the region covers the instance and the reel is NOT left expanded (the second click's expansion is undone); on the level line the gain is 0 dB (the double click's reset, one history step) and the region is unchanged
- **PATH** triple-click a reel instance → it expands on the second click and collapses back on the third → the region band over the instance → triple-click an audio event's level line → the line jumps to 0 dB, the region stays where it was
- **TEST** t928 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Three ways to a region over an event are one body and are promised
- **WHEN** for the same event the region is set by a triple click, by Shift+double-click and by the menu item "Region over this event"; the event's hover hint and the menu item's chord column are read
- **THEN** the three regions are equal; the hint of an event names the triple click next to "double click — FX" in all five catalogs; the menu item's chord column names the triple click; the hint does not promise a gesture the timeline does not perform (t83 family)
- **PATH** hover an event → the status line "… double click — FX, triple click — region …" → right-click it → "Region over this event   3×click" → click the item → the same region as the triple click gave
- **TEST** t929 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Model edits from the UI go through wrappers; autosave is background and guarded
Any model edit from the UI SHALL go through the application wrappers (`apply`,
`apply_batch`, `apply_batch_restoring`, `apply_coalesced`), which advance the model
revision; live paths send a command ONLY when the value has actually changed.
Periodic autosave SHALL run in the background (a snapshot in the UI, serialization and writing
in a separate thread), not during a gesture or playback; the asterisk is cleared
only if the revision has not changed since the snapshot. All project writes SHALL go through
a guarded write, which does not write if a newer version is already on disk. Editing the region and
the loop MUST NOT rebuild the engine (`set_project`): the loop reaches the player by a separate
command, while content edits are conveyed to the player.
Why: FX edits made after a snapshot looked saved and were lost; a synchronous
write every 30 s dropped the FPS to zero; a background snapshot overwrote a fresh Ctrl+S; on a
1961-event project every shift of the region emptied the ring and gave underruns.

#### Scenario: Autosave in the background and clearing the asterisk
- **WHEN** there are edits, the autosave interval passes; after the snapshot one more edit is made
- **THEN** the write does not run on the UI thread, frames do not stall; after the write the asterisk stays lit if the revision has moved ahead
- **TEST** t194 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A live gesture edit advances the revision
- **WHEN** a background write is in flight, the user edits the FX chain with a window gesture
- **THEN** the revision has grown, the asterisk stays, the edit is in place; a frame without mouse movement produces no command
- **TEST** t200 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The guarded write does not go backwards
- **WHEN** a write rev=3 arrives after the written rev=5; then rev=9
- **THEN** the first does not touch the file, the second is written; a background autosave does not overwrite a fresh Ctrl+S
- **TEST** t199 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The region does not rebuild the engine, the content must
- **WHEN** the boundary of a region is moved / Loop is pressed; content is edited
- **THEN** the number of renderer rebuilds does not grow from the region; a content edit rebuilds
- **TEST** t286 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Effect windows and input fields: one widget per meaning
A numeric parameter SHALL be drawn only by the shared widgets of the parameter row/grid,
with the range, step and unit from the model spec; the labels come from the catalog.
The chain strip SHALL be one per event and morpher (master checkbox, a checkbox and a cross
at the slot, reordering by mouse, copy-paste); for a morpher the strip is one and starts
with a pinned morph slot. The FX window SHALL drive the whole link group in one session and
one undo step; "Cancel" rolls back all chains. Editing a parameter while a live
curve exists SHALL be a curve command, not a base one, and the test SHALL assert the model
through characters typed into the field. The morpher source field SHALL accept a file
from the system, a bin card, a picker with search, and a dialog. Curves SHALL draw
ONE automation widget (graph ↔ table), the axis is the event's material, the value scale is
taken from the curve's keys, and editing time sorts only on release.
Input SHALL be drawn in the same rectangle and font as the value it replaces ("you can't just slap an inputfield there"); everything that takes focus gives it back
on close.
Focus of an input field SHALL be released by ONE rule for the whole program: a pointer press outside the
rectangle of the focused field (on the timeline, preview, toolbar, another window) returns
focus in the same frame, and hotkeys (Space, M, Del, Ctrl+Z…) act at once, with any
window open. A field MUST NOT take focus back by itself without a new click into it.
Why: Gain was −24…24 in one window and −48…24 in another; slowed drew a graph,
pan3d a table; "I type into the rate field, and it snaps back to the previous
value"; Ctrl+L after Enter killed all hotkeys. R.63: egui hands focus from a
`TextEdit` only on Esc/Tab/another focusable widget; a click on the timeline
(`Sense::click_and_drag`) does not touch it — "however I poke at the rest of the
interface, space does not lead to pause until I close the effect window".

#### Scenario: One chain strip, fields per the specs
- **WHEN** the event window and the morpher window are open
- **THEN** the master checkbox is in both (the widget is shared); the batch slot switch removes the effect from the targets without losing it; the field ranges match the spec
- **TEST** t217 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The morpher source field
- **WHEN** a file, a bin card is dropped into the field, or chosen with the picker
- **THEN** a file that was not there is added and substituted in one undo step; "Cancel" rolls the session back
- **TEST** t218 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: One window per link group
- **WHEN** FX is opened from the partner of the pair
- **THEN** a second window is not created; the window drives both chains in the collapsed forms; the rollback returns both; for the video instance of warp the reverb handles are hidden and the frame mode is shown; the reverb handles are visible exactly once
- **TEST** t219, t248c (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Automation in the FX window — one history step
- **WHEN** a curve is added, keys are added, slots are moved, Cancel is pressed
- **THEN** the curves move in parallel with the chain; the whole session is one undo step; Cancel returns both the chain and the curves
- **TEST** t193 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Automation widget: mapping and stable input
- **WHEN** a key on a forward, reversed and accelerated event is converted material ↔ timeline; in the table t=0.5 is typed after t=1.0
- **THEN** the conversion converges in both directions; the rows are not re-sorted until release
- **TEST** t221 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A key value typed into the field reaches the model
- **WHEN** in a live FX window "3.0" is typed into the parameter field with a live curve (the coordinate comes from the real layout, since DragValue has no accessibility node)
- **THEN** the curve in the model contains 3.0 (on the old code — 1.0); a character without a change of the command value produces no command
- **TEST** t281 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Transition window
- **WHEN** double-click on the transition window; choice of the look and the audio curve; "Cancel"; the transition dies
- **THEN** the look and the curve reach the model on the same parameter grid; the rollback returns the original look; the session is one history step; the window of the dead transition closes by itself
- **TEST** t227 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Camera window
- **WHEN** double-click on the camera block; edits; the camera is deleted
- **THEN** the window opens; the edits are one undo step; the window of the dead camera closes
- **TEST** t231b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Morpher window: targets, time, one strip with the morph slot
- **WHEN** the morpher is open in "below" / by-targets mode; "+M" is pressed in the toolbar
- **THEN** the mode and the explanation are visible; the "by time" checkbox is one history step; the frozen list is shown by name; "+M" creates a layer on top in one step, the strip is one, the morph slot is first and cannot be reordered, the effects of both kinds lie in it and are unpacked back by kind
- **TEST** t242, t244 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Insert prefix window
- **WHEN** Ctrl+V with a named item in the buffer; Esc; a second Ctrl+V; Enter
- **THEN** without a named item there is no window; Esc leaves no trace and releases focus; the second Ctrl+V opens the window again; confirmation is one history step with the frozen insertion position
- **TEST** t250 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Presets and paired widgets (capability `automation`)
- **WHEN** a chain/preset is inserted into the morpher; the preset name window; the frequency-response graph is dragged and turned with the wheel
- **THEN** the curves move with the chain (a silent branch is impossible by type); the name window follows the rules (it grows from input — probe the buttons after the redraw); the graph writes to the same place as the field; the wheel is consumed only at its own point
- **TEST** t343, t345, t346, t349, t350 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A click past the field releases focus, keys stay alive while a window is open
- **WHEN** in the FX window one clicks into the formula field and types; then clicks on the timeline (empty space), on the preview, on the toolbar; presses Space; the same with the name field in the track window and the search field of the picker; clicks again into the field and types
- **THEN** after a click past the field it has no focus, Space starts playback (on the previous code — no, while the window is open), text typed before the click is kept; the field accepts input again after a click into it; the test presses from the point of a person and asserts `is_playing()`
- **TEST** t750 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/formula.py`

### Requirement: Thumbnails, sidecar and the event preview compute the same map as render
Scrolling/zooming the timeline MUST NOT generate decode requests (invariant 8):
waveforms come from the peak pyramid, thumbnails from the sidecar DB; while there is no index, placeholders.
Everything that may fail in the worker SHALL publish a FAILURE (a negative
cache); the thumbnail sources are LRU. Thumbnail keys are quantized FROM ZERO OF THE FILE (a shared
cache bin ↔ timeline). The translation "event tick → source frame" SHALL be one
function for the filmstrip, the waveform and the picker thumbnails, taking warp AND reverse into account;
the warp map is cached by the pair (revision, number of clips). A generator SHALL get
a thumbnail both without a file and without a DB.
Why: a storm of FFmpeg opens of ~250/s at rest and 1.6k/s on mouse movement; a slowed and
reversed clip drew something other than what played.

#### Scenario: Peaks and thumbnails without a DB, and for media added after start
- **WHEN** the project is unnamed (no DB); media is added after the service has started
- **THEN** the pyramid is built, thumbnails arrive
- **TEST** t23 (`src/apps/kt-app/src/util.rs`)

#### Scenario: Negative caches and LRU of sources
- **WHEN** the duration/key failed to be obtained; ≥ 4 media
- **THEN** a repeated request is not queued, `invalidate_media` resets both caches; exactly the oldest source is evicted, the file is not reopened for every key
- **TEST** t69, t71, t62 (`src/apps/kt-app/src/thumbs.rs`)

#### Scenario: One "tick → material" map knows about reverse
- **WHEN** an event is reversed without warp; an event at 0.5×; the warp slot is off
- **THEN** the first column of a reversed event is the last frame of the piece (the old formula gave 0); by the middle of the event a quarter of the material is consumed; a disabled slot removes the stretch; the filmstrip step is taken from the consumed span; the map cache is not rebuilt per frame
- **TEST** t549 (`src/apps/kt-app/src/thumbs.rs`), t249 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Generator thumbnail without a file
- **WHEN** a `gen:` source without a file, with a sidecar DB and without one
- **THEN** the service returns the look (video without audio), zero duration and a real thumbnail of the given colour
- **TEST** t476 (`src/apps/kt-app/src/thumbs.rs`)

### Requirement: Missing sources do not block; returning ones are swapped on the fly
An unavailable file SHALL get a stub (a frame with the file name, silence), the project
opens, the preview is complete, and every missing file is named in a notification. Rendering
SHALL warn before starting and count the stubs in the summary, rather than being cancelled.
A returned file SHALL be reopened in the background and swapped in at the player. Strict
opening remains strict where a stub is harmful (adding media, polling
permissions).
Why: "the absence of source files must not be a blocker"; of ten missing
files the user saw one, and the preview hung in "Loading".

#### Scenario: Opening with stubs
- **WHEN** a project with two missing files
- **THEN** both are named in the notifications, the set of stubs contains both, the preview is complete
- **TEST** t209 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Render with stubs
- **WHEN** rendering with unavailable sources
- **THEN** the settings window warns before starting, the export proceeds, the summary counts the stubs and the lost layers
- **TEST** t210 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Source returns
- **WHEN** the missing file is available again
- **THEN** the source is reopened in the background and replaces the stub at the player; the cache of the new source is filled (the project version is bumped)
- **TEST** t211 (`src/apps/kt-app/src/layout_tests.rs`), t211 (`src/libs/kt-engine/tests/player_dynamic.rs`)

### Requirement: Render file name by template
The export file name SHALL be `kt-<project>-<region>-<N>s.mp4`; the region segment
is substituted only if the bounds match a named range marker;
user text is sanitized.
Why: `kp-render-<unix-time>.mp4` distinguished files only by the number of epoch seconds
(the owner's wording — the template verbatim).

#### Scenario: Template matrix
- **WHEN** an unnamed project; a region without a marker; a region by a named marker
- **THEN** `untitled`; the region segment is omitted; the marker name is substituted, the length is the whole number of seconds of the actual range
- **TEST** t201 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The GPU backend is honest; device loss is announced once
The video-FX engine SHALL be created from the eframe device on the first frame and
injected into the player; if it was not created there is no picture and the performance panel line
says "FX: no device"; `DeviceLost` in the middle of a session "poisons" the engine until the end of
the session: no picture, the warning exactly once, and the line "FX: GPU lost: <reason>". The export
builds its own headless engine.
Why: the UI queue must not share time with a 4K export; a lost picture without a notice
would look like a broken project.

#### Scenario: GPU loss
- **WHEN** the device is lost during a session
- **THEN** the notice is shown once, there is no picture after that, the backend status is honest
- **TEST** t185 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The performance panel tells the person the truth
The underrun counters SHALL be labelled (render / callback); callback starvation
is also shown in milliseconds of silence; "FPS" means UI frames, video is discussed in
V-fps and "V-skipped cells". The SPEED SHALL be judged by the release build:
the dev profile is built with `opt-level = 1`, and the tempo measurement is `kt-cli probe --time`.
The performance panel SHALL be a section of the Diagnostics window, not a box of the left column:
the column belongs to the media bin, and the always-visible layer is the status line (vfps, cache,
underruns in ms). The Diagnostics window SHALL carry all three graphs — Quality (FPS, V-fps,
A-hit, V-hit), FFmpeg activity (Total, Open, Read, V-dec, A-dec) and Cache churn (evict, miss,
no-read) — next to every line the box had, the cache occupancy bar and the problem log; the
graphs SHALL share the window's height, so a larger window shows larger graphs. A graph SHALL
be redrawn from what was built for its last sample: a frame without a new sample rebuilds
nothing, and an open Diagnostics window SHALL NOT make an idle program repaint more often.
Why: the user read `Underruns X/Y` as L/R; in the debug build the audio render
of a dense edit ran at 112 % of real time against 10 % in release. The owner, kkt-26.11.01
item 14: "Perfomance надо переместить в окно diagnostics, чтобы там три графика было" — the box
squeezed the bin all the time, and diagnosing a slow spot meant looking in two places.

#### Scenario: Underrun labels and silence in ms
- **WHEN** the performance panel and the status line show starvation
- **THEN** both counters are labelled, the callback silence is expressed in milliseconds
- **TEST** t289 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Tempo is measured by the tool, not by eye
- **WHEN** one needs to answer "does the audio render keep up with real time"
- **THEN** `probe --time` prints the fraction of real time and `OVER BUDGET`, not a UI impression
- **TEST** t290 (`src/apps/kt-cli/tests/cli.rs`, capability `transport`)

#### Scenario: The left column is the bin's, the panel is in Diagnostics
- **WHEN** the editor is open with a project and the Diagnostics window is closed
- **THEN** no "Performance" title, graph or perf line is painted in the left column; the bin's
  rectangle reaches the bottom of the column; the gauge button of the toolbar and View →
  Diagnostics… both open the window
- **PATH** look at the left column → only the media bin, down to the bottom; click the gauge
  button in the toolbar → the Diagnostics window opens with its "Performance" section at the top;
  close it with the cross; View → Diagnostics… → the same window opens again
- **TEST** t1145 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: Nothing of the former box is lost
- **WHEN** the Diagnostics window is open
- **THEN** it paints the three graph titles (Quality, FFmpeg activity, Cache churn) and every
  line the box had — cache MB, underruns render/out, preview size, clips and media, latency and
  callback, V-shown and V-skip/back, the FX backend, the compose cost, the FFmpeg totals — and the
  "Save the playback journal" button; the "Economy UI" switch lives only in File › Preferences…, with its hint
- **PATH** open View → Diagnostics… → the window shows, top to bottom: the "Performance" rows in
  two columns, the GPU/FX/compose/FFmpeg lines and "Save the playback journal", the cache
  occupancy bar, the three graphs one under another, the problem log; File › Preferences… → hover "Economy UI"
  → the status line says what the switch does
- **TEST** t1146 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: The graphs grow with the window
- **WHEN** the Diagnostics window is dragged by its bottom-right corner to a larger size
- **THEN** each of the three graphs gets taller, all three and the log stay inside the window,
  and the window keeps the size it was given over the next frames (it does not grow by itself)
- **PATH** open Diagnostics → drag its bottom-right corner down and right → the three graphs
  stretch to the new height, the log stays at the bottom; leave the mouse → nothing moves
- **TEST** t1147 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: Diagnostics opens at its floors
- **WHEN** the Diagnostics window opens for the first time (egui knows no size of it) on a screen with the room
- **THEN** it opens tall enough for the instruments, the three graphs at their floor and the journal's six lines: the third graph ends above the journal's head row, and a corner drag gives each graph a quarter of the growth
- **PATH** click the gauge button → the window shows the Performance rows, the memory line, all three graphs whole and the journal under them, nothing under a fold; drag its corner down → each graph grows by a quarter of what the window grew
- **TEST** t2304 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: An open Diagnostics window does not heat an idle program
- **WHEN** the Diagnostics window is open and nothing plays or moves
- **THEN** the frame asks for its next repaint no sooner than the idle tick (≥ 100 ms) and the
  graphs rebuild nothing between frames; during playback they follow the samples
- **PATH** open Diagnostics and leave the mouse → the graphs stand still, the program is idle;
  press Space → the lines run; Space again → they stop
- **TEST** t1148 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: A graph is drawn from its last sample's build
- **WHEN** a rolling graph is drawn several times without a new sample, then gets one sample,
  then is drawn in a larger rectangle
- **THEN** the repeated draws reuse the built lines (no rebuild), the new sample rebuilds once,
  the larger rectangle rebuilds once; the auto-scale moves only on a sample, so ten draws of the
  same data paint the same scale
- **PATH** open Diagnostics and press Space → the lines step on every half-second sample and hold
  still between samples; the scale eases to a new peak over a few samples, as fast at the idle
  tick as at 60 fps
- **TEST** t1149 (`src/apps/kt-app/src/graphs.rs`)

### Requirement: Localization — five catalogues, not one label missed
The source language of literals SHALL be English; UI text SHALL live only in the
catalogs `i18n/{en,ru,es,zh-Hans,ar}.toml` and be taken via `tr`/`tf!` with
positional placeholders; there is no plural form ("noun: N").
Catalogs SHALL be complete, with no extra keys and no unused ones. Every
non-ASCII character of string literals and catalogs SHALL have a glyph in the six
loadable fonts; icons come from Phosphor (PUA) or as vectors; ideographs and
Arabic come from Noto subsets assembled by a script. The font order MUST NOT
change. Arabic is reordered WORD BY WORD (shaping atoms); the content of a
`TextEdit` — never. Every window SHALL have an explicit id. Strings in a background
thread MUST NOT be translated. The language on first launch comes from the OS locale; the choice
is stored outside the project. Layout is checked with the real fonts in all languages.
Cyrillic (U+0400…U+052F) SHALL appear only in `i18n/ru.toml`, in the endonym `Русский` of the
`[lang]` section of every catalog, and in test data declared as such; a non-Russian catalog and
a committed project fixture MUST NOT carry it.
Why: the user twice got a square placeholder stub; changing a window title
would reset its position; per-character reversal makes an Arabic phrase worse.
2026-10-27 (ktprompts 26.10.18 item 10): "in a composition's fx-chain there is a published
element with parameters, and in the English version it literally says «Лента»" — t131 already
kept Cyrillic out of string literals, so the word came from somewhere a literal guard does not
look: a name the program itself stored in the project.

#### Scenario: Glyphs of all non-ASCII characters
- **WHEN** the kt-app sources (recursively) and the `i18n/*.toml` catalogs are scanned, expanding `\u{…}`
- **THEN** every non-ASCII character has a glyph; test code is cut off at `#[cfg(test)]` with zero indentation
- **TEST** t100 (`src/apps/kt-app/src/font_tests.rs`)

#### Scenario: CJK and Arabic subsets are rebuilt from the catalogs
- **WHEN** a new ideograph or an Arabic ligature is added to a `i18n/*.toml` catalog
- **THEN** the Noto subsets are rebuilt by the script from the catalog contents (`python3 scripts/subset-fonts.py`), otherwise t100 turns red on the new glyph; the full font is not put into the binary
- **TEST** t100 (`src/apps/kt-app/src/font_tests.rs`) after `src/scripts/subset-fonts.py`

#### Scenario: Catalogs are complete and used
- **WHEN** the build generates tables from the catalogs
- **THEN** a missing/extra key, a mismatch of placeholders, PUA in a catalog, an ASCII comma in Arabic break the build; a key that nobody calls by its FULL path is red; the parser is strict
- **TEST** t125 (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: Arabic — the order of words, not of letters
- **WHEN** a phrase of two Arabic words is rendered; the search sections are read
- **THEN** the leftmost glyph of the word is the last logical letter, the second word is to the left of the first; the sections `[fxreg.kw]`/`[lang]` are not reordered
- **TEST** t126 (`src/apps/kt-app/src/i18n_tests.rs`), t126 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Language switch on the fly and locale detection
- **WHEN** three windows are open, the language is switched; first launch with `LC_ALL`/`LANG`
- **THEN** the window positions are kept (explicit id); `zh_TW`/`zh_HK` → simplified, `C`/`POSIX` → English; the user's choice overrides
- **TEST** t129 (`src/apps/kt-app/src/layout_tests.rs`), t130 (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: Literals in English
- **WHEN** `src/**/src/**/*.rs` of the whole workspace is walked
- **THEN** zero Cyrillic in string and character literals outside test code
- **TEST** t131 (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: Cyrillic only where it is the subject
- **WHEN** the catalogs `en`, `es`, `zh-Hans`, `ar` are scanned value by value, and the committed project and exchange fixtures (`src/fixtures/**` `.kt`/`.otio`/`.ktproj`, the live-scenario projects) are scanned as text; a Cyrillic word is temporarily put into `es.toml` and into a copy of a fixture
- **THEN** the only Cyrillic found in the four catalogs is the endonym `Русский` of `[lang]`; a fixture carries Cyrillic only if it is in the test's declared list of Cyrillic test data, each with a reason; both temporary additions are red with the file and the key or line
- **PATH** there is no gesture: the guard reads the catalogs and fixtures and names the file and the key or line of a Cyrillic word that does not belong there — it is verified by running the test, not by mouse
- **TEST** t933 (`src/apps/kt-app/src/i18n_tests.rs`)

### Requirement: Compositions in the interface (capability `compositions`)
Gestures over the reels SHALL obey the shared layout rules: what plays is decided
in one place; entry and exit are identical by look and cursor; Enter — a cascade inward,
Esc — a cascade outward; the selection lives in one reel; inside a reel, zero is not a wall
for gestures but is a wall for the cursor; the reel preview is ordered once per key and
does not depend on scrolling; the badges and author names are drawn verbatim.
The composition parameters of an INSTANCE SHALL be edited in the FX window of that instance — by the pinned
head of the chain strip (the same machinery as the generator: `HeadSlot`, `ChainSel::Head`),
where the author's handles with their widgets by type, the reset, and the same automation section live;
a separate instance parameters window MUST NOT exist; the menu item and the double-click on the instance
lead to this window with the head selected. The author's "Public parameters" window (inside the reel)
remains.
Why: the content contract is in capability `compositions`; only the layout and interactions
are fixed here. R.63: "composition parameters open in a separate window,
when they can be placed in the same fx-chain" — and the owner's ban on windows without an obvious
purpose.

#### Scenario: Entry, exit, cascades and minimap
- **WHEN** the reel is collapsed and expanded by hand; entered by four paths and exited with Esc; Enter pressed twice
- **THEN** the look and the cursor return identically (a scene with the SHIFTED instance), Esc exits last, the minimap and the ×N badge stay in place; inside the reel, the reel plays; entry brings the look to the cursor
- **TEST** t430b, t456, t457, t458, t552, t553, t597 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Expansion strip in place
- **WHEN** one expands in place, edits from the strip, clicks in another strip, presses Esc; expands inside a full entry; changes the context
- **THEN** the document and what plays do not change; the edit reaches the reel; the selection moves with a message; Esc collapses the strip to the exit; the strip opens inside an entry too and survives a context change
- **TEST** t570, t571, t572, t573, t634, t636 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Zero inside a reel
- **WHEN** an event inside a full entry is dragged to the left of zero; the view is scrolled to zero at the root and in the reel
- **THEN** the event goes past zero, the cursor never does; the view goes to zero only in the reel; there are no bare `.max(0)` over the time of a gesture
- **TEST** t637, t637b (`src/apps/kt-app/src/layout_tests.rs`), t637c (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Bin, project drop, insertion by click
- **WHEN** a `.otio` is dropped onto the window; inserted from the strip to the click point; a track is added under the ceiling
- **THEN** the program asks and does not lose work; insertion as a composition is one step; the pair lands at the click point; the new track is the top layer; the reel is a bin object with all its verbs
- **TEST** t576, t577, t578, t594, t596 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Reel preview is a service, not a drawing
- **WHEN** the reel is visible on the timeline, is scrolled and zoomed; the project is opened a second time; instances are overridden in different ways
- **THEN** a request is placed once per key, scrolling orders no work, the second opening has zero renders (the disk tier), the preview key is per instance
- **TEST** t599, t601, t640, t680 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Instance parameters window and badges
- **WHEN** the FX window of an instance is open (double-click, menu item "Composition parameters"); the handle name is set by the author; overrides zero/N; a number and a colour are edited, the reset is pressed; a curve is added via "Automation: add curve…"
- **THEN** the strip head is pinned first and shows the author's handles, each type with its own widget; the reset sends `ClearOverride`; the name is drawn verbatim (galleys, not the accessibility tree); the pin and the count appear only when non-zero; the curve is visible with the same automation widget, the keys are in the instance's material; there is no separate parameters window (`InstParamsWin` removed); the assertions of the earlier t683/t684/t685/t691 are performed on the strip head
- **TEST** t754 (`src/apps/kt-app/src/layout_tests.rs`), t683, t684, t685, t691 (`src/apps/kt-app/src/layout_tests.rs`, moved to the strip head by the owner's decision R.63), t693 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Any layout edit is accompanied by a harness test that asserts the consequence
A change of layout or interactive behaviour SHALL be accompanied by a test in
`layout_tests.rs` (egui_kittest, real frames without a window and GPU, real
fonts). The test SHALL assert WHAT THE PERSON SEES AND DOES (the rectangle
grew, the click landed, the window command went out), not "the field was set"; probe FROM THE POINT
WHERE THE PERSON ACTS; check the click, not the hover; a drag — in three steps;
a double click — with a small step and its own harness; the release — as an explicit event;
the test MUST turn red on the old code. The harness is not proof: the item
is closed by a live scenario and by the build of what the user launches.
Why: three layout breaks in a row were caught by nothing; the metres limit was green
in the test, while the user launched a stale `target/debug/kt-app` that had not been rebuilt.

#### Scenario: Click, not hover
- **WHEN** the test of the "Close" button at the top edge
- **THEN** the press and the release are checked with their consequence (the window command), not the appearance of a hint
- **TEST** t76 (`src/apps/kt-app/src/layout_tests.rs`), t101 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Three-step drag and double click with its own harness
- **WHEN** the harness drags an event and double-clicks
- **THEN** press → 8 px → target (otherwise the capture offset is counted at the target); the double click with `with_step_dt(0.02)` and a new harness for each gesture
- **TEST** t204, t207 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The "no dead ends" mechanism (capability `ui-reach`)
- **WHEN** any UI is edited
- **THEN** there are no orphaned commands, each entity has a full set of verbs, a click reaches its target without `dnd_drag_source`, a long name is truncated
- **TEST** t252, t253, t254, t255 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: Shapes, derived graph and formula in the automation widget
The shape of a segment SHALL be edited from the same automation widget as the keys: by right-clicking
the SECTION between two keys (neighbouring sections are not touched), with the shape's parameters
shown next to it; the same actions are available for a selection of several sections. The target of a right
click (a section or a key) SHALL be fixed AT THE MOMENT of pressing and held while the menu is open: the movement
of the pointer towards the menu items MUST NOT close it and MUST NOT change the target; the section under the menu
SHALL be highlighted on the graph, and the menu header SHALL name its bounds (the times of both
keys of the material and the fractions of the event) — the person sees BETWEEN WHAT they are setting the shape.
The graph SHALL draw the VALUE of the curve — a line built by the same evaluator that the render uses, with a
step no coarser than one pixel per frame — and not the segments between keys: a zigzag is seen as a zigzag,
a step as a step, the segment formula as its own line; there is no second drawing formula.
A section with a shape that has a parameter SHALL have a HANDLE under the mouse at the middle of the section:
a vertical drag edits the main parameter of the shape (the tension for third order and ease,
the amplitude for the zigzag), a horizontal drag with Shift — the number of periods; the handle is drawn at
the section under the pointer and at the selected one, the number is shown next to it for the duration of the drag, and the history
step is one per gesture.
The "curve as a parameter" block (the choice of shape for all segments and its parameters, the formula layer, the link,
delete the curve) SHALL be ONE per effect slot, generator and reel instance, and SHALL
refer to the curve that the person SEES as selected: the block header names it by its name
and colour, and the choice of curve is made by a click on its name, on its line or on its field; a block
without a named curve MUST NOT exist.
A curve with a DERIVED source (a link or a formula) SHALL be drawn distinguishably from
a keyed one and MUST NOT be edited with the mouse: it has no keys, and dragging a point would silently
create a third truth about the value; at the same time its VALUES SHALL be drawn — resolved by the same code
as the render (`resolve_derived`), and not from an empty list of
keys; the `t` of the preview SHALL be counted from the start of the material, as in the render. A curve with a
FORMULA LAYER SHALL be drawn with two lines: `k` (the keys with shapes, edited with the mouse) and
the layer's result (dashed, not editable) — the person sees both what they set and what
sounds. A derived curve SHALL show what drives it — the source address or the text of the
formula — and have one action "unlink" that turns it into ordinary keys at the current
values. A link SHALL be assigned by the "Driven by…" DOOR in the same block: the choice of event (by
default its own) and the properties from the passport of its carriers (slots, geometry, generator), with the
same recalculation text next to it; an unresolvable address is named as an error, not silence.
The formula field SHALL hold the TYPED text between frames (the text lives in the widget state,
not re-read from the model every frame): an unfinished input is not erased, a parse error is shown with its position and
name and is not applied to the model; the text is applied to the model on confirmation (Enter, loss of focus) — and MUST NOT be applied on the
first legal prefix (typing `t*2` does not turn the curve into "t" after the first letter, the keys
are not zeroed before confirmation). The formula reference SHALL be IN THE BLOCK (a collapsible
group: the variables `t`/`k`/`src`/`s`/`a`/`b` with their purpose, functions with their arity, operators)
and in the status line (the field hint names the variables; the error "no such name" lists the
closest by prefix); Tab in the field SHALL complete the name by prefix in place, without a list
over the interface. The shape default and the command "apply to all" are reachable from the same place
as the other actions on the curve. The width of the numeric fields, the formula field and the shape choice
SHALL be fixed by the galley of the longest value and MUST NOT depend on the window width. All new controls SHALL have a
hint in the status line.
Why: the owner asked for "the final graph is also drawn, but not edited with the mouse";
an editable derived graph would be exactly the "mute dead end" for which the project
paid seven rounds. R.63: "I write it, and it either erases the formula or nothing
happens" — the field re-read the model every frame, applied on the first legal letter
and zeroed the keys, while the graph drew empty `keys`; "the context menu simply disappears,
I can't reach it in time" — the target was computed from `hover_pos()` every frame. R.67: "visually
the user cannot see at all what kind of curve is there, and there is not even a way to
control it with the mouse instead of typing digits" — the graph drew straight lines between the keys, and t698 checked
the model, not the drawing; "only one field changes, although I set zigzags in two fields" —
"the shape of all segments" silently affected the active curve; "view curves — I don't
understand at all how to assign them" — there was no door; "no reference, no hints at all";
"the fields grow in width again with the window". Popups over the interface are forbidden (`ui-reach`, rejected),
therefore the reference is a group in the block, and completion is Tab in place.

#### Scenario: The shape is edited per section, neighbours intact
- **WHEN** a right click on a section selects a shape, then its parameter is edited, then the shape is applied to all sections of the curve
- **THEN** only the selected section changes and only its parameters; the graph is redrawn by the same value function that the render uses; "to all" is one undo step
- **TEST** t698 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A derived curve is visible but not edited with the mouse
- **WHEN** the handle's source is a link; a drag and a double-click are made on its line; then "unlink" is pressed
- **THEN** the line is drawn distinguishably and labelled with its source; neither the drag nor the double-click changed the model; "unlink" gave ordinary keys at the current values in one step
- **TEST** t706 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The formula field explains the error and does not spoil the model
- **WHEN** text with an unclosed bracket is typed into the formula field, then with an unknown name, then a correct one
- **THEN** the first two show an error with its position in the line and do not touch the model; the correct one is applied; the field has a hint in the status line
- **TEST** t709 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The formula is typed in full, the keys stay alive until confirmation, the curve is drawn
- **WHEN** on a curve with three keys, `sin(t)*0.5` is typed into the formula field character by character with pauses of several frames, then Enter; then `t*` is typed and focus is moved away; then the text is erased
- **THEN** after each character the field shows what was typed (in the previous code `sin(` disappeared on the next frame); until Enter the curve's keys are intact and the model is untouched (in the previous code `t` was applied on the first letter and zeroed the keys); after Enter the source is the formula, and the graph draws a line with a non-zero number of points, equal to `resolve_derived` at the same ticks; the unfinished `t*` is not applied when focus leaves, and the error is named; empty text returns the keys by "unlinking", not silently
- **TEST** t749 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/formula.py`

#### Scenario: The section menu stays under the pointer and says what the shape is between
- **WHEN** right click on a section between keys at 1.0 s and 2.0 s; the pointer is moved 40 px to the menu and the shape is clicked; right click on a key and the same movement towards the "Delete" item
- **THEN** the menu did not close from the movement (in the previous code it disappeared on the very first frame outside the section); the header names 1.000 and 2.000 s and the fractions of the event; the section is highlighted (a backing is drawn between the x of the keys); the shape lay on this section; the key menu likewise stays and deletes exactly that key
- **TEST** t751 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/formula.py`

#### Scenario: The graph draws the shape as it sounds
- **WHEN** a curve has a zigzag section (4 periods, amplitude 0.2), a step section and a formula section `a + (b-a)*s*s`; the graph is drawn at two zoom levels
- **THEN** the drawn line at 16 equally spaced x between the keys of each section matches `curve_value` at the same ticks to within a pixel (in the previous code — a straight line between the keys: a deviation at the zigzag peak of ≥ 10 px); the zigzag has no fewer than 8 extrema; the number of points of the line is no less than the width of the section in pixels / 2
- **PATH** open the FX window of an event with an effect → add a parameter curve → place three keys by clicking on the graph → right click on a section → "Zigzag" → between the keys a trembling line is visible on the graph, not a straight one
- **TEST** t784b (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: The shape parameter handle under the mouse
- **WHEN** on a zigzag section the pointer goes to the middle of the section, the handle is dragged up by 20 px; on a third-order section — down; on a zigzag section with Shift — to the right
- **THEN** the zigzag amplitude has grown, the tension has decreased, the number of periods has grown — each as one history step; during the drag the number is shown next to the handle; the graph is redrawn with the new value; a straight line and a step have no handle
- **PATH** hover over the middle of the zigzag → a small square appears → drag it up → the zigzag is taller, the amplitude number is next to it → release → Ctrl+Z restores it
- **TEST** t785b (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: The curve block refers to the visible curve and is one for all carriers
- **WHEN** a slot has two curves; the person clicks on the name/line/field of the second, chooses "shape of all segments — zigzag" and edits the amplitude; the same is done in a generator and in the head of a reel instance
- **THEN** the zigzag went onto the SECOND curve, the first is untouched (in the previous code the shape went onto the invisibly active first); the block header names the second curve by its colour; the reel instance has the same block (the same fields and the same labels, one code) — "shape of all segments", the layer, the link and deletion are available there too
- **PATH** FX window → two curves → click on the name of the second → the block under the graph is labelled with its name and colour → choose "Zigzag" → the second line trembles, the first stays as it was
- **TEST** t785, t791 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: The formula layer over the keys, in the graph and in the field
- **WHEN** on a curve with keys and a zigzag, `k * 0.5 + 0.1` is typed into the layer field and Enter is pressed; then the field is cleared and Enter pressed
- **THEN** the graph shows two lines: `k` (solid, the keys with diamonds) and the result (dashed); the keys and the zigzag are intact; after clearing, there is no dashed line, `k` is as before; the edits go into the session of the FX window (one history step per session, as for the whole window); when the field is closed, focus is released (Space again sets pause)
- **PATH** curve block → "Layer" field → type `k*0.5+0.1` → Enter → a dashed line half as low appears above the keys → erase → Enter → the dashed line disappears
- **TEST** t787e (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: The link door assigns the source
- **WHEN** in the curve block "Driven by…" is pressed, one's own event and a property of the neighbouring slot are chosen; then another event and its property; then the link is removed with "unlink"
- **THEN** the curve has a link source with the address of the chosen property, the curve name shows the address, the line is dashed; for the other event the address carries its number; an impossible address (across the reel boundary) is named as an error in the status line; "unlink" returns the keys; the item is reachable from the FX window, the generator and the reel instance
- **PATH** curve block → "Driven by…" → list "this event / others" → choose a property → the curve name becomes "amount <- fx.0.radius", the line is dashed → "Unlink" → the keys are back
- **TEST** t788 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: Formula reference in the block and in the line, completion by Tab
- **WHEN** the "Reference" group in the curve block is opened; `cl` is typed into the formula field and Tab is pressed; `foo(t)` is typed and Enter; the cursor stays in the field
- **THEN** the group lists all variables and all functions of the evaluator with their arity (the list equals the evaluator's registry, not a copy); Tab completed to `clamp(` without a popup list; the error names `foo` and the closest names; the status-line hint for the field names `t`, `k`, `src`; no layer above `Order::Middle` appeared
- **PATH** curve block → "Reference" → one sees `t — seconds of material`, `k — keys…`, `clamp(x, lo, hi)`… → type `cl` in the field, Tab → it becomes `clamp(` → type `foo(t)`, Enter → below, the error "no name foo; there is floor"
- **TEST** t789 (`src/apps/kt-app/src/layout_tests.rs`), t789b (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/curves.py`

#### Scenario: Field width does not depend on the window width
- **WHEN** the FX window with a curve block is opened at screen widths of 900 and 1800 px; the rectangles of the numeric fields, the formula field, the layer field and the shape choice are measured
- **THEN** the width of each field is the same at both widths (in the previous code the shape choice and the formula field grew beyond the window); the width of a numeric field equals the galley of the longest passport value plus the field
- **PATH** open the FX window → stretch the main window twice → the fields in the curve block stayed the same width
- **TEST** t790 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Project settings window — one body with quick access
Project settings SHALL be edited in the "Project settings" window: the raster by two fields with
validation (even, 16…8192), the frame rate — a list of divisors of 90 000 in 1…120 plus a field with
the check `90000 % fps == 0` (an unexpressible value is refused with a reason in the error style, not
silence), the audio rate, the default conform rule, the pitch engine, the segment shape.
The combo in the preview panel SHALL remain a quick access to the same commands (one body,
rule 17). The window SHALL open from the project menu and from the address in the status line,
release focus on closing, and open again. Applying the window SHALL set the flag
of explicit project settings.
Why: "at least there is no project settings window as in serious programs" (the owner,
R.62); five resolution presets express neither 1440×1080 nor a vertical project.

#### Scenario: Validated fields and reopening
- **WHEN** the window is opened twice in a row; 1441×1080, 29.97 fps, 1440×1080, 50 fps are entered
- **THEN** the odd width and 29.97 are rejected in the error style with a reason; 1440×1080/50 are applied by one command; the preview combo shows the same; focus is released; the flag of explicit settings is set
- **TEST** t720b (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Source setup — the first clip sets the project, a mismatch is reported
Placing a video on the timeline of the root, where there are NO events and the project settings are not explicit, SHALL
adopt the project raster equal to the source's view (with the pixel aspect ratio, aligned to even) and the fps of the source, if it divides 90 000; otherwise the fps is not touched and the status line
says why. Inside a reel the rule does not apply. Explicit settings MUST NOT be changed
by placement (rule 20). Placing a source whose view differs from the container raster
by more than 1 % SHALL report in the status line (one hint in one place) that the source was conformed by the project rule and where this can be changed ("Event geometry / Project settings"); badges and questions MUST NOT exist (rule 4).
Why: Premiere asks "change sequence settings to match the clip", Kdenlive — "adjust project settings to match the clip"; silence is the worst part of today's defect
(rule 16).

#### Scenario: An empty project takes the settings of the first clip
- **WHEN** 1440×1080/25 is dropped into an empty project with implicit settings; then, after the settings window was applied, 640×480/30 is dropped; then 29.97 is dropped into the empty project
- **THEN** the first case — the project becomes 1440×1080/25 in one undo step together with the placement; the second — the settings are untouched; the third — the raster is adopted, the fps is as before, the hint names the reason
- **TEST** t721 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A mismatch is reported with its address
- **WHEN** 4:3 is dropped into a 16:9 project; then 16:9
- **THEN** in the first case the hint names the rule and the address "where to change it"; in the second there is no hint; the event is placed in both cases
- **TEST** t722 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Leading node in the chain strip and thumbnails in source proportions
The effect strip of a video event SHALL show the leading node "Geometry" first on creation,
with the leading flag; the cross at the leading node SHALL give an error with a reason, rather than go dark;
the catalog SHALL contain "Geometry" as an addable node; the conform rule SHALL be
localized in all five catalogs. The labels of the fields of the leading node SHALL carry the same publishing door
as the labels of any slot (address `ParamOwner::Geometry`): inside a reel, a right
click on the label of the zoom, position, crop, rotation or anchor of the leading "Geometry" publishes
a handle, as for the second "Geometry" node in the chain. Filmstrip thumbnails SHALL be ordered as
the source view in the thumbnail frame and drawn with the colour margins of the row; scrolling and zooming still
do not generate decode requests.
The form of the leading node in the FX window SHALL be a "Transform" section after the industry model:
collapsible groups (frame / position / crop), X/Y pairs on one line, a unit on each
number (× for zoom, ° for rotation, % for crop and anchor), a slider for zoom and rotation next to the
number, a frame diagram showing the crop and the anchor, and one action "reset geometry".
The value shown by the field and the value given by the on-screen gesture SHALL be ONE number
(rule 17).
Why: the combo `Fit/Fill/Stretch` showed `Debug` names and did nothing; the filmstrip
of a vertical clip was squashed — the first place where the user could see the defect.
R.63: "for what reason can the parameters of the first geometry not be published, while the
second one can?" — the leading node's address is the event field `geometry`, and there was no owner
variant for it; the door was closed deliberately (`(!lead_sel).then(...)`). R.64: "plain boring
input fields, even Vegas does it better" — and the owner's direct clarification "item 2", that is
the "Transform" section with groups, sliders and a diagram, not a bare grid of numbers.

#### Scenario: The leading node in the editor
- **WHEN** the editor of the chain of a new video event is open; the cross at the leading node is pressed; the node is dragged after Blur; a second "Geometry" is added
- **THEN** the leading node is first and marked; the cross is an error with a reason; reordering and the second node are one history step each; the rule's labels come from the catalog, the glyphs are in the font
- **TEST** t716b (`src/apps/kt-app/src/layout_tests.rs`), `i18n_tests.rs`, `font_tests.rs`

#### Scenario: Changing the rule on pause rebuilds the preview
- **WHEN** the project is paused, and in the settings window the rule is changed from "fit" to "fill"
- **THEN** the preview is rebuilt without any cursor movement: a pixel of the field becomes a pixel of the source; the project version is part of the preview key
- **TEST** t722b (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/conform.py`

#### Scenario: Thumbnails of a vertical clip
- **WHEN** a vertical clip is on a track with a filmstrip; the timeline is scrolled and zoomed
- **THEN** the thumbnail is in the proportions of the source with margins on the sides (centred in the cell); the sidecar table `thumb_meta` carries the thumbnail size of the media (old squashed ones are not read); there are no decode orders on scroll/zoom
- **TEST** t719 (`src/apps/kt-app/src/thumbs.rs`, `src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The leading geometry is published by the same door
- **WHEN** inside a reel a right click on the "Zoom" label of the leading node and on the "Zoom" label of the second "Geometry" node; outside the reel — the same
- **THEN** in both cases inside the reel there is a publish item (in the previous code the leading one had none); the handle lands in the reel with the address `Geometry`; outside the reel there is no item; the instance field shows and edits the zoom of the leading geometry, the instance preview changes
- **TEST** t752b (`src/apps/kt-app/src/reach_tests.rs`), t752c (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The "Transform" form instead of a bare grid of numbers
- **WHEN** the leading "Geometry" node of a video event is open; the groups are collapsed; zoom and rotation are edited with a slider and a number; "reset geometry" is pressed
- **THEN** position and anchor are set as X/Y pairs on one line, each number has its own unit, the frame diagram shows the crop and the anchor, the collapsed state of a group is a state of the window (not of the project), the slider and the number edit ONE value, the reset returns the node to its default in one history step
- **TEST** t759 (`src/apps/kt-app/src/layout_tests.rs`), t759b (`src/apps/kt-app/src/i18n_tests.rs`)

### Requirement: Track on screen — badges, properties window, chain, lock as a refusal
The track header SHALL carry a row of badges M / S / lock / FX under the name (service rows —
only the lock); the width of the header strip SHALL be fixed by the galley of the row and not change
with state (rule 3), the grab zone of a badge is at least 14 px (rule 1); the badge works
repeatedly and during playback. The same verbs SHALL be in the right-click menu and on the
hotkeys ALT+M / ALT+S — one body (rule 17); bare M and S remain with the marker and the cut
(the owner's bindings), and the target of the key is the track of the SELECTED event; without a
selection — the row under the pointer. The "Track properties" window SHALL edit the name, level,
blend mode, switches AND THE CHAIN in one session and one undo step, release focus and
close with the Esc cascade. The track chain SHALL be drawn by the same strip widget as that of the
event and the morpher; a track has no automation widget, and envelopes that arrived
with an inserted chain SHALL be rejected with a reason and the address where to put them. A gesture over an
event of a locked track SHALL be shown as an error with the track highlighted, not silently.
The name and the height of the track SHALL go through undo.

DUCKING in the track window SHALL be shown by TWO COMPRESSORS — with the same parameter widget
as any compressor, without its own form of separate fields: on the left the compressor of THIS track
(the background) with the selected key, on the right — the real compressor of the KEY track, drawn muted and editable
right from here; if there is no compressor on the key — it is created by one
action and one history step. The choice of key SHALL be a list of the audio tracks of the same reel with
their names; the target track is absent from the list, tracks that would give a cycle are absent too,
and the reason is named (rule 16). Ducking MUST NOT have its own window, nor a separate set
of four numbers. The edits of both compressors SHALL go in the same window session and the same
undo step as the other track properties, and the device parameters SHALL live in the project (in the
track chain), not in the user storage.
Why: "Dead ends: you can create it, you can't control it" — seven rounds; rule 13 forbids a strip
the full height of the track for the sake of three buttons, so the badges stand under the name in the place already taken.
R.63 (the owner): "meaningless copy-pastes of the same code pile up everywhere, when one can even
visually show the user that ducking works in the program, for example through
two compressors"; "this is trivially more conveniently done through fx-audio on the whole track".

#### Scenario: The badges are hit, do not jump, work repeatedly
- **WHEN** from the person's point M, S, lock, FX on a clip track and the lock on a morph row are pressed, twice in a row, including the edge of a badge at the window edge; the states change
- **THEN** each press reaches the model (the window edge does not steal clicks), the width of the header strip does not change, a repeated press clears the state, a service row has no M/S/FX, ALT+M and ALT+S do the same and do not take the marker and the cut
- **TEST** t742 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The track properties window
- **WHEN** the window is opened twice; the name, level, mode, switches are edited; "Cancel"
- **THEN** the edits are one undo step, "Cancel" rolls back everything, focus is released, the limits come from the passport
- **TEST** t743 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The track chain in its window
- **WHEN** the track window is open; a chain WITH ENVELOPES is inserted into the strip
- **THEN** the strip is the same as that of the event, the effect goes into the model in one step, the curves are dropped with a named reason and an address, not silently; the labels from the catalogs, the glyphs in the font
- **TEST** t744 (`src/apps/kt-app/src/layout_tests.rs`), `i18n_tests.rs`, `font_tests.rs`

#### Scenario: A lock refuses a gesture
- **WHEN** an event on a locked track is dragged, trimmed, cut, deleted; its mute is edited
- **THEN** each gesture is an error with the track highlighted and the reason; the model is untouched; the mute is applied
- **TEST** t745 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Ducking — two compressors, not its own form
- **WHEN** in the window of the background audio track the key is selected — the speech track; the speech has no compressor yet; then the threshold on the background and the threshold on the speech are edited; then the window is closed with "Cancel"
- **THEN** two compressors are shown with the same widget (the second is muted); the speech compressor is created in one history step; both edits are one window session and one step; "Cancel" rolls back both; there is no ducking window of its own, no separate ducking fields, the parameters live in the project
- **TEST** t775 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The key list does not offer the impossible
- **WHEN** the key choice is opened on a track which already serves as the key of another track; and on a track where the key points to a deleted track
- **THEN** the list contains neither the track itself nor those that would give a cycle, and the reason is named in the status line; a dead key is shown as "no key" and cleared by one action
- **TEST** t776 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Transform frame over the preview picture — gestures, damping, one arithmetic with render
The preview SHALL show the transform frame of the leading "Geometry" node of the SELECTED
video event while the display of controls is on: the outline of the frame, eight zoom handles, a
rotation handle, the anchor point and four crop handles. A gesture over the frame SHALL edit the `EventGeometry`
of the same event: the body — the position, the corner and side handles — the zoom, the rotation handle — the angle,
the anchor point — the anchor, the crop handles — the crop. All gestures SHALL run at the tempo of the gesture (the picture
is rebuilt on each movement) and be recorded in the history as ONE step per gesture.

The frame geometry on screen SHALL be computed by THE SAME placement function that builds the geo-program
of the render, and the inverse conversion "shift on the canvas → parameter value" SHALL be its pair:
no second placement arithmetic is introduced into the program. The number that the gesture gives and the number
in the field of the FX window SHALL coincide.

A SMALL FRAME has no right to eat the move: when a side of the frame on screen is smaller than three
grab zones, the eight zoom handles SHALL be drawn and caught OUTSIDE the edges of the frame, and
its body SHALL remain under the move. The rotation handle and the anchor point stay in their
places. The rule SHALL act on the size ON SCREEN, not on fractions of the canvas: the same
geometry at different fractions of the preview gives a different frame.

Dragging the ANCHOR MUST NOT move the picture: the position is compensated by the same history step,
and this holds at any rotation of the layer. The anchor SHALL be grabbed by ITS OWN point without a modifier
(a narrow zone, so as not to take the move of the picture — it sits at the centre of the frame), and a modifier
SHALL only widen its zone to the common one: a gesture available ONLY with Alt never reaches the
program at all — the window manager intercepts Alt+drag. The other handles SHALL be
large enough to hit at the first try (the grab zone is at least 14 px even with a frame of a few pixels)
and MUST NOT overlap: when the frame is smaller than the sum of the grab zones, the priority goes to the handle nearest the pointer.

A parameter with a LIVE CURVE SHALL be edited by a gesture with a KEY at the cursor — by the same rule as
the FX window field — and not by a constant; the frame SHALL show the EFFECTIVE geometry (the base under the
curves) in that case, that is, exactly what the render draws.

The controls SHALL fade during playback and return on pause; the display of controls SHALL
be ON by default (the "Transform" mode), and the toggle — on the preview itself
(a button in the corner of the frame) and in its menu; the state of the toggle is a property of the workplace,
lives in the user storage and comes back at the next launch: switched off — remembered,
reinstalled the program — on again. The frame diagram in the "Transform" form SHALL FIT the frame
into its own rectangle at any zoom and position (the scale of the diagram is chosen by the span of the frame
and the canvas; the canvas is drawn inside the same diagram): at 2× zoom a frame twice
the size of the canvas is visible, not an empty rectangle. Without a gesture the controls MUST NOT raise the cost
of a frame: no allocations in the draw path, no repaint when idle (invariant 12). An event without a leading
node (audio, a foreign placeholder) gets no frame, and the impossibility is named by a status-line
string, not by silence.
Why: R.63 item 6 of the owner — "AT MINIMUM we must have editing right in the preview that is
convenient"; the industry hides the controls during play (FCP — always, AE — automatically), and the compensation
of the anchor on a rotated layer is a defect of After Effects confirmed by quotations, which we have no reason
to repeat (`docs/features/ui/viewer/on-screen-controls.md`). R.67: "instead of
control in the preview we have only meagre control inside the fx widget" — the frame was
off by default, was not remembered, and was switched on only from the right click on the preview,
so for the person it did not exist (checklist rule 4: a useful thing is on by default); "if you scale even by 2x — you simply have nothing in the preview" — the diagram was clipped by its own rectangle.

#### Scenario: Move, zoom and rotation edit the same numbers as the fields
- **WHEN** a video event is selected; the frame is dragged by the body, by a corner handle and by the rotation handle
- **THEN** the event's `pos_x`/`pos_y`, `zoom` and `rot` change; the values in the fields of the FX window match the on-screen gesture to the last digit; one history step per gesture; the intermediate frames go to the player; rotation with Shift steps by 15°
- **TEST** t760, t760b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The anchor moves, the picture stays — and on a rotated layer too
- **WHEN** the anchor is dragged on a layer without rotation and on a layer rotated by 30°
- **THEN** the anchor values change, while the placement of the picture on the canvas stays the same in both cases (a comparison of the placement rectangle before and after); the position compensation is part of the same history step
- **TEST** t761 (`src/libs/kt-fx/tests/placement.rs`), t761b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Crop with its own handles, the margins are transparent
- **WHEN** the four crop handles are dragged
- **THEN** `crop_l`/`crop_t`/`crop_r`/`crop_b` change; the frame follows the cropped view by the conform rule; the cut-off part is transparent (not black)
- **TEST** t762 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A small frame gives its handles outward, the body stays its own
- **WHEN** the frame is smaller than three grab zones on a side (the "as is" rule with a small source or a stub): the pointer is moved over each zoom handle and to the centre of the frame
- **THEN** the handles are caught OUTSIDE the edges of the frame and drawn there too; the centre of the frame gives a MOVE, not a handle; on an ordinary frame the handles stay on the edges, as before
- **TEST** t781 (`src/apps/kt-app/src/layout_tests.rs`), t782 (`src/libs/kt-fx/tests/placement.rs`)

#### Scenario: Hitting a handle and priority with a small frame
- **WHEN** the pointer is moved over each of the handles, including on a frame whose size is smaller than the sum of the grab zones; separately — onto the anchor point and onto a point a few pixels away from it
- **THEN** the grab zone of each handle is at least 14 px, the hit is unambiguous (the nearest to the pointer), the frame body does not intercept a handle gesture; the anchor is grabbed at its own point, aside from it — already the body; with a modifier — the zone is common
- **TEST** t763 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A gesture over an animated parameter sets a key
- **WHEN** the event's `pos_x` has a live curve, and `pos_y` does not; the frame is dragged by the body
- **THEN** the constant `pos_x` is intact, a key appeared in the curve at the cursor with the new value; `pos_y` edits the base, no curve is created for it; the history step is one; before the gesture the frame showed the value of the CURVE under the cursor
- **TEST** t762b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Controls fade during playback and do not heat the idle state
- **WHEN** playback is started; then pause; then idle without pointer movement
- **THEN** during playback there are no controls, on pause they are back; in idle there are no additional repaints and no allocations in the draw path
- **TEST** t764 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The frame is there at once, switching off is remembered, the diagram fits
- **WHEN** the program is launched without storage; a video event under the cursor is selected; then the display is switched off with the button on the preview and the program is restarted; separately — on the event a 2× zoom and a position beyond the edge of the frame are set, the "Transform" form is opened
- **THEN** on the first launch the frame is drawn without any action (in the previous code — never, until the right-click item was found); after switching off and restarting there is no frame, and the button shows "off"; deleting the storage returns the default "on"; the form's diagram draws both the frame and the frame's whole content inside its own rectangle (all four corners of the frame inside; in the previous code — none)
- **PATH** open the project → click a video event → the frame with handles is immediately on the picture → the button in the corner of the preview switches it off → close and open the program → no frame, the button shows "off" → switch on → it is there
- **TEST** t794 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/geomctl.py`

#### Scenario: Live scenario of a gesture over the picture
- **WHEN** in the real kt-app an event is selected, the frame is dragged by the body, by a corner, by the rotation and by the anchor
- **THEN** the preview pixel changes on move, scale and rotation and does NOT change on the anchor move; the file after saving carries the new geometry values
- **TEST** `src/scripts/live/geomctl.py` (live scenario, `./scripts/live-check.sh`)

### Requirement: The preview share is similar to the project frame
The preview fraction SHALL give a size similar to the project frame: the aspect ratio of the preview differs
from the aspect ratio of the project by no more than half a pixel on the short side. The conform rule on the preview and in the export SHALL give
one and the same frame in fractions of the canvas.
Why: rounding both axes to even independently gave 1920×1080 → 240×134 at 1/8 and 120×66
at 1/16 (1.791 and 1.818 against 1.778), so "fit" computed a different frame on the preview than
in the export, and the on-screen control would show something other than what goes into the file.

#### Scenario: All fractions are similar to the project frame
- **WHEN** the preview fraction is 1/1, 1/2, 1/4, 1/8 and 1/16 with a project frame of 1920×1080 and of 1080×1920
- **THEN** the aspect ratio of each fraction matches the project's within half a pixel; the "fit" frame in fractions of the canvas is the same at all fractions
- **TEST** t765 (`src/apps/kt-app/src/app.rs`, `src/libs/kt-fx/tests/conform.rs`)

### Requirement: Head of the chain strip of a reel instance — a short label, not an accent
The head of the chain strip of a reel instance SHALL be labelled with a pin and the NAME OF THE REEL,
truncated by the same rule as the slot name (the full name is in the status-line hint), and
drawn in the colour of an ordinary slot, not the accent; the selected head SHALL take the same
width as the unselected one. The chain strip with a head SHALL leave the slots no less
room than without it, minus one slot width.
Why: R.67, the owner: "why should the user block fx-chain with a long label
"Composition parameters"? It is also blue… if you select it, you get a long blue
rectangle" — rules 7 (the text fits) and 13 (do not waste space) of the checklist.

#### Scenario: The head is short and not blue
- **WHEN** a reel instance with a 40-character name is opened in the FX window; the head is selected
- **THEN** the width of the head is no more than the width of the slot with the longest name (truncated with an ellipsis), the colour of the label equals the colour of the slot label, the width of the selected head equals the unselected one; the status-line hint carries the full name
- **PATH** FX window of the instance → at the start of the strip "📌 Reel…" the width of a slot → click on it → the highlight of the same width → the full name at the bottom of the line
- **TEST** t793 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/forms.py`

### Requirement: A chord with Alt or Ctrl does not also fire its bare letter
A hotkey that is a bare letter (split `S`, marker `M`, region `I`/`O`, loop `L`,
navigation `A`/`D`) SHALL act only when neither Alt nor Ctrl is held. A chord that adds
Alt or Ctrl to that letter (`Alt+S` solo, `Alt+M` mute, `Ctrl+S` save, `Ctrl+L` go to
timecode) SHALL perform only its own action. Shift is not covered by this rule: `Shift+A`/`Shift+D`
are the same jump with the region edge dragged along.
Why: the split condition excluded only Ctrl, so `Alt+S` toggled solo AND split the event
under the playhead in the same frame. The t742 check of `Alt+S` asserted only the solo, so
the split went unnoticed. Found while building the full shortcuts reference (2026-10-26).

#### Scenario: Alt+S only solos
- **WHEN** an event is selected and the playhead is inside it; `Alt+S` is pressed; then `Alt+M`
- **THEN** the event's track is soloed, then muted; the number of events and the selected event's range are unchanged; a bare `S` afterwards still splits it
- **PATH** a clip is selected, the playhead stands inside it → Alt+S → the S badge in the track header lights up, the clip stays whole (no cut line at the playhead) → Alt+M → the M badge lights up, the clip is still whole → S → the clip is cut in two at the playhead
- **TEST** t868 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A chord read on a stalled frame stays a chord
- **WHEN** one frame reads Ctrl down, S down and Ctrl up together, so the frame's own modifiers no longer hold Ctrl, with the playhead inside the selected event
- **THEN** the project is saved and the event is not split: a chord is what was held when its key went down
- **PATH** the playhead inside a selected event → the interface stalls for a moment → Ctrl+S pressed and released meanwhile → the picture comes back → the status line says the project was saved, the event is whole
- **TEST** t1672 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A fixed dark theme and one decision about the window frame

The interface SHALL be drawn with our dark visuals whatever theme the system reports: the theme
preference is Dark, and our visuals are those of the dark style. A light system theme (wine
reports one; some Linux desktops do) MUST NOT bring egui's default light style into any part of
the interface.

The window frame SHALL be one decision, asked by everything that depends on it (the window
builder, the window buttons, the resize edges, the no-resize zone of the menus):
- borderless — our own title row with the logo, the menus and the window buttons — by default,
  on Linux and on real Windows;
- the system frame when the program runs under wine, or when `KT_DECORATIONS=1`;
- borderless when `KT_DECORATIONS=0`, also under wine.

With the system frame our window buttons are not drawn and the resize edges are the system's;
the logo and the menus stay in our row below the system bar.

Why: `set_visuals` changes only the current theme's style, so a light system theme switched
egui to its default light style and painted the standard parts white over our dark panels
(the owner, under wine, 2026-10-26). wine lets the window manager decorate every window, a
borderless one too; the frame covered the top 22 px of our window — the menus and the window
buttons — and cut the status bar. The owner's priority: real Windows stays borderless with no
Windows title bar; under wine it must simply work.

#### Scenario: A light system theme does not reach the interface
- **WHEN** the harness reports a light system theme to the program for several frames, and then a dark one
- **THEN** the context's theme stays Dark throughout, the panel and window fills are ours (`theme::PANEL`, `theme::WINDOW`) and the inactive widget fill is our dark one — the same values as with no system theme reported
- **PATH** there is no gesture: the system switches to a light theme while the program is open → the preview's settings strip, the buttons and the windows stay dark, nothing turns white
- **TEST** t862 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The frame decision per environment
- **WHEN** the decision is computed for: no variable, not under wine; no variable, under wine; `KT_DECORATIONS=1`, not under wine; `KT_DECORATIONS=0`, under wine
- **THEN** it is borderless, system frame, system frame, borderless; with the system frame the window buttons are not drawn (the harness finds no window-button rectangle), with borderless they are
- **PATH** there is no gesture: the program starts under wine → a system title bar, below it our row with the logo and the menus, no window buttons of ours; started with `KT_DECORATIONS=0` → our row with the window buttons at the top, no system bar
- **TEST** t863 (`src/apps/kt-app/src/window_frame.rs`, the harness part in `layout_tests.rs`)

#### Scenario: Under wine and on real Windows
- **WHEN** the Windows build runs under wine (default settings, a window manager) and on real Windows 10/11
- **THEN** under wine: a system title bar, below it our row with the logo and File/Edit/View, the status bar whole; on real Windows: no Windows title bar, our row at the top with the window buttons, resize from the edges and maximize work
- **PATH** wine: start `kinetren.exe` → the system title bar, below it the logo and File / Edit / View, the menus open on a click → no white strips anywhere; real Windows: start `kinetren.exe` → our row at the top with minimise/maximise/close on the right, no Windows bar → drag the right edge → the window resizes → double-click the row → maximised without covering the taskbar
- **TEST** the wine part — a scripted run in the build container (`src/scripts/wine-shot.sh`, screenshot checked by the agent and kept in `target/wine-out`); the real Windows part — a manual check recorded in `tasks.md`

### Requirement: The bottom toolbar row never cuts off a button
When the bottom toolbar row of the timeline does not fit the window's width, the button groups that
do not fit SHALL move, as whole groups and from the row's end, into a "more" (⋯) menu button that
stays at the row's right end; every moved button SHALL work from that menu exactly as from the row
(the same body, the same hint). When the row fits, there SHALL be no "more" button. The transport
group and the timecode SHALL never move. The row's height does not change.
Why: found by the `tour-lessons-fixes` executor while walking the lessons (kkt-26.11.01): the row
needs about 1460 points, and in a narrower window its tail — Render, Search, Diagnostics, Save — was
clipped with no way to reach it; item 12 of the same round is "nothing looks through a needle's eye".

#### Scenario: Render stays reachable in a narrow window
- **WHEN** the main window is 1100 points wide, then 1700
- **THEN** at 1100 the row ends with a "more" button inside the window, every button of the row is
  either inside the window or in that menu, and clicking Render in the menu opens the render window;
  at 1700 there is no "more" button and Render is in the row
- **PATH** make the program window 1100 wide → the toolbar under the timeline ends with "⋯" at the
  right edge, nothing is cut → click "⋯" → a menu row with Undo/Redo, Render, Search, Diagnostics,
  Save → click Render → the render window opens; widen the window to 1700 → "⋯" is gone, Render sits
  in the row
- **TEST** t1132 (`src/apps/kt-app/src/window_fill_tests.rs`)

### Requirement: Masks are drawn and edited on the preview picture with the frame's gestures
While a mask owner is being edited, the preview SHALL show its masks over the picture through the
same placement the render uses (a picture or slot mask follows the event's Geometry node; a track
mask is in the canvas) and SHALL hide the R.64 frame (one set of handles at a time). Tools: Rectangle
and Ellipse draw by a drag (Shift — square/circle, Alt — from the centre); Pen adds a corner vertex by
a click and a smooth vertex by a click-drag, closes on a click on the first vertex or Enter, Esc drops
an unfinished path, Backspace removes its last vertex. Editing: a drag of a vertex, a tangent, a scale
or rotation handle, the body, or the feather handle on the dashed outer contour; Alt+click on a
vertex toggles corner/smooth; a double-click on a segment inserts a vertex; Delete removes the
selected vertex (a path keeps three — the refusal is shown as an error). Grab zones are the frame's;
the nearest handle wins; every drag is recomputed from the press and is ONE history step; a row with
a live curve gets a key at the cursor only by the gesture. During playback the contours follow the
animation and the handles are hidden. A click on the picture outside every handle and contour still
plays and pauses.
Why: R.64 — "editing right in the preview, convenient"; F-MSK-001's admission that a contour needs a
third kind of editor, introduced once and shared by every owner.

#### Scenario: A rectangle drawn from the preview menu is one step
- **WHEN** a video event is selected under the playhead; the preview's right-click menu → Mask → Rectangle; a drag over the picture; then undo
- **THEN** the event has one rectangle mask covering the dragged box (in source coordinates through the placement), the picture outside it shows the lower layer; the frame's handles are hidden and the mask's are shown; undo removes the mask in one step
- **PATH** right click on the picture → Mask → Rectangle → the cursor is a cross → drag a box over the face → the box stays, outside it the track below shows through, eight handles sit on the box → Ctrl+Z → the mask is gone
- **TEST** t1276 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: The pen draws a Bézier path
- **WHEN** the Pen tool is armed; three clicks and one click-drag place four vertices; a click on the first vertex closes; separately — an unfinished path is dropped by Esc
- **THEN** a path mask with four vertices, the dragged one smooth with mirrored tangents, the others corners; after Esc nothing was created and a second Esc leaves mask editing (the frame returns)
- **PATH** Masks → Pen → click, click, click-and-pull → a curved segment appears under the cursor → click the first point → the path closes and cuts the picture → Pen again, two clicks, Esc → the dots disappear → Esc → the transform frame is back
- **TEST** t1277 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: Dragging a point of an animated path keys the whole path
- **WHEN** a path is keyed by its stopwatch at one tick; the playhead moves; a vertex is dragged; separately — a mask without curves is merely drawn for many frames
- **THEN** every shape row has a key at the new tick and only the dragged vertex moved; drawing without a gesture writes nothing; the drag is one history step
- **PATH** in the mask row click the stopwatch → move the playhead → drag a point → the path bends; the graph below shows keys at both ticks on every point
- **TEST** t1278 (`src/apps/kt-app/src/mask_tests.rs`)

#### Scenario: The preview click and the playback around masks
- **WHEN** a mask is being edited; a click on the picture far from every handle and contour; then playback starts
- **THEN** the click plays and pauses as before; during playback the contour is drawn at the animated position and no handle is hit-tested
- **PATH** with a mask selected click an empty corner of the picture → playback starts → the mask's line moves with the animation, its points are gone → click again → it stops, the points are back
- **TEST** t1282 (`src/apps/kt-app/src/mask_tests.rs`)

### Requirement: A menu label never carries its key; a chord is shown only where it holds
A menu item's translated label SHALL NOT contain its key or gesture. The key SHALL be drawn in the
chord column (`widgets::menu_item`), and a gesture there as a translated chord text (like
"Enter, 2×click"). An item SHALL show a chord only where pressing it in that context does exactly
what the item does. Paste from a context menu pastes at the click, so it shows no `Ctrl+V`. Delete
morpher shows `Del` only when that morpher is the whole selection.
Why: `i18n/README.md` rule 3; five `[tl.menu]` labels carried "(Ctrl+C)", "(Ctrl+V)", "(Del)" and
"(Shift+double click)" in the text, and two of those promises were false where they stood (owner,
2026-10-26: do it, and put the gesture in the chord column).

#### Scenario: No menu label carries a key, and the chords shown are true
- **WHEN** every `[tl.menu]` and `[menu.*]` value (except the `*_accel` chord texts) is read in each of the five languages; the event menu and the empty-space menu with a region are opened by a right click; the morpher Delete chord is asked for with that morpher alone in the selection and with a second morpher selected
- **THEN** no label contains `Ctrl+`, `Del`, `Supr`, `Shift+` or a key in parentheses; Copy shows `Ctrl+C`, Region over this event shows the translated triple-click chord ("3×click" in English; it was "Shift+2×click" until `selection-copy-target` made the triple click the simplest way), Delete clips in the region shows `Del`, both Paste items show no chord; Delete morpher shows `Del` only when it is alone in the selection
- **PATH** right-click a clip → the menu: "Copy" with Ctrl+C on the right, "Paste" with nothing on the right, "Region over this event" with 3×click on the right → Esc → draw a region, right-click empty track space → "Delete clips in the region" with Del on the right
- **TEST** t871 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: The video compose cost is a number, in the panel and in the probe
The performance panel SHALL show the compose time of the last shown video frame and its
average over the last second, in milliseconds, next to the existing FX backend line ("FX:
GPU (<adapter>)", "FX: GPU lost: <reason>" or "FX: no device"). `kt-cli probe <project> <from> <to> --time`
SHALL print, after the audio section, a video section: the frames composed, compose ms p50 /
p95 / max, the backend, and the passes and sub-samples per frame. The section is computed on
a warm cache, without the UI, through the same plan executor as the preview. The SPEED is
judged on a release build, as for audio.
Why: ktprompts v26.10.18 item 8. "fps drops" could not be split between the grid, the
morpher's strength slot or the GPU readback, because there was no number for
the video compose. `probe --time` measured only the audio render (R.43, item 301).

#### Scenario: The probe and the panel name the cost and the backend
- **WHEN** `kt-cli probe` is run with `--time` over a project with a morpher event grid 12×30 on a 4 s event; and the same project is played in the harness with the performance panel open
- **THEN** the probe prints the video section with the frame count of the range, p50/p95/max in ms, the backend, and per-frame passes and sub-samples equal to the counters of t946; the panel shows "Compose" with a millisecond value that changes while playing, next to the FX backend line, and stops repainting when playback stops
- **PATH** open the project with the morpher grid → open the Performance panel → it shows "Compose — ms" under "Preview 960×540" and the line "FX: GPU (<adapter>)" (or the line that names why there is no device) → press Play → the compose value updates while the grid grows from 1×1 to 12×30 → press Stop → the last values stay and the panel no longer repaints
- **TEST** t947 (`src/apps/kt-cli/tests/cli.rs`, `src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A window does not grow by itself
The width and height of a window SHALL change only by the person's resize gesture or by content
that became larger in its own right (a slot added, a group expanded, a longer name). Repeated
frames with the same content — pointer motion, hover, playback, idle repaints — MUST NOT change the
size of any window by more than one physical pixel in total, at any display scale, including
fractional ones and the program's own zoom (1.1, 1.2, 1.25, 1.3, 1.5). A width once reached by such growth is not a "size the person
chose" and SHALL NOT be remembered as one.
Why: kkt item 2 (2026-10-27), the owner: "on my other system the morpher window endlessly tries
to grow in width; other windows don't do this. Move the mouse anywhere in the main window or the
morpher window — it widens fast; don't move it — slowly." Growth that follows the frame count
is a ratchet: every frame may only add, never take back (egui 0.36 `Resize` keeps the maximum of
the requested and the last content size). The R.67 lesson "one scroll per window" removed the
window-level scroll from the FX and transition windows, but the morpher, camera, notices and
diagnostics windows kept it, because nothing held the rule for all windows. Found 2026-10-28: the
window-level scroll under the program's style grows the morpher window one physical pixel per frame
at zoom 1.1 (the owner's first Ctrl+=), 1.3, 1.35, 1.85 — and a frameless button grows two points
while hovered, which a content-sized window keeps for good.

#### Scenario: Every window keeps its size over frames at fractional scales
- **WHEN** each window of the registry that the harness can open (the morpher, transition and camera windows in their own scenes) is opened at display scale 1.0, 1.1, 1.2, 1.25 and 1.5, and 300 frames are run with the pointer moving over the main window and over the window itself
- **THEN** the window's outer rectangle at frame 300 differs from the one at frame 3 by at most one physical pixel in width and height; a window that fails is named with its scale and both widths
- **PATH** open the morpher window from the morph block's double-click → move the mouse over the timeline and over the window for five seconds → the window's right edge stays where it was
- **TEST** t918 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The owner's morpher scene does not widen
- **WHEN** a morpher layer in batch mode with a chain of twelve slots, two area references and an open curve graph is opened at the program's zoom 1.1, 1.2, 1.25, 1.3 and 1.5 (the program's own style applied); 600 frames are run with the pointer moving over the main window, then the same over the morpher window, then the same with no pointer motion and a frame every step
- **THEN** the width at the end equals the width at frame 3 within one physical pixel in all three runs; the window-level scroll is absent (one body scroll); the test fails on the code before the change in the same scene that the first investigation stage recorded as growing
- **PATH** put a morpher layer on a morph track → double-click it → the morpher window → add effects until the strip wraps → open a curve → move the mouse around the main window → the morpher window's width does not change
- **TEST** t923 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/winshell.py` (the real program, Ctrl+= once and twice — zoom 1.1 and 1.2, the owner's case)

### Requirement: An editing window refuses a kind of effect or an event only for a technical reason it names
The morpher window's effect picker SHALL offer every effect kind of the registry, video and audio
together, as its preset list already does; a picked audio effect SHALL land in the patch's audio chain
and act on the audio targets. An event carrying slowed reverb (a time warp, with or without a speed
curve, a shaped curve or a formula) SHALL be copied, pasted, cut to the clipboard and split at the cursor
by the same gestures and with the same result as any event: no refusal, the chain and the curves travel,
every piece's material is the integral of its speed. Where a window does refuse something, it SHALL say
the technical reason in the status line, and that reason SHALL hold in the current engine.
Why: ktprompts v26.10.18 items 1 and 3 — "the morpher does not let me put audio effects on", "slowed does
not let me copy-paste the event and cut it — childish excuses". The morpher's single strip (R.35 S2) passed
"video" to the picker by oversight; the "cannot cut a warped clip" answer outlived its reason (R.39).

#### Scenario: The morpher picker offers audio effects
- **WHEN** the harness app has a morpher over an audio and a video track; its window is opened; "+ effect…" is clicked; "pitch" is typed in the search; Pitch shift is picked; the window is closed and the project rendered
- **THEN** the picker lists the audio kinds (Reverb, Pitch shift, EQ, Compressor, …) next to the video kinds; Pitch shift appears in the morpher's single strip after the morph slot; the model holds it in the patch's audio chain; the audio target sounds shifted inside the block and unchanged outside it
- **PATH** right-click the morph track → "Add morpher" → the purple block appears → double-click it → the morpher window opens with one chain strip starting with the purple morph slot → click "+ effect…" → the gallery lists video effects and, under their own heading, audio effects → type "pitch" → Pitch shift remains → click it → the strip shows "Pitch shift" after the morph slot → close the window → press Space → inside the block the sound is higher
- **TEST** t912 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A slowed event is copied, pasted, cut and split like any event
- **WHEN** the harness app has an audio event with slowed reverb at 0.8× and a speed curve 0.5→1.5 (linked to its video event); the event is selected and Ctrl+C, then the cursor moved and Ctrl+V; S is pressed with the cursor in the middle of the original; one half is selected, copied (Ctrl+C), deleted (Delete) and pasted elsewhere (Ctrl+V) — the program has no Ctrl+X; a region over half the event is copied and pasted
- **THEN** no gesture is refused and no error note appears (the split was refused before this change: the right half's speed keys moved with the anchor to a negative tick, and validation rejected the whole batch — `BadFxAuto`); the pasted copy has the same chain, bypass and curves, and sounds byte-for-byte like the original over its length; the split pieces' entry points are the integral of the speed and their joint has no material gap; the link partner is split and pasted with it; the region copy holds the piece by the same integral
- **PATH** click the slowed event → it is outlined → Ctrl+C → the status line says "1 event copied" → click the ruler further right → Ctrl+V → a copy with the same "Slowed reverb" badge appears at the cursor with its video partner → click the middle of the original on the ruler → press S → the event and its partner split into two → click the right half → Ctrl+C → Delete → it disappears → click the ruler elsewhere → Ctrl+V → it reappears there, the filmstrip continuing where the left half ended
- **TEST** t917 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A name the program chose follows the interface language
A reel that the program named by itself — the default reel name of the collapse
gesture, with or without the number `unique_name` adds — SHALL be SHOWN in the current
interface language wherever a reel name is shown (the head of an instance's chain strip, the path
crumbs, the media bin card, the reel's settings window title, search hits, the rename dialog's
prefill). A name is "chosen by the program" when it equals the default reel name of ANY of the
five catalogs, optionally followed by a space and a number; any other name is the person's text
and is shown exactly as stored. The stored name in the project file SHALL NOT change by being
shown; renaming stores what the person typed. Lookups by name (search, the rename uniqueness check)
SHALL compare both the stored and the shown form.
Why: ktprompts 26.10.18 item 10. The default name is written into the file silently at collapse
(`tr(msg.nest_default_name)`, no question asked), so a reel created in the Russian interface
carried «Лента» into every other language — a word the person never typed. Existing files are
covered too, because the rule reads the name, not a flag.

#### Scenario: A default reel name is shown in the current language
- **WHEN** a selection is collapsed twice in the Russian interface (names «Лента», «Лента 2»), a third reel is renamed «Интро», the project is saved and reopened in English; then switched to Chinese
- **THEN** in English the reels are shown as "Reel", "Reel 2" and «Интро»; in Chinese as the zh-Hans default name, its "2", and «Интро»; the project file still holds «Лента», «Лента 2», «Интро» byte for byte; renaming "Reel 2" to "Outro" stores "Outro"
- **PATH** in the Russian interface select two events → Collapse into a reel → the bin card «Лента» → View → Language → English → the bin card reads "Reel", the instance on the timeline reads "Reel" → right-click it → Rename the reel… → the field shows "Reel" → type "Outro" → Enter → the card reads "Outro" in every language
- **TEST** t932 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Every place that shows a reel name uses the shown form
- **WHEN** a reel stored as «Лента» has an instance with one published parameter; in English the instance's FX window is opened, the reel is entered (crumbs), its settings window is opened, the bin is shown and Ctrl+F searches "Reel"
- **THEN** the head of the instance's chain strip reads "📌 Reel" (the owner's case), the crumbs, the settings window title, the bin card and the search hit all read "Reel"; the search finds the reel by "Reel" and by «Лента»
- **PATH** double-click the reel instance → the FX window → the head of the strip "📌 Reel", the published parameter below it → Enter the reel → crumbs "Root › Reel" → Ctrl+F, type "Reel" → the reel is found
- **TEST** t934 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Event gesture zones follow the event's size on screen
On an event the trims SHALL be tested before the level line, so a press in the outer columns of an
event (its bottom corners included) always starts a length drag. The level line's grab SHALL scale with
the row's height on screen. On a row too low or an event too narrow for three separate zones, there
SHALL be no level gesture at all: only move, trim and the corner fades. The thresholds are set in
design.md and named next to their constants. Moving an event by its body SHALL never change its gain or
opacity. The hover cursor and hint SHALL name the zone that a press would take.
Why: the owner, `ktprompts_v26.10` USABILITY 5.1–5.3: "so many times I wanted to quickly move events,
and the volume change keeps triggering". The level line was tested first and at a fixed 5 px. On a
16–30 px row that covered a third of the row, including the corners.

#### Scenario: The bottom corner is a trim, a low row has no level gesture
- **WHEN** an audio event with gain −6 dB on an 84 px row is pressed 2 px inside its right bottom corner and dragged; separately — the same event on a 24 px row is pressed on its level line in the middle and dragged sideways; separately — on a 300 px wide event at 84 px the level line is dragged
- **THEN** the first changes the length, not the gain; the second moves the event and its gain is unchanged; the third changes the gain
- **PATH** zoom the rows out until they are low → drag an event by its middle → it moves, the volume stays; zoom back in → drag the bottom right corner → the event gets longer; drag the level line in the middle → the volume changes
- **TEST** t1628 (`src/apps/kt-app/src/layout_tests.rs`), t1629 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: An event's gain goes down to silence, and a non-default level is written on the event
The event's gain SHALL span from +6.02 dB down to silence. The level line's bottom and the menu's
number field's floor SHALL give silence, and the field SHALL read `mute` there. The upper half of the line keeps today's mapping, so 0 dB
stays at the middle. A gain or opacity other than the default SHALL be written as a small chip after
the source name in the event's label row (for example `−12 dB`, `40 %`, `mute`). It SHALL use no new
strip (rule 13), be truncated with the name by the shared rule, and be absent at the default.
Why: the owner, `ktprompts_v26.10` USABILITY 6: "let the gain go down to mute, not only to −6 … show
somewhere on the event what gain/opacity is set". The track's fader reached −60 dB, the event's line
stopped at −6.02 dB, and the line's height was the only indicator.

#### Scenario: Down to mute, and the chip says so
- **WHEN** an audio event's level line is dragged to the bottom; separately — a video event's opacity is set to 40 % in the menu; separately — both are reset
- **THEN** the event renders silence (see `audio-render`), its label row shows `mute`; the video event shows `40 %`; after reset there is no chip
- **PATH** drag an audio event's level line all the way down → the label reads "name · mute", play → silence → double-click the line → back to 0 dB, the chip is gone
- **TEST** t1630 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A range marker's end is grabbed where it is drawn and does not snap to itself
The end of a range marker SHALL be grabbed on the ruler at the nearest end within the grab radius. On
the ruler it wins over a coincident region edge. Below the ruler a coincident region edge keeps
priority, and the hover hint SHALL say which edge a press takes. A drag of the end SHALL NOT snap to
the marker's own start or its own old end.
Why: the owner, `ktprompts_v26.10` USABILITY 7 and question 9 of R.71: "something is wrong with the
second marker … the end of the range marker". A marker made over the region lies exactly on the
region's end. Its end snapped back to that position and could be taken only on the ruler, without a
hover cue.

#### Scenario: The end of a marker made over the region moves
- **WHEN** a region 2–6 s, "Marker over the region" makes a 2–6 s range; its end is dragged on the ruler from 6 s to 7 s in three steps; separately — hovered at the end
- **THEN** the range is 2–7 s (no snap back to 6 s), the region is unchanged; the hover shows the resize cursor and the hint of the marker's end
- **PATH** select a region → right-click the ruler → "Marker over the region" → hover the right flag of the marker → the hint says "marker end" → drag it right → the marker grows, the region stays
- **TEST** t1632 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Vertical zoom keeps the row under the mouse
A vertical zoom gesture SHALL keep the same row, at the same fraction of its height, under the
pointer. This SHALL hold on rows of different heights (service rows, per-row overrides). The scroll
range SHALL be computed from the real row heights.
Why: the owner, `ktprompts_v26.10` TIMELINE 1: "when you scale vertically the vertical position flies
down to the lowest track". The anchor assumed equal row heights and the clamp pinned the view to the
bottom.

#### Scenario: The row under the pointer stays under it
- **WHEN** a timeline with two morph rows, a camera row and four clip rows (one with a height override), the pointer over the third clip row, vertical zoom in and out by five steps each
- **THEN** after every step the row under the pointer is the same row, its fraction within ±1 px
- **PATH** point at the third track → Ctrl+Shift+wheel up → the tracks grow, the same track stays under the mouse → wheel down → they shrink around it
- **TEST** t1633 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The scrollbar and both jump zones can be taken
The timeline's vertical scrollbar SHALL be a no-resize rect (the outer 4 px of the window stay the
window's). Its grab SHALL be wider than its drawing, and a click on its track SHALL page. The right jump
zone SHALL end before the scrollbar. The left jump zone SHALL keep its full width on every row, badges
included, and its hot end under the track header SHALL answer: a click on the header's free part is the
hottest left jump, a badge stays a button, and a double click there stays the rename, giving the view
and the cursor back. In a jump zone a click SHALL jump even over an event, a morpher, a camera or a
transition, and SHALL NOT take the selection. A quick second click there SHALL be a second jump, not a
double click that opens the event. With the project's start on screen the left zone has nowhere to jump,
so an event there SHALL take its clicks as usual. Dragging an event that starts in the zone SHALL still
move it, and Ctrl or Shift on a click keep their meaning there.
Why: the owner, `ktprompts_v26.10` TIMELINE 2–4: the scrollbar "is very hard to find, either the window
resize or the jump zone triggers"; "the added track buttons broke half of the jump zone on the left";
"if an event is in the jump zone, it opens for some reason". This is the theme of r.12/r.13 ("resize
steals clicks", t101) returning on a new element. The zone's gradient has run through the gutter to the
panel edge since item 20 of round 6, while clicks there did nothing; R.62 grew the gutter from 34 to 62 px.

#### Scenario: The scrollbar at the window edge, the jump over an event
- **WHEN** the timeline is flush with the window's right edge; the scrollbar is pressed and dragged; separately — the left jump zone is clicked on a row where badges are, then the header's free part, then a badge; separately — an event lying in the right jump zone is clicked twice within 0.2 s
- **THEN** the scrollbar scrolls and no `BeginResize` is sent; the left click jumps the view, the header jumps it further, the badge toggles and does not jump; the two clicks give two jumps and no FX window
- **PATH** maximise the window → grab the timeline's scrollbar at the right edge → the tracks scroll, the window does not resize → click near the left edge on a track with buttons → the view jumps left → click twice near the right edge on an event → the view jumps twice, no window opens
- **TEST** t1634 (`src/apps/kt-app/src/layout_tests.rs`), t1635 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Empty rows carry no captions
An empty service row (morpher, camera) and an empty clip row SHALL draw no text inside the row. The
action of adding a morpher or a camera SHALL stay in the row's menu, and its description in the
status-line hint on hover. A test SHALL fail when any catalog text is painted inside an empty row.
Why: the owner, `ktprompts_v26.10` TIMELINE 4: "meaningless obvious captions over the tracks 'Add
morpher here' and the camera one too, I ask to forbid agents to make dumb captions". The prose rule
(item 17 of R.7, checklist rule 9) was broken again, so the rule becomes a test.

#### Scenario: An empty timeline paints no captions in rows
- **WHEN** a project with one empty morph row, one empty camera row and one empty video row is drawn
- **THEN** no painted text lies inside the rows' rectangles; hovering the morph row shows its hint in the status line; its menu has "Add morpher here"
- **PATH** add a morph track → the empty row is plain, no caption → hover it → the status line explains it → right-click → "Add morpher here"
- **TEST** t1636 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: At rest the work counters stand still
On a pause with nothing moving, on a project that carries heavy effects, the work counters
(`kt_time::work`) SHALL stand still once what the opening started is over: no UI frame, composite,
device submit, decode order, prefetch pass, thumbnail or composition preview. The verdict is the
counters: a machine with no card rasterizes in software, and every frame looks expensive there.
Why: the owner, 2026-11-07 (E1): on a pause, with nothing happening, the program held the whole
processor. Every green test ran an empty or a light project.

#### Scenario: A pause on the heavy project
- **WHEN** the heavy project (`kt-cli showcase --heavy`) has gone quiet after opening, and nothing is touched
- **THEN** every counter stands still
- **PATH** open the project and touch nothing → the picture stands, and nothing runs behind it
- **TEST** t1830, t1833 (`src/apps/kt-app/src/idle_tests.rs`)

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
- **PATH** open a long edit with hundreds of files in the bin → move the mouse along a track, then over the bin → Diagnostics → Performance reads the same "UI frame" at the start of the edit, at its end and zoomed out
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

### Requirement: The Diagnostics window shows where the memory went
Under the cache occupancy bar the Diagnostics window SHALL show one line of the memory ledger: the
kept composites, the stream history, the decoded frames and the device buffers in megabytes, and the
room the machine gives (the bases and the growth budget). It SHALL read the ledger's counters only and
SHALL ask no repaint of its own.
Why: found-defects 310 and F-RES-011 — the occupancy bar showed the decoded frames alone, so a loop
that was not kept whole gave no sign of which memory had run out.

#### Scenario: The ledger line is painted
- **WHEN** the Diagnostics window is open while the present memo and the burst cache hold bytes
- **THEN** a line names the kept composites, the history, the frames and the device buffers with their megabytes and the room, under the occupancy bar
- **PATH** open View → Diagnostics… → under the cache occupancy bar a "Memory" line reads kept, history, frames, device and the room in MB; play a loop → the kept number grows lap by lap
- **TEST** t2301 (`src/apps/kt-app/src/diag_tests.rs`)

### Requirement: Keys and clicks a slow frame reads keep their meaning
Every command key a frame reads SHALL run, once per press, in the order pressed; a command that opens a
window or writes the file runs once a frame, and one that opens a text field ends the frame's keys. Esc
and Enter run once per press. egui's double-click and click windows SHALL widen by the busy frame the
program measures (`KeyClock::busy`, the smoothed gap before a frame the previous one asked for at once);
the harness keeps egui's windows, its clock being a fixed step.
Why: under the full live run a slow frame read → → → ← together and the head did not move (a set of the
frame's commands: +1 −1), two Ctrl+Z undid one edit, and a double click whose two clicks were read a
600 ms frame apart was two single clicks (`formula`, `lessons`, `forms`; found-defects 99, 166). egui
stamps an input with the frame that read it: a gesture's length is known to a busy frame.

#### Scenario: Several keys in one frame
- **WHEN** one frame reads → → → ← with the head on a frame boundary; then one frame reads Ctrl+Z twice
  after two edits
- **THEN** the head is two frames on; both edits are undone
- **PATH** the cursor stands on a frame → the person taps → three times and ← once quickly → the cursor
  is two frames on, whatever the machine's frame time; two quick Ctrl+Z → both edits are gone
- **TEST** t2211 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The click windows widen by the busy frame
- **WHEN** the program has measured frames of 450 ms
- **THEN** egui's double-click window is 0.3 s + 0.45 s and its click window 0.8 s + 0.45 s; in the
  harness both stay egui's
- **PATH** on a loaded machine the person double-clicks an event → its effects window opens, as on a
  free machine
- **TEST** t2214 (`src/apps/kt-app/src/layout_tests.rs`); live `formula`, `lessons`, `forms`

### Requirement: The memory numbers say what they count and name the whole process
The status bar SHALL show the decoded frames held against the bound they may grow to now, and the
program's resident memory, with a hint saying what each is; it SHALL NOT show a pair a person reads as
used/limit when it is two different caches. The Diagnostics window SHALL show the decoded frames'
occupancy of their bound now, the caches' line (kept composites, history, frames, the kept mix, device
buffers, the room) on one line whatever its numbers, and the program's memory with the rest no cache
counts, the rest named by its hint as the decoders, the graphics driver and the interface. The process's
memory SHALL be read at most once a second, in frames drawn anyway, and no figure appearing or growing
a digit SHALL change the window's height.
Why: found-defects 397 — "cache: 39/535 MB" was audio/video and read as used/limit, Diagnostics said
"100 % (535/537)" of a bound that never moved, and 4.7 GB of the program were named nowhere
(kkt-26.11.10 J4).

#### Scenario: The status bar says frames against their bound and the program
- **WHEN** the editor is open with a project
- **THEN** the status bar shows "frames: V/cap MB" with the cache's bytes and its bound now, and "memory: P MB" with the process's resident memory
- **PATH** look at the status bar → "frames: 0/512 MB · memory: 600 MB"; hover either → the hint says the frames are decoded frames against the bound that grows with the loop and quality, and the memory is all the program holds; play a loop → the bound grows past 512
- **TEST** t2397 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: Diagnostics names the whole process
- **WHEN** the Diagnostics window is open
- **THEN** the first performance row gives the program's memory and the rest, the rest a part of it; hovering it, the status line names the rest as the decoders, the graphics driver and the interface; the caches' line stands under the rows
- **PATH** open View → Diagnostics… → the first row reads "Program 7254 MB · rest 897"; hover it → the status line: "Program: all the memory it holds. Rest: what no cache counts (decoders, graphics driver, interface)…"; under the occupancy bar "Caches: kept … · mix … · room … MB"
- **TEST** t2398 (`src/apps/kt-app/src/diag_tests.rs`)

### Requirement: V-fps counts new pictures of the moment asked
The performance panel's V-fps SHALL count the shown frames that are a new picture of the moment the
preview asked for; a frame that shows a stream stage's held picture SHALL NOT count, and SHALL be
counted apart as "held" frames a second, shown next to V-fps in the Diagnostics window while non-zero.
The trace's `frame_show` record SHALL carry the held flag.
Why: found-defects 393 — his trace: 114 shows in 6 s (19/s), 111 of them a held stream frame, 45
distinct pictures, all from the history before his loop; the counter said 20 where "at most 5 are new".

#### Scenario: Every frame held is V-fps 0
- **WHEN** the app takes a second of composites of advancing moments that all show a held stream frame, then a second of complete new moments
- **THEN** the first second's V-fps is 0 and its held count is the composites taken; the second's V-fps counts them and held is 0
- **PATH** play over a Datamosh event whose history is behind → the toolbar reads 0 vfps, Diagnostics shows the held frames a second and the preview's marker; when the history catches up → the vfps rise and the marker goes
- **TEST** t2358 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The preview marks a held stream frame
While the frame on screen is a held stream frame, the preview SHALL draw a small "catching up" marker in
its corner, without dimming the picture; the Loading scrim keeps its own rule (an incomplete frame
past 180 ms). The marker SHALL not ask for frames at rest.
Why: kkt-26.11.10 J2 — a held frame looked like playback; the Loading scrim was decided by the owner for
a frame with no picture (the flicker "black frame ↔ Loading", items 14–15 of round 14) and stays for that.

#### Scenario: The marker follows the held flag
- **WHEN** the app takes a held composite, then a complete one, then sits at rest
- **THEN** the marker is drawn over the first and not over the second, the scrim over neither, and at rest no frame is asked for
- **PATH** play over a Datamosh event whose history is behind → a small "catching up" note in the preview's corner over the moving picture; it goes when the picture is the moment's own
- **TEST** t2359 (`src/apps/kt-app/src/layout_tests.rs`)

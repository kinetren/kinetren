# Spec Delta

## MODIFIED Requirements

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

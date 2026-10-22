# Spec Delta

## MODIFIED Requirements

### Requirement: Windows only from the registry; the tool remembers its parameters; name by industry; one widget
A new window (`egui::Window`, `Modal`) SHALL be created only together with an entry in the window registry (`src/apps/kt-app/src/reach_tests.rs`, `WINDOWS`): its id, what it contains, and WHY the feature cannot work without a window; the guard SHALL enumerate all places in the kt-app sources where windows are created and SHALL fail on any place outside the registry. Things that already have a place (the chain strip, the track window, the panel, the status line) MUST NOT get their own window. A tool with parameters (render, analysis) SHALL remember the last entered parameters in the user storage and substitute them on the next call; a tool whose result is an envelope SHALL return it as ordinary automation curves. The name of the menu item, the tool and the window SHALL be the generally accepted industry name (Vegas/Premiere/Resolve/FCP), not a description of the mechanics.
A device that already exists in the program SHALL show the new function by itself, not by a copy of its own fields: ducking is a COMPRESSOR WITH A KEY INPUT in the track's chain, shown by its own widget next to the compressor of the key track. The parameters of such a tool live in the PROJECT (they are the content — the track's chain) and go through undo; a workspace store is not created for them — "remembering what was entered" is here ensured by the project itself.
Settings of a live device SHALL be reachable from where the person already stands: the window of the audio track and its chain strip; a refusal (a cycle, a key on itself, a vanished track) SHALL be named in words, not fade out silently.
THE FORM OF A WINDOW SHALL obey one contract common to all windows of the registry: the window body scrolls, and the action buttons ("Close", "Cancel", "Apply", "Render", "Restore") live OUTSIDE the scroll and remain pressable at any screen height — a window that outgrows the screen MUST NOT hide its actions. A numeric field SHALL be drawn by the COMMON row of widgets; there are no windows with their own `DragValue` outside the common widget. The limits, step and unit SHALL come from the parameter's passport (`ParamSpec`), and for a quantity without a passport they SHALL be declared in ONE place next to its definition and passed to the row as arguments; writing them into the drawing code MUST NOT happen.
The six densest forms (the effect chain, morpher, transition, camera, project settings, render) SHALL be laid out on the model of the "Transform" section: collapsible groups, pairs on one row, a unit for each number, a slider for a quantity with both limits, a reset with one action. The other windows of the registry do NOT fall under the re-layout, but they are subject to the form contract.
Why: R.63, the owner's words: "I impose a ban on any agents generating any additional windows without an obvious purpose (like the window on closing the program)"; "why the agent… forces him to enter all the parameters again"; "duck under a track… why didn't you name it normally"; "everywhere, meaningless copy-pastes of the same code pile up". R.65: the ducking snapshot was removed together with its parameter memory — a live device has its memory by construction, and a second one (in the storage) would diverge from the project. R.66 (the owner's direct request of R.64 "in all the other widgets and windows it will also have to be gone through"): prose about the look of a form did not work — body scrolling at that point existed for exactly one of the twenty-one windows, and it was added only after a test dropped a window with buttons hidden behind the edge.

#### Scenario: The window registry
- **WHEN** the kt-app sources are scanned for `egui::Window::new` and `Modal::new`; a window without an entry is temporarily added; the entry of a live window is removed from the registry
- **THEN** every place where a window is created is found in the registry with a reason at least 25 characters long; both violations are red; the `duck` and `inst_params` windows are absent from the registry and do not exist in the code
- **TEST** t757 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A tool remembers its parameters and delivers curves
- **WHEN** the workspace (the meter width, the project view) goes through a round trip via the storage file, and a file without the section is read again; separately — the track's sidechain compressor: the key and the parameters are edited in the track window
- **THEN** the workspace settings are restored, a file without the section is still read; the sidechain parameters live in the PROJECT (the track's chain), go through undo and are edited by the common parameter widget — there is no second copy of them in the user storage
- **TEST** t756b (`src/apps/kt-app/src/user_store.rs`), t775 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Names — by industry
- **WHEN** the catalogs are scanned for words describing the mechanics instead of a name ("under a track", "Russian: "under the track"", "snapshot of" in item names)
- **THEN** there are no menu item names or windows with such phrasing; the items whose names were introduced by R.63 have all five translations
- **TEST** t756c (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: The sidechain is reachable and explains a refusal
- **WHEN** the track window and its chain strip are scanned for the path "assign key"; an attempt is made to choose the track itself and a track that would produce a cycle; a live scenario sets ducking by hand and listens to the result
- **THEN** the path is found from both places and leads to one command; an impossible choice is named with its reason in the status line, rather than missing silently; the live scenario shows that the music dipped under the speech and came back after it
- **TEST** t777 (`src/apps/kt-app/src/reach_tests.rs`), `src/scripts/live/sidechain.py` (live scenario, `./scripts/live-check.sh`)

#### Scenario: The form contract — across all windows of the registry
- **WHEN** each window of the `WINDOWS` registry is opened in a LOW window; separately the kt-app sources are scanned for `egui::DragValue::new` outside `widgets.rs`
- **THEN** in each window the action buttons are visible within the screen, and the body lives in a scroll; a window that closes by the title's cross, and a window that is short BY CONSTRUCTION, are declared explicitly — with a reason and a measurable row threshold; not a single own `DragValue` outside the common widget is found
- **TEST** t779 (`src/apps/kt-app/src/reach_tests.rs`), `src/scripts/live/forms.py` (live scenario: the buttons are visible and pressable in a real low window)

#### Scenario: Six dense forms are laid out on the model
- **WHEN** the windows of the effect chain, morpher, transition, camera, project settings and render are opened
- **THEN** each has collapsible groups, pairs of values on one row, a unit for each number, and a slider for a quantity with both limits; the grouping is the state of the window, not the project; the limits match the parameter's passport
- **TEST** t780 (`src/apps/kt-app/src/layout_tests.rs`), `src/scripts/live/forms.py` (live scenario: a group collapses and expands)

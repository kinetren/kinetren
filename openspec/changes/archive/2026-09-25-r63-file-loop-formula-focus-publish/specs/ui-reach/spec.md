# Spec Delta

## ADDED Requirements

### Requirement: Windows only from the registry; the tool remembers its parameters; name by industry; one widget
A new window (`egui::Window`, `Modal`) SHALL be created only with an entry in the window registry
(`src/apps/kt-app/src/reach_tests.rs`, `WINDOWS`): the id, what is in it, and WHY it cannot be done without a window; the guard SHALL enumerate all places where windows are created in the kt-app sources and fail at a place outside the registry. Things that already have a place (the chain strip, the track window, the panel, the status line) MUST NOT get their own window. A tool with parameters (ducking, render, analysis) SHALL remember the last entered parameters in the user store and substitute them at the next call; a tool whose result is an envelope SHALL hand it back as ordinary automation curves. The name of the item, the tool and the window SHALL be the generally accepted industry name (Vegas/Premiere/Resolve/FCP), not a description of the mechanics.
An instrument that already exists in the program SHALL show the new function by itself, rather than being a copy of its fields (ducking — by a compressor with a key input, not a set of four numbers).
Why: R.63, the owner's words: "I am imposing a ban on any agents creating any additional windows without an obvious purpose (like the window that appears when closing the program)"; "why does the agent… force him to enter all the parameters again"; "duck under a track… why didn't you name it properly"; "meaningless copy-pastes of the same code pile up everywhere". The rules of the ui-timeline checklist were written down and were still violated — so a guard is needed, not prose (precedent t252/t253).

#### Scenario: The window registry
- **WHEN** the kt-app sources are scanned for `egui::Window::new` and `Modal::new`; a window without an entry is temporarily added; an entry of a live window is removed from the registry
- **THEN** each window creation place is found in the registry with a reason at least 25 characters long; both violations are red; the windows `duck` and `inst_params` are absent from the registry, and there is no code for them
- **TEST** t757 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A tool remembers its parameters and delivers curves
- **WHEN** ducking is invoked from the audio track window with changed parameters, then the window is opened again; the parameters have gone round through the user store file
- **THEN** the window shows the parameters of the last call (not the defaults); the store holds the same ones; a file without the section is still read as before; the targets are the events of THIS track, and the result lies as ordinary level curves, editable with the common automation widget
- **TEST** t756 (`src/apps/kt-app/src/layout_tests.rs`), t756b (`src/apps/kt-app/src/user_store.rs`), t420 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Names — by industry
- **WHEN** the catalogues are scanned for words describing the mechanics instead of the name ("under a track", "under a track", "snapshot of" in item names)
- **THEN** there are no menu item names or windows with such phrasings; the items whose names were introduced by R.63 have all five translations
- **TEST** t756c (`src/apps/kt-app/src/i18n_tests.rs`)

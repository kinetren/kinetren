# Proposal

## Why

Round R.63 (2026-09-24, version 26.09.23-d5da7cf): the owner's working project stopped opening; the tail of a slowed event reads past the end of the file; after a jump into a long file the sound is silent for seconds; the formula field erases input and shows nothing; the input field keeps the hotkeys taken forever; the segment's shape menu disappears under the pointer; the lead "Geometry" is not published; a published effect parameter does not take effect; publishing from the strip forcibly enters the reel; composition parameters live in an extra window; ducking is named and built in a way that you cannot recognise it. Plus four owner rules for all agents: name things by their names, windows only where you cannot do without them, the tool remembers parameters and gives curves, one widget — do not copy it. The claims verbatim and the reconnaissance — `docs/user-feedback.md`, R.63.

## What Changes

- **The file opens again (item 1).** The reader converts the speed curves (`rate`) with the old field `ease: true` into the LINEAR form — that is how they always sounded (the warp reads a piecewise-linear profile and never saw `ease`); the rejection by `RateShapeMustBeLinear` when reading an old file is a regression of the migration, not a protection. The file `untitled_v26.09.19.otio` and its recovery must open and sound byte-for-byte the same as with the version before R.61.
- **The tail of an event past the end of the material (item 2).** An event that consumes more material than there is in the file (Rolling Stones: 237.7 s with a 226.3 s file) plays silence/a freeze frame WITHOUT underruns; the tail without material is visible on the event (hatching) and is named in `kt-cli probe`; the right-edge trim (taking the speed into account) stops at the end of the material of known length — existing events are not shortened by themselves (rule 20). The first task is to establish where the extra 11 s came from (a trim past the material, the stretch, or the duration from FFmpeg).
- **A jump into a long file does not starve the sound (item 13).** Audio requests for playback are served before the video prefetch of the same worker; after a seek into the 870 s file the underruns after pre-roll are zero. It is reproduced by a live scenario on `kt-untitled-review-870s.mp4` and by an engine test with a slow synthetic video.
- **The formula is edited as text (item 3).** The field holds the text being typed between frames and applies it on confirmation (Enter/loss of focus), or when the text is legal and is NOT a prefix of a longer input; the keys are not zeroed by the first legal letter; the graph draws the resolved formula curve (today it draws empty `keys`).
- **A click outside the field gives up focus (item 4).** One rule for the whole program: a pointer press outside the focused input field removes the focus; Space and the other hotkeys work immediately after a click on the timeline, the preview, or the toolbar — with any window open.
- **The shape menu does not run away and says which segment it is about (item 5).** The segment/keyframe under the right click is fixed at the moment of the click, while the menu is open; the segment is highlighted on the graph, and the menu header holds the times of its keyframes; the keyframe menu — likewise.
- **The lead "Geometry" is published (item 9).** A new address `ParamOwner::Geometry` — the same captions, the same door as for the second node; the engine applies the geometry override.
- **A published effect parameter takes effect on the instance (item 10).** The defect is reproduced by a test on `RadialBlur.amount` (model → engine → preview) and is fixed where it is found.
- **Publication does not enter the reel (item 11).** `PublishParam` carries the reel address explicitly; the confirmation from the strip stays in the strip.
- **Composition parameters — in the strip of the instance's chain (item 12).** A pinned head of the strip (as for a generator, R.59) instead of the separate `InstParamsWin` window; the author's "Public parameters" window remains (this is the author inside the reel); the tests of the consumer window move to the head without weakening.
- **Ducking is named and remembers its parameters (item 8).** "Ducking (background attenuation under speech)" in all five catalogues; the parameters live in `user_store` and are substituted next time; the call is made from the track window (its chain), there is no separate window. A live sidechain compressor is the owner's decision — a separate change `sidechain-ducking-compressor` to follow.
- **The owner's rules for agents — into the code and into the guards.** A registry of permitted windows (scanner: `egui::Window::new` only from the list, with a reason); the rules "name it in industry terms", "the tool remembers parameters", "one widget, not a copy" — in `apps/kt-app/CLAUDE.md` and the checklist.
- **Not in this change:** the geometry frame in the preview and the proper form of the fields (item 6) — a separate change `geometry-on-screen-controls` right after (the owner's decision: "in addition to the fields, and in a proper form"); the live sidechain (item 8) — `sidechain-ducking-compressor`; the track controls (item 7) — already done by change B; they were not in version d5da7cf.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `project-format`: "Old files migrate on read, one-way" — speed curves with `ease: true` are read in the linear form; a file rejected by its speed shape is a regression.
- `transport`: "Video under signed speed — … the edge of the material …" — the edge of the material for BOTH paths: past the end of the file a freeze frame/silence without underruns; the tail without material is named in probe; the trim runs up against the end of the material of known length.
- `media-index`: "Live playback goes through the priority queue of MediaWorker" — playback audio does not starve behind the video prefetch of the same worker after a jump.
- `ui-timeline`: "Effect windows and input fields" (a click outside the field gives up focus); "Shapes, the derived graph and the formula in the automation widget" (the formula text is held between frames, the formula curve is drawn, the segment/keyframe menu is fixed and labelled); "Compositions in the interface" (instance parameters — the head of the strip); "The lead node in the chain strip" (the lead geometry is published).
- `compositions`: "Public parameters — the template value is not stored…" (the `Geometry` address, the publication command with an explicit reel, the effect slot override takes effect); "The publication interface — one door…" (no entry from the strip; the consumer edits in the head of the strip).
- `audio-render`: "Ducking — an envelope snapshot, not a live sidechain" (name, parameter memory, no window; the live sidechain — the next change).
- `ui-reach`: ADDED "Windows only from the registry; the tool remembers parameters; the name — by industry" — the guards of the owner's rules.

## Impact

- `src/libs/kt-otio` (reading `ease` of `rate`), `src/libs/kt-model` (`ParamOwner::Geometry`, `PublishParam { seq }`, the trim clamp to the material — requires knowing the length of the media in the model: `MediaRef` gets an optional duration from whoever opened the source), `src/libs/kt-engine` (the edge of the material without underrun in `fill_source`/the fetcher, audio Playback priority in `worker.rs`, applying the geometry override, probe), `src/apps/kt-app` (the formula field and graph in `automation.rs`, the common focus reset in `app.rs`, the segment menu, the head of the strip "Composition parameters" in `fx_editor.rs`, removal of `InstParamsWin`, ducking in `track_window.rs` + `user_store`, the window registry in `reach_tests.rs`, CLAUDE.md), `src/apps/kt-cli` (probe: "past the end of material"), five i18n catalogues.
- Tests t747–t758 (`layout_tests.rs`, `reach_tests.rs`, kt-otio/kt-model/kt-engine), live scenarios `formula.py`, `oldfile.py`, `seekjump.py`, extension of `nestparam.py`.
- Files: the reading of old speed curves changes, writing does not; the envelope version does not move. Tests t683/t684/t685/t691 (the consumer window) move to the head of the strip — the owner's decision, not a weakening.

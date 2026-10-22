# Tasks

## 1. Key input on the device (kt-fx, without the engine and without the model)

- [x] 1.1 `AudioNode::process` accepts an optional KEY slice (as a third argument, next to the position); all other nodes ignore it verbatim with the previous code. Check: `cargo test -p kt-fx` is green without regeneration, the t322 skeleton passes over `FxInstance::all()`.
- [x] 1.2 `CompNode` computes the detector from the key when it is present, and from its own input when it is absent; the compression law, limits and smoothing do not change. Test t767 (`src/libs/kt-fx/tests/sidechain.rs`): a steady tone + a speech sample as the key — a dip exactly within the speech intervals; the same run without the key is byte for byte equal to the previous code.
- [x] 1.3 Neutrality and cost: test t768 (`src/libs/kt-fx/tests/sidechain.rs`) — silence in the key leaves the input untouched (up to makeup), there are no allocations in processing; `GOLDEN_AUDIO`, t2, t3 are green without regeneration.

## 2. The key in the model: address, validation, refusal instead of silence

- [x] 2.1 `FxInstance::Compressor` gets a key field (`Option<TrackId>`, serde default "none"); the parameter passport and labels do not change. Check: `cargo test -p kt-model` is green, t48 (the registry of kinds) does not turn red — no new kind of effect was added.
- [x] 2.2 Model validation: a key is allowed ONLY in the chain of an audio track (not on an event, not on video), points to an existing audio track of the same reel, and does not create a cycle of any length. Test t772 (`src/libs/kt-model/tests/model.rs`): each of the four attempts is a named error and an untouched model; red on the previous model (it would have accepted everything).
- [x] 2.3 Deleting the key track does NOT edit other people's devices (rule 20): the key remains written, and is read as "no key". Test t772 (continuation): after deletion the command did not touch the neighbours; undo restores the track and the key is live again.

## 3. Live sidechain in the engine

- [x] 3.1 The order of stages is topological by key, stable (with equal ranks — the previous row rank); the assembly of stages refuses a cycle (a safeguard for 2.2). Test t770 (`src/libs/kt-engine/tests/sidechain.rs`): a key BELOW the target by rank is computed first; a project without keys keeps the previous summation order.
- [x] 3.2 The key signal: its own render branch of the key track (events + its chain + level + muting) on the WINDOW OF THE TARGET, with its own state in `fxstore`; inside the branch a compressor with a key works as an ordinary one (the recursion is cut). Test t769 (`src/libs/kt-engine/tests/sidechain.rs`): the music dips exactly under the speech and returns between the lines.
- [x] 3.3 Alignment and determinism with lookahead on the target: test t770 (continuation) — with a lookahead node in the target's chain, the ducking coincides in time with the speech; two runs with different chunk boundaries are byte for byte equal.
- [x] 3.4 Dead and muted key: test t771 (`src/libs/kt-engine/tests/sidechain.rs`) — with a deleted track the device sounds like an ordinary compressor; with mute on the key there is no ducking; there is no panic and no silence in the music.
- [x] 3.5 The cost of the key branch is measured on a dense scene (a dozen tracks, a key with a chain): test t778 (`src/libs/kt-engine/tests/sidechain.rs`) holds a ceiling relative to the same project without keys — the rule "cost is measured, not asserted".

## 4. Format

- [x] 4.1 The key is written as a track address, only when it is assigned; a file without a sidechain is byte for byte the same as before; a file with a key — v12; a key to a non-existent track is read as "no key" and is not revived on writing. Test t773 (`src/libs/kt-otio/tests/convert.rs`), t433b is green without edits.

## 5. Form: two compressors in the track window

- [x] 5.1 The ducking section in `track_window.rs` is rewritten into TWO compressors with the same `params_grid`: on the left the device of this track with the key choice, on the right the muted compressor of the key track (if absent, it is created by one action). Both edits are one window session and one history step; "Cancel" reverts both. Test t775 (`src/apps/kt-app/src/layout_tests.rs`).
- [x] 5.2 The key list does not offer the impossible (itself, a cycle), the reason is named in the status line; a dead key is shown as "no key" and is cleared with one action. Test t776 (`src/apps/kt-app/src/layout_tests.rs`).
- [x] 5.3 New labels — into all five catalogs; the font subsets are rebuilt (`python3 scripts/subset-fonts.py`). Tests `i18n_tests.rs`, `font_tests.rs`, t756c are green.
- [x] 5.4 Reachability: the path "assign key" exists both from the track window and from its chain strip, and it is one body. Test t777 (`src/apps/kt-app/src/reach_tests.rs`).

## 6. Removing the snapshot

- [x] 6.1 Remove `src/libs/kt-engine/src/ducking.rs`, `kt_model::ops::duck_curve_commands`, the background ducking task (`analysis.rs`), `Workspace::duck` (`user_store.rs`) and their calls in `app.rs`; remove the ducking parts of tests t341 and t420/t421/t756/t756b or port them to the remaining analyses. Check: `cargo test --workspace` is green, and the words `duck_spans`/`duck_curve_commands` do not appear in the kt-app and kt-engine sources.
- [x] 6.2 The guard "the ducking task no longer exists": test t774 (`src/apps/kt-app/src/analysis.rs`) — it does not exist in the queue of background analyses, loudness and sync work as before.
- [x] 6.3 The window registry and reachability are intact: t757 and t252/t253 are green — removing the section left no orphaned command and no dead menu item.

## 7. Live scenario and finish

- [x] 7.1 `src/scripts/live/sidechain.py`: on the speech track two lines, on the music track a steady tone; the key is assigned by hand from the track window; assert that during playback the music level dips under the speech and comes back between the lines (measured on the output, not from the picture), and that after Ctrl+S the key is in the file. Run — `./scripts/live-check.sh sidechain`.
- [x] 7.2 The capability specs are updated by the delta, `openspec validate --all --strict`; in `openspec/specs/audio-render/design.md` and `audio-fx/design.md` the "why" is recorded (the revision of the snapshot decision, the key window, the order of stages, the ban on cycles); in `docs/features/audio/processing/compressor-and-limiter.md` the "Verdict" notes that the former "live sidechain — not now" is cancelled by the owner's decision, with the change's address.
- [x] 7.3 `cargo test --workspace` (kt-app per crate under the SIGBUS flake), `cargo clippy --all-targets`, `./scripts/live-check.sh` (all scenarios), `./src/scripts/finish.sh` — the version in the report; a commit of the stage; open the owner's project with the built kt-app and say in the report what is visible.

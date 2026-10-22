# Tasks

## 1. Track as an object (B1)

- [x] 1.1 `kt-model`: `Track { id, kind, name, height, mute, solo, lock, level, blend, fx, fx_bypass, fx_off, fx_auto }`, a vector at the root and in each reel; remove the counters, `lane_order`, `UiState.lane_names/lane_heights`; `rows()` from the vector; validation "number = rank within the kind"; commands `InsertTrack/RemoveTrack/MoveTrack/SetTrackProps` (one reorder door). Tests t49/t241/t588–t594 rewritten for the vector without weakening; new t729 (`src/libs/kt-model/tests/track.rs`): the id is stable on insertion/removal/reordering, name and height in undo, t426 (destructuring) updated.
- [x] 1.2 `kt-model`: the passport of track properties (level, opacity, mode, mute), curves on a track are rejected loudly; test t730 (`src/libs/kt-model/tests/track.rs`).
- [x] 1.3 Lock: `target_track(cmd)` — one function for all commands with an event address, `Refuse::TrackLocked`; a guard over the command registry; test t738 (`src/libs/kt-model/tests/track.rs`): each registry command either passes through the gate or is declared "does not touch events"; the properties of a locked track are editable.
- [x] 1.4 `kt-otio`: the track envelope, `enabled = !mute`, `Track.effects`, migration of counters/order/names/heights on reading, the v11 predicate; test t731 (`src/libs/kt-otio/tests/convert.rs`): the round trip with non-defaults is idempotent, a file without novelties is byte-stable (t433b), an old file with `lane_order` and `ui.lane_names` is read into tracks, a foreign disabled track is read as mute and written back as `enabled: false` (t145/t146 class).

## 2. Mute / solo / level in the engine (B1–B2)

- [x] 2.1 `kt-engine` audio: the track stage by condition (chain / level / mute / solo), scratch buffer, multipliers; without the condition — the former summation order. Test t732 (`src/libs/kt-engine/tests/track_mix.rs`): t2/t3/GOLDEN_AUDIO byte for byte without a stage; level −6 dB exact; mute — the track is silent while the clip's FX state is alive (the clip's reverb rings out after unmuting without a cold start); the solo set by kind.
- [x] 2.2 `kt-engine` video: mute excludes the track from the composite, the prefetch order remains; solo by kind; the track level is the composite's alpha. Test t735 (`src/libs/kt-engine/tests/track_video.rs`): the frame without the silenced track, `wanted_frames` contains it, clearing mute — a frame without a miss.
- [x] 2.3 Export: the render window lists the muted tracks and the active solo before the start, kt-cli prints a notice; mute/solo act on the file. Tests t740 (`src/apps/kt-app/src/layout_tests.rs`), t740b (`src/apps/kt-cli/tests/cli.rs`).

## 3. Track FX chain and blend mode (B3)

- [x] 3.1 `kt-fx`: split `ClipProcessor` into the producer and the chain; `TrackProcessor` (a stream, without the clip shape), the kernels are not touched — t322/t328 green. Test t733 (`src/libs/kt-engine/tests/track_fx.rs`): the track chain sounds on the sum of events (reverb extends across the seams, as t449), the tail past the last event is honest, PDC: the output is aligned with the input (shift 0 relative to a clip without a chain), the cost of the window is measured.
- [x] 3.2 `kt-engine` `fxstore.rs`: composite key `Clip | Track`, diff by fingerprint, `retain` by tracks; test t734 (`src/libs/kt-engine/tests/track_fx.rs`): inserting a track above does not kill the state of the chains below (invariant 3), removing a track removes its state.
- [x] 3.3 `kt-engine` video: `PlanLayer::TrackNest` by the same code as `NestSrc`; the enabling condition; memo by `(TrackId, revision, t)`. Test t736 (`src/libs/kt-engine/tests/track_video.rs`): without the condition — the former bytes and cost (t11/t67/t184/GOLDEN_VIDEO green); with a chain — the vignette on the track composite covers the transition entirely (≠ per clip); the memo is hit on the second composite; preview == export.
- [x] 3.4 Blend modes in pairs: CPU `compose` + `composite.wgsl`, tolerance class as for transitions; test t737 (`src/libs/kt-engine/tests/track_blend.rs`, `src/libs/kt-gpu/tests/parity.rs`): each mode against the analytic formula, Normal byte for byte as before, level 0.5 — half the way to the result of the mode, a hole on a track with Multiply does not darken the lower ones, GPU conformance.
- [x] 3.5 The morpher and reels: "below" by the track vector (t235 green), the tracks of a reel are objects of the reel, the sub-mix of a reel applies its stages; test t739 (`src/libs/kt-engine/tests/nest_render.rs`): a track chain inside a reel is heard and seen in the instance, the law E with tracks without stages.

## 4. Interface

- [x] 4.1 `kt-app` `timeline.rs`: badges M / S / lock / FX in the header, gutter width fixed by the gallery of the row, hit zone ≥ 14 px, service rows — only the lock; right click — the same verbs; hotkeys M/S. Test t742 (`src/apps/kt-app/src/layout_tests.rs`): a mouse hit on each badge from the person's point, the width does not jump when the state changes, a repeated press.
- [x] 4.2 "Track properties" window (name, level, mode, switches, chain button), one undo step per session, hands back focus; test t743 (`src/apps/kt-app/src/layout_tests.rs`).
- [x] 4.3 `fx_editor.rs`: target `Track`, the same strip and automation widget (a curve on a track — a rejection with a reason), localization in five catalogs (`i18n_tests.rs`, `font_tests.rs`); test t744 (`src/apps/kt-app/src/layout_tests.rs`).
- [x] 4.4 The lock in the UI: a gesture on a locked track — an error with the track highlighted (mechanism t172); test t745 (`src/apps/kt-app/src/layout_tests.rs`).
- [x] 4.5 `reach_tests.rs`: the track has the full set of verbs, paired paths (menu/badge/key) one body; test t741 (`src/apps/kt-app/src/reach_tests.rs`).
- [x] 4.6 `kt-cli probe` prints the tracks with switches, level, mode and stage; test t746 (`src/apps/kt-cli/tests/cli.rs`).
- [x] 4.7 Live scenario `src/scripts/live/track.py`: mute a track — the picture and meters change, solo, the lock refuses a drag with a highlight, the track chain with a vignette, the render window lists the mute; screenshots in `target/live-out`.

## 5. Finish

- [x] 5.1 The capability specs updated from the delta, `openspec validate --strict`; F-TL-006, F-AUD-004, F-MPH-005, F-CMP-003 → "in-progress" with `round: R.62`; in design.md of audio-render/video-plan — "Deferred: track envelopes, buses/sends, clip blend mode, skipping the decode of a silenced track".
- [x] 5.2 `cargo test --workspace`, `cargo clippy --all-targets`, `./src/scripts/finish.sh` — the version in the report; commit of the stage.

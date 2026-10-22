# Proposal

## Why

Ducking in the program is a SNAPSHOT: a button computes the envelope of the speech track and bakes a volume curve onto the music events. The owner in R.63 rejected this way of control ("this could obviously be done more conveniently via fx-audio on the whole track") and adopted the decision recorded in `docs/user-feedback.md`: ducking becomes a **live sidechain compressor**, and the snapshot is left exactly until this change. He also named the form: "as ducking one can show the same duplicated audio-compressor widget with dimmed fields, i.e. a compressor on the background object and a compressor on the foreground object… meaningless copy-pastes of the same code pile up everywhere."

Why now: half of the work already stands in the engine — the audio track has a stage with a chain, lookahead and an honest tail (R.62); the compressor exists as a node (`CompNode`); PDC exists (R.48). What is missing is exactly a second detector input and the stage order. And the snapshot, while it lives, is a second way of doing the same thing: two forms, two sets of parameters, two places where it breaks.

## What Changes

- **The compressor gets a KEY INPUT** (sidechain): `FxInstance::Compressor` gets a field "key — an audio track". With no key the instrument works exactly as today, byte-for-byte. With a key, the detector computes the envelope from a FOREIGN track, while it compresses its own input. A second kind of effect is not created: it is the same instrument with the address of a second input (the owner's rule "one widget — not a copy").
- **The key — only in the chain of a TRACK** (the owner's decision): an event's compressor does not accept a key, and the model rejects it with a named error, not silently.
- **The stage order becomes a dependency**: the key track is computed before the target track. A cycle (A listens to B, B listens to A) is rejected by the model on write; the render additionally cuts the recursion at the first level.
- **The shape of ducking — TWO COMPRESSORS in the same widget**: in the window of the background audio track, on the left the compressor of this track (the key chosen), on the right — the real compressor of the KEY track, shown dimmed and editable from the same place (if absent, it is created by one action). No new windows and no own form of four numbers.
- **BREAKING (by the owner's decision): the snapshot is removed entirely** — `kt-engine/src/ducking.rs`, `kt_model::ops::duck_curve_commands`, the ducking background task, the parameter memory in `user_store`, and the button section in the track window. Projects where curves are already baked are read and sound as before: these are ordinary `Gain.db` curves and go nowhere — only the way of CREATING them with this button disappears.
- **Format**: the key field — by content (no key — nothing changes in the file); old files are read without migration.
- NOT included: buses and sends (F-AUD-004, deferred), a sidechain from an EVENT or from a bus, lookahead on the sidechain detector, an external key from a file, a gate/expander with a key. Reason: each adds its own address of a second input and its own fork in the stage order; the owner named exactly the track.

## Capabilities

### New Capabilities

(none — all the work falls on the existing capabilities)

### Modified Capabilities

- `audio-fx`: the compressor gets a key input — a detector on a foreign signal with the compression law unchanged; neutrality without a key, byte-for-byte; the node receives the key as a processing parameter, not in its own state.
- `audio-render`: the requirement "Ducking — envelope snapshot" is replaced by a requirement on a live sidechain: the stage order by key dependency, the window and alignment of the key signal, the prohibition of cycles, determinism and cost; a track stage can now depend on another stage.
- `project-format`: the key address (`TrackId`) in the compressor parameters, the version by content, reading of old files, behaviour when the key track is deleted.
- `ui-timeline`: the ducking shape in the audio track window — two compressors in one widget instead of a section of four fields and a button.
- `ui-reach`: reachability of the sidechain from where the person acts (the track window, the chain strip), a named refusal reason (cycle, key deleted), a live scenario; the disappearance of the former button leaves no dead end.
- `media-index`: ducking is removed from the requirement on offline analysis (volume and sync remain) — there is no ducking background task any more.

## Impact

- `src/libs/kt-model/src/project.rs` — the key field of `Compressor`, validation (the key only on a track; the key exists and is audio; no cycle).
- `src/libs/kt-fx/src/audio/{nodes,mod}.rs` — the key signal reaches the node as a processing parameter; `CompNode` computes the detector from the key when there is one.
- `src/libs/kt-engine/src/audio.rs` — the stage order by dependency, rendering of the key signal on the target's window; `fxstore.rs` — the state of the key path.
- `src/libs/kt-engine/src/ducking.rs`, `src/libs/kt-engine/tests/ducking.rs`, `src/libs/kt-model/src/ops.rs` (`duck_curve_commands`) — are DELETED.
- `src/apps/kt-app/src/track_window.rs` — the ducking section is rewritten into two compressors; `app.rs` (`start_ducking`, `duck_*`), `user_store.rs` (`Workspace::duck`), `analysis.rs` (the ducking task) — are cleaned up.
- Tests: t341, t420, t421, t756, t756b and their parts about ducking are removed or rewritten; the new ones start from t767; live scenario `src/scripts/live/sidechain.py`.
- `docs/features/audio/processing/compressor-and-limiter.md` — the "Verdict" records that the former "live sidechain — not now" is cancelled by the owner's direct decision.

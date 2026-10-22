# Design

## Context

See proposal.md — Why. Below only what is already in the code and determines the shape of the solution.

- **The audio track stage is ready** (`src/libs/kt-engine/src/audio.rs`, `TrackStage`): the sum of the track's events → `TrackProcessor` → level/mute → to the output. The stage already has lookahead (`la`), an honest tail, and state keyed by `TrackId` (`fxstore.rs`).
- **The compressor is already a node** (`src/libs/kt-fx/src/audio/nodes.rs`, `CompNode`): a peak detector `EnvFollower` with stereo link, smoothed parameters. The detector reads the SAME buffer that it compresses — this is the only place that has to be separated.
- **The nodes already receive the position**: `AudioNode::process(buf, pos)`; the key signal goes alongside as a third argument, not into the node's state.
- **The ducking snapshot** (`kt-engine/src/ducking.rs` + `kt_model::ops::duck_curve_commands` + the background task in `analysis.rs` + the section in `track_window.rs` + `Workspace::duck`) — intact and subject to complete removal (the owner's answer in R.65: "remove it entirely").
- **The track address is stable**: `TrackId` already serves as the key of the FX state precisely because the track number is the row rank (R.62).
- **The envelope version is v11**; a new field that the old build silently drops MUST raise it.

## Goals / Non-Goals

**Goals:**
- A live sidechain on the existing compressor: the key is another audio track; the compression law is the same.
- The order of stages by dependency, without losing determinism and without changing the f32 summation order where there are no keys.
- The shape of two compressors in the already existing place (the audio track window).
- Full removal of the snapshot together with its parameter memory and background task.

**Non-Goals:**
- Buses, sends, a sidechain from an event, from a bus or from a file (F-AUD-004 stays deferred).
- Lookahead on the sidechain detector and an external key filter (HPF on the detector).
- A key on a gate/expander: the owner was talking about ducking, and each new instrument with a key has its own set of questions about the stage order.

## Decisions

- **Revision: "Ducking — snapshot, live sidechain rejected (XL)" is cancelled.** In `openspec/specs/audio-render/design.md` it said: "Live sidechain — a second input to the effect, buses, a format change (XL); practical use is covered by the envelope". The reason for the cancellation is the owner's direct decision (R.63, confirmed by the answers in R.65), and the XL estimate is no longer correct: buses are not needed (the key is a TRACK, and track stages appeared in R.62); the second input is a processing argument, not a new graph; the format change is one field by content.
- **The key is a field of the existing `Compressor`, not a new kind of effect.** The owner's rule "one widget — not a copy": "ducking" is a compressor with an assigned key. The alternative (`FxInstance::Ducker`) was rejected: it would double the parameter passport, the registry, the captions and the skeleton tests, and would give two different compression laws at the first edit.
- **The key is `TrackId`, not the track number.** The number is the row rank; inserting a track above would hand the key to someone else's signal — exactly the defect because of which FX state has been keyed by `TrackId` since R.62.
- **The key is "how the track sounds": the events with their weights and FX, the key's chain, level, mute/solo.** The owner chose "a real speech compressor" as the second widget — so the detector must hear its work. The alternative "the sum of the events without the key's chain" is cheaper, but then editing the speech compressor would not change the ducking, and the second widget would lie.
- **The key is computed AT THE SLOT'S POSITION, on its own thread, with its own state.** The target stage with lookahead reads its input on a shifted window, and the nodes AFTER the node with the analysis window see samples older than the input by its window; the detector must be aligned with what it compresses (see "Execution lessons"). The alternative "take the cached output of the key stage" was rejected: with lookahead on the target the windows do not coincide, and one processor cannot be run twice per frame — it would lose continuity (invariant 3). The cost is a second run of the key's chain; it is measured by a test, not asserted (the audio-fx requirement "the cost is measured").
- **The stage order is a topological sort by key, stable.** With no dependencies the order is exactly as before (the row rank); otherwise the f32 summation order and `GOLDEN_AUDIO` would shift.
- **A cycle is forbidden IN THE MODEL, and the render guards with a break at the first level.** A command that creates a cycle is rejected with a named reason (rule 16); inside the key branch the compressor with a key works as an ordinary one — then even a file assembled by another hand gives a definite frame, not a panic.
- **The sidechain adds no lookahead.** `chain_lookahead` does not depend on the key: ducking has no output delay, and PDC (R.48) requires that lookahead lives in both formulas and matches.
- **A dead key means "no key", not an error.** The track is deleted or became a video — the instrument works as an ordinary compressor, the project opens, and the interface says so in words. Rule 20: nothing is cut off by itself, but the render has no right to silently produce silence.
- **The shape is two compressors in one `params_grid` widget.** On the left, the chain of this track; on the right, the compressor of the key track, dimmed; if it is absent, it is created by one action and the same undo step of the window session. No new windows (rule 22), no own form of four numbers (rule 24); the parameters live in the project and therefore "are remembered" by construction (rule 23).
- **The snapshot is removed entirely.** The owner's direct answer. Curves already baked into projects are ordinary automation: they are read and sound, and their format does not change.
- **The envelope version is v12 by content**, only when a key is assigned.

## Risks / Trade-offs

- [The key chain is computed twice — for its own output and for the detector] → a typical key is speech with a compressor (cheap); the cost is measured by a test on a dense scene, and if it turns out to be noticeable, we have a cheap fallback — the key's output cache at `la == 0` on the target.
- [The key is muted → no ducking, and this can be taken for a breakage] → the status line says so at the key's selection; the behaviour is the same as what the person hears.
- [The topological order changes the f32 summation order in projects WITH keys] → such projects have no former reference by construction (a sidechain did not exist before); projects WITHOUT keys keep the former order, and `GOLDEN_AUDIO`, t2, t3 stand on them.
- [Removing the snapshot takes away the path "send the envelope outward"] → the owner chose this as a direct answer, knowing the loss; the curves baked into old projects are intact.
- [The second compressor is edited from another window — it is easy to get an unexpected undo step] → both edits go through one session of the track window and one history step; "Cancel" rolls back both.

## Execution lessons (R.65)

- **The key is aligned to the SLOT, not to the chain's window.** The node after the limiter processes samples delayed by that limiter's window; the key taken on the input window of the chain arrived 5 ms earlier than the speech. `TrackProcessor::slot_delay(slot)` appeared — the sum of the lookaheads of the nodes BEFORE the slot — and the window of the key is shifted by it. Caught by t770; in the UI harness this is not visible.
- **The reference for "did it dip" is a run WITHOUT THE INSTRUMENT, not without the key.** A compressor without a key compresses its own input, and under speech the two runs coincidentally match: the first version of the live scenario declared a working ducking broken, and the second declared a broken one working.
- **A window that grew breaks reachability.** With two compressors the track window outgrew the screen, and "Close"/"Cancel" moved past the edge (t775 failed precisely on this). The window body is now in a scroll area, and the buttons are under it.
- **Removing a function drags the catalogue along.** With the snapshot, the key `msg.analysis_stale` was orphaned (its only consumer), and t125 honestly turned red; the key was removed from all five catalogues.
- **`kt-cli render` writes a float32 WAV**, which Python's `wave` module refuses to read (`unknown format: 3`) — the live scenario parses the chunks itself.

## Migration Plan

Format: the key field is written only when a key is assigned; a file without a sidechain stays byte-for-byte the same and on the same version. A file with a key — v12; an old build gets an honest refusal by version instead of a silent loss. Rollback: remove the key — the next write returns the file to the former version. Projects with baked ducking curves work without changes.

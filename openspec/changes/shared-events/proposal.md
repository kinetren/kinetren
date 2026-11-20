# Proposal: shared-events

> DESIGN ONLY (spec session). Code starts after the owner answers the question at the end and the
> orchestrator gives the go. Tests reserved: t3367–t3378. Base: `orch-26-11-01` at `502947e3`.

## Why

Two measured rows have one mechanism: **an edit copies, compares and hashes the WHOLE project, whatever
it touched.**

- **found-defects 641**: on the UI thread, an edit hands the player a whole-project clone (`doc_arc`).
  `kt-cli edit-cost` measured 0.3 ms at 1357 events, 4 ms at 13 357 and 10.9 ms at 41 357.
- **found-defects 192**: off the UI thread, the incremental build still runs the root's resolve and the
  Debug-text hash of every event, once per version. Measured with `edit-cost --far`: 6.6 ms at ×1 and
  49 ms at ×10, linear in the project. At ×10 a 60 Hz drag coalesces to 74 builds over 120 steps, at 128 % of
  a core.

The frame-work-by-edit design (D9) traced both rows to one place: the model has no event identity that
lasts from one version to the next. Every version is a fresh deep copy. Reading `model.rs`, one
`apply_batch` makes four to five whole-project copies plus one whole-project comparison:

- the scratch, `bench.clone()`;
- the equality check `scratch == project`, which compares every event;
- the undo snapshot, `project.clone()`;
- `rebuild_bench`, `project.clone()` plus the bands;
- the player's snapshot, `doc_arc`. Inside a reel it is `reel_document`, and the Viewer makes one more
  clone.

Row 641 measured only the last of these. This is the audit's step 4 (`docs/engine-audit.md` §2a):
"a persistent per-event identity across versions".

## What Changes

- **Events are shared between versions.** The model holds each event and its siblings (transitions,
  morpher layers, cameras, foreign placeholders) in a `Shared<T>` cell: an `Arc` that keeps value
  semantics.
  - A clone copies pointers.
  - An edit copies only the events it writes (`Shared::edit`, copy on write).
  - Equality is pointer-first.
  - Debug and serde are transparent: the file bytes, every Debug-text key and every hash stay as they
    are.
- **An event's fingerprint is computed once per version of the event.** A `Shared` cell keeps a lazily
  computed hash of its Debug text, the same value `hash_debug` gives. An edit replaces the cell, so a
  stale fingerprint cannot exist. The builders' per-event key reads it instead of re-hashing the text.
- **The resolve passes through what no layer reaches.** `resolve_reel` returns the same `Shared` cell for
  an event that no morpher layer, link or override touches. A layer's reach is found through the
  existing indexes, so the resolve costs pointer copies plus the events actually reached.
- **Work is counted, not timed.** New cost counters `EventClone` and `EventHash` let the guards assert
  that an edit of one event deep-copies and hashes O(1) events at any project size.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `project-format`: an edit copies only the events it writes, and the file stays byte-equal.
- `transport`: one build per version hashes and derives only the events that changed.

## Impact

- **kt-model:**
  - `project.rs`: a new `shared.rs`; the `Vec<Shared<…>>` fields of `Project` and `Sequence`; the
    `ResolvedReel` clips.
  - `model.rs`: `simulate`, `push_undo`, `rebuild_bench`, `doc_arc`, the equality checks.
- **kt-engine:** `incremental.rs` and the builders' keys (`audio.rs:849`, `video.rs`).
- **kt-app:** the Viewer's cut, which edits one event; call sites that build events take
  `Shared::new`.
- **Invariants:** 11 is untouched, because every key keeps its value. 12 is helped: the UI thread no
  longer copies the project.
- **The format does not change.** t148, t811 and t822 hold as they are, since serde is transparent.
- **OTIO export:** nothing is lost, because the doors read through `Deref`.
- **Dependencies:** none new. `Arc` is std, and serde's `rc` feature is not needed because `Shared`
  implements serde itself.

## Questions for the owner

**Q1. Do this now, or when a project of 10 000+ events is reported?**

What the row says today: "when a project of this size is reported". Your project has 1974 events. What
`kt-cli edit-cost` measured (release):

- **×1 showcase (1357 events):** UI-thread handover 0.3 ms, incremental build 6.6 ms, 11 ms of a core per
  edit.
- **×10 (13 357 events):** UI thread 4 ms per edit, build 49 ms, a 60 Hz slider drag at 128 % of a core.

The drag line was not recorded at ×1. Your rule (2026-11-07) is that a drag must not heat the machine.

- **a) Measure your size first.** An executor runs the existing `edit-cost` on a ~2000-event project; the
  drag line decides. Building starts if a drag at your size costs more than half a core.
  *(Recommended: no code before the number, as with rows 770/771.)*
- **b) Build now.** It removes the linear cost at every size. The risk is a wide mechanical change in
  kt-model (every `iter_mut` and `push` on the event vectors), with no format change.
- **c) Later.** Wait until a ×10 project is reported.

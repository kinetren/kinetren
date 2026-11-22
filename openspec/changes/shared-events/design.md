# Design: shared-events

## Context

- `TimelineModel` (`kt-model/src/model.rs`) holds the canon `project`, the working view `bench` (the canon
  plus open bands), a lazily built `doc: OnceLock<Arc<Project>>`, and `undo`/`redo` of `Snapshot { project,
  scope, bands }` ("snapshots of the whole `Project`", the module doc).
- `apply_batch`: `simulate` clones `bench`, runs the commands, assembles the canon; `scratch == self.project`
  compares the whole project (input-without-work); `push_undo` clones the canon; `rebuild_bench` clones it
  again and merges the bands; `push_project_to_player` asks `doc_arc` (one more clone, or `reel_document`
  inside a reel; the Viewer clones once more and cuts one event).
- The builders (`kt-engine/src/incremental.rs`, edit-without-rebuild): `BuildMemo` keys an event's derived
  state by `hash_debug(c)` (the event's Debug text, streamed into xxh3) plus its reel context; `resolve`
  memoizes a reel per build by the project's ADDRESS. Every build hashes every event's text and resolves
  the root afresh.
- builder-cost-guard and frame-work-by-edit already count per-element work (`kt_time::cost`), and
  `kt-engine/tests/build_cost.rs` holds every pass linear by count.

## Goals / Non-Goals

Goals: an edit of k events deep-copies and hashes O(k) events, at any project size; the UI thread hands
the player a snapshot in O(pointer copies); no byte of the file, no key, no hash and no render changes.

Non-goals: a persistent index patched in place (an interval tree per track kept across versions); an
`im`/`rpds` persistent vector (a new dependency); making the resolve sub-linear (it stays a linear pass of
pointer copies, D4).

## D1. `Shared<T>`: an event cell with value semantics

`kt_model::shared::Shared<T>`: `Arc<Cell<T>>` where `Cell { value: T, fp: OnceLock<u64> }`.

- `Deref<Target = T>`: every reader (`c.id`, `c.start`, `iter().find(..)`) reads as before.
- `Shared::new(T)`, `Shared::edit(&mut self) -> &mut T`: `Arc::make_mut` (copies the value if shared,
  counting `Cost::EventClone`), then CLEARS `fp` (a fresh `OnceLock`) — a written cell never carries the
  old fingerprint.
- `Clone`: an `Arc` clone. `PartialEq`: `Arc::ptr_eq` first, then the values.
- `Debug`: the value's Debug, nothing added — so `hash_debug` of an event, of a `Vec<Shared<ClipEvent>>`,
  of a whole project, is the same string as today (render-cache-disk's durable keys, the scene hash of
  the kept mix, the generator salt, the trace's model text all hash Debug text).
- `Serialize`/`Deserialize`: the value's own (transparent), so the `.kt` text and the clipboard are byte
  for byte.
- `fp(&self) -> u64`: `hash_debug(&value)` computed on first ask and kept in the cell (the hash function
  moves from kt-engine's `incremental.rs` into kt-model, the same xxh3 over the same text).

Which fields: `Project.{clips, transitions, morphers, cameras, foreign}`, the same in `Sequence`, and
`ResolvedReel.clips`. Media, tracks, markers and areas stay plain (small, rarely the bulk; listed in D6).

## D1a. The container `Events<T>` (implementation-ready, 2026-11-20)

A bare `Vec<Shared<T>>` would touch every reader: `clips.iter()` has 443 call sites, `clips[i]` 417,
`clips.len()` 111 and `clips.push` 634 (most in tests). So the fields become
`kt_model::shared::Events<T>`, a newtype over `Vec<Shared<T>>`, whose surface is a `Vec<T>`'s where that
is cheap and honest:

| `Vec<T>` use | `Events<T>` | Cost |
|---|---|---|
| `iter()`, `&events` in `for` | yields `&T` (through the cell) | none |
| `events[i]` (read) | `Index<usize, Output = T>` | none |
| `events[i].x = v` | `IndexMut` → `Shared::edit` (copy-on-write of that one cell) | one copy if shared |
| `iter_mut()` | yields `&mut T` through `edit()` on EVERY cell | copies every shared cell: correct but costly; the guard t3375 counts the copies of every command family on a large project, so a command left on `iter_mut` is red |
| `push(T)`, `insert(i, T)`, `extend(iter of T)` | take `T` and wrap it | none |
| `len`, `is_empty`, `first`, `last`, `get`, `remove`, `retain`, `sort_by_key`, `truncate`, `drain(..)` | same names; `remove`/`drain` give `T` by `Arc::unwrap_or_clone` | none |
| `vec![…]` literals | `vec![…].into()` (`From<Vec<T>>`), `collect()` (`FromIterator<T>`) | none |
| `&events` passed as `&[T]` | not possible (cells, not values): the 19 signatures taking `&[ClipEvent]` take `&Events<ClipEvent>` or `impl IntoIterator<Item = &ClipEvent>` | none |
| `Debug` | `debug_list` of the values: the same text as `Vec<T>`'s | none |
| serde | a sequence of `T`: the same `.kt` text and clipboard | none |
| `Clone` | pointer copies | O(n) refcount increments |
| `PartialEq` | pointer-equal cells skipped, then values | O(edited) |

Fine-grained writers use `events.edit(i)`, `events.edit_where(|c| c.id == id)`, or
`events.edit_if(i, |v| changed)` (no copy when nothing changes). `ResolvedReel`'s `Cow<'a, [T]>` fields
become `Cow<'a, Events<T>>`: the borrowed path stays free (t673/t676), and an owned one is pointer copies.

## D2. The model's copies become pointer copies

| Copy | Today | After |
|---|---|---|
| `simulate`: `bench.clone()` | deep | pointers; commands call `edit()` on the events they write |
| `scratch == project` | every event's fields | pointer-equal events skipped; only edited cells compared |
| `push_undo` | deep | pointers |
| `rebuild_bench` | deep + merge | pointers + merge (a band's events are edited where moved) |
| `doc_arc` / `reel_document` | deep | pointers (the swap moves cells, not values) |
| Viewer cut (`kt-app`) | deep | pointers + one `edit()` |
| autosave (from `doc_arc`) | serialization only | unchanged |

The rule that keeps it honest: **a command writes an event only through `edit()`**. `iter_mut()` on a
`Vec<Shared<T>>` yields `&mut Shared<T>`, which does not deref mutably, so the compiler finds every
writer; each becomes `c.edit().field = …`. A writer that edits EVERY event (a lane renumbering,
`set_lane_order`) copies only the events whose value changes: it compares before calling `edit()`
(`Shared::edit_if(|v| …)`), or a renumbering of 3 rows would copy the project.

## D3. The builders key by the cell's fingerprint

`incremental.rs`: the audio and video per-event keys take `c.fp()` instead of `hash_debug(c)` — the same
number, computed once per version of the event instead of once per build. `resolve`'s per-build memo
keeps its key; the whole-project questions that hashed every event (`scene_base` parts, generator sources)
read the cells' fingerprints where they hashed the events' text. `Cost::EventHash` counts each `fp`
computed: an edit of one event computes one (t3371).

## D4. The resolve passes cells through

`Project::resolve_reel` / `effective_clips`: an event that no morpher layer reaches (`windows_on` — the reach
test runs on the existing `ZoneIndex`/layer indexes), with no link to resolve and no override, is pushed
as the SAME cell (a pointer copy); a reached event is a new cell, as today. The pass stays linear (one
reach question per event, already O(log n) each), but it allocates and copies only the reached events.
The cell's fingerprint survives the pass for an untouched event, so D3 hashes nothing new for it.

## D5. What the numbers should become (to measure, `kt-cli edit-cost`, release)

Guesses to check, not promises: ×10 UI-thread handover 4 ms → well under 0.5 ms (pointer copies of
13 357 cells and their refcounts); ×10 incremental build 49 ms → the resolve's linear pass plus the memo
lookups (~5 ms); a drag's CPU falls in the same proportion. The guards do not time anything: they count
`EventClone` and `EventHash` (t3367–t3372), as test-stability requires.

## D6. Class walk: every place that copies, compares or hashes the whole project per edit or per frame

| Place | Per | Today | After |
|---|---|---|---|
| `simulate` scratch | edit | deep clone | D2 |
| input-without-work compare | edit | full compare | D2 |
| undo / redo snapshots | edit | deep clone | D2 |
| `rebuild_bench` | edit, setter outside history | deep clone | D2 |
| `doc_arc`, `reel_document` | edit (player), window publish | deep clone | D2 |
| Viewer cut | edit | deep clone | D2 |
| `apply_coalesced` (drag) | step | as `apply_batch` | D2 |
| builders' event keys | build | hash every event | D3 |
| root resolve | build | new vector of every event | D4 |
| `scene_base` / generator salt | build | hashes model text | D3 (fingerprints) |
| autosave | quiet | serializes | unchanged (must write text) |
| telemetry `Signal::Command` | edit | the command, not the project | unchanged |
| `validate` after a batch | edit | linear by requirement (t-linear) | unchanged; its per-event checks could skip pointer-equal events — a follow-up if measured |
| media, tracks, markers, areas | edit | deep clone (small vectors) | unchanged |
| timeline drawing (`doc()`) | frame | borrowed (ui-frame-cost) | unchanged |
| render dialog, settings, fx editors (`document()`) | open / publish | deep clone | pointers (they clone a `Project`) |
| kt-otio export | export | reads | unchanged (`Deref`) |

## D7. Stages

The owner chose to build it now (kkt-26.11.20 A19, his Q31 b: "Строить сейчас"); the measurement is the
before/after record, not a gate.

1. Before: `kt-cli edit-cost` (release) on the generated 2000- and 13 000-event projects, the numbers into
   D5.
2. D1 + D2: `Shared`, the fields, the model's copies; t3367–t3370; the format and golden tests unchanged.
3. D3 + D4: the fingerprints in the builders, the resolve's pass-through; t3371–t3374; the `edit-cost`
   numbers before/after in this design.
Spare: t3375–t3378.

## Risks

- A writer that mutates through interior mutability bypassing `edit()`: there is none in `ClipEvent`
  (`FxInstance` holds plain values); the fingerprint test t3370 edits every field family through every
  command and checks `fp()` equals `hash_debug` of the value.
- Refcount traffic on the engine threads: a build holding `Arc<Project>` already shares it; cells add one
  atomic per event per clone, which is the cost the change trades the deep copy for.

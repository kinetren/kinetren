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

## D1a. The container `Events<T>` (implementation-ready, 2026-11-20; `&mut` revised 2026-11-24)

A bare `Vec<Shared<T>>` would touch every reader: `clips.iter()` has 443 call sites, `clips[i]` 417,
`clips.len()` 111 and `clips.push` 634 (most in tests). So the fields become
`kt_model::shared::Events<T>`, a newtype over `Vec<Shared<T>>`, whose surface is a `Vec<T>`'s where that
is cheap and honest:

| `Vec<T>` use | `Events<T>` | Cost |
|---|---|---|
| `iter()`, `&events` in `for` | yields `&T` (through the cell) | none |
| `events[i]` (read) | `Index<usize, Output = T>` | none |
| `events[i].x = v` | `IndexMut` → `Shared::edit` (copy-on-write of that one cell) | one copy if shared |
| `iter_mut()`, `for c in &mut events` | **not there** | the compiler lists every writer (121 `&mut …clips`-shaped borrows, 73 of them `for … in &mut …` loops, on 2026-11-24); each becomes `edit_if`, `edit_where` or `edit(i)`. A container that yielded `&mut T` through `edit()` would compile them as they are and copy the whole project per `MoveClip` (a loop over every event that writes two) |
| the same, where every event IS written (loading: `normalize`, `migrate_gen_to_events`, importers, test builders) | `edit_all()` | copies every shared cell, and its name says so; a freshly loaded project's cells are not shared, so it copies none |
| `push(T)`, `insert(i, T)`, `extend(iter of T)` | take `T` and wrap it | none |
| `len`, `is_empty`, `first`, `last`, `get`, `remove`, `retain`, `sort_by_key`, `truncate`, `drain(..)` | same names; `remove`/`drain` give `T` by `Arc::unwrap_or_clone` | none |
| `vec![…]` literals | `vec![…].into()` (`From<Vec<T>>`), `collect()` (`FromIterator<T>`) | none |
| `&events` passed as `&[T]` | not possible (cells, not values): the 19 signatures taking `&[ClipEvent]` take `&Events<ClipEvent>` or `impl IntoIterator<Item = &ClipEvent>` | none |
| `Debug` | `debug_list` of the values: the same text as `Vec<T>`'s | none |
| serde | a sequence of `T`: the same `.kt` text and clipboard | none |
| `Clone` | pointer copies | O(n) refcount increments |
| `PartialEq` | pointer-equal cells skipped, then values | O(edited) |

Fine-grained writers use `events.edit(i)`, `events.edit_where(|c| c.id == id)`, or
`events.edit_if(|v| would_change, |v| write)` over the whole vector (the test reads, the write copies). The row
shifts of `insert_track`, `merge_reel` and `unmerge_reel` (`project.rs:815–828`, `:925–936`, `:6726–6797`, `:6910–6920`)
are `edit_if`: an unchanged lane copies nothing. `ResolvedReel`'s `Cow<'a, [T]>` fields
become `Cow<'a, Events<T>>`: the borrowed path stays free (t673/t676), and an owned one is pointer copies.

Where the types move (checked against orch `286c406c`, 2026-11-24):
- **`trait Reel`** (`project.rs:400`): its five accessors return `&Events<…>`. Three of them have default bodies `&[]`,
  which borrow a promoted constant; an `Events` is not promoted (it drops a `Vec`). Each default returns a `static EMPTY:
  Events<…> = Events::new()` of its own (`new` is a `const fn`; `Shared<T>` is `Sync` since `OnceLock<u64>` is).
- **`apply_overrides` and `apply_one`** (`published.rs:252`, `:290`) take `&mut Events<…>` and write one event by
  `edit_where`: an instance's override copies the events it names, not the reel. Their `clips.to_vec()` snapshot is a
  pointer copy.
- **`links::resolve_derived`** returns `Cow<'_, Events<ClipEvent>>`; a project with one link copied every event of the
  reel (`into_owned` in `morph_pass`, `project.rs:7006`): only the followers are edited now.
- **`listen::feed_for`** (gen-audio-reactive, `kt-engine/src/listen.rs:80`) and its `Vec<&[ClipEvent]>` take `&Events`.
- **Unchanged:** `ClipBlock` (the clipboard, `ops.rs:15`) and `Command` hold plain values; `fx_in`, the wiring of
  `fx-graph-dag`, is a field of the event: it is in the Debug text, so in the fingerprint, with nothing to add.
- **The wiring's writers** (`mirror_halves`, `model.rs:91–348`; `SetFxIn`, `:2616–2643`) take several `&mut` fields of one
  event: one `let c = clips.edit(i);` and then the field borrows, as today.

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

The rule that keeps it honest: **a command writes an event only through `edit()`**. `Events` has no `iter_mut` and no
`IntoIterator` by `&mut` (D1a), so the compiler finds every writer. A writer that edits EVERY event (a lane renumbering,
`set_lane_order`) copies only the events whose value changes: it compares before calling `edit()`
(`Shared::edit_if(|v| …)`), or a renumbering of 3 rows would copy the project.

## D3. The builders key by the cell's fingerprint

`incremental.rs`: the audio and video per-event keys take `c.fp()` instead of `hash_debug(c)` — the same
number, computed once per version of the event instead of once per build. (`audio.rs:924`, `video.rs:1404`; the other callers of `hash_debug`, `incremental.rs:157` the overrides and
`video.rs:1359` the cameras, hash no event.) `resolve`'s per-build memo keeps its key. The video key's text goes on holding
`heard` (gen-audio-reactive) and the `raster` beside the fingerprint.

**A reel's revision keeps its number** (`hash_reel`, `video.rs:549`, revised 2026-11-24). It hashes the Debug text of the
reel's vectors, and the number names reel strips on disk (`reels/` of the cache folder) and the nested keys: mixing the
cells' fingerprints instead would change it. So the reel's OWN part (everything before its nested reels) is memoized on
the builder's thread beside `BuildMemo`: the last `Events` vectors of that reel (pointer copies), its tracks, published
set, raster and fps, and the number. Pointer-equal vectors and an equal rest give the number with no text. An edit in the
root hashes no reel; an edit in a reel hashes that reel and the digests above it. `Cost::EventHash` counts each `fp`
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

**Before** (2026-11-25, `41d149bc`, release `kt-cli edit-cost`, `showcase --heavy` over the fixture videos: 518 events, grown to
×1 and ×10 of rows 192/641 by `--far 420` and `--far 6420`; minima of three runs, load average 10 on the machine):

| | 518 events | 1358 | 13 358 |
|---|---|---|---|
| document clone | 0.11 ms | 0.24 ms | 2.47 ms |
| a build, fresh / incremental (the root's resolve in it) | 3.4 / 2.0 (0.2) ms | 6.4 / 4.6 (0.3) ms | 51.4 / 44.4 (4.8) ms |
| one edit, UI thread p50 / CPU of the process | 0.52 / 4.0 ms | 0.74 / 7.0 ms | 3.86 / 54.0 ms |
| a 60 Hz drag of 120 steps: UI thread p50 / CPU / builds | 0.36 ms / 18 % / 240 | 0.53 ms / 34 % / 240 | 4.23 ms / 120 % / 76–82 |

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
| a reel's revision (`hash_reel`), per reel | build | hashes the reel's text | D3 (its own part memoized by pointer) |
| stream fronts, masks, cameras (`video_stream.rs:276`, `:479`, `:515`; `video.rs:1359`, `:1460`) | build of a touched layer | hashes that layer's text | unchanged: inside `incremental::video_clip`'s miss |
| `links::resolve_derived` | build, with any link | clones the reel | D1a (the followers only) |
| an instance's overrides (`resolve_reel`) | build, per instance with overrides | clones the reel | D1a (the named events only) |
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
- **A long branch.** Section 3 touches some 120 writers and every reader that names `&[ClipEvent]`. It starts when
  `fx-graph-dag` v2d and `durable-moment-key` have merged, is one commit that changes no logic, and nothing else runs in
  kt-model or kt-engine until it is in orch (`docs/build-queue.md`).

# Design: ui-frame-cost

## The mechanism

A UI frame's work grew with the size of the project, not with what the frame shows. Three forms of it:

1. **A copy of the whole document per question.** `TimelineModel::document()` clones the project (and
   swaps a reel back when one is entered). The media bin asked it four times a frame plus twice per
   reel card, the effects window three times, the camera, composition, transition, render, project
   settings and published-parameters windows once or twice each, the reel badge once per reel
   instance in view. On the owner's 1974 events one copy is hundreds of microseconds.
2. **A walk of every event for a thing the frame then culls.** The hatching of a tail past the
   material built the speed map of EVERY event from its curves (formula evaluation included) before
   asking whether the tail was in view; the coverage strip, the event pass, the hit test under the
   pointer and the fade handles walked the list; the transitions looked their events up by a search.
3. **A layout redone where nothing changed.** `source_progress` laid the speed curve's segments out
   on every call (and shifted a vector once per hidden key, quadratic on a trimmed sampled curve);
   the waveform asks it per pixel column, the composite per layer per frame. The bin allocated and
   painted every card of its scroll and hashed every card to place new ones.

Frames are many: egui draws one per pointer event (60 a second on his monitor, ~100–200 under Xvfb).
So the cost was paid per mouse move, and grew with the scroll position and the zoom (more events in
view meant more culled-and-computed work, and the zoomed-out view held all of them).

The rule this change establishes and tests: **a frame costs in proportion to what is visible; nothing
is recomputed for a frame whose inputs did not change.** Caches are keyed by what determines them
(the model revision plus the counts the model can change wholesale without one, the media lengths,
the language, the view), are bounded by the project's own size, and are invisible to determinism.

## Measurements

Release build, his relinked project (1974 events, 8 morphers, 2 reels, Slowed + VHS Tape + Camera ISP
on video and audio) on llvmpipe in Xvfb; UI thread processor time per frame, from `/proc` thread ticks,
and the program's own draw cost by stage (`frame_cost`, new):

| Scene | before | after | our drawing after |
|---|---|---|---|
| pointer over the preview, pause | 6.1 ms | 1.6 ms | 0.35 ms |
| pointer over the timeline | 6.0 ms | 2.0 ms | 0.43 ms |
| the same, zoomed out to the whole edit | 10.1 ms | 5.2 ms | 0.51 ms |
| playback (UI thread incl. the composite) | 32.4 ms | 10.8 ms | 0.8 ms without the composite |
| effects window of event 2132 open, pointer over the preview | 291 ms, 1 frame/s | 5.0 ms, 95 frames/s | 1.4 ms (window 0.9) |

The last row is measured with `input-without-work` merged too: its fix (a VHS value clamped every
frame into a false edit, pushing the project to the player) removed the `set_project` that took 42 of
the 50 ms that remained of the window's frame after this change.

Before this change a gdb sample of the pointer-over-preview frame put 53 % of `draw_all` in project
copies of the bin, 20 % in the waveform's per-column warp and 13 % in the tail hatching.

What the rest is: on this machine most of the UI thread's frame outside our drawing is the GL
driver's (llvmpipe) frontend in `paint_and_update_textures`, and in playback the composite's CPU part is
the same driver (`kt-cli preview`: plan 0.02 ms, encode 0.1–0.2 ms per composite). **Not proven on a
real card:** his Radeon (RADV, Vulkan) has neither path; the numbers he will see are our drawing plus
egui's tessellation plus a Vulkan submit.

## Decisions

- **D1 The borrowed document.** `doc()` returns the canon at the root (no copy) and a swapped copy
  inside a reel, built once per state (`OnceLock`, emptied in `rebuild_bench` and the four setters
  outside history); `doc_arc()` for a window that holds it across its own `&mut` drawing. `document()`
  stays for what leaves the model (file, export, player): t456b and t554 are untouched.
- **D2 The event index** (`EventIndex`, in `TlFacts` keyed by revision, event count and a hash of the
  media lengths): events sorted by start with the running latest end, so a view's events are two
  halvings and a filter; the composition's draw order by rank; ids to indices; the material tails.
  Dragged events are added to the view's set explicitly — they are drawn at their new place.
- **D3 The laid-out integral.** `ClipWarp` keeps `segments()` and the material consumed before each
  segment, summed in the loop's own order, so `material_ticks` is the same bits as
  `warp_material_ticks` (t1910 over sorted, unsorted, hidden-left and sampled curves). The engine's
  composite uses it too (`video.rs`): the same number, no new path.
- **D4 The virtual bin.** In the grid the cards' rectangles are arithmetic (the wrapped row's numbers),
  only visible cards are allocated and drawn, every card keeps its rectangle (`bin_card_rects`, t578).
  The free layout draws only visible cards; new cards are placed when the list or the layout changed.
  A card being dragged is always drawn.
- **D5 Coverage for the view.** The strip polls the cache for the events in view; the view is in the
  stamp, so a scroll polls what came into view, still throttled to 4 Hz during playback.
- **D6 The frame meter** (`frame_cost`): the UI thread's own processor time (`CLOCK_THREAD_CPUTIME_ID`;
  a monotonic clock elsewhere) per stage, two clock reads a stage. A wait is not charged, so the
  number is the same on a card and on llvmpipe. Shown in Diagnostics → Performance ("UI frame … ms" and
  the stages), written per second into the load report (", ui frame N ms"), dumped for live scenarios.
  `visit()` counts per-item drawing work per thread: the structural verdict of t1912.

## Rejected

- A cache of the drawn timeline as a texture: the hover replay (`EventPaint`) already exists for
  pointer frames, and playback changes the picture every frame.
- Throttling frames: egui draws per input event, the cost of a frame is what was wrong.
- A spatial index per row: one sorted list answers the view in microseconds at 20000 events (t1913).

## The class, walked (the owner's rule)

Every draw path of the main window and of every window, what it did per frame, what it does now:

| Place | Found | Now |
|---|---|---|
| Media bin | 4 document copies a frame + 2 per reel card; every card painted, allocated, hashed; path list rebuilt; a lock and two strings per card for a hint; group members found by search | borrowed doc, virtual grid, list and placement per state, hint only under the pointer, members by id |
| Timeline: event pass | walked all events to cull | the index's view set |
| Timeline: waveform | speed curve laid out per pixel column | laid-out integral |
| Timeline: filmstrip | the same map per cell | the same fix |
| Timeline: tail hatching | a speed map built for every event of the project | tails per state, the view's only |
| Timeline: global coverage strip | all events; the poll asked all events with a quadratic quality search | the view's events, qualities grouped once |
| Timeline: hit test under the pointer | all events, twice a hover frame | the events under the pointer's few pixels |
| Timeline: fade handles | all events filtered by two ids | two lookups |
| Timeline: transitions (hit, body, handles) | an event search per transition | by id |
| Timeline: reel ×N badge | a document copy and a walk per reel instance in view | uses counted once per state |
| Timeline: rulers, rows, markers, areas, morphers, cameras, bands | per marker/morpher/camera/row, small sets | checked, unchanged |
| Timeline: names, draw order, overlaps, warps, reel keys | already per revision | checked |
| Timeline: `view_floor` inside a reel | a min over the reel's events | left: a cheap linear min, only inside a reel |
| Transport bar, toolbar, status bar, meters | no project walk | checked |
| Preview and its overlays (geometry frame, masks) | per selected event | checked |
| Composite plan | `kt-cli preview`: 0.02 ms a frame on his project | checked; the warp map per layer now laid out once |
| Effects window | 3 document copies a frame | `doc_arc` / `doc` |
| Camera window | a copy a frame + one per curve | `doc_arc` / `doc` |
| Composition (morpher) window | a copy a frame | `doc` |
| Transition window | a copy a frame | `doc_arc` |
| Render window | a copy a frame for the silenced tracks | `doc_arc` |
| Project settings window | two copies a frame | `doc` |
| Published parameters window and its dialog | four copies a frame | `doc` / `doc_arc` |
| Track window, preferences, controls, welcome, Guide, lessons | no copy; lessons evaluate on a key change | checked |
| Diagnostics | settings cloned per frame (small) | checked |
| Generator sync, media sync (per edit, per coalesced gesture frame) | a copy per edit | `doc` / `doc_arc` |

Left, named in the report: the free layout's per-card rectangle arithmetic for cards below the fold
(t1913 bounds it); a cheap linear `view_floor` inside a reel; the engine's `video_decode_qualities`
on a coverage poll is per event of the project (once per poll, at most 4 Hz in playback).

## The tests

- t1910 — the laid-out integral is `source_progress` bit for bit.
- t1911 — the borrowed document is one per state and follows every edit, undo, place and setter.
- t1912 — events, files and tails outside the view add no drawing work (`frame_cost::visit`).
- t1913 — a frame's processor time does not follow 20000 hidden events or 1000 files below the fold
  (the cheapest of three rounds of ten frames, the test thread's own clock; red before: 11.9 ms against 1.5).
- t1914 — the same with the effects and project settings windows open (red on the old effects
  window: 82 ms against 3.6).
- live `ui_frame` — the heavy showcase, pointer over the preview, the timeline, the zoomed-out timeline
  and an open effects window: each frame's own time under a limit; Diagnostics shows it.

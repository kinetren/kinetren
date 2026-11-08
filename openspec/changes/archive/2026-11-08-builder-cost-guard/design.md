# Design: builder-cost-guard

## The mechanism

**An O(whole) helper called once per element of another structure** (`docs/engine-audit.md` M2).
The helper is linear and looks harmless where it is written; the loop around it is somewhere else.
On a hand-made project both sizes are small and the product is invisible. On a generated one — a
speed curve sampled every 5 ms, a reel placed 400 times, an edit of two thousand events with
transitions — it is the whole cost. Nothing counted it: the only witness was a clock, and a clock
says nothing about growth.

Row 151 is the instance that hurt: `warp::sign_knots` asked `dir_after`, `consumed_abs` and
`source_progress` for every direction-change candidate of a speed profile, and each of them laid the
whole segment list out again (`segments`, `abs_segments`). A shaped speed curve becomes a point
every 5 ms (`rate_profile_keys`, `slowed-reverb-is-an-event` D2), so a 360 s curve has 72 000 points
and the knots cost 72 000 × 72 000 segment visits. `kt_fx::chain_addr_knots` calls it per event in
every audio build, and every edit runs 2 audio builds (`edit-without-rebuild` is the change that
makes it one).

## Decisions

### D1. Lay out once, read by halving — the same bits

`Integral` (ui-frame-cost) already kept `segments` with the material consumed before each, summed
in the loop's own order, and read `source_progress` by halving bit for bit (t1910). `Integral::over`
takes any contiguous segment list, so `abs_segments` gets the same reader for `consumed_abs`.
`sign_knots` lays both out once and asks `dir_after_in` (the first segment ending after the node, by
halving, then the same `eff_rate` at the same midpoint). `span_over` reads its zero crossings from the
same laid-out integral. t2060 holds both against the former functions frozen inside the test, on
flat, straight, zero-crossing, zero-holding, trimmed, unsorted and 3000-point swinging profiles.

`curve_value` walked the keys from the first per reading. Every curve the model holds is validated
sorted (`check_fx_auto` and its siblings), and the sampled and resolved ones are built sorted, so a
halving finds the segment the walk found — the same arithmetic on it. Its per-sample callers were the
profile sampling (S × K), link resolution (frames × keys) and the decode zoom probes (K² per clip).

### D2. A count, not a clock (t1671)

`kt_time::cost` is a thread-local meter: a walk over a whole structure adds the elements it visited
(`warp_segments`, `clip_searches`, `reel_hashes`, `curve_walks`, `reel_passes`). The builders run on
the calling thread, so a test reads exactly its own work. The guard also counts the bytes allocated
on the thread (a counting global allocator in the test binary): a rebuilt `Vec` per element shows
there without anyone instrumenting it — the generic net under the named rows.

The bound: the work at ×4 and ×16 may be at most 2× the scene's factor (8× and 32×). A sort is
n log n; constant terms make the ratio smaller. A quadratic walk gives ~16× and ~256×: on the former
code the guard measured 255× (curve: warp segments), 4480× (validation: clip searches), 79× (links)
and 233× (placements, nested placements). Every axis was red on the former code (the former files
restored with the same counters at the same scan sites) and is green on the new.

Rejected: a wall-clock budget (machine-dependent, rule t1671); counting only the named rows (a new
quadratic site that allocates would pass until someone instrumented it).

### D3. The axes

A project grows along several independent sizes, and the product of two is the mechanism. Each scene
grows them together where the defect is a product:

| test | scene, ×1 | what was quadratic |
|---|---|---|
| t2062 | 24 linked A/V pairs with shaped speed curves, a crossfade at every joint | transition ends scanned in validation and both builds (T × C); the engine events of an end found by `iter_mut().find` (T × C) |
| t2063 | one pair, a shaped speed curve crossing zero over 8 s | `sign_knots` (K²), `curve_value` per sample (S × K) |
| t2064 | a reel of 8 pairs placed 8 times (both grow) | the reel's revision hashed per placement, 2× in the audio build and 4× in the video one (P × M) |
| t2065 | a reel holding 8 placements of a reel of 8 pairs | the inner reel's text fed into its parent's hash per placement |
| t2066 | 24 butted events with crossfades under a cutting layer | the forbidden zones walked every transition and event per target (C × (T + C)) |
| t2067 | 12 events, each gain linked to the previous one's | one link of a chain resolved per pass over the reel (D × C), the leader scanned per sample (S × C) |

The passes: `validate`, `effective_clips`, `AudioRenderer::new`, `VideoRenderer::new`,
`gen_sources`, the project file round trip. `effective_clips` flattens every placement into the
root, so its output is P × M by definition; its bound is its output's size (it is kt-cli's and
`BakeMorpher`'s, not a build's).

### D4. Reel revisions once per build; a child by its revision

`content_hash` hashes the Debug text of a whole reel. The builds ask it per placement (the clip list,
the sub-renderer key, `nest_scales`, `gen_sources`, and the timeline's preview keys per revision).
`revisions_scope` memoizes it per (project, reel) for the duration of a build — the project is
borrowed throughout, so the key cannot go stale (the caches rule of CLAUDE.md). The recursion into
nested reels fed the child's whole text into the parent's hasher per placement; it now feeds the
child's revision, computed once per reel. A reel without nested reels hashes the same bytes as before
— the same revision bit for bit. A reel holding reels gets a new revision ONCE: its sidecar previews
are rendered again once on opening (the same one-time move R.59 made, kt-engine CLAUDE.md).

### D5. Indexes that answer what the scan answered

- `Reel::clip(id)` (default: the scan, counted) and `Indexed` (a hash map, first event of each id;
  `Indexed::over` borrows an index the caller already keeps — the timeline's `EventIndex.by_id`, so
  a draw path builds nothing per frame). `joint`, `overlap_pair`, `window` and
  `check_transition_full` ask it.
- `PieceEdges` (kt-engine): the engine events of a reel by (original id, edge) — exactly what
  `clips.iter_mut().find(|c| same_clip(c.id, id) && c.end == e)` returned, the first such.
- `ZoneIndex` (the cut pass): transitions by the events they name, events by track sorted by start
  with the latest end so far; a target's zones in the scan's order (`snap_out_of` takes the first
  zone holding the point, so order is part of the answer).
- `resolve_derived`: leaders first (a depth-first post-order over the links), each leader by index
  once per curve. A link to the event's own other curve reads a copy of the event taken before its
  step — what the former per-pass snapshot gave it.

### D6. t1935 keeps its case by a seam

t1935 (cached-range-plays) asserted "a rebuild outlasts the ring" as a scene guard, and its scene got
that from the per-placement hash of a 3000-event reel placed 400 times: 412 ms. The build is now
22 ms. A heavy enough honest scene (long shaped curves) would hold gigabytes of points and still
depend on the machine's speed. `hold_builds_for_tests(ms)` makes the builder thread hold each build;
zero in the product, one relaxed load per build.

## Measured: the relinked copy of the owner's project

His project: 1974 events, 4 transitions, 2 reels with 6 placements, 31 media, 8 morpher layers, five
events with shaped speed curves (281: 360 s). Release `kt-cli probe … 100 101`, three alternating
runs of each binary:

| release `kt-cli probe`, load 37–66 | before | after |
|---|---|---|
| `AudioRenderer::new`, whole project | 368, 373, 572 ms (388–412 ms at load 36 earlier) | 11, 12, 12 ms |
| `VideoRenderer::new` | 18, 18, 38 ms | 18, 19, 20 ms |
| `effective_clips` (1987 clips) | 8, 12, 39 ms | 7, 8, 10 ms |
| event 281 alone (360 s curve) | 79 ms | 1 ms |
| events 2132/2133, 2145/2146 alone (reel placements with curves) | 328 ms | 3 ms |
| the sound of 281 (`kt-cli render`, 138 MB) and of 2132–2146 (44 MB) | — | byte-identical to the former binary |

Per edit, as the player builds today (2 audio + 2–3 video builds, rows 110, 111, 117, 150): ~2 × 400
+ 3 × 19 ≈ 860 ms of builds before, ~2 × 12 + 3 × 19 ≈ 80 ms after, of which one video build (19 ms)
still on the UI thread (row 150). The video build was never quadratic on this project (four
transitions, six placements); what it costs is `edit-without-rebuild`'s to remove. `kt-cli probe`
now prints what each build walked (the `kt_time::cost` rows) under its time.

## The class walked

Every builder and per-build pass, the size variables of its loops, and what was done. "Linear" means
the guard holds it.

| pass | loop × inner work (sizes) | verdict |
|---|---|---|
| `sign_knots` (audio build, per event with a signed curve) | knots × segment list (K²) | **fixed** D1, t2060/t2061/t2063 |
| `span_over` (prefetch window, mirror axis, `clip_warp_of`) | zero crossings × segment list (Z × K) | **fixed** D1, t2060 |
| `rate_profile_keys` (every `from_curves_rev`: both builds, `clip_warp`, the timeline's maps) | 5 ms samples × keys walked (S × K) | **fixed** by `curve_value` halving, t2063 |
| `clip_head_zoom` (video build, per video event and per placement) | probes (33 + key ticks) × curve walk (K²); × cameras per probe | **fixed** for the walk (K log K); the camera scan per probe stays (cameras are few) |
| transitions in `AudioRenderer::build_reel` / `VideoRenderer::build_reel` | transitions × (`joint`/`overlap_pair` scans + `iter_mut().find` of engine events) (T × C, ~6 scans each) | **fixed** D5, t2062 |
| `Project::validate` → `check_transition_full` | transitions × clip scans (T × C) | **fixed** D5, t2062 (the former: 4480× at ×16) |
| `reconcile_reel` (every batch) | transitions × clip scans | **fixed** (by id) |
| reel revision per placement (both builds, `nest_scales`, `gen_sources`, the timeline's `reel_keys`) | placements × reel text (P × M), × nesting depth | **fixed** D4, t2064/t2065 |
| `hash_reel` recursion | child text per placement | **fixed** D4 |
| `morph_pass` → `windows_on` → `forbidden_zones` (cutting layers) | targets × layers × (T + C) | **fixed** D5, t2066 |
| `resolve_derived` / `resolve_one` (links, formulas) | chain depth × reel copy; samples × leader scan (S × C) | **fixed** D5, t2067 |
| the timeline's transition draw and hit tests (per UI frame) | transitions × 4–5 clip scans (T × C) | **fixed** (`Indexed::over` the per-state `EventIndex`, no allocation per frame) |
| the app's names cache (per revision) | audio events × all events for the A/V pair; overlaps × transitions | **fixed** (sets) |
| the app's `reel_keys` (per revision) | placements × reel hash + placements × clip find | **fixed** (scope, map) |
| `EventIndex::of` (per revision) | events × media list (`media_duration`) | **fixed** (map) |
| `effective_clips` / `inline_pass` | placements × reel events — its OUTPUT | linear in its output; declared bound (D3) |
| `split_predecessors`, `build_lane_covers`, `overlapping_clips`, `build_index`, `scene_hash`, `compute_preroll_bursts` | hashed by (lane, end) / one sweep / per clip | linear (t2062) |
| `sync_fx` / `sync_fx_since` | by the `same` map; stage keys × keys × C | linear; the sidechain branch is bounded by the few key tracks |
| `nests` (sub-renderers) | deduplicated by (reel, revision) | linear; a reel under several parents is built once per parent |
| `track_gain`, `silent_lane`, stage ordering | per clip × tracks; stages² | bounded by the track count |
| `Project::sequence` per event | events × reels | counted (`reel_passes`); bounded while reels are few — row 157 |
| overlapped transitions in validation and reconcile | overlapped transitions × events | row 156 |
| the project file write and read | per element | linear (t2062–t2067) |
| `gen_sources` | resolves the root twice and every reel twice | linear; part of row 155 |
| resolutions of a reel per build | ~5 (`resolve_reel` in audio build, video build, `nest_scales`, `gen_sources` ×2) | linear each; row 155, `edit-without-rebuild` |
| prefetch: `wanted_audio_bursts_for` | per pass: every event; a warped active event lays its profile out (`span_over`, `chain_source_used` uncached; the render keeps `src_ends`) | linear per pass — O(P) per active warped event; listed, not fixed |
| the video plan per frame (`layers_at`, `build_plan`, `stream_advance`; prefetch × 60 frames) | every video event per frame | row 153 |
| `nest_coverage` → `reel_coverage` (4 Hz) | placements × inner events, quadratic `subtract` | row 154 |
| the LFO phase (`param_integral`, per sample) | samples × keys | row 152 |

## Found, not fixed

Rows 152–157 of `docs/found-defects.md`: the LFO's key walk per sample (render path, plugin API),
the video plan's walk of every event per frame, the reel coverage bars (placements × inner events at
4 Hz), the ~5 resolutions of each reel per build (for `edit-without-rebuild`), overlapped transitions'
zone scan, `Project::sequence` as a scan of the reels.

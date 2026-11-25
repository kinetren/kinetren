# Design: point-tracker

> DESIGN ONLY, revision 3 (2026-11-25), base `fc60a5d9`: the owner's A20 read again (`proposal.md`), the design checked against the
> merged tree. Implementation-ready; what may start before its dependencies is in D11.

## D0. What revision 2 said and the code contradicts

1. **"Its Geometry `pos_x/pos_y` links".** A link has no geometry at either end: `links.rs:46–72` knows `gen`, `clip`, `mask`, `fx`, `fx.mask`; `resolve_derived` never reads
   `geometry_auto` (found-defects 838). Attach and Stabilize need the owner `geo` on both ends: task 1.2, with row 838's fix or after it.
2. **"The apply gestures write the mapping as the link's expression".** An expression sees one value; a canvas position under a zoom and a turn needs x and y together. The channels are given in the
   CANVAS (D2), computed by the placement; the expression keeps a constant offset only.
3. **The placement is in kt-fx** (`geom.rs:145`), links resolve in kt-model (kt-fx depends on it). The affine placement of the Geometry node (frame, rule, zoom, turn, anchor) moves to
   `kt_model::placement` (numbers only), kt-fx calls it: one arithmetic (R.64's rule). The source's size at tracking is kept in the tracker (`view`), for the model knows no source sizes.
4. **"The mask's transform rows link".** A rectangle and an ellipse have `cx, cy, w, h, rot`; a pen mask has vertex rows only, and a homography moves vertices by two numbers each. `Mask.follow` (D5).
5. **Sample keys by "material frame number (pts / frame period)"**: variable-rate files have none. The key is the material tick on the project's frame grid, the one `clip_source_pts` reads, so a
   sample stands for exactly the frame the plan shows.
6. **`BTreeMap` in the file as objects**: serde's pretty form would be a line per number; a second of samples is one string (D6).
7. **"Head controls"**: the corner row of the preview (`preview-canvas-and-gizmos`).
8. `clip_slice` copies the event's fields: a split gives both halves the tracker by itself; a pasted event brings it; a link into it is re-hung as links to a copied event are today.

## Context

Read on `b1b04b6b`:

- **Links** (`src/libs/kt-model/src/links.rs`, R.61): `CurveSource::Link { path, expr }`, a path
  `"<event>/<owner>/<param>"`, owners from `ParamOwner` (`src/libs/kt-model/src/published.rs`: FxSlot,
  Generator, ClipAttr, Transition, Camera, Geometry, Mask…). Between events the denominator is the
  timeline tick (`clip_timeline_at` / `clip_material_at`); a link does not cross a reel boundary; cycles
  are refused; derived sources resolve in `morph_pass` in topological order; curves are compiled once per
  revision (compiled-curves).
- **Masks** on the preview (shape-masks): vertices by stable id, `Mask.auto` curves in the owner's
  material domain; `ParamOwner::Mask`.
- **Corner Pin** (F-FX-004) and effect centres on the preview (F-UI-009) are numeric pairs.
- **Offline sources** (invariant 5) and the scheduler's `Background` class; the reel strip/poster jobs as
  the pattern for an offline frame job.

## Goals / Non-Goals

**Goals:** AE-class point tracking and Mocha-class planar tracking, both sub-pixel with a confidence; correctable sessions; one
address for every consumer; deterministic samples.

**Non-Goals:** 3D camera solve, AI object tracking (a matte, not a
transform — F-MSK-005), automatic reframe for formats (a consumer later: `frame-format-variants`).

## D1. The model

```text
ClipEvent.trackers: Vec<Tracker>                      // empty = not written
Tracker { id: TrackerId(u32), name, view: (f64, f64) /* the source's view when tracked */, points: Vec<TrackPoint>,
          threshold: u8 /* conf % */ }
TrackPoint { id: u8, name, feature: Rect, search: Rect, attach: (f64, f64),
             samples: BTreeMap<Tick /* material, on the project grid */, Sample> }
Sample { x: f64, y: f64, conf: u8, manual: bool }     // x, y in source pixels of the event's media
```

Samples are keyed by the material frame number of the event's media (`source pts / frame period`), so
split, trim, move, reverse and speed need no edit (R.33's representation rule). A split gives both halves
the tracker (the same samples; ids of trackers are per event). A morpher source swap does not carry
trackers (they describe the original material). `validate`: ≤ 4 points, rects inside the source frame,
search ⊇ feature.

## D2. The address

`ParamOwner::Tracker { id }` in a path as `trk<id>`. Channels per point: `p<k>.x`, `p<k>.y` — in the CANVAS (−1..1 of `pos_x/pos_y`'s space, the event's own placement at that tick
applied), `p<k>.sx`, `p<k>.sy` — source fractions, `p<k>.conf`. With two points `rot` (degrees from the reference frame) and `scale`; with four `c<k>.x/.y` in 0..1 of the canvas (Corner Pin's rows). For Stabilize
`inv.x`, `inv.y`, `inv.rot`, `inv.zoom`: the geometry rows that hold the feature where it stood at the reference frame, over the event's own base geometry.
Between samples linear; outside the nearest; below the threshold the last good sample (extrapolation is an option of the gesture, off).

## D3. The matcher

Per run: an offline source of the event's media opened at the native size; for each frame in the run's
direction: luma (integer Rec.709) → a 3-level pyramid (2× box) → ZNCC of the feature template over the
search box, coarse to fine (search ±r at the top level, ±2 px below), sub-pixel by fitting a parabola
through the peak and its 4-neighbours; confidence = peak ZNCC × 100. Template adaptation: when conf ≥ 90,
the template becomes 7/8 old + 1/8 new (integer); below, it is kept. The search box moves with the
predicted position (constant velocity from the last two samples). All arithmetic in f64 with the project
math; the luma and the pyramid are integer — samples are byte-identical across machines.

Cost: a 64×64 feature in a 160×160 search at three levels ≈ 1.5 M multiply-adds per point per frame —
milliseconds; the decode dominates. A run reads frames sequentially (one span; the decode-request-storm
rule of reading forward), backward runs read GOP-sized spans backward.

## D4. The session

- Start: right click on the preview → "Track point" puts a point at the click with default boxes; up to
  four.
- Head controls (Q2 a): ◀◀ (backward continuous), ◀| (one frame back), |▶, ▶▶, stop (Esc), threshold.
  A run starts at the playhead's material frame, writes samples as it goes (one `SetTrackers` command at
  the end = one undo step; the progress shows live from the job's channel), stops at a low confidence
  (the stop frame is shown in the error style with its confidence), at the event's material edge, on
  Esc, on Play.
- Correction: dragging a point at a frame writes a `manual` sample; tracking from there continues.
- The preview draws: the boxes at the current frame, the path of samples ±2 s as one mesh (cached per
  revision), low-confidence samples red.

## D5. Apply gestures (write links)

| Gesture | Writes |
|---|---|
| Attach event… (choose an event) | its Geometry `pos_x`, `pos_y` (+ `rot`, `zoom` with two points) linked to the channels, `value + offset` (`value * factor`) keeping it where it stands now |
| Effect centre (choose a slot) | the slot's `_x`/`_y` pair links, mapped into the slot's coordinate space |
| Corner pin | adds a Corner Pin slot (or uses the selected) with its four corners linked to c1..c4 |
| Mask follows (choose a mask) | `Mask.follow: Option<Follow { event, tracker, ref_tick }>`: its points are carried by the tracker's motion since `ref_tick` (shift; turn and scale with two points; the homography of a plane), over its own curves, when `ClipMaskPlan` evaluates the mask |
| Stabilize | the tracked event's own Geometry rows linked to `inv.*` |

Each is one undo step; the links are ordinary links (visible in the curve editor, removable).

## D6. Format and OTIO

`.kt`: the next free version by content (`has_trackers()`); a second of samples is one string per
point (`"2700000 3000: 0.41321 0.27710 96, …"`), deterministic and git-friendly (R.70's line rule).
OTIO: trackers in the clip's `metadata["kt"]`; linked parameters are effect parameters there already; a
foreign editor sees static values (a row in "What OTIO export loses").

## D7. Idle

A tracker does nothing until a run; at rest the path mesh is cached; a link reading a tracker is compiled
once per revision (compiled-curves), the per-frame cost is a binary search in the samples.

## D8. Class walk — "a result that must be addressable, not buried in its consumer"

| Analysis | Today's home | Addressable? |
|---|---|---|
| sync offsets (F-SPD-004) | sidecar | applied as moves, not linked — fine (a one-time result) |
| loudness (LUFS) | sidecar, shown | not a property — could become a link source later (a level follows loudness) |
| peak pyramid | sidecar | read by meters/waveforms; not linkable — out of scope |
| tracker | new | yes, `ParamOwner::Tracker` |
| the planar tracker (D9) / a stabilizer (F-MSK-006) | this change / later | same owner kind, other params |

## D9. The planar kind (revision 2, the owner's A20)

```text
Tracker { …, kind: TrackerKind }       // Points (D1) | Plane
Plane { quad: [(f64, f64); 4] /* reference corners, source px */, ref_frame: i64,
        ignore: Option<MaskId> /* a mask of the event: the area left out */,
        samples: BTreeMap<i64, PlaneSample> }
PlaneSample { h: [f64; 8] /* reference → frame, h33 = 1 */, conf: u8, manual: bool }
```

Matcher per frame, in the run's direction (all integer luma, the project math, byte-identical across machines):
1. predict H from the last two samples (constant velocity in the homography's parameters);
2. corners (Harris on the integer luma, the strongest 200 inside the warped quad minus the ignore mask, a fixed
   tie-break by position) are matched by the point matcher's ZNCC (D3) between the previous and this frame;
3. a robust fit: RANSAC with a fixed seed derived from the frame number (deterministic), 4-point DLT, then an
   iteratively reweighted least squares (Huber) over the inliers;
4. refinement on the whole region: an inverse-compositional Lucas–Kanade on the 3-level pyramid minimising the
   luma difference between the reference template (the region at `ref_frame`, adapted at conf ≥ 90 as in D3) and
   the warped frame, masked by the ignore mask; at most 20 iterations per level;
5. confidence = the ZNCC of the warped region against the template × 100.
A manual correction drags one corner at a frame; that sample's H is refitted from the four corners and the run
continues from it.

Channels (`ParamOwner::Tracker`): `c1..c4.x/.y` (the quad through H), `x`, `y` (its centre), `angle`, `scale`,
`h0..h7`, `conf`. Apply gestures: Corner pin (c1..c4), Attach event… (a Corner Pin on the attached event's
picture fitted to the quad — full perspective), Effect centre (centre), Mask follows (`Mask.follow`, D5), Stabilize (the inverse homography onto the
tracked event, as a corner pin of its own picture).

## D10. Defaults — the most convenient and exact (the owner's A20)

| Setting | Default | Why |
|---|---|---|
| point feature box | 1/24 of the shorter side, 24–96 px, centred on the click | big enough to be unique, small enough to be one feature |
| point search box | 3× the feature, moved by the predicted velocity | follows fast motion without a manual box |
| plane | a rectangle drag or four clicks; the quad shown with its grid | Mocha's spline replaced by a quad: a plane is what is tracked |
| "Track" (one button) | both directions from the frame under the playhead to the event's edges | one gesture for the usual case; the four direction buttons stay |
| resolution | the native-size frame, never the preview's | exact |
| sub-pixel | on (parabola for points, LK refinement for the plane) | exact |
| stop threshold | 60 % points, 70 % plane | a plane's ZNCC over an area is higher for the same quality |
| template adaptation | at conf ≥ 90, 1/8 per frame | holds through slow light changes, not through occlusions |

## D11. What may start before its dependencies

| Part | Needs | May start |
|---|---|---|
| 2.1 and 2.3: `kt_media::tracker` (pyramid, ZNCC, sub-pixel, the plane's fit) over decoded frames, with its fixtures (t3520, t3528) | nothing: a new file, no model | **now** |
| 1.2a: `kt_model::placement` moved from kt-fx (a pure refactor, t-placement green) | nothing | **now** |
| 1.2b: the link owner `geo` both ways | row 838's answer | with it (fx-graph) |
| 1.1, 1.3: the model | `shared-events` | after it |
| 3, 4: gestures, the preview | `preview-canvas-and-gizmos` (the corner row, handles) | after it |
| `Mask.follow` | MASKS free (`magic-wand-mask` writes `mask.rs` too) | not beside it; it needs nothing of it |

## Rejected

- **A tracker inside a mask** (Premiere/FCP) — one consumer only (F-MSK-005).
- **Timeline-time samples** — every trim would drift them.
- **An ML tracker** — a matte, not a transform; weights and runtime are a separate decision (the owner: a neural
  one later).
- **Mocha's spline as the planar region** — a quad plus an ignore mask covers it; splines are the masks' job.

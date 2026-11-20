# Design: point-tracker

> DESIGN ONLY, revision 1 (2026-11-20), base `b1b04b6b`. Q1–Q2 in `proposal.md`; the defaults follow the
> recommended options.

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

**Goals:** AE-class point tracking with sub-pixel accuracy and confidence; correctable sessions; one
address for every consumer; deterministic samples.

**Non-Goals:** planar tracking (Q1; a later kind), 3D camera solve, AI object tracking (a matte, not a
transform — F-MSK-005), automatic reframe for formats (a consumer later: `frame-format-variants`).

## D1. The model

```text
ClipEvent.trackers: Vec<Tracker>                      // empty = not written
Tracker { id: TrackerId(u32), name, points: Vec<TrackPoint>, threshold: u8 /* conf % */ }
TrackPoint { id: u8, name, feature: Rect, search: Rect, attach: (f64, f64),
             samples: BTreeMap<i64 /* material frame */, Sample> }
Sample { x: f64, y: f64, conf: u8, manual: bool }     // x, y in source pixels of the event's media
```

Samples are keyed by the material frame number of the event's media (`source pts / frame period`), so
split, trim, move, reverse and speed need no edit (R.33's representation rule). A split gives both halves
the tracker (the same samples; ids of trackers are per event). A morpher source swap does not carry
trackers (they describe the original material). `validate`: ≤ 4 points, rects inside the source frame,
search ⊇ feature.

## D2. The address

`ParamOwner::Tracker { id }`, params `p<k>.x`, `p<k>.y`, `p<k>.conf` per point; with two points also
`angle` (degrees of p1→p2 relative to the reference frame) and `scale` (|p1p2| / reference); with four
`c<k>.x/.y` = the points (for Corner Pin). Values are in the event's SOURCE pixels; a consumer on another
event gets them mapped to the canvas through the tracked event's Geometry at that tick
(`expr` space: the link reads `src`, the apply gestures write the mapping as the link's expression — no
hidden transform). Between material frames the value is linear; outside the sampled range the nearest
sample (the link rule's extreme value). A frame with confidence below the threshold is a gap: the value
holds the last good sample (AE "extrapolate motion" is an option of the apply gesture, off by default).

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
| Attach event… (choose an event) | its Geometry `pos_x/pos_y` (+ `rotation`, `zoom` with two points) links to the tracker with an offset expression that keeps the event where it stands at the current frame |
| Effect centre (choose a slot) | the slot's `_x`/`_y` pair links, mapped into the slot's coordinate space |
| Corner pin | adds a Corner Pin slot (or uses the selected) with its four corners linked to c1..c4 |
| Mask follows (choose a mask) | the mask's transform rows link (translate; rotate/scale with two points) |
| Stabilize | the tracked event's own Geometry links to the inverse (pos = −(p − p_ref), rotation = −angle) |

Each is one undo step; the links are ordinary links (visible in the curve editor, removable).

## D6. Format and OTIO

`.kt`: the next free version by content (`has_trackers()`); samples are written one line per second of
material frames (`{"f0":…,"s":[[x,y,c],…]}`), deterministic and git-friendly (R.70's line rule).
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
| a future planar tracker / stabilizer (F-MSK-006) | — | same owner kind, other params |

## Rejected

- **A tracker inside a mask** (Premiere/FCP) — one consumer only (F-MSK-005).
- **Timeline-time samples** — every trim would drift them.
- **An ML tracker** — a matte, not a transform; weights and runtime are a separate decision.

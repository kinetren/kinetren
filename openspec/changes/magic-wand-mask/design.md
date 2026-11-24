# Design: magic-wand-mask

> DESIGN ONLY, revision 2 (2026-11-24), base `0c2a4065`.

## D0. What revision 1 said and the code contradicts

1. **A raster coverage with a soft ramp, a `grow` row and its own chamfer.** The rasterizer (`kt-fx/src/mask.rs:173`
   `one_mask`) works on a POLYLINE: a signed distance to it gives feather, falloff and expansion in mask px. A wand
   that made its own soft raster would have had a second feather and expansion beside the existing rows. The wand
   gives CONTOURS instead; `soft` and `grow` are gone (feather and expansion exist).
2. **"The matte view (the existing mask overlay)".** There is none. The scenario reads the outline; a matte is seen by
   the Viewer's Matte of `chroma-key-upgrade` when that is in.
3. **Seeds mapped through the inverse of the Geometry node.** Every mask's coordinates are already those of the source
   frame (`maskctl::mask_view`, `View::screen_to_ref`); seeds are the same. Nothing to invert.
4. **"The bytes are in the frames cache."** Not for an event with a stream stage (its ordinary frame is never ordered,
   `leaf_keys`, `video.rs:2050`) nor for a composition instance (no bytes at all). D3.
5. **t3536 needed `point-tracker`**, which is built after this. It is a keyed seed now; the link is `point-tracker`'s own test.

## D1. The shape

```text
MaskShape::Wand { seeds: Vec<Seed>, tol: f64, #[serde(default)] range: bool, space: WandSpace, fill: bool }
Seed { id: u32, x: f64, y: f64, #[serde(default)] sub: bool }        // normalized, COORD_LIM as a vertex
WandSpace = Rgb (default) | Luma | Chroma
```
Rows: `tol` (0..100, default 12, step 0.5), `s<id>.x`, `s<id>.y`. `parse_vertex_row`, `is_shape_row`, `shape_rows`,
`write_gesture`, `key_path` learn the `s<id>` rows as they know `v<id>`. `range`, `space`, `fill` are plain fields.
`check_masks`: at least one add seed (`NoSeed`), no two seeds of one id. `kind_tag` "wand". `cubics` gives the empty list.
`check_track_mask_list` (`project.rs:8267`): `WandOnTrack` (the track's composite is on the device). The seed's colour is not stored: it is
read at the seed every frame, so a change of light moves with it.

## D2. From pixels to contours (`kt_fx::wand`)

`contours(frame, shape) -> Contours` (closed polylines in normalized coordinates, holes wound the other way):
1. box mean to at most 960 wide (integer, the scopes' rule); a frame smaller than that as it is;
2. the colour of a seed: the mean of 3×3; the distance of a pixel to it: RGB — the largest channel difference; Luma —
   `|ΔY|`; Chroma — `max(|ΔCb|, |ΔCr|)` (Rec.709 integers); in hundredths of 255, compared with `tol`;
3. per add seed: a 4-connected scanline fill over `distance ≤ tol` from the seed (or, `range`, every such pixel); the
   union; minus the same of the subtract seeds;
4. `fill`: holes (the complement not reachable from the border) join the region; without it, holes and islands under 16
   pixels are dropped too (sensor noise is not a shape);
5. the field `tol − distance` (its sign forced by membership, at least half a unit), marching squares with linear
   interpolation: the contour runs where the colour crosses the tolerance, between pixels. This is the anti-aliased
   edge; a saddle takes the mean's side.
Integers until step 5, IEEE-exact operations there. Same bytes on every platform (t3537).

`rasterize` draws a wand as it draws any mask: `polyline` becomes `contours_of(m, sp, step, pixels) -> Vec<Vec<Pt>>`
(one contour for the old shapes, the same numbers), `crossings` and the distance pass walk all of them, the winding is
non-zero as now (a hole's opposite direction cancels). t1269's frozen bytes do not move.

## D3. The pixels

- `MaskOp` gains `pixels: Option<WandPixels>`, set only when its list holds a wand: `{ frame: Arc<VideoFrame>, leaf: Leaf,
  id: [u64; 2] }`. Its `Debug` prints the `leaf` only: `moment_key.rs:119` hashes `MaskOp` by Debug, and the durable key
  needs the file's identity, not the process's serial. `id` (content serials) keys the memory memos.
- `layer_src` (`video.rs:2530`) has the leaf already and hands it to `ClipMaskPlan::ops`; frame blending hands the blended
  frame with both leaves.
- **A stream event** (Datamosh and its kind): the owner's choice is the SOURCE picture, so `leaf_keys` also orders the
  event's frame at its decode quality when its masks hold a wand, and `layer_src` reads it beside the stream's picture.
  No frame yet: the composite is incomplete, as for any missing frame. The order is the read (invariant 4).
- **The stream's own inputs** (`ops_for`): the frame of the position being walked, which the walk decodes anyway.
- **A second read of a graph** (`LayerSrc.reads`): that read's own frame.
- **A composition instance**: no bytes; a wand in its list is skipped as if absent, its row in the list says "a
  composition has no picture to read here", the menu's item is refused on it. Not refused by the model: a media replacement
  or a morpher's source may make an event an instance, and a batch must not fail for that.
- **A title or a generator**: frames of its source (processor bytes) — works.
- **Device decoding** (`device-decode-sources` D2): "surfaces never meet the caches", so the bytes are host bytes as today.
- What "the source picture" costs (Q35 a): a wand on a slot after a strong colour effect selects by the colour before it.
  The hint of the wand in an effect's list says "reads the source picture".

## D4. Memos and cost

- `wand::contours` behind a memo of 16 keyed by (`id`, the resolved shape's bits, the frame size): `Arc<Contours>`. A paused
  frame, a static seed and the overlay share one entry (polylines: no ledger account).
- `rasterize_cached`'s key gains `id` when the list holds a wand.
- Cost at 960 wide: box mean of a 1080p frame ≈ 1 ms, fill and field ≈ 2–4 ms on one core, the rasterizer as before by
  the contour's length. Measured by t3542's companion bench line; above 8 ms a frame the analysis width falls to 640
  (a constant, not a setting).
- The plan's build does it on the compose thread; the prefetch is not touched. At rest nothing.

## D5. The interface

- `MaskTool::Wand` in the three lists that name tools: the preview's menu (`app.rs:12185`), the mask list's buttons
  (`masks.rs:95`, not for a track), `MaskTool` itself.
- With the tool armed the picture is the tool's, as for the others: a click makes the mask (first) or a seed (then);
  Alt+click is caught by window managers (geomctl's lesson), so **Ctrl+click** makes a subtract seed; Backspace drops the
  last seed (`mask_keys`); Esc ends.
- A drag that starts on a seed within `GRAB`: without moving off the seed for 4 px it is a seed move; the tolerance is
  dragged on a ring around the selected seed, its radius a fixed map of `tol` (8 + 0.9·tol screen px). One command per
  gesture (`SetMasks`, coalesced), keys by the rule of rows.
- The overlay draws the contours from the memo through `View`; a mask whose contours are not in the memo yet (the frame
  is being composed) draws its seeds only. Probes: `mask.contour.<i>` (as today), `mask.seed.<id>`, `mask.tol`.
- The list row of a wand: tolerance in the number row, then "Every matching pixel", the three spaces as words, "Fill holes".
- Nothing in the interface waits: contours come from the compose thread's memo.

## D6. Format, OTIO

`Project::has_wand_masks()` → the next free version by content (`project_file::version_for`). OTIO: the planned row.

## D7. Class walk — "a mask's coverage that depends on pixels"

| Place | Depends only on the shape today? | Here |
|---|---|---|
| `rasterize_cached` memo | yes | + `id` |
| the GPU's coverage upload | keyed by the coverage's hash | fine |
| the plan key, the durable key | `MaskOp` by Debug | `leaf` in Debug |
| the present and read-back memos | by plan key | fine |
| the disk frames | by the durable key | fine |
| a reel's content hash | the model's `Debug` | a wand inside a composition is an event's own: fine |
| export | per frame | fine, native-size source |
| strips, posters, picker cells | `frame_plan` | fine |
| the morpher's baked events, copy-paste, split | masks travel | fine; seeds in the material's domain |
| a replaced source under a morpher layer | keys move with the anchor (R.43 item 300) | `s<id>` rows are curves like any |
| masks of `shared-events` | lists by value | no change |

## Rejected

- **A soft raster coverage** (revision 1) — a second feather.
- **A fill on the device** — contiguity is serial.
- **A stored seed colour**, **temporal propagation** — as in revision 1.
- **Alt+click** — taken by window managers.
- **Refusing a wand on an instance in the model** — D3.

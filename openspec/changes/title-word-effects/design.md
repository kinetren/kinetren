# Design: title-word-effects

> DESIGN ONLY, revision 3 (2026-11-25), base `fc60a5d9`: the owner's B8 (D7), checked against the merged tree. Implementation-ready; it needs nothing of
> `magic-wand-mask` (D8).

## D0. What revision 2 said and the code contradicts

1. **"Animators by characters/words/lines (F-TXT-006)".** There are none: a title has `reveal`, `reveal_soft`, `offset_x/y` (clusters, whole block). The coverage follows those.
2. **"A raster-made shape exists in the design of `magic-wand-mask`".** Its revision 2 gives CONTOURS to the one polyline rasterizer. Contours would be wrong here: an anti-aliased edge pixel of
   alpha `a` split by a coverage `c ≈ a` leaves `a(1 − c)` of the sharp letter outside the effect, a visible outline around the blurred word. A letter's pixel belongs to ONE group whole: an
   id map (D1), binary.
3. **"At the plan's raster".** The title's frame is made by its source at the layer's decode size, before the Geometry node. The map is made at that size and read through the mask space's
   inverse (`MaskSpace`), nearest.
4. **`MaskOpKind::Isolate { from }` "with two passes the GPU has"**: the matte pass and the mix exist; in a graph arm the op is of the line (`ClipMaskPlan::of_line`). Both are taken.
5. **Counters**: a shown value is made from the moment (`text-counter-source`): `from..to` are offsets of the shown string; `every` is refused there (the text changes every frame).
6. The mask list's coverage is memoized by raster (`rasterize_cached`); this one by (generator state, pts, size) in kt-media, one entry per group set.

## Context

- A title is `GeneratorKind::Text` with values and curves on the event (`kt-model/src/genmedia.rs`), rasterized by
  `kt-text` (harfrust shaping, ab_glyph) with a per-frame layout: crawl offsets, animators by characters/words/lines
  (F-TXT-006), counters (text-counter-source).
- Slot masks (shape-masks): `MaskOpKind::Mix { from }` — the slot's input at chain position `from` is the dry side,
  its output the wet, mixed per pixel by the coverage (`kt-fx/src/mask.rs`, the GPU's mask pass). A raster-made
  shape exists in the design of `magic-wand-mask` (`MaskShape::Wand`).

## D1. The text-range shape

`MaskShape::TextRange { spans: Vec<TextSpan> }`, `TextSpan { from, to: u32 /* chars */, text: String, every: bool }`; legal on the picture masks and slot masks of a title event, refused elsewhere, named.
`kt_text::id_map(&TextParams, groups: &[Vec<Range>], w, h) -> Vec<u8>`: the same layout as `render_rgba`; a pixel's value is the group of the glyph with the most ink there (fill, outline and shadow are the glyph's, the plate is nobody's), 0 —
none. Ties go to the later glyph. `kt_media::generator::title_groups(state, pts, w, h, masks)` keeps the last few maps. The mask's coverage is the indicator of its group; `feather` and `expansion` by the distance transform kt-text has for
the outline; modes and opacity as for any shape.

## D2. The isolate mode

`MaskMix::{Mix (today), Isolate}` on a slot's mask list. `Isolate`: before the slot, its input is multiplied by the
coverage (premultiplied, transparent outside); the slot runs; after it, the result is composited `over` the input
multiplied by (1 − coverage). For a blur this blurs the word's glyphs against transparency — the neighbours never
enter. The plan expresses it as `MaskOpKind::Isolate { from }` with two passes the GPU already has (multiply by a
mask, `over`).

## D3. Edits keep the range

The title's text edit command (`SetClipGen` of `text`) carries the edit as a diff (the field knows the replaced span);
`apply` moves every TextRange of that event: edits before the range shift it, edits inside grow/shrink `to`, a
deletion that covers it leaves `from == to` (an empty mask, listed as "text: (deleted)"). `every` re-finds `text` in
the new string. A counter title's value changes every frame: a range on it is by offsets of the shown string.

## D4. The interface

The title form's text field: select characters → right click → "Mask this text" (and "… every occurrence") → a
picture mask on the event's selected slot (or the event's picture when no slot is selected) in Isolate mode, named
"text: “SALE”", selected in the mask list. The mask list draws its row like any mask; the preview outlines the
range's glyph boxes while the mask is selected.

## D7. Many words (the owner's B8)

- One mask, many spans: "Add to this mask" (Ctrl+select a word, right click) appends a span to the selected slot's
  text-range mask; "Remove from mask" on a selection removes it. The mask list shows "text: 20 pieces" and expands to
  the list of words.
- One rasterization for all masks of a title per frame: kt-text lays the title out once and writes an R8 id map — each
  glyph's value is the index of the mask group whose spans include it (up to 255 groups; overlapping spans of different
  masks: the later mask wins, named in the mask list). Each slot's coverage is its group's indicator in that map
  (feather and expansion applied per mask). Twenty words in one mask cost the same as one; ten masks cost one layout
  plus ten cheap selects.
- Cost bound: the frame cost of a title with one isolating slot over 20 spans stays within 1.2× of the same slot over
  one span (a test).
- Edits move every span (D3); spans that touch after an edit merge.

## D5. Class walk — "a part of an object that should be an object"

| Object | Part | Addressable today? | Here |
|---|---|---|---|
| title | a word, a line | animators by unit (F-TXT-006), no effects | D1–D2 |
| shape/plate generators | a sub-shape | no | not asked |
| reel instance | a layer inside | public parameters (R.59) | exists |
| audio event | a time range | split + chain | exists |
| subtitles (change) | a word in time (karaoke) | the highlight | same range idea, own change |

## D8. What may start before its dependencies

`magic-wand-mask` shares only the file (`mask.rs`, the `MaskShape` enum): the two do not run side by side, in either order. **Now**: `kt_text::id_map` with t3942/t4246 (kt-text alone), and `Isolate` in the
plan and the executor with an ordinary shape (t3944 with a rectangle). After `shared-events`: the shape, the span edits (1.2), the field.

## D6. Idle

The coverage is made with the title's raster for a shown frame; at rest nothing runs.

## Rejected

- **A second title event for the word** — the owner's point is one object; alignment would break on every edit.
- **A rectangle mask tracked to the word** — slides with proportional fonts and animators; the glyphs are known.

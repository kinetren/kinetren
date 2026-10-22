# kt-text

> The crate's contracts and mandatory tests — `openspec/specs/{generators}/spec.md`; the reasons for the decisions — `design.md` next to it.

The rasterization of titles: a pure function (parameters, W, H) → RGBA bytes.
With no dependencies on our crates or on the GUI: kt-app and kt-cli MUST
get the IDENTICAL frame bytes (invariant 7), therefore drawing a title
inside egui is impossible in principle. The stored representation of the parameters is in
kt-model (`GeneratorKind::Text`); it is converted here by the SINGLE mapping
function in kt-media (do not create a second parser).

## The determinism rule of a title (invariant 11; written BEFORE the code — verdict F-TXT-001)

A title frame is a deterministic function (parameters, W, H). Therefore:

1. **The font of a frame — ONLY the one vendored from `assets/`** (the passport —
   `assets/FONT-MANIFEST.txt`: the release, sha256). System fonts never get into the path
   of a FRAME; the choice of a system family (C4) is a resolve into a CONCRETE file, whose path and sha256 enter the identity of the frame, and
   a miss on another machine is a substitution with the vendored font + a visible notice.
   Replacing/updating a font file is a deliberate regeneration of the title goldens
   with an entry in the manifest.
2. **`harfrust` and `ab_glyph` are pinned as `=x.y.z`** in Cargo.toml (like wgpu:
   "STRICTLY the same version"). Their `cargo update` = a regeneration of the goldens,
   not a routine. If an upgrade of egui raises the transitive version of harfrust —
   two of them LAWFULLY live in the tree: the bytes of the title do not depend on the GUI.
3. **Hinting is not executed**: the unhinted build is vendored, ab_glyph
   ignores the instructions. No "system" text rendering whatsoever.
4. **No sixth library in `kt-ffmpeg/src/loader.rs`** (libass/
   freetype/avfilter drawtext): they can do all this and make the bytes of the frame
   a function of the user's machine (verdict F-TXT-005).
5. **The layout is computed over the FULL line**; no dependence on the
   locale, time, environment. The door F-TXT-006 was opened in R.53 (items 423-424)
   exactly as promised: **there is no time in kt-text and there will not be** — the animation
   arrives as TWO numbers, which kt-media computes from the pts of the material:
   - `reveal` / `reveal_soft` — the fraction of the revealed CLUSTERS and the width of the front.
     The modulation stands in one line in the loop of the silhouette, and at full
     visibility (`reveal >= 1 && reveal_soft <= 0`) the former branch is taken EXACTLY:
     the golden of the title cannot move from the appearance of the animation.
   - `offset` — the shift of the WHOLE block in px. The caller MUST quantize
     it to 1/64 px BEFORE passing (multiples of 1/64 are exactly representable in f64),
     then the mutual geometry of the glyphs between frames is bit-for-bit constant.

   A cluster, not a character and not a glyph: harfrust gives grapheme clusters
   for free, and a `fi` ligature, a base with a diacritic and a ZWJ emoji arrive as one
   unit. The project has no segmenter of its own, and creating one for the sake of
   the animation is a separate subsystem that does not belong here.

   A known limitation: with a SOFT appearance the outline "clicks" in the middle — the EDT is computed over the binarized silhouette. A typewriter does not have this.
6. **`GOLDEN_VIDEO` does not move**: a scene with a title is a separate reference
   `GOLDEN_TITLE_*`. The goldens and the unit tests — only on the vendored
   fonts; the f32 rasterization is the status quo of "linux x86_64", as for the others.

## Structure

- **harfrust — positions** (GPOS kerning, ligatures; Noto Sans legacy
  `kern` tables are NOT present at all — without the shaper the kerning would be zero),
  **ab_glyph — pixels** by GlyphId. The metrics (upem, ascent/descent/gap)
  are taken from ab_glyph from the SAME bytes. The trap of `PxScale`: it is the height
  (ascent−descent), NOT the em — the scale is derived via `units_per_em`
  by a single function, fixed by a metric test (t481).
- All sizes of the parameters are RELATIVE (the point size is a fraction of the frame height,
  tracking/outline/shadow — fractions of the em): a frame is born at any Quality without
  recomputing the parameters; the decode scale step S×Quality is free.
- The output is straight-alpha RGBA8 (the engine's composite multiplies it itself,
  kt-media/src/lib.rs: "premultiplied is not required"); the internal composite of the
  decoration layers (C3) — premultiplied u16 with a single
  un-premultiplication at the output.
- `render_rgba` draws the frame EXACTLY (w, h); `layout_info` gives the boxes
  of the block and of the lines (drag in the preview, the plate, the door F-TXT-006); `uncovered` —
  codepoints without a glyph (a visible answer "there is no glyph" in the window — UI rule 11:
  t100 checks the catalogues, and the title text comes from the user).

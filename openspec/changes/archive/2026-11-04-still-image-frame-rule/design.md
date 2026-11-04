# Design

## Context

See proposal.md, section Why. The facts below were read in the code on 2026-10-28 and are marked as
observed. Assumptions are marked as such.

**Observed: the chain of the defect.**

1. Probe of a still. `src/libs/kt-ffmpeg/src/source_impl.rs:240-245` recognises a still by its
   demuxer (`image2`, `*_pipe`). It reports the real native width and height and the SAR
   (`:274-276`), and `fps: if still { None } else { fps }` (`:285`). `src/libs/kt-media/src/ffmpeg.rs:73-82`
   turns this into `MediaInfo { still: true, fps: None, width, height, sar }`.
2. `SourceFacts::observe` (`src/libs/kt-engine/src/video.rs:766-788`) stores
   `views[media] = info.view_size()` correctly. Then, by the rule "no grid of its own =
   procedural" (`:779-787`), it puts the still into `procedural`.
3. The renderer's lead context (`video.rs:935-953`) does this:
   `view = if nest { reel_raster } else if procedural { None } else { views[addr] }`. The
   comment there says "a PROCEDURAL source (generator, title, placeholder) … has no proportions:
   kind None → frame = canvas, the node is identity (spec generators, R.62)". A still is
   none of those three, but it lands in this branch anyway.
4. `conform_dest(None, …)` returns the canvas for every rule (`src/libs/kt-fx/src/conform.rs:48-52`).
   `clip_base_quality` (`video.rs:591-606`) therefore asks the decoder for a canvas-sized frame,
   so sws stretches the square picture to 16:9. The lead Geometry node gets `dest == out`
   and is skipped as an identity (`src/libs/kt-fx/src/geom.rs:83-86`).
5. The other two readers take `views` directly and never ask `procedural`:
   `Player::source_view` (`src/libs/kt-engine/src/player.rs:409-411`), which feeds the
   on-screen frame (`src/apps/kt-app/src/geomctl.rs:274-279`), and the probe
   (`src/apps/kt-cli/src/main.rs:415`). They show the Fit rectangle, while the render
   stretches. The owner's question "duplicated code?" has this answer: the arithmetic is
   one, but the DECISION about the source's proportions is made in three places, and only
   one of them has the "procedural" branch.

**Observed: who else has `fps: None`.** Generators (`src/libs/kt-media/src/generator.rs:89`,
`still: false`, native = the container's raster, per the generators spec) and the placeholder
(`src/libs/kt-media/src/placeholder.rs:44-54`, `still: true`, 640×360, drawn at the requested
Quality). The engine does not read `MediaInfo::still` anywhere. Its consumers are the kt-media
FFmpeg source (`ffmpeg.rs:122,137`) and `kt-cli showcase` (`src/apps/kt-cli/src/showcase.rs:57`).

**Observed: why the existing tests never saw it.** t718 (`src/libs/kt-engine/tests/conform_rule.rs:57-58`)
builds `SourceFacts` by hand with `procedural` empty. The t796b still fixture is 320×180
(`src/fixtures/gen.sh:41`) on a 320×180 canvas (`src/libs/kt-engine/tests/stills.rs:25`), so
fit equals stretch there.

**Inference, not checked at run time:** the GPU receives the same `lead` through the plan
(`src/libs/kt-gpu/src/engine.rs:415`, `put_lead` ~1954), so both backends show the same defect.
Nothing GPU-specific is wrong. t938 checks this.

## Goals / Non-Goals

**Goals:**
- A still follows its frame rule exactly like a video of the same shape, on both backends,
  in preview, in export and in the frame drawn over the picture.
- The source's proportions are decided once. Every reader asks the same function.
- Generators, titles and the placeholder keep "frame = canvas". Goldens do not move.

**Non-Goals:**
- Changing R.67 timing: a still is still delivered at any tick, has no floor to the
  frame grid and no end of material (`video.rs:299`, `:1002`).
- A new conform rule, or a separate default rule for stills (e.g. "fit for stills, fill for
  video"). Nobody asked for it, and a single project default with a per-event exception is
  the owner's R.62 decision.
- Any kernel change in `kt-fx/geom.rs` ↔ `kt-gpu/geom_fused.wgsl`.

## Decisions

### D1. Two facts, one place: `procedural` (no grid) and `shapeless` (no proportions)
`SourceFacts::observe` keeps `procedural` exactly as now (`fps.is_none()`): the timing
paths read it, and R.67 needs it. It stops being the source of the proportions decision.
The view is stored only for a source that has proportions. "Shapeless" means a source that
DRAWS its frame at the requested quality: generator, title, placeholder. The rule:
`shapeless = has_video && fps.is_none() && !still`. The placeholder sets `still: false`.
The placeholder draws its frame at the requested Quality (`render(label, q.width, q.height)`), so
640×360 is not a shape, and since nothing in the engine reads `still`, this edit does not
change timing. Task 1.1 re-checks that claim with a grep and with t796d before the edit.
- Alternative: a new `MediaInfo` field (`draws_at_quality: bool`). It is explicit, but it is one more
  field on every constructor (synthetic, generator, placeholder, ffmpeg, tests) for a
  distinction that `still` already makes once the placeholder stops lying about being a
  picture. It is kept as the fallback if task 1.1 finds a reader of `placeholder.still`.
- Alternative: check `still` only at the renderer's branch (`else if procedural && !still`).
  Rejected: that is a fourth place that decides, and the overlay and the probe would still
  have their own copies. This defect exists exactly because of copies like that.

### D2. One accessor that every reader calls
`SourceFacts::view_of(addr) -> Option<(f64, f64)>` returns `None` for a shapeless source or
an unknown one. The renderer (`video.rs:942-949`, the non-nest branch), `Player::source_view`
and the probe all call it. The renderer's `else if procedural { None }` branch is deleted.
The nest branch (`reel_raster`, law E) stays where it is, because a reel is not a source of
`SourceFacts`. The probe also uses `reel_raster` for nest clips, through the same small
helper the renderer uses (`lead_view(project, facts, clip, nest)`). Otherwise the probe
would be a second copy for reels too.

### D3. Square fixtures for still against video
`fixtures/gen.sh` gains `still_square.png` (180×180, `testsrc`, one frame, `image2`) and
`square.mp4` (180×180, the same `testsrc`, 1 s). The pattern is the same, so the geometry of
the white rectangle can be compared by a pixel probe. Bytes cannot be compared, because a
lossy codec stands between the two. The existing 320×180 fixtures and their tests are not
touched.

### D4. Scope of the GPU check
No kernel changes, but t938 runs the lead node of a still on both backends, and the
change's scenario names `geom.rs` and `geom_fused.wgsl` by the pair rule. The tolerance class
is the one t713 uses for a vertical video (EXACT at K = 1). There is no new class.

### D5. "As is" is counted in the composite's raster (found while applying)
The live `stills` scenario showed a 360×360 picture under "as is" filling the WHOLE height of
a ½ preview of a 1280×720 project — the same size as fit. `conform_dest` put "as is" at the
view in CANVAS pixels, and the preview canvas is a fraction of the project (640×360), so the
preview drew the picture twice as large as the file gets it. R.64 forbids exactly this: the
frame on screen must be the frame that goes to the file.
- The lead context (`kt_fx::conform::LeadCtx`) carries `raster` — the logical size of the
  composite the layer stands in: the project, or a reel's own raster (a reel without one is
  taken at the project's raster, the same approximation `nest_scales` makes). "As is" scales
  the view by canvas ÷ raster; when the canvas IS the raster the factor is not applied at all,
  so exports at the project size and every golden keep their bytes.
- `conform_geometry_in` uses the same raster for the decode size, so the decoder is asked for
  what the frame shows. The GPU plan key hashes the raster (it moves the frame).
- The overlay asks `Player::raster()` (what is playing: the project or the entered reel), the
  probe the project's settings.
Alternative rejected: scaling the view at every caller. Fit, fill and stretch do not depend on
the view's scale, so the bug would hide in "as is" alone; one field in the one context keeps
the rule in one function.

## Risks / Trade-offs

- [Existing projects with non-matching stills will look different: fit adds margins where the
  still used to be stretched] → This is the requested behaviour, because the old picture contradicted the rule
  shown in the window. The project format is not touched, so there is no import fix. Anyone who
  wants the old look sets "stretch" on the event or the project. The release note names it.
- [The placeholder's `still: false` may break a reader that the grep misses (e.g. a future
  "Loading…" scrim test)] → t796d and t941 cover the placeholder (full frame, no scrim, no
  margins in 4:3). If a reader exists, fall back to the explicit field (D1 alternative).
- [The decode size of a still now follows the rule (fit → 180×180 instead of 320×180)] → It
  is the same arithmetic as for video (`conform_geometry`). The decode step of R.46 still
  multiplies it by the chain's head zoom, so a zoomed still keeps its detail. t937 checks the
  decode sizes.

## Migration Plan

No data migration. Rollback is reverting the commit, and the old stretching comes back.

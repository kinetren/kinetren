---
id: F-MSK-006
title: Stabilization: Warp Stabilizer and relatives
status: candidate
priority: B
cost: XL
category: masks/stabilization
editors: [premiere-26, after-effects-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: no
crates: [kt-media, kt-engine, kt-fx, kt-gpu, kt-model, kt-otio, kt-app]
invariants: [4, 5, 7, 8, 11]
ui_rules: [2, 13, 16, 20]
round: null
point: null
tests: []
depends_on: [F-MSK-005]
checked: 2026-09-07
---

# F-MSK-006 — Stabilization: Warp Stabilizer and relatives

## Essence

Handheld footage shakes. The editor applies one effect to the clip, waits while the
editor runs through the footage frame by frame, and gets a steady image —
the camera either pans smoothly or stands still like on a tripod. The price is paid at
the edges of the frame: to compensate for the shake, the picture is moved inside the frame,
so the frame has to be either cropped or zoomed in. This is one of the few
features whose result is visible on the very first clip, without any explanation — and
one of the few that the editor turns on before color correction.

## Who has it and how

| Editor | Version | Name | How it works |
|---|---|---|---|
| Adobe Premiere | 26 | Warp Stabilizer | REFERENCE, came from AE. An effect on the clip; the analysis runs in the BACKGROUND in two stages ("Analyzing in Background, Step 1 of 2" → "Stabilizing"). `Result`: Smooth Motion (with `Smoothness` in %) or No Motion (Smoothness is ignored). `Method` is a ladder: Position → Position, Scale, Rotation → Perspective → Subspace Warp. `Framing`: Stabilize Only / Stabilize, Crop / Stabilize, Crop, Auto-scale (default) / Stabilize, Synthesize Edges. Plus `Detailed Analysis` and `Rolling Shutter Ripple`. It does NOT combine with `Speed/Duration` on the same clip — the editor refuses with an explicit error |
| After Effects | 26 | Warp Stabilizer VFX + Rolling Shutter Repair | the same effect plus a VFX mode (lets you export the result as a transform and stabilize "in reverse" — pin an object). Rolling shutter is a SEPARATE effect `Rolling Shutter Repair` with `Rolling Shutter Rate` and `Scan Direction` [method Warp/Pixel Motion — NOT VERIFIED] |
| DaVinci Resolve | 21 | Stabilization in Inspector (Edit) + Tracker in Stabilizer mode (Color) | TWO places. On the Edit page — "built in stabilization and lens correction controls for every clip", with the Camera Lock, Zoom, Cropping Ratio, Smooth and Strength controls; modes Perspective / Similarity / Translation [mode names — NOT VERIFIED against the primary source]. On the Color page the same tracker that drives Power Windows is switched to stabilization: "The tracker can also be used to stabilize images". Camera Lock is a separate checkbox, not a value of Smooth: this is exactly "No Motion" |
| Final Cut Pro | 12.3 | Stabilization + Rolling Shutter (Inspector) | `Method`: Automatic (picks by itself) / InertiaCam (for pans and pushes, with a Tripod Mode checkbox) / SmoothCam (three separate controls — Translation Smooth, Rotation Smooth, Scale Smooth). Rolling shutter is a separate block with `Amount`. Analysis is a background task; historically FCP offered to analyze at IMPORT, before editing [whether this is in 12.3 — NOT VERIFIED] |
| VEGAS Pro | 2026 | VEGAS Video Stabilization | Applied **either as Media FX or as Event FX**, and these are different things (see "The trick behind it"). An explicit `Analyze Motion` button, a Fast/Accurate choice, three UI complexity levels (Basic/Professional/Expert). Modes: Smooth motion (with a smoothing factor) and Freeze motion (reference frame). Edges: Transparent / Mirror padding / Repeat / Clamp to edge + Auto Zoom + Additional Rotation/Translation. Optional artificial motion blur. There is NO rolling shutter correction in the documentation. Mocha VEGAS (Boris FX) ships separately with its own stabilization |

## The trick behind it

**1. In Vegas the analysis is attached either to the MEDIA or to the EVENT, and the editor
chooses this before the pass begins.** Media FX runs the analysis over the whole file —
slow, but it "permits event editing post-stabilization": trim the event, cut it, move it — the trajectory stays intact.
Event FX analyzes only the piece that the event takes from the file — fast, but "Changing the event's
start and/or time in the media file makes the analysis invalid". This is exactly the same fork that we have
already passed for automation keyframes (F-AUT-002, key domain = tick of the MEDIA, test t212), except that
Vegas presents it to the user as a choice, while we decided it for them. Neither Premiere,
nor Resolve, nor FCP has this fork in the UI: there the analysis always
belongs to the clip on the timeline, and cutting a stabilized clip
means getting two effects, each with its own future pass.

**2. Premiere FORBIDS Warp Stabilizer together with Speed/Duration on the same
clip** — not "works badly", but refuses with an error. The reason is the order of computation:
the stabilizer builds the trajectory over the FRAMES OF THE SOURCE, while
retiming changes the very set of frames that will reach it; which of the
two is computed first is determined by "how things are written into that
file". Hence the universal forum advice — a nested sequence: it imposes the order
explicitly. The same mechanism explains why stabilization is advised to be placed FIRST
in the chain: any effect before it changes what
the analyzer sees, and a trajectory computed from a graded image
will not match the trajectory from the original.

**3. `Stabilize, Synthesize Edges` breaks the rule "a frame is a function of t".**
The first three Framing modes are a transformation of ONE frame: moved, rotated, zoomed, cropped.
The fourth fills in the lost edges, taking pixels from frames NEIGHBORING IN TIME
(the window width is the `Synthesis Input Range` parameter). So the last item of the dropdown,
indistinguishable from the other three in the UI, turns a per-frame transform into
a temporal operation with a window: a different memory profile, a different cost, different
cache behavior, and ghosting on moving objects. For us this is a direct
consequence: the three modes fit into the existing FX slot, the fourth does not.

## Pros

- **The analysis is DATA, not a picture**, so the second phase is free:
  turning Smoothness up, changing Framing, switching Smooth Motion to No
  Motion can be done instantly, with no repeated pass over the frames. Vegas
  draws this boundary explicitly: "You cannot change the method of the analysis
  without doing the analysis again" — but the smoothing can be changed.
- **The ladder of models is monotonic.** Position → PSR → Perspective →
  Subspace Warp — one control with which the editor trades the strength of the correction
  for the amount of artifacts, without changing the tool or the workflow.
  If you overshoot, you step down a rung instead of redoing the shot.
- **The analysis by the material survives editing** (Vegas Media FX): trim, split,
  move and reordering of tracks do not cost a recalculation, because the trajectory is
  addressed by the tick of the material, not by the position on the timeline.
- **The same core gives two features instead of one.** Motion analysis is
  tracking (F-MSK-005); stabilization = background tracking + compensation,
  "pin an object" = the same tracking with the opposite sign. AE admits this directly,
  exporting the Warp Stabilizer VFX result outward as a transform.
- **The compensation fits the rest of the path**: three of the four
  Framing modes are an ordinary affine/projective transformation of the frame,
  that is, exactly what the GPU path can already do.

## Cons

- **Stabilization ALWAYS costs the edges, and there is no free option.**
  Auto-scale is a zoom inward, that is, a loss of resolution; the stronger the
  smoothing, the stronger the crop. In 4K material on a 1080 timeline there is
  room to spare; in 1080-on-1080 every percent of smoothing is paid for in sharpness.
  Hence in Premiere the slider is called "Crop Less ↔ Smooth More": this
  is not a quality setting, it is a trade-off.
- **Subspace Warp bends straight lines.** Grid deformation cures rolling shutter and
  "jelly", but on scenes with parallax (the foreground moves not the way
  the background does) the analyzer does not know what exactly it is stabilizing, and the result is
  a "floating background" — wobbling that was not in the source. This is the price
  for exactly what makes this model stronger than the previous ones.
- **The analysis is tightly bound to what the analyzer sees.** An effect before
  the stabilizer in the chain changes the input; retiming changes the set of frames;
  nesting changes the order. Instead of subtle handling, Premiere simply forbids
  the pair — and this is more honest than silently computing the wrong thing.
- **The pass is long and opaque.** "Analyzing in Background (Step 1 of
  2)" with no progress bar and no time estimate — the most common complaint about
  the feature over the fifteen years of its existence (see the quotes).
- **Synthesize Edges is expensive in memory and time** and produces ghosts on
  moving objects: it honestly takes pixels from neighboring frames, where the object
  is already somewhere else.
- **Rolling shutter is a separate problem, not a special case of shake.**
  A stabilizer that does not know about the skew of the rows AMPLIFIES the jelly:
  it aligns the geometry of the frame, leaving the intra-frame deformation uncompensated.
  That is why AE has it as a separate effect, FCP has a separate block with `Amount`, and
  Premiere has the `Rolling Shutter Ripple` parameter inside the stabilizer.
  Vegas has no rolling shutter correction in its documentation at all.

## What users say

About the artifacts of the senior compensation model:

> I wouldn't recommend that subspace warp setting if there's bigger
> perspective changes because it can create wobbliness when it doesn't know
> what it should be stabilizing.
> — Tero Ahlfors, Creative COW, 10.12.2014,
> https://creativecow.net/forums/thread/warp-stabilizer-what-am-i-doing-wrongae/

In the same thread — a ready-made recipe for stepping down the ladder of models, which
confirms that it is monotonic and that users really do use it this way:

> changing the mode from Subspace warp to "Position, Scale, Rotation"
> and/or reducing the smoothness to less than 15 percent helps to reduce
> the wobble.
> — Merlin Vandenbossche, Creative COW, 11.12.2014,
> https://creativecow.net/forums/thread/warp-stabilizer-what-am-i-doing-wrongae/

About the opacity of the analysis phase — a complaint that has not been fixed in ten years:

> I have been waiting for hours and I am still getting "Analyzing in
> Background (Step 1 of 2)". Is there any sort of progress bar?
> — Dustin Anstey, Creative COW, 14.09.2015,
> https://creativecow.net/forums/thread/analyzing-in-background-forever/

About the ban on "stabilization + speed" — the answer of a community expert to the question
"why not", directly naming the price of the feature:

> You are using two of the most complicated computing effects possible at
> the same time on the same clip.
> — R Neil Haugen (Legend), Adobe Support Community, 23.01.2024,
> https://community.adobe.com/questions-729/why-can-t-warp-stabilizer-and-speed-be-used-on-the-same-clip-1404724

And "I turned it on and it got worse" — a result that usually stands behind an attempt
to stabilize already-stabilized (steadicam) footage with default settings:

> after applying the Warp stabilizer with the standard settings, the
> footage actually got worse with more movement.
> — Benjamin Gs, Creative COW, 12.10.2013,
> https://creativecow.net/forums/thread/warp-stabilizers-makes-steadycam-footage-worse/

## How it fits Kinotrain

### What already exists

The executor of the second phase is half-written for us; the analysis of the first phase is not written at all. Checked by grep:
`stabiliz|стабилиз|tracker|optical.flow|
rolling.shutter` over `src/` gives not a single hit in the code.

- **The lower rung of the compensation ladder is an existing effect.**
  `FxInstance::Pan { mode, pan_x, pan_y, zoom }`
  (`src/libs/kt-model/src/project.rs`, line 1947), execution —
  `pan_zoom` in `src/libs/kt-fx/src/video.rs` (line 69), modes
  `PanMode::{Fit,Fill,Stretch}` (line 2456 of `project.rs`). This is exactly
  Translation + Scale, that is, the "Position" level and half of "Position,
  Scale, Rotation" — rotation is missing.
- **The upper rung is there too, and that is unexpected.** `FxInstance::Pan3d`
  (`src/libs/kt-model/src/project.rs`, line 2075) places the layer as a plane
  in world coordinates and takes it with a perspective camera — that is, it computes
  a HOMOGRAPHY. This is literally the "Perspective" mode, already written,
  mirrored in WGSL and covered by t187/t184. The caveat is a heavy one: `Pan3d`
  belongs to `FxClass::Video3d` (`project.rs`, lines 2108–2128), and
  Video3d is computed ONLY on the GPU, and `export.rs` (line 403) runs a preflight with
  `ExportError::GpuRequired` before the file is created. A stabilization built on
  `Pan3d` would make the project unexportable on a machine without a GPU — so a separate
  warp effect with a CPU reference is needed.
- **The storage scheme for the analysis result already exists and has already been tested.**
  `ParamCurve` / `AutoKey { t: Tick, v: f64 }`
  (`src/libs/kt-model/src/project.rs`, line 1622), the key domain is the tick
  of the MATERIAL (`check_fx_auto`, line 1635, a comment about the "key hidden by trimming"), test t212. The stabilization trajectory has exactly
  this domain: 4–8 scalars per frame of material. What Vegas puts into the choice
  "Media FX or Event FX" is already decided for us in favor of the material — and decided
  correctly.
- **A background offline pass over the material is an existing class of work, with
  two implementations.** `spawn_index_worker`
  (`src/apps/kt-app/src/util.rs`, line 89) builds the peak pyramid in its own thread
  through a SHORT-LIVED `FfmpegSource::open_indexed`, explicitly "not playback
  decoders". The thumbnail worker (`src/apps/kt-app/src/thumbs.rs`,
  lines 340–420) keeps its OWN pool of sources, reads from the DB, on a miss decodes
  itself and writes back via `put_thumbnail`, plus a negative cache of
  failures. Both go AROUND `MediaWorker`.
- **The place for the result is the sidecar index DB.** `MediaIndexDb`
  (`src/libs/kt-media/src/index.rs`, `SCHEMA_VERSION = 2` on line 15),
  the schema and the invalidation rules are in `docs/media-index.md`. It already holds the
  packet index, the peak pyramid and the thumbnails; the key is `media_id` with a check of
  size/mtime/xxhash.
- **A single composition code and its place in the chain.** `VideoRenderer::frame_plan`
  (`src/libs/kt-engine/src/video.rs`, line 446), FX are computed per layer before the
  composite (`src/libs/kt-engine/src/video_plan.rs`), the key of the post-FX cache is
  `fx_chain_hash` (`video.rs`, line 153).

### What is missing

- **The whole first phase.** There is no feature detector, no optical flow,
  no model estimation from correspondences (RANSAC/LMedS), no trajectory smoothing. The same core is
  needed by F-MSK-005 — it has to be written once, for two consumers,
  and the order "first tracking, then stabilization" suggests itself.
- **Rotation in the compensation.** `Pan` handles shift and zoom; there is no rotation in any
  one of the ten video FX; the "Position, Scale, Rotation" rung is a
  new effect, not a new parameter.
- **A "result of offline analysis" section in the DB schema.** The tables
  `packet_index` / `audio_peaks` / `thumbnails` describe the material as it is;
  there is no field for "motion trajectory computed with such-and-such parameters".
- **Edge filling.** `Pan` with `Fit` gives a black field; there is no mirror/repeat/
  clamp at the edges of the frame, as in Vegas, in any effect.
- **Progress and cancellation of a long background task in the UI.** Thumbnails and peaks
  show placeholders and do not report on themselves; for a twenty-minute
  pass this is not enough — Anstey's complaint is exactly about this.
- **Nested sequences** — the common workaround of other editors
  ("stabilize on a nested sequence") is not available to us, and this is rather a plus: we
  will have to resolve the order conflict honestly, rather than hand it to the editor.

**Separately: video retiming we ALREADY HAVE**, and this changes the picture for the second
"trick". Item 2 of R.37 extended `SlowedReverb.rate` to the picture: `clip_video_warp` (`src/libs/kt-engine/src/video.rs`,
lines 83–110) builds a `VideoWarp` with the same profile as the audio (one domain for both
kinds, otherwise sound and picture would drift apart), and `FrameMode::{Nearest,
Blend}` (`src/libs/kt-model/src/project.rs`, line 2469) decides what
to do with a frame that is absent from the source. So the conflict because of which
Premiere simply REFUSES ("Warp Stabilizer and Speed cannot be
used on the same clip") will be live for us from day one — but solved:
our domain "tick of the material" (t212) removes it by itself. The trajectory is a
function of the MATERIAL, the warp maps the output tick to the material tick, so the
composition "retiming + stabilization" is defined unambiguously and a nested
sequence is not needed. One cost that must be planned for at once: with
`FrameMode::Blend` the output frame is a blend of TWO material frames, each with its
own transformation, so the compensation must be applied BEFORE blending,
per frame of the material, not after. Applying the warp to an already blended
frame is the very same wobble, only created by our own hands.

### What gets in the way

**The main point: this is no longer a special case, but a CLASS of tasks — "offline analysis
of the material".** It is the third feature in a row that needs a pass over the frames outside playback:
tracking (F-MSK-005), stabilization (this one) and, next,
everything that computes per-frame statistics in advance. It is time to acknowledge the class explicitly,
rather than designing each feature as an exception.

- **Invariant 5 in its current wording does not suit this class, and the code already knows it.** The text of the root `CLAUDE.md` says: "All clients
  (playback/prefetch/thumbnails) go through its priority queue
  (Playback > NearPrefetch > FarPrefetch > Thumbnail)". But `Priority::
  Thumbnail` (`src/libs/kt-engine/src/lib.rs`, lines 199–204) in reality
  is NOT enqueued by anyone: the only mention is collecting the span in
  `src/libs/kt-engine/src/worker.rs` (line 236), and the thumbnails live on their own
  sources in `thumbs.rs`. So offline work has de facto already been moved
  out of the queue, and the right answer for stabilization is the same one (its own
  short-lived source, as the index worker has), not a fifth priority level. A tail
  below `Thumbnail` would starve forever, and one placed above it would take the decoder away from playback for tens of minutes. At the same time,
  the wording of invariant 5 should be refined: "the decoders of PLAYBACK are owned by exactly one MediaWorker; offline passes open their own
  short-lived sources".
- **The index DB suits the trajectory — but not on all axes, and this must be decided before, not after.** In favor:
  the key `media_id` with invalidation by size/mtime/xxhash (rule 1 of
  `docs/media-index.md`) — exactly the semantics that are needed: the analysis is a function
  of the material, not of the timeline; a blob-per-section is already used for
  `audio_peaks`; the DB moves together with the project. Against:
  1) **The DB is declared a CACHE** ("loss or corruption is not an error, the DB
  is recreated" — `index.rs`, lines 1–5). A peak is rebuilt in seconds, a twenty-minute analysis is not. So either the data class in the DB stops
  being homogeneous, or the heavy analysis lives alongside it but with a different
  contract. Silently losing and silently recomputing is the worst option: the user's PICTURE would change.
  2) **Bumping `SCHEMA_VERSION`** (rule 2 of `media-index.md`) wipes the DB
  entirely — that is, adding an analysis section throws away from everyone's
  existing peaks, thumbnails and packet indexes. The section
  has to be introduced in a single step, not iteratively.
  3) **The volume depends on the compensation model by three orders of magnitude.** An affine
  trajectory is 6 f32 per frame ≈ 24 bytes, an hour at 24 fps ≈ 2 MB, which is nothing.
  A Subspace Warp grid of 16×9 nodes with 2 f32 each — ≈ 1.1 KB per frame, an hour ≈
  95 MB for one clip. A sidecar of that size next to the project is already
  a conversation with the user, not an implementation detail.
  4) **The analysis parameters must live in the PROJECT, and the result in the DB.**
  Otherwise "Fast/Accurate" becomes a cache setting, and two people with one
  `.otio` would get different pictures.
- **Determinism is incompatible with the "random" RANSAC from the textbook.**
  Invariant 11 requires byte-for-byte matching against GOLDEN_VIDEO on the CPU; therefore
  the seed is fixed, the order of traversing features is fixed, and the floating-point sum
  accumulates in a fixed order. This is a constraint on the choice of algorithm, not on its implementation.
- **The rule of the kt-fx crate**: the warp effect will have to be written as a triple at once —
  scalar, AVX2 (`src/libs/kt-fx/src/video_simd.rs`) and WGSL
  (`src/libs/kt-gpu/src/shaders/`), byte-for-byte SIMD against the scalar; the defaults of the
  new parameters must give the previous behavior, otherwise 26
  goldens of t26/t67 move.
- **The format is cheaper than it seems.** Under the rule in `src/libs/kt-otio/src/
  meta.rs` (lines 11–17, `KT_META_VERSION = 4`) an unknown effect
  degrades into foreign passthrough and does NOT bump the version — one Pan3d
  precedent was created. So a new FX by itself does not move the version;
  a bump to v5 will be needed only if the trajectory goes into the clip envelope
  (an old build would lose it on overwrite).
- **The key of the post-FX cache.** `fx_chain_hash` (`src/libs/kt-engine/src/
  video.rs`, line 153) hashes the Debug output of the chain — if the trajectory gets into the
  effect parameters whole, the hash would be computed over megabytes on every
  frame. The chain must hold a REFERENCE to the analysis, not the analysis itself.

### Which invariants it touches

- **No. 5 (decoder)** — see above: not a new priority, but its own
  short-lived source modeled on `spawn_index_worker` / `thumbs.rs`;
  it makes sense to refine the wording of the invariant in the same round.
- **No. 8 (scroll/zoom does not generate decodes)** — the analysis is started ONLY
  by an explicit user action, never "by itself, when the clip landed on the timeline". This is the same as rule 20 of the UI checklist ("nothing grows or shuts off by itself"), and here we explicitly diverge from the FCP model
  "analyze everything on import": it is forbidden for us by our own
  rule. An "Analyze" button, progress, cancellation, one undo step.
- **No. 4 (the cache is the only truth about coverage)** — the analysis progress is NOT
  coverage and has no right to be drawn with the coverage bars of `BurstCache::coverage()`. It
  needs its own honest indicator; Anstey's quote is exactly about the cost of the absence of such an indicator.
- **No. 7 (one composition code)** — the compensation must be an effect in the layer's
  chain, which `frame_plan` computes, not a separate path "before the plan"; otherwise preview and export would diverge. The morpher path
  (`Project::effective_clips()`) is not suitable for this: it rewrites
  clips, not pixels.
- **No. 11 (determinism)** — a fixed seed and a fixed order of accumulation; cross-backend CPU↔GPU — only via
  `kt_gpu::assert_frames_close`.
- **UI rules**: 2 (the stabilization handles work during playback too),
  13 (the analysis panel takes the already occupied place of the FX editor, rather than a new
  row), 16 (impossibility is shown by an explicit error: "this clip is not
  analyzed", not by a grayed-out hover'), 20 (see above). Numeric
  parameters — only via `widgets::param_row` over `ParamSpec`; a smoothing curve, if it is needed, is drawn by the single widget in
  `src/apps/kt-app/src/automation.rs`.

## Estimate

| What | Answer |
|---|---|
| Size | XL — a whole round plus a revision of invariant 5 and of the sidecar DB schema |
| Where the main work is | kt-media (analysis section in `index.rs` + bump of `SCHEMA_VERSION`), a new motion analysis core (shared with F-MSK-005), kt-fx + kt-gpu (the warp effect as a triple: scalar/AVX2/WGSL), kt-app (start, progress, cancellation) |
| Project format | most likely does NOT touch it: a new FX degrades into foreign-passthrough (`src/libs/kt-otio/src/meta.rs`). v5 is needed only if the trajectory goes into `.otio`, not into the sidecar DB |
| Tests that will have to be written | determinism of the analysis (the same material → byte-for-byte the same trajectory, twice in a row and after a restart); round-trip of the DB section + invalidation by mtime/xxhash; unchanged picture at the defaults (26 goldens of t26/t67 do not move); CPU↔GPU conformance of the warp via `kt_gpu::assert_frames_close`; "the analysis does not go through MediaWorker" (the `DecodeStats` counters of playback do not grow during the pass); **composition with retiming** — stabilization over `SlowedReverb.rate` on video gives the same result as without retiming, per frame of the material, including `FrameMode::Blend`; a harness test of the window with progress and cancellation |
| Depends on | F-MSK-005 (the same motion analysis core — do it first) |
| Breaks compatibility | a bump of `SCHEMA_VERSION` in `src/libs/kt-media/src/index.rs` wipes the sidecar DB for everyone: peaks, thumbnails and packet indexes are rebuilt from scratch. The project format — no |

## Verdict

Take it, but not first and not as a "stabilization feature". First F-MSK-005:
the motion analysis core is one, and tracking presents it in a simpler
wrapper. The main conclusion of this note is not about stabilization, but about the fact that
**"offline analysis of the material" has become a class of tasks**, and we already have
a place for it — the sidecar `<project>.ktindex.db`, where peaks, thumbnails and
packet indexes lie in the background. The trajectory fits there by domain (a function of the
material, invalidation by xxhash — for free), but it requires two questions to be decided BEFORE the first
line of code: a heavy analysis cannot have the contract "cache, loss is not an
error", and the Subspace Warp data is three orders of magnitude larger than the affine data.
One must start with the two lower rungs of the ladder (Position and Position/Scale/
Rotation) — their executor is almost written already (`Pan` + rotation), they
cover most real material, and they do not drag along either a grid warp
or edge synthesis. A pleasant side conclusion: the conflict
"stabilization + speed", which Premiere has been closing with a ban for fifteen years, is closed for us in advance by the choice of domain — but only if
the compensation goes BEFORE the frame blending in `FrameMode::Blend`.

## Sources

- Creative COW, "Warp stabilizer – What am I doing wrong?",
  https://creativecow.net/forums/thread/warp-stabilizer-what-am-i-doing-wrongae/
  (accessed 2026-09-07) — quotes from Ahlfors and Vandenbossche, the ladder of models
- Creative COW, "Analyzing in Background forever",
  https://creativecow.net/forums/thread/analyzing-in-background-forever/
  (2026-09-07) — quote from Anstey
- Creative COW, "Warp Stabilizers makes Steadycam footage worse",
  https://creativecow.net/forums/thread/warp-stabilizers-makes-steadycam-footage-worse/
  (2026-09-07) — quote from Benjamin Gs
- Adobe Support Community, "Why can't warp stabilizer and speed be used on
  the same clip?",
  https://community.adobe.com/questions-729/why-can-t-warp-stabilizer-and-speed-be-used-on-the-same-clip-1404724
  (2026-09-07) — quote from Haugen
- Adobe Support Community, "Warp Stabilizer + Speed in nested sequence
  produces glitches",
  https://community.adobe.com/t5/premiere-pro-discussions/warp-stabilizer-speed-in-nested-sequence-produces-glitches-dead-pixels/m-p/12906033
  (2026-09-07) — the workaround via a nested sequence
- Boris FX, official documentation of VEGAS Pro 2026, "Stabilizing Video
  Clips",
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/video_stabilization_new.htm
  (2026-09-07) — Media FX vs Event FX, Analyze Motion, modes, edges
- MAGIX, VEGAS Pro 22/23 help, the same section,
  https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/video_stabilization_new.htm
  (2026-09-07) — checking the wording against the previous line
- Apple, Final Cut Pro User Guide, "Correct shaky video",
  https://support.apple.com/guide/final-cut-pro/correct-shaky-video-verbacf92b/mac
  (2026-09-07) — Automatic / InertiaCam / SmoothCam, Tripod Mode,
  rolling shutter
- Blackmagic Design, DaVinci Resolve — Edit,
  https://www.blackmagicdesign.com/products/davinciresolve/edit
  (2026-09-07) — quote about the built-in stabilization in the Inspector, the
  camera lock / zoom / crop ratio / smoothing / strength controls
- Blackmagic Design, DaVinci Resolve — Color,
  https://www.blackmagicdesign.com/products/davinciresolve/color
  (2026-09-07) — the tracker as a stabilizer on the Color page

**Honestly about the gaps.** `helpx.adobe.com` does not open from this environment (the page `warp-stabilizer-settings.html` was found by search, but
not read), so the exact LIST of Warp Stabilizer parameters was assembled from
forum quotes (Method and Result are confirmed verbatim) and secondary
write-ups (the names of the four Framing modes, `Synthesis Input Range`,
`Rolling Shutter Ripple`) — marked `[NOT VERIFIED]` where there is no primary source. The names of the three Resolve
stabilization modes (Perspective / Similarity / Translation) are taken from secondary
write-ups; the official BMD page confirms only the set of controls. Reddit and
`discussions.apple.com` are unavailable, the `WebSearch` budget is exhausted — the search
ran through direct queries to Yahoo. None of the five editors was tested live.

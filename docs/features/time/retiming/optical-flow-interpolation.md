---
id: F-SPD-003
title: Frame interpolation when slowing down: three quality levels
status: candidate
priority: B
cost: XL
category: time/retiming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-cache, kt-gpu, kt-app]
invariants: [4, 5, 7, 8, 11]
ui_rules: [4, 9, 12, 19, 20]
round: null
point: null
tests: []
depends_on: [F-SPD-001]
checked: 2026-09-07
---

# F-SPD-003 — Frame interpolation when slowing down: three quality levels

## Essence

Slow it down fourfold — the source gives 25 frames per second, the timeline needs 100.
Seventy-five frames do not exist in ANY form, they have to be invented. The industry invents them in
three ways, and the editor chooses from a drop-down list not "quality" but the KIND OF DEFECT: hold a frame (in jerks, but honest),
blend two neighbours (softly, but with ghosts), or synthesise a new one from motion vectors (smoothly, but on complex motion the picture
wobbles like jelly). There is no fourth option — "make it good" — for anyone.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | `Resample` in the event properties (Smart / Force / Disable) **plus a separate OFX effect `Slow Motion`** | TWO DIFFERENT MECHANISMS. The classic `Resample` — blending of neighbours, switched on by the "smart" mode by default. Optical flow came not into the event properties but as an EFFECT with its own `Playback Rate`, `Quality` (Low/Medium/High), `Blending` (`Blend` \| `Nearest Frame`), an `Adjust Event Length` button, and **`Show Optical Flow Field`**. In the event properties the sampling method is also switched to `Optical Flow` |
| Adobe Premiere | 26.x | `Speed/Duration` → **`Time Interpolation`**: `Frame Sampling` / `Frame Blending` / `Optical Flow` | one list of three, exactly that ladder. `Frame Sampling` repeats and discards frames, `Frame Blending` repeats AND mixes in, `Optical Flow` synthesises. After the choice a red bar appears over the clip — until `Sequence \| Render In to Out` the preview shows NOT what will be in the export. The same list exists separately in the export settings, and there it works on something else — frame-rate conversion |
| DaVinci Resolve | 21 | `Retime Process`: `Nearest` / `Frame Blend` / `Optical Flow`; with `Optical Flow` **`Motion Estimation`** and **`Motion Range`** open up | the only one where the third step is broken down into settings: `Motion Estimation` — `Standard`/`Enhanced` × `Faster`/`Better` plus **`Speed Warp`** on the DaVinci Neural Engine (**Studio only**), `Motion Range` (Small/Medium/Large) tells the solver how large a motion to search for. Officially: "Control the quality of retimed footage by choosing between optical flow, frame blending and nearest frame rendering" |
| Final Cut Pro | 12.3 | `Retime` → `Video Quality`: `Normal` / `Frame Blending` / `Optical Flow` / **`Best (Machine Learning)`** | the fourth step is the neural-network `Smooth Slo-Mo` (since FCP 11), "requires a Mac with Apple silicon", frames larger than 4K require M2+, the ceiling is 8K. The flow has NO settings at all: a list of four items and that is all. The optical-flow analysis is written as files into the event folder (`Analysis Files`) and survives the project |
| After Effects | 26.x | `Frame Blending` on the layer, two modes: **`Frame Mix`** and **`Pixel Motion`** | not a "retime quality" but a switch of the LAYER: `Frame Mix` blends neighbours, `Pixel Motion` synthesises from vectors. Officially: "Frame Mix takes less time to render, but Pixel Motion provides much better results, especially for footage that has been drastically slowed down". The same solver is sold separately as the `Timewarp` and `Pixel Motion Blur` effects. The layer switch does not work until the composition's master switch is turned on [NOT VERIFIED: `helpx.adobe.com` does not open from this environment] |

## The trick behind it

**Inside the third step sits the second — as an emergency exit, not as an alternative.** The OFX effect `Slow Motion` in VEGAS has a parameter
`Blending` with the values `Blend` and `Nearest Frame` — that is, the ladder is not three neighbouring items of one menu but a MATRYOSHKA: the flow solver itself decides
where it has failed, and there it falls back to the cheap method. From this follows a non-obvious thing: "turned on optical flow" does not mean "the whole frame is synthesised"
— it means "synthesised where the solver is confident". That is why in Resolve too
`Motion Estimation` is not monotonic: on the BMD forum they write directly that "Enhanced
or Better doesn't always look better" — the items of the list are not the strength of the effect but
DIFFERENT algorithms with different zones of failure [NOT VERIFIED: the forum itself returns
403, the quote is taken from the search index].

**VEGAS shows the vector field, and nobody else does.**
`Show Optical Flow Field` — "Enable this option to visualize the motion
flow fields used to calculate the in-between frames. This can help
diagnose issues with motion detection". This turns "jelly" from mysticism into diagnostics: one can see that the solver took a glare for an object, that on a
repeating texture the vectors fan out, that on the edge of a semi-transparent object the field tears. The cost on the other side: the effect has its OWN
`Playback Rate` alongside the live velocity envelope of the event — two speed controls on one clip, and they do not know about each other (hence the button
`Adjust Event Length`, which fits the event length to the effect).

**The third step costs not render time but a PRELIMINARY PASS, and this changes its class of task.** The first two steps are pure functions of
frames: give two neighbours, get the result, remember nothing. Optical flow is a two-phase pipeline: first analysis of the material, then synthesis. FCP
writes the analysis result into the event folder as separate files, they survive
the project and are reused by other projects; Premiere requires a render
and before it LIES in the preview. So by its design it is a relative of tracking
(F-MSK-005) and stabilisation (F-MSK-006), not of effects: all three have
a state "analysed" that an ordinary FX does not have.

**A change of shot is not a "hard case" but a mathematically empty one.** The solver
looks for the correspondence of pixels between two frames under the assumption that brightness
is preserved and objects do not teleport anywhere. At the junction of two shots
there is NO correspondence AT ALL: any field the solver finds is noise,
and the synthesised frame turns out to be a mash of two scenes. This is exactly what is caught
by hand: the same `Time Interpolation` list in Premiere's EXPORT settings
works on the whole film at once and therefore cuts across all the splices — "Optical
Flow… makes some kind of glitches between each clip", and the industry answer
to this is: "Optical flow in the timeline is only used on slomo's. You only
use optical flow in the export settings when export framerate is different
from source". Hence the advice to cut BEFORE the retime: slow down a piece, not the sequence.

## Pros

- Three steps are THREE DIFFERENT kinds of defect, not three levels of one scale:
  step 1 gives a jolt, but every pixel shown is real, captured by the
  camera; step 2 gives a ghost, but does not invent geometry; step 3
  invents geometry. The editor chooses what to pay with, not "how much".
- Step 1 is free by construction: "which frame to show" is decided by the same
  rounding that the time mapping already uses, and it computes NOTHING.
- Step 2 is a single dissolve between two neighbours: no analysis, no state, no
  second pass, no dependence on the frame content. Larry Jordan gives it a
  working range down to 10 % speed, so it covers the main mass of practical
  slowdowns.
- A matryoshka (a flow with a fallback to a cheap method) degrades smoothly: one
  failed region does not spoil the whole frame.
- Preliminary analysis stored in files next to the media (FCP) is paid once per
  media, not once per project: move the clip to another project and the analysis
  is already there.
- `Motion Range` in Resolve is a rare case where the user is allowed to tell the
  solver what it guesses anyway (the search radius), instead of hiding it behind
  "quality".

## Cons

- Optical flow rests on the assumption "pixel brightness is preserved, the object
  does not teleport". Hence the whole list of failures, and it is NOT random:
  a change of shot (no correspondence), fast motion (the object left the search
  window), semi-transparency and reflections (one pixel has two motions at once),
  repeating texture - grid, picket fence, titles, watermark (the correspondence is
  ambiguous), change of lighting and flicker (brightness is not preserved),
  motion blur (object edges are smeared, and the edges of the vector field too).
- "Quality" is not monotonic: a more expensive `Motion Estimation` preset may look
  WORSE on a particular footage, because it is a different algorithm. The rule
  "set the maximum and don't think" does not work, so the choice cannot be hidden
  behind a single checkbox.
- The best step is everywhere paid or tied to hardware: `Speed Warp` is only in
  DaVinci Resolve **Studio**, `Best (Machine Learning)` is only on Apple silicon and
  only up to 8K, above 4K an M2+ is needed.
- The preview lies until render (Premiere): the red bar means that the screen now
  shows NOT what will go to the master, and the real result can be seen only by
  paying for a render.
- Step 2, enabled by default, spoils material where it was not asked for: the
  "smart" resample in VEGAS catches any frame-rate mismatch (59.94 → 29.97) and
  mixes in neighbours, and the editor sees ghosts on the helicopter blades. The
  industry answer is to disable it with a script on all events before rendering.
- Analysis is a third state of the project besides the model and the cache: it has
  to be stored somewhere, invalidated when the source is replaced, and not
  recomputed on scrolling.
- The solver's result is not reproducible between implementations: two passes on
  two devices give DIFFERENT pixels. For an NLE this is not an abstraction - it is
  the question "does the preview match the export or not".

## What users say

About step 2, enabled by default, it is the classic "turned it off for good":

> Yeah, its long established that Vegas's resampling cases ghosting,
> That's why i always recommend disabling resample on every event before a
> render.
> — Steve Rhoden, Creative COW, 20.02.2014,
> https://creativecow.net/forums/thread/tenative-conclusion-smart-resample-causes-ghosting/

The thread author had earlier guessed WHAT exactly was happening:

> I'm speculating that because the source is double frame rate, Vegas in
> Smart Re-sample mode is not just throwing away the frames, it is trying
> to do some kind of blending or interpolation.
> — Stephen Crye, Creative COW, 20.02.2014,
> https://creativecow.net/forums/thread/tenative-conclusion-smart-resample-causes-ghosting/

About step 3 at the cuts, both sides of one thread:

> Doesn't make the fade thing but instead in makes some kind of glitches
> between each clip
> — David Robisco, Creative COW, 26.06.2022,
> https://creativecow.net/forums/thread/glitches-when-exporting-with-optical-flow/

> Optical flow in the timeline is only used on slomo's. You only use
> optical flow in the export settings when export framerate is different
> from source.
> — Ann Bens, Creative COW, 26.06.2022,
> https://creativecow.net/forums/thread/glitches-when-exporting-with-optical-flow/

And the most useful wording of the difference between steps 2 and 3, with working
thresholds:

> The problem is that hallucinations exist; especially when moving elements
> overlap. […] Dissolves don't hallucinate.
> — Larry Jordan, "Caution: Slow Motion in Apple Final Cut Pro Tends to
> Hallucinate", 16.07.2025,
> https://larryjordan.com/articles/caution-slow-motion-in-apple-final-cut-pro-tends-to-hallucinate/

His thresholds are there too: "From 25% to 10%, I recommend frame blending",
"Slower than 10%, though, the dissolves become noticeable", "Below 10%, though, be
careful" - that is, step 3 is needed not "for quality", but starting from roughly
a tenfold slowdown.

## How it fits Kinotrain

### What already exists

**Steps 1 and 2 are ALREADY DONE, and this changes the statement of the question.** The
note F-SPD-001 was written from the premise "we have no video retiming at all" - this
premise is outdated: round R.37 (commits `6301291`…`bdff0b0`, 07.09.2026) closed
video retiming in five stages. Verified by grep, not from the brief.

- **Video retiming**: `VideoWarp { base, prof, used, blend }`
  (`src/libs/kt-engine/src/video.rs:70`), extraction from the active chain
  `clip_video_warp` (`video.rs:83`), material position by the INTEGRAL of the
  speed `clip_material_pos` (`video.rs:182`), the map
  `kt_fx::warp_material_ticks` (`src/libs/kt-fx/src/lib.rs:735`).
  The speed lives in `FxInstance::SlowedReverb.rate`
  (`src/libs/kt-model/src/project.rs:1911`), shared by the A/V pair
  (`sync_warp_to_link_group`, item 248). The linear path remains exactly where
  there is no warp (`video.rs:184`) - `GOLDEN_VIDEO` stands on this. Items 246–251
  of `docs/porting-notes.md`.
- **Step 1 is free by construction**: choosing the source frame is one line
  `grid.align_floor(clip_material_pos(c, t, grid))` in
  `clip_source_pts` (`src/libs/kt-engine/src/video.rs:175`). A subtlety worth
  knowing in advance: this is FLOOR, not rounding to the nearest - at 0.5× it gives
  a systematic shift of half a frame backwards, against the literal "nearest" of
  other editors.
- **Step 2 is done in full** (R.37 C5, item 250 of `docs/porting-notes.md`):
  `FrameMode { Nearest, Blend }` (`src/libs/kt-model/src/project.rs:2469`),
  the pair of neighbours is resolved by ONE function `clip_frame_keys`
  (`src/libs/kt-engine/src/video.rs:233`, weight `k` in 4096ths), the mix
  is made in `layer_src` (`video.rs:485`) by the ready byte-frozen kernel
  `blend_frames`/`dissolve_into` (`video.rs:846`/`:867`,
  `(av·ik + bv·k + 2048) >> 12`) with an AVX2 twin
  (`src/libs/kt-engine/src/video_simd.rs:45`). Prefetch warms BOTH frames of
  the pair, `max_concurrent_layers` counts the blending layer as two
  (`video.rs:424`), the `ReadbackCache` key in kt-gpu carries `LayerBlend`
  (`src/libs/kt-gpu/src/engine.rs:1077`). Tests 250 and 250b,
  `src/libs/kt-engine/tests/video_warp.rs:201`.
- **And the main thing - the key decision of item 250 is already made for us:** the
  blending lives in the COMMON code of the plan, not in the executors; both backends
  receive an already blended pre-FX frame, WGSL is not touched at all, CPU↔GPU
  conformance is free. This is a direct consequence of invariant No. 7 and a ready
  model of where stage 3 must land.
- **The place in the UI is ready**: the frame-mode picker is a `ComboBox` in
  `src/apps/kt-app/src/fx_editor.rs:606-621`, the translation keys
  `frames_nearest`/`frames_blend` (`src/apps/kt-app/i18n/en.toml:325`).
  The third option is one more line in an array, not a new widget.
- **The owner has already named the third stage**: "fruc_vulkan - as a third mode
  later" (`docs/user-feedback.md`, R.37 decisions, item 3). The same is written in
  the code: the comment above `FrameMode` (`project.rs:2466`) states directly
  "The third will fit here: fruc_vulkan".
- **The verification apparatus is ready**: `FrameTol { EXACT, BILINEAR, THRESHOLD }` and
  `assert_frames_close` (`src/libs/kt-gpu/src/lib.rs:18-36`, `:41`),
  `GOLDEN_VIDEO` (`src/libs/kt-engine/tests/golden_export.rs:275`).

### What is missing

- The solver itself. Optical flow is not expressed by a per-frame filter:
  our video FX take one frame and return one frame (`kt_fx::video::Frame`,
  `src/libs/kt-fx/src/video.rs:21`), while synthesis needs TWO frames plus the
  fraction `k` - that is, a shape exactly like `blend_frames`, but with state
  (the vector field) between calls.
- A place where the vector field lives. Our cache is the single source of truth
  about warmth (invariant No. 4), and the kinds of bursts are listed explicitly:
  `BurstKind::{Video, PostFxVideo, …}` (`src/libs/kt-cache/src/lib.rs:31`).
  The field is a third kind, keyed by a pair (neighbour idx + quality), because
  a flow computed at preview resolution is NOT equal to a flow at
  export resolution, and `Quality` is already part of the key.
- Decisions about the preliminary pass. We have a ready place for
  "computed in advance and lying next to the material" - the sidecar
  `<project>.ktindex.db` (`src/libs/kt-media/src/index.rs`), where the
  probe, the batch index, the peak pyramid and the thumbnails already live.

### What gets in the way

**1. Invariant No. 11 against the very nature of the algorithm.** Step 2 got conformance
for free: integer arithmetic in 4096ths, one kernel, AVX2 byte-for-byte with scalar.
A flow solver is iteration in floating point, and it is reproducible byte-for-byte only
with a fixed summation order, thread count and SIMD path. The kt-fx requirement "when you
change the formula, you change the scalar, the AVX2 kernel and the WGSL in one commit, SIMD
byte-for-byte with scalar" is an exercise for `dissolve` - for a pyramidal solver it is the
most expensive part of the work. Plus `GOLDEN_VIDEO` becomes a guard of the algorithm, not
only of the composition.

**2. `fruc_vulkan` - a choice directly opposite to item 250, and tolerances do not save it
here.** Putting the synthesis on the GPU means gaining speed and losing everything else: the
CPU path will not give the same pixels, and the same project rendered on the CPU would come
out as a DIFFERENT picture. This is not a difference "within tolerance" - a synthesized frame
against a held one differs on a moving edge by tens of LSB, while our softest preset
`THRESHOLD` allows a mean |Δ| ≤ 1.0 LSB (`src/libs/kt-gpu/src/lib.rs:35`). So the question is
not "which tolerance to take", but "which mode was it computed in" - and invariant No. 7 says
the backend is a parameter of execution, not a second path. Hence an honest fork, which must be
decided BEFORE the code: either one solver in the common plan code (as `blend_frames`) - slow,
but all invariants stay intact and WGSL again is not touched; or a GPU solver - and then the third
mode exists ONLY on the GPU backend, and the CPU must refuse with an explicit error, as export
without a GPU already refuses for 3D effects.

**3. `fruc_vulkan` is also Vulkan next to wgpu.** Our GPU lives ONLY in kt-gpu and only through
wgpu (strictly the same version as in eframe); the spirit of invariant No. 10 is "one and the same
executable works everywhere". A direct dependency on a Vulkan library breaks this, and our pixels on
both sides are eight-bit integers (`Rgba8Uint`, `src/libs/kt-gpu/src/pool.rs:13`) - the solver would
work on quantized data.

**4. A second pass against invariants No. 8 and No. 5.** Analysis of the material is decoding, and
decoding is owned by exactly one `MediaWorker` with a priority queue
(`src/libs/kt-engine/src/worker.rs`), and scrolling/zooming the timeline has no right to generate
decode requests. Prefetch can already warm both sides of the pair (test 250b, 61 keys instead of 60),
but the solver needs a window wider than the pair, and `span_key_budget`
(`worker.rs:327`) is where it will hit a limit.

**5. Our own version of the "plan change".** The neighbour in `clip_frame_keys`
(`video.rs:242`) is taken FORWARD by the material index - including for `reversed`, and including
beyond the boundary of the trimmed event window. For a mix this is an invisible subtlety (an extra
source frame is still source), but for a flow solver it is exactly the case where the pair of frames
can end up on opposite sides of a cut, if the cut sits inside the source file itself.

### Which invariants it touches

- **No. 4** (the cache is the single source of truth about coverage): the vector field and/or
  the synthesized frame must become a kind of burst, otherwise a second bookkeeping of warmth
  will appear.
- **No. 5** (one MediaWorker owns the decoder): the analysis window goes through the same
  queue with the same generation cancellations.
- **No. 7** (one code for composition): the fork "common code versus GPU solver" is the main
  question of the note.
- **No. 8** (scrolling/zooming does not generate decoding): the preliminary pass has no right
  to start from scrolling.
- **No. 11** (determinism): both byte-for-byte between runs on the CPU, and the impossibility of
  reconciling the CPU with the GPU by tolerances.
- UI: **4** (the default is useful - an expensive mode cannot become the default, `Nearest` stays),
  **9** (the third item of the existing `ComboBox`, not a new panel), **12** (background analysis
  not during a gesture and not during playback), **19** (the banner about an unavailable mode must
  say where it is switched), **20** (the analysis does not start by itself).

## Estimate

| What | Answer |
|---|---|
| Scope | XL - a whole round PLUS an EXPLICIT decision on invariants No. 11/No. 7 before the first line of code |
| Where the main work is | the solver itself (a candidate for a separate crate such as `kt-flow`, so that kt-engine does not get thicker), `clip_frame_keys`/`layer_src` in `src/libs/kt-engine/src/video.rs`, a new burst kind in kt-cache, the third variant of `FrameMode` in kt-model, a row in the picker of kt-app |
| Project format | DOES NOT TOUCH: `frames` travels through serde-derive of `FxInstance`, the default `Nearest` is not serialized (`is_default_frame_mode`, `project.rs:1554`), the version `KT_META_VERSION = 4` stays. The precedent is `FrameMode` itself in R.37 |
| Tests | determinism of the synthesized frame between runs and against the reference; "prefetch == what was read" on a window wider than the pair (after the pattern of t247d/t250b); behaviour at the boundary of the event window and on reverse; an explicit refusal when the mode is unavailable on the current backend; in kt-gpu - NOT conformance by tolerances, but a check that both executors received ONE AND THE SAME frame |
| Depends on | F-SPD-001 (video retiming - already done, item 247 of `docs/porting-notes.md`). Relatives by class of task, not by code: F-MSK-005 (tracking), F-MSK-006 (stabilization) - all three have a "analysed" phase |
| Breaks compatibility | no |

## Verdict

Take it, but last: steps 1 and 2 are already in place and paid for (R.37,
items 247/250), and by Larry Jordan's thresholds blending covers slowdowns down to
~10 % - that is, the overwhelming majority of real ones. Step 3 is needed below this
threshold, and it is worth not "one more effect" but a decision on invariants No. 7 and No. 11.
Until this decision is made, the work must not start: `fruc_vulkan` in the
literal reading (a GPU solver) means that the CPU and the GPU render a different
picture, and no `assert_frames_close` will cover this - one must either
write the solver in the COMMON plan code next to `blend_frames` and pay
in speed, or honestly declare the third mode GPU-only with an explicit refusal on
the CPU. The first agrees better with everything already built.

## Sources

Official documentation:

- DaVinci Resolve, Edit page (the wording about three methods and Studio-only
  Speed Warp): https://www.blackmagicdesign.com/products/davinciresolve/edit
  (accessed 2026-09-07)
- VEGAS Pro, help on the `Slow Motion` effect (parameters `Quality`,
  `Blending`, `Show Optical Flow Field`, `Adjust Event Length`, switching of resampling in the
  event properties):
  https://help.magix-hub.com/video/vegas/22/en/content/topics/8-design/fxplugins/slowmotion.htm
  (accessed 2026-09-07). **The help branch for 2026 returns 404**, checked
  via the tree `/22/` - divergences from 2026.0 are not excluded, marked in the table
- Final Cut Pro, "Change clip speed" (`Normal` / `Frame Blending` /
  `Optical Flow` / `Best (Machine Learning)`, requirements of `Smooth Slo-Mo`):
  https://support.apple.com/guide/final-cut-pro/change-clip-speed-ver40b00150/mac
  (accessed 2026-09-07)

Forums and analyses (live quotes):

- Creative COW, "Tentative conclusion: smart resample causes ghosting",
  20.02.2014:
  https://creativecow.net/forums/thread/tenative-conclusion-smart-resample-causes-ghosting/
- Creative COW, "Glitches when exporting with Optical Flow", 26.06.2022:
  https://creativecow.net/forums/thread/glitches-when-exporting-with-optical-flow/
- Larry Jordan, "Caution: Slow Motion in Apple Final Cut Pro Tends to
  Hallucinate", 16.07.2025:
  https://larryjordan.com/articles/caution-slow-motion-in-apple-final-cut-pro-tends-to-hallucinate/
- Larry Jordan, "FCP X: Understanding Optical Flow" (storing analysis files
  in the event folder): https://larryjordan.com/articles/fcpx-optical-flow/
- Frame.io Insider, "Mixing Frame Rates in DaVinci Resolve, Part 3"
  (`Motion Estimation`, `Motion Range`, description of the artifact "the drummer's arm
  tears from frame to frame"): https://blog.frame.io/2019/10/17/mixed-frame-rates-part-3/
- Boris FX, "3 Methods for Using Optical Flow in Premiere Pro" (the path
  `Speed/Duration` → `Time Interpolation`, the render requirement):
  https://borisfx.com/blog/how-to-use-optical-flow-in-premiere-pro/

What could NOT be verified:

- `helpx.adobe.com` does not open from this environment (`proxy refused`),
  so the Premiere and After Effects lines rely on the Boris FX blog and on
  Adobe's wording taken from a search index
  ("Frame Mix takes less time to render, but Pixel Motion provides much
  better results…"). The statement about the master switch `Frame Blending`
  of a composition in AE is marked `[NOT VERIFIED]`.
- `forum.blackmagicdesign.com` returns 403 on all requests; the quote about
  "Enhanced or Better doesn't always look better" is taken from the Yahoo index and
  marked `[NOT VERIFIED]`.
- `reddit.com` and `discussions.apple.com` are unavailable (recorded in
  `_editors.md` already in the first wave).
- **The point "4:2:0 and chroma interpolation" could NOT be confirmed by vendor
  documentation.** It was searched for specifically (combinations of "optical flow"
  + "chroma subsampling" / "4:2:0", the FAQ and product page of Twixtor from
  RE:Vision Effects, the help of Vegas/FCP/Resolve). None of the five vendors
  writes about colour halos from interpolation of half-resolution chroma;
  only secondary general texts about subsampling turn up. Therefore the point has NOT been
  entered in the note as a fact: that the solver usually computes the field from LUMA, while chroma
  follows at half the resolution, is plausible, but is not confirmed by a primary source, and presenting
  it as fact would mean exactly that confident tone on the unverified, against which the base was set up.

The forum `vegascreativesoftware.com` had moved by the time this note was written:
`/us/forum/` redirects to the Discourse `forum.borisfx.com`, where
the Vegas Pro section has only 11 threads, and the archive of the old forum is unavailable.
The next wave will have to look for quotes on VEGAS at Creative COW and
videohelp.

---
id: F-RES-007
title: "The preview showed one thing, the export another"
status: already-have
priority: A
cost: S
category: resilience/environment
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: yes
crates: [kt-engine, kt-gpu, kt-fx, kt-app]
invariants: [4, 7, 11]
ui_rules: [16]
round: null
point: null
tests: [t11, t67, t66c, t181, t182, t183, t184, t198]
depends_on: []
trackers: [kde-bugzilla, kde-invent, pitivi, shotcut, olive]
checked: 2026-09-08
---

# F-RES-007 — "The preview showed one thing, the export another"

## Essence

The editor sees a frame in the preview window, makes a decision based on it — the strength of a blur,
the moment of a transition, the position of a title — and presses "Render". Forty minutes later
there is a file where the blur is different, the title is missing, and the transition stands in the wrong place. The work was lost not because the program crashed: it ran and produced something other than what it showed. This is a separate class from "the export failed": there the user learns about the trouble at once, here — after publication.

## Who has it and how

| Editor | Version | Called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Preview quality (Draft/Preview/Good/Best × Auto/Full/Half/Quarter), Dynamic RAM Preview | the preview window has its own pair "quality × scale", the render has its own template; the "Best/Full" level exists precisely because the other levels compute differently |
| Adobe Premiere | 26.x | Playback Resolution / Paused Resolution, High Quality Playback; on export — "Use Previews", "Use Maximum Render Quality" | the fork is declared by checkboxes: the export may TAKE the already computed preview files, or may compute again and at a different precision |
| DaVinci Resolve | 21 | Timeline Proxy Mode (Half/Quarter), Optimized Media, Render Cache; on Deliver — "Force sizing/debayer to highest quality" | editing runs on proxies and cache, delivery runs on the originals; the "force … highest quality" checkboxes are an admission that there are two paths |
| Final Cut Pro | 12.3 | Viewer: Better Performance / Better Quality, proxy media, render files | "Better Performance" is half resolution and proxies; "Better Quality" switches the viewer to the same material that will go into Share |
| After Effects | 26.x | Resolution Full/Half/…, Adaptive Resolution, Fast Previews (Draft/Fast Draft), Draft 3D; a separate Render Settings | the two paths are not hidden but separated into different dialogs: the composition has its own Quality/Resolution, the render queue has its own, and they are not required to match |

**Outside the core.** Without Kdenlive this feature cannot be described: the five core editors
have the fork, but its insides are closed, and nobody has a count of the pain.
Kdenlive is the only program where a SECOND executor is both documented and
visible by name in the public tracker.

| Program | Version | Called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 | Timeline Preview (Render Zone), Preview Resolution, Proxy Clips | preview zones are rendered in advance into SEPARATE chunk files and then played back as material; the preview resolution and the proxies change the input of the same MLT graph — three independent ways to get a picture different from the final one |

## The trick behind it

**The guarantee "preview == export" rests not on code but on a PROHIBITION, and therefore it cannot be added later.** We have no function that ensures it; there is a rule
(invariant 7) that forbids writing a second one. `build_plan`
(`src/libs/kt-engine/src/video.rs`) is the only place where the order of
layers, transition weights, levels and the "tick → source frame" mapping are decided; the preview calls
`compose_from_cache_into`, the export calls `compose_export_into` or
`frame_plan` + the GPU executor, and all three go into the same function. As soon as
someone optimises the preview separately once — "here it can be cheaper, after all
it is only a display" — the guarantee disappears for good, because gluing two paths
back together costs more than never splitting them. All five core editors have already
gone through this fork, and none of them can go back.

**The preview's resolution fraction reduces the WORK, but does not change the CODE.** The switch
1/16…1/1 (`draw_preview_controls`, `src/apps/kt-app/src/app.rs`) is a `Quality` value that is passed as a PARAMETER into the same `build_plan`. It goes into the cache key (invariant 4), so frames of different fractions do not mix, and in
`world_scale = quality.width / project_size.0` — which is why a 3D camera, whose
coordinates live in PROJECT pixels, shows the same thing at a quarter. There is not a single
`if preview { … } else { … }` branch in the composition.

**The only place where preview and export still differ is the decision about CACHING, and it is proven not to change the bytes.** `FramePlan::memoize_postfx`
is `false` in export: there each moment is composed exactly once, and the full-size post-FX frames were
evicting from the export cache the not-yet-used pre-frames of the neighbouring layers. The difference must be invisible, and this is checked separately: t66c (the export composite does not grow the cache's `video_bytes`) and
t67 (a cold and a warm preview composite are both byte-for-byte equal to the frame
of the export stream).

## Pros

- The class "the effect is visible in the preview and gone in the file" is not expressible by construction:
  the FX list of a layer is one and the same object (`LayerSrc::fx`), automation curves
  are resolved ONCE when the plan is built, and both executors get ready
  numbers.
- Byte-for-byte is checked, not declared: t11 (frame No. 17 of the export stream
  == the preview composite of the same moment), t67 (the same with the Mirror+Glow+VHS chain,
  cold and warm), t184 (the same entirely on the GPU, including a fresh engine
  against the export engine).
- The guarantee does not depend on the processor: t198 iterates `div255` over all 2^24
  inputs and `alpha_over` over all 256×256 pairs, requiring the AVX2 kernel to match the scalar one byte for byte. The SIMD dispatcher cannot quietly make the preview "almost the same".
- Compositing between backends is also exact, not approximate: t182 checks
  eleven cases (alpha-over, level, three kinds of dissolve, solo fade, starved layers, a layer of a foreign size, an empty plan) with the tolerance
  `FrameTol::EXACT` — divergences are possible only inside individual FX kernels, but not
  in how the layers are stacked.
- A started export does not mix backends: a GPU error in the middle goes via `?` out of
  `render_raw_video_range` and becomes an export error
  (`src/libs/kt-engine/src/export.rs`). There is no quiet "let's finish the rest on the CPU" — exactly the thing because of which half of someone else's file looks different — there is none here.
- A project with 3D effects without a GPU refuses to export BEFORE the file is created
  (`ExportError::GpuRequired`, t187c), rather than handing over a file without these effects. This is rule 16 of the UI in its pure form: an impossibility is an error, not a silent result.
- We have no proxies at all (F-MED-002 — `no`), so one third of someone else's pain
  ("the preview took the proxy, the render took the original") has no subject.

## Cons

- **The preview cannot be faster than the export on the same frame — this is the price we pay every day.** The only permitted way to speed up the display is
  to reduce `Quality`, that is, to compute the same thing on a smaller frame. Neither
  a "draft mode of effects", nor "skip FX while scrubbing", nor
  Kdenlive's pre-rendered chunks are available to us without abandoning invariant 7.
- **Between backends the guarantee is weaker: there it is a tolerance, not bytes.** GPU against CPU
  is checked by `assert_frames_close` with `FrameTol::BILINEAR` (|Δ| ≤ 2 LSB, up to 0.1 %
  of pixels up to 4 LSB, mean |Δ| ≤ 0.5) — and for threshold effects
  `FrameTol::THRESHOLD` does not limit the outlier at all. That is, "preview ==
  export" holds true INSIDE a backend; a user whose preview is on the GPU while
  the export fell back to the CPU will get a picture within tolerance, not the same one.
- **And such a fallback is possible silently.** `export_video_gpu()`
  (`src/apps/kt-app/src/app.rs`), on failure to create the headless device,
  prints `eprintln!("video export on CPU (no GPU): …")` and returns `None`.
  There is not a line about it in the render window; the performance panel shows the PREVIEW's backend, not the backend of the job.
- **There is also a reverse skew:** a preview poisoned by `DeviceLost` goes to the CPU until the end of the session (a notice once, t185), while the export still creates a new
  headless device and goes to the GPU. The poisoning of the preview is not part of the export's decision.
- **The real remainder of the class: FX parameters set in PIXELS are computed in the pixels of the CURRENT fraction.** `blur` takes `step = radius/3 + 1` and squeezes the frame into `f.w/step`; `glow` squeezes into `f.w/r`; `vhs` shifts the channels by `rgb_shift`
  pixels (`src/libs/kt-fx/src/video.rs`). The multiplier is fixed, while a frame at 1/4 is four times smaller — which means that on a 1/4 preview the blur covers four times
  the share of the picture that it does in the export. Byte equality at the SAME `Quality` does not suffer from this and all tests stay green; what suffers is exactly
  the promise for which the invariant was created. It is telling that for 3D this
  problem was seen and closed (`world_scale`), while for flat kernels it was not.
- The hint of the switch (`preview_quality` in `apps/kt-app/i18n/*.toml`)
  says "the project's resolution fraction (smaller — faster)" and says nothing about this remainder.
- The video reference `GOLDEN_VIDEO` is gated by `cfg(target_os = "linux",
  target_arch = "x86_64")`: on another platform "byte-for-byte against the committed one" is not checked at all, only "byte-for-byte between
  runs" remains (t11 without the gate).

## What users say

SIGNAL: a divergence of preview and output — 14 tasks in kde-bugzilla, `(preview|monitor|playback).*(\bbut\b|although|differ).*(render|export)|(render|export)\w*.*(\bbut\b|although|differ).*(preview|monitor|playback|timeline)`; share 0.2% versus 0.0% in pitivi

SIGNAL: the export CRASHED, rather than "came out not the same" — 397 tasks in kde-bugzilla, `(render|export|encod)\w*.*(crash|fail|freeze|hang|error|stuck|never finish)|(crash|fail|freeze|hang|stuck).*(render|export|encod)`; share 6.3% versus 2.5% in pitivi

Two lines are needed together, otherwise the topic is read wrongly. "Render/export" as a whole is
2311 tasks across seven trackers with a share of real fixes of 0.60 and a `gap` of 2.8
(`--themes`), but within it a crash occurs 28 times more often than a divergence:
397 against 14 in the same kde-bugzilla. From this the first conclusion: the class
"it came out wrong" is RARE by count — and that is precisely why almost nobody counts it as a subject of architecture.

The second conclusion is more important, and it is about zero. Across the seven exported trackers the filter gives
19 tasks, and 18 of them are Kdenlive (14 in kde-bugzilla + 4 in kde-invent).
The nineteenth is Olive. In pitivi 0 of 2672, in shotcut 0 of 1368, in mlt 0 of 601,
in otio 0 of 680. These are not "small trackers" and not "no such word": the same vocabulary
`differ|not match|mismatch|inconsisten` occurs in pitivi 27 times (1.0 %), in
shotcut 21 times (1.5 %), in mlt 9 times (1.5 %). The language is there, the subject is not. The class
gathered in one program — in the single one whose second executor is not one but three at once: the pre-render chunks of the preview zone, the preview scale and the proxies.
Causality cannot be proven from the headlines of seven trackers, but the coincidence is
too dense to be noise: the pain concentrates where the most paths have been split apart.

The third conclusion is about the price for those who have the class. Of the 14 kde-bugzilla tasks,
2 were closed with a real fix, another 8 were closed as `WORKSFORME`. A divergence of two
paths is unreproducible by definition: the maintainer has a different machine,
a different backend and a different preview scale, so "it all matches for me" is an
honest but useless answer.

> Kdenlive plays ca. -33db but renders -13db.
> — kde-invent #792, "AudioMixer, playback volume differs from rendering
> volume", 06.09.2020,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/792

The mechanics of the "second executor" are stated by the reporter in plain text:

> It appears that the timeline preview uses the proxy clips as source for it's
> render.
> — kde-invent #1332, "BUG timeline preview uses proxy clips as source for
> render", 10.02.2022,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1332

And exactly our remainder — an effect parameter in the pixels of the preview fraction — only for them
it is about position, not about radius:

> If I apply the filter using a preview scale of let's say 270p, when I change
> the preview back to 1:1 or render the video the filter is in a different part
> of the screen.
> — kde-invent #649, "Preview scaling bug with effects", 30.04.2020 (in the body
> of the report this is a user's quote from the forum, cited by the reporter),
> https://invent.kde.org/multimedia/kdenlive/-/work_items/649

The single case outside Kdenlive — and it is not about colour but about time:

> fade out perfectly timed in preview, but off-sync in rendered product
> — olive #130, 25.09.2018, closed completed,
> https://github.com/olive-editor/olive/issues/130

Verbatim headings of the same class, three wordings of one trouble:
"Defish effect render differs from preview" (kde-invent #308, 04.08.2019),
"The blur effect works properly on the preview but is applied wrong on the
export." (kde-bugzilla #469905, 17.05.2023, UNCONFIRMED),
"Transform effect displays on preview but does not apply in render"
(kde-bugzilla #481786, 24.02.2024, WORKSFORME).

## How it fits Kinotrain
CAPABILITY: openspec/specs/export/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Already in place: preview == export byte-for-byte on one backend; GPU against CPU — only `assert_frames_close`/`FrameTol`.
ADDRESS: src/libs/kt-engine/src/video.rs — for us this class is impossible by construction: `build_plan` builds the frame plan ONCE for the preview (`compose_from_cache_into`), for the CPU export (`compose_export_into`) and for the GPU executor (`frame_plan`), and the preview's resolution fraction enters there as the `Quality` parameter; but the remainder is possible, and nothing catches it: the FX parameters in PIXELS (`blur`/`glow`/`vhs` in `src/libs/kt-fx/src/video.rs`) are computed in the pixels of the current fraction, so on a 1/4 preview the blur covers four times the share of the frame that it does in the export — and not one test sees this, because all of them compare preview and export at the SAME `Quality`.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage of the round (a remainder, not a feature) |
| Where the main work is | kt-fx (scaling of the pixel parameters), kt-engine (pass the quality/project ratio into `VideoFxCtx`), kt-gpu (a mirrored edit of the kernels), kt-app (the backend line in the render window) |
| Project format | not touched: the parameter values stay in project pixels |
| Tests that will have to be written | "1/1 against 1/2, stretched back" within the `FrameTol` tolerance for a chain with blur and glow; a check that `GOLDEN_VIDEO` and the goldens of `video_fx.rs` are unmoved at scale 1; a layout test of the line "export: GPU/CPU" |
| Depends on | nothing; borders on F-RES-008 (what to do when the environment is absent) and F-MED-002 (proxies — if we ever introduce them, this invariant will become the main constraint on their design) |
| Breaks compatibility | no |

## Verdict

It is already there, and this is one of the few positions of the base where we are not catching up but holding
what the five core editors do not have and will never have. The note was opened not for the sake of
the verdict "we take it", but for the sake of a prohibition: any future proposal "let's speed up the preview,
it is only a display" must be rejected by a reference to invariant 7, and the price
(the preview is not faster than the export on the same frame) must be named in advance, not discovered in an optimisation round.

The remainder is to be closed: scale the FX pixel parameters by `quality / project`
the same way as `world_scale` for 3D, and add a test "the fraction does not change
the picture", which does not exist today. This does not undermine the guarantee — it is about
the same `Quality`, and in that form it is checked — but it undermines what
the user understands by it.

Boundaries: "the export crashed / the file did not come out" — not here (there the count is 28 times
larger and the mechanics are different); a missing source — F-HLP-012 and t209/t210;
the absence of FFmpeg and someone else's hardware — F-RES-008; the queue and background render —
F-EXP-002.

## Sources

- The registry and the counting method: `docs/features/_trackers.md`; each number is reproduced by
  `python3 src/scripts/tracker-rank.py --query '<filter>'`
  (export date 08.09.2026). At the time of writing seven of the twelve trackers were in the cache:
  `kde-bugzilla` (6346), `kde-invent` (2217), `pitivi` (2672),
  `shotcut` (1368), `mlt` (601), `otio` (680) — the export is complete, and `olive`
  (405 of the 1853 declared in the registry) — its export was still running, so the olive share
  is given here, but no conclusion is built on it. The other five trackers of the registry were not exported. For the divergence filter pitivi, shotcut, mlt and otio give 0
  matches, and `--query` does not print such rows at all: "0 of 2672"
  is checked by the ABSENCE of a tracker row in the output.
- https://invent.kde.org/multimedia/kdenlive/-/work_items/792 — AudioMixer,
  playback volume differs from rendering volume (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1332 — timeline
  preview uses proxy clips as source for its render (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/649 — Preview scaling
  bug with effects (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/308 — Defish effect
  render differs from preview (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/825 — Lower quality
  "preview resolution" works only on HIGH QUALITY track compositing
  (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1125 — Changing the
  Preview Resolution is distorting the clip (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=469905 — The blur effect works properly
  on the preview but is applied wrong on the export (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=481786 — Transform effect displays on
  preview but does not apply in render (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=446242 — Preview effects and Wipes seem
  fine but Rendered video is a mess, closed FIXED (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=439695 — fade transition apply in
  preview but not in render (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=376464 — JPG clip rendered as white
  screen although preview works (accessed 2026-09-08)
- https://github.com/olive-editor/olive/issues/130 — fade out perfectly timed
  in preview, but off-sync in rendered product (accessed 2026-09-08)
- The table "Who has it and how" is compiled from the vendors' documentation and the composition of the dialogs
  (Playback Resolution and Use Previews in Premiere, Timeline Proxy Mode and "force
  … highest quality" in Resolve, Better Performance/Better Quality in FCP,
  Render Settings in After Effects, Preview quality in VEGAS); not checked live — none of the five
  has a public tracker (`docs/features/_trackers.md`, "Who we do NOT take").
- Our code and tests were checked by grep over `src/` on 08.09.2026: `video.rs`,
  `video_plan.rs`, `export.rs`, `player.rs`, `kt-fx/src/video.rs`,
  `kt-gpu/src/lib.rs`, `golden_export.rs`, `parity.rs`, `gpu_render.rs`,
  `postfx_cache.rs`, `simd_parity.rs`.

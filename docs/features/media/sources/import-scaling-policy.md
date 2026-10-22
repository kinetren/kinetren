---
id: F-MED-004
title: Frame conform rule — fit, fill, stretch or leave
status: in-progress
priority: A
cost: L
category: media/sources
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: partial
crates: [kt-ffmpeg, kt-media, kt-engine, kt-fx, kt-gpu, kt-model, kt-app]
invariants: [4, 7, 8, 11]
ui_rules: [4, 9, 16, 19, 20]
round: R.62
point: null
tests: [t711, t712, t713, t714, t715, t716, t717, t718, t719, t720, t721, t722, t722b, t723, t724, t725, t726, t727, t728, t728b]
depends_on: [F-MED-003, F-EXP-001]
trackers: [kde-invent, olive, pitivi, kde-bugzilla, mlt]
checked: 2026-09-08
---

# F-MED-004 — Frame conform rule — fit, fill, stretch or leave

## Essence

A vertical 1080×1920 video, a 200×200 logo and a 4000×3000 scan get into a 1920×1080 project. For each of them the editor must decide the same thing: **fit** with margins, **fill** with cropping, **stretch** to the edges, or **leave** at the source pixels as they are. The decision is taken at the moment of placement on the timeline, then lives silently and is discovered at export. Each of the four answers is right in its own case, so the question of the note is not "can we scale", but **whose choice it is and where it is recorded**.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Event Pan/Crop → Source: "Stretch to fill frame", "Maintain aspect ratio"; in the context menu "Match Output Aspect" / "Match Source Aspect" | the rule lives in the EVENT and is edited in a separate window per clip. "Stretch to fill frame = No" — "the media retains its original size, and the area outside the selection box is cropped"; "Maintain aspect ratio = No" — "the source media's ratio of height to width is not preserved when video is stretched in the output frame". On fitting the frame the help speaks directly: "Matching the output aspect ratio can prevent black bars from appearing when you use source media (such as photographs) that does not match your project's aspect ratio" |
| Adobe Premiere | 26 | Preferences → Media → **Default Media Scaling**: None / Set to Frame Size / Scale to Frame Size; the same two items — in the clip's context menu | the rule is a setting of the PROGRAM, not of the project or the clip, and it acts only on what is imported AFTER its change; what is already placed is not touched. "Set" adjusts the Motion → Scale parameter and keeps the original resolution of the file, "Scale" rasterises the file to the sequence frame (an analysis of irreversibility — F-MED-003) |
| DaVinci Resolve | 21 | Project Settings → Image Scaling → **Mismatched Resolution Files** (Scale entire image to fit / Center crop with no resizing / Scale full frame with crop / Stretch frame to all corners), plus separate Input Scaling and Output Scaling | the rule belongs to the PROJECT, is switched at any moment and applies to all material retroactively. The default is "Scale entire image to fit", that is, margins. The analysis of the two-stage nature (input/output) and of the fact that the Deliver preset knows nothing about them — F-EXP-001 |
| Final Cut Pro | 12.3 | **Spatial Conform**: Fit / Fill / None (Video inspector) | the rule belongs to the CLIP. Fit — "The default setting. Fits the clip within the project's frame size without cropping the clip's video"; None — "Leaves the clip's frame size unchanged". The section is not shown at all when there is nothing to decide: "If the clip you selected has the same frame size (resolution) as the project, the Spatial Conform section does not appear" |
| After Effects | 26 | nothing by default; Layer → Transform → **Fit to Comp** (Ctrl+Alt+F), Fit to Comp Width (Ctrl+Alt+Shift+H) / Height (Ctrl+Alt+Shift+G) | the only one of the five whose default is "leave as it is": a layer enters the composition at its own size and may stick out beyond the frame. Moreover, the standard "Fit to Comp" really STRETCHES (proportions are not kept), while the commands by width and by height keep them — that is, in AE "fit" and "do not distort" are separated onto different keys |

**Outside the core.** Without Kdenlive one cannot describe the subject itself: the live request for which the note was opened, and its six-year fate, are visible only there.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08 | Settings → Configure Kdenlive → Misc: "Keep clip original frame size on import" and "Check if first added clip matches project profile" | the rule of conforming belongs not to the editor but to the project profile in the MLT framework: a clip is conformed to the profile, and there is no level of its own "a rule per clip" in the model. The answer to a mismatch is to change the PROJECT: "If they differ, Kdenlive asks if the project settings should be adjusted to match the values of the clip". The option "keep the original size" appeared later and, by the maintainer's admission, merely adds the Transform effect to the clip (quotes below) |

## The trick behind it

**1. The level at which the rule lives differs for everyone — and it is that, not the set of variants, that determines what happens next.** The sets nearly coincide
(fit / fill / stretch / leave), but the address of the rule does not: in Resolve it is the PROJECT, in FCP the CLIP, in Premiere a PROGRAM SETTING, in Kdenlive the profile in the framework. Hence three different behaviours that are in none of the feature lists. In Resolve changing the switch rewrites the geometry of all the already assembled edit retroactively. In Premiere changing the default does not act on what already stands on the timeline — it is corrected only one by one, so "I changed the setting" and "mine is still the old way" are both true at once. In FCP there is no project rule at all: two hundred clips — two hundred values of Spatial Conform, and Olive #1106 asks exactly for a rescue from this ("Because then I have to reset each
individual clip to fit the sequence change"). The question "why are there bars" has three different addresses for the answer, and a person who came from another editor looks for it in the wrong place.

**2. "Fit" puts BLACK into the frame, and black is opaque — therefore a rule that is safe on the lower layer is destructive on the upper one.** Pitivi #1378: a 4:3 picture over a 16:9 video, fitting added margins — and the margins covered the lower layer. "You
end up with black bars on the left and right side of the 4:3 image, and you can't
see the 16:9 video below it." It follows what the help pages do not write: a conforming rule has two different correct executions. For the lower layer, filling the margins with black is the picture itself; for any layer above, the filling must be TRANSPARENT, otherwise "fit" turns into "erase everything beneath it". It also follows clearly why "leave as it is" (AE, Premiere None) is not the absence of a rule but an honest fourth rule: it is the only one that does not invent pixels beyond the material.

**3. The request "do not touch my size" is satisfied by an option that does not do what its label says — and this is not carelessness but a consequence of the level.** kde-invent
#538 ("Don't resize image, image sequences or videos when importing", 2020) lived six years with the `upstream` label and was closed in January 2026. By then the option "Keep clip original frame size on import" already existed, and in the KDE Discuss it turned out that it does not change the conforming rule but adds the Transform effect to the clip: the maintainer agreed with this publicly ("So, I stand corrected…"). The level remained the same — the project profile — and the option could reach only the clip. The same mechanism is visible in MLT #521 ("Scaling regression (affine / qtblend transitions) on different aspect ratio"): when the conforming rule lives in the framework, the editor fixes it with composite filters, while the name of the setting promises a level it does not touch.

## Pros

- A rule at the PROJECT level (Resolve) — one switch conforms all mixed material at once and works retroactively: a decision taken on the first day does not become a sentence for the whole edit.
- A rule at the CLIP level (FCP) is the only way to express a mixed project where the logo needs "leave" and the panorama needs "fill": two neighbouring events honestly receive different answers.
- A control that appears only on a mismatch (FCP) both takes no space and REPORTS a fact: if the Spatial Conform section is visible, the sizes have diverged, and this is seen before the export.
- "Leave as it is" (AE by default, Premiere None, Premiere "Set to Frame Size") preserves the source pixels: a 4000 px scan in 1080p can later be pushed in with a camera move without soap — the material is enlarged, not the result of conforming.
- Separating "fit" and "do not distort" into different commands (AE: Fit to Comp versus Fit to Comp Width/Height) makes distortion an EXPLICIT choice, not a side effect of a harmlessly sounding item.

## Cons

- A rule at the clip level without a rule at the project level — two hundred clips, two hundred edits; changing the sequence resolution retroactively requires going through each one (Olive #1106).
- A rule at the project level, on the contrary, rewrites frames already built: framings and push-ins set by hand mean something other than what they meant after the switch.
- The default is one for all material, while material is different: "fit" is safe for video and destructive for graphics, so an editor with a single default is guaranteed to get a request of the kind #538 — not because the default is bad, but because there is only one.
- Rasterising to the frame (Premiere "Scale to Frame Size") is irreversible: the file physically becomes 1080p, and further enlargement makes it soft (F-MED-003).
- Fitting with black breaks a multi-layer composite (Pitivi #1378), and transparent filling requires the layer to have alpha — that is, the conforming rule cannot be decided apart from the interpretation of alpha (F-MED-003).
- The name of the option promises a level that it does not reach (Kdenlive "Keep clip original frame size on import" = the Transform effect on the clip).

## What users say

> When importing data into the timeline, it is resized to whatever is the video
> size. This is very bad default.
> — kde-invent #538, 24.01.2020, https://invent.kde.org/multimedia/kdenlive/-/work_items/538

> Of course, there could be a feature to "fit" the input to the output, but this
> should not be the default, because that's basically destructive.
> — same source, same author, 24.01.2020

> You end up with black bars on the left and right side of the 4:3 image, and
> you can't see the 16:9 video below it.
> — Jeff F.T., Pitivi #1378, 30.10.2013, https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1378

> That setting does not prevent this scaling when you drop the clip on the
> timeline. In fact, that option merely adds a Transform effect to the clip.
> — Fml, KDE Discuss, 28.06.2025, https://discuss.kde.org/t/keep-clip-original-frame-size-on-import-is-misleading/36286

> So, I stand corrected: Kdenlive does indeed add a Transform effect when
> enabling the Keep original size option.
> — berndmj (maintainer), ibid., 29.06.2025

> Because then I have to reset each individual clip to fit the sequence change.
> — Olive #1106, 30.09.2019, https://github.com/olive-editor/olive/issues/1106

SIGNAL: the frame conforming rule — 5 tasks in olive, `(resiz|rescal|scal|stretch|squash|distort|letterbox|pillarbox|black bar)\w*[^.]{0,45}(frame size|project (profile|resolution|size)|sequence|profile|canvas|aspect ratio)|(aspect ratio|frame size)[^.]{0,45}(resiz|rescal|scal|stretch|letterbox|black bar)|(don.?t|do not|without) (resiz|rescal|scal|stretch)|auto.?(scale|resize|fit)`; share 0.3% versus 0.1% in kde-invent

SIGNAL: the aspect ratio as an independent topic — 18 tasks in pitivi, `aspect ratio|anamorphic|pixel aspect|square pixels?`; share 0.7% versus 0.1% in olive

Read the first line like this: the topic is small EVERYWHERE (0.0–0.3% across eight trackers), and this is itself a finding — the conforming rule does not generate a stream of tasks, because it is silent: a person does not write "the editor stretched my clip", he writes "bad quality" or writes nothing. The three-times denser share at Olive (5 out of 1853 versus 2 out of 2217 at kde-invent) is explained not by Olive being worse, but by the rule there being EXPLICIT and named ("auto-scale mode") — a named mechanism acquires tasks, a nameless one does not. Of Olive's five tasks, four are on topic; the fifth (#1938, "resize clip from the sequence viewer") is filter noise.

The second line, about the neighbouring fork, measures the same thing from the other side: in Pitivi, which honestly carries the pixel aspect ratio through the whole GStreamer pipeline, the "aspect ratio" topic is six times denser than in Olive, which brings everything to the sequence frame. The visibility of a mechanism costs tasks in the tracker; its invisibility costs wrong frames that nobody counts.

## How it fits Kinotrain

### What already exists

Measured on 08.09.2026 by running, not by reading the code: the built
`src/target/debug/kt-cli export`, the sources generated by the system `ffmpeg`, the geometry of a white rectangle in the output frame measured from the PNG.

- **We DO have a conforming rule, it is "stretch", and the decoder chose it.** A source 360×640 (vertical) with a 200×200 square in a 1280×720 project gives on output a rectangle **715×224** — the square is squashed to 3.2 : 1, there are no bars, no cropping. This is exactly `to_rgba` (`src/libs/kt-ffmpeg/src/source_impl.rs`, line 904): `sws_getContext(f.width, f.height, …, dst_w, dst_h, …)`, where `dst_w`/`dst_h` is the requested `Quality`; the aspect ratio does not enter the call. The finding coincides with what F-EXP-001 already recorded from the export side — there is also the conclusion that the item "Max (W×H)" (`max_source_resolution`, `src/apps/kt-app/src/app.rs`, line 1693) adds the maxima of width and height independently and offers a resolution that no source has.
- **Material SMALLER than the project is stretched too.** A source 240×240 with a 120×120 square in the same project: the square became **639×360**, that is, the picture is not "placed as it is" but stretched to the edges of the frame — the logo scenario from request #538 is reproduced here literally.
- **None of our controls changes this — measured.** `Pan` with the `mode` `fit`, `fill` and `stretch` gave THREE BYTE-FOR-BYTE IDENTICAL frames, coinciding with the frame without `Pan` at all (one md5 for four files). The reason is in `pan_frame` (`src/libs/kt-fx/src/video.rs`, line 172): the frame is computed in the coordinates of the SOURCE, but before the FX the source is already brought to the size of `Quality`, so `sw/sh == ar_out`, and all three branches (lines 189–197) return `(sw, sh)`. F-UI-001 found that `Fit` and `Fill` compute a bit-identical frame; the measurement adds a third to this: in our pipeline ALL THREE MODES are one mode, and the combo of three items in the FX editor (`src/apps/kt-app/src/fx_editor.rs`, line 597) does nothing. That the FX is alive is verified in the same place: `zoom = 2` does change the picture (715×224 → 1280×450).
- **The backend has nothing to do with it.** The same measurement with `KT_VIDEO_GPU=0` gives the same 715×224: the decision is taken before `frame_plan`, so the GPU and CPU executors equally receive the already stretched frame. Invariant 7 is formally not violated — preview and export agree, they just agree on the wrong rule.
- **The pixel aspect ratio is not read at all.** Two files of 640×480 with identical pixels and a different SAR (1:1 → DAR 4:3 and 2:1 → DAR 8:3) export to a **byte-for-byte identical** frame (one md5). Anamorphic is inexpressible for us: in `ProjectSettings` (`src/libs/kt-model/src/project.rs`, line 1769) there are only `fps`/`width`/`height`/`sample_rate`, and in `Quality` (`src/libs/kt-media/src/lib.rs`) only two numbers.
- **The size of the source we know and store.** `MediaInfo.width/height` travel through the probe and the sidecar index `<project>.ktindex.db` (`src/libs/kt-media/src/index.rs`), that is, the data for any conforming rule is sufficient — it is simply that nobody asks for it at placement: `place_media_at` (`src/apps/kt-app/src/app.rs`, line 3166) reads from `info` only the duration and the presence of tracks.

### What is missing

- **The very concept of a "conforming rule" — at no level.** Neither a field in `ProjectSettings`, nor a field in `ClipEvent` (`src/libs/kt-model/src/project.rs`), nor an item in the UI. Others have this choice in three different places; we have it in none, and the person does not even know that the choice was made for him.
- **Transparent filling.** Our composite `alpha_over_scalar` (`src/libs/kt-engine/src/lib.rs`) handles straight alpha, but the frame from the decoder arrives with `has_alpha: false` hard-coded (F-MED-003), so "fit" today could give only BLACK margins — the very error of Pitivi #1378 on the upper layers. The conforming rule and alpha are linked and cannot be closed separately.
- **An honest place for the frame.** We have no geometry of a layer: a clip has neither a size nor a position in the frame — there is only `Pan`, which is a crop frame rather than a transformation. Therefore "leave as it is" (material larger than the frame sticks out beyond the edge) is in principle inexpressible today.
- **Thumbnails are conformed by the same silent rule.** `THUMB_W`/`THUMB_H` = 320×180 (`src/apps/kt-app/src/thumbs.rs`, line 18), the frame is requested of exactly this size, so the filmstrip of a vertical clip is squashed too — and this is the first place where the user could see the problem, but sees it as "that is how it should be".

### What gets in the way

**1. The contract "the decoder delivers exactly Quality" is built into the plan executor.** `execute_plan_cpu` (`src/libs/kt-engine/src/video.rs`, line 610) sets up `fits` and SILENTLY skips a layer whose frame did not match the size of the plan — with the comment "the contract 'the decoder delivers Quality'". Any rule that preserves proportions immediately violates this contract: a 1080×1920 frame, conformed honestly, is not equal to 1920×1080. That is, the conforming rule cannot be "added on the side" — one will have to introduce the geometry of the layer into the plan and move the resampling from the decoder into `frame_plan`.

**2. The resampler today is one, and it is the cheapest.** `SWS_FAST_BILINEAR` (`src/libs/kt-ffmpeg/src/source_impl.rs`, line 929) — the same for both preview and export. As soon as conforming becomes a choice, the question of the quality of this choice immediately arises, which does not exist now.

**3. `Quality` is part of the cache key (invariant 4).** Today the key describes both the request and the result, because they coincide. If the decoder starts delivering a frame of the source proportions, "warmth" will stop mapping unambiguously onto the output size — one will have to decide what exactly is cached.

**4. The CPU + WGSL pair.** The geometry of a layer is invariant 7 in the edition of R.29: `frame_plan` is one, the executors are two, conformance is held by t181/t182, the byte equality of preview and export by t11/t67 and t184. Any new frame must arrive at once as a pair and at once with conformance, otherwise the GPU and the CPU will diverge.

### Which invariants it touches

- **7** (the composition code is one) — the main one: the conforming rule must live in `frame_plan` and be computed identically on the CPU and the GPU. Today it lives in the decoder, that is, OUTSIDE the single code — formally the invariant is intact (both executors receive the already conformed frame), in fact the decision is taken where nobody looks for it.
- **11** (determinism) — changing the rule changes the bytes: GOLDEN_VIDEO will have to be recalculated deliberately, not "it drifted".
- **4** (the cache is the single source of truth about coverage) — see "What gets in the way", item 3.
- **8** (scrolling does not generate decoding) — thumbnails are conformed by the same rule and live in the sidecar DB; changing the rule invalidates the strip.
- UI rules: **4** (the default must be useful — today's default "stretch" is not useful for any kind of material), **9** (nothing superfluous: a combo of three modes that does nothing is exactly that superfluous widget), **16** (impossibility = an error: today a frame mismatch is not reported in any way), **19** (say where to change it back), **20** (nothing grows by itself: if the rule becomes a property of the clip, it must not silently change when the project resolution changes).

ADDRESS: src/libs/kt-ffmpeg/src/source_impl.rs — the class "the preview diverged from the export" is impossible for us, because both call one `frame_plan` (invariant 7); but the class "the conforming rule was chosen silently and wrongly" is possible, and nothing catches it: `to_rgba` (line 904) scales to `dst_w × dst_h` without the aspect ratio, `execute_plan_cpu` (line 610) silently skips a layer of a different size, and none of the 221 regression tests feeds in material whose proportions do not match the project — all golds are taken on matching frames, so the stretching passes green. The three modes of the `PanMode` combo, meanwhile, give a byte-for-byte identical result (measured), that is, the only control visible to the user on this topic is dead.

## Estimate

| What | Answer |
|---|---|
| Scope | L — a whole round: (1) the geometry of the layer in the model and in `frame_plan`; (2) moving the resampling from the decoder into the plan, a CPU + WGSL pair; (3) the level of the rule (the project + an exception on the clip) and the UI |
| Where the main work is | kt-engine (`video.rs`, `frame_plan` and the `fits` contract), kt-fx (`pan_frame` stops being the only geometry), kt-gpu (the mirror in WGSL), kt-ffmpeg (the decoder delivers the source size or an explicitly conformed one), kt-model (the rule field), kt-app (the default, display of the mismatch) |
| Project format | a new version is needed: the rule on the project is a field of `ProjectSettings`, the exception on the clip is a field of `ClipEvent`; both must have a serde default, so that old files are read as "stretch" and do not drift silently |
| Tests to write | conforming a vertical source into a horizontal project by each of the four rules (a geometry measurement, as in this note); CPU↔GPU conformance of the conforming (after the pattern of t181/t182); "preview == export" on a non-matching frame (t11/t67, t184); unchanged GOLDEN_VIDEO under the rule "stretch" — old projects must render byte for byte as before; SAR/anamorphic — a separate test, if we take it |
| Depends on | F-MED-003 (alpha: without it "fit" can only produce black margins), F-EXP-001 (the same fork from the output side; the conclusion there is that no one has a conforming rule in the preset) |
| Breaks compatibility | no, if the default for files without the field is today's "stretch"; any other default changes the picture of existing projects |

## Verdict

**We take it, priority A, and we take it as the geometry of a layer, not as a checkbox.** The measurement shows not "no feature", but a wrong result on correct input: vertical video is squashed (715×224 from a square), the logo is stretched to the edges, and there is nothing inside the program to get around it — the three `PanMode` modes give one byte-for-byte frame. Material of this kind is today more common than "correct" material, so this is not a niche gap.

The order is reasonable. First — to SAY (UI rule 16): the frame mismatch is known at the moment of placement (`MediaInfo.width/height` are already read) and today is not reported in any way; this is cheap and removes the worst part — the silence. Then — the geometry of the layer in `frame_plan` as a CPU + WGSL pair, and together with it the transparent filling, otherwise "fit" will repeat the error of Pitivi #1378 on the upper layers. And only then — the level: the rule on the PROJECT with an exception on the CLIP, exactly the pair whose absence produced both request #538 and the option that did not carry it out. The dead three-item `PanMode` combo cannot be fixed until then: it will become meaningful exactly when the frame of the source proportions begins to reach the FX before them.

Keeping "stretch" as the default for old files is mandatory — otherwise existing projects will drift silently (UI rule 20).

## Sources

- https://invent.kde.org/multimedia/kdenlive/-/work_items/538 — live request
  «Don't resize image, image sequences or videos when importing», 24.01.2020;
  6 votes, 20 comments, label `upstream`, closed 13.01.2026 (access date
  2026-09-08, text taken from a tracker dump)
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1378 — «Compositing /
  mixing videos with different aspect ratios results in letterboxing (black
  bars)», 30.10.2013 (2026-09-08)
- https://github.com/olive-editor/olive/issues/1106 — «Auto-Scale check in
  sequence preference», 30.09.2019 (2026-09-08)
- https://github.com/mltframework/mlt/issues/521 — «Scaling regression (affine
  / qtblend transitions) on different aspect ratio» (2026-09-08)
- https://discuss.kde.org/t/keep-clip-original-frame-size-on-import-is-misleading/36286
  — «"Keep clip original frame size on import" is misleading», 28–29.06.2025,
  Kdenlive 25.04.2 (2026-09-08)
- https://docs.kdenlive.org/en/getting_started/configure_kdenlive/configuration_misc.html
  — «Check if first added clip matches project profile» (2026-09-08)
- https://support.apple.com/guide/final-cut-pro/conform-frame-sizes-and-rates-ver3363b44e/mac
  — Spatial Conform: Fit / Fill / None, default and the condition for showing the section
  (2026-09-08)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/2-window/pancropwindow.htm
  — VEGAS Pan/Crop: "Stretch to fill frame", "Maintain aspect ratio", Match
  Output/Source Aspect (2026-09-08; help page for version 22, not re-checked
  against the primary source for 2026)
- Premiere "Default Media Scaling" (None / Set to Frame Size / Scale to Frame
  Size) and the fact that changing the default does not affect what is already placed:
  https://larryjordan.com/articles/a-premiere-preference-that-makes-a-difference-set-vs-scale/
  and the official Adobe community (2026-09-08). **NOT VERIFIED against the primary source:**
  `helpx.adobe.com` still does not open from this environment (the same limitation is
  recorded in the first wave in `_editors.md`)
- After Effects, Fit to Comp (Ctrl+Alt+F) versus Fit to Comp Width/Height:
  the official Adobe community and Creative COW (2026-09-08). **NOT VERIFIED against the primary source** — same
  reason
- DaVinci Resolve, Image Scaling → Mismatched Resolution Files: the set of options
  and the default "Scale entire image to fit" are compiled from secondary sources and from the
  Blackmagic forum (2026-09-08); **the manual page did not open from this environment**, the
  analysis of the two-stage behavior is taken from F-EXP-001
- The `SIGNAL:` line counts are reproduced with `python3
  src/scripts/tracker-rank.py --query '<filter>'` on dumps from 08.09.2026.
  At the time of the run the export was still in progress: `blender-vse`, `ffmpeg-trac` and `openshot`
  were empty or incomplete, so the comparison is built on `olive`, `kde-invent`
  and `pitivi`, whose volumes matched the estimate in the registry `_trackers.md`
- Measurements of our behavior — 08.09.2026, `src/target/debug/kt-cli export` on
  synthetic input from the system `ffmpeg`; the input, the output and the measured geometry are described in
  the section "What already exists"

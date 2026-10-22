---
id: F-MPH-004
title: Adjustment Layer in Adobe Premiere
status: researched
priority: B
cost: XL
category: morpher/analogues
editors: [premiere-26, vegas-pro-2026, resolve-20, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-fx, kt-app]
invariants: [7, 11]
ui_rules: [13, 17]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MPH-004 — Adjustment Layer in Adobe Premiere

## Essence

The editor creates an empty "correction layer" in the bin, puts it on the top track and
hangs effects on it — grade, blur, grain, vignette. Everything that lies BELOW it on the tracks and falls under
the block by time is shown through these effects. The length of the block is the length of the action: stretch it — and it covers the next three cuts; cut it — and
half the scene is left without the grade. The layer behaves like an ordinary clip: trim,
split, copy-paste, opacity, blend mode, keyframes.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Adobe Premiere | 26.x | Adjustment Layer | a PROJECT ELEMENT in the bin with its own frame size and PAR; on a track it is an ordinary clip, the effects are computed on the mixed picture of the tracks below |
| VEGAS Pro | 2026 | Adjustment Event | an event on a track, acts on the composite of the lower ones; has an opacity envelope and fades at the edges |
| DaVinci Resolve | 20 | Adjustment Clip | taken from Effects Library → Toolbox, edited in the Inspector and on the Color page |
| Final Cut Pro | 12 | Adjustment Clip | built in since 11.1 (Edit → Add Adjustment Clip, ⌥A, default 10 s); before that — a homemade generator from Motion |
| After Effects | 26 | Adjustment Layer | a layer without content; effects are applied to the result of the composition of the layers below |

## The trick behind it

**1. The output of the layer is composited over exactly the picture that the layer consumed.** Three seemingly unrelated
behaviours follow from this one fact. (a) Anything that changes only the ALPHA is meaningless on the layer: Luma/Color/Ultra Key honestly punches
a hole, but in the hole the very same frame is visible — and premultiplied black
bleeds in at the edges, and instead of transparency the user sees "the dark became
even darker" (the thread below, 2024, lives on without a fix). (b) The opacity of the layer
works for free as the strength slider of the effect: 60 % is a crossfade of the processed
picture with the unprocessed one; Adobe did not need a separate "strength" parameter.
(c) An empty layer with the Overlay blend mode changes the picture, although there is
nothing inside it — the frame blends with itself. Exactly the same hole as (a), only
with the opposite sign.

**2. It is a project element with its own frame size, not a service marker.**
File → New → Adjustment Layer asks for width, height, timebase and PAR (by default — from the current sequence). The layer lives in the bin on a par with the media, so it is carried by copy-paste
into a sequence of another resolution — and then the effect covers
not the whole frame: people get an "unpainted strip on the right". It is cured by "Scale to
Frame Size" on the clip or by a new layer under the new sequence; the layer itself does not
adapt to the sequence.

**3. The mask belongs to the EFFECT, not to the clip.** In Premiere 26 a mask is created
"unattached" and dragged onto Opacity or onto any effect in Effect
Controls. On an adjustment layer this difference is fatal: a mask put on the
Opacity of the layer does not limit the area of its effects — it makes a hole in the output of the layer,
where the same frame is visible (the mechanism of item 1). Only a mask lying
INSIDE the effect limits it. The new AI Object Mask and tracking "up to 20× faster" from 26.0 live in
the same scheme — they speed up the path "mask on the effect", they do not cancel it.

## Pros

- The layer sees the mixed frame, so blur and grade act at the JOINT of the sources: a vignette lies over the frame
  as a whole, not separately on each layer, and on a crossfade one cannot see that there are two different materials underneath.
- Opacity and blend mode work as the strength and character of the mix without
  a single additional parameter — this is a consequence of the layer being composited
  by the ordinary route.
- The layer is an ordinary clip, so trim, split, copy-paste, keyframes, mute and nesting already work
  with the same code; Adobe did not write a separate UI for the "correction layer".
- Layers stack: grade → blur → vignette on three tracks, the order is set by the tracks,
  not by a list inside one window.
- The scope of action is the geometry of the block, so "extend the effect by two more
  cuts" is a scissors gesture, not an edit of a target list.

## Cons

- Any effect that produces alpha gives no visible result on the layer:
  keying, a mask on Opacity, alpha effects. The only sanctioned workaround is
  nesting, that is, changing the render order at the price of the flatness of the edit.
- Its own frame size, not inherited from the sequence: a layer moved into a project
  of another resolution silently stops covering the whole frame. The error is visible to the eye,
  not in a message.
- The layer does not know WHAT is under it, only "the picture". Warp Stabilizer on a layer above a
  retimed clip is not analysed: tracking requires unchanged frames, and the layer does not pass them on.
- The workaround (nesting) breaks the next layer itself: inside the nest the
  adjustment layer sees only the tracks of that nest, outside the nest is one clip, and the
  decomposition of the edit stops being flat.
- The transfer to After Effects is one-way: a layer with effects goes from Premiere to
  AE, but an adjustment layer cannot be copied back from AE to Premiere (on a par with
  shape/text/camera/light).

## What users say

> I tried to use an adjustment layer (over a couple of clips) that contains some
> effects and I wanted to use a Luma Key on this adjustment layer so the darker
> tones would become transparent, but it doesn't work.
> — Kata Lin, Adobe Community, 2 February 2024,
> https://community.adobe.com/t5/premiere-pro-discussions/luma-key-or-color-key-or-ultra-key-not-working-on-adjustment-layers-bug-or-limitation/td-p/14397400

Adobe's answer in the same thread — not a fix but a change of the render route:

> You may need to reroute the render order pipeline via nesting.
> — Kevin J. Monahan, Adobe staff, 2 February 2024, same thread

The author declined: in his words, giving up adjustment layers "massively complicate
my workflow". The thread has been open since 2024, the behaviour in 26.x is unchanged.

The community expert formulates the same thing about masks — the mask must be on the
effect, otherwise the layer paints everything:

> unless of course there is a mask _on the effect_ in the AL
> — R Neil Haugen, Adobe Community, 16 April 2024,
> https://community.adobe.com/t5/premiere-pro-discussions/adjustment-layer-in-premiere-blend-mode-issues/m-p/14558936

## How it fits Kinotrain

### What already exists

- `MorpherEvent` with `MorphTargets::Below` in
  `src/libs/kt-model/src/project.rs` — a direct analogue in intent: "everything below the
  line, within the block window". The blending is computed bottom to top, like adjustment layers in
  AE (`Project::effective_clips()`, same place).
- **The construction is different, and this is fundamental.** `MorphPatch::apply_to()`
  (`project.rs`, ~line 832) appends effects to the chain of EACH target clip, rather than processing the composite. A partially covered target is cut into pieces.
  The morpher has no frame size at all: its structure has only `lane/start/duration/targets/patch`,
  therefore the whole class of defects "a layer of the wrong width" is impossible for us by
  construction, and copy-pasting a morpher into a project of another resolution is safe.
- **Keying from the morpher works for us** — verified by the code, not by analogy:
  `greenscreen()` in `src/libs/kt-fx/src/video.rs` (~line 348) sets `px[3] = 0`
  and `has_alpha = true`; the composite `alpha_over()` in
  `src/libs/kt-engine/src/video.rs` (~line 767) skips such a pixel, and
  the layer BELOW on the tracks is visible. So Premiere's complaint does not apply to us: for them the
  hole reveals the same picture, for us — the next layer. The cost is symmetric:
  our key cuts each clip separately, not the mixed frame.
- The master switch of the event chain does not turn off the layer's effects: `apply_to()` with
  `c.fx_off` clears ITS OWN clip chain and appends the morpher one
  (`project.rs`, ~line 850); fixed by test t214
  `src/libs/kt-model/tests/fx_auto.rs`.
- FX are computed per layer, the composite comes after them: the chain lives in `LayerSrc.fx`
  (`src/libs/kt-engine/src/video_plan.rs`), and "post-FX" in the cache means "after the
  chain of the LAYER", not "after the composite".
- The layer's UI — `src/apps/kt-app/src/morpher_editor.rs`, a single chain strip
  (t217).

### What is missing

- ~~Blend strength~~ **done in R.38 item 12** (`docs/porting-notes.md`, item 267):
  `MorpherEvent.strength` + `fade_in`/`fade_out`, and the mechanism is exactly the one
  described above in "The trick behind it": a crossfade of the processed picture with
  the unprocessed one. It is implemented as a CHAIN SLOT (`FxInstance::LayerMix`), therefore
  automation, the post-FX cache key and copy-paste came to it without separate
  code. The mixing goes with the state BEFORE the layer's contribution (the `from` field), not with the input of
  the whole chain — otherwise the strength would also roll back the event's own effects.
- Processing of the MIXED picture: there is no post-composite step in the plan,
  `PlanLayer` describes sources and transitions, but not a "step over the result".
- Masks and tracking: a search over `src/libs/kt-fx/src` and `src/libs/kt-model/src` finds not a single `mask` — neither on a clip nor on an effect.
- ~~Automation inside the patch~~ **done in R.38 item 13** (item 266):
  `MorphPatch.video_auto`/`audio_auto`, the domain is the time of the BLOCK (the layer has no material), `apply_to` converts the keys to the material of the target.
- We have no nested compositions (`docs/porting-notes.md`, item 158: "there we have a void … until nested compositions are supported"), so Adobe's workaround "reorder the render by nesting" is unavailable to us — the solution must work without it.

### What gets in the way

The main emphasis is invariant 7. The post-composite step must live inside
`VideoRenderer::frame_plan` and be executed by BOTH backends, otherwise a
second composition path appears: so a step in the plan, a mirror in the shader and CPU↔GPU
conformance. The strength slider on our model is not "the alpha of the layer" but a mixing of the pre-FX and
post-FX frame PER LAYER: this is a new operation and a second frame in memory for each
layer, and moreover the post-FX cache key is computed from the hash of the chain
(`src/libs/kt-engine/src/video.rs`, ~line 99), which would have to be extended
with the strength, otherwise 60 % and 100 % would collapse into one frame. **It went differently** (R.38
item 267): the strength became a CHAIN SLOT, so it entered the hash by itself, the mirror
became a separate pass `layer_mix.wgsl` (the same integer formula as
`dissolve_into`), and the second frame lives exactly where the slot stands — the snapshot
is taken at the boundary of `from`, not at every layer. Masks "on the effect" after the
Premiere 26 pattern are a field of `FxInstance`, not of the clip, that is, a change of the format and
of all 10 video FX at once.

### Which invariants it touches

Root ones: 7 (the composition code is one — any post-composite step and any strength slider), 11 (determinism: the new step must be byte-for-byte on the CPU and match
the GPU within tolerances). UI: 13 (the strength slider must sit in the already occupied place in
`morpher_editor.rs`, not in a new strip), 17 (the strength set from the window and
by a gesture on the block must behave the same way).

## Estimate

| What | Answer |
|---|---|
| Size | XL — the post-composite step changes the frame plan and the format |
| Where the main work is | kt-engine (`video_plan.rs`, `video.rs`), kt-gpu (the shader), kt-model (the patch schema) |
| Project format | a new version is needed: the strength field in `MorphPatch` + a sign of "layer over the composite" |
| Tests that will have to be written | a model round-trip and reading of old files; CPU↔GPU conformance of the new step; preview == export; strength 0/50/100 % against bake; a regression "keying from the morpher still reveals the layer below" |
| Depends on | — |
| Breaks compatibility | no: the default "strength 100 %, no step" reads old files as they are |

## Verdict

A layer as a PROJECT ELEMENT with its own frame size we do not take: it is the source of a
whole class of Premiere defects that we do not have by construction, and there is no reason to
change an event on a track into a bin object. We take two things selectively: the blend-strength
slider (the discrepancy with Vegas, item 3 of R.36) and — as a separate note — the question
of a step ABOVE the composite, without which a layer's grade on a crossfade is computed twice.
The main lesson is negative: their keying on a layer does not work because the output of the layer
lands on the same picture; our per-layer scheme does not have this hole, and it must not be lost
when moving to a post-composite step — this needs a regression test.

## Sources

- https://community.adobe.com/t5/premiere-pro-discussions/luma-key-or-color-key-or-ultra-key-not-working-on-adjustment-layers-bug-or-limitation/td-p/14397400 (access date 2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/adjustment-layer-in-premiere-blend-mode-issues/m-p/14558936 (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/adjustment-layer-not-covering-full-video/td-p/10839862 (2026-09-07)
- https://community.adobe.com/bug-reports-733/warp-stabilizer-inop-in-adjustment-layer-906424 (2026-09-07)
- https://helpx.adobe.com/premiere/desktop/add-video-effects/apply-video-effects/create-adjustment-layers.html — creating the layer and the Video Settings dialog (per the documentation, not checked live)
- https://helpx.adobe.com/premiere/desktop/add-video-effects/apply-video-effects/resize-adjustment-layers.html — layer size against sequence size (per the documentation)
- https://helpx.adobe.com/in/premiere/desktop/use-premiere-with-other-apps/working-with-other-adobe-applications/copy-from-after-effects-to-premiere.html — which layers are NOT copied from AE to Premiere (per the documentation)
- https://www.provideocoalition.com/tool-tip-tuesday-for-adobe-premiere-pro-2026-mask-tool-changes-to-know/ — masks in 26.0: the "unattached" mask, dragging onto Opacity or onto an effect (2026-09-07)
- https://larryjordan.com/articles/adobe-updates-and-modifies-masking-in-premiere-26/ — Object Mask and the restriction "one mask per effect" (2026-09-07)
- https://petapixel.com/2026/01/20/rebranded-adobe-premiere-26-arrives-with-one-click-object-tracking/ — product renaming and tracking up to 20× faster (2026-09-07)
- https://www.cined.com/apple-final-cut-pro-11-1-introduces-adjustment-clips-and-enhances-magnetic-masks/ — Adjustment Clip built into FCP since 11.1 (2026-09-07)
- https://www.ledet.com/davinci-resolve-tip-of-the-week-use-adjustment-clips-for-quick-non-destructive-edits/ — Adjustment Clip in Resolve (2026-09-07)

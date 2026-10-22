---
id: F-MPH-008
title: Automation (curves) on the morpher layer itself
status: candidate
priority: B
cost: XL
category: morpher/gaps
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-otio, kt-engine, kt-app]
invariants: [3, 7, 11]
ui_rules: [13, 16, 17, 20]
round: null
point: null
tests: []
depends_on: [F-MPH-006]
checked: 2026-09-07
---

# F-MPH-008 — Automation (curves) on the morpher layer itself

## Essence

The morpher block acts exactly as it was set up: one and the same value of each parameter from the first frame of the block to the last. The editor needs something else — a grade that GROWS towards the end of the scene; a blur that fires on a flash and then drifts away; a move between two images within one layer. Today the only way is to cut the block into pieces and set the values stepwise: on a long scene that is a dozen blocks instead of one curve, and a step is visible at every joint.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Adjustment Event | the FX keys and the corner fades belong to the EVENT, the domain is the event's time; the composite envelope lives on the track in project time, and whether it moves with the event is decided by the global `Options \| Lock Envelopes to Events` (analysed in F-MPH-005) |
| Adobe Premiere | 26.x | Adjustment Layer | the layer is an ordinary clip; the keys of Opacity ("intrinsic effect") and of any Lumetri parameter live in the time of this clip and move with it |
| DaVinci Resolve | 20.3.2 | Adjustment Clip | the keys of the Inspector and of the Color page are in the time of the layer-clip; a separate "layer time" does not exist, because the layer is the clip |
| Final Cut Pro | 12.3 | Adjustment Clip (native since 11.1) | Opacity/Transform/Crop/Distort are keyframed on the layer-clip itself, as on any other clip |
| After Effects | 26.x | Adjustment Layer | the keys are stored in the TIME OF THE LAYER; stretching and time remapping of the layer drag them along |

The answer is the same for all five, and it is not about automation: **the layer has its own time, because the layer is a clip.** The key sits in this time, and the question "in which domain" simply does not arise.

## The trick behind it

**The domain of a key is not an implementation detail but a promise to the user.** For us `AutoKey.t` is a tick of the MATERIAL (`src/libs/kt-model/src/project.rs:1587`), and it was chosen not for beauty: that is exactly why trimming does not stretch the animation but hides the keys; a cut gives each half its piece of ONE curve, and the value at the joint matches from both sides; reversal mirrors the animation together with the picture. No geometric command moves the keys — "stickiness" is ensured by the REPRESENTATION, not by the code of the commands (test t212, `src/libs/kt-model/tests/fx_auto.rs`).

**The morpher layer has no material.** It is not a clip: it never has `media`, `source_in` or `reversed` — only `lane/start/duration/targets/patch` (`project.rs:569`). Therefore the single automation mechanism is not applicable to it at all: the domain will have to be CHOSEN anew, and each choice pays for something. Project time — the curve stays in place when the block is moved, and the keys would have to be moved with commands, which our mechanism does not do on principle. Time from the start of the block (this is how the camera and the transition live) — but in the `Marks`/`Areas` modes the action window comes from a REFERENCE, not from the block (`project.rs:929`): the block stands at the start of the timeline, acts on an area in the middle, and a curve "from the start of the block" drifts past entirely. The material of each target — the layer has many targets, with different sources and entry points, and one curve in "material" cannot be addressed.

**And this is the first carrier whose domain is not derived from itself.** We already have three carriers of curves and TWO domains: the effect slot — the material of the clip (`project.rs:1520`), the camera — the tick from the start of the event (`project.rs:2471`), the transition — the tick from the start of the window (`project.rs:382`). This is not a mess: the domain each time is determined by what the carrier has as its own axis. The morpher has no axis of its own — the block sets the time in some modes and does not in others — therefore the morpher requires not "a third domain" but the first DECISION about the domain in the project.

## Pros

- The widget is already written and has already been reused three times: `automation::draw` serves the effect slot, the camera and the transition through `AutoCtx` with a pair of functions `to_timeline`/`to_curve` (`src/apps/kt-app/src/automation.rs`; the connections are at `fx_editor.rs:480`, `camera_editor.rs:150`, `transition_editor.rs:273`). The morpher needs a FOURTH pair of functions, not a fourth keyframe editor — the rule "no effect should have its own keyframe editor" (`src/apps/kt-app/CLAUDE.md`) holds by itself.
- Storage is ready: `ParamCurve`/`AutoKey` are serialized with `skip_serializing_if = Vec::is_empty`, so files without curves stay byte-stable — this is already done for cameras and transitions.
- A curve is attached only to `ParamKind::Smooth` (`project.rs:2117`): structural parameters (the geometry of the reverb delay lines) are not subject to automation. The rule transfers to the morpher without a single reservation.
- The domain "tick from the start of the block" would give the morpher exactly the behaviour the user has already seen with the camera: moving carries the animation along, editing the length reveals and hides keys, nothing is stretched.

## Cons

- The morpher has NO curve at all, and it stays silent more honestly than it seems: `MorphPatch::apply_to` appends the effects to the target via `c.fx.extend(add)` (`project.rs:858`) and does not touch `c.fx_auto`, while `active_fx_auto()` gives the appended slots empty vectors (`project.rs:1560`). The layer does not "fail to animate" — it animates with an empty set of curves, and no error is raised.
- Reference modes deprive the block even of its own time: `Marks` and `Areas` take the windows from a reference and do not limit them by the block (`project.rs:929`), and there may be several windows per target. Any domain tied to the block is wrong in these modes.
- The layer cuts the target into pieces along the edges of the windows (`Project::effective_clips`, `project.rs:2971`), and each piece is a separate `ClipEvent` with its own `start`/`source_in`. The curve of the layer must give one value at the seam of the cut from both sides — otherwise a step will appear exactly where by construction it cannot be.
- A reversed target. If the projection naively puts the key positions of the layer straight into `fx_auto` of the piece, then for a reversed clip the animation of the LAYER will run backwards: the material is mirrored (`clip_material_at`), while the time of the layer must not be mirrored.
- **A side effect that already costs the user data: pasting a copied chain into the morpher SILENTLY loses the envelopes.** In the morpher window `chain_strip` is called with `auto: None` (`src/apps/kt-app/src/morpher_editor.rs:381`); the paste branch puts `slot.fx` and `slot.bypass`, and skips `slot.curves` (`src/apps/kt-app/src/widgets.rs:369`) — and yet still sets `ch.auto = true` (`widgets.rs:386`), that is, it reports a change of curves that it did not carry over. The same widget in the event window does carry the curves over and rebases them to the new entry point (`src/libs/kt-model/src/ops.rs:892`, `fn rebase`). Copy a chain from an event, paste it into the morpher — and the envelopes are gone, and nobody says so.
- The morph row is three times lower than the clip rows (`src/apps/kt-app/src/timeline.rs:1389`): there is no room to draw the envelope right on the block; a place for it will have to be looked for in the morpher window.

## What users say

> I want a color grade (using Lumetri) to fade in over the course of a
> scene. There does not seem to be a way to do this.
> — James Strecker, Creative COW, 25.05.2018,
> https://creativecow.net/forums/thread/keyframe-lumetriadjustment-layerae/

The canonical answer of the industry is to animate not the parameters of the effect but the STRENGTH of the layer as a whole:

> In that Lumetri instance, grade to the total change you want to apply.
> In the Effects Control Panel (ECP), use the Opacity 'intrinsic' effect
> to animate (keyframe) the effect of that Lumetri instance on the total
> image.
> — R Neil Haugen, Adobe Community, 19.01.2021,
> https://community.adobe.com/t5/premiere-pro-discussions/color-grade-transition/m-p/11763446

The negative side of the same thread: when there is no mechanism, people duplicate the clip onto the track above and make a crossfade of "the unprocessed with the processed".

> It's dumb that there has to be that workaround, but I will try it out!
> — James Strecker, Creative COW, 04.06.2018,
> https://creativecow.net/forums/thread/keyframe-lumetriadjustment-layerae/

## How it fits Kinotrain

### What already exists

- A single widget `src/apps/kt-app/src/automation.rs`: `AutoCtx { specs, range, to_timeline, to_curve, frame_step }`, graph ↔ table, keys on the frame grid, the mini-timeline moves the main cursor. Three carriers are already connected — the effect slot (`fx_editor.rs:480`, domain: the material of the clip), the camera (`camera_editor.rs:150`, domain: tick from the start of the event), the transition (`transition_editor.rs:273`, domain: tick from the start of the window).
- The curve model: `AutoKey`/`ParamCurve` (`src/libs/kt-model/src/project.rs:1587` and `:1598`), `curve_value` (linear and smoothstep with `ease`; Bézier is NOT there), `apply_fx_auto`, `check_fx_auto`.
- Carriers: `ClipEvent::fx_auto` (`project.rs:1520`), `TransitionEvent::auto` (`project.rs:382`), `CameraEvent::auto` (`project.rs:2471`).
- Copy-paste of a chain TOGETHER with the envelopes: `kt_model::ops::FxSlotBlock` (`src/libs/kt-model/src/ops.rs:851`) and the rebasing of keys "entry point to entry point" (`ops.rs:892`), `paste_fx_chain` (`ops.rs:947`), test t215.
- The morpher: `MorpherEvent` (`project.rs:569`), `MorphPatch::apply_to` (`project.rs:832`), the action windows `windows_on`/`ref_windows` (`project.rs:929`/`:892`), the editor window `src/apps/kt-app/src/morpher_editor.rs` (the slot parameters are drawn by the same `draw_video_fx_params` as the event window — but statically).

### What is missing

- The `auto` field on `MorpherEvent`: this is the only carrier of parameters in the project without curves.
- The projection "layer domain → material of the piece" inside `effective_clips` (`project.rs:2971`): the layer cuts the target into pieces, each with its own `start`/`source_in`/`reversed`, and the result must be continuous at the seam — the same requirement that t212 checks for the cut of a clip.
- Automation sections in the morpher window (currently `auto: None`, `morpher_editor.rs:381`).
- Transfer of envelopes when a chain is pasted into the morpher (`widgets.rs:369`) — either transfer, or an honest refusal.

### What stands in the way

The choice of domain, and nothing else. Three options and their costs:

1. **Project time.** Unambiguous and works in reference modes, but moving the block leaves the curve in place; copy-paste of morphers already rebases `start` (`ops.rs:133`, `rebase_morpher`), so the keys would have to be moved by COMMANDS — and the main property of the current mechanism falls: "no geometric command touches the keys".
2. **Tick from the start of the block** (as for the camera). Moving and trimming behave as expected, the keys are not moved by anything. It breaks on `Marks`/`Areas`, where the window comes from a reference rather than from the block, and where there may be several windows per target: the curve runs over each window anew or once across the whole block — there is no answer, one has to be CHOSEN.
3. **The material of each target.** It would survive trimming and reversal of targets, but the layer has many targets and their materials differ; the option falls apart into "a curve per pair (layer, target)" — bookkeeping that nobody owns and that breaks when the set of targets changes.

Separately: a correct projection must run "layer domain → timeline tick → material of the piece" and therefore depend on the `reversed` flag of each target, otherwise the animation of the layer is mirrored on reversed clips.

### Which invariants it touches

- Root `CLAUDE.md`: **7** — the value of the layer's curve must be computed by ONE code for preview and export; the projection lives in `effective_clips`, that is, before `frame_plan`, and both backends receive already projected curves. **11** — the value at the seam of a target's cut must match byte-for-byte, otherwise GOLDEN_VIDEO will move. **3** — slots appended by the layer are diffed by clip id, and parameters changing along a curve must go through `set_params` with smoothing, not recreate the FX state on every frame.
- `src/apps/kt-app/CLAUDE.md`: **17** — paired ways of one action behave alike, and at present pasting a chain into an event and into the morpher behave differently; **20** — nothing is switched off by itself, and a paste silently cuts the envelopes; **16** — the impossibility of an action is shown as an error, not by silence; **13** — the morph row is three times lower, so no new strip is to be created under the envelope.
- Format: `KT_META_VERSION = 4` (`src/libs/kt-otio/src/meta.rs:17`). An old build would read the layer's curves as their absence and would silently render the wrong thing — under the policy of v3 and v4 ("refusal is more honest than silent corruption") this is v5 with a hard refusal.

## Estimate

| What | Answer |
|---|---|
| Size | XL — the choice of domain changes the semantics of automation and raises the file version |
| Where the main work is | kt-model (the `auto` field + projection in `effective_clips`), kt-app (automation section in the morpher window), kt-otio (v5) |
| Project format | v5 needed (`KT_META_V5`, hard refusal of old builds) |
| Tests that will have to be written | domain: moving and trimming the block do not move the keys (on the pattern of t212/t231); continuity of the curve through the seam of a target's cut; a reversed target does not mirror the animation of the layer; `Marks`/`Areas` — where the curve runs; morpher render == bake with curves (an extension of t36); transfer of envelopes when a chain is pasted into the morpher (t215/t217); round-trip of the model and of .otio |
| Depends on | F-MPH-006 (the layer's strength slider) |
| Breaks compatibility | yes — files with layer curves are not readable by builds before v5 |

On a separate line: **repairing the silent loss of envelopes when a chain is pasted into the morpher — cost S, and it does not depend on the choice of domain.** While `auto: None`, the paste must say in words that the curves will not be carried over (rule 16), rather than report `ch.auto = true` and stay silent.

## Verdict

We take it, but AFTER F-MPH-006. The curve is needed first of all for the strength of the layer: while the morpher has no single scalar "how much of it", there is nothing to lead with a curve — "the grade grows" cannot be expressed through the parameters of individual effects, but through the strength it is expressed by one line, as in all five editors. The order: strength (F-MPH-006) → decision on the domain and a curve for the strength → curves of the layer's other parameters by the same mechanism and the same widget. The silent loss of envelopes when a chain is pasted into the morpher is a defect, not a missing feature, and it is fixed immediately and separately from everything listed here.

## Sources

- https://creativecow.net/forums/thread/keyframe-lumetriadjustment-layerae/ (accessed 2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/color-grade-transition/m-p/11763446 (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/adjustmentevents.htm (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/21/en/content/topics/event_envelopes.htm (accessed 2026-09-07)
- https://larryjordan.com/articles/new-features-in-apple-final-cut-pro-11-1/ (accessed 2026-09-07)
- https://helpx.adobe.com/premiere/desktop/add-video-effects/work-with-composites/specify-clip-opacity.html (accessed 2026-09-07)
- The Resolve and After Effects lines — per the documentation and common practice; not checked live in this wave.

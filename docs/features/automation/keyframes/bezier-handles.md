---
id: F-AUT-001
title: Bézier handles on keyframes
status: candidate
priority: B
cost: XL
category: automation/keyframes
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-otio, kt-app, kt-engine, kt-fx, kt-gpu]
invariants: [7, 11]
ui_rules: [1, 3, 8, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-AUT-001 — Bézier handles on keyframes

## Essence

A keyframe has not only a time and a value but also a "character of entry/exit": two handles stick out of the keyframe, and the editor pulls them so that the motion starts softly, accelerates and arrives softly. A separate keyframe type — hold: the value stays until the next keyframe and jumps abruptly. Without handles the animation runs at constant speed and to the eye reads as mechanical: the object starts and stops instantly.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | envelope point types; Bezier for OFX keyframes | Two systems in ONE product. On envelopes and in Pan/Crop — six predefined shapes per SEGMENT (Linear, Fast, Slow, Smooth, Sharp, Hold), nothing to pull; the curvature of the path is set by a separate Smoothness 0…100 number on the keyframe. For OFX effects — a real curve editor with "Bezier Curve (locked tangents)" and "(split tangents)" |
| Adobe Premiere | 26.x | Bezier / Auto Bezier / Continuous Bezier / Hold + Ease In/Out | Right-click on the keyframe. Two INDEPENDENT sets on one keyframe: temporal (speed over time) and spatial (shape of the path in the frame). Handles — in the mini-graph of Effect Controls. There is no single-key "Easy Ease" button: Ease In and Ease Out are set separately |
| DaVinci Resolve | 20.3.2 | Inspector keyframes + Curve Editor; in Fusion — Spline Editor | Right-click on the orange diamond: Linear / Ease In / Ease Out / Ease In and Out. Handles — only in the Curve Editor. Inside Fusion there is its own full spline editor with a different set of types, so within one application there are again two systems |
| Final Cut Pro | 12.3 | Video Animation editor | Right-click on the keyframe: Linear / Ease / Ease In / Ease Out. There are no separate handles — the curvature of a segment is set by Command-dragging the line itself between keyframes. Full 2D Bezier paths live in Motion, not in FCP |
| After Effects | 26.x | Graph Editor: Value Graph and Speed Graph | Linear / Bezier / Continuous Bezier / Auto Bezier / Hold; Easy Ease (F9). A handle is described by a NUMBER: the Keyframe Velocity dialog gives speed and influence 0.1…100 %. Temporal and spatial handles are different, and are set in one Keyframe Interpolation dialog. Plus roving keyframes: for spatial properties the middle keyframes are detached from time |

## The trick behind it

**1. A value handle and a path handle are not the same thing, and confusion between them has lived on forums for decades.** A Position keyframe in AE and Premiere has TWO interpolations at once. Setting temporal = Linear does not straighten the path: the path is straightened by spatial = Linear. Hence the "boomerang": two keyframes with the SAME value, but with Bezier handles of spatial interpolation, give visible movement back and forth — the numbers say "stay", the picture shows a loop. The reverse side: if only the temporal one is correct and the spatial one has been straightened, instead of the expected arc you get a polyline with a kink exactly at every keyframe.

**2. A vector property simply has NO value handles — there is nowhere to draw them.** The Value Graph draws one scalar function, while Position is (x, y, z); that is why AE does not show Bezier handles for it in the value graph and sends the user to the Speed Graph, where the quantity is scalar (px/s). The practical consequence for anyone building ONE automation widget: "Bezier handle" of a scalar parameter and of a vector parameter physically live on different graphs, and this has to be solved in the model, not in drawing. AE's answer to "how do I keep constant speed along a curved path" is roving keyframes: the middle keyframes stop being tied to time and slide along it themselves, and ease remains only at the ends.

**3. "Easy Ease" became a button not because it is a special kind of curve, but because the handle is a NUMBER.** Influence in AE lies in 0.1…100 %, and Easy Ease simply sets the same value on both ends (33 %). Exactly for this reason it could be hung on F9, and Premiere never got it. And yet none of the five editors lets you save YOUR OWN curve as a named preset: the set of shapes is either closed (Vegas, FCP, Resolve), or the curve is drawn anew every time (AE, Premiere). Hence an entire industry of paid plugins for exactly this — Flow, Ease and Wizz, Motion, Essential Motion; people buy them not for a new kind of interpolation, but for the button "apply MY curve once more".

## Pros

- The handle is a local property of the keyframe: editing one keyframe does not touch its neighbours, and a curve of forty keyframes does not have to be moved as a whole, as with a flag on the entire curve.
- Separating temporal and spatial gives what a single flag cannot reach: constant speed ALONG a curved path (AE's roving keyframes).
- Influence is a number, not a gesture: the curve can be carried between projects, printed in a dialog, set by a script, compared by eye between two keyframes.
- A hold keyframe removes the typical crutch "duplicate the keyframe one frame earlier": stepped animation stops doubling the number of keyframes.
- A closed set of shapes (Vegas, FCP) pays for the lack of flexibility with predictability: the handle cannot be grabbed by accident and break the curve, and the look of the curve is the same for all users.

## Cons

- A keyframe grows two vectors, and EVERY operation on keyframes must carry them along: Split, trim, reverse, speed change. Reverse must also swap the incoming handle with the outgoing one — otherwise a smoothed curve runs backwards, and the defect is visible only by eye.
- Bezier over time can produce non-monotonicity: a handle pulled past the neighbouring keyframe makes the value overshoot and come back. In AE and Premiere this is standard behaviour, not a bug, and "unexpected movement" is the most frequent complaint about automatic handles.
- Auto Bezier rearranges the handles ITSELF when the keyframe's value changes: the animation changes without the user touching the curve.
- Two graphs (values and speed) double the vocabulary: the user does not know which one they are in now and edits the wrong thing. In AE this is distinguished by line color (color — values, grey — speed), which as a hint does not work.
- A cubic per frame per parameter is cheap, but an x-monotone Bezier requires SOLVING an equation for the parameter — that is iteration, not arithmetic; for an engine with a byte-for-byte reference this is fundamentally worse than a polynomial.

## What users say

> I suggest that you edit the Speed Graph instead. It is nearly
> impossible to edit a position path by using the Value Graph.
> — Rick Gerard (Adobe Community Expert), 21.08.2023,
> https://community.adobe.com/t5/after-effects-discussions/missing-bezier-handles-for-position-on-value-graph/m-p/14023458

> I have not ONCE in my 17 year editing career wanted to use
> auto-beizer, yet it defaults to it constantly.
> — Will.., 01.11.2024, in the idea «Linear spatial interpolation by default»,
> https://community.adobe.com/t5/premiere-pro-ideas/the-option-to-have-keyframes-use-linear-spatial-interpolation-by-default-instead-of-auto-beizer/idc-p/15080705

> Auto bezier leads to all sorts of unintentional movement.
> — TaranVH (author of the same idea), 24.01.2023,
> https://community.adobe.com/t5/premiere-pro-ideas/the-option-to-have-keyframes-use-linear-spatial-interpolation-by-default-instead-of-auto-beizer/idc-p/15080705

Both complaints are about the same thing: an automatic handle set on the user's behalf is perceived as a defect of the program, not as a convenience.

## How it fits Kinotrain

### What already exists

- `src/libs/kt-model/src/project.rs`: `AutoKey { t: Tick, v: f64 }`,
  `ParamCurve { param, ease: bool, keys }` and `curve_value(keys, ease, at)`
  — linear between keys, with `ease` — smoothstep `s*s*(3-2s)`, and the extreme values beyond the edges. The flag sits on the WHOLE curve: a key has no character of its own, hold is absent.
- The key's domain is the MATERIAL tick (R.33): trimming hides keys rather than dragging the animation; reverse mirrors it (`clip_material_at`, test t212).
- Curves are attached only to `ParamKind::Smooth` — `auto_params()` is there too.
- One mechanism for three carriers: `ClipEvent.fx_auto`, `CameraEvent.auto`
  (`params_at`), `TransitionEvent.auto` (`kind_at`). It is evaluated in two
  places in the engine: `src/libs/kt-engine/src/video.rs` (lines 85 and 411).
- One widget for everything: `src/apps/kt-app/src/automation.rs` (graph ↔
  table); the key snaps to the frame grid via `frame_step`, the value window is frozen on drag (`AutoView.drag_win`). The `ease` checkbox is at
  `src/apps/kt-app/src/fx_editor.rs:391`, one per curve.
- Format: the `metadata["kt"]` envelope of version 4
  (`src/libs/kt-otio/src/meta.rs`, `KT_META_VERSION`); curves round-trip
  through `src/libs/kt-otio/src/convert.rs` (t192); files up to v3 are migrated to material ticks (t212).
- Cameras: `CameraEvent { auto, ease }`, `CameraParams` goes to
  `src/libs/kt-engine/src/video_plan.rs` and is executed in
  `src/libs/kt-gpu/`.

### What is missing

- Character OF THE KEY: a type (linear / hold / bezier), handles on each side, different shapes of entry and exit. Right now there are exactly two shapes for the whole curve, and both are symmetric.
- A way to save one's own curve — though no one else's has it either.
- Spatial interpolation. We do not have it not "yet" but by design: `CameraParams.pos_x/pos_y/pos_z` are THREE independent scalar `ParamCurve`s, and a diagonal camera move across three keys gives a polyline with a kink at every key in the frame, not an arc. That is exactly what our "mechanical" camera move looks like, and it cannot be cured with handles on the values alone: one needs either a shared 3D point per key, or a spatial handle on top of the three curves.

### What gets in the way

1. **The analytic warp map.** `src/libs/kt-fx/src/lib.rs`: the `rate` profile must be piecewise LINEAR, because the closed form of the map rests on it — `S(τ) = ∫rate` is piecewise quadratic, the inverse `T = S⁻¹` is the root of a quadratic equation, "neither ln nor expm1: only + − × ÷ √", that is, bit-portable. `rate_at` already now IGNORES `ease` for exactly this reason. A cubic handle on `rate` gives an `S` of the fourth degree and the solution of a fourth-degree equation in the inverse map — there is no bit-portable closed form there. So the handles cannot be the same for all parameters: either `rate` remains an exception (and this must be shown honestly in the UI), or invariant No. 11 for audio dies.
2. **Determinism (invariant No. 11).** An x-monotonic CSS-style cubic Bezier requires finding the parameter from time — Newton or bisection; the result depends on the number of iterations and the order of operations, not only on IEEE-754 rounding. Byte-for-byte against the committed GOLDEN_VIDEO may survive this, but it stops being obvious. The choice of form (a Bezier with a solve versus a Hermite/Catmull, where the value is a POLYNOMIAL of the parameter and there is nothing to solve) is the main decision of the round, and it is made in `kt-model`, not in the widget.
3. **Rule 20 of `src/apps/kt-app/CLAUDE.md`** ("nothing grows back and nothing switches off by itself") explicitly forbids Auto Bezier as a MODE: a handle that rearranges itself when the value is edited is exactly what users of Premiere complain about in the quotes above. Only a one-off command "smooth the keyframe" is allowed.
4. **Invariant No. 7.** `curve_value` is called by the preview, the export, the CPU and the GPU plan (`video.rs:85`/`:411` → `video_plan.rs`). A new interpolation goes ONLY into `curve_value`; one must not create its own version of ease in the shader `src/libs/kt-gpu/src/shaders/`, otherwise the backends will diverge beyond the tolerances of `assert_frames_close`.
5. **Grabbing a handle (UI rule 1) on a floating scale.** In `automation.rs` the value window is computed FROM THE KEYS and frozen for the drag; a handle drawn with a fixed length in pixels changes its model value when the scale is recalculated. The units of the handle (the share of the segment in time + the share of the value difference) must be defined before drawing.
6. **Mirroring and transfer.** Split, trimming, reverse, `migrate_legacy_*` — every place where keyframes are copied or mirrored must do the same with the handles, and reverse must also swap them.

### Which invariants it touches

- No. 7 (one composition code) — interpolation lives in one `curve_value`.
- No. 11 (determinism) — the new formula must be reproducible; GOLDEN_VIDEO and GOLDEN_AUDIO must not move on files without handles.
- UI rules: 1 (grabbing a handle), 3 (nothing jumps when handles appear), 8 (the handle is drawn in the language of the widget, not as a standard control), 20 (auto-bezier as a mode is forbidden).
- No. 1, No. 4, No. 5, No. 6 are not touched: neither the audio format, nor the cache, nor the ownership of the decoder, nor the clocks are affected by this.

## Estimate

| What | Answer |
|---|---|
| Scope | XL — a round plus a version of the format envelope |
| Where the main work is | kt-model (`AutoKey`, `curve_value`), kt-app (`automation.rs` — handles on the graph and columns in the table), kt-otio (v5) |
| Project format | a v5 version of the envelope is needed (`KT_META_VERSION`, `src/libs/kt-otio/src/meta.rs`). Files without handles must remain byte-stable (as in t190/t192), and a file WITH handles must be rejected by an old build EXPLICITLY through `KtMetaTooNew`, rather than read "almost the same" |
| Tests that will have to be written | the shape and monotonicity of the new interpolation next to t190; mirroring of handles at Split/trimming/reverse next to t212; round-trip and v5 next to t192/t222; cameras next to t231; grabbing a handle on the graph next to t221 in `layout_tests.rs`; no change of GOLDEN_VIDEO/GOLDEN_AUDIO (t11/t67) and repeatability of two runs |
| Depends on | — |
| Breaks compatibility | forward no (old files are read as they are), backward yes (a file with handles will not open in a build before v5) |

## Verdict

Take it, but not as a copy of After Effects. One value curve per scalar parameter, a handle as a NUMBER on each side of the keyframe plus the type of the keyframe (linear / Bezier / hold); we do not set up a speed graph — we have no vector parameters, and a second graph would double the vocabulary for a task that we do not have. We do not extend the handles to `rate`: there sits the analytic warp map, and it is more expensive. Auto Bezier as a mode is forbidden by rule 20 — only a one-off command "smooth the keyframe". The main consumer is the cameras: three independent scalar curves `pos_*` give a polyline instead of an arc, and this is more noticeable than any FX; spatial interpolation of the camera is a separate note, this one does not close it.

## Sources

- https://helpx.adobe.com/after-effects/using/keyframe-interpolation.html
  (kinds of interpolation, temporal/spatial, roving keyframes; accessed
  2026-09-07)
- https://ae-scripting.docsforadobe.dev/other/keyframeease/ (influence
  0.1…100 %; accessed 2026-09-07)
- https://helpx.adobe.com/premiere/desktop/add-video-effects/control-effects-and-transitions-using-keyframes/control-effect-changes-using-bezier-keyframe-interpolation.html
  (Bezier / Auto Bezier / Continuous Bezier / Hold; accessed
  2026-09-07)
- https://community.adobe.com/t5/premiere-pro-ideas/the-option-to-have-keyframes-use-linear-spatial-interpolation-by-default-instead-of-auto-beizer/idc-p/15080705
  (the idea and responses about Auto Bezier by default; accessed
  2026-09-07)
- https://community.adobe.com/t5/after-effects-discussions/missing-bezier-handles-for-position-on-value-graph/m-p/14023458
  (Position has no handles in the Value Graph; accessed 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/adjust-effect-curves-ver8e3f1f04/mac
  (Linear / Ease / Ease In / Ease Out, Command-drag of the line; accessed
  2026-09-07)
- https://www.vegascreativesoftware.info/us/forum/what-is-the-name-of-keyframe-curve-bezier-curve-b-spline-x-spline--127039/
  (cubic Bezier with locked tangents; handles — only in OFX, not in Pan/Crop and Track Motion; accessed 2026-09-07)
- https://www.vegascreativesoftware.info/us/forum/pan-crop-the-smoothness-setting-is-important--46721/
  (Smoothness 0…100 as a SEPARATE control of the path's curvature; accessed 2026-09-07)
- https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=152527 and
  https://davinciresolveclub.com/davinci-resolve-keyframes-curve-editor/
  (Linear / Ease In / Ease Out / Ease In and Out at the Inspector diamond, handles in the Curve Editor; accessed 2026-09-07)
- https://mtmograph.com/blogs/tools/how-to-ease-keyframes-in-premiere-pro
  (why curve presets are sold as plug-ins; accessed
  2026-09-07)

Vegas 2026 and Resolve 20 were checked by the help and forums, not run live; the table rows for them rely on items 1–2 of the trust order from `docs/features/_editors.md`.

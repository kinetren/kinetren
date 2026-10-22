---
id: F-UI-001
title: On-screen controls in the viewer: transform frame, handles, guides
status: done
priority: B
cost: M
category: ui/viewer
editors: [after-effects-26, premiere-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: yes
crates: [kt-app, kt-engine, kt-gpu, kt-model]
invariants: [7, 12]
ui_rules: [1, 2, 3, 6, 9, 10, 13, 15, 16, 17, 20]
round: "R.64"
point: "change geometry-on-screen-controls (chronicle frozen at §470)"
tests: [759, 760, 761, 762, 763, 764, 765, 766]
depends_on: []
checked: 2026-09-25
---

# F-UI-001 — On-screen controls in the viewer: transform frame, handles, guides

## Essence

Editing the position, scale and rotation of the picture directly in the preview window: around
the layer a frame with eight handles is drawn, at its centre — the anchor point, on the side — the
rotation handle; the crop is stretched by its own handles, the mask by points with Bézier tangents,
and movement by keyframes is shown as a trajectory. On top of the frame lies markup that is never
rendered anywhere: a grid, guides, the centre, safe zones for titles and action. And there too live
the comparison modes — "before/before" and "before/after" in one frame, divided by a wipe. All of this
is switched by a "show/hide" button: the controls lie exactly on what a person looks at the frame for.

## Who has it and how

| Editor | Version | What it is called | How it is built |
|---|---|---|---|
| After Effects | 26.x | Layer Controls, Anchor Point (Pan Behind) tool, Motion Path, Guides / Proportional Grid / Title-Action Safe | THE REFERENCE. Controls exist while the layer is selected: the frame, the anchor as a "crosshair", the mask tangents, the motion trajectory with handles. The anchor is moved by a SEPARATE tool (`Y`), and this is the only gesture that edits two parameters at once (quote below). The markup — rulers, guides, proportional grid, title/action safe (Preferences → Grids & Guides). Everything is hidden by one command `View > Show Layer Controls` (Ctrl+Shift+H), the composition is set by `View Options` (Ctrl+Alt+U). During playback the controls hide THEMSELVES, and Adobe explains why (quote below) |
| Adobe Premiere | 26.x | Motion / transform handles in Program Monitor, Comparison View, Safe Margins | Controls appear only when the word Motion (or another effect with a spatial parameter) is SELECTED in Effect Controls — hence the classic complaint "I cannot call them up" (quote below). The second way — a double click in the Program Monitor. Safe Margins — in the monitor's wrench menu. Comparison is moved to Comparison View: Side by Side, Vertical Split, Horizontal Split — the FRAMES of the edit are compared, not "with effect / without effect" |
| DaVinci Resolve | 21 | Transform / Crop / Dynamic Zoom (Timeline Viewer), View → Viewer Overlay, Safe Area, Split Screen + Image Wipe (Color) | The controls are switched on by the "Transform/Crop/Dynamic Zoom button at the bottom left of the Timeline Viewer", exactly ONE set is active: "These on-screen controls can also be selected by choosing an option from the View > Viewer Overlay submenu". The chosen overlay is hidden with `Shift-\`` or the Show Controls item in the viewer menu. Safe zones — `View > Safe Area`, with separate toggles for parts. Comparison — a developed tool of the Color page: the Wipe / Split Screen / Highlight panel, a wipe with a frame from the gallery or another VERSION of the grade |
| Final Cut Pro | 12.3 | Onscreen controls (Transform Shift-T / Crop Shift-C / Distort / Reorient), Title & Action Zones, Comparison Viewer | "Many effects, transitions, and other items use onscreen controls, superimposed over the video in the viewer". They are switched on by menus in the BOTTOM LEFT corner of the viewer; the markup — in the TOP RIGHT (`Show Title/Action Zones`, title-safe = the inner 80%, action-safe = 90%, "Output (exported) media files are not affected"). Corner handles keep proportions, side handles change them, a white circle SHOWS the anchor, and the blue rotation handle physically grows OUT of this circle. About the anchor Apple writes directly: "Anchor X and Y… **This control isn't available in the onscreen controls**". There is no wipe comparison at all — instead a SECOND viewer beside it (Comparison Viewer, Control-Command-6, modes Saved / Timeline) |
| VEGAS Pro | 2026.0 | Event Pan/Crop (a separate window), Overlays, Split-Screen View, Crop OFX (interact controls) | There is no transformation of the EVENT in the main preview: Pan/Crop is its own window with its own canvas and a frame over the source picture (there is also its own rotation point — "drag the point at the center of the selection point"), and the result is watched in another window. In the Video Preview itself there is only markup (Overlays: Grid, Safe Areas "outer border… visible on a television screen", the inner one for titles, channel isolation, 360°) and Split-Screen View: a comparison of "FX Bypassed" or "Clipboard", with the comparison area being an ARBITRARY rectangle dragged with the mouse (presets Select Left/Right Half, Select All). An exception that is easy to miss: OFX plug-ins DO draw handles in the preview — the 2026 documentation contrasts "Crop OFX… offers an intuitive way to crop using the **Video Preview interact controls**" with the internal Pan/Crop and its separate window |
| Kinotrain | — | none | Over the frame there is not one interactive element, except a single `ui.interact` with `Sense::click()` (play/pause, and double-click to detach the window). A 1 px frame, the "Loading" scrim and the ambient glow are drawn. No handles, no anchor, no grid, no safe zones, no comparison. At the same time the parameters for the control already exist: `Pan3d` has pos/rot/scale AND `anchor_x`/`anchor_y` |

## The trick behind it

**1. The anchor point is the only handle that MUST change TWO parameters at once, and for that reason
it is either hidden or moved into a separate tool; and the compensation breaks on rotation.** The layer
position is measured BEFORE the anchor, so "simply shift the anchor" is impossible: the layer will move.
After Effects solves this by making the gesture and the number deliberately do DIFFERENT things — "The
Anchor Point tool, sometimes called the Pan Behind tool in the toolbar, moves the Anchor Point around the
layer without changing the position", whereas editing the same anchor as a number in the timeline "the
Position property does not change, so the layer moves" (both quotes are from Rick Gerard, below). That is,
in the most famous editor the paired ways of one action are DELIBERATELY non-equivalent — because of this,
newcomers understand for years why "it rotates around the wrong place". Apple went the other way and wrote
the refusal straight into the help: the white circle SHOWS the anchor, but "This control isn't available in
the onscreen controls" — the only transformation parameter without an on-screen control, because dragging the
anchor would collide with the gesture "drag inside the picture = move it". Vegas hid it even deeper: the
rotation point exists, but lives inside the separate Pan/Crop window. And the compensation itself has a bottom
over which people stumble on forums: it is computed in the UNROTATED coordinate system, and "It works perfectly
until I rotate the layer, at which point when I move the Anchor Point, the layer also shifts position" (quote
below). For us this is a ready-made test case: `Pan3d` rotations go in ZXY order, and an anchor shift must be
compensated by THE SAME matrix, otherwise we would repeat someone else's bug.

A side detail of the same node, and the best finding on ergonomics: in FCP the rotation handle is a blue
"stem" that PHYSICALLY GROWS out of the anchor circle, so the link "this handle rotates around that point" is
visible without explanations. In Resolve the rotation handle lies in the centre of the image, separately — and
in April 2026 a Blackmagic product manager had to personally explain to a user where it was, because the user
was looking for it along the edges of the frame, as in photo editors.

**2. The control lives in SCREEN coordinates, the parameter in FRAME coordinates, and when a resolution factor
stands between them, it is precisely the mouse hit that breaks.** Vegas chose the radical variant: it moved its
OWN transformation out of the preview into its own window with its own canvas — and got a whole class of
complaints like "the image in the event pan/crop tool doesn't match up to the image in the preview window or the
render" (quote below). Tellingly, it ALLOWS foreign OFX plug-ins to draw handles in the preview: that is, the
matter is not impossibility, but that its own old subsystem lives in another coordinate space and did not move.
The price of the other variant is visible in Premiere: the controls are drawn over the real frame, but are shown
only when the needed effect is selected in Effect Controls — and half the questions on the forum are not about
transformation but about how to see it at all — up to the point where for one of the askers the handles came back
only after moving the Program Monitor to another monitor and changing its size, and the standard expert advice in
such threads is to reduce the zoom of the program monitor. This is the closest thing to our problem found in
other forums: visibility and hitting a control depend on the display scale of the preview. Our own code shows where
this breaks numerically: a preview at 1/16 of 1920×1080 is 120×66 (rounding to an even number cuts 67.5 to 66), an
aspect ratio of 1.818 against 1.778, and `world_scale` in the plan is taken ONLY from the width.

**3. "Compare before and after" is not an overlay, but a second composite of the same tick, and "before" means
different things to everyone.** Vegas Split-Screen View compares the frame with ITSELF WITHOUT all video effects,
or with the contents of the CLIPBOARD, and divides the frame not with a wipe but with an arbitrary rectangle dragged
by the mouse. Premiere Comparison View compares two FRAMES of the edit (Side by Side / Vertical Split / Horizontal
Split) and knows nothing about effects. Resolve's wipe on the Color page compares with a frame from the gallery or
with another VERSION of the grade, that is, "before" means "before this node". FCP has no wipe at all: there is a
second viewer beside it (Comparison Viewer). One and the same button name in four editors answers four different
questions, and we will have to choose separately which one we answer.

The reverse side of the same is the switch "hide controls", and it is not a luxury. Adobe hides them ITSELF during
playback and says plainly: "Layer Controls are intentionally hidden during preview playback so masks and shape paths
don't get in the way"; Apple does the same automatically and without a choice. Resolve left this to the person and
got both prices at once: the complaint "there is no direct way to turn the on screen controls on/off. You either need
to look away from the screen to find the Ctrl+k keys or go down two levels of right click menu", and users buying a
THIRD monitor for a clean output to see the frame without overlays. Moreover, one must hide not everything at once:
the main junk is not the handles but the trajectory, because the overlay draws the keys of ALL clips at once ("there
are so many line and control points that appear, i literally cannot even see the video content under it all. It's
like a birds nest"), and the request reads "hide the vectors and still be able to transform with a mouse". There is
no separate toggle "handles / trajectory" in any of the five.

## Pros

- A control takes no space: it lies over an already existing frame. This is the only class of interface
  that adds actions without taking away a single row or column (rule 13 of the checklist — a direct argument FOR).
- One movement of the mouse expresses a pair of numbers at once: a point on the screen is (x, y), and in a grid
  of parameters that is two fields and a glance at the frame between them.
- The scale of the gesture is set by the frame, not by the range of the number: "put the layer HERE" does not require
  knowing that the position is measured in world pixels and the anchor in fractions of the layer.
- The rotation handle shows the CENTRE of rotation by being drawn together with the anchor: the invisible link
  "what it rotates around" becomes a picture.
- The markup (grid, safe zones, centre) is not an effect: it does not get into the export, does not change a single
  number, and is hidden entirely by one command.
- The comparison mode does not know what it compares: it is two composites of one tick, so it works on any material
  and any set of effects.
- Vegas shows that the markup can be introduced SEPARATELY from the transformation: Overlays exist in the Video
  Preview, but there are no handles of its own transformation there. That is, the feature can be taken in parts, and
  the markup is the cheapest part.

## Cons

- The controls cover what a person looks at the frame for. Hence some have a manual switch (AE — Ctrl+Shift+H,
  Resolve — Show Controls), while others have a rigid automatic rule: "Onscreen controls are always hidden when you
  play clips in the timeline" (FCP) — there they are not hidden at will, they are always hidden.
- The precision of the gesture falls together with the quality of the preview: a handle of 8 screen px on a preview
  at 1/16 corresponds to a hundred pixels of the frame, and "set exactly 100%" with the mouse is impossible in
  principle — the numeric field remains mandatory.
- One frame serves several tools (transformation, crop, mask, trajectory), and they compete for the same mouse.
  Hence the fork: either exactly one set is active (Resolve, `View > Viewer Overlay`), or the controls are tied to
  the selection of an effect (Premiere) — and both variants produce the question "where are my handles".
- Every additional "visibility gate" multiplies the question "where are my handles": in Resolve an OFX control is
  visible only if BOTH Open FX Overlay in the viewer AND On-Screen Controls in the inspector are switched on, and this
  is a steady source of enquiries.
- The movement trajectory is not decoration but junk: in Resolve the transformation overlay draws the keys not only
  of the current clip, and after a dozen keys the frame beneath them is not visible at all. So the switch is needed not
  as one common toggle, but as separate ones: handles separately, trajectory separately.
- Shifting the anchor rewrites the meaning of ALL already placed rotation and scale keys; the gesture is one, and the
  undo step is one. Yet the compensation that holds the picture in place breaks on a rotated layer in the reference.
- Separation "edit in one's own window, display in the main one" (Vegas) saves all the mathematics of the hit, but pays
  with a mismatch of two pictures — and this is a price confirmed by complaints, not a theory.
- Dragging requires the frame to be rebuilt on every mouse movement: a control without a live picture is useless, and
  a live picture is a full recomposition of the frame at the tempo of the gesture.

## What users say

The classic formulation of the anchor's incomprehensibility — a person changes a number and gets not what they expected:

> Anchor Point is 612, 360 according to timeline data. When you change
> the Anchor Point value to 0, 0 only still moves not anchor point.
> — MahaMN, Adobe Community, 24.05.2022, https://community.adobe.com/t5/after-effects-discussions/anchor-point-issue/m-p/12978119

The expert's answer in the same thread is the "trick behind it" in one line — the tool and the number do DIFFERENT things:

> The Anchor Point tool, sometimes called the Pan Behind tool in the
> toolbar, moves the Anchor Point around the layer without changing the
> position so that you can change the center of the scale and rotation
> properties.
> — Rick Gerard, Adobe Community, 25.05.2022, https://community.adobe.com/t5/after-effects-discussions/anchor-point-issue/m-p/12978119

And the bottom of the compensation itself: the tool holds the picture in place only until the first rotation of the layer —

> the problem is that when the anchor point moves, the layer moves with
> it (the pan-behind tool won't help in this case, it only messes up the
> rest of the animation).
> — Greg Digenti, Creative COW, 19.06.2009, https://creativecow.net/forums/thread/animate-anchor-point-without-moving-layer/

> It works perfectly until I rotate the layer, at which point when I move
> the Anchor Point, the layer also shifts position.
> — same place, same thread (an attempt to compensate with an expression)

The price of the solution "controls are shown only when an effect is selected" (Premiere) is a question not about
transformation but about how to see it:

> Sometimes without even trying, transform handles appear in my program
> window. But whenever I want them, it's always a battle to make them
> show up.
> — Steve Zeeeee, Adobe Community, 08.08.2020, https://community.adobe.com/t5/premiere-pro-discussions/where-are-my-transform-handles/m-p/11346974/highlight/true

The price of the opposite solution — "edit in one's own window, look in another" (Vegas Event Pan/Crop):

> the image in the event pan/crop tool doesn't match up to the
> image in the preview window or the render
> — Daniel Johnson, Creative COW, 15.07.2019, https://creativecow.net/forums/thread/image-crop-doesnt-match-preview-or-render/

The same, in a long list of complaints about VEGAS Pro 2026 on the official Boris FX forum (the author himself marks
the text as "Translated using GPT" — this is a machine translation of his Korean original, so the quote is exact, but
the wording is not his own):

> The workflow is also slower than adjusting the event directly in the
> Preview window, as in Premiere Pro.
> — cglaa5000, Boris FX Forum, 01.08.2026, https://forum.borisfx.com/t/suggestions-for-boris-regarding-vegas-pro-2026-very-long-post/24911/2

> When using masks, the Pan/Crop window where the actual editing takes
> place is separated from the Preview window where the result appears.
> — same place

Why the switch for the controls exists — Adobe's official answer:

> Layer Controls are intentionally hidden during preview playback so
> masks and shape paths don't get in the way.
> — Nishu Kushwaha, Adobe Community Manager, 03.06.2026, https://community.adobe.com/questions-529/layer-controls-are-not-staying-visible-1626156

And what happens when the switch exists but is hidden, while the trajectory is drawn in full:

> But there are so many line and control points that appear, i literally
> cannot even see the video content under it all. It's like a birds nest.
> — Ed Brister, Blackmagic Forum, 01.12.2024, https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=212742

> One constant irritant for me is that there is no direct way to turn the
> on screen controls on/off. You either need to look away from the screen
> to find the Ctrl+k keys or go down two levels of right click menu.
> — Iain Fisher, Blackmagic Forum, 28.01.2026, https://forum.blackmagicdesign.com/viewtopic.php?f=22&t=232477

The primary source on the anchor being an exception among the controls, and on the controls always being hidden during playback:

> Anchor X and Y: Move the image's center point (the point that the image
> rotates around). Note: This control isn't available in the onscreen
> controls.
> — Final Cut Pro User Guide, Apple, https://support.apple.com/guide/final-cut-pro/resize-move-and-rotate-clips-verfc8a4713/mac

> Onscreen controls are always hidden when you play clips in the timeline.
> — Final Cut Pro User Guide, Apple, https://support.apple.com/guide/final-cut-pro/show-onscreen-controls-verbd553847/mac

## How it fits Kinotrain
CAPABILITY: openspec/specs/ui-timeline/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
The transformation frame of the leading "Geometry" node over the preview frame (move, eight scale handles, rotation, anchor with compensation, crop) and the "Transformation" fields section beside it;
the placement rectangle is computed by ONE function `kt-fx::geom`, shared with rendering.
Cameras and `Pan3d`, masks, the keyframe trajectory, the markup and the comparison modes are not included.

## Estimate

| What | Answer |
|---|---|
| Volume | M — 2–3 stages. S1: shared geometry (moving `pan3d_mvp` from kt-gpu into shared code + the conversion "screen → layer plane", fixing the recomposition gate and the vertical scale). S2: ONE shared control widget over the frame (frame, 8 handles, anchor, rotation) on `Pan3d` and cameras, paired with `params_grid`. S3: markup (safe zones, grid, centre) and the show switch |
| Where the main work is | kt-app (`app.rs::draw_preview` + a new shared widget next to `widgets.rs`/`automation.rs`), kt-engine (`video_plan.rs` — layer geometry outward), kt-gpu (the matrix becomes shared, not its own) |
| Project format | not touched: existing fields are corrected by existing commands. The state "show controls/grid" is viewport state, and by the decision of R.7.1 it is stored in the project |
| Tests that will have to be written | harness `layout_tests.rs`: a hit on a handle the first time and its grab zone; the anchor gesture does not move the picture — **and on a rotated layer too** (someone else's bug from the quotes); the screen gesture and `params_grid` give the same number (rule 17); one undo per gesture; the controls fade during playback; the "handles" and "trajectory" toggles are independent. Pure functions: "screen → frame" at all five fractions (including 120×66) and back, inversion of the MVP against the direct one — all on the CPU, without an adapter. A live scenario in `src/scripts/live/`: drag a layer — the pixel in the frame has CHANGED (`pixel_settled(differs_from=…)`), plus `img_rect` in `KT_GEOM_DUMP` |
| Depends on | nothing mandatory. The reverse dependency is stronger: F-MSK-001 (masks) and F-TXT-001 (text) both run into "not one gesture over the frame exists" — this work unblocks them |
| Breaks compatibility | no |

## Verdict

Take it, but start not with the frame, but with the geometry. The parameters for the control are all
already there (`Pan3d` with the anchor, cameras with look-at), the format does not change, and it takes no
place on the screen — by the "cost/effect" ratio it is the cheapest of the remaining large UI features.

The decisive argument is cameras. `CameraParams` is seven numbers, of which the position and the point of
interest are set in world pixels, and the rotations in degrees ZXY; nobody in their right mind will set a
camera frame blindly through a `DragValue` in the properties window. Until a screen control exists, the whole
branch of R.29 (3D, several cameras, the course "towards After Effects" — the owner's wording) remains practically,
not technically, unavailable.

First of all, two defects are fixed, which the control will expose immediately and which are useful in themselves:
the preview recomposition gate, which does not look at the model revision, and the preview at 1/16–1/8 that is not
similar to the project frame. The comparison modes (split-screen, the "before/after" wipe) are NOT part of this pass:
they are a second composite of the same tick and a separate note — there is a separate fork of their own there,
"what counts as the side of 'before'".

### Outcome of R.64 (2026-09-25)

DONE: the transformation frame over the frame of the selected video event — move by the body, eight scale handles,
rotation (Shift — 15°), anchor with position compensation (correct also on a rotated layer, where AE has the defect),
four crop handles; one history step per gesture, a live picture at the tempo of the gesture, a parameter with a curve
is edited by a KEYFRAME. Both defects named above as "will be exposed immediately" are closed: the recomposition gate —
already in R.62 (t722b), the similarity of the preview fraction — here (t765, the nearest EVEN pair). The fields were
brought into the "Transformation" section (groups, X/Y pairs, units, sliders, the frame scheme, reset).

DEFERRED and why: the screen controls of CAMERAS and `Pan3d` (their own units — world px — and their own fork "which camera
is active under the cursor"), masks and the keyframe trajectory (their own set of handles over the same frame), the markup
(safe zones, grid, centre) and the comparison modes "before/after" (a second composite of the same tick). Each is a separate
set over the frame and a separate answer to the question "where are my handles"; they cannot be decided for the owner in one pass.

## Sources

- Final Cut Pro User Guide, "Intro to onscreen controls" (accessed
  2026-09-07) — https://support.apple.com/guide/final-cut-pro/intro-to-onscreen-controls-verbd553437/mac
- Final Cut Pro User Guide, "Show onscreen controls" —
  https://support.apple.com/guide/final-cut-pro/show-onscreen-controls-verbd553847/mac
- Final Cut Pro User Guide, "Resize, move, and rotate clips" —
  https://support.apple.com/guide/final-cut-pro/resize-move-and-rotate-clips-verfc8a4713/mac
- VEGAS Pro Help (MAGIX), "Split-screen previews" —
  https://help.magix-hub.com/video/vegas/22/en/content/topics/5-preview/split-screen_previews.htm
- VEGAS Pro Help (MAGIX), "Using the Video Preview window" (Overlays:
  Grid, Safe Areas) —
  https://help.magix-hub.com/video/vegas/22/en/content/topics/5-preview/videopreview.htm
- VEGAS Pro Help (MAGIX), Pan/Crop overview —
  https://help.magix-hub.com/video/vegas/23/en/content/topics/7-edit/pan-crop-overview.htm
- Adobe Community, thread "Anchor Point Issue" —
  https://community.adobe.com/t5/after-effects-discussions/anchor-point-issue/m-p/12978119
- Adobe Community, thread "Where are my transform handles?" —
  https://community.adobe.com/t5/premiere-pro-discussions/where-are-my-transform-handles/m-p/11346974/highlight/true
- Creative COW, thread "Image crop doesn't match preview or render" —
  https://creativecow.net/forums/thread/image-crop-doesnt-match-preview-or-render/
- VEGAS Pro 2026, Boris FX CDN documentation: the list of Video
  Preview buttons and the caveat about Crop OFX —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/5-preview/videopreview.htm
  and https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/pan-crop-overview.htm
- Boris FX Forum, "Suggestions for Boris Regarding VEGAS Pro 2026" —
  https://forum.borisfx.com/t/suggestions-for-boris-regarding-vegas-pro-2026-very-long-post/24911/2
  (the author marks the text as a machine translation from Korean)
- Creative COW, thread "Animate anchor point without moving layer" —
  https://creativecow.net/forums/thread/animate-anchor-point-without-moving-layer/
- Blackmagic Forum: "Transform tool overlay has unwanted vectors to
  Keyframes" — https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=212742 ;
  thread on the switch for the controls —
  https://forum.blackmagicdesign.com/viewtopic.php?f=22&t=232477
- Adobe Community, "Layer Controls are not staying visible" —
  https://community.adobe.com/questions-529/layer-controls-are-not-staying-visible-1626156
- DaVinci Resolve, the Color page on the Blackmagic site (Wipe /
  Split Screen / Highlight, gallery styles) —
  https://www.blackmagicdesign.com/products/davinciresolve/color
- The wording of the Resolve manual ("Transform/Crop/Dynamic Zoom button
  at the bottom left of the Timeline Viewer"; "View > Viewer Overlay
  submenu"; `Shift-\``; `View > Safe Area`) is taken from mirrors of the official
  manual (18.6 and 17) —
  https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part2092.htm
  and https://manualzz.com/doc/o/235u5a/blackmagic-davinci-resolve-17-instruction-manual-compositing-and-transforms-in-the-timeline
  **[NOT VERIFIED against the Resolve 21 manual]**: the direct mirror pages returned 403/404 from this environment, version 21 was not checked live;
  the mechanics are independently confirmed by a Blackmagic product manager in the thread
  https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=234723 (14.04.2026).
- The names of the Premiere Comparison View modes (Side by Side / Vertical
  Split / Horizontal Split), Safe Margins in the Program Monitor menu, and the After Effects
  command `View > Show Layer Controls` (Ctrl+Shift+H, the composition — `View Options`, Ctrl+Alt+U) are taken from snippets of Adobe's help
  and confirmations by Adobe Community experts (including Todd Kopriva,
  Creative COW, 18.03.2010 —
  https://creativecow.net/forums/thread/cant-see-bounding-boxes-or-pan-behind-tool/):
  `helpx.adobe.com` does not open from this environment **[NOT VERIFIED against the primary source]**.
- No direct complaints in other forums were found about mouse hits on the preview at a FRACTION of the resolution (half/quarter) — the closest are
  the tips to change the size and zoom of the Program Monitor so that the controls appear. The numeric example 120×66 was computed from our code, not taken from someone else's bug report.
- All statements "we have / we do not have" were checked by grep over `src/` and by reading the code at commit `995899c`; the behaviour was not run live —
  the corresponding places are marked in the text.

## Kinotrain: change `geometry-on-screen-controls` following R.63

In R.63 the owner said directly: **"the geometry fx is still very poor, solid boring input fields, even in Vegas it is done better, and we need it
better than in Vegas, in the same Pr/Ae it is more convenient than in Vegas, we need to make it more convenient than everyone. AT MINIMUM we must have
editing right in the preview that is convenient, that is the minimum"**. Asked directly, he clarified: the on-screen controls are **IN ADDITION to the
fields, not instead of them**, and the fields themselves "in a normal form".

Hence the verdict on this feature changes from "candidate" to "we do the next
change" — `geometry-on-screen-controls`: the transformation frame of the leading "Geometry" node directly in the preview (dragging, corners, rotation, anchor), while keeping the numeric fields and their automation. The reconnaissance above (Vegas Pan/Crop,
Premiere Motion, AE Layer Controls) is the entry to this work.

**Implemented in R.64** — change `geometry-on-screen-controls`; the further life of the feature is
in `openspec/specs/ui-timeline/` and `openspec/specs/video-plan/`.

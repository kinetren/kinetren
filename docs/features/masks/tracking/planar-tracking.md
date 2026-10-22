---
id: F-MSK-005
title: Tracking: point, planar and "AI object tracking"
status: researched
priority: B
cost: XL
category: masks/tracking
editors: [after-effects-26, premiere-26, resolve-21, resolve-20, fcp-12, vegas-pro-2026]
we_have: no
crates: [kt-model, kt-otio, kt-media, kt-engine, kt-app]
invariants: [4, 5, 8, 11, 12]
ui_rules: [2, 6, 12, 13, 16, 19, 20]
round: null
point: null
tests: []
depends_on: [F-MSK-001]
checked: 2026-09-07
---

# F-MSK-005 — Tracking: point, planar and "AI object tracking"

## Essence

The tracker looks at a sequence of frames and computes how far whatever the editor pointed at has moved over that time: a spot, a plane or a whole object.
The result is an animation: a mask, an effect or a backdrop moves with the object by itself.
Without a tracker the mask stays where it was placed, and any movement of the camera or
the hero means placing keyframes by hand on every frame. Tracking is not an
independent feature but the MOTOR for masks (F-MSK-001), stabilization and
secondary correction (F-CLR-003): by itself it shows nothing.

## Who has it and how

| Editor | Version | Name | How it works |
|---|---|---|---|
| After Effects | 26.x | Tracker (Track Motion / Stabilize Motion), Mocha AE, 3D Camera Tracker, Object Matte (26.2) | Three generations in one program. The point `Tracker` — a feature rectangle + a search rectangle, outputs position/rotation/scale and is applied to a layer or to a POINT OF AN EFFECT via an attach point. The planar one — a separate program Mocha AE shipped with it, tracks a spline plane and returns corner pin/transform back to the composition. The neural one — `Object Matte` (26.2): "Instantly select and isolate objects using AI", "Automatically propagate mattes across frames", a separate `Refine Edge Tool`; the output is a MATTE, not a transformation |
| Adobe Premiere | 26.x | Mask Tracking, Object Mask | There is no tracking as a separate tool: the tracker lives INSIDE the mask, and the mask lives inside the effect, "Each mask includes its own set of tracking controls". In 26 an AI `Object Mask` was added ("hover and click"), and it is claimed that shape masks (Ellipse/Rectangle/Pen) "track up to 20 times faster" plus bidirectional tracking and perspective [NOT VERIFIED — see "Sources"] |
| DaVinci Resolve | 21 (checked also against 20.x) | Tracker palette (Window / Stabilizer / FBX), Magic Mask | The tracker lives on the Color page and by nature is close to the planar one: "an incredibly simple, yet powerful, 3D cloud tracker", solves pan/tilt/zoom/rotation and 3D perspective from a CLOUD of automatically found points, "highly occlusion-resistant". The same palette switches to the stabilizer and to FBX export. Magic Mask — neural (Studio only); in 21 it got "render in place": "cache a tracked mask as a traveling matte node" |
| Final Cut Pro | 12.3 | Magnetic Mask, Auto Mask | Only the third generation, there is no classic tracker. `Magnetic Mask` — an eyedropper on the object plus `Analyze Forward` / `Analyze Backward` / `Analyze`, interrupted by Esc. `Auto Mask` — semantics without selection: "analyzes your video and instantly isolates recognized objects" from a fixed list ("people, facial features, hair, skin, clothing, skies, foliage, and animals") |
| VEGAS Pro | 2026 (Boris FX); the help was checked against 21 | Motion Tracking, Bézier Masking, Mocha VEGAS | The only one of the five whose tracker is a SEPARATE tool with an explicit output. Planar, five motion models: "Location", "Rotation & Location", "Scale & Location", "Shape & Location: detection of affine transformation (all of the above + shearing)", "Perspective". The result is distributed through the `Transfer Motion Data` menu. Plus the OFX plugin Mocha VEGAS: "This version of the Mocha motion tracking tool has been developed by Boris FX specifically for use in VEGAS Pro" |

The three generations differ not in quality but in WHAT EXACTLY is solved.

1. **Point.** The model is a small spot and its displacement. From two to four
   spots a rotation and scale are assembled. Cheap, understandable, and it falls apart
   exactly where the spot stops being a spot: occlusion, motion blur,
   a change of lighting. One pixel of noise is one pixel of error, and there is nothing to average over.
2. **Planar (Mocha).** The model is a PLANE and its homography. What is tracked is not
   a point but the whole texture inside a spline: "translation, rotation and
   scaling data based on the movement of a user-defined plane". Hence the
   robustness to partial occlusion — not because the algorithm is smart, but
   because the residual is computed over thousands of pixels, and a quarter of the area covered by a hand changes the solution by a quarter of the weight. The output is a perspective
   transformation, that is, corner pin.
3. **Neural.** The model is "what this object is": a network segments the frame and carries the segmentation through time. The output is fundamentally different — not a transformation, but a MASK FOR EVERY FRAME. You can click on anything, but
   "attaching a title to a car's license plate" does not follow from this: the network does not produce numbers
   that could be hung on an effect's position.

## The trick behind it

**1. The main question about a tracker is not "does the mask move", but WHO ELSE the result
can be handed to.** In the feature lists all five write "motion tracking", but
their output differs, and that is the difference between a tool and a gimmick.

VEGAS has the track as an independent entity with an explicit address. The help says
directly: "The tracking data are event envelopes. When they are transmitted
from the tracked event, their timing is relative to the event", and the
`Transfer Motion Data` menu hands them to "the event itself and any other event that
shares time positions with the tracked event", inside them — "all suitable FX
on these events", optionally "add a Bézier Mask FX there", and further "all FX
parameters that represent location data". So one track can land on a
mask, on the position of a pixelate effect, and on another clip standing at the same time on another track. From the same source comes the historical split: "The Motion
Tracking feature is no longer part of the Bézier Masking FX. To use Bézier
Masking with Motion Tracking, track the motion data and create the Bézier
masking FX from the Transfer Motion Data Menu". The tracker was evicted FROM the mask,
because it does not belong to it.

The opposite case is Premiere and Final Cut. In Premiere the tracker is built into the mask, and the mask into the effect: "Each mask includes its own set of tracking controls".
Having tracked a face for a blur, you cannot use the same track to move an arrow-callout:
you have to track again inside a second effect. Apple has turned this into a formal limitation — in the help for `Magnetic Mask` there is a line:
"Magnetic Masks are not included in XML exports". The track not only fails to move to a neighboring effect, it does not leave the program at all. Mocha is at the other end of the scale: it has a separate `Export Data Dialog` with 2D and 3D data
and presets for hosts, and in VEGAS the Mocha data is "send … to the VEGAS Pro
Motion Tracking tool" with the `Apply Export` button, after which it is an ordinary
VEGAS track that can be handed on further.

**2. A broken track is the normal mode of work, not an error.** This is what is absent from
every advertising text and what determines the entire interface design. The VEGAS help describes the cycle verbatim: "If the object being tracked moves out of
view, is obscured, or becomes confused with other elements of the scene,
tracking may stop automatically. Tracking can be continued by manually
advancing the video to a point where the object is visible again, manually
moving the mask to the proper position and then clicking Start again".
So a track is not a "compute button" but a session: start from an arbitrary frame, forward and backward, stop, a manual correction, continuation. Hence Apple has two separate buttons `Analyze Backward` / `Analyze Forward` and interruption by
Esc, Mocha has an entire `AdjustTrack` module for "slipping tracks", Premiere has a frame step next to the analysis button. A product that has only "Track"
and a progress bar is not used on real material: the first
occlusion turns the result into garbage, and there is nothing to fix it with.

**3. A track is cached separately from rendering, and that is the whole cost of it.** A track is
the most expensive thing on the timeline after export: it requires SEQUENTIAL reading of every frame, and the neural one also requires running the network over every frame.
That is why Blackmagic in 21 sells the ability to freeze it as a separate feature:
Magic Mask can "render in place" — "cache a tracked mask as a traveling
matte node", and the cache "automatically link[s] to your active node, allowing
you to continue grading and applying effects in real time, with
significantly lighter processing". The existence of such a button means that by default the network runs ON EVERY DISPLAY OF A FRAME, including export. A classic
tracker is free of this problem by design: it gives numbers once, the numbers lie
in the project, and the render only reads them. The difference "project data versus
computation on the render path" is the real boundary between the second and third
generations — not accuracy, but the fact that in the first case the result is
reproducible, while in the second it has to be baked by hand.

## Pros

- **The planar model averages the residual over the area, so occlusion degrades the solution in proportion to the covered share, rather than knocking it down.**
  The same property gives robustness to motion blur: blur changes the gradients of
  the whole region equally, and the solution by area survives, whereas a point
  correlation track on a blurred spot finds no maximum. Resolve
  describes its cloud tracker with the same words — "highly
  occlusion-resistant".
- **The output of a planar tracker is a homography, that is, exactly what corner pin needs.**
  That is why "put a picture on a phone screen held in a hand" is done with the same track as
  "guide a mask along a wall": both tasks are one and the same matrix applied to different consumers. A point tracker does not solve
  such a task in principle, no matter how many points it is given.
- **A track separated from the mask survives a change of consumer.** In VEGAS
  one can track once and hang the result on several effects of several events standing at the same time; in Mocha — export to another program. One heavy pass over the material pays off many times over,
  and deleting an effect does not take half an hour of machine time with it.
- **The ladder of motion models is a stability control exposed to the outside.** The five items of VEGAS (Location → Rotation & Location → Scale &
  Location → Shape → Perspective) are a choice of the number of unknowns: the fewer of them, the less texture is needed and the harder it is to break the track. An
  editor who knows that the wall does not rotate sets `Location` and gets a
  track where `Perspective` dances.
- **Neural tracking removes the requirement for texture.** A plain T-shirt, a glass bottle, water — where correlation has nothing to latch onto, segmentation works, because it solves a different task: not
  "where did this piece move", but "where is the person here".

## Cons

- **A point tracker gives position/rotation/scale, and that is all.** It
  cannot do perspective, so any plane receding into depth requires
  either four points and manual assembly, or a planar tracker. This is not
  "less accurate", it is a different class of tasks.
- **A planar tracker requires that the plane BE a plane.** A spline thrown over two walls at once
  looks for one transformation for two homographies and drifts away; on the Creative COW forum this is exactly the diagnosis
  that the Mocha developer gives the user: "Your search area is far
  too big and is overlapping multiple planes". Hence also PowerMesh — a sub-planar mesh, added precisely because organic things (a face, fabric) are not described by a plane.
- **Neural tracking does not output a transformation.** `Object Matte` in AE and
  `Auto Mask` in FCP give a matte and only a matte; you cannot hang the position of
  a title on them. This is "one-click object masking", not "one-click
  tracking" — the word has been swapped in the marketing, but not in the interface.
- **The price of the third generation is the render path.** A network not baked into
  an intermediate layer is computed at every display of a frame. From this grow both the
  "render in place" button in Resolve 21, and the complaints about speed above 1080p in
  AE, and the fact that `Magic Mask` in Resolve is only in the paid Studio edition.
- **Semantic tracking is limited to the list of what the network knows.**
  Apple's list is directly in the help: "people, facial features, hair, skin,
  clothing, skies, foliage, and animals". Anything not on the list, `Auto
  Mask` does not see at all, and there is no way around it — there are no parameters for "what to look for".
- **The result of AI tracking travels badly and survives edits poorly.**
  "Magnetic Masks are not included in XML exports"; the same help says
  not to move such clips inside a compound clip and not to copy them into a timeline with a different frame rate. That is, the mask is tied to a SPECIFIC FRAME
  GRID, and an editing operation breaks it.

## What users say

About the third generation — the official Adobe thread about `Object Matte` in AE,
quotes verbatim:

> Works great on footage that is up to 1080p but if you go higher it's
> getting painfully slow, to the point that you're doubting yourself whether
> you actually clicked the mouse button
> — kaczorefx, 20.07.2026, https://community.adobe.com/announcements-527/new-in-ae-object-matte-ai-powered-rotoscoping-1557065

> Looks promising. When I try to use it, I do get the pink selection
> preview, but on click, nothing is selected.
> — Helga Bergers, 24.07.2026, https://community.adobe.com/announcements-527/new-in-ae-object-matte-ai-powered-rotoscoping-1557065

The very same thread also gives the opposite pole — and this matters, because both reviews are about one tool and were written in the same month:

> Tried it recently, and it's a huge time-saver. The AI does a great job for
> most shots, and only a little manual cleanup is needed
> — william111ttt23, 06.07.2026, https://community.adobe.com/announcements-527/new-in-ae-object-matte-ai-powered-rotoscoping-1557065

Separately valuable is the demand to BRING BACK the old tool — a classic sign
that the new generation replaced rather than supplemented the old one:

> We need to be able to revert to the old brush, and actually call it 'Roto
> brush and refine edge'
> — Passionate_nature5867, 20.08.2026, https://community.adobe.com/announcements-527/new-in-ae-object-matte-ai-powered-rotoscoping-1557065

About the second generation — Creative COW, a live analysis of how exactly the planar track falls apart (a user tracks a window, the plane drifts out of perspective):

> when you look at its Planar Surface, it moves very fast out of the correct
> perspective after I start tracking.
> — Alexi Vanderstraeten, 08.12.2016, https://creativecow.net/forums/thread/ae-mocha-tracking-planar-surface/

> For the second one, I blocked at point 4. It's not following the window
> what so ever... Both give different results but none follow the window.
> — Alexi Vanderstraeten, 10.12.2016, https://creativecow.net/forums/thread/ae-mocha-tracking-planar-surface/

And a diagnosis from the same forum, explaining why a planar track breaks most often not because of the material, but because of an incorrectly placed area:

> Your search area is far too big and is overlapping multiple planes
> — Ross Shain, 03.10.2014, https://creativecow.net/forums/thread/mocha-issues-with-tracking/

Fresh forum quotes about track loss in Resolve could not be obtained:
the search results point to threads on `forum.blackmagicdesign.com` with
telling titles ("Magic Mask Re-tracking again", "Magic Mask Tracking
At Export", "Magic mask Loosing tracking"), but the forum itself answers all direct
queries from this environment with HTTP 403, and reddit is unavailable —
details in "Sources". A paraphrase instead of a quote is not put into the base.

## How it fits Kinotrain

### What already exists

There is no tracking: a grep over `src/libs` and `src/apps` for `track|planar|corner
pin|optical.?flow|stabiliz|roto` gives only `mask_digits`/`mask_to_tick`
(the timecode input mask, `src/apps/kt-app/src/util.rs:286`), "interleaved →
planar" about audio buffers, and `Track` as a timeline track. Not a single pixel
of motion analysis exists in the project.

But there are three things that matter for a tracker more than the algorithm itself.

- **A ready precedent of a long background task that reads frames OUTSIDE
  the engine.** `ThumbService` (`src/apps/kt-app/src/thumbs.rs`) — its own thread,
  its own short-lived `FfmpegSource` per media, its own queue with dedup,
  a negative cache of failures, the result is persisted in the sidecar DB. In the header of the file
  this is formalized as an explicit amendment to invariant No. 8: "does NOT touch the playback
  decoders … paint may only ENQUEUE requests to this dedicated
  worker". A tracker is a task of exactly this form, only heavier.
- **A ready precedent of progress and cancellation.** `render_dialog.rs`
  (`src/apps/kt-app/src/render_dialog.rs`) plus `ExportProgress`
  (`src/libs/kt-engine/src/export.rs`) — phase, share, speed, ETA,
  cancellation checked on every frame. We will not have to build a second such construct.
- **A key domain into which a track falls without a single new entity.** Automation keys
  are stored in the tick of the MATERIAL (`AutoKey { t: Tick, v: f64 }`,
  `src/libs/kt-model/src/project.rs:1611`, decision F-AUT-002), so
  trim/split/move/reverse do not touch the curves. A track by its nature is also
  bound to the material: it describes what happened in the FILE. So
  we get for free exactly what Resolve users complain about and what
  Apple warns about with a list of limitations: moving a clip, trimming,
  splitting and reversing an event do not break the track and do not require recalculation. This is
  the strongest thing we have on this topic, and it is already written.

### What is missing

- **Masks.** The tracker outputs a transformation; there is nothing to apply it to.
  The order is strict: first F-MSK-001 (the mask as an entity and as an effect
  parameter), then the tracker. The reverse order gives numbers with nowhere to put them, and a "feature" that cannot be shown. The only consumer that already exists today is the parameters of `Pan` (crop/pan/zoom) in
  `src/libs/kt-fx/src/video.rs`: a track can be hung on them as on
  "location data" in VEGAS terms, and this is a legitimate minimal scenario
  ("pixelate following a face" without masks, with a rectangle).
- **The algorithm itself.** Neither convolutions, nor pyramids, nor a solver. The
  `Cargo.toml` of the workspace has no opencv, no onnx/tract/candle — nothing for
  vision. A planar tracker is Lucas–Kanade over a pyramid plus an
  iterative homography solver; it has to be written by hand, no foreign code can be taken without
  a new heavy dependency.
- **Data schemas for DENSE keys.** `ParamCurve` (`param`, `ease`,
  `keys`) is designed for a dozen keys placed by hand, and for
  `curve_value()` with linear interpolation. A track is 8 numbers for EVERY
  frame of the material; a five-minute shot at 60 fps gives 18000 samples for one plane.
- **UI sessions of a track.** Not a "Track button", but a start from the current frame, forward
  and backward, stop, a manual correction of position, continuation — plus a visualization of the
  search area over the preview. We have nothing similar to direct manipulation of
  geometry over the picture in the preview: `draw_preview`
  (`src/apps/kt-app/src/app.rs`) draws a finished composite and does not accept
  gestures on the frame.

### What gets in the way

**1. The tracker is a new CLASS of client, and it does not fit into the MediaWorker queue.**
Invariant No. 5 requires that exactly one thread own the decoder, and clients
go through the priority queue `Playback > NearPrefetch > FarPrefetch >
Thumbnail` (`src/libs/kt-engine/src/worker.rs`, `Priority` —
`src/libs/kt-engine/src/lib.rs:199`). The temptation to add a fifth level below
`Thumbnail` must be rejected, and not because of latency:

- the worker queue is optimized for the decoder cursor ("requests FORWARD along the
  cursor, nearest first; backward only when forward is empty") and for
  dedup by key. A track is a strictly sequential pass over EVERY frame,
  where neither dedup nor reordering is needed and only gets in the way;
- the results of the worker go into `BurstCache`, and invariant No. 4 declares the cache
  the ONLY truth about warmth. A track would push the whole clip through the cache and
  either evict everything that playback needs, or draw the user a
  fully "warm" bar with nothing behind it. Our bars are honest, and they must not be made dishonest;
- a track has cancellation, progress and a lifetime bound to the TASK, not to the
  generation of the transport; the worker's `generation` cancellation cancels on seek,
  which is directly harmful for a track.

Conclusion: the tracker goes along the road of `thumbs.rs` — its own thread, its own
short-lived source, its own progress, the result AROUND `BurstCache`.
The cost is known and already paid for by the thumbnails: a second open decoder on the
same file, extra memory and extra CPU if tracking runs during playback. UI rule
12 ("background work does not get in the way") requires that a track not start with a gesture
and not jerk the interface more often than a human needs; invariant No. 12 (repaint only
on movement) — so that the progress bar does not keep repainting forever after the end of the
task.

**2. Dense keys versus snapshot undo.** `TimelineModel` keeps the history as full clones of the project:
`undo: Vec<Project>`, `MAX_UNDO = 80`
(`src/libs/kt-model/src/model.rs:339`, `:11`), and every gesture does
`self.project.clone()`. Putting a track into `Project` as it is means copying these 18000×8 numbers eighty times, plus on every autosave, plus
`Project::effective_clips()` clones the clips on every construction of the plan.
This is the first real design obstacle of the topic, and it is not about the algorithm.

**3. Where to put the result — a question with three bad answers.**
`<project>.ktindex.db` (`src/libs/kt-media/src/index.rs`) is the right
place by data type (an expensive result of media analysis), but this DB
is REGENERABLE: it can be deleted and built again from the files. A track with
manual corrections cannot be regenerated — it is user data, and its
loss is irreversible. So, `.otio` (`src/libs/kt-otio/`), where now
`KT_META_VERSION = 4` (`libs/kt-otio/src/meta.rs:17`) and where everything lies as
JSON numbers: the project file would swell by an order of magnitude, and with it the autosave. The third
option — a separate sidecar next to the project — breaks the main property of
our format ("the project is one file, it is copied and moved"). The note does not make the
choice: it is work for the round, but a choice will have to be made, and there is no cheap
option in the list.

**4. Change of source.** We have replacement and restoration of the source (t211,
`src/libs/kt-media/src/placeholder.rs`). A track is bound to the PIXELS
of a specific file, not to the clip; replacing the media must mark the track
stale, rather than silently continuing to apply it. UI rule 20 ("nothing grows or
shuts off by itself") forbids both silently deleting it and silently
recalculating it: an explicit visible status is needed, and impossibility of application is
shown as an error (rule 16), not as a faded hint.

**5. "AI object tracking" is a separate conversation, and it is expensive.** A neural network
needs a runtime and weights: the workspace has not a single dependency for vision,
and our shipping is strict — `scripts/release-build.sh` builds the tarball in
a Ubuntu 22.04 container with checks for "ldd without libav*" and a ceiling of `GLIBC_`.
An add-on in the form of ONNX Runtime and hundreds of megabytes of weights is a new class of
dependency, comparable in weight to the whole rest of the product. GPU cannot be relied upon either:
`kt-gpu` is deliberately built on FRAGMENT passes, "NOT compute
— the default egui-wgpu device on the GL backend lives within downlevel limits, and the
live-check runs on llvmpipe" (`src/libs/kt-gpu/src/engine.rs:1`), and wgpu
is rigidly tied to the version inside eframe. A network is not computed without compute pipelines.

### Which invariants it touches

- **No. 5 (one MediaWorker per decoder)** — touched tangentially and resolved by precedent: the tracker does not become a fifth priority of the queue, but goes through the costly `thumbs.rs` with its own source. Formally this is the same amendment
  to No. 5/No. 8 that has already been made for thumbnails, and it has to be recorded explicitly, rather than "inherited".
- **No. 8 (scroll/zoom does not generate decodes)** — not violated: the track starts by an
  EXPLICIT user action, not by movement along the timeline. This has to be fixed by a test, otherwise "tracking up to the playhead as it approaches" will appear by itself.
- **No. 4 (the cache is the only truth about coverage)** — the track does NOT put frames into
  `BurstCache`. Otherwise the warmth bar will start to lie.
- **No. 12 and UI rules 2/12** — progress, cancellation, no extra repaints; tracking during playback is allowed (rule 2 forbids a gate "only when paused" without an invariant behind it), but with the honest cost of a second decoder.
- **No. 11 (determinism) — lifted, but not entirely.** Here is the answer to the direct question: yes, the fact that the track is PROJECT DATA, computed once, removes the main problem. The render reads ready numbers from the file,
  so `GOLDEN_VIDEO`/`GOLDEN_AUDIO` do not move, the CPU and GPU see the
  same input, and "preview == export byte-for-byte" (invariant No. 7) is not touched at all: the tracker draws nothing and is not part of `VideoRenderer::frame_plan`. Three tails remain:
  1. **Tests of the tracker itself.** Its reference cannot be written as
     a byte-for-byte hash if the computation goes on the GPU. Either the tracker is entirely on the CPU in
     a deterministic order (then an ordinary golden), or a comparison with
     tolerances via a helper like `kt_gpu::assert_frames_close` — ad-hoc epsilons are forbidden by invariant No. 11.
  2. **The tracker's input is non-deterministic between machines.** Invariant No. 10
     means that the same binary works on FFmpeg 6/7/8; different decoders give slightly different pixels, so "re-tracked on another
     machine — got other numbers". Rendering does not break (the numbers are already in the file), but one cannot promise "the same track", and this has to be said aloud in the documentation.
  3. **Lazy recalculation of a track would kill invariant No. 11.** This is exactly what ails
     Magic Mask: the network runs on render, and Blackmagic sells "render in
     place" as a cure. For us recalculation on the render path must be forbidden by construction: a track is computed only by an explicit command, and a stale track is a visible status, not a silent reload.

## Estimate

| What | Answer |
|---|---|
| Size | XL — a whole round, and that is only a point + planar tracker without AI. AI tracking is a separate XL and with a new class of dependency (ML runtime + weights), see "What gets in the way", item 5 |
| Where the main work is | a new vision crate (Lucas–Kanade over a pyramid + homography solver, CPU, no dependencies — modeled on kt-fx); `kt-app` (a track session over the preview, progress/cancellation modeled on `render_dialog.rs`, a background service modeled on `thumbs.rs`); `kt-model` + `kt-otio` (storage of dense keys); `kt-media` (reading frames around `BurstCache`) |
| Project format | a v5 version is needed: `KT_META_VERSION` is 4 now. The volume question must be decided separately — dense keys in JSON plus snapshot undo with 80 copies |
| Tests that will have to be written | synthetic material with KNOWN motion (shift/rotation/homography) and a check that the solution converges to it within tolerance — the current `SyntheticSource` (`src/libs/kt-media/src/synthetic.rs`) encodes the frame number in color and gives no textures, so a generator of a moving pattern will have to be added; behavior on occlusion and on loss; round-trip of the model and `.otio` v5 + reading of files up to v5; "scroll/zoom still does not decode" (invariant No. 8) and "the track did not get into `BurstCache`" (invariant No. 4); a harness test of the track session (start/stop/correction/continuation — rule 14: the feature must work AGAIN); `golden_export` did not move |
| Depends on | F-MSK-001 (masks) — firmly, without them there is nowhere to apply the track except the parameters of `Pan`. Related: F-CLR-003 (secondary windows — the second consumer), F-AUT-002 (the key domain that makes the track robust to editing) |
| Breaks compatibility | no: old files are read, the new field is optional |

## Verdict

Take it, but strictly after masks and strictly as the second generation: a planar tracker with an
explicitly separated result. Making a point tracker separately is pointless — it is a special case of the planar one with two or three unknowns, and the VEGAS ladder of models
(Location → … → Perspective) shows how to set this with one switch instead of two tools. The main design decision
is made once and for all: a track is an INDEPENDENT ENTITY with an explicit
address, which can be hung on a mask, on an effect parameter and on another
clip, not a field inside the mask. Premiere and Final Cut show how the reverse choice
ends: "Magnetic Masks are not included in XML exports".

"AI object tracking" is not taken this time — not because it is not needed, but
because it is not a tracker: it outputs a matte, not a transformation, and it solves
the task of F-MSK-001, not this one. Plus an ML runtime with weights against our shipping as one
tarball and `kt-gpu` without compute pipelines — a separate large
decision that cannot be made in passing inside a note about tracking.

## Sources

Primary sources (help and vendor materials):

- https://help.magix-hub.com/video/vegas/21/en/content/topics/motiontracking.htm
  — VEGAS Motion Tracking: five motion models, `Transfer Motion Data`,
  "The tracking data are event envelopes", the split with Bézier Masking FX
  (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/21/en/content/topics/8-design/fxplugins/beziermask.htm
  — Bézier Masking: mask types, the track into the Width/Height/Angle/Location keys,
  the scenario of loss and continuation (2026-09-07)
- https://help.magix-hub.com/video/vegas/en/content/topics/mochavegas.htm
  — Mocha VEGAS, transfer of data into `Motion Tracking` with the `Apply Export`
  button (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/add-magnetic-masks-ver1d67e3a53/mac
  — Magnetic Mask: eyedropper, `Analyze Forward`/`Backward`, Esc, "Magnetic
  Masks are not included in XML exports", the Apple silicon requirement
  (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/use-the-auto-mask-ver6c261d07c/mac
  — Auto Mask: list of recognized categories, the Shrink/Expand,
  Erode, Blur parameters (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/whatsnew
  — Magic Mask "render in place", "cache a tracked mask as a traveling
  matte node" (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/color
  — the tracker of the Color page: "analyzing the pan, tilt, zoom, rotation, and 3D
  perspective of the shot" (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/
  — "point, planar and 3D camera tracking" in Fusion, IntelliTrack,
  Neural Engine (2026-09-07)
- https://borisfx.com/products/mocha-pro/ and
  https://borisfx.com/documentation/mocha/2026.5.0/
  — the planar tracker, "translation, rotation and scaling data based on the
  movement of a user-defined plane", PowerMesh, AdjustTrack, Export Data
  Dialog, the list of hosts (2026-09-07)
- https://community.adobe.com/announcements-527/new-in-ae-object-matte-ai-powered-rotoscoping-1557065
  — the official announcement of `Object Matte` in AE and all user quotes
  (2026-09-07)

Forums:

- https://creativecow.net/forums/thread/ae-mocha-tracking-planar-surface/ (2026-09-07)
- https://creativecow.net/forums/thread/mocha-issues-with-tracking/ (2026-09-07)

Secondary sources and what could not be confirmed:

- The Premiere line relies on https://petapixel.com/2026/01/20/rebranded-adobe-premiere-26-arrives-with-one-click-object-tracking/
  (2026-09-07) and on a search result with an Adobe quote "From the toolbar, you can
  quickly create an Object Mask…". The claim "bidirectional tracking and
  3D perspective" was found only in secondary publications and is marked in the text
  as **[NOT VERIFIED]**: `helpx.adobe.com` (including the pages
  `track-masks.html` and `mask-tracking-tools.html`) does not open from this environment, and the mirror `helpx-origin-*.adobeitc.com` also rejects
  the connection.
- Resolve's `Magic Mask v2` with a brush in 20.x is confirmed only
  by secondary reviews (reduser.net, jayaretv.com); a primary source for
  the version in which it appeared could not be found.
- That Mocha AE is "bundled with After Effects" is known from the list of hosts
  of Boris FX and from reviews; it was not confirmed by a separate official page in this session.
- `forum.blackmagicdesign.com` answered HTTP 403 to all direct requests (`viewtopic.php`,
  `viewforum.php`, `search.php`), so there are no quotes about track loss in
  Magic Mask in the note — only thread titles from the search results, cited as the fact that complaints exist, not as quotes.
  Also unavailable: `reddit.com`, `web.archive.org`, `discussions.apple.com`,
  and the old forum `vegascreativesoftware.info` (301 to the root of
  `forum.borisfx.com`, the threads did not move to the addresses). The `WebSearch` budget
  in the session is exhausted; the search went through direct queries to Yahoo/Bing/Mojeek.

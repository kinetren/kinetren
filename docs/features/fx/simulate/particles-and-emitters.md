---
id: F-FX-006
title: Particles and emitters
status: researched
priority: C
cost: XL
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-fx, kt-media, kt-engine, kt-gpu]
invariants: [4, 7, 11]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-FX-005]
trackers: []
services: []
checked: 2026-09-21
---

# F-FX-006 — Particles and emitters

## Essence

From a point (a line, an area, shapes) objects pour out: sparks, snow, rain,
smoke, confetti, dust in a beam. The editor draws none of them — they set the source, the birth rate, the lifetime, the initial speed and the forces that act on them.
The difference from an "effect" is that the amount of what is drawn is not known in advance and changes from frame to frame: this is not a filter over a picture but a small world that has to be lived through.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | Particle Playground, CC Particle World / CC Particle Systems II (built-in) | built-in ones exist, but the industry standard for fields is a THIRD-PARTY **Trapcode Particular**: AE has the same arrangement as with physics (F-FX-005) — it has its own, but people use someone else's. The simulation runs frame by frame, and "not from the start" gives the wrong frame: Particular has a separate parameter for this, Pre-Run, in Emitter/Emission Extras |
| DaVinci Resolve | 21 | **Fusion: pEmitter → pRender** (plus `pGravity`, `pBounce`, `pTurbulence`, `pDirectionalForce`, `pCustom`…) | the only editor of the core with a REAL particle system inside the product. The key detail of the design is the split in two: the particle nodes output a set of particles, not a picture, and are not shown in the viewer at all; the picture is made by a separate node `pRender`, which has its own pre-roll |
| Final Cut Pro | 12.3 | has none of its own; emitters and replicator come from **Motion** | Motion has an emitter (particles) and a replicator (a regular lattice of copies) — two different things with a common origin. FCP does not create or configure them: it CONSUMES them as Motion templates with parameters taken outward |
| Adobe Premiere | 26.x | none | the task goes to After Effects |
| VEGAS Pro | 2026.0 | none | the nearest are third-party OFX sets (the same Boris FX), that is, an alien ecosystem as a whole |

## The trick behind it

**1. Particles are the same historical problem as physics (F-FX-005), plus one more:
the number of objects is not known in advance.** Therefore a particle system cannot
be a pure function of the frame — UNLESS the randomness in it is
counter-based: the seed is computed from the frame number and the particle number, and is not taken from a generator that "goes further" with each call. Then
"which particles live on frame N" is computed directly, without frames 1..N−1.
This very technique is already used by our noise and VHS: `noise_seed(burst_index)`
in `src/libs/kt-fx/src/video.rs` — the seed comes from the absolute position on the timeline,
no state between frames. That is, half of the domain is already expressible with us today, and this is not a guess but working code.

**2. Fusion split the emitter and the renderer (`pEmitter` / `pRender`) not for the sake of a beautiful graph, but because the simulation and the drawing have DIFFERENT resolutions and DIFFERENT caches.** The particle set is coordinates and velocities, it does not depend on the frame size; the picture does. A preview at a quarter of the resolution must reuse THE SAME simulation, otherwise the quarter and the full size will give different sparks. And conversely: the pre-roll ("calculate the history up to the current frame") is needed by the renderer, not by the emitter — that is why the button lives on `pRender`. A single "particle effect", which has one input and one output, cannot express this difference in principle: it has one cache for two different things.

**3. Pre-Run in Trapcode Particular is a recognition of the same thing, but from the user's side.** On the first frame there are no particles yet, because nobody has had time to emit them, and the user sees an empty screen; it is cured by a parameter that asks the system to "live through" the specified share of the history before the first frame. That is, the parameter in the effect's interface exists EXACTLY to compensate for the absence of history — and this is not written in any list of features, yet people ask about it on forums every year.

## Pros

- One source describes thousands of objects: the cost in the model is constant, while the expressiveness is not; no other class of effects gives this.
- Counter-based randomness makes a subset of the domain compatible with a pure function of the frame: snow, rain, dust, sparks "without interaction" are computed on any frame directly.
- The split "particle set" / "drawing" gives a free win for the preview: the simulation is computed once and is suitable for any display resolution.
- The forces (gravity, turbulence, wind) are the same numbers that physics F-FX-005 needs: the two notes share one subsystem rather than creating two.

## Cons

- Interaction of particles with one another (collisions, sticking, repulsion) is not expressed at all by the counter-based model: as soon as a particle depends on its neighbour, the full history is back.
- The number of objects is a cost parameter, and the user turns it freely: an emitter with a million particles looks in the interface the same as one with a thousand.
- An empty first frame is the standard behaviour of any honest implementation, and it has to be explained to the user by a separate parameter (Pre-Run) or a separate button (`pRender`).
- Sorting by depth and blend modes make the result depend on the drawing order, that is, on exactly how the thousands of sprites were stacked; byte coincidence of the two executors is achieved here harder than for an ordinary filter.

## What users say

> Maybe because you are on first frame of your animation and particular did
> not emit any of particles yet? Try to move playhead somewhere further into
> timeline.
> — imeilfx, Adobe Community, 01.11.2017, https://community.adobe.com/t5/after-effects/cc-2018-trapcode-particles-not-appearing/td-p/9391987

> Pre-Roll is necessary because the state of a particle system is entirely
> dependent on the last known position of the particles.
> — reference of the pRender node (Fusion), https://jayaretv.com/fusion/prender-node/ (accessed 2026-09-21)

The pair reads as one statement from two sides. The user sees a symptom ("there are no particles") and gets the answer "you are on the first frame" — that is, an explanation through a history that had not yet happened. The documentation of the same mechanism in Fusion names the cause directly: the state of the system is determined entirely by the last known position of the particles, and not by the current time.
This is the boundary of the domain: everything that is not expressed by a formula of the frame number requires living through the history — and both products moved this price into a separate visible control (Pre-Run, Pre-Roll), instead of hiding it.

`forum.blackmagicdesign.com` and `helpx.adobe.com` are unavailable from this environment (403), therefore the Fusion reference was read from a public mirror of the node documentation; the wording was checked against the description of the same mechanism on the `pEmitter` page of the same mirror.

## How it fits Kinotrain

### What already exists

- **Counter-based randomness is a working technique of our video path.**
  `noise_seed(burst_index)` (`src/libs/kt-fx/src/video.rs:1031`) and its two calls in the noise and VHS: the seed comes from the absolute position on the timeline, the state between frames is zero. The same technique is applied in the generator too:
  `src/libs/kt-media/src/generator.rs` computes the seed of a cell from the base grid 1920×1080 and the `pts` of the material, so that the picture depends neither on the Quality nor on the decode scale step.
- **The split "decision" / "execution" is already done and protected with us.**
  `kt_fx::plan::plan_chain` (`src/libs/kt-fx/src/plan.rs:243`) makes all the decisions about the chain, and the CPU and GPU executors carry them out (invariant 7). The boundary for the sake of which Fusion split `pEmitter` and `pRender` is conceptually already drawn with us — only along a different line.
- **The decode scale step and intermediate resolutions are also a solved class.**
  `clip_decode_scale` (`src/libs/kt-engine/src/video.rs:352`) and `in_scale` in `VideoFxCtx`: "how many times the input is larger than the logic" already travels in the order. The question "simulation at one resolution, drawing at another" fits into the existing arithmetic and does not create a new one.

### What is missing

- There is no concept of an "object" in the video path: the chain takes a frame and returns a frame (`Frame` in `src/libs/kt-fx/src/video.rs:27`). A particle set is not a frame, and it does not pass into today's contract in any form.
- The post-FX key (`postfx_key_hash`, `src/libs/kt-engine/src/video.rs:1802`) expresses the description of the frame; for a particle system "how much history has been lived through" is added to the description, and there is no field for this and nowhere to take one from.
- There is no budget of "how many objects in the frame" at all: all our effects have a cost limited by the frame size, and not one has a parameter with which the user can multiply the work a thousandfold.

### What gets in the way

1. **Invariant 11.** The order of overlaying thousands of sprites must be declared and identical in the CPU and the GPU, otherwise the cross-backend conformance (t181/t182) will not converge even within tolerances: summation in a different order is exactly what `src/libs/kt-fx/CLAUDE.md` forbids as a class.
2. **Invariant 4.** Either the "lived-through history" enters the cache key — and the cache stops hitting on every seek — or it does not enter, and the cache lies. A third way out is to make the history unnecessary (counter-based), and it is also the only one that keeps the invariant whole.
3. **Invariant 7.** A particle system with two stages requires that the decision "which particles" be made once for the preview and the export; it cannot be placed anywhere except in the shared plan code.

### Which invariants it touches

- **No. 4** — "how much history has been lived through" is not expressed by today's key.
- **No. 7** — the two stages must remain one code of decisions.
- **No. 11** — the order of summing a set of objects and its coincidence on two
  executors.

## Estimate

| What | Answer |
|---|---|
| Size | XL — a new class "many objects instead of one frame", plus everything already listed for F-FX-005 |
| Where the main work is | kt-fx (emitter model and drawing), kt-engine (order and key), kt-gpu (mirror of drawing), kt-media (emitter as a kind of generator — the probable form) |
| Project format | a new version is needed |
| Tests to write | "frame N from a scrub == frame N from the start" for the counter-based subset; CPU↔GPU conformance on a scene of thousands of objects; "the preview at a quarter and the full size give ONE simulation" |
| Depends on | F-FX-005 (the shared subsystem of forces and step) |
| Breaks compatibility | no |

## Verdict

**Researched, priority C, after F-FX-005.** The order is exactly this:
particles are physics plus an unknown number of objects, and building the superstructure
before the foundation makes no sense. The main finding of the reconnaissance is an honest
subset: **a particle system WITHOUT interaction between particles, with a
counter-based seed (frame number × particle number), is expressible in our
current video path without a single amendment to the invariants.** Snow, rain,
dust, sparks, confetti are a function of the frame number, and our own
`noise_seed` proves that the technique works for us. Everything that requires
collisions and sticking goes into the same history as F-FX-005, and is solved there and
by the same checkpoints — no separate mechanism for particles will have to be created.

If this note is ever reopened, it should be reopened not with the question "should we create particles"
but with the question "what is a particle set in
our plan": Fusion's answer (the set is a separate entity, the picture is a
separate node) is the only one found that survives
a preview at reduced resolution.

## Sources

- https://community.adobe.com/t5/after-effects/cc-2018-trapcode-particles-not-appearing/td-p/9391987
  — the thread "trapcode particles not appearing", the answer by imeilfx of 01.11.2017 about
  the first frame and Pre-Run (access date 2026-09-21)
- https://jayaretv.com/fusion/prender-node/ — the reference of the `pRender` node:
  "Pre-Roll is necessary because the state of a particle system is entirely
  dependent on the last known position of the particles", the description of
  Automatic Pre-Roll and the Restart button (access date 2026-09-21)
- https://jayaretv.com/fusion/pemitter-node/ and
  https://jayaretv.com/fusion/particle-nodes/ — the composition of the particle nodes and the rule "all particle nodes except `pRender` output a set of particles, not
  a picture" (access date 2026-09-21). `[NOT VERIFIED against the primary source]`:
  the official Fusion documentation inside Resolve and
  `forum.blackmagicdesign.com` do not open from this environment; the mirror was
  cross-checked against two pages
- https://creativecow.net/forums/thread/trapcode-particular-disappearing-particles-problem/
  — the thread of 24.02.2015 about disappearing particles at the layer boundary; it was read as a confirmation of the class of failure, the quote was not taken into the note (access date
  2026-09-21)
- `docs/features/fx/simulate/physics-and-dynamics.md` (F-FX-005) — the shared subsystem of history and the owner's decision about the fixed step;
  `docs/user-feedback.md`, section "Round R.59 (2026-09-20)", item 4 — the requirement of zero cost for a project without tricks.

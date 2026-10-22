---
id: F-FX-005
title: Physics: motion that is not in the keys
status: researched
priority: B
cost: XL
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2]
we_have: no
crates: [kt-model, kt-media, kt-engine, kt-text]
invariants: [4, 7, 11]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-TXT-006]
trackers: []
services: []
checked: 2026-09-21
---

# F-FX-005 — Physics: motion that is not in the keys

## Essence

The editor does not set keyframes but gives a rule: "fall", "bounce", "chase the cursor with inertia", "scatter". After that the motion comes out by itself and looks plausible precisely because nobody drew it: the speed accumulates, the impact absorbs part of the energy, the letters fall out of sync. The classic occasion for the owner is the letters of a title that must fall when the event slows down and jerks. Keyframes are fundamentally unsuitable here: they would have to be placed one per letter, and any edit of the text or the speed would wipe out the work.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | no built-in rigid-body physics | AE's own is only the Puppet tool mesh (it has damping/stiffness, but this is deformation of a grid, not body dynamics) and "inertia" recipes made with expressions (`velocity`, a decaying sine in a property). The industry standard for the field is a THIRD-PARTY plugin **Newton** (Motion Boutique): it simulates 2D layers as rigid bodies and returns the result to the composition as ordinary keyframes. So even where physics exists, it lives outside the render |
| Adobe Premiere | 26.x | no | an editing application; there is no dynamics in any form, the task goes over to AE |
| DaVinci Resolve | 21 | on the Edit page — no | there is no physics on the Edit page at all. Forces and collisions live in Fusion (`pGravity`, `pBounce`, `pDirectionalForce` over a particle set) — this is the neighbouring note F-FX-006, and it is characteristic that the physics there appeared precisely as part of PARTICLES, not as a clip effect |
| Final Cut Pro | 12.3 | none of its own; **behaviors** arrive from Motion | the strongest line of the table. Motion has a Simulation behaviors group — Throw, Gravity, Spring, Attractor, Repel, Vortex, Orbit Around, Random Motion, Edge Collision — and these are NOT keyframes: a behavior does not create keyframes, applies to the object as a whole and combines with other behaviors and with keyframes at the same time. In FCP they arrive inside Motion templates |
| VEGAS Pro | 2026.0 | no | no dynamics and no behaviors; "motion" means Pan/Crop, Track Motion and keyframes |

**Outside the core.** Without Blender the topic cannot be described: it is the only one of the available programs where simulation is declared a subsystem with its OWN storage, and one can see what it costs.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Rigid Body / Soft Body + Rigid Body World → **Cache / Bake** | physics is not a function of the frame: the simulation world has a point cache with a frame range, and until it is baked, scrubbing does not give the right state — a frame is computed only as a continuation of the previous one. Hence the whole routine: "Bake" computes the range as a whole and makes the timeline scrubbable; a baked cache locks the settings until "Delete Bake" is done; a jump forward and a return back leave in the cache the state of the wrong frame |

## The trick behind it

**1. A simulation is not a function of the frame but a function of the ENTIRE history up to it, and every program that introduced it admits this in the same way: with a cache and a "bake" command.** In Blender this is the point cache and the Bake button of the Rigid Body World: before baking, the state at a frame is undefined, because the frame is computed as a continuation of the previous one, and scrubbing forward and back leaves in the cache the state of someone else's frame. Newton takes this to the limit: the plugin does not take part in AE's rendering at all — it opens its own window, solves the problem, and returns ORDINARY KEYFRAMES into the composition, after which it exits. So the most popular physical tool of the most animation-oriented program in the industry is built as a converter "rule → keyframes", not as an effect in a chain. The industry's formulation, shorter than it can be: physics is either cached or baked; there is no third way to make it scrubbable.

**2. Motion's behaviors are a closed form precisely FOR the sake of scrubbing, and this is the reverse side of the same choice.** "Gravity" as a behavior is not an integrator but an equation of time: the displacement is computed from the time since the start of the behavior, so frame 300 is taken without frames 1–299, and the playhead can be dragged backward. The price for this is exactly what a closed form does not have: collisions between objects (Edge Collision hits the edge of the frame, not a neighbour), accumulation of energy and any feedback. That is why Apple can afford "physics" on every object without a cache and without baking, while Blender cannot: they have different tasks, but the boundary between them runs not through quality but exactly through the presence of history.

**3. People start asking for physics specifically of TEXT because letters already have coordinates.** Any simulation needs initial conditions — bodies, their positions and sizes. For arbitrary video there is nowhere to take them from (a frame is pixels), while the layout of a title hands them over for free: each letter already has its box computed, because otherwise it could not have been drawn. Hence the market too: falling letters are the most frequent showcase clip for physics plugins for AE, although their API knows nothing about letters. The same explains why this note sits as a neighbour of F-TXT-006 in our project: "per letter" already addresses parts of the caption, and physics would only have to replace the law of motion.

## Pros

- A rule instead of keyframes survives an edit of the content: add a word to the title — it falls together with the others, because there are more bodies now, while keyframes would have to be placed anew.
- The motion comes out non-synchronous without manual work: each body has its own initial speed and its own contact, and exactly this separates "falling" from "shifting forty layers along one curve".
- A link to an already existing event parameter (speed, jerk) gives the editor one knob instead of forty: slow down — the letters hang; jerk — they scatter.
- A closed form (the Motion path) gives part of the effect without any subsystem at all: gravity, throw and spring are three formulas of time, and they are scrubbable by construction.

## Cons

- The cost of a frame stops being constant: the further the frame is from the start of the event, the more expensive it is to compute, and when scrubbing to the end of a long event the user pays for the entire history at once.
- Any edit of a parameter invalidates the whole computed history, not just the current frame: in the interface this looks like "I moved the slider — and everything started thinking".
- The simulation cache is a second store of truth next to the frame cache, and its invalidation does not coincide with theirs (an edit of the gravity changes none of the parameters of the effect chain but cancels everything).
- Baking into keyframes (the Newton path) solves the speed and kills the link to the source: after the bake an edit of the text no longer shows up in the motion, because the keyframes do not know where they came from.
- Collisions between bodies are non-local: a body depends on its neighbours, so "compute only the visible part of the frame" is impossible, and the usual ways of cutting the work into tiles are not applicable.

## What users say

> Newton interprets 2D layers of the current composition as rigid bodies in
> a physical world.
> — Newton 2.1 User's Guide, Motion Boutique, https://www.motionboutique.com/files/newton2/documentation/ (accessed 2026-09-21)

> The simulation is solved again, but this time animation data are stored in
> memory. At the end of the simulation, stored values are converted to
> standard After Effects keyframes, and Newton quits.
> — same place, section on rendering the simulation (accessed 2026-09-21)

How to read this. The second quote is the passport of the whole domain in three sentences: the physics is solved SEPARATELY, the result is laid into memory, turned into keyframes, and the tool exits the process. Not "an effect computes the frame", but "an external solver once produced the data". That is exactly why the AE line in the table above is written honestly: the most animation-oriented program in the industry has no dynamics of rigid bodies of its own, and what everybody uses lives outside its pipeline and returns the result in the language the pipeline already understands.

It was not possible to add verbatim user complaints to this, and this should be said directly: `docs.blender.org`, `projects.blender.org` and `helpx.adobe.com` return 403 from this environment, and Apple's pages on Motion behaviors return only the table of contents of the manual without the article body. The facts about the Blender and Final Cut lines in the table were collected from secondary retellings of the official pages and are marked in the Sources section as `[NOT VERIFIED against the primary source]`.

## How it fits Kinotrain

### What already exists

- **The initial conditions for the letters are computed and exposed outward.**
  `src/libs/kt-text/src/lib.rs` — `LayoutInfo` has a field `clusters`:
  the boxes of clusters in text order, with end-to-end numbering over the whole title.
  This is exactly a list of bodies: position, width, height. It was set up in R.53 for animation per letter (F-TXT-006), and there is nothing to compute a second time.
- **The event's speed is already described by numbers and is already signed.**
  `src/libs/kt-model/src/warp.rs`: `ClipWarp` and `rate_at(base, points,
  tau)` — a continuous speed function over the material, with signed R.57. "A slowdown and a sharp jerk", which the owner asks for, are exactly the value and the derivative of this function.
- **Deterministic pseudo-randomness is already used in the video path.**
  `noise_seed(burst_index)` in `src/libs/kt-fx/src/video.rs` — the seed
  is computed FROM THE FRAME NUMBER, rather than stored between frames. This is a ready
  example of how "different on every frame" coexists with the absence of state.
- **A precedent for "a fixed step instead of history" exists in the project — in sound.** `src/libs/kt-engine/src/fxstore.rs` holds the state of the chains
  by `ClipId`, and the reset of the history after a seek is paid for by an explicit pre-roll
  `max(tail, 100 ms)` (invariant 11) and by the function
  `chain_tail` (`src/libs/kt-fx/src/audio/mod.rs:191`), which honestly says how much tail the chain has. The notion of "how much prehistory must be computed so that the frame is correct" already exists with us — but only for sound.

### What is missing

- **The video path is strictly stateless, and this is not an accident but a load-bearing structure.** `VideoFxCtx` (`src/libs/kt-fx/src/video.rs:10`) carries
  exactly six things: `out_w`, `out_h`, `u`, `burst_index`, `cam3d`,
  `in_scale`. No previous frame, no pass number, no reference to anything that survives the frame. Both memoizations rest on this purity:
  `postfx_key_hash` (`src/libs/kt-engine/src/video.rs:1802`) and `plan_key`
  (`src/libs/kt-gpu/src/engine.rs:1900`) hash the DESCRIPTION of the frame and
  assume that the same description gives the same bytes. Simulation cancels this assumption directly.
- **There is no notion of a pre-roll for video at all.** Sound has a tail, has
  `reset()` on seek, has a verified rule "compute 100 ms before the point". There is not a single place in video where a frame depends on its neighbour — so there is also no mechanism that would express this.
- **The generator does not know the speed of the event.** `src/libs/kt-media/src/generator.rs`
  returns a frame via the function `render_at(kind, pts, w, h)`, and outwards — via `read_video_frame(pts, quality)`: the generator's input is the MATERIAL tick and the size, and nothing else. The speed lives a floor higher, in the engine, and physically does not reach the generator today — yet the owner asks for the falling of the letters precisely FROM IT.
- **Bodies do not exist as a concept.** `clusters` are layout boxes, not objects with mass and speed; between "there are coordinates" and "there are bodies" lies a decision about what a body is and where it is declared.

### What gets in the way

1. **Invariant 11 (determinism).** It does not forbid simulation — it forbids non-deterministic simulation. Any step that depends on WHICH FRAME the computation started from breaks the golden reference immediately and silently: export starts from the beginning, while the preview starts from the point where one clicked.
2. **Invariant 4 (the cache is the only truth).** Today the frame key expresses everything the frame depends on. Simulation adds a dependence on the frame number of the START of the computation, and if it is not put into the key — the cache will start returning someone else's state; if it is put in literally — the cache will stop hitting at all.
3. **Invariant 7 (one composition code).** The decision "what to compute and with what step" must live where the other plan decisions live
   (`kt_fx::plan::plan_chain`, `src/libs/kt-fx/src/plan.rs:243`), otherwise the preview and the export will diverge on the very first frame — they have different histories of access.
4. **The requirement of zero cost (item 4 of R.59).** An event where there is no physics must not pay a single operation; an event with physics must not become more expensive than its neighbours. Therefore the flag "history is computed here" must be on the REQUEST, and not be checked in the hot path.

### Which invariants it touches

- **No. 11 (determinism)** — the main one: the result must not depend on where one started looking.
- **No. 4 (the cache is the only truth)** — something appears in the key that is absent today.
- **No. 7 (one composition code)** — the decision about the step and about the prehistory is made once, in the shared plan code.

## Estimate

| What | Answer |
|---|---|
| Size | XL — not an effect but a new axis of frame dependence; it touches the cache keys and the determinism contract |
| Where the main work is | kt-model (what declares a body and a rule), kt-engine (delivery of the event speed and of the prehistory into the request), kt-media (the generator accepts numbers), kt-text (the cluster boxes as initial conditions) |
| Project format | a new version is needed: a simulation rule is absent from the model as a class |
| Tests that will have to be written | "frame N from a scrub == frame N from the start" (byte for byte); "an event without physics pays nothing"; "an edit of the text changes the number of bodies, rather than breaking the motion"; the golden is not touched |
| Depends on | F-TXT-006 (addressing parts of the caption), the neighbour — F-FX-006 |
| Breaks compatibility | no (backward compatibility has been waived by the owner, but the class is new and old files do not contain it) |

## Verdict

**Researched.** The owner's decision in R.59 (`docs/user-feedback.md`, section
"Owner's decisions") is in essence this: **simulation is made a FUNCTION again.** The state at a frame is computed from the start of the event with a fixed step, and then it is unambiguous — that is, the cost is paid by counting, not by storing. Checkpoints in memory are allowed, but only as an OPTIMIZATION: the bytes of the frame must not change depending on their presence, otherwise exactly the dependence on the viewing history comes back, for whose absence the whole thing was undertaken. And third: **physics feeds the generator with NUMBERS, not pixels** — it computes where the letter has moved, and it still draws with the same composition code (invariant 7); physics has no path of its own rendering.

The R.59 round writes a note and an interface, but not a subsystem: the scope of the round is the foundation (the property model, parameters of generators on an event, the public parameters of the composition), and physics within it is paper and types.

**An open question for the next round, and there is exactly one: what the fixed step equals and what limits the cost of a long event.** A step tied to the project's fps makes the result dependent on the project setting (change 25 to 30 — the motion became different); a step not tied to fps drifts out of the frame grid and requires interpolation between steps. The cost: for an event five minutes long at a step of 1/120 s — thirty-six thousand steps up to the last frame, and for this a ceiling with a declared behaviour is needed (cut the history? freeze? refuse with a notice?). The industry has no answer: Blender pays with a cache, Newton — with baking into keyframes, Motion — with abandoning history altogether. While the question is not closed, writing code is impossible — it would be a fourth answer, chosen at random.

## Sources

- https://www.motionboutique.com/files/newton2/documentation/ — Newton 2.1
  User's Guide: "Newton interprets 2D layers of the current composition as
  rigid bodies in a physical world", "Once simulation is completed, Newton
  export the animation to After Effects using standard keyframes", the description
  of rendering the simulation with the output of the plugin (accessed 2026-09-21)
- https://www.motionboutique.com/products/newton-4 — the current version of Newton and
  the wording about "editable keyframes for post-simulation tweaks"
  (accessed 2026-09-21)
- https://docs.blender.org/manual/en/latest/physics/baking.html — the page
  "Baking Physics Simulations" of the Blender 5.2 LTS manual; `[NOT VERIFIED
  against the primary source]`: the domain returns 403 from this environment, the content
  of the Cache/Bake section (baking a range, locking the settings until
  Delete Bake, an incorrect state on jumps along the timeline) was taken from
  secondary retellings of the page (accessed 2026-09-21)
- https://support.apple.com/en-ie/guide/motion/motnf425e02f/mac — "Behaviors
  versus keyframes" of the Motion manual; `[NOT VERIFIED against the primary source]`:
  the page returns only the table of contents without the body of the article; the composition of
  Simulation behaviors (Throw, Gravity, Spring, Attractor, Repel, Vortex, Orbit Around,
  Random Motion, Edge Collision) and the claim "a behavior creates no keys" were taken from
  secondary retellings of the official manual (accessed
  2026-09-21)
- `docs/user-feedback.md`, section "Round R.59 (2026-09-20)" — the owner's verbatim
  wording about the falling letters and his decision about the fixed step;
  `docs/features/titles/text/text-animation-presets.md` (F-TXT-006) — addressing
  of parts of the caption and the requirement of layout stability.

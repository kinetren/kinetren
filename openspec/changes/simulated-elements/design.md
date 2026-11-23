# Design: simulated-elements

> DESIGN ONLY, revision 2 (2026-11-23), base `e7b19346`: the owner's D3 — an architecture, not an animator. Revision 1
> (`text-letter-physics`, 2026-11-22) was a title's Physics animator; its physics survives as the first application
> (D8). Q1–Q2 in `proposal.md`; defaults recommended.

## Context

- **The video path is stateless by construction.** `VideoFxCtx` carries the size, the position and the camera, and
  nothing that survives a frame. The composite memo (`plan_key`), the disk frames (`durable_hash`) and invariant 11
  rest on "the same description gives the same bytes" (F-FX-005 "What is missing").
- **One precedent of a frame depending on its past: the stream stage (Datamosh).** It has paced history, reports a frame
  "incomplete, N positions lacking", and an export pre-rolls it (video-plan "A live preview paces a stream stage's
  history"). It works on PICTURES; this design needs only NUMBERS.
- **Parts of a picture that already have coordinates:**
  - a title's cluster boxes (`kt_text::LayoutInfo::clusters`);
  - mask primitives and path vertices (`kt_model::mask::MaskShape`);
  - tracker points (`point-tracker`, designed);
  - a composition's child layers (the reel's sub-plan, `NestKey`);
  - 3D group planes (`real-3d-groups`, designed);
  - a transition's panels (`TransKernel::ext`).
- **Drivers that already exist or are designed:**
  - links (R.61, `kt_model::links`);
  - the outer map's channels (`motion-driven-params`);
  - the audio-reactive listening (generators, t3716);
  - the morpher's warps.
- **The owner's rules (R.59):** simulation is a function from the start with a fixed step; physics gives numbers, not
  pixels; control from plugins; zero cost where unused.

## D1. The element set — the one new noun

```
ElementSet { source: ElementSource, ops: Vec<Operator> }
Element    { id: ElemId (stable), rest: Pose, size: [f32; 2], mass_hint: f32 }
Pose       { pos: [f64; 2] (or 3 in a 3D group), rot: f64, scale: [f64; 2], opacity: f64 }
```

- `ElementSource` names a producer and its granularity: `TitleClusters | TitleWords | TitleLines` of a title,
  `MaskVertices | MaskShapes` of a mask, `TrackerPoints` of a tracker, `ReelLayers` of an instance, `GroupPlanes` of a
  3D group, `TransitionPieces` of a transition, `Emitter` of a particle generator.
- A producer gives, for a moment, the element list with rest poses. It is the same function the producer already uses
  to draw: the layout's boxes, the mask's evaluated vertices, the tracker's point at that frame.
- **Identity.**
  - Most producers number elements by their own stable index: a cluster index, a vertex index, a point id, a layer's
    track id.
  - A text edit that changes the number of clusters re-homes the elements by index. The simulation restarts from rest
    at the owner's start, because the state is a function of the edit (D4).
  - Particles have the identity (spawn moment, spawn index), and their seed is a hash of it (no RNG state).
- An element is addressable as a property (`<owner>/elements/<id>/pos.x` …), so a link can read a simulated pose (a
  glow that follows the third letter) and a plugin can drive one.

## D2. Operators — stateless and stateful on one list

An operator maps the poses of a set at a moment to new poses.

- **Stateless:** keys and curves per element, today's per-letter animators (F-TXT-006, re-expressed here), a link
  expression. Closed form, cost per frame only.
- **Stateful (simulation):** a solver from the `sim` plugin family with its passport rows (curves allowed). Each element
  is a body tied to its pose AFTER the stateless operators before it, its "home". Bodies are moved by forces from
  drivers (D3), and the solver integrates.
- The order is the list's order, as the effect chain: keys → spring → a small wobble key on top is legal.

The `sim` family (kt-plugin-api) is shaped like the other kernels:

```
trait SimKernel {
    fn decl(&self) -> &SimDecl;                     // passport, version, state size per element
    fn init(&self, p: &[f64], e: &Element, s: &mut [f64]);
    fn step(&self, p: &[f64], env: &StepEnv, homes: &[Pose], state: &mut [f64]); // one fixed step, all elements
    fn pose(&self, p: &[f64], home: &Pose, s: &[f64]) -> Pose;
}
```

- `StepEnv` holds the step length, the drivers' values at this step, and the collider list (D3).
- The state is plain f64 rows, so a checkpoint is a copy and its hash is cheap.
- Built-in solvers (`plugins/sim-basic`):
  - spring (sway);
  - pendulum (hang);
  - rope (drag behind the previous element);
  - fall-and-collide (tear off past a jolt threshold, gravity, bounce on colliders);
  - particles (spawn, lifetime, forces);
  - flock (separation, alignment, cohesion; later).

## D3. Drivers — only what the edit determines

A force term reads one of:

- **The outer time map's channels** of the owner's instance path: speed, acceleration, speed change
  (`motion-driven-params` D1). This is the example's driver: a crawl braked by Slowed&Reverb, or a composition braked
  from outside, gives the homes an acceleration and the bodies lag behind it.
- **The home's own acceleration.** d²(home)/dt², from the stateless operators' closed form. Keyed motion of a mask or a
  layer then gets inertia without any outer speed.
- **A link source.** Any property by address (R.61): an effect's parameter, another event's level, a marker's distance.
- **Sound.** A track's loudness or band energy at the step's moment, from the analysis the audio-reactive generators
  already read (a pure function of the files, t3716).
- **A tracker.** A point's motion as the home of a set (a mask on a tracked face that follows with inertia).
- **Another set.**
  - Its elements are colliders or attractors: letters fall onto a mask's floor, particles flow around a title.
  - Sets form a dependency graph, evaluated in topological order at each step. A cycle is refused at edit time, as a
    link cycle is (R.61), naming the sets.
  - Two sets that collide with each other are declared as ONE set with two sources, so they step together.

## D4. A function of the edit — time, step, instance

- **Clock.** The owner's OUTER time on its instance path: the timeline time of the outermost reel the owner is played
  in, through every nesting map. Two instances of one composition at different speeds therefore run two simulations
  and give two looks — the owner's B7 case.
- **Origin.** The moment the owner starts on that path; the state there is rest.
- **Step.** Δ = 1/240 s of outer time (Q2 a), semi-implicit Euler, f64, `kt_plugin_api::math` only (invariant 11).
  A frame at t reads the state interpolated linearly between the steps around t.
- **Instance identity.** The simulation key = hash(the set's source content, the operators' values, the drivers'
  profiles over [origin, t], the instance path). The drivers' profiles are closed-form curves, so their hash is a hash
  of the curves, not of samples.
- **Checkpoints.** The full state every 240 steps, in memory per simulation key (bounded LRU, counted in the memory
  ledger). They are never written to disk: the owner's D1/D2 — no intermediate results stored. A test proves the bytes
  are equal with and without them.
- **Cost bound.** Work = elements × steps since the nearest checkpoint below t.
  - A cold seek deep into a long simulation computes from the origin. It runs on the compose worker, never on the UI
    thread.
  - Until it reaches t, the frame reports incomplete ("simulation: N s behind"), as a stream stage does, and the
    preview shows Loading (the owner's choice for stream stages, found-defects 403).
  - An export integrates forward from the origin, as it pre-rolls a stream stage.
  - A set declares a ceiling on elements (particles: 20 000 by default); past it, spawning stops with a notice and no
    silent cap.

## D5. In the plan — numbers, keyed

- `frame_plan` builds a layer's plan as today. For an owner with element sets it asks `kt-sim` for the placed poses at
  this moment (memo by simulation key + t) and writes them into the layer's plan as numbers:
  - glyph offsets for the title's rasterization;
  - vertex positions for the mask raster;
  - transforms for reel layers or 3D planes;
  - an instance buffer for particles.
- The plan key hashes those numbers. A memo hit or a disk frame (persistent cache) therefore stays exact without any
  history in the key (invariant 4), and invariant 7 holds: the drawer is the producer's own.
- The `durable_hash` of a frame with a simulated set includes the poses. The frames on disk are final pictures (the
  owner's D1), and nothing else of the simulation is kept.

## D6. Drawing

- **Title.** The rasterizer takes a per-cluster pose (offset, rotation, scale, opacity), as it takes animator offsets
  today.
- **Mask.** The raster takes evaluated vertices; a simulated vertex simply replaces the evaluated one.
- **Reel layers, 3D planes.** Their transforms are the Geometry node's numbers, composed after their own.
- **Particles.** A new instanced-sprite pass in kt-gpu draws N quads from a pose buffer with a sprite (a generator's
  picture, a glyph, an image), additive or over. It is one pass, warmed with the project (video-plan "A frame never
  compiles").
- **Transition pieces.** The volumetric kinds already take panel homographies (`TransKernel::ext`); a simulated piece
  feeds those numbers.

## D7. Zero cost where unused (R.59 item 4)

- An owner with no element set has no `kt-sim` call in its plan and the same plan key as before. This is a test: the
  bytes and the key of a project without simulation are unchanged.
- An owner with only stateless operators pays the closed form per frame.
- A simulated owner pays its own steps and slows no other layer: the steps run on the compose worker per owner, and
  the frame of a moment where it is not visible does not step it.

## D8. The first applications (Q1 a)

1. **Letters driven by the edit's speed** (the example):
   - the set is `TitleClusters`, the operator is spring (default) or hang, drag, fall;
   - the driver is the outer acceleration (D3), the drawer is the title.
   - Revision 1's dynamics stay as the solvers' laws:
     - spring `m·p'' = −k·p − c·p' − m·a_home`;
     - pendulum `θ'' = −(g/L)·sin θ − c·θ' − (a_home/L)·cos θ`;
     - rope (a spring of rest length to the previous element);
     - fall (detach when |m·a_home| > threshold, gravity, restitution 0.3).
2. **Mask vertices following a tracker with inertia**:
   - the set is `MaskVertices`, the home is the tracker-driven vertex, the operator is spring;
   - a rotoscoped edge then lags and settles naturally instead of snapping with the track's jitter.
3. **Particles from an emitter**:
   - the set is `Emitter` (a generator kind with spawn rate, lifetime, initial velocity, as rows with curves);
   - the forces are gravity, wind, attraction to another set, and a sound-driven burst;
   - the drawer is the sprite pass (F-FX-006).

## D9. Class walk — "where in the whole program parts of a picture could have simulated state"

| # | Place | Elements | A simulated use | Producer today | Here |
|---|---|---|---|---|---|
| 1 | Titles (text generator) | clusters, words, lines | letters sway, hang, drag, fall with the edit's speed | `LayoutInfo::clusters` | first (D8.1) |
| 2 | Title word effects (`title-word-effects`) | text-range spans | a word knocked out of the line by a beat | the range mask's spans | uses the system |
| 3 | Subtitles (`subtitles-and-text-editing`) | words of a cue | karaoke words bounce in on their timing | cue word timings | later |
| 4 | Masks | vertices, primitives | jelly mask, edge lag on a tracked shape | `MaskShape` evaluation | first (D8.2) |
| 5 | Magic wand mask (`magic-wand-mask`) | outline vertices | a smoothed, inertial outline across frames | the wand's contour | later |
| 6 | Point tracker (`point-tracker`) | tracked points | a spring smooths a shaky track (a stabilizer's path) | the tracker's samples | home source for #4 |
| 7 | Generators | shapes, particles | an emitter, falling confetti, rain on a lens | a generator kind | first (D8.3) |
| 8 | Lens dirt and drops (v26.10 line 108, item 1) | drops and specks on the lens | drops run down and smear when the camera moves | `fx-lens-and-camera` | later, from the same emitter |
| 9 | Compositions (reels) | child layers | secondary motion: layers overshoot when the instance brakes | the reel's sub-plan | later |
| 10 | Morpher | layers under a morpher | a morph layer's warp as a driver of every set under it | the morpher's warp | a driver (D3) |
| 11 | Transitions | panels and pieces | shatter pieces fall with gravity | `TransKernel::ext` panels | later |
| 12 | Geometry and camera (CamMotion) | the event's placement, the camera | a handheld camera: a spring on the keyed path, shake from speed | the Geometry node, `cam3d` | later |
| 13 | Real 3D groups (`real-3d-groups`) | planes | planes as rigid bodies that tumble and collide | the group's planes | later |
| 14 | Audio-reactive generators | bars, dots | bars with mass that overshoot the level | the reactive kinds | a driver and a set |

Places where the system does NOT apply, checked:

- **The timeline and the panels.** The interface is not the composite; its motion is not part of a frame (invariant 12).
- **Audio effects.** The render thread's own state (invariant 2); a sound's "physics" is a filter.
- **The export's encoder.** It does not compose.

Paired sound (the owner's simulated-device rule) is checked: these are not devices that record sound. One future link is
recorded instead: a set's collision events as a driver of an audio event (an impact sound when a letter lands). The
collisions would be a pure function too, so the sound would land on the frame.

## D10. Idle

- Nothing runs at rest.
- A paused frame computes its numbers once; the plan memo reuses them.
- Hover and an open window cost nothing extra. The rest-pose overlay is drawn from numbers already in the plan.
- The load check runs on the heavy showcase with a simulated title and an emitter added.

## D11. What the tree has (engine side, spec-engine, checked at orch `d613b228`)

- **A title is not drawn in the plan: it is a media source.** `kt-media/src/generator.rs` (`GeneratorSource`,
  `read_video_frame` `:351`) rasterizes it through `kt-text` on the MediaWorker queue, and the plan reads it as
  `LayerInput::Frame` from the burst cache (`video_plan.rs:130`) under `(source, quality, index)`. A procedural source
  is asked at any tick (`video.rs:629`), but the source's time is the EVENT's material time, and one source serves every
  instance of a reel. So D5/D6's "glyph poses for the title's rasterization" cannot reach the rasterizer through the
  plan: poses on the outer clock of an instance path are not in the source's key, and two instances would read one
  cached raster (against t3971).
  - **Decision (engine side): a simulated title's letters are drawn in the plan by D6's instanced-sprite pass.** The
    source rasterizes the text ONCE into a glyph atlas (one sprite per cluster, `LayoutInfo::clusters`,
    `kt-text/src/lib.rs:160`), keyed by the text's values as today; the plan carries one pose per cluster, and the
    pass places the sprites. A title with no element set keeps today's path, bytes and key (D7; the title goldens and
    t535 stay as they are). At rest poses the sprite title equals the source's raster within one class
    (`FrameTol::BILINEAR` at an integer offset, EXACT at the rest pose) — t4107.
  - Particles and letters then share one pass and one atlas form: the "drawer is the producer's own" (D5) holds for
    masks, reel layers and planes, and is replaced for letters by the producer's atlas plus the shared pass.
- **Masks are evaluated at plan time** (kt-gpu never rasterizes a mask; `chain_ops` asks `kt-fx`'s rasterizer at the
  plan's positions, `PlanLayer.masks: Vec<MaskOp>` `video_plan.rs:118`): simulated vertices replace the evaluated
  ones in `MaskOp` before the plan is keyed. No new path.
- **Reel layers:** a reel's sub-plan is `LayerInput::Nest(NestSrc { seq, content_hash, inner_t, … })`
  (`video_plan.rs:172–182`); the outer clock of D4 is the ROOT plan's `t` carried down the recursion beside `inner_t`,
  and the instance path is the chain of `NestSrc` the recursion went through. Both are new arguments of the nested
  build, not fields of the source.
- **The keys:** `plan_key` (`kt-gpu/src/engine.rs:4330`) and `durable_hash`'s `put_plan`
  (`kt-engine/src/render_cache.rs:715`) hash the new pose data; t2981 ("a new field a plan key hashes is hashed here
  too") guards the pair.
- **The incomplete report** follows the stream stage's: a thread-local "short" counter (`STREAM_SHORT`,
  `note_stream_short`, `video.rs:2399–2410`) read into the preview's catch-up line. A simulation behind its moment adds
  a second counter (seconds behind), not a new mechanism; the advance off the compose thread follows
  `stream_advance_ahead` on the prefetch thread (`player.rs:3609`).
- **The export's forward integration** sits beside `stream_preroll` (`kt-engine/src/export.rs:510`).
- **Checkpoints in the memory ledger:** a new tenant kind appended after the last (`kt-time/src/mem.rs`, `KINDS`
  `:46`; `device-decode-sources` appends `Surfaces` first, so `Sim` takes the next number at merge).
- **Crates:** `kt-sim` (new, depends on `kt-plugin-api` and `kt-time`), the `sim` family in `kt-plugin-api`
  (beside `stream.rs`, `barrier.rs`), `plugins/sim-basic` and its id in `kt-plugins-builtin`; `kt-engine` depends on
  `kt-sim`. No FFmpeg or GPU in `kt-sim` (it is core).

## Rejected

- **A Physics animator on a title only (revision 1)** — rejected by the owner (D3): one control in one form, the system
  not built.
- **Simulation as a stateful playback-time process** — depends on where playback started (F-FX-005, R.59).
- **Baking to keys** (Newton) — every edit of the text or the speed would need a rebake.
- **Simulating pictures (per-pixel fluid, smoke) in this system** — those are effects with stream history (the stream
  stage); this system moves the parts of a picture that already have coordinates.
- **Checkpoints on disk** — the owner's D1/D2; memory and recomputation are enough at these costs.
- **Driving by the playhead** — the B7 lesson: the edit's speed is the driver.

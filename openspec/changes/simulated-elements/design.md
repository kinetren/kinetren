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

Where the poses are computed depends on who draws the elements today; the function (`kt-sim`) is one either way.

- **Plan-side sets** (mask vertices, reel layers, 3D planes, transition pieces). These elements are numbers the plan
  already evaluates at the frame's material tick: `LayerSrc.masks`, a sub-plan's transforms, the trans panels.
  `layer_src` asks `kt-sim` for the poses and writes the simulated numbers in place of the evaluated ones. They enter
  `fx_hash` and kt-gpu's `plan_key` as the evaluated numbers do today, and so `durable_hash`.
- **Atlas-drawn sets** (a title's glyphs, an emitter's particles). The picture of each element is made ONCE by the
  generator source on the media worker as an atlas (a title: one sprite per cluster, keyed by the text's values as
  today; an emitter: its sprite), and the PLAN places the elements: one pose per element, drawn by the instanced-sprite
  pass (D6). Two instances of one reel read one atlas and give two plans (D12, spec-engine). A title without a set
  keeps today's path, key and bytes.
- No history and no start moment ever enters a key (invariant 4). The frames on disk are final pictures (the owner's
  D1), and nothing else of the simulation is kept.

## D6. Drawing

- **Title.** With a set, the source rasterizes the glyph atlas and the instanced-sprite pass places each cluster by its
  pose (offset, rotation, scale, opacity); at rest poses the result equals the source's own raster (t4107). Without a
  set, the title is drawn as today.
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

## D10. The engine contract (implementation-ready, checked against orch `d613b228`)

1. **`kt-sim` is pure** (kt-time, kt-plugin-api). `Sim::state_at(rule, drivers, t) -> Poses` reads a `SimMemo` of
   checkpoints. The memo is shared by key: one per process, in the memory ledger as an account of its own, bounded, LRU.
   It never blocks: `state_at` either reaches t from the nearest checkpoint within a step budget, or answers
   `Behind { secs }`.
2. **The key is a prefix** (D12.2). `SimKey(t) = xxh3(set source content, operators' rows and curves, instance path,
   the drivers' closed-form curves restricted to [origin, t])`. The drivers' profiles come from `motion-driven-params`
   D3; the outer clock and the instance path go down the nested build beside `NestSrc.inner_t`. The plan and the probe
   hash `(SimKey(t), t)`, so an edit after a moment keeps its key (t3987). The per-event memo key (`incremental.rs`,
   t2004) takes only the static part (content, operators, instance path). No generator address is salted (D12.1).
3. **Catch-up off every hot thread.**
   - A `sim` thread of the player advances the memos at class `near`, like `stream_advance_ahead`: the frame on
     screen first, then ahead while playing.
   - Only the plan build calls `state_at`, with a small budget; the media worker never steps (it draws atlases). On
     `Behind`, the plan notes `note_sim_short(secs)` beside `note_stream_short` (`video.rs:2399`).
   - The frame is incomplete and the preview shows Loading. `render_ahead::Waits` waits on the sim thread's walk as on
     a stream walk (`kt_plugin_api::stream::walked` gets a sibling `sim::walked`).
   - An export calls `Sim::preroll` before its first frame and never sees `Behind` (no budget offline).
   - The UI thread never steps.
4. **Keys.** `plan_key` (kt-gpu `engine.rs:4330`) and `durable_hash`'s `put_plan` (`render_cache.rs:715`) hash the
   poses: the simulated numbers where they hash the evaluated ones today (`MaskOp` in `LayerSrc.masks`, the transforms)
   and the new per-element poses of atlas-drawn sets (t2981 guards the pair). The rule "a new field `plan_hash` hashes is
   hashed by `durable_hash` too" holds by construction.
5. **The probe plan** (`probe_plan`, `show_kept`) must not step. A plan-side set in a probe asks
   `Sim::key_only(SimKey, t)` and hashes the key with t, like `StreamKernel::key`. The kept composite is therefore
   found without the simulation, and a probe plan's key equals the full plan's.
6. **Zero cost.** An owner without a set takes none of these branches; t3986 checks the plan keys and the bytes of the
   golden projects unchanged.
7. **The sprite op is one new executor path** (D12.3). The plan op is an instance buffer of poses plus an atlas input.
   kt-gpu runs it as one instanced pass for letters and particles: a `Compiled` pipe with its `Slot`, timed, cut by
   the arbiter, warmed by `prewarm::effect_ids`. `plan_key` and `put_plan` hash `(SimKey(t), t)` and the atlas's
   leaf (t2981).

## D11. Idle

- Nothing runs at rest.
- A paused frame computes its numbers once; the plan memo reuses them.
- Hover and an open window cost nothing extra. The rest-pose overlay is drawn from numbers already in the plan.
- The load check runs on the heavy showcase with a simulated title and an emitter added.

## D12. What the tree has (engine side, spec-engine, checked at orch `d613b228`): corrections to D10

D10's points 1, 4 (plan-side), 5 and 6 hold. Three corrections follow from the tree; the first answers D10's open
question (does a media worker that answers `Behind` with starvation fit "request == read"?). **No: every set is
simulated plan-side, and a source never steps.**

1. **Source-side sets break t3987 and the kept composites.** A source's address cannot depend on the moment, so a
   salt from the `SimKey` must carry the drivers' WHOLE profiles. Then a driver edit after a moment re-salts every
   moment of the title; and a driver change through `Retune::Gen` runs `cache.invalidate_source` on the worker
   (`kt-engine/src/worker.rs:363–380`), which drops every frame of the source and forgets their serials. Every kept
   composite of the title misses, against t3987 ("a change after the moment keeps the key"). A worker that answers
   `Behind` also breaks the order's contract: the prefetch orders the frame, the worker gives nothing, the key stays
   wanted (an eternal re-order or a "failed" mark, `failed_video`), and the Loading line cannot say why.
   - **Decision.** The source draws only what does not move: a title's glyph atlas (one sprite per cluster,
     `LayoutInfo::clusters`, `kt-text/src/lib.rs:160`; rasterized by `GeneratorSource`, `kt-media/src/generator.rs`,
     keyed by the text's values as today), an emitter's sprite. The poses are computed in `layer_src` like every
     plan-side set, and an instanced-sprite op in the plan places the sprites. A title or emitter with no set keeps
     today's path, address and bytes (salt 0, D7). At the rest pose the sprite title equals the source's raster of the
     same title (t4107).
   - So D10 point 2's salting, point 3's worker branch and point 7's draw on the media worker go; the `sim` thread,
     `note_sim_short`, `sim::walked` and `Sim::preroll` stay, one mechanism for every set.
2. **The key is the prefix, not the whole profile.** `SimKey(t) = xxh3(the set's source content, the operators' rows
   and curves, the instance path, the drivers' closed-form curves restricted to [origin, t])`: a curve's keys up to t
   and the key after it (the segment t falls in). The plan and the probe both hash `(SimKey(t), t)` and the pose
   digest follows from it; `Sim::key_only` computes `SimKey(t)` without stepping (D10 point 5 holds with this key).
   The per-event memo key (`incremental.rs`, t2004) takes the STATIC part only (content, operators, instance path):
   drivers vary per moment, not per event.
3. **The sprite op is a new executor path, and it must be one.** Letters and particles become one plan op (an
   instance buffer of poses + an atlas input) that kt-gpu runs as one instanced pass: a `Compiled` pipe with its
   `Slot`, timed, cut by the arbiter, warmed by `prewarm::effect_ids`. `plan_key` (`kt-gpu/src/engine.rs:4330`) and
   `put_plan` (`render_cache.rs:715`) hash the op's `(SimKey(t), t)` and the atlas's content serial / leaf; t2981 holds
   the pair.

The rest of the tree, as D10 relies on it:
- **Masks** are evaluated at plan time (`PlanLayer.masks: Vec<MaskOp>`, `video_plan.rs:118`; kt-gpu never
  rasterizes a mask), so simulated vertices replace evaluated ones before the plan is keyed.
- **Reel layers:** `LayerInput::Nest(NestSrc { seq, content_hash, inner_t, … })` (`video_plan.rs:172–182`). The
  outer clock is the root plan's `t` carried down the nested build beside `inner_t`; the instance path is the chain
  of `NestSrc` the recursion went through.
- **The durable leaf** hashes the source's address (`note_leaf(source.0, …)`, `video.rs:2562`), so an atlas keyed
  by the title's values is already in the disk key; the poses enter through the op.
- **The incomplete report:** beside `STREAM_SHORT`/`note_stream_short` (`video.rs:2399–2410`), read into the catch-up
  line; the sim thread's advance follows `stream_advance_ahead` (`player.rs:3609`); the export's pre-roll sits beside
  `stream_preroll` (`export.rs:510`).
- **Checkpoints in the memory ledger:** a tenant kind appended after the last (`kt-time/src/mem.rs`, `KINDS` `:46`;
  `device-decode-sources` appends `Surfaces` first, `Sim` takes the next number at merge).
- **Crates:** `kt-sim` (kt-time, kt-plugin-api; no FFmpeg, no GPU), the `sim` family in `kt-plugin-api`,
  `plugins/sim-basic` with its id in `kt-plugins-builtin`; kt-engine depends on `kt-sim`.

## Rejected

- **A Physics animator on a title only (revision 1)** — rejected by the owner (D3): one control in one form, the system
  not built.
- **Simulation as a stateful playback-time process** — depends on where playback started (F-FX-005, R.59).
- **Baking to keys** (Newton) — every edit of the text or the speed would need a rebake.
- **Simulating pictures (per-pixel fluid, smoke) in this system** — those are effects with stream history (the stream
  stage); this system moves the parts of a picture that already have coordinates.
- **Checkpoints on disk** — the owner's D1/D2; memory and recomputation are enough at these costs.
- **Driving by the playhead** — the B7 lesson: the edit's speed is the driver.

# Design: simulated-elements

> DESIGN ONLY, revision 2 (2026-11-23), base `e7b19346`: the owner's D3 — an architecture, not an animator. Revision 1
> (`text-letter-physics`, 2026-11-22) was a title's Physics animator; its physics survives as the first application
> (D8). Q1–Q2 decided by the owner's own words (`proposal.md` "Decided"). D13–D18 (2026-11-24, orch `5986f08e`) close
> what an implementer would have had to invent: the model, the origin under cuts, the key of each driver, the frame of
> reference, the title's layers, memory, the stages. Where D1–D9 and D13–D18 differ, D13–D18 hold.

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
- Solver rows are addressable properties (D13). An element's placed pose is read by other sets and by the overlay. A
  LINK reading a pose (a glow that follows the third letter) is stage B (D18): links compile to curves once per build
  (R.61, compiled-curves), and a pose has no curve.

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
- **Origin.** The moment the owner starts on that path, cuts looked through (D14); the state there is rest.
- **Step.** Δ = 1/240 s of outer time (Q2 a), semi-implicit Euler, f64, `kt_plugin_api::math` only (invariant 11).
  A frame at t reads the state interpolated linearly between the steps around t.
- **Instance identity.** The simulation key = hash(the set's source content, the operators' values, the drivers'
  profiles over [origin, t], the instance path). The drivers' profiles are closed-form curves, so their hash is a hash
  of the curves, not of samples.
- **Checkpoints.** The full state every 240 steps, in memory per simulation key (bounded LRU, counted in the memory
  ledger). They are never written to disk: the owner's D1/D2 — no intermediate results stored. A test proves the bytes
  are equal with and without them.
- **Cost bound.** Work = elements × steps since the nearest checkpoint below t.
  - A cold seek deep into a long simulation computes from the origin. It runs on the `sim` thread (D10.3), never on the UI thread.
  - Until it reaches t, the frame reports incomplete ("simulation: N s behind"), as a stream stage does, and the
    preview shows Loading (the owner's choice for stream stages, found-defects 403).
  - An export integrates forward from the origin, as it pre-rolls a stream stage.
  - A set declares a ceiling on elements (particles: 20 000 by default); past it, spawning stops with a notice and no
    silent cap.

## D5. In the plan — numbers, keyed

Where the poses are computed depends on who draws the elements today; the function (`kt-sim`) is one either way.

- **Plan-side sets** (mask vertices, reel layers, 3D planes, transition pieces). These elements are numbers the plan
  already evaluates at the frame's material tick: `LayerSrc.masks`, a sub-plan's transforms, the trans panels.
  `layer_src` asks `kt-sim` for the poses and writes the simulated numbers in place of the evaluated ones. They are
  hashed by `(SimKey(t), t)` (D12.2), so that a probe plan, which never steps, has the same key.
- **Atlas-drawn sets** (a title's glyphs, an emitter's particles). The picture of each element is made ONCE by the
  generator source on the media worker as an atlas (a title: one sprite per cluster, keyed by the text's values as
  today; an emitter: its sprite), and the PLAN places the elements: one pose per element, drawn by the instanced-sprite
  pass (D6). Two instances of one reel read one atlas and give two plans (D12, spec-engine). A title without a set
  keeps today's path, key and bytes.
- No computation state (a checkpoint, where playback began) ever enters a key (invariant 4); `SimKey(t)` is made of the
  edit only. The frames on disk are final pictures (the owner's
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
- A simulated owner pays its own steps and slows no other layer: the steps run on the `sim` thread per owner, and the
  frame of a moment where it is not visible does not step it.

## D8. The first applications (stage A)

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
4. **Keys.** `plan_key` (kt-gpu `engine.rs:4336`) and `durable_hash`'s `put_plan` (`render_cache.rs:715`) hash
   `(SimKey(t), t)` in place of the simulated numbers (`MaskOp` in `LayerSrc.masks`, the transforms, the per-element
   poses); t2981 guards the pair. The cost of it: two moments with equal poses have two keys, so a set at rest is
   composed per moment rather than once; the owner's frames are still reused at the SAME moment (loop, scrub back). The rule "a new field `plan_hash` hashes is
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
   `Slot`, timed, cut by the arbiter, warmed by `prewarm::effect_ids`. `plan_key` (`kt-gpu/src/engine.rs:4336`) and
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

**After `durable-moment-key`** (2026-11-24; it is built first): `plan_key` and `put_plan` are one function,
`kt_engine::moment_key`, so the op and the numbers are hashed in one place; t2981 no longer guards a pair but the one key.
The atlas enters by `LayerSrc.leaf` (the identity the cache holds), not by a content serial, and there is no `note_leaf`.

## D13. The model (kt-model `elements.rs`)

```
ClipEvent.sets: Vec<ElementSet>                  // empty = not written
ClipEvent.sim_span: Option<SimSpan>              // None = not written; D14
ElementSet { id: u32 (unique on the event, never reused), source: ElementSource, ops: Vec<SetOp>, ceiling: u32 }
ElementSource = TitleText { by: Cluster | Word | Line }       // the event's generator is a title
              | MaskVertices { slot: Option<usize>, mask: MaskId }
              | Emitter                                       // the event's generator is `emitter`
SetOp  = { on: bool, solver: PluginSlot /* family Sim */, auto: Vec<ParamCurve>, drivers: Vec<Driver>, hits: Vec<SetRef> }
Driver = { what: Outer(Rate | Accel | Change) | Sound { track: TrackId, band: Level | Low | Mid | High }
               | Elements { of: SetRef, pull: f64 }, to: String /* a row of the solver */, amount: f64 }
SetRef = { clip: Option<ClipId> /* None = this event */, set: u32 }
```

- The home's motion is not a driver: every solver reads the homes themselves (D15), so a crawl, a keyed mask and a
  tracked vertex all pull without a setting. `Outer(Accel)` is for elements that stand still in their own composition
  while the composition is braked from outside.
- Solver rows take curves in the event's material domain, like `fx_auto`; `clip_slice`/`shift_curves` move them
  (`project.rs:2623`, `:2645` gains the sets' curves beside the masks').
- **Address:** `ParamOwner::Set { set, op }`, text `set<id>.op<k>` in `LinkPath`; the rows are the solver's passport. So
  a stiffness is linked, published and driven like any other row.
- **Commands:** `SetSets { id, sets }` (as `SetMasks`), validated: source fits the event, ids unique, the solver's rows,
  `SetRef` targets in the same reel, no cycle (the error names the two sets by the events' names).
- **Copy, paste, duplicate:** sets travel with the event; a `SetRef` to an event inside the pasted block is remapped as
  a link's address is, one to an event outside it is kept in the same reel and dropped in another. Deleting the target
  leaves the row: it reads nothing and its row says "missing" (as a link's). A changed generator kind or a deleted mask
  removes the sets whose source is gone, in the same undo step.
- **A missing solver** (a plugin this build lacks) is kept and written back (`MissingSlot`); its op places homes.
- **Family:** `Family::Sim`, `Media::Sim`, `Plugin::sim() -> Option<&dyn SimKernel>` beside `trans()`
  (`kt-plugin-api/src/lib.rs:842`, `:854`, `:1466`); its WGSL-less manifest takes part in the durable key by
  `(id, version)`.
- **Format:** `PROJECT_FILE_VERSION_ELEMENT_SETS`, the next number at the merge, by `project.has_element_sets()`
  (`project_file.rs:104`).

## D14. The origin looks through cuts

A cut, a trim and an instance's window must not change the picture: an edit that only says WHERE a thing is shown
cannot restart the letters. (`clip_slice` keeps that promise for curves today: "the whole curve would move off".)

- `SimSpan { lead: Tick, stop: Option<Tick> }`. The origin of a set is `event.start − lead` in the timeline of the
  event's reel. `Split`, `FrameHold` and `TrimStart` carry it: the right piece's lead is the left's plus the cut's
  distance; a left trim by δ adds δ, an extension takes δ away and stops at zero (then the origin is the new start). It
  is written only when the event has sets or is an instance of a reel that has.
- `FrameHold`'s piece gets `stop` = the moment of the hold: a held picture holds its letters too, where a rate of zero
  would let them swing on. `rest` (the piece after) takes the lead of the moment it continues from.
- Homes and drivers before the piece are evaluated by the same functions: a cut keeps every key of every curve (nothing
  is pruned; `shift_curves` only moves them), so the rate profile, the mask and the generator rows before the cut are
  the left piece's. A rate curve made of a FORMULA of `t` restarts at a cut today (its `t` is seconds from the material
  entry, `warp.rs:932`), and so does its simulation.
- **Through instances.** Going back from t, the owner's origin is the earliest outer moment from which the owner has been
  continuously played: the outer map run backwards through every nesting map until the owner's own origin. A reel
  shown from its 5th second therefore opens with letters that already fell. Where the map never gets there (a frozen or
  reversed instance), the origin is the instance's `start − lead`.
- A moment before the origin (a transition's head) has the state of rest.

## D15. Homes and the frame of reference

- **Never the preview's size.** A simulation runs in the owner's reference frame: the title's layout at the PROJECT's
  size, a mask's normalized coordinates times its `ref_px`, an emitter's canvas pixels. The result per element is a
  DELTA (offset in fractions of the reference height, turn, scale, opacity) that the drawing applies to the element's
  own rest place at whatever quality. 360p, 1080p and the export move alike (t3991).
- **Homes are exact.** The layout rounds to 1/64 px (`kt-text`); read at 240 Hz that rounding is a false acceleration.
  A home is the layout at offset 0 (memoized by the values that shape it) plus the offset row as f64, at the
  CONTINUOUS material position of the step, not the frame's snapped pts.
- Solvers read homes and their velocity by difference of two steps; they integrate absolute positions
  (`m·p'' = −k(p − h) − c(p' − h')`), so a kink of the home is a kink of a velocity, never an infinite force.
- **Words and lines** move as one body each: their clusters share its pose around the body's centre.
- A running title (a counter) keeps its elements by place: the third digit keeps its swing as it changes.

## D16. Each driver's share of `SimKey(t)`

`SimKey(t)` hashes, per set, in this order: the source's static content (layout values without offset, mask id and shape
kind, emitter rows' tags), the ops (solver id, version, rows, curves restricted to [origin, t]), the origin as an offset
from the event's start, the instance path, and:

| Driver | Hashed | Not ready |
|---|---|---|
| homes: curves (offset, vertices, geometry) | keys up to t and the key after it; a formula's text | — |
| homes: a link | the source property's own prefix, recursively (the cycle check bounds it) | — |
| homes: a tracker | the samples up to the material frame after t | — (a gap holds, as the tracker says) |
| outer maps, morpher warps | each map's prefix by its own time; a layer that starts after t is not hashed | — |
| sound | `(series.hash(), offset)` of each feed piece that starts before t (`GenFeed`, `genmedia.rs:952`) | `Behind { why: Sound }` |
| another set | its `SimKey(t)` | its `Behind` |

- A hash of the curves costs O(keys) per frame; a per-set memo keyed by (curve, segment index) makes it O(log keys).
- **Sound that is not analysed yet** makes the answer `Behind`, never silence: silence would be kept as a true frame.
  The Loading line says "listening to <track>". An export waits for the series before its first frame.
- A probe whose sound is not analysed hashes a mark in place of the series: it matches nothing kept (an incomplete
  frame is kept nowhere), and the key moves when the series arrives.
- Sound is heard as the reactive generators hear it: the files of the track, before effects (t3715); the mix after
  effects joins when theirs does.

## D17. The title's layers (what the atlas holds)

`kt_text::render_rgba` makes ONE silhouette of all glyphs (overlaps by maximum), then plate, shadow, stroke and fill,
each over the whole text in the title's own order. One coloured sprite per cluster would lay the second letter's shadow
over the first letter's fill. So:

- **Atlas:** per cluster two masks in one RGBA8 cell: R the fill's coverage, G the stroke's alpha, each rasterized on
  the same sub-pixel place as the whole title's; equal cells are shared, cells are shelf-packed, the atlas's size a
  function of the layout. No colour, no reveal weight, no plate, **no shadow**.
- **Address:** the generator's address salted with `ATLAS` alone (`gen_source_addr_in`, `video.rs:2935`). Not with the
  quality: `gen_sources(project)` raises the sources and knows no quality, and the decode quality (R.46's step in it)
  is in the burst key already. `GenState` gains `atlas: bool`, so the worker, the source's identity
  (`durable-moment-key`) and `can_retune_gen` see it. Index — the source pts, as today: a title whose size is animated
  has an atlas per frame.
- **The atlas is the one the poses are for.** The cell table is a function of the values, computed by the plan; the
  frame comes from a worker that may not have taken the retune yet. A changed text has other clusters, and element i
  would be drawn from the old text's cell i. So the op takes the atlas only when the cache's identity of the source
  is the identity of the renderer's values; otherwise the layer is starved, as with any absent frame.
- **Too large an atlas** (past the device's texture limit): the title is drawn at rest, whole, and its set's row and a
  notice say so.
- **Pass A, instanced:** each cluster's quad with its pose into ONE `Rg8Unorm` target with MAX blending (R fill, G
  stroke). The element's opacity and the reveal weight scale the masks. Not the pool's `Rgba8Uint` (`pool.rs:12`):
  integer targets do not blend. One target and one blend state: different blend states per target need
  `INDEPENDENT_BLEND`, which GL devices below ES 3.2 lack.
- **Pass S, the shadow:** the placed fill, moved by the shadow's offset (whole pixels, as `shadow_alpha`) and blurred by
  the same three box passes over integers; skipped when the shadow is transparent.
- **Pass B, resolve:** the plate from its rectangles (the plate does not move with letters), then the four layers `over`
  in the title's order with its colours, in `render_rgba`'s integer arithmetic. All are compiled with the project.
- **Why the shadow is blurred after the letters are placed** (spec-engine, 2026-11-24; a mask per cluster was designed
  first):
  - a letter that turns would turn its shadow's offset with it — a swinging letter's shadow would circle it, while the
    light does not move;
  - the blur rounds to whole levels six times: a wide blur leaves less than half a level of one letter's tail, which
    rounds to 0, and the sum of ten such tails is lost. The sum of rounded shadows is not the rounded shadow of the sum;
  - overlaps are the maximum, as in the raster, not a sum.
- **Why this equals the raster at rest (t4107):** the stroke is a distance to the silhouette, so a union's stroke is the
  maximum of the parts' strokes; the fill is a maximum already; the shadow is the same function of the same fill. Held
  to the `FrameTol` of the stored title frames, on a static title with all four layers and a wide blur, letters
  overlapping. A soft reveal is the exception: the raster weighs the coverage BEFORE the stroke's threshold (α ≥ 128),
  the op weighs the finished stroke; a set's title fades its stroke in with the letter.
- **Particles** use pass A's instancing with a colour sprite straight to the layer, over or additive (`emitter` row
  `blend`); no resolve.

## D18. Memory, threads, stages

- **Checkpoints:** one state per second inside the view window (the playhead − 2 s … + 10 s, the loop region), and one
  per 1, 2, 4, 8… seconds further back, so a long simulation keeps O(log) states. Ledger tenant `Sim` (base 32 MB,
  growing last, after `Frames`); evicted by the farthest from the playhead. A cleared memo changes no byte (t3968).
- **Cost, for the review of the build:** a spring step is about 20 ns an element. A 60-letter title costs 0.3 ms of
  processor per second of simulated time, 20 000 particles about 0.1 s per second (a tenth of one core while playing,
  nothing on a pause).
  - **An emitter's cold seek does not start at the origin.** 20 000 particles over 4:50 would be 29 s of Loading. A
    particle's state depends on its own spawn alone, so the state at t needs the steps of one lifetime:
    `SimDecl::forgets(rows) -> Option<Tick>` (particles: the longest lifetime over the prefix; springs: `None`, they
    never forget exactly). The catch-up starts at the later of the checkpoint and `t − forgets`. Equal bits need two rules:
    steps lie on the grid counted from the ORIGIN, not from where the walk began; spawn k (its moment, its seed) is a
    closed form of the rate's curve. An emitter that reads another set (attraction, colliders) needs that set's states
    over the window: they come from its own catch-up, from its origin where it never forgets, and the emitter's `Behind`
    is the later of the two.
  - **The ceiling** is counted from that closed form too: spawn k is skipped when the spawns SCHEDULED in the longest lifetime
    before it (skipped or not) already reach the ceiling. Counting the living would make each spawn depend on all
    before it, and the window would not give the same state.
- **Stage A** (this change): the system, letters, mask vertices, the emitter.
- **Stage B** (one change per row of D9 once A is in, in this order, each needing only a producer, a drawer that exists
  and a section in a window): reel layers (9), transition pieces (11), the camera and Geometry (12), the reactive bars
  (14), lens drops (8, from the emitter), a link reading a pose, word effects (2) and subtitle words (3) with their
  changes, the tracker, the magic wand and 3D planes (5, 6, 13) after the changes that build their producers.

## Rejected

- **A Physics animator on a title only (revision 1)** — rejected by the owner (D3): one control in one form, the system
  not built.
- **Simulation as a stateful playback-time process** — depends on where playback started (F-FX-005, R.59).
- **Baking to keys** (Newton) — every edit of the text or the speed would need a rebake.
- **Simulating pictures (per-pixel fluid, smoke) in this system** — those are effects with stream history (the stream
  stage); this system moves the parts of a picture that already have coordinates.
- **Checkpoints on disk** — the owner's D1/D2; memory and recomputation are enough at these costs.
- **Driving by the playhead** — the B7 lesson: the edit's speed is the driver.

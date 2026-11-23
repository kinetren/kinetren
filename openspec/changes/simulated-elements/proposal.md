# Proposal: simulated-elements

> DESIGN ONLY (spec session), revision 2 (2026-11-23) after the owner's answer to Q56 (`kkt-26.11.22` D3): not a
> letter-physics animator but a COMPOSITING SYSTEM in which the parts of a picture are objects with simulated state,
> and a walk of the whole program for where it applies. Revision 1 was `text-letter-physics` (a title's Physics
> animator). Registry `v26.10-index.md` F2; notes F-FX-005 (physics), F-FX-006 (particles). Tests reserved:
> t3968–t3973, t3983–t3990.

## Why

The owner's items, whole:

- `docs/ktprompts/v26.10.txt` lines 109–111 (the FX section, item 2; line 109 holds item 1 and the start of item 2):
  > 2. Генератор текста скудный, управляется не удобно, нихера не удобно. Также генератор слишком топорный, например,
  > можно было бы разрешить ввести буквы как физические объекты,
  > и когда, например, есть та же бегущая строка - если управлять например rate таким событием, например, через
  > slowed&reverb - то можно было бы добавить инерцию буквам, они ведь могут туда-сюда относительно себя
  > двигаться/падать/таскаться/висеть и т.д.
- `kkt-26.11.22.txt` line 6 (D3), the answer to Q56:
  > нет, речь идет не про то как в очередной раз высрать в форму какой-то контролс и соплями смазать код. а про то - как
  > построить ТАКУЮ систему композитинга, которая позволит В ТОМ ЧИСЛЕ сделать такого рода эффект. этот эффект это просто
  > пример применения, а так нужно еще посмотреть по всей программе где такую систему можно применять и добавить еще мест
  > таких.
- `docs/user-feedback.md`, round R.59 (2026-09-20), items 3–4: physics for the letters of a title "a really very good
  system must be thought out, so that one can control everything, including from plugins"; "if physics was added to
  only one source - that is no reason to break and weigh down the work of the whole project". His decision then:
  simulation from the start of the event with a fixed step, the state at a frame a pure function, physics gives
  NUMBERS, not pixels.

**Reading, checked.** Falling letters are the EXAMPLE. The thing asked for is how the composite is built: today a
frame is a pure function of its description, and every part of a picture (a glyph, a mask vertex, a tracked point, a
layer of a composition) is placed by closed-form numbers — keys, curves, links. The system adds one kind of thing to
the composite: a SET OF ELEMENTS whose placement numbers come from a simulation with state. The simulation is driven by
what the edit already has (time maps and their speed, links, sound, trackers, other sets). It stays a function of the
edit, so preview, scrub and render agree. The driver of the example is the edit's speed (an event's rate through
Slowed&Reverb, an instance's speed), not the playhead — the B7 lesson.

## What Changes

- **Element sets.** Any producer of parts of a picture exposes them as a set of elements with a stable identity and a
  rest pose:
  - a title's clusters, words or lines;
  - a mask's vertices or primitives;
  - a tracker's points;
  - a composition's child layers;
  - a 3D group's planes;
  - a transition's pieces;
  - an emitter's particles.
  Elements are addressed like any property (R.61 addresses), so links and plugins can read and drive them.
- **Operators on a set.** An ordered list of operators turns rest poses into placed poses:
  - stateless operators are closed form: today's per-letter animators, keys, links;
  - stateful operators are SIMULATIONS: springs, pendulum, rope, fall and collide, particles, flocking.
  The solvers are a new plugin family (`sim`) with a passport, a version and a kernel, so a plugin can add one
  (wasm later).
- **Drivers.** A simulation's forces come only from what the edit determines:
  - the outer time map's channels (speed, acceleration, speed change; `motion-driven-params`);
  - any property by link;
  - the sound of a track (the audio-reactive analysis);
  - a tracker's motion;
  - other sets' elements, for collision and attraction (an ordered graph, cycles refused).
- **A function of the edit.** The state at any moment is integrated from the owner's start with a fixed absolute step
  in the owner's outer time, per instance path. It uses the frozen math, so the bytes are the same on every machine.
  Checkpoints are kept in memory only: they never change a byte and are never written to disk (the owner's D1/D2:
  nothing intermediate is stored).
- **In the plan, as numbers.** Simulation runs while the frame plan is built (CPU, before the GPU), and the plan
  carries each element's placed pose. The plan key hashes those NUMBERS, not the history, so the memo, the disk
  frames and the coverage bars stay true (invariant 4). Masks, reel layers and planes are drawn by
  their own code (the mask raster, the Geometry node); letters and particles by one instanced-sprite pass from an
  atlas their source draws once (invariant 7: one composition path).
- **Zero cost where unused.** An owner with no stateful operator runs no simulation code and keeps its plan key as
  today; one simulated title does not slow the rest of the project (R.59 item 4).
- **The first applications** (Q1): letters on springs driven by the edit's speed (the example), mask vertices that
  follow a tracker with inertia, and an emitter of particles. The class walk in `design.md` D9 lists every other place.

## Capabilities

### New Capabilities

- `simulation`: element sets, operators, the solver family, drivers, the fixed-step function, checkpoints, the
  dependency order, the zero-cost rule.

### Modified Capabilities

- `video-plan`: the plan carries simulated poses as numbers in the key; a frame whose simulation is still catching up
  reports itself incomplete, as a stream stage does.
- `project-format`: element operators and simulation rows by content.
- `ui-reach`: adding a simulation to a set from the owner's window; the first application on a title.

## Impact

- New crate `kt-sim` (depends on kt-time and kt-plugin-api only): `ElementSet`, `Pose`, the integrator, checkpoints,
  the dependency order.
- kt-plugin-api: the `sim` family (`SimKernel`), its registry; the frozen math.
- New plugin crate `plugins/sim-basic`: spring, pendulum, rope, fall-and-collide, particles.
- kt-model:
  - `ElementSource` addresses (title clusters/words/lines, mask vertices, tracker points, composition layers, 3D
    planes, transition pieces, emitters);
  - `SimRule` rows on an owner, with drivers as link sources.
- kt-text: the cluster boxes as rest poses (exists: `LayoutInfo::clusters`); a glyph atlas of a title (one sprite per
  cluster) drawn by the title's source.
- kt-engine (design D10, D12): every set is simulated while the plan is built, the outer clock and instance path
  carried down the nested build; a `sim` thread advances the memos; `Behind` makes the frame incomplete (Loading); the probe plan keys without stepping;
  an export pre-rolls.
- kt-gpu: one instanced-sprite pass for letters and particles (an atlas, poses from a buffer).
- kt-app: the "Simulation" section in an owner's window (title form, mask panel, composition window, emitter); a faint
  overlay of the set's rest poses and links while editing.
- Format: by content. OTIO: the planned row (renamed from `text-letter-physics`).
- Idle: nothing at rest. A paused frame computes its numbers once, and a plan memo hit reuses them.

## Questions to the owner

**Q1. Which applications come with the system first?** Your words (D3): "этот эффект это просто пример применения, а
так нужно еще посмотреть по всей программе где такую систему можно применять и добавить еще мест таких". Today: no part
of the program simulates; the walk in `design.md` D9 finds 14 places. The system is built once, and each application
then costs a producer of elements (most already exist) plus a form section.
- (a) **The system plus three applications that prove it on different kinds of element:** letters (title clusters)
  driven by the edit's speed, mask vertices following a tracker with inertia, and particles from an emitter. The other
  places follow one per change (*recommended*: three different producers and drawers prove the contract is general;
  the system is not shaped around letters).
- (b) The system plus letters only, the rest later (a faster first build; the contract is proven on one producer).
- (c) The system plus every place of D9 at once (one large change; long before anything is usable).

**Q2. The simulation's step.** Your decision of R.59: "a simulation from the start of the event with a fixed step". Its
value was left open (F-FX-005): a step tied to the project's frame rate makes the motion depend on that setting
(change 25 to 30 fps and the letters move differently).
- (a) **A fixed step of 1/240 s of the owner's outer time, the same at any frame rate; a frame reads the state at its
  moment, interpolated between two steps** (*recommended*: the same edit moves the same way in any project and any
  export rate; 240 is a multiple of 24, 30, 48, 60 and 120).
- (b) One step per project frame (simpler, but the motion changes with the frame rate).

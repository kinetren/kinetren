# Proposal: text-letter-physics

> DESIGN ONLY (spec session, 2026-11-22). Registry `v26.10-index.md` F2 (no). Note F-FX-005 (refreshed). Tests reserved:
> t3968–t3973.

## Why

The owner, `docs/ktprompts/v26.10.txt` lines 109–111, whole (the FX section, item 2):

> 2. Генератор текста скудный, управляется не удобно, нихера не удобно. Также генератор слишком топорный, например, можно
> было бы разрешить ввести буквы как физические объекты,
> и когда, например, есть та же бегущая строка - если управлять например rate таким событием, например, через
> slowed&reverb - то можно было бы добавить инерцию буквам, они ведь могут туда-сюда относительно себя
> двигаться/падать/таскаться/висеть и т.д.

**How this design reads the item** (checked against the B7 lesson, where "перемотка" meant the edit's speed): the
driver is the EVENT'S RATE as the edit sets it — a running line whose speed is changed by Slowed&Reverb's rate curve —
not the playhead. When that speed changes, each letter, being a body with inertia, moves "relative to itself" — lags,
overshoots, swings, drags, hangs, or falls — instead of moving rigidly with the line. The same item's first half (the
text generator is poor and awkward) is the generator's form, covered by `title-word-effects` and the title changes; this
design takes the physics.

Today a title's letters move only by the four animation numbers (reveal, offset) and per-letter animators (F-TXT-006);
nothing in them reacts to a change of speed. F-FX-005 (R.59) decided the shape: simulation is a FUNCTION of the edit —
the state at a frame is computed from the event's start with a fixed step, checkpoints only as an optimization; physics
feeds the title NUMBERS, the title still draws.

## What Changes

- **A "Physics" animator on a title**: each letter is a body tied by a spring to its place in the laid-out line (its
  "home"). The home moves with the line (the crawl, the offset, the event's warp); a change of the line's speed is an
  acceleration that the springs pass to the letters with inertia — they lag when the line speeds up, run ahead when it
  brakes, and settle back. Parameters, each a passport row with curves: stiffness, damping, mass spread (letters differ
  a little, so they do not move as one), and the behaviour (Q1): spring back (sway), hang (a pendulum from the letter's
  top), drag (a rope behind the previous letter), or fall (a letter tears off when the jolt passes a threshold and falls
  under gravity onto the frame's bottom).
- **Driven by the edit's speed**: the acceleration comes from the outer time map of the title (its own warp, a morpher
  layer's warp, the enclosing instances' speeds — the channels of `motion-driven-params`), so a running line slowed by
  Slowed&Reverb, or a composition braked from outside, shakes its letters; preview and render agree.
- **A function, not a history**: the state at any frame is integrated from the event's start with a fixed absolute step
  (Q2), the project's frozen math, the same bytes everywhere; checkpoints every second of material, kept in memory and in
  the persistent cache by content (the title's values, the drive's profile), make a seek cost at most one second of
  steps; their presence never changes a byte.
- **Numbers to the layout**: the simulation gives each glyph an offset and a rotation; the title draws them with its own
  code (invariant 7).

## Capabilities

### Modified Capabilities

- `generators`: the Physics animator of a title — bodies, springs, behaviours, the drive from the outer speed, the
  fixed-step function and its checkpoints.
- `ui-reach`: choosing the animator and its behaviour in the title form; seeing the springs while editing.
- `project-format`: the animator's rows by content.

## Impact

- kt-text / kt-model: `TextAnim::Physics { stiffness, damping, spread, behaviour, threshold, gravity }` as passport rows of
  the title (curves allowed); per-glyph homes from the layout the title already computes (F-TXT-006's boxes).
- kt-engine / kt-media (the generator's source): the drive — the acceleration of the line's home along the material,
  from the title's offset curves and the outer-speed channels (`motion-driven-params`), handed to the generator as
  numbers with the frame request; the integrator with checkpoints.
- kt-app: the animator's choice and rows in the title form; a faint spring line per letter while the form is open.
- Format: the next free `.kt` version by content. OTIO: a planned row (a foreign editor shows the letters still).
- Idle: nothing at rest; a frame costs the steps since the nearest checkpoint.

## Questions to the owner

**Q1. What do the letters do by default?** Your words: "они ведь могут туда-сюда относительно себя
двигаться/падать/таскаться/висеть и т.д." — four behaviours. One is the default of a new Physics animator; the others are a
choice in the form.
- (a) **Sway: each letter springs back to its place with inertia — lags when the line speeds up, overshoots when it
  brakes; "fall", "hang" and "drag" are the other choices** (*recommended*: the line stays readable; the stronger
  behaviours are one click away).
- (b) Fall: letters tear off at a jolt and fall to the bottom of the frame.

**Q2. The simulation's step.** The physics note (F-FX-005, R.59) left one question open: a step tied to the project's frame
rate makes the motion depend on that setting (change 25 to 30 fps — the letters move differently); a fixed step in seconds
does not, at the cost of computing between frames.
- (a) **A fixed step of 1/240 s of material time, the same at any frame rate; the frame reads the state at its moment**
  (*recommended*: the same edit moves the same way in any project setting and export rate).
- (b) One step per project frame (simpler, but the motion changes with the frame rate).

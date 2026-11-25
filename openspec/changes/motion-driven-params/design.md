# Design: motion-driven-params

> IMPLEMENTATION-READY (spec-engine, revision 3, 2026-11-25, orch `fc60a5d9`). Revision 2 took the owner's B7 (the EDIT's speed); its D3 ("the channels are resolved in
> `morph_pass` into keys") does not hold and is replaced: see "Why not keys". Code is named by function and file.

## Context (what exists)

- **A curve's domain is the event's material ruler**: `clip_material_at`, LINEAR in the timeline even under a warp (`layer_src`). Curves are compiled once per build
  (`AutoPlan`, kt-model `autoplan.rs`) and applied per frame; links and formulas become keys once per build (`links::resolve_derived`, "the only door").
- **Speed** is an event's `ClipWarp` (`warp.rs`): a rate profile over the same linear ruler, its integral the position map. A composition's instance is an event whose source is a reel: its warp plays the reel
  (`clip_inner_tick`, continuous, and not one-to-one: a rate of 0 holds the reel's moment, a negative one goes back). A morpher layer's warp is already the covered event's warp (`clip_warp_of` over the patched
  chain).
- **A reel's renderer is shared** by every instance with the same overrides (`NestKey`, t442/t443); a sub-plan is built at the instance's inner tick with the instance's layer above it (`layer_src` → `PlanCtx`).
- **The sub-mix** of an instance is a stream of its own, rendered only FORWARD in the reel's time (law C4); the instance's speed acts on its samples.

## Why not keys

The owner's case is a braking: the composition slows to a stop and the fisheye dies away AFTER it stopped. While the instance stands, the reel's moment stands, and so does the material position of everything inside: a
value that is a function of the inner time is frozen at its peak through the whole stop, and jumps when the motion resumes. Two moments of the outer timeline with one inner moment need two values. So the picture reads
the channels at the OUTER moment, while the plan is built (D3). The sound has no such case: its sub-mix is produced once per inner moment, forward, so there keys are exact (D4).

## D1. The source (kt-model)

```rust
CurveSource::Outer { channel: OuterChannel, full: f64, attack: f64, release: f64, expr: Option<String> }
enum OuterChannel { Rate, Accel, Change }
```
- `expr` is the layer formula of every derived curve (`t`, `src`, `k`; R.67), `src` the channel; without it the value is `src`. The keys under it live (`k`).
- Defaults: `full` 1 (a change of 1× a second is a whole 1), `attack` 0.1 s (0..1), `release` 0.5 s (0.05..4); used by `Change` only.
- **Where**: numeric rows of an effect slot (an event's chain, a morpher patch's two chains) and of a mask. Refused with a reason (`LinkError::Outer(why)`): the warp's own `rate` (the value would be its own cause), a generator's rows
  (their picture is drawn by a source on its worker and addressed by the event: two outer moments would share one frame; moving letters is `simulated-elements`), a track's chain (no curves there), a published override's curve, a choice or text row.
- **A link to a row led by an outer channel** is refused ("read the speed itself here"): the leader has no keys to read.
- `needs_resolve()` is false for it: `resolve_derived` leaves it. `param_idx`, shifts and slices treat it as a curve with its keys (`clip_slice`, `shift_curves`).
- `AutoPlan::compile` takes it: `PlannedCurve { keys, outer: Option<OuterCurve { channel, full, attack, release, program }> }`; `apply`, `apply_slot` take `&dyn OuterNow` (`OuterNow::NONE`: rate 1, the rest 0: what an event no map plays reads).
  A plan without one costs what it costs today (the branch is one `Option`).

## D2. The path and the channels (kt-model `outer.rs`)

```rust
pub trait PlayMap { fn active(&self, t: Tick) -> bool; fn rate(&self, t: Tick) -> f64; fn inner(&self, t: Tick) -> Tick; }
pub struct OuterPath<'a> { pub up: Option<&'a OuterPath<'a>>, pub map: &'a dyn PlayMap, pub t: Tick }   // root's map first reached through `up`
```
- `VideoClip` and `AudioClip` are `PlayMap`s: `rate` = `ClipWarp::rate_at(tl/dur)` (the very effective rate `Integral` integrates; a new accessor), −1× under reverse, 1 without a warp; `inner` = the function the plan uses, without a reel's own-fps quantization.
- `rate(s)` at a moment `s` of the OUTERMOST timeline: the product over the path, each map asked at the time the maps above it give for `s`: forward, so nested maps, stops and returns need no inverse.
- `accel(s) = (rate(s) − rate(s − h)) / h`, h = 375 ticks (1/240 s, divides every frame).
- `change(t) = min(1, max over j·h ≤ attack + release of |accel(t − j·h)| / full · g(j·h))`, `g(u) = u/attack` up to the attack, then `1 − (u − attack)/release`: each acceleration rises over the attack and falls over the release; their maximum. No state: a seek, a scrub,
  a loop and an export give one value.
- Where a map of the path does not play at `s` (before the instance's start), the rate is the one at the first moment it plays: entering is not an acceleration.
- Only `+ − × ÷` and `max`: no libm (invariant 11).
- Cost: (attack + release)·240 samples × the depth, 144 at the defaults, at most 1200; memoized per (owner's path, parameters) in the plan's context: one evaluation a frame for an event however many rows follow it.

## D3. The picture: evaluated with the plan

- `PlanCtx` gains `outer: Option<&OuterPath>` (`None` at the root, inherited by a track stage's sub-context, set to the instance's path for a reel's sub-plan). `layer_src` builds the event's own path (its own map at `t` over `ctx.outer`) and hands it to
  `AutoPlan::apply`, to `ClipMaskPlan::ops` and to the reads' lines.
- `ChainMemo` is keyed by the material tick; a plan with outer curves keys it by (tick, digest of the channels used).
- **The key**: `LayerSrc.outer: Option<u64>`, the digest of the channel values the layer used; `put_src` hashes it when it is there (a layer without one keeps its key). The sub-plan is hashed whole, so a kept composite of one speed is never shown at another.
  `picture_key` (cached-final-render) hashes the resolved values and needs nothing.
- **The sub-renderers stay shared**: two instances of one reel are two sub-plans of one renderer (t3950). `NestKey` does not move.
- `render_cache::put_rows` sees the resolved rows already.
- **A stream stage** (Datamosh) whose rows or front slots follow a channel reads a HISTORY: each position is an inner moment, and the kernel's schedule is one per build. There the channel is asked at the FIRST outer moment that plays the position's inner moment
  (`OuterPath::first(inner)`, the position map's first crossing by halving over its monotone pieces), the current one included; the path's digest (each map's warp and placement) joins `plan_cell`'s key and `ident` (through `front_key`) when such a curve exists, so two
  instances have two histories. Said in the row's hint: "a stop does not change it".
- **The decode step** (`clip_head_zoom`, `nest_scales`): probed at 17 values across each channel's range over every path that plays the reel (the tree `nest_scales` walks, at frames), computed once per build and only when a reel reads a channel.
- `kt-cli probe` prints the channel's value beside an effect that reads it.

## D4. The sound: keys per instance

- `AudioRenderer::build_reel` gets the path of its instance. Where its content (or a deeper reel's) reads a channel, the instance's `AudioClip.nest` key mixes the path's digest, so it gets a sub-renderer of its own; a reel that reads none shares as today.
- Each outer curve of an event is sampled on the frame grid over its material window, at the first outer moment that plays each inner moment, into ordinary keys, before `AutoProfiles`. The sample path is the same function as D2.
- The path's digest joins the event's derive key (`incremental`) and `fp`: an edit of the instance's speed changes the scene of exactly the bursts of that instance (the kept mix).
- The sub-mix of an instance that stops goes silent with it (its samples are not consumed): nothing to decay.

## D5. The gesture (kt-app)

The curve's "Driven by…" menu (`automation.rs`) gains "The speed it is played at" → "Speed change", "Speed", "Acceleration". Choosing writes `CurveSource::Outer` with `expr`
`k + src * (MAX − k)` where the curve has keys, else `VALUE + src * (MAX − VALUE)`, MAX the row's maximum, for "Speed change"; `VALUE * src` for "Speed"; none for "Acceleration". One undo step. The legend reads "strength ← speed change"; `full`, `attack`, `release` stand beside the formula field. "Unlink" turns it into
keys at the values of the place it is shown in (the reel opened by itself: rate 1), and says so. A row that cannot (D1) shows the menu entry with its reason.

Inside an opened composition the preview shows it played at 1×: the form says "played at its own speed here; see it where it is placed".

## D6. Format

`PROJECT_FILE_VERSION_OUTER_SPEED`, the next free number at the merge, by content. OTIO: the curve is written into `metadata["kt"]`; another editor reads the static value (a row in "What OTIO export loses").

## D7. Class walk

| Place | Here |
|---|---|
| event's effect rows (picture) | D3 |
| event's effect rows (sound) | D4; at the root the path is the event's own warp, exact |
| morpher patch, both chains | the covered event's path, its warp the patched chain's |
| masks of an event, of a slot | D3 |
| reads (a time kind in an arm): the arm's own warp | the read's line has its own map: the path of that line |
| a transition's, a camera's curves | not links today (no owner of that kind): not here |
| generator and title rows | refused (D1); `simulated-elements` |
| track chains, track masks | no curves |
| geometry rows | row 838: whether a link resolves there is being looked at; the same answer holds for this source |
| an override's curve, a published parameter whose target is led by a channel | the override replaces the curve, as it replaces keys |
| `resolve_derived`, `resolve_curve_in`, `formula_preview` | leave it; the preview of a formula with `src` shows `src` = 0..1 |
| the named path `apply_fx_auto` (the oracle of `AutoPlan`) | takes the same `OuterNow` |
| value displays (`fx_editor`, pinned rows, the curve panel at the playhead) | the value at the shown moment through `VideoRenderer`'s path of the opened composition |
| copy, paste, presets of a chain | the source travels; it names no event |
| `cut` and `trim` | the path is asked by placement: a cut piece reads the same outer moments |
| previews of effects, filmstrips, posters, reel strips | a strip of a reel is played at 1× (`OuterNow::NONE`) |
| the export, `render_raw_*` | the same plan |
| render-ahead, kept composites, the disk tier | keyed by the layer's digest |
| the interface's speed badges | unchanged |
| `simulated-elements` `Outer(..)` | reads `OuterPath` and hashes each map's prefix (its D16) |

## D8. Tests

t3947 (the closed form of the three channels on one map, steady, braking, a step, a reverse; kt-model), t3949 (nested maps multiply; a stop inside), t3948 (seek, loop and export equal the continuous play byte for byte; preview equals export),
t3950 (two instances, two looks, one sub-renderer; a reel without it unchanged), t4146 (the value dies away while the instance stands; a Datamosh row inside follows the first play and two instances have two histories), t4147 (the sound: two instances, two sub-mixes; an edit of one's
speed drops only its bursts; the same samples in an export), t3951 (the gesture), t3967 (format).

## Rejected

- **Keys for the picture** (revision 2) — "Why not keys".
- **A sub-renderer per instance for the picture** — a reel placed 400 times would be built 400 times; the plan has the path at hand.
- **The playhead's motion as an input** (revision 1) — the owner chose the edit's speed; the render must agree.
- **A link to the outer event's `rate` curve** — crosses the reel boundary (R.61 forbids) and ignores nesting.
- **An exact derivative of the profiles** — nested maps make it a product of functions of functions, and a step has none; a difference over 1/240 s is the same number everywhere and needs no special case.

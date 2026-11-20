# Design: motion-driven-params

> DESIGN ONLY, revision 1 (2026-11-20), base `7c828c2b`. Q1 in `proposal.md`; the default is the recommended option.

## Context

- Links (R.61, capability `automation`): `CurveSource::Link` reads a source property by address, with an expression;
  between events by the timeline tick; resolved in `morph_pass` in topological order, once per build.
- Speed: the warp slot's `rate` curve (signed, −32…32, R.57), sampled into a piecewise-linear profile
  (`rate_profile_keys`, `warp.rs`), memoized by content (compiled-curves).
- A composition instance has its own warp (R.56/R.57: speed on an instance); a morpher patch may carry a warp
  (`MorphPatch::chain_for` yields it).
- The transport: play rate, the space boost (R.57), scrubbing by dragging the playhead; the player's kept composites
  are keyed by the scene (invariant 4).

## D1. Speed sources

A speed owner is one of: an event's warp slot, an instance's warp, a morpher layer's warp. The link address gains
the channel: `<owner>/warp/rate`, `/accel`, `/change`. Reading at a timeline tick t: the owner's profile R(τ) at its
own time τ(t) (the same conversion as every link).

## D2. The channels in closed form

- `rate(t) = R(τ)`.
- `accel(t) = dR/dτ` per second of the owner's time: on a piecewise-linear profile the segment's slope (a step at a
  knot takes the slope of the segment on the right; equal-tick knots — the step is ignored, a speed jump is not an
  acceleration).
- `change(t) = min(1, max over s ∈ [t − release, t] of |accel(s)|/full · w(t − s))`, with `w(u) = 1 − u/release`
  (linear release), and an attack: the raw value ramps over `attack` from the segment's start
  (`min(1, (t − s₀)/attack)` per segment). Evaluated over the profile's segments that intersect the window — a few
  segments, no state, so a seek, a scrub and an export give the same number at t. Defaults: full = 1 (rate units per
  second), attack 0.1 s, release 0.5 s.

## D3. The transport channels (preview only, Q1 a)

`transport.speed`: the player's effective playback speed (rate × boost; while the playhead is dragged, its velocity
in timeline seconds per wall second, smoothed by a 100 ms exponential on the UI clock); `transport.rewinding` =
`min(1, |speed − 1| / 3)`. They are frame inputs handed to the plan with the playhead; offline plans (export,
kt-cli, reel strips, posters, the disk prerender) hand 1 and 0. A plan that reads them marks its frame
`transport_bound`; such a frame is kept only under a key that includes the transport value quantized to 1/8, and the
disk tier never stores it. Paused, the values are 1 and 0 (a paused frame is the edit's frame).

## D4. The gesture

A parameter's link menu: "Follow speed change of…" → pick an event / instance / morpher block on the timeline, or
"the playhead (preview only)" → a link to `change` (or `transport.rewinding`) with the expression `base + v · (max −
base)`, `base` the parameter's value now. Attack, release and full-at show in the link's row and are editable.

## D5. Physics later

F-FX-005 (letters as bodies with inertia) reads `accel` of the same owners as a force; no physics in this change.

## D6. Class walk — "a value derived from motion"

| Input | Deterministic? | Kept frames? | Here |
|---|---|---|---|
| an event's speed | yes | yes | exists |
| acceleration / speed change | yes (closed form) | yes | D2 |
| composition / morpher speed | yes | yes | D1 |
| the playhead's motion | no (the person's hand) | keyed by value, never on disk | D3 |
| audio: the scrub's pitch | preview only already (the scrub sound) | — | the same rule, named |

## D7. Idle

Paused, the transport channels are constant; nothing is computed outside a frame that is drawn anyway.

## Rejected

- **A stateful envelope follower** (a one-pole filter run along time) — depends on where playback started; a seek
  would give another value.
- **Rendering the transport by default** — the render would depend on how the person last played the range.

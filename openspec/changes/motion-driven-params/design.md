# Design: motion-driven-params

> DESIGN ONLY, revision 2 (2026-11-22), base `888e0793`: the owner's B7 — the edit's speed, read from the outer time map.
> Revision 1 (the playhead as an input) is withdrawn.

## Context

- Speed: a warp's signed `rate` curve (R.57) on an event; an instance of a reel has its own speed (the instance plays the
  reel's time by its warp; R.56/R.57); a morpher patch may carry a warp (`MorphPatch::chain_for` yields it); profiles are
  piecewise-linear (`rate_profile_keys`), memoized by content.
- Links (R.61): a property follows another by address with an expression, resolved in `morph_pass` once per build; a
  link does not cross a reel's boundary — the outer speed is not a link to an outside event but a CHANNEL of the time map
  the content is played by (an input, like the material time).
- A reel's sub-plan and sub-mix are keyed by instance (`NestKey`); overrides resolve per instance (`resolve_reel`).

## D1. The outer map

For content at a moment, the outer map is the chain of time maps that play it, inner to outer: the event's own warp
(for the event's effects), the morpher layer's warp (for the events the layer covers), the instance's warp (for
everything inside a reel), and the same again for each enclosing reel. Its rate at timeline tick t:
`R_outer(t) = Π rᵢ(τᵢ(t))` (each map's rate at its own time), its acceleration the derivative by the product rule over
the piecewise-linear profiles (a knot takes the right segment's slope).

## D2. The channels

`rate = R_outer`, `accel = dR_outer/dt` per second of the timeline, `change = min(1, max_{s∈[t−release, t]}
|accel(s)|/full · (1 − (t−s)/release))` with an attack ramp per segment; defaults full = 1, attack 0.1 s, release 0.5 s.
Closed form over the profiles' segments in the window: no state, so seek, scrub, loop and export give the same value.

## D3. Per instance

The channels are resolved in `morph_pass` while the instance's reel is resolved (`resolve_reel`), where the instance's
warp is known: two instances of one reel get different channels. The sub-plan key of an instance includes the content
hash of its outer profile only when the reel's content holds an `outer/*` link (a flag computed per reel per revision),
so reels without such links keep sharing their sub-plans.

## D4. The gesture

A parameter's link menu: "Follow the outer speed change" → a link to `outer/change` with `base + v · (max − base)`;
attack, release and full-at in the link's row. "Follow the outer speed" and "… acceleration" are the other two items.

## D5. Physics later

F-FX-005 reads `outer/accel` as a force on letters; no physics here.

## D6. Class walk — "a value an effect needs from its container"

| Value | Visible inside today? | Here |
|---|---|---|
| the material time | yes (curves' domain) | — |
| the instance's public parameters | yes (R.59 overrides) | — |
| the outer speed | no | D1–D3 |
| the instance's level and reversal | no (they act on the assembled picture) | not asked |
| the morpher layer's strength | no (a mix slot) | not asked |

## D7. Idle

Nothing at rest: the channels are evaluated with the curves at the frames shown.

## Rejected

- **The playhead's motion as an input** (revision 1) — the owner chose the edit's speed; the render must agree.
- **A link to the outer event's `rate` curve** — crosses the reel boundary (R.61 forbids) and ignores nesting.

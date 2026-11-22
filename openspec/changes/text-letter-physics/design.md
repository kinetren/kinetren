# Design: text-letter-physics

> DESIGN ONLY, revision 1 (2026-11-22), base `17c34167`. Q1–Q2 in `proposal.md`; defaults recommended.

## Context

- A title (`GeneratorKind::Text`) lays out shaped glyph runs per frame (kt-text); per-letter animators by characters,
  words or lines (F-TXT-006) already address glyphs and their boxes; the four animation numbers move the block.
- The outer speed (change `motion-driven-params`): the rate, acceleration and speed change of the time map that plays an
  owner — its own warp, a morpher layer's, enclosing instances' — pure functions of the edit.
- F-FX-005 (R.59 decision): simulation is a function of the edit, a fixed step from the event's start, checkpoints only
  as an optimization; physics feeds numbers, the title draws (invariant 7).
- The frozen math (`kt_plugin_api::math`) for bit-equal results across machines (invariant 11).

## D1. Bodies and homes

Each visible glyph g has a home h_g(t) — where the layout puts it at material time t (including the crawl and offset
curves) — and a body state (p_g, v_g, θ_g, ω_g) relative to the home. Mass m_g = 1 + spread · u_g, with u_g a fixed
pseudo-random value of the glyph's index (a hash, no RNG state). A text edit re-homes glyphs by their cluster index;
states start from rest at the event's start, so an edit changes the motion from the start (a function of the edit).

## D2. The drive — the edit's speed

The acceleration of the frame the letters live in: a_home(t) = d²h(t)/dt² in timeline time, where the home's timeline
motion = the crawl/offset in material time composed with the outer time map (the title's own warp and its containers',
`motion-driven-params` D1). A running line at a constant speed has a_home = 0 (the letters rest at home); a change of the
Slowed&Reverb rate or of an instance's speed gives a_home ≠ 0 along the line's direction. Computed in closed form from
the piecewise-linear profiles and the offset curves.

## D3. The behaviours

- Sway (default, Q1 a): a damped spring per glyph, `m·p'' = −k·p − c·p' − m·a_home`; rotation by the same law with a lever
  of half the glyph height.
- Hang: a pendulum from the glyph's top centre, angle θ with `θ'' = −(g/L)·sin θ − c·θ' − (a_home/L)·cos θ`.
- Drag: each glyph is roped to the previous one (a spring of rest length = their layout distance), the first to its home.
- Fall: as Sway until |m·a_home| > threshold, then the glyph detaches and falls under gravity, bouncing on the frame's
  bottom with restitution 0.3 and friction; a detached glyph stays detached for the event.
Parameters are passport rows with curves (stiffness, damping, spread, threshold, gravity).

## D4. The function and its cost

Step Δ = 1/240 s of material time (Q2 a), semi-implicit Euler, f64 with the frozen math. The state at time t is the
integration from the event's material start to t; a frame at t interpolates linearly between the two steps around it.
Checkpoints: the full state every 240 steps, kept in memory per title (bounded, LRU) and written to the persistent cache
keyed by the title's values, the glyph homes' hash and the drive's profile hash. Cost: glyphs × 240 per second of
material from the nearest checkpoint — 100 glyphs, one second: 24 000 body steps, microseconds. A test proves a frame
equals byte for byte with and without checkpoints.

## D5. Output

Per glyph an offset (x, y) and a rotation, handed to the title's layout as it hands animator offsets today; the title
rasterizes as always. Nothing else draws.

## D6. Class walk — "motion that must react to the edit's speed"

| Thing | Reacts today? | Here |
|---|---|---|
| title letters | no | D1–D5 |
| an effect parameter (fisheye) | `motion-driven-params` | the same channels |
| particle generators (future) | — | would use the same drive |
| a reel's own content | via outer speed links | — |

## D7. Idle

No work at rest; a paused frame is computed once from its checkpoint.

## Rejected

- **Stateful playback-time simulation** — depends on where playback started (F-FX-005, R.59).
- **Baking to keys** (Newton) — every edit of the text or the speed would need a rebake.
- **Driving by the playhead** — the B7 lesson: the edit's speed is the driver.

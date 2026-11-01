# Design

## Context

`pitch-event-edges` D2 limited the context to chains with no tail and no warp before the last
edge-reading node: "a reverb before pitch would carry the context into its tail", "the source beyond a
warped event's edges is a function of a rate curve that does not exist there". The owner's events
are exactly such a chain (a morpher appends pitch AFTER the event's own Slowed+Reverb), so they kept
cold edges: a dip to 3 % at every join. Observed in the code:

- the warp map is defined outside `[0, 1]`: `consumed_abs`/`source_progress` extrapolate the first
  segment's speed before the start, the last segment runs on past the end; the signed address map
  (`addr_run_at`) extends its first run backwards;
- every node before the pitch is causal or has its own look-ahead counted in `La`: feeding it the
  material past the out-point changes its output only past the out-point, which the content gate cuts.

## Decisions

### D1. The chain up to the last edge-reading node processes the material

`chain_context` no longer looks at the nodes before the last edge-reading node. The pre-roll (still
`max(chain tail, 100 ms)`, unclamped) runs the WHOLE chain on the material before the in-point; the
input continues `La` past the out-point; the content gate after the last edge-reading node cuts to
`[−head_ext, clip_len + tail_ext)`. A reverb before the pitch starts the event with the reverb of the
material before it and its tail past the out-point is cut with the event.

Alternatives: (a) reset the nodes before the pitch at the in-point (the reverb starts fresh as
without pitch) — keeps the start, but the end still needs material past the out-point for the pitch,
and there the prefix's tail and the material are one signal; the rule would differ at the two edges;
(b) feed the material past the out-point only for the look-ahead and let the prefix's tail ring
after — the pitched material past the out-point is then audible (≈ 40 ms of foreign sound after every
event), or the tail has a 96 ms hole; (c) keep the cold edges for such chains — the owner's complaint.
(a)–(c) rejected; the cost of D1 is the lost tail of a node standing before the pitch, named in the
report so the owner can decide on it.

### D2. A warp before the pitch: forward at the start

The context applies with a time warp when the speed at the event's start is at least the audio floor
(forward motion): the pre-roll consumes the material before the in-point at that speed, which is what
`consumed_abs` and the address map already give for negative positions. A curve that starts in hold or
backwards keeps the cold edges (the material "before" such a start has no meaning).

### D3. The hand-off stays narrow

`run_predecessors` additionally requires no warp: under a warp `source_in + len` is not where the next
piece's material continues. A tail before the pitch already fails `ext_end == end`.

## Risks / Trade-offs

- [The tail of a reverb/echo BEFORE the pitch ends with the event] → D1; the owner is asked.
- [A Slowed+Reverb + pitch event now pre-rolls `max(tail, 100 ms)` of material before every entry] →
  the same warm-up a seek inside the event always paid; the prefetch orders it (t1108 with a warp).
- [Chains with a node before the pitch sound different at their edges] → it is the fix; t973c covers
  single kinds only and does not move.

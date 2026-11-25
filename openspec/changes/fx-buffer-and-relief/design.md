# Design

## Context

Read on 36d751a7: a chain's only time sources are the event's own moment, a stream stage's history and, since
`fx-graph-dag`, READS — a derived clip of the event whose chain is a line, with its own prefetch and cache keys
(`fx-graph-dag` D2; MAX_READS 4). Warp plugins bring their own WGSL arm (`warp_<id>`), so a warp needs no core mode.

## Decisions

### D1. `droste`
`warp(k, p)`: to log-polar (`ln r`, θ) with one seam at θ=π, `ln r` shifted by `zoom·turn` and wrapped by the period
`ln scale`, then back. Rows `scale` 1.2–16, `turns` 0–4 (the spiral), `phase` (curvable: the endless zoom), `cx, cy`.
Seven numbers; the probe map in f64 and the WGSL mirror are one formula (the pair rule of warps). `scale` near 1 or the
pole are clamped away by `r ≥ 1e-4`; a probe of the pole counts as minified (the planner's supersample is
bounded by its 4× cap). `turns = 0, phase = 0` is no step (neutral), not an identity map computed.

### D2. `relief`
One pass: Sobel of the luma (premultiplied on alpha) → normal `(−dx·depth, −dy·depth, 1)` normalised → `ambient + diffuse·
dot(N, L) + spec·pow(dot(N, H), gloss)` on the source colour or on grey (a choice: colour / relief). Rows: `depth`, `light
angle`, `height`, `ambient`, `specular`, `mode`. K×: depth is in pixels of the logical grid (multiply by `res_scale`).
`depth 0` gives the mode's flat result, `mode colour` at ambient 1 is the input: neutral.

### D3. `trails`: the past window
`Decl.past: Option<PastDecl { count: &'static str, step: &'static str, max: u8 }>` names the two rows whose values give N
(0–7) and the step in frames (1–30). The host, for a kind with `past`, builds N reads of the SAME line up to the slot at
`t − i·step` (project-frame grid, the event's own map for the moment), exactly as `fx-graph-dag`'s reads: derived clips, their
keys, their prefetch, their bars. The merge is one pass of N+1 inputs in a chain of two-input merges (levels `level_1…`
by a geometric `decay`): `out = in·w0 + Σ in_i·w_i`, normalised by `Σw` or not (`add` choice; `add` clamps at 255).
Before the event's start a read gives the event's first moment (no black), as a blended stretch does.
- Bounds: N ≤ 7 per layer, a bound of its own (`MAX_PAST`): `MAX_READS` (4) counts the arms' reads of `fx-graph-dag` and a
  past frame is cheaper only where the decode order is sequential, so it is stated apart. Two `trails` in one layer share
  the 7; the editor refuses over it with the reason (the read law's text). A read is made for a leaf event only
  (`fx-graph-dag` v2b): on a reel instance `trails` is refused with that reason. `count = 0` is the plain line: no read, the same bytes.
- Placement: `trails` in the middle of a chain means each read runs the prefix again: it is stated in the picker.
- A stream slot or a warp before `trails` in its line: the read is a line with them (a Datamosh's history is per
  read: the memo ledger counts it): allowed, with the cost in the tooltip of the slot ("7 past frames, each N ms").
- Export, preview, cells: the same plan (invariant 7). Cells of the gallery use 2 reads.
- The prefetch lists `wanted_frames` of each read; the persistent cache keeps the finished composite (final frames only, his
  D1), the ledger counts the reads' frames.

Rejected: a recursive echo (frame t blends the OUTPUT of t−1): it is a state, a seek would need a walk (Datamosh's
machine for one effect), and the owner's "buffer + blend levels" is a set of frames with weights.

### D4. Class walk
Who else wants the past: `temporal-barrier-3dnr` (its own window, it is a stream), motion blur by frame blending (a
stretch), datamosh (a history). `PastDecl` is for point-in-time windows only; the three stay. The sound has no past window: delay
is the node.

# Design: fused-pointwise-runs

## Context

engine-audit §3 and row 10, mechanism M7; feature note F-EXT-005. Before this change a chain of
point-wise effects outside a geometry series ran one fragment pass per effect: `chain_ops` turned
each `PlanOp::Barrier` of a point-wise plugin into `FxOp::PluginTap`, its own pipeline, a pool
texture, a bind group and a full-frame read and write. Point-wise stages already fused in one place:
a stage between two geometry steps is absorbed as a tap op of the geometry pass (R.45, item 312).
Leading and trailing point-wise stages, and every run that a barrier or a stop isolates from
geometry, did not.

## D1. The class walked

The mechanism: a stage whose output pixel depends only on the same input pixel is executed as a pass
of its own, so a run of k such stages moves the frame through memory k times and pays k dispatches.
Every chain stage of every family and every core node, sorted:

| Stage | Class | In a run? | Why |
|---|---|---|---|
| color_grade, posterize, duotone, vignette (`fx-video-color`) | point-wise, `tap_<id>` WGSL, no barrier kernel | **fuses** | the barrier position is the tap; vignette is positioned (reads the stage's size, the same in a run since point-wise never changes it) |
| greenscreen (`fx-video-color`) | point-wise with a barrier kernel | ends a run | its barrier position runs `greenscreen.wgsl` (the kernel's own text with its despill channel in `u0`), not its tap; fusing the tap there would change its bytes (t181 holds the kernel) |
| noise (`fx-video-stylize`) | point-wise with a barrier kernel | ends a run | the barrier kernel nails the grain to the K× grid (`res_scale`); the tap is the series' form |
| pixelate, halftone (`fx-video-stylize`) | barrier | ends a run | a cell / a screen: reads a neighbourhood |
| edges, chromatic_aberration, vhs (imitation) (`fx-video-stylize`) | barrier | ends a run | neighbour reads (gradients, shifted channels, line jitter) |
| blur, glow, sharpen, radial_blur, drop_shadow (`fx-video-blur`) | barrier | ends a run | mips and taps over a neighbourhood |
| camera_isp (`fx-video-camera`) | barrier, 12 passes | ends a run | a mosaic, NR, demosaic: neighbourhoods; its point-wise passes (YUV, view) live inside its own graph — see "not taken" |
| vhs_tape (`fx-video-tape`) | barrier, 14 passes on its own grid | ends a run | a signal simulation on a 4 fsc grid |
| datamosh (`fx-video-codec`) | stream (source stage) | transparent | the planner never sees it; it runs before the chain |
| lens_distort, twirl, ripple, kaleidoscope, cylinder3d, corner_pin, mirror, event_grid, pan3d (`fx-video-distort`) | geometry | ends a run | a coordinate map: the geometry series (R.44), which absorbs point-wise stages between its steps already |
| Geometry node, CamMotion (core) | geometry | ends a run when active; an identity Geometry node is transparent | as a geometry window treats it |
| LayerMix (core strength slot) | two inputs | ends a run (its snapshot point is a stop, its mix an op) | the mix reads the snapshot and the current state |
| shape masks (core, `MaskAlpha`/`MaskMix`) | two inputs (the coverage) | end a run (mask points are stops) | a coverage texture is a second input; folding it into a run is a different pass shape (not taken) |
| unpremul/premul, resample (host) | host passes inside barriers / at a nest boundary | not chain stages | |
| audio slots | transparent | transparent | the planner skips them |
| transitions (`transitions-*`) | the composite's two-input kernel | not a chain stage | they run inside the composite pass already, one per layer pair |
| generators (`generators-*`) | sources | not a chain stage | a generator's frame is the chain's input texture, drawn by its own shader; nothing to fuse with |
| composite, display (host) | the frame's end | not a chain stage | one pass a layer (the ping-pong of determinism, t183) |

What the class says for each "ends a run": the stage needs a neighbourhood (a barrier), a second
input (a mix, a mask), a different text at its barrier position (noise, greenscreen), or it is a
coordinate map handled by the geometry series' own fusion.

**Not taken, recorded:** point-wise passes INSIDE a barrier's graph (Camera ISP's YUV/view, VHS
Tape's colour passes) — a barrier declares its graph; letting it mark nodes point-wise is a later
contract change. A trailing run right after a geometry series could be applied after the series'
box average in the same pass (the leading load is the geometry pass's own); that changes bytes by
the removed boundary and the geometry text is shared with t1766's specialization — a later change.
A mask's alpha multiply at a run's end likewise.

## D2. Who decides: the plan names the run, the executor runs it

`plan_chain` is the one place of chain decisions (invariant 7 at the chain level): it emits
`PlanOp::PointRun { i, j, res_scale }`. The K of a run is the K its first stage had as a barrier:
consecutive barriers already got the same K (the same tail zoom and compression), so the plan's
resolution arithmetic did not change. A lone stage stays `Barrier` — its bytes and op are as before.

Which stage fuses is the plugin contract's answer (`kt_plugin_api::fuses_in_runs`): class `PointWise`,
GPU side `Gpu::Wgsl` (its `tap_<id>`), no barrier kernel. Nothing is guessed from a kind. The registry
now refuses a point-wise WGSL without `fn tap_<id>(` or one that reads a texture or the host's `P`:
such a text compiled in its own pass (which binds `P` and `T0`) and would break inside a generated one.

The executor's freedom is the same as for geometry shapes (pipeline-prewarm D4): a shape whose
pipeline is not warm runs its parts, with the same bytes.

## D3. The generated pass, and why its bytes equal the parts

Text (`runs::run_text`): the geometry pass's bind group layout (a 1024-byte uniform and `T0`), one
`textureLoad`, then for every stage `v = clamp(round(tap_<id>(v, R.p[2k], R.p[2k+1], c)), 0, 255)`,
then one store. The own pass stores `vec4<u32>(clamp(round(tap(v)), 0, 255))` and the next own pass
loads it as `f32` again: an integer is exact in `f32`, so the fused text feeds every stage the very
value the own pass would read. The fused and unfused forms are the same per-stage operations on the
same integers.

The owner agreed (H16) to let fused chains move by ≤ 1 LSB per former boundary, which would allow the
intermediate to stay `f32`. We keep the rounding, for three reasons:
- posterize is a step function: an unrounded input can cross a level and move a pixel by a whole
  level, not by 1 LSB;
- equal bytes make the run-time switch (D4) invisible: a timing-dependent choice between two programs
  is safe for invariant 11 (same bytes between runs) and for the kept composites only if both programs
  give the same frame;
- the stored references do not move.

The cost of keeping it is one `round` and one `clamp` per stage per pixel in registers; the memory
traffic — the cost this change is about — goes. Measured: byte-equal on llvmpipe over every ordered
pair of the four fusable plugins at defaults and tuned values, six stages, forty stages (two pieces),
a run in a strength slot and runs beside a geometry series (t2190). The tolerance the test enforces
is the owner's contract (≤ 1 LSB per removed boundary) for a compiler that contracts inside the
inlined stages differently; on a software adapter it asserts exact equality.

The uniform holds 32 stages (two `vec4` each); a longer run is cut into pieces of 32 (`MAX_RUN_STAGES`,
a property of the uniform, not of a machine).

## D4. The switch and the cap, measured on the device

A warm shape is tried (`runs::Trial`): frames alternate between the fused pass and the parts, the
pass timer of gpu-pass-cost times both (`Timer::probe` marks the passes; `harvest` returns the
summed device time per piece), and after five tries of each the minima per pixel are compared
(minima of repeats, interleaved: the CLAUDE.md rule for a shared machine). A fused pass slower than
its parts is SPLIT into two halves, each a shape with its own trial — the run-length cap is what the
device says, by halving, never a constant. A device without timestamps (the GL backend) keeps the
fused pass: it moves fewer bytes on every device. A trial whose samples never come back (the timer's
eight pending frames full) ends fused after 64 asks. Because D3 makes the two forms byte-equal, the
trial moves no stored, kept or exported frame; the export (an engine of its own) tries on its own
frames the same way.

`FuseRuns::{Always, Never}` force the forms for the tests and `kt-cli fuse-cost`.

## D5. Compile and prewarm

`Compiled` keeps run shapes by their stage indices (`plugin_taps()` order) like geometry shapes: a
new shape goes to the warm worker (`Job::Run`, under `exit::busy()`, one pipeline a submission after
the frames composing, a shadow device first on a watched software device), compiled on the geometry
layout with the disk pipeline cache, then drawn once into the warm kit's 6×6 target. `run_use` hands
out only warm pipelines, so a frame never compiles and never draws cold (t2192). The player hands
the project's run shapes (`prewarm::run_shapes`: `kt_fx::plan::run_shapes` over clips, tracks,
morpher patches and compositions) to `VideoGpuExec::prewarm_runs` with the effect ids, and asks
again only when either set changed (t2196); the export does the same on each device before it settles.
`run_shapes` reads a chain without its frame context (no masks, no lead): a run the plan cuts
differently compiles when a frame meets it.

## D-measure

`kt-cli fuse-cost`, release, llvmpipe 20.1.2 (Vulkan, 256-bit), 1920×1080, the colour family in turn
at a third of each row's range, 7 interleaved blocks of 4 frames a form, minima; load average 10.9 on
a shared machine. "Chain" is the device time of the chain's passes from the pass timer; "frame" the
wall time of a frame with its upload, the composite and the readback.

| stages | bytes moved by the chain, unfused → fused | chain, device ms | frame, ms | same bytes |
|---|---|---|---|---|
| 2 | 33.2 → 16.6 MB | 3.11 → 1.77 | 5.8 → 4.1 | yes |
| 3 | 49.8 → 16.6 MB | 4.72 → 1.81 | 7.9 → 4.4 | yes |
| 4 | 66.4 → 16.6 MB | 7.22 → 2.33 | 10.9 → 5.5 | yes |
| 5 | 83.0 → 16.6 MB | 7.47 → 2.05 | 10.1 → 4.4 | yes |
| 6 | 99.5 → 16.6 MB | 9.30 → 2.22 | 12.6 → 4.8 | yes |

A fused pass costs about what ONE own pass costs (1.8–2.3 ms whatever k): on llvmpipe these stages
are bound by the frame's trip through memory, not by their arithmetic, so the audit's guess of
"(k − 1) × 2 ms" holds (k = 6: 7.1 ms saved). The bytes are device-independent: k × 8 B/px → 8 B/px.
On llvmpipe the trial kept every shape fused (t2192 decided after 10 frames; no split).

**The heavy showcase** (`kt-cli showcase --heavy`, the footage in /dev/shm): its chains are
VHS Tape, Camera ISP, Slowed+Reverb, blurs and glows; it holds ONE point-wise run (Posterize →
Vignette, one event at 62.4 s). `kt-cli preview heavy.kt 62.4 67.4`, A/B by `KT_FUSE_RUNS`, two runs
each: a frame there costs 1059–1073 ms of device time, of which Camera ISP and VHS Tape take ~95 %;
V-fps 0.2 either way. The run's two passes are 4.3–5.7 ms of it — the change cannot be seen on this
project, and the measurement says so instead of claiming a gain. (Only two frames of the range were
composed, both before the shape was warm, so both forms ran the parts there.)

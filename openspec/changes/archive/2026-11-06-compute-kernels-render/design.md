## Context

`compute-kernels` moved the Datamosh encoder to compute shaders and left one measured wait: 6628 ms on
the first frame after a start, written down as device open plus shader compile.

## Decisions

### D1. No disk cache of shaders: Mesa's already does the work, and the 6.6 s was a load spike

Measured through `first_frame_cost` (ignored test, fresh process, 1080p, llvmpipe, load 17–18):

| | open | intra | first P | next P |
|---|---|---|---|---|
| empty Mesa cache | 62–209 ms | 247–690 ms | 130–225 ms | 61–82 ms |
| second start, same cache | 37–44 ms | 79–85 ms | 71–90 ms | 53–60 ms |
| `MESA_SHADER_CACHE_DISABLE` | 48–56 ms | 231 ms | 112 ms | 52–59 ms |

The composite engine opens in 660–890 ms warm or cold. The 6628 ms was one run at load 20 right after
the shader changed, and 2.6 s was seen once at load above 50; five later cold runs took under 0.7 s.
A second start already has no wait on Mesa, and the wgpu pipeline cache is Vulkan only, so a cache of
ours adds a store with no measured gain (project rule: caches where they pay). Not measured: a real
driver, DX12 (FXC compiles on every start there and wgpu has no cache for it).

### D2. Datamosh decodes in the encoder's job

`decode.wgsl` is the block copy plus the scaled residual, integers only, so the closed loop's
reconstruction comes from the same job as the residual and `encode` uploads nothing twice. A broken
frame is its own job. t1572 holds the clamp, the floored chroma vector and the gain.

### D3. The host pools buffers, not the kernel

A kernel asks for the same set every frame, whatever kernel it is, so the pool sits in `ComputeDevice`
by (size, usage), bounded by 1 GiB. A reused `Init::Zero` buffer is cleared in the job (t1570): the
pool is invisible to the bytes.

### D4. A software adapter composes on several devices

llvmpipe runs one frame per queue, and that was the whole gap against the CPU workers: a device per
frame worker removes it. An export asks `kt_gpu::export_engines`: a card gets one device, a software
adapter a quarter of the cores, 2 to 8, each with its own worker. The workers compose as the CPU's did
and the file does not depend on their number (t1573).

Measured with `kt-cli export` (release, 32 cores, libx264, other sessions running; two passes of the
same day, seconds of the whole export, peak memory on the second):

| | CPU composite | one device | 8 devices | 16 devices |
|---|---|---|---|---|
| demo, first pass | 101.0 | 115.6 | 70.5 | |
| demo, second pass | 36.8 (2.4 GB) | | 43.1 (6.6 GB) | 58.3 (11.2 GB) |
| showcase, first pass | 390.7 | 457.8 | 222.5 | |
| showcase, second pass | 167.4 (7.2 GB) | | 154.7 (17.0 GB) | 187.6 (25.5 GB) |

The load swung between 30 and 190 in these two passes. The pairs below ran one after the other, the
CPU binary built before the deletion against the final one, and they are the numbers to quote:

| | CPU composite | 8 devices |
|---|---|---|
| showcase | 114.7, 115.9 s (7.2 GB) | 144.8, 144.1 s (17.0 GB) |
| demo | 31.3, 48.8 s (2.4 GB) | 41.4, 41.8 s (6.6 GB) |

The showcase repeats to a second: 25 % slower. The demo is noisier and lands in the same range or
better. The earlier "about even" did not survive this. Before
this the owner was told 134 s against 96 s; that figure was the single device. The price is memory:
each device keeps its own textures. Limiting llvmpipe's threads per device gave no steady gain
(34 s to 300 s on the demo under load) and stays out.

### D5. What holds the picture once the CPU composite is gone

- The GPU tests compared every frame with the CPU composite. The last output of that composite is
  stored in `src/libs/kt-gpu/references/` (21 MB, 10 MB packed), and the same tests keep the same
  `FrameTol`: only the source of the left side changed.
- Every other test composes through `kt_gpu::testkit`: one engine per test process, skipped with a
  printout without a device, a failure under `KT_REQUIRE_GPU=1`. An export gets an engine of its own,
  as in the product: one engine's readback ring does not serve two exports.
- Byte hashes of video (t11, t485, t946) did not move: llvmpipe gives the CPU's bytes on them. They
  hold on a software adapter only (`hash_holds`); a card is held to the stored frames.
- Removed with the code they tested: the CPU post-FX memo and the sub-composite memo (t66, t66b, t66c,
  t66d, t441, t443, t582), compose-ahead (t1530–t1534, t1536), the SIMD parity of the composite
  (t198). t1019 keeps the numbers and panels of a transition; its pixels are t224's.

### D6. One engine serves several caches, and a blended frame is not its base frame

Two defects the CPU path had been hiding:

- Serials were per cache, so the offline engine, which serves a fresh cache per job, could take a
  frame of one project for another's. They are process-wide now (t1574).
- `UploadCache` is keyed by the serial, and a blended frame carries its base frame's serial, so a
  slowed event in blending mode showed the texture of another weight on the GPU. The key now takes
  the pair and the weight (t250 found it the moment it ran on the GPU).

### D7. What stays on the CPU, and what lives on only in tests

- The planner reads `GeoKernel::warp`: `probe_program_lens` measures a program's zoom through it, so
  the K of the plan is a function of the map. It is plan math, one text beside its WGSL form.
- Audio, generators, titles, a stream stage's history and frame blending of footage are not the
  composite and stay. Generators are the next math to move.
- The VHS physics tests and the camera's consequence tests assert what the signal does BETWEEN
  stages: the luma on the tape's grid against the input, a raw plane after readout. A shader gives no
  such grid. Rather than lose them, the f32 models moved to `tests/model/` of their crates: nothing
  ships them. The price is written down: they test the model, and t1186 and t1217 tie the shaders to
  frames the model made on the last day it was the product. An edit of a stage's shader must edit
  the model or these tests stop meaning anything.

Rejected: reading each stage back from the GPU for the tests alone (a test-only path through the
graph, slower and with nothing to compare a physical expectation against at f32 precision).

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

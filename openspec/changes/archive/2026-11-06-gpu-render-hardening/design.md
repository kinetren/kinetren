## Context

`compute-kernels-render` deleted the CPU composite. What it left is listed in `docs/found-defects.md`;
the orchestrator took it up on 2026-11-06.

## Decisions

### D1. The render's memory: devices, decoders, a tight mode

The 17 GB came from two places. Each device adds 1.3 GB at 1080p. The base of about 6 GB was mostly not
ours: seven VP9 files at `threads=auto`, sixteen frame threads each, held 2.7 GB of frames. A render
decodes a chunk at a time, not against a clock, so the decoders of a render now open with four threads,
and the render got faster: 130 s against 144 s at four devices. A preview's stay on auto.

- The device ceiling is an eighth of the cores, 2 to 4: eight devices took 6 GB more and gained nothing.
- The count follows the memory (`memory::engines_for`). The available memory is `MemAvailable` or the
  control group's limit, whichever is less, and `GlobalMemoryStatusEx` on Windows.
- When one device and the base do not fit, the render runs tight: two decoder threads, half the frame
  cache, four encoder threads.

The showcase under a hard limit (`systemd-run -p MemoryMax=… -p MemorySwapMax=0`, 32 cores, release):

| limit | devices | time | peak |
|---|---|---|---|
| 16 GB | 4 | 160.5 s | 8.7 GB |
| 8 GB | 3 | 139.3 s | 7.6 GB |
| 6 GB | 1 | 150.6 s | 4.8 GB |
| 5 GB | 1 | 140.8 s | 4.8 GB |
| 4 GB | 1, tight | 142.6 s | 3.1 GB |
| 3 GB | 1, tight | 143.0 s | 3.2 GB |

Before this, a 6 GB limit was killed by the OOM killer. Pairs one after the other: showcase 129.1 and
130.1 s at four devices, 136.7 and 144.0 s at one; demo 34.4 to 34.8 s and 35.6 s. The times in the table
differ by load, not by limit. A machine with 3 GB is also a machine with few cores, which only helps.

Against the deleted CPU composite, final code, pairs one after the other: showcase 128.8 and 129.8 s
against 99.1 and 101.4 s, 29 % slower; demo 34.2 to 34.7 s against 29.6 to 30.2 s, 14 % slower. Peak
8.7 GB against 7.2 GB and 4.3 GB against 2.4 GB.

### D2. Rules come back on the GPU path, under their numbers

t303 runs a true K: the frame physically K× the logical size with a zoom after the barrier, and the
test asserts from the plan that K is what it claims. Its thresholds did not move. t441 keeps the
starvation and preview-equals-export; its memo half has no subject. t942 counts the executor's
sub-samples, and a marked grid runs one more pass than the CPU did (the cell, then the copy). t943 keeps
its refusals; the tile's bytes are t944's. t544e and t544f run on `render_job`.

### D3. A model is held to its shaders live

Stored frames tie a shader to the day the model last made them, so an edit of a shader alone turned
only t1186 and t1217 red, and those could be re-blessed. t1640 and t1641 run the model and the GPU side
by side at every preset. `FrameTol::SIGNAL` was too wide to see it: moving one chroma constant by 3 %
passed it (mean 0.47 against the class's 0.65). So the tests assert the mean itself: 0.0005 clean, under
0.02 asserted. The one preset with a clogged head reads 0.44 clean, FM clicks one ULP apart, and has
its own 0.9. Both tests were seen red on a shader edited alone.

### D4. Windows: FXC always, a device where one can be had

The runner has WARP only, and WARP dies running our passes. Its compiler does not: t1650 and t1651
build every engine pipeline and compute shader on the D3D12 adapter and run no frame. That is the
compiler of every D3D12 machine, and it needs no download. `windows-picture` adds a real device:
Mesa 26.2.6's lavapipe and LunarG's loader 1.4.335.0, both pinned by hash, `WGPU_BACKEND=vulkan`,
`KT_REQUIRE_GPU=1`. It is `continue-on-error` until it has run once: this session has neither `gh` nor
leave to push, so it has not run. The compute host now skips WARP in the adapter list instead of taking
the first one, or it would refuse beside a working lavapipe.

### D5. The memo goes

No code outside kt-cache's own tests reads `PostFxVideo`, and nothing reads `memoize_postfx` or
`nest_memo_key`. t65, t65b and t1535 went with it.

### D6. Nothing may be on the device when the process exits

With the picture on the GPU, a background frame can be on llvmpipe when the process exits: `exit` runs
libLLVM's destructors under llvmpipe's threads, and LLVM segfaults. t547 went red that way one run in
three after passing. In the program this is a crash on close, and the supervisor would show it. The
thumbnail service now joins its worker on drop, and `on_exit` parks the offline engine: it waits for
the device and keeps the engine's lock, so a reel strip's next frame waits for the process to end.

### D7. Generators move where it pays

A compute frame with its readback costs 1.1 to 1.4 ms at 1080p on llvmpipe, so a kind moves only if it
costs more:

| kind | CPU | shader |
|---|---|---|
| line | 92.4 ms | 1.7 ms |
| ellipse | 90.5 ms | 1.8 ms |
| rect | 35.5 ms | 2.3 ms |
| leader | 32.0 ms | 2.3 ms |
| gradient | 13.8 ms | 2.2 ms |
| corners | 13.1 ms | 1.9 ms |
| noise | 7.3 ms | 2.2 ms |

Solid, transparent, timecode, window, checker, smpte and table stay: 0.2 to 3 ms, nothing to win, and
text is CPU work anyway (the leader's digit is drawn over its shader frame). Titles stay in kt-text.

The rule "float only in the preamble, the pixel loop is integer" cannot hold here: the distances are
i64 of 1/256 px squared, and WGSL has no i64. The shaders work in f32 pixels, and the bytes of the
moved kinds change by the rounding (t1048's digests were reprinted, and hold on a software adapter). The
noise stays integer and keeps its grain. With no device a shader kind gives a transparent frame.

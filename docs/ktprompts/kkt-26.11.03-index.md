# Registry of the owner's list `kkt-26.11.03`

Source — [`kkt-26.11.03.txt`](kkt-26.11.03.txt) (verbatim, the owner's message of 03.11.2026: two
requests). Columns and states — as in [`v26.10-index.md`](v26.10-index.md). The round is in
`docs/user-feedback.md` ("2026-11-03 — the final render is slow; one way to accelerate the math").

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| C1 | 1-2 | the final render (export) is VERY slow, slower even than the preview, and looks not parallel but strongly sequential | no: defect, measured before the fix → export, video-plan | `export-throughput` | queue 1 (next free executor) |
| C2 | 4-5 | "the math" is separate in every place and not accelerated; one mechanism to accelerate computations on whatever the machine has (AVX, CUDA, OpenCL, Vulkan) — something like compiled shaders, or another approach altogether | question: the owner allows removing or updating the video and audio references (2026-11-04), and a machine with no graphics card runs the kernels through Vulkan on LLVM; delete the CPU and AVX copies of the moved effects, leaving Windows with no driver and Linux with no mesa-vulkan-drivers without them? (F-EXT-004: Datamosh encoder as compute, 34 ms against 67 ms on software Vulkan, 46.01 dB both) → plugins | `compute-backends` | the owner's answer, then the migration round |

# Registry of the owner's list `kkt-26.11.03`

Source — [`kkt-26.11.03.txt`](kkt-26.11.03.txt) (verbatim, the owner's message of 03.11.2026: two
requests). Columns and states — as in [`v26.10-index.md`](v26.10-index.md). The round is in
`docs/user-feedback.md` ("2026-11-03 — the final render is slow; one way to accelerate the math").

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| C1 | 1-2 | the final render (export) is VERY slow, slower even than the preview, and looks not parallel but strongly sequential | no: defect, measured before the fix → export, video-plan | `export-throughput` | queue 1 (next free executor) |
| C2 | 4-5 | "the math" is separate in every place and not accelerated; one mechanism to accelerate computations on whatever the machine has (AVX, CUDA, OpenCL, Vulkan) — something like compiled shaders, or another approach altogether | question: start stage A — a shared multi-core CPU executor with SSE4.1/AVX2/AVX-512 copies of one loop, no byte changes? Stage B — the 6 point-wise kinds and 32 transitions as one Rust∩WGSL text? Keep the f64 reference or move to f32? Is ARM a target? (F-EXT-004 `docs/features/plugins/kernels/compute-backends.md`, memo at its end; measured on Color Grade at 4K: 108 ms today → 3.1 ms with the same bytes, t1500/t1501; no CUDA/OpenCL recommended) → plugins | `compute-backends` | the owner's answer, then the migration round |

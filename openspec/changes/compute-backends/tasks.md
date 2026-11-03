# Tasks

## 1. The prototype

- [x] 1.1 `kt_plugin_api::shade`: the Rust/WGSL subset prelude (f64 and f32 forms), `shared_kernel!`, `run_pointwise` (row bands on scoped threads × baseline/SSE4.1/AVX2/AVX-512 copies of one loop)
- [x] 1.2 Color Grade written once (`fx-video-color/src/single_source.rs`); t1500 — byte for byte the shipped kernel at every level and band count
- [x] 1.3 t1501 (`kt-gpu/tests/single_source.rs`): the WGSL form compiles in the own-pass frame and holds `FrameTol::BILINEAR`; the hand-written text beside it in the same harness
- [x] 1.4 Measured on release: CPU (shipped scalar 26.7/108.4 ms → single text AVX2 ×1 7.9/31.1, ×32 1.0/3.1 ms at 1080p/4K; byte-identical; the f32 form 1 LSB on ≈1/80 000 channels), GPU on llvmpipe (no real GPU in the VM: pass 1.5/5.3 ms, round trip 3.8/13.5 ms, both texts within BILINEAR), build ≈ equal, rlib +24 KB, ≈28 KB code per kernel for four SIMD copies

## 2. The note and the memo

- [x] 2.1 `docs/features/plugins/kernels/compute-backends.md` (F-EXT-004) with the numbers of 1.4; README coverage; `features-index.py --check`
- [x] 2.2 The decision memo in Russian at the end of the note; registry row C2

## 3. Close

- [x] 3.1 `cargo test --workspace --no-fail-fast` green but kt-app t640, red once under the full parallel run and green alone 3×3 (the known sidecar load red; t476/t411 time out only inside the Bash sandbox), clippy of the touched crates green; finish.sh is run by the orchestrator on the merged tree (owner, 2026-11-03); target-prune; archive

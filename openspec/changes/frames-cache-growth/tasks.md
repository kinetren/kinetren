# Tasks: frames-cache-growth

## 1. Before numbers

- [x] 1.1 Release `kt-cli` of the base; his region at 1280×720, 120 s loop, `--trace`: V-fps, frame
  evictions, frames cap, peak RSS (design "Measured")
- [ ] 1.2 The heavy showcase loop with the base binary (same measures)

## 2. The cache asks (found-defects 396)

- [x] 2.1 `BurstCache::want_video`, cap read at use, shrink hook evicting at once, own lower want
  evicts, `Drop` gives the want back, `with_ledger` for tests — t2390, t2391, t2392
- [x] 2.2 Prefetch: the whole window's bytes, `loop_frame_bytes`, `frames_cache_want`,
  `size_frames_cache` on the player's prefetch thread — t2393
- [x] 2.3 The stream pacing reads the cap through `video_cap_reader` — t2394

## 3. The numbers say what they count (found-defects 397)

- [x] 3.1 `kt_gpu::memory::resident`, `kt_time::mem::set_resident_probe` — t2399
- [x] 3.2 Status bar frames/cap and program memory with a hint, performance row, occupancy, caches'
  line with the mix, process line; five catalogs — t2397, t2398, t2301, t128, t100, t125
- [x] 3.3 Trace kinds 93–95, `mem::sample` once per busy second (load watch, `kt-cli preview`),
  `kt-cli trace memory` — t2395, t2396

## 4. Verification

- [ ] 4.1 Tests of the touched crates (nextest), clippy
- [ ] 4.2 After numbers: his region and the heavy showcase, release, same commands as 1.x
- [ ] 4.3 Live `diagnostics`, `idle`, `heavy_loop` with screenshots of the new numbers
- [ ] 4.4 found-defects (396, 397 leave; rows 440–441), What's new, specs merged on archive

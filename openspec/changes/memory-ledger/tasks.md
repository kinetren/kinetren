# Tasks: memory-ledger

## 1. Before numbers

- [x] 1.1 Release `kt-cli` of the base; loop over `problem_region_26_11_09` at 640×360 for 240 s with
  `--trace`, `kt-cli trace loop` per lap (design.md Context)
- [ ] 1.2 Export peaks of our showcase at 1080p and 4K (max RSS, `--time`) with the base binary (design D7)

## 2. The ledger (kt-time)

- [ ] 2.1 `kt_time::mem`: `Tenant` (order of value), `Account` (atomics), bases, wants, `set_probe`,
  sizing with the growth budget of D2, `on_cap` hooks outside the lock and never for the asker,
  `Claim`, `snapshot()`; trace kinds `MemBudget = 91`, `MemGrant = 92` written only on change —
  t2290, t2291
- [ ] 2.2 `kt_engine::video::set_memory_probe`/`memory_available` forward to the ledger; kt-app and
  kt-cli inject the probe once

## 3. Keepers report and obey

- [ ] 3.1 BurstCache: accounts `Frames` (video + audio) with the config as base
- [ ] 3.2 kt-gpu: PresentMemo (`Kept`: cap = base + grant, D5 no-admit with the loop's length, hook
  evicting under the engine's lock), UploadCache, ReadbackCache, TexturePool, compute pool accounts;
  `VideoGpuExec::keep_loop` and the executor's want from its composite size — t2292, t2293, t1934
- [ ] 3.3 Refusal guard on non-CPU devices with a test hook — t2295
- [ ] 3.4 Export device uploads: frame counter, frames-in-flight rule on export devices, same-frame
  reuse guard on all — t2298, t1573, t1457
- [ ] 3.5 Stream memo through the ledger (`History`, hook = `set_cap`), MixStore account — t2275,
  t2276, t2297
- [ ] 3.6 Player: `keep_loop` on the display executor when the region or frame count changes —
  t2296; row 119 end to end — t2294
- [ ] 3.7 Export claim in the pipelined and serial export bodies — t2299; t1600–t1605, t1570 green

## 4. Visible

- [ ] 4.1 `kt-cli trace memory` — t2300
- [ ] 4.2 Diagnostics memory line, catalog key in the five catalogs — t2301, t100, t125

## 5. Verification

- [ ] 5.1 `cargo test` of kt-time, kt-cache, kt-gpu, kt-engine, kt-cli, kt-app; the TEST lines of
  video-plan, media-index, export, telemetry; clippy
- [ ] 5.2 After numbers: the owner's region loop (release, same command), the heavy showcase loop, the
  export peaks at 1080p and 4K; into design.md
- [ ] 5.3 Live `idle` and `heavy_loop` (rest costs nothing, the loop turns bright); CPU at rest with
  Diagnostics open on the heavy showcase
- [ ] 5.4 docs: crate CLAUDE.md (kt-time, kt-gpu, kt-engine, kt-cache), `docs/engine-audit.md` row 9
  and M9, `docs/found-defects.md` (310, 119 out; 10, 85, 96 updated; 330–331 in), whats-new line,
  F-RES-011 verdict
- [ ] 5.5 Archive (specs merged); `src/scripts/finish.sh` is run by the orchestrator per stage

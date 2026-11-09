# Tasks: stream-layout-per-build

## 1. Measure first

- [x] 1.1 `kt-cli stream-cost <p.kt> <from> <to> [--laps N] [--size WxH] [--loop]`: the advancing
  thread's processor time, the jobs' wall time, bytes up and back per frame (a counting wrapper of the
  compute host), the prefetch window's question, lap by lap with the preview's memo
- [x] 1.2 Before numbers on `problem_region_26_11_09` of the relinked copy (release): design.md Context

## 2. The schedule compiled once (video-fx, audio-fx)

- [x] 2.1 `mosh::Plan`: per-position forward recurrences (rows, node, dependency, chain index, keys);
  `needs`/`held`/`key`/`render` read it — t2270 (pinned from the per-call walk), t2271
- [x] 2.2 `StreamLayout::plan_cell` + `PlanCell`; the engine's cell per stage, build and stream size,
  fresh in `link_stages` — t1241, t1242, t1246, t1248, t2040–t2045
- [x] 2.3 The audio half keeps one plan per rows, curves, placement — t2272, t1471–t1476
- [x] 2.4 `stream_plan` trace record — t2277

## 3. Packed planes and the conversion on the device (video-fx)

- [x] 3.1 residual 16-bit pairs, picture bytes four to a word (residual.wgsl, decode.wgsl, codec.rs) — t2273, t1239
- [x] 3.2 `convert.wgsl` as the encoder job's first step (`encode_rgba`), `rgba.wgsl` for the output
  (`to_rgba_gpu`), the CPU functions as reference — t2274; prewarm list with per-shader bindings — t1651, t2120-family tests in the crate

## 4. The memo against the loop (video-plan, telemetry)

- [x] 4.1 `StreamMemo::set_cap`; `stream_memo_bytes` and `size_stream_memo`; the injected memory probe
  (kt-app, kt-cli) — t2275
- [x] 4.2 The player sizes it when the loop or the stage size changes; `stream_memo_cap` record — t2275
- [x] 4.3 A loop the sized memo holds runs no job on lap 2 — t2276

## 5. Found while measuring

- [x] 5.1 `StreamKernel::advance`: the advance converts no picture — t2278
- [x] 5.2 `StreamMemo::put_transient`: the encoder's state evicted before the pictures — t2276, t2275

## 6. Check and record

- [x] 6.1 After numbers (`stream-cost`, the preview loop on the region, release), design.md
- [x] 6.2 Rest costs nothing: live `idle`, `heavy_loop`, `datamosh` green (pause 2 % of a core, effects window open 2 %, a covered loop plays only kept composites)
- [x] 6.3 found-defects: rows 135, 143, 144 to fixed as far as closed; rows 310–314 for the class walk
- [x] 6.4 engine-audit row 8 done; whats-new line; clippy; target-prune
- [x] 6.5 Archive (specs merged), design record in video-fx/video-plan design.md
- [ ] 6.6 `src/scripts/finish.sh` — run by the orchestrator per stage (not by this executor)

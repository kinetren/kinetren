## 0. Records

- [x] 0.1 The owner's request of 2026-10-30 is in `docs/user-feedback.md` (check that the entry names this change)

## 1. Reproduce and separate the hypotheses (design D1)

- [x] 1.1 A `--ignored` release measurement `t991_bench` in `src/libs/kt-engine/tests/cost.rs`: the lone Slowed+Reverb slot at 70 % of its number rows over 10 s of clicks, best of 5, printed. Run it in 20 process runs and record the two modes here (`cargo test --release -p kt-engine --test cost -- --ignored --nocapture t991`)
  - Lone slot, source built in the test: 20 runs 103.1–111.8 ms, ONE mode. The stage-3 order (gain first) on ed78f71: slowed_reverb 405 / 429 / 328 ms in 3 of 3 runs, the 18-kind chain 627–687 ms. `gain,slowed_reverb` in one process: renders 416, 103, 416, 103, 416 ms — the mode flips between renders, gain too (5.4 / 43 ms). The bench now builds the source with a clean and with a dirty pattern payload: 109 / 421 ms before the fix
- [x] 1.2 H1 placement: 20 runs under `setarch -R`; log each comb/allpass buffer address mod 4096 per run; `perf stat -e ld_blocks_partial.address_alias,l1d.replacement` on a slow and on a fast run. Record which modes appear and what the addresses were
  - `setarch -R`: 20 runs 101.7–113.0 ms (one 243.9 under another session's build). `perf stat`: unavailable (KVM guest, `perf_event_paranoid` = 4, no hardware PMU events). The addresses were logged per render; a fetcher-only micro gave 52.4 and 346.6 ms at the SAME source and burst addresses. Rejected
- [x] 1.3 H2 core type: `lscpu --extended`; 10 runs pinned to a P-core, 10 to an E-core (`taskset -c N`). Record the mode per pin
  - The machine is a KVM guest (32 vCPUs, i9-13900KF host); the guest does not see P/E cores. A `taskset` sweep over the 32 vCPUs ran under load and was inconclusive; 40 unpinned runs landed on 20 different vCPUs, all 101–111 ms; the mode flips between renders milliseconds apart in one process. Rejected
- [x] 1.4 H3 subnormals: count subnormal values in the comb/allpass state after the render (diagnostic code, not committed); one run with FTZ/DAZ set (diagnostic only). Record
  - The state after the render: 0 subnormal values of 96,246. During it: 267,675 of 11,520,000 comb/allpass operations touch a subnormal, the same count every run. FTZ+DAZ: 103 → 86–92 ms, and the 415 cluster disappears — but its subnormals were the SOURCE's (below). The reverb's own are a constant ~12 ms, not a mode
- [x] 1.5 H4 the warp: the same slot at rate 1.0, and at mix 0 (warp only). Record
  - Rate 1.0: 142 ms (5 of 5); mix 0: 104–114 ms; both: 148–154 ms. Stage split of a slow and a fast render: reverb nodes 23.7–27.1 ms in both, source fetch 370–397 against 58 ms, copy 1 ms. The warp only multiplies the source reads by 12.8. Rejected
- [x] 1.6 Name the cause in `design.md` (a dated "Finding" section) with the numbers
  - `SyntheticSource::read_audio_burst` matched the pattern per sample; the compiler hoisted the `Sine`/`ClicksBed` `sin()` out of the match, and for `Clicks` it ran on the payload's leftover bytes — a stack address, 6.77e-310, a subnormal in the slow runs

## 2. Fix without moving a byte (design D2)

- [ ] 2.1 The fix of the named cause. For H1: one buffer per reverb with alias-free line offsets. For H3: flush on write, ONLY if t973c and `GOLDEN_AUDIO` stay. For H2: no code; the finding goes into `openspec/specs/audio-render/design.md`. Verify t11, t973c and t6 green
- [ ] 2.2 t991 (`src/libs/kt-engine/tests/cost.rs`): the proxy assertion of D3 for the named cause. Write it first and see it red on the unfixed code, then green
- [ ] 2.3 Re-measure 1.1 (one mode over 20 runs), the 18-kind chain and t293's cost bound; record here
- [ ] 2.4 kt-fx's `CLAUDE.md`: the code rule if the cause was in the code (for example, "a reverb's lines live in one buffer at alias-free offsets"); commit

## 3. Finish

- [ ] 3.1 `cargo test --workspace` and `cargo clippy --all-targets` green; `python3 src/scripts/target-prune.py`; `./scripts/finish.sh`; the version in the report

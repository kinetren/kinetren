## 0. Records

- [x] 0.1 The owner's request of 2026-10-30 is in `docs/user-feedback.md` (check that the entry names this change)

## 1. Reproduce and separate the hypotheses (design D1)

- [ ] 1.1 A `--ignored` release measurement `t991_bench` in `src/libs/kt-engine/tests/cost.rs`: the lone Slowed+Reverb slot at 70 % of its number rows over 10 s of clicks, best of 5, printed. Run it in 20 process runs and record the two modes here (`cargo test --release -p kt-engine --test cost -- --ignored --nocapture t991`)
- [ ] 1.2 H1 placement: 20 runs under `setarch -R`; log each comb/allpass buffer address mod 4096 per run; `perf stat -e ld_blocks_partial.address_alias,l1d.replacement` on a slow and on a fast run. Record which modes appear and what the addresses were
- [ ] 1.3 H2 core type: `lscpu --extended`; 10 runs pinned to a P-core, 10 to an E-core (`taskset -c N`). Record the mode per pin
- [ ] 1.4 H3 subnormals: count subnormal values in the comb/allpass state after the render (diagnostic code, not committed); one run with FTZ/DAZ set (diagnostic only). Record
- [ ] 1.5 H4 the warp: the same slot at rate 1.0, and at mix 0 (warp only). Record
- [ ] 1.6 Name the cause in `design.md` (a dated "Finding" section) with the numbers

## 2. Fix without moving a byte (design D2)

- [ ] 2.1 The fix of the named cause. For H1: one buffer per reverb with alias-free line offsets. For H3: flush on write, ONLY if t973c and `GOLDEN_AUDIO` stay. For H2: no code; the finding goes into `openspec/specs/audio-render/design.md`. Verify t11, t973c and t6 green
- [ ] 2.2 t991 (`src/libs/kt-engine/tests/cost.rs`): the proxy assertion of D3 for the named cause. Write it first and see it red on the unfixed code, then green
- [ ] 2.3 Re-measure 1.1 (one mode over 20 runs), the 18-kind chain and t293's cost bound; record here
- [ ] 2.4 kt-fx's `CLAUDE.md`: the code rule if the cause was in the code (for example, "a reverb's lines live in one buffer at alias-free offsets"); commit

## 3. Finish

- [ ] 3.1 `cargo test --workspace` and `cargo clippy --all-targets` green; `python3 src/scripts/target-prune.py`; `./scripts/finish.sh`; the version in the report

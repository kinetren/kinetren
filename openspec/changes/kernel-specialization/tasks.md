# Tasks: kernel-specialization

- [ ] 1.1 Measure before: `kt-cli fx-cost --secs 5` (release), his chain (`--project haski-relinked.kt --clip 2133`), `probe 4632 4652 --time`
- [ ] 1.2 Print the reference outputs on the base tree: loudness of fixed material, the peaks pyramid digest, the mixdown digests of the run cases
- [ ] 2.1 `cpu::with_general` / `cpu::specialized`; `SmoothedParam` rest and `advance`; `Biquad` tune memo
- [ ] 2.2 Filter and EQ: re-tune only on change; distortion's compensation in the memo; gain at rest
- [ ] 2.3 Compressor below threshold (safe level per threshold)
- [ ] 2.4 Mixdown constant-weight runs (plain and effect events)
- [ ] 2.5 Loudness meter (`fast!`, doubled true-peak ring, table once per feed), peaks pyramid (`fast!`)
- [ ] 2.6 Camera microphone resampler: doubled rings
- [ ] 3.1 Tests t2170–t2179; t1980/t1981 green on both paths
- [ ] 3.2 Measure after (the same three), CPU at rest on the heavy showcase
- [ ] 3.3 The class walked and written down (design.md); crate CLAUDE.md, engine-audit row, found-defects row 129

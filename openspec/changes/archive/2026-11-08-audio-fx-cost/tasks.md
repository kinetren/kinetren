# Tasks: audio-fx-cost

- [x] 1.1 Measure every audio kind, defaults and heavy, release: `kt_fx::audio::cost`, `kt-cli fx-cost` (and `--project/--clip`, `--digest`)
- [x] 1.2 His relinked project: `fx-cost --project … --clip 2133`, `probe 4632 4652 --time`, before and after
- [x] 1.3 Print the digests of every kind on db6df6eb's plugins (the bytes to keep)
- [x] 2.1 `kt_plugin_api::cpu`: `fast!` / `run_fast` / `with_baseline`; every audio plugin's `process` through it
- [x] 2.2 VHS Tape's sound: FM sub-samples in lanes, `powf_neg_half`, the deferred linear `tanh`, the dropout cells, the doubled FIR ring, the passport once
- [x] 2.3 `dsp::Memo` in gate, limiter, compressor, distortion, bitcrusher
- [x] 3.1 t1980 (the cost bounds), t1981 (the bytes, both paths), t1982 (`powf_neg_half` on every float)
- [x] 3.2 The class walked and written down (design.md)
- [x] 3.3 kt-plugin-api and kt-fx CLAUDE.md, What's new, the registry row G8

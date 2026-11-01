# Tasks

Tests t1235–t1259 are reserved by this change. Every stage ends green and is committed at once
(owner's rule), then `src/scripts/target-prune.py`.

## 1. The codec model (family crate)

- [ ] 1.1 `src/plugins/fx-video-codec`: planes, colour conversion, motion estimation, DCT/quantiser, closed-loop encoder, GOP schedule
- [ ] 1.2 The breakages and the decoder: drop, bloom, freeze, vector ops, transplant, residual loss, macroblock loss
- [ ] 1.3 Anchor, history (`needs`) and the content-keyed memo entries; the intra refresh at 300
- [ ] 1.4 t1235 round trip, t1236 cut mosh, t1237 bloom, t1238 seek = play, t1239 determinism, t1243 other breakages

## 2. The plugin contract

- [ ] 2.1 `kt_plugin_api::stream`: `StreamKernel`, `StreamLayout`, `StreamInput`, `StreamMemo`; `VideoClass::Stream`; `Plugin::stream()`
- [ ] 2.2 Registry refusals + t1240; kt-fx / kt-gpu treat a `Stream` slot as no step
- [ ] 2.3 The manifest, presets (cut mosh, bloom, transplant, melt), assembler entry, t973/t328 rows, t1247

## 3. The engine

- [ ] 3.1 `layer_src`: the stage in the plan's common code, output fitted and opaque, slot removed, keys carry the output
- [ ] 3.2 The order: `layers_at` names the stream's needs at the codec size; decode qualities; reels and track stages skip the stage
- [ ] 3.3 Export pre-roll; t1241 seek = play, t1242 chunks = one go, t1248 the order

## 4. The editor

- [ ] 4.1 Catalog keys (name, rows, presets) in every language; reach/layout/i18n tests green
- [ ] 4.2 Live scenario t1249 with screenshots on our fixtures

## 5. Close

- [ ] 5.1 Cost measured (release, one build); recorded here
- [ ] 5.2 What's new line; registry D1 closed; F-FX-011 → done
- [ ] 5.3 `cargo test --workspace`, `cargo clippy --all-targets`, `git merge master`, `./scripts/finish.sh` — version recorded
- [ ] 5.4 Archive: specs merged into video-fx and plugins, the why into their designs

# Tasks

Tests t1235–t1259 are reserved by this change. Every stage ends green and is committed at once
(owner's rule), then `src/scripts/target-prune.py`.

## 1. The codec model (family crate)

- [x] 1.1 `src/plugins/fx-video-codec`: planes, colour conversion, motion estimation, DCT/quantiser, closed-loop encoder, GOP schedule
- [x] 1.2 The breakages and the decoder: drop, bloom, freeze, vector ops, transplant, residual loss, macroblock loss
- [x] 1.3 Anchor, history (`needs`) and the content-keyed memo entries; the intra refresh at 300
- [x] 1.4 t1235 round trip, t1236 cut mosh, t1237 bloom, t1238 seek = play, t1239 determinism, t1243 other breakages

## 2. The plugin contract

- [x] 2.1 `kt_plugin_api::stream`: `StreamKernel`, `StreamLayout`, `StreamInput`, `StreamMemo`; `VideoClass::Stream`; `Plugin::stream()`
- [x] 2.2 Registry refusals + t1240; kt-fx / kt-gpu treat a `Stream` slot as no step
- [x] 2.3 The manifest, presets (cut mosh, bloom, transplant, melt), assembler entry, t973/t328 rows, t1247
  - `FxInstance::all()` lists `datamosh`; t328 gives it the identity row (the chain passes the slot
    through) and skips the barrier probe for it; t329 excludes it — there is no GPU text of a CPU
    stream stage to hold to a tolerance, its bytes reach both executors as a pre-FX frame.

## 3. The engine

- [x] 3.1 `layer_src`: the stage in the plan's common code, output fitted and opaque, slot removed, keys carry the output
- [x] 3.2 The order: `layers_at` names the stream's needs at the codec size; decode qualities; reels and track stages skip the stage
- [x] 3.3 Export pre-roll; t1241 seek = play, t1242 chunks = one go, t1248 the order
  - t1242 exports a range whole and as three range exports, each with a fresh renderer, memo and
    cache (one starting inside the mosh) — harder than a chunk boundary, which keeps the memo.
  - Found while writing t1243: two test streams with the same addresses share memo keys — the key
    must carry the SOURCE identity, which the engine's addresses do (media, pts, size, revision).

## 4. The editor

- [x] 4.1 Catalog keys (name, rows, presets) in every language; reach/layout/i18n tests green
  - The numeric rows are in the shared grid; the encoded height, block and vector source are combos
    and the vector swap a checkbox in the FX window. The picker's template is the intact codec, the
    breakages are the presets (t543: a variant must differ from the template). The GOP strip was not
    built (secondary per the owner's p.4): it would need the schedule's text a second time in kt-app.
- [x] 4.2 Live scenario `datamosh.py` with screenshots on our fixtures
  - Green on :287 (:283 gave an X BadMatch on SetInputFocus — a display another run was using). The
    Cut mosh preset got `gop` 60: with 24 the next intact I-frame ended the mosh 0.8 s after the cut.

## 4b. The owner on the first build (2026-11-01): curves, reset on the beat, no caps, a region door

- [x] 4b.1 The rows per position with their curves (`StreamLayout::rows_at`; the engine applies the event's compiled automation at the frame's material position); the gates `mosh`, `reset`, `bloom`, `freeze` replace the frame-number rows; the per-frame quantities are Smooth
- [x] 4b.2 Ranges to the physics: GOP 0–600 (0 = only at the stream start, the cut and the resets; the encoder's keyint 600), search 64, before 300, 1080 lines, vector scale ±16; the decoder's refresh 600
- [x] 4b.3 The region of interest in the contract (`region_ident`, `with_region`), the datamosh's meaning (broken inside, intact outside, per-pixel), the docs (design D9, the capability designs)
- [x] 4b.4 Tests: t1244 (region), t1245 (reset and inactivity by curves), t1246 (engine: a reset key on the event, seek = play), t1238 extended with curve cases, t1247's form re-frozen
  - Found by t1244: the region blend's truncating division put a fully covered pixel one level off
    the broken picture for a negative difference — `div_euclid`.

## 5. Close

- [x] 5.1 Cost measured (release, one build); recorded here
  - `cargo test --release -p fx-video-codec --test codec cost -- --ignored --nocapture`, 640×368
    (360 lines), 16×16 blocks, ±16 search, under a machine load of ~40: encode 7.56 ms/frame, the
    broken decode 0.52 ms/frame. Sequential playback pays one encode + one decode per frame (the memo
    holds the rest); the worst cold seek, through a full 300-frame chain, ≈ 2.4 s.
- [ ] 5.2 What's new line; registry D1 closed; F-FX-011 → done
- [ ] 5.3 `cargo test --workspace`, `cargo clippy --all-targets`, `git merge master`, `./scripts/finish.sh` — version recorded
- [ ] 5.4 Archive: specs merged into video-fx and plugins, the why into their designs

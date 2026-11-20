# Tasks: hdr-sources-to-sdr

> Owner Q1–Q3 first. Tests reserved: t3800–t3811. Base: orch `3420bf58`.

## 0. Notes first

- [x] 0.1 feature note F-CLR-009; proposal, design (D1–D7, class walk D6), deltas `media-index`, `export`; `openspec validate hdr-sources-to-sdr --strict`

## 1. Tags

- [ ] 1.1 `ColourTags.transfer`, the mastering peak and MaxCLL from the stream's side data on every major; t3800
- [ ] 1.2 the path resolve (SDR / WIDE / HDR); t3801

## 2. The pipeline on the processor

- [ ] 2.1 the tables from `kt_plugin_api::math` (EOTF PQ/HLG, tone map, BT.709 encode); the Q15/Q14 matrices
- [ ] 2.2 swscale to `yuv444p16` at the decode size for WIDE/HDR pictures; the row-parallel integer kernel to RGBA8; the context key carries the path
- [ ] 2.3 PQ, HLG, BT.2020-SDR and P3 fixtures in `fixtures/gen.sh` (tags set by the encoder's options)
- [ ] 2.4 t3802–t3806; stored frames looked at by eye before they are stored (with 4.1)
- [ ] 2.5 the thumbs' and strips' pixel versions moved
- [ ] 2.6 the cost of a 4K HDR frame measured on release against the decoder's own time (`kt-cli probe … --time`), written in design D7

## 3. The device twin

- [ ] 3.1 when device-decode-sources lands: the WGSL pass with the tables as buffers; t3807

## 4. Export and facts

- [ ] 4.1 the tone map's anchors checked by eye on the fixtures against a player that tone-maps (mpv `--tone-mapping=bt.2390`), recorded in design D2
- [ ] 4.2 the export default by size, `matrix=auto`; t3808, t3809; found-defects 1053's comment in `format.rs` fixed
- [ ] 4.3 `MediaInfo` facts, `kt-cli probe`, the trace's decode record; t3810, t3811
- [ ] 4.4 found-defects 724 and 1054 closed; What's new line; `cargo test` of the touched crates, clippy, target-prune; commit per stage

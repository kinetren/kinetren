# Tasks: hdr-sources-to-sdr

> The owner decided (kkt-26.11.20): the 8-bit path (stages 0–4) as designed, the export default BT.709 from HD (Q50), and
> a 16-bit path chosen per project (A25, stage 5) his answers to the revised Q1–Q4 are in (kkt-26.11.20b B4). Tests reserved:
> t3800–t3811, t3877–t3899. Base: orch `3420bf58`, revised on `5e7a8ea9`.

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
- [x] 4.2 the export default by size, `matrix=auto`: built outside this change by the mosh-sharp executor
      (found-defects 1054, t3603 in `export/spec.md`); t3808 and t3809 are not needed and stay free
- [ ] 4.3 `MediaInfo` facts, `kt-cli probe`, the trace's decode record; t3810, t3811
- [ ] 4.4 found-defects 724 closed; What's new line; `cargo test` of the touched crates, clippy, target-prune; commit per stage

## 5. The 16-bit path (kkt-26.11.20 A25; the owner's Q1 a, Q2 a, Q3 b, Q4 a in kkt-26.11.20b B4)

- [ ] 5.1 `Precision` on `ProjectSettings` and `Sequence`; the next free project file format by content; OTIO
      `metadata["kt"]` and a row in "What OTIO export loses"; t3877–t3880
- [ ] 5.2 the decode to linear BT.2020 f16 (D10), `VideoFrame.format`, the cache key's precision;
      t3881–t3883
- [ ] 5.3 kt-gpu: the pool by format, `tex_bytes` by format, the core float shaders, `FrameTol` float
      classes; t3884–t3887
- [ ] 5.4 `Manifest.precision`; the island and the chain's mark; float kernels of Geometry, CamMotion,
      LayerMix, the crossfade, the title and the colour family; t3888–t3891
- [ ] 5.5 the display: the surface chosen from wgpu's formats and colour spaces at start and on a monitor
      change (scRGB/EDR/HDR10 where offered), the linear write with the monitor-peak roll-off, the tone map on
      SDR, the facts naming which; t3892
- [ ] 5.6 export SDR / HDR10 / HLG, tags, mastering display and MaxCLL; t3893–t3895
- [ ] 5.7 the reels' boundary rule; the prerender's Main10 segments or the reason; t3896, t3897
- [ ] 5.8 Project settings and composition settings: the precision; i18n; `reach_tests`, `layout_tests`;
      t3898; planned live scenario `hdr_project` with a screenshot per step
- [ ] 5.9 the idle check on the heavy showcase switched to 16-bit (pause, mouse over a static place, an
      effects window open); t3899; What's new line; F-CLR-008 and F-CLR-009 verdicts; commit per stage

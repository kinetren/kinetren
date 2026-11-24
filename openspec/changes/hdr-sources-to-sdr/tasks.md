# Tasks: hdr-sources-to-sdr

> **Sections 1–4 (the 8-bit path) are IMPLEMENTATION-READY** (revision 3, 2026-11-24, orch `229c694b`). Section 5 (16-bit, kkt-26.11.20 A25) is a stage of
> its own, re-read when its turn comes. Tests: t3800–t3807, t3810, t3811, t4142, t4143 (8-bit); t3877–t3899 (16-bit). Areas of 1–4: DECODE (`colour.rs`, new `hdr.rs`, `source_impl.rs`, `kt-media/ffmpeg.rs`), one
> constant in `render_cache.rs`, `copy.rs`'s name. Needs `persistent-cache` merged (`ffmpeg.rs`, `copy.rs`); NOT `device-decode-sources` (t3807 runs where it is in; whichever comes second adds it). Not beside
> `device-decode-sources` 1–2 (`source_impl.rs`, `ffmpeg.rs`). Read: design D1–D7, kt-ffmpeg `CLAUDE.md`. Commit per section; target-prune after each.

## 0. Notes first

- [x] 0.1 F-CLR-009; proposal, design, deltas; `openspec validate hdr-sources-to-sdr --strict`

## 1. Tags

- [ ] 1.1 `ColourTags.transfer` in `or`, the stream's and the picture's readers; `resolve_path` (D1); t3801; t2890, t2891 gain the transfer's numbers
- [ ] 1.2 `av_frame_get_side_data`, `mastering_display_metadata.h`; the PQ stream's first picture at open, `PrebuiltIndex.hdr_peak` (D2); t3800
- [ ] 1.3 fixtures in `fixtures/gen.sh`, 10-bit libx265, cut to 0.4 s: PQ patches (1, 10, 26, 50, 100, 203, 400, 1000 cd/m²) with `master-display`/`max-cll`, the same without, PQ with MaxCLL 4000 in its second keyframe only, HLG patches (0.38, 0.5,
      0.75, 1.0), a PQ ramp 0–1000, BT.2020 SDR and P3 SDR saturated patches with neutrals; tags by the encoder's options; commit

## 2. The pipeline

- [ ] 2.1 `hdr.rs` (D3): tables, matrices, the kernel; kt-ffmpeg depends on kt-plugin-api; t4142 (constants derived, neutrals neutral, no value out of range over random pixels)
- [ ] 2.2 `Rgba::convert` takes it; `SwsKey` gains the target; `RGBA_TAIL_PAD`'s rule for the 16-bit target (t1450's guard runs on it)
- [ ] 2.3 t3802–t3806; the stored frames looked at by the executor as pictures, and beside `mpv --tone-mapping=bt.2390 --target-peak=203` of the same fixture where mpv is installed, the pictures into the report
- [ ] 2.4 `cargo test -p kt-ffmpeg -p kt-media`; clippy; commit
- [ ] 2.5 the cost on release (D7), written into D7

## 3. Names and facts

- [ ] 3.1 `file_ident` / `identity()`, `Made.pixels`, the two versions, the copy's name (D5); t4143; t3807 if device-decode-sources is in
- [ ] 3.2 `MediaSource::colour`, `kt-cli probe`, the trace's record, the bin's facts line (i18n ×5); t3810, t3811; commit

## 4. Close

- [ ] 4.1 found-defects 724 closed; What's new ("HDR footage from a phone is no longer grey: its highlights and colours are brought into the picture's range"; "a clip you corrected by hand, and a key
      or a wand colour picked from one, looks different now: pick again"); registry A25's 8-bit half; the owner's check (a phone's HDR clip beside its SDR clip, his eye) to the orchestrator
- [ ] 4.2 `cargo test` of the touched crates, clippy, target-prune; commit

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

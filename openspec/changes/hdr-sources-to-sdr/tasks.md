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

> IMPLEMENTATION-READY (revision 4, 2026-11-25, orch `fc60a5d9`): D8–D15. After section 4, `shared-events`, `device-decode-sources`, `export-into-surface`, `track-stream-stage`, `scopes`, `chroma-key-upgrade` (all write what this doubles). **v+**. Six sessions, a merge after each; t3877–t3899, t4155, t4156.

- [ ] 5.1 `Quality::new` and `VideoFrame::rgba8` (two mechanical commits, no byte moves); `Precision`, the format, OTIO; `Quality.deep`, `FramePlan.deep`, the leaves' form; t3877–t3880, t3883
- [ ] 5.2 `hdr.rs` `Out::Deep` (D10), `VideoFrame.form`, `blend_frames`' twin; t3881, t3882
- [ ] 5.3 kt-gpu: forms in the pool and every count, the deep twins, `FrameTol::DEEP`, transitions through the encoded form, boundaries, generators; GOLDEN_VIDEO and every 8-bit test untouched; t3884–t3887, t3896
- [ ] 5.4 `Decl.extended`, `PlanOp::Island`, `to_sdr`/`from_sdr`, streams, the chain's mark; t3888–t3891
- [ ] 5.5a the tone-mapped display, Project settings and composition settings, i18n ×5, reach and layout tests, live `hdr_project`; the idle check on the heavy showcase switched to 16-bit; t3892 (SDR half), t3898, t3899
- [ ] 5.5b egui-wgpu vendored with its three changes and guard; the HDR surface, the callback, the facts; t3892, t4155; the C row's text (which Wayland compositor, Windows) to the orchestrator
- [ ] 5.6 export SDR / HDR10 / HLG, the light levels, the prerender's Main10 segments; t3893–t3895, t3897, t4156; What's new; F-CLR-008 and F-CLR-009 verdicts; registry A25, B4; commit per session

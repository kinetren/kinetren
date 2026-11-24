# Tasks: device-decode-sources

> IMPLEMENTATION-READY (2026-11-22). The owner answered (kkt-26.11.20 A12): on by default, a checkbox in
> Settings. Tests: t3300–t3311, t4100–t4102. Code references re-checked
> against orch `286c406c` (2026-11-24); D2, D4, D6 revised that day. Read first:
> `design.md` D1–D4, D6, D11; `src/libs/kt-ffmpeg/CLAUDE.md`, `src/libs/kt-media/CLAUDE.md`;
> `openspec show media-index --type spec`. Commit per section; `scripts/target-prune.py` after each.
> Build-queue areas: DECODE, `kt-time` mem/trace, SHELL (Preferences). Do not run beside another change
> writing `source_impl.rs`, `mem.rs` or `trace.rs`. **Section 1 and 2.1 may start beside anything; 2.2 on not
> beside `persistent-cache`** (`ffmpeg.rs`, `worker.rs`, `copy.rs`; `durable-moment-key` is in); 3 after own-data-dir
> (`trace_query.rs`, kt-cli `main.rs`). Not beside `export-into-surface` section 2 (`ktfake_drv_video.c`, `mem.rs`).

## 0. Notes first

- [x] 0.1 F-MED-010 (`docs/features/media/sources/hardware-decoding.md`)
- [x] 0.2 proposal, design (D1–D11, class walk D9), deltas `media-index`, `export`, `telemetry`, `ui-reach`
- [x] 0.3 the owner's answers folded in (Q1 a, Q2 b)

## 1. kt-ffmpeg: the layout, one device, eligibility

What exists (keep it): `Source::open_hw` (`lib.rs:1080` → `source_impl.rs:726`), `VideoDecode` (`lib.rs:552`),
`decode_device_verdict` (`lib.rs:563`, the per-node trial memo), `VideoPath`/`HwDecode` (`source_impl.rs:143`,
`:185`), `pick_hw_format` (`:195`), `open_decode_device` (`:217`), `open_decoder` (`:1022`), `use_hw` (`:1073`),
`download` (`:1091`, with its fallback detection), `seek_and_reopen` (`:1126`, keeps the device), `decode_next_frame` (`:1162`), `to_rgba` (`:1362`). `source_impl.rs` is `include!`d once per FFmpeg major
(`lib.rs:295`): never put tests in it.

- [ ] 1.1 D1, the exact layout: after `download`, copy NV12 → yuv420p/yuvj420p and P010 → yuv420p10le into
      the processor decoder's format of the stream (the tag decides yuvj), keep pts and colour tags, never
      read line padding; then the one `to_rgba`. t3301 in a new `src/libs/kt-ffmpeg/tests/device_layout.rs`
      on synthetic frames
- [ ] 1.2 D3, one device per process: `open_decode_device` opens once (a process-wide `OnceLock` per API,
      shaped like `decode_device_verdict`), each decoder takes an `av_buffer_ref`; `open_hw` gains `file_key`; Drop unrefs its own reference only, the
      process's reference is never released (`hw_device_opened`)
- [ ] 1.3 D2, eligibility: the codec table (H.264, HEVC, VP9, AV1), 4:2:0 8/10-bit offered by `get_format`,
      no odd crop the copy cannot keep; the first-keyframe proof (device picture == processor picture of the
      same packet), memoized per (`file_key`, device); t3302 in `device_layout.rs`
- [ ] 1.4 D6, a device fault mid-stream reopens that decoder on the processor at the same keyframe and marks
      the file processor-only for the process
- [ ] 1.5 the bit: `set_decode_device(bool)` beside `set_decode_threads` (`lib.rs:23`), the third field of `DecodeSettings`
      (`lib.rs:52`), read in `use_hw`;
      `KT_DECODE_DEVICE=off|on` read once at start and winning over the setter
- [ ] 1.6 `src/fixtures/fake_va/ktfake_drv_video.c`: a mutex around the driver's state, a display counter,
      two new modes in the existing `KT_FAKE_VA` convention (`:157`, list `:18`): `perturb` (one sample per
      picture, optionally after the first keyframe) and `KT_FAKE_VA_FAIL_AT=N`; t3303, t3304, t3306
- [ ] 1.7 `cargo test -p kt-ffmpeg`; clippy; commit "device-decode-sources 1: exact layout, one device,
      eligibility"

## 2. kt-media, kt-time, kt-engine: who opens on the device

- [ ] 2.1 `kt-time/src/mem.rs`: append `Tenant::Surfaces = 8` (the file's rule: a new kind is appended),
      `KINDS = 9`, add it to `ALL` and `name`, NOT to `GROWING` (D4); `surfaces_bound(available)`; `mem.rs`'s own
      tests: a live claim leaves the bound as it is (part of t3305)
- [ ] 2.2 `FfmpegSource` (`kt-media/src/ffmpeg.rs:11`): its `hw: bool` field becomes the kind chosen per decoder by eligibility, the bit and the `Surfaces` bound; it
      keeps the `DecodeSettings` it was opened with and `cursor_for` (`:149`, the open at `:167`) opens its cursors under
      them (found-defects 1064); released on drop; t3305
- [ ] 2.3 D6: `factory::open_source(path, store, Purpose)` in `kt-media/src/factory.rs` (beside `open_indexed`, `:103`); route
      the openers through it: `kt-app/src/util.rs:36`, `render_dialog.rs` `open_all` (`:616`, today
      `FfmpegSource::open` with no index), kt-cli export (`main.rs:2056`), `open_sources` (`:1344`), `preview` (`:1130`), `pause-scan` (`:1007`);
      `copy.rs`'s `copy` and `make` (persistent-cache, once merged); thumbnails, filmstrip and stills stay software; t4102, t3309, t3310
- [ ] 2.4 t3129 (`kt-media/tests/segment.rs:105`): "within swscale's rounding" becomes "equal"; edit its
      scenario in `openspec/specs/video-plan/spec.md`; t3300
- [ ] 2.5 the export's stage profile: pictures per kind and the transfer time in `ExportStage`
      (`export_pipeline.rs:132–171`, `decode_chunk` `:1226`); t3307 (`kt-engine/tests/export_options.rs`)
- [ ] 2.6 `cargo test -p kt-media -p kt-time -p kt-engine`; clippy; commit "device-decode-sources 2: sources
      open through one factory call and decode on the card"

## 3. Trace, Preferences, Diagnostics

- [ ] 3.1 `Ev::Decoded` (`kt-time/src/trace.rs:78`, emitted at `kt-engine/src/worker.rs:313`) gets the kind
      as aux; an open/refusal record with the reason; transfer per source
- [ ] 3.2 `kt-cli trace summary` (`trace_query.rs:503`) prints the decode line; t3308 (`trace_query/tests.rs`)
- [ ] 3.3 `kt-cli probe … --decode-check` (`main.rs:1795`, beside the arms v2d 2.7 prints); t3311 (`kt-cli/tests/cli.rs`)
- [ ] 3.4 D11: `StartChoice.decode_on_card` (`user_store.rs`, beside `economy_ui` `:681`), its setter in t1294's
      writer list (`welcome_prefs_tests.rs:350`), the checkbox in `prefs_window.rs`'s FFmpeg section (`:556`, after the Economy pattern `:494–500`), the bit set at start and on toggle, the player's sources reopened
      on toggle; the Diagnostics line in `draw_diagnostics` (`app.rs:11244`); i18n in every catalog;
      `reach_tests`, `layout_tests`, font tests; t4100, t4101
- [ ] 3.5 kt-media and kt-ffmpeg `CLAUDE.md`: "a new device format gets its exact layout or stays on the
      processor"; What's new line ("video decodes on the card where it can, with the same picture; a
      checkbox in Preferences turns it off"); found-defects 775's follow-up closed
- [ ] 3.6 the idle check on the heavy showcase (pause, the mouse over a static place, an effects window
      open): nothing new at rest; commit; the report's "Найдено, не исправлено"

# Tasks

Test numbers t2800–t2829 are reserved by this change. Every stage ends with a commit (English
message, the first line = the stage and what was done) and `scripts/target-prune.py`.

## 0. Records before code

- [x] 0.1 The reconnaissance note F-EXP-006 (`docs/features/export/presets/render-settings.md`) and
  `python3 src/scripts/features-index.py --check` green; this change's artifacts;
  `openspec validate render-settings --strict` green

## 1. The format reaches the file (kt-ffmpeg, kt-engine)

- [ ] 1.1 `kt-ffmpeg/src/format.rs`: `OutputFormat` and its enums, the container table (D1, D2)
- [ ] 1.2 `encoders.rs`: per-codec cascades, 10-bit input, rate modes, speed and rate options (D3–D5); t2801
- [ ] 1.3 `muxer_impl.rs`: container, codec, rate, GOP, B-frames, depth, colour conversion and tags,
  audio codecs and sample formats, metadata, `hvc1` (D6–D8); the output probe reader
- [ ] 1.4 `ExportOptions.format`, `.fps` (D1); `export_settings.rs` (D9); t2800
- [ ] 1.5 `tests/render_settings.rs`: t2802, t2803, t2804, t2805, t2806; the existing export tests green

## 2. kt-cli

- [ ] 2.1 `export` takes `--set` and the short flags (D9); t2808; usage text

## 3. The window and the presets (kt-app)

- [ ] 3.1 `RenderPresetStore` in `user_store.rs` (factory, user, last); t2809
- [ ] 3.2 The window (D10), the availability, the estimate, the range, the extension; translation
  catalogs; t2810, t2811, t2813; `reach_tests`, `layout_tests`, `font_tests`, `i18n_tests` green
- [ ] 3.3 Live scenario `render_settings` with a screenshot per step; `./scripts/live-check.sh render_settings`

## 4. Close

- [ ] 4.1 What's new line; found-defects row for the decode-side BT.601 default; clippy; idle measure
- [ ] 4.2 Archive the change (specs merged), commit

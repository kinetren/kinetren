# Tasks

## 1. The contract: sound hook and one sine (plugins)

- [ ] 1.1 `kt_plugin_api::generator`: `Synth`, `GenKernel::synth`, `Kind::sound(..)` (blank frame, `blank: true`), `sine(n, hz)`, `db_amp(db)` through `FrozenMath` (t1066)
- [ ] 1.2 `Registry::register` refuses a kind with both a synth and a tone, by id (t1162, `src/libs/kt-plugin-api/tests/registry.rs`)
- [ ] 1.3 `kt-media` `tone_1k` calls `sine`/`db_amp` (bytes unchanged: t1048, t530, t533); `info_for` (`has_video = synth.is_none()`, `has_audio`), `read_audio_burst` fills a synth burst from the curve-resolved values at the burst's tick (t1156, `src/libs/kt-media/tests/gen_sound.rs`)

## 2. The family `generators-audio`

- [ ] 2.1 New leaf crate `src/plugins/generators-audio` (workspace member, depends on `kt-plugin-api` only — t975): `tone`, `audio_noise`, `sweep`, `beep`, `silence` per design D5–D10
- [ ] 2.2 `kt-plugins-builtin`: family after `generators_test`, ids appended to `BUILTIN_IDS`; t973 menu count 19 → 24 with the first 19 unchanged
- [ ] 2.3 Kernel tests `src/plugins/generators-audio/tests/sound.rs`: t1155 (tone = SMPTE bytes), t1157 (level, frequency, channels), t1158 (noise colour slopes, RMS, seed, stereo), t1159 (sweep curve, phase continuity, repeat), t1160 (beep = gated tone, silence)
- [ ] 2.4 Render test t1167 (`src/libs/kt-engine/tests/gen_sound_render.rs`): tone + beep events rendered offline, level and timing, two renders byte-identical

## 3. The UI: placement and the Audio group (ui-reach, generators)

- [ ] 3.1 `gen_ui`: `GenGroup::Audio` (`genkind.group_audio`, probe `audio`, last), a stream filter in the one menu body; catalogs (en, ru, es, zh-Hans, ar) for the group, five kind names and the param labels; font subsets if needed (t100)
- [ ] 3.2 `toolbar.rs`: the item leaves File, enters Edit; `app.rs`: `create_generator_at(kind, at, lane)`; `timeline.rs`: the clip track menu offers the filtered list and creates at the clicked track and `ctx_at`
- [ ] 3.3 `lessons::GENERATOR_MENU` → Edit; lesson 1 text/outline/"Show" follow (t1087 and live `lessons`)
- [ ] 3.4 t560 rewritten (owner's decision); new `src/apps/kt-app/src/gen_audio_tests.rs`: t1163 (24 rows, Audio group, names and labels), t1164 (Edit at the cursor, audio clip only), t1165 (track menu at the clicked place, filtered lists), t1166 (sound kind card named)
- [ ] 3.5 Live scenario `src/scripts/live/genaudio.py`: Edit → create; right-click on a track → create at that spot; play an audio generator with meters moving; a screenshot per step

## 4. Docs and close

- [ ] 4.1 `docs/guide/whats-new.md` (`## kt26.11`) line; the K4 row of `docs/ktprompts/kkt-26.11.01-index.md` → done (`ktprompts-index.py --check`)
- [ ] 4.2 The F-AUD-014 note → done with `CAPABILITY:`; the "why" into `openspec/specs/generators/design.md` (Revision of R.55's File entry; D3/D4/D7)
- [ ] 4.3 `git merge master`, `cargo test --workspace`, `cargo clippy --all-targets`, `src/scripts/finish.sh` — the version recorded here

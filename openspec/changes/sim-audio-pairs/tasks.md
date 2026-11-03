# Tasks: sim-audio-pairs

## 0. Notes first

- [x] 0.1 F-FX-012 (VHS sound), F-FX-013 (the broken stream's sound), F-FX-014 (the camera's
      microphone) in `docs/features/fx/simulate/`; `features-index.py --check` green

## 1. The mechanism and the VHS pair (stage a)

- [x] 1.1 kt-plugin-api: `Decl.pair`, `Placement` + `AudioState::place`, the registry's pair
      refusals (`pair_fits`), `counterpart`, `unpaired`; the assembler asserts no pair is missing
- [x] 1.2 kt-model: `sync_pairs_to_link_group` beside the warp's; `MorphPatch::derived_pairs` in
      `chain_for`/`auto_for`
- [x] 1.3 kt-fx `ChainOpts.fps`, `ClipProcessor::set_origin`; kt-engine `sync_clip` passes the start
- [x] 1.4 `vhs_tape_sound` (`fx-video-tape/src/sound.rs`), the deck's sound rows; `Decl.foreign_rows`
      (the rows a half does not read: t322 skips them on the sound, the format reads them on the picture)
- [x] 1.5 tests: t1460 (registry), t1461/t1461b (model mirroring, morpher), t1462–t1466 (physics),
      t1467 (engine: placement, bypass identity, determinism), t1485 (format 4 / OTIO v16 by content);
      t973c row printed, t322 enablers
- [x] 1.6 kt-app: the audio kind in the picker (no variants: R.54), the deck face's sound section on
      both halves, labels, catalogues ×5, font subsets; t1468; live scenario `vhs_sound` (t1469) — it
      found that the off-screen half's face published the probe names (fixed: only a face on screen does)
- [x] 1.7 specs, What's new line, README/guide mention; commit; `target-prune.py`

## 2. The datamosh pair (stage b)

- [x] 2.1 the codec in kt-plugin-api (`audio_codec`): MDCT 2048/1024 sine window, per-line
      prediction with resets, Rice-coded lines, the bit reservoir, a decoder with concealment
- [x] 2.2 `datamosh_sound`: the schedule on the picture's frame grid, the gates' damage; the panel's
      audio bitrate and prediction, catalogues ×5
- [x] 2.3 tests t1470–t1476; specs; commit; prune

## 3. The camera pair (stage c)

- [ ] 3.1 the microphone rows in `camera_isp`'s passport, the looks set them
- [ ] 3.2 `camera_isp_sound`: wind, port, capsule, AGC, NS, the codec
- [ ] 3.3 the ISP panel's sound blocks; tests t1477–t1484; specs; commit; prune

## 4. Close

- [ ] 4.1 notes frozen as done; registry D1 closed; What's new; README mention
- [ ] 4.2 `cargo test --workspace`, clippy, `./scripts/finish.sh` — version recorded here
- [ ] 4.3 archive the change

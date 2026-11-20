# Tasks: device-decode-sources

> DESIGN ONLY until the owner answers Q1–Q2 (`proposal.md`) and the orchestrator gives the go. Tests
> reserved: t3300–t3311 (of the spec session's t3300–t3399). Base: orch `ec30cb02`.

## 0. Notes first

- [x] 0.1 F-MED-010 (`docs/features/media/sources/hardware-decoding.md`); `features-index.py --check` green
- [x] 0.2 proposal, design (D1–D10, class walk D9), deltas `media-index`, `export`, `telemetry`;
      `openspec validate device-decode-sources --strict`
- [ ] 0.3 his answers Q1–Q2 in `docs/user-feedback.md` and `docs/owner-checks.md` (orchestrator), folded in

## 1. kt-ffmpeg — layout, device, eligibility

- [ ] 1.1 the exact layout after `download` into the processor decoder's format of the stream (D1); t3301
- [ ] 1.2 one decode device per process, a reference per decoder (D3); `open_hw` keeps its signature
- [ ] 1.3 the eligibility table (codec, flavour, crop) and the first-keyframe proof, memo per (path, size,
      mtime, device) (D2); t3302
- [ ] 1.4 a device fault reopens on the processor at the keyframe, the file processor-only (D6)
- [ ] 1.5 `fixtures/fake_va`: a mutex around the driver state, a display counter, `KTFAKE_PERTURB`
      (change one sample per picture, optionally after the first keyframe), `KTFAKE_FAIL_AT=N`; t3303,
      t3304, t3306

## 2. kt-media, kt-time, kt-engine — who opens on the device

- [ ] 2.1 kt-time `mem.rs`: tenant `Surfaces` (base 0, order `[Mix, Kept, History, Surfaces, Frames]`) (D4)
- [ ] 2.2 `FfmpegSource`: the kind per decoder by eligibility and the ledger, kept across reopen,
      released on drop; `KT_DECODE_DEVICE=off|on` (default per Q1); t3305
- [ ] 2.3 the factory's preview and export opens use it; thumbs/filmstrip/stills stay software; t3309, t3310
- [ ] 2.4 t3129: "within swscale's rounding" becomes "equal" (edit its scenario in `video-plan/spec.md`);
      t3300
- [ ] 2.5 export: stage profile with pictures per kind and transfer; t3307
- [ ] 2.6 `cargo test` of kt-ffmpeg, kt-media, kt-engine, kt-time; clippy; target-prune; commit

## 3. Trace and report

- [ ] 3.1 `Ev::Decoded` aux = kind; open/refusal record; transfer per source (D7)
- [ ] 3.2 `kt-cli trace summary` decode line; t3308
- [ ] 3.3 `kt-cli probe … --decode-check`; t3311
- [ ] 3.4 Diagnostics line per Q2 (and a Settings checkbox only on Q2 b: `reach_tests`, i18n, font tests)
- [ ] 3.5 kt-media/kt-ffmpeg `CLAUDE.md`: the layout rule ("a new device format gets its exact layout or
      stays on the processor"); `docs/guide/whats-new.md` line; found-defects 775's follow-up closed
- [ ] 3.6 idle-load check (paused, mouse over a static place, effects window open) on the heavy showcase:
      no change expected at rest (decoders are not touched at rest)

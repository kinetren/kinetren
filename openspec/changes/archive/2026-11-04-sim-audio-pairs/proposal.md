# Proposal: sim-audio-pairs

## Why

The owner, kkt-26.11.03b (registry row D1, `docs/ktprompts/kkt-26.11.03b.txt`): "запрос: парные
эффекты к симуляционным. вот это опять агенты не работали и не подумали что к тому же vhs tape должен
быть парный audio-фильтр, к datamosh тоже" (paired effects for the simulation ones; VHS Tape must
have a paired audio filter, Datamosh too). The root `CLAUDE.md` rule written with the request: a
simulated device that records picture AND sound simulates both; the pair is driven by the same device
state, so a defect is seen and heard at the same moment.

The three simulation effects (`vhs_tape` F-FX-009, `datamosh` F-FX-011, `camera_isp` F-FX-010) model
devices that record sound: a VHS deck, a muxed codec stream, a phone/webcam/security camera. None has
an audio half. The reconnaissance notes F-FX-012 (VHS sound), F-FX-013 (the broken stream's sound) and
F-FX-014 (the camera's microphone) lay out the physical chains.

## What Changes

- **A pairing mechanism (general, not three special cases).** A video kind declares its audio pair
  (`Decl.pair`); the two share ONE passport (the registry refuses otherwise). The model mirrors the
  pair across an A/V link group like the warp: adding the video effect to an event with linked audio
  puts the pair on that audio (visible in its chain, switchable off by the slot's checkbox); values,
  curves, presence follow; bypass and masks stay each side's own. A morpher patch hands the other
  half to the other kind's events. The audio state learns where its clip sits on the timeline and the
  project frame rate (`AudioState::place`), so it derives the device state at the timeline position
  the picture reads — no state is shared across threads (invariant 2).
- **`vhs_tape_sound`** — the deck's sound: the linear track (stationary head, bias, gap/spacing/coating
  losses at the tape speed, hiss by track width, dropouts of the oxide passing the head, capstan
  flutter from the picture's time base, Dolby B) and Hi-Fi FM (complex baseband FM at 8×, carriers
  read by the head that plays the picture's line, the picture's own dropout field, FM threshold
  clicks, switching-pulse and dropout holds, 2:1 compander). New deck rows: audio output, linear
  tracks, Dolby, the audio head's gap and spacing, bias, record level. The deck face gains a sound
  section.
- **`datamosh_sound`** — a small own codec (MDCT, AAC-Main-like per-line prediction with resets on the
  picture's I-frame grid, Rice-coded coefficients, a bit reservoir) damaged by the datamosh rows on
  the picture's frame positions: dropped I-frames lose the predictor reset, bloom repeats a packet,
  freeze holds the reference, loss flips bits; concealment in the decoder.
- **`camera_isp_sound`** — the camera's microphone chain: wind at the port, the Helmholtz port, the
  capsule (sensitivity, self-noise, overload), AGC, noise suppression, the device's codec (AAC-like,
  16 kHz USB, G.711 µ-law 8 kHz); the looks set it.
- Specs (plugins, audio-fx, video-fx), the deck/ISP/datamosh panels, catalogues ×5, What's new, the
  README/guide mention, live scenario.

## Capabilities

### Modified Capabilities
- `plugins`: a declared pair; the registry's pair refusals; the audio state's placement.
- `audio-fx`: the three sounds and the pair's mirroring in the model.
- `video-fx`: the three simulations name their pairs; the deck's sound rows do not touch the picture.

## Impact

- `kt-plugin-api` (Decl, Placement, registry), `kt-model` (link-group mirroring, morpher patch),
  `kt-fx` (ChainOpts.fps, ClipProcessor::set_origin), `kt-engine` (FxStateStore::sync_clip takes the
  clip's start), the three family crates, `kt-plugins-builtin`, kt-app panels and catalogues.
- Project format: unchanged — new plugin ids and rows written only off their default; an older build
  reads the audio half as a missing plugin and keeps it.
- OTIO: the pair slots travel as any plugin slot in `metadata["kt"]`; nothing is lost beyond what
  plugin slots already lose.

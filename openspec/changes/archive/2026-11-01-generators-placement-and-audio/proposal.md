# Proposal

## Why

The owner (kkt-26.11.01 item 4, registry row K4): "why on earth is the generator in File… why not
in Edit? why not in a track's right-click menu? And where are the sound generators, if the program
has them?" — and his answer p.2: "WHY THE HELL LEAVE IT IN FILE?! … of course the audio ones are
needed — you hide some generators in the tests — pull them out". Today "Create a generator" sits in
File (`toolbar.rs`) and in the bin's background menu; the only sounds a person can make are the
1 kHz tone inside SMPTE bars and the beep inside the countdown leader (`generators-test`).

## What Changes

- "Create a generator" LEAVES the File menu entirely. It appears in the Edit menu (created at the
  playhead, as before) and in a clip track's right-click menu (header and body), where the generator
  is created AT THE CLICKED PLACE — that track and the clicked time. The bin keeps its item. All
  three entries keep ONE list of kinds (rule 17); the track menu lists the kinds that land on that
  track's kind (picture kinds on a video track, sound kinds on an audio track).
- The lessons' one path constant `lessons::GENERATOR_MENU` moves to Edit, so lesson 1's text, its
  outline and "Show" follow the item.
- Sound generator kinds become real plugins in a new family crate `src/plugins/generators-audio`:
  `tone` (frequency, level dBFS, channels), `audio_noise` (white/pink/brown, level RMS dBFS, seed,
  stereo), `sweep` (start/end Hz, duration, linear/log, level), `beep` (frequency, level, period,
  length — the leader's countdown pip as a reusable kind) and `silence`. A sound kind has NO
  picture: it lands on an audio track only.
- The generator contract (`kt_plugin_api::generator`) gains a sound hook: a kind may fill a burst
  itself from the global sample number (`Synth`), and the one sine formula moves into the API
  (`generator::sine`) so the host's `tone_1k` (SMPTE, leader) and the new `tone`/`beep` kinds share
  ONE text — a 1 kHz `tone` at −20 dBFS equals SMPTE's tone byte for byte.
- The creation menu gets an "Audio" group (`genkind.group_audio`).

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `generators`: the creation entries (Edit, track right-click at the clicked place, bin — not
  File); sound kinds without a picture, their determinism, level and seek rules; the menu row
  count grows from 19 to 24.
- `plugins`: the generator kernel's sound hook (`Synth`) and the shared sine; the closed built-in
  set grows by the `generators-audio` family.
- `ui-reach`: the paired entries of "create a generator" (Edit / track / bin) are one body with one
  list; the track entry creates at the clicked track and time.

## Impact

- Code: `src/libs/kt-plugin-api/src/generator.rs` (hook, `sine`), `src/libs/kt-media/src/generator.rs`
  (`info_for`, `read_audio_burst`), new `src/plugins/generators-audio`, `src/libs/kt-plugins-builtin`
  (ids, families), `src/Cargo.toml` (member), `src/apps/kt-app/src/{toolbar.rs,timeline.rs,gen_ui.rs,
  lessons.rs,app.rs}`, catalogs `i18n/*.toml`.
- Tests: t560 rewritten (owner's decision), t973 menu count, new t1155–t1169.
- Project format: untouched — a new kind is a new plugin id; old files never name it. The `.otio`
  export writes a sound generator as it writes every generator (`GeneratorReference.1` with the
  values in its parameters); no new OTIO loss.
- Docs: `docs/features/audio/generators/tone-noise-sweep-beep.md` (F-AUD-014),
  `docs/guide/whats-new.md`, the K4 row of `docs/ktprompts/kkt-26.11.01-index.md`.

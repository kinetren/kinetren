# Design

## Context

- "Create a generator" lives in File (`src/apps/kt-app/src/toolbar.rs`, `menu_items`) and in the
  bin's background menu (`bin_panel.rs`), both calling the one body `gen_ui::generator_menu`
  (rule 17, t560). R.55 put it in File because the bin entry alone was not found; the owner now
  says File is the wrong place (kkt-26.11.01 item 4, answer p.2).
- A generator kind is a plugin (`kt_plugin_api::generator::Kind`, D13 of plugin-api-stage1). Its
  sound is a level (`tone`) and a gate (`gate`) played by the host's one sine `tone_1k`
  (`src/libs/kt-media/src/generator.rs`); `GeneratorSource::info_for` reports `has_video: true` for
  every kind and `has_audio` only when the tone is on. The SMPTE tone and the leader's beep are the
  only sounds (`src/plugins/generators-test`).
- An audio event of a generator already reads its source by the event address
  (`clip_source_addr_in`, `src/libs/kt-engine/src/audio.rs`); `KtApp::clip_cmds_for` creates a clip
  per present stream, so a kind with `has_video: false` gets an audio clip only.
- A right-click on a clip track (header or body) is `CtxTarget::Lane { kind, lane }`, its menu is
  `clip_lane_menu` + `empty_place_menu`; the clicked time is `KtApp::ctx_at` (used by "Paste here").
- Lessons name the item through the one constant `lessons::GENERATOR_MENU` (tour-lessons-fixes).

## Goals / Non-Goals

**Goals:** the item leaves File; Edit and the track menu create; the track menu creates at the
clicked track and time; real sound kinds as plugins, byte-deterministic, seekable, with honest
dBFS levels; the lesson follows.

**Non-Goals:** waveforms other than sine (square/saw are a distortion FX on a tone), partials,
DTMF, a frequency-modulation tone, an oscillator patched to a bus (Resolve's form), a level meter
calibration window. They stay out until asked for (F-AUD-014 verdict).

## Decisions

### D1. The entries: Edit at the playhead, the track at the clicked place, the bin unchanged

File loses the item (owner's decision, verbatim in `docs/user-feedback.md` 2026-11-01). Edit gets
it after the clipboard block, creating at the playhead on track 1 of each stream (the former File
behaviour). The clip track's right-click menu (`clip_lane_menu`, reached from the header and from
the track body) gets it first in the menu and creates at the clicked track and at `ctx_at` (the
body's clicked time; a header click has no time and takes the playhead).

The track entry lists only the kinds that land on that track's kind: picture kinds on a video
track, sound kinds on an audio track. One body still builds the list
(`gen_ui::generator_kind_menu_keyed` with a stream filter): a kind gets into all three entries by
itself. Offering a picture kind on an audio track would put its clip on another track than the one
clicked, which is exactly what "at the clicked place" forbids.

`KtApp::create_generator(kind)` becomes `create_generator_at(kind, at, lane)`; Edit and the bin
call it with the playhead and no lane, the track with the clicked place. One body — rule 17.

**Revision of R.55 ("the same list hangs in the main menu too" — in File):** the main-menu entry
moves from File to Edit; the reason of R.55 (a person could not find the only entry) is kept —
there are now three entries, each where a person looks for a verb that makes something.

### D2. The lesson follows by one constant

`GENERATOR_MENU = [menu::edit::TITLE, bin::menu::NEW_GENERATOR, genkind::GROUP_TITLES]`. Edit
registers its menu root under `("kt_menu_root", edit::TITLE)` already, so "Show" opens Edit. The
lesson's text is an `Arg::Path` of the same constant, so the words change with it.

### D3. A sound kind fills its burst itself (`Synth`), and has no picture

`kt_plugin_api::generator` gains `pub type Synth = fn(&[f64], i64, &mut [f32])` — the values, the
GLOBAL sample number of the burst's first frame (48 kHz), an interleaved stereo buffer of
`AUDIO_BURST_FRAMES` frames — and `GenKernel::synth(&self) -> Option<Synth>`. `Kind::sound(..)`
builds a sound kind: its frame is a transparent fill nobody asks for, `GenDecl.blank` is on (the
bin card writes the kind's name), `live` is off.

The host reads it in two places: `GeneratorSource::info_for` (`has_video = synth.is_none()`,
`has_audio = synth.is_some() || tone.is_some()`) and `read_audio_burst` (a synth fills the burst;
otherwise the former `tone_1k` path, byte for byte). A burst is a pure function of (values,
burst index): seek and a straight play give the same bytes, the curves are resolved by the source
at the burst's tick like the frame's (`GenState::kind_at`).

`Registry::register` refuses a kind that declares both a `Synth` and a `tone` (two answers to
"what does it sound like").

Rejected: widening `tone`/`gate` (a level and an on/off cannot express a frequency, a colour or a
sweep); a separate `Media::SoundGenerator` family (every host path that already knows a generator
— the factory, the `gen:` codec, the card, retune, the event's values — would need a twin).

### D4. One sine text for every tone

`generator::sine(n, hz) = ((n as f64) * hz * TAU / 48_000.0).fsin()` and
`generator::db_amp(db) = 10f64.fpowf(db / 20.0)` move into the API; `tone_1k` calls them with
`hz = 1000.0` — the same operations in the same order, so SMPTE's and the leader's bytes do not
move (t1048), and a `tone` at 1 kHz −20 dBFS equals SMPTE's tone byte for byte (t1155). The phase
is a function of the global sample number (seek does not click), the rule of R.53 stays: the
"second sine" it forbade is now the same function. The argument stays precise for hours: at
n = 1.7·10⁸ (an hour) and 20 kHz it is ~4.5·10⁸ rad, f64's step there is ~6·10⁻⁸ rad.

### D5. The family: `src/plugins/generators-audio`, five kinds

A new leaf crate (t975: depends on `kt-plugin-api` only), registered after `generators_test`, so
`BUILTIN_IDS` grows at its end and the first 19 menu rows keep their order. Not inside
`generators-basic` (backgrounds and shapes) nor `generators-test` (signal-check pictures): an edit
to a sound kernel rebuilds only this crate, as the audio FX are split from the video ones.

Ids (unique among generators — `noise` is taken by the picture noise): `tone`, `audio_noise`,
`sweep`, `beep`, `silence`. The menu group is `genkind.group_audio`, last in `GenGroup::ALL`.

| Kind | Rows (passport, file order) |
|---|---|
| `tone` | `freq` 20…20000 Hz (1000), `level` −60…0 dBFS (−20), `channels` both/left/right |
| `audio_noise` | `color` white/pink/brown, `level` −60…0 dBFS RMS (−20), `seed` 0…9999 (1), `stereo` flag (off) |
| `sweep` | `freq_start` (20), `freq_end` (20000) Hz, `duration` 0.1…600 s (10), `curve` log/linear, `level` (−20), `channels` |
| `beep` | `freq` (1000), `level` (−20), `period` 0.05…60 s (1), `length` 1…1000 ms (40), `channels` |
| `silence` | none |

Defaults: −20 dBFS is SMPTE RP 155's reference and SMPTE's own tone default here; 1 kHz is the
line-up tone; a log sweep is the measuring one.

### D6. Levels are AES17 dBFS

A sine's level is its PEAK: `amp = db_amp(level)` (a full-scale sine is 0 dBFS). A noise's level is
its RMS read on the same scale: `rms = db_amp(level) / √2`, so −20 dBFS noise and −20 dBFS tone read
alike on an RMS meter. Noise samples are clamped to ±1 (a 0 dBFS noise's peaks clip, honestly).

### D7. Noise is a pure function of the sample number

A burst may be asked in any order (seek, prefetch, export), so no filter state: every sample is
`hash(seed, channel, row, n >> row)` mapped to a uniform [−1, 1) (splitmix64 finaliser, integer
only). White is row 0. Pink is Voss–McCartney: rows 0…11 held 2ᵏ samples, equal weights — the
spectrum falls 3 dB/octave down to ~12 Hz. Brown weighs row k by 2^(k/2), rows 0…9 — 6 dB/octave
down to ~47 Hz, flat below, so it does not wander as a DC offset. The normaliser is exact:
`var = Σ wₖ² / 3`. `stereo` off — both channels from channel 0; on — channel 1 hashes its own index.
The spectral slope is measured in octave bands by a test (t1158), not promised by the formula.

### D8. A sweep's phase is the integral of its frequency

With `t = (n mod N) / 48000`, `N = duration·48000`: linear — cycles `f₀t + (f₁−f₀)t²/2T`; log —
cycles `f₀T/ln k · (k^(t/T) − 1)`, `k = f₁/f₀` (`fpowf`, `ln k = flog10(k)·ln 10`). The sample is
`sin(2π·frac(cycles))·amp`. The sweep repeats every `N` samples from the start of the material.

### D9. A beep is the tone with a gate

On while `(n mod P) < L` (`P` = period, `L` = length, in samples, from the material's start),
the sine from D4 — the leader's beep is this pip with `period = 1 s`, `length = 1/fps`. The gate
is hard, as the 2-pop and the leader beep are.

### D10. `channels`

`both` writes the sample to L and R; `left`/`right` write one and leave the other silent —
channel identification at the head of a master.

## Risks / Trade-offs

- [The track menu hides picture kinds on an audio track] → the bin and Edit list every kind; the
  track menu's list is "what lands here", stated in the scenario's PATH.
- [Held-row noise is pink/brown in octave bands, not a textbook line] → t1158 bounds the slope
  per octave; the note says so (F-AUD-014, Cons).
- [A sound kind's frame exists but is never composited] → `blank` makes the bin card honest;
  `has_video: false` means no video clip is ever created for it.
- [A test that counts menu rows (t973) changes] → it is updated to 24 rows with the first 19
  unchanged, not weakened.

## Migration Plan

None: no file names the new ids before this change; the `gen:` codec and the `.otio` door write a
sound kind like any generator.

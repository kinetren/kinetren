---
id: F-FX-013
title: The sound of the broken stream — an audio codec damaged with the picture
status: in-progress
priority: C
cost: M
category: fx/simulate
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-plugin-api, kt-fx, kt-engine, kt-app]
invariants: [1, 2, 11]
ui_rules: [17]
round: kkt-26.11.03b
point: D1
tests: []
depends_on: [F-FX-011, F-FX-012]
trackers: []
services: []
checked: 2026-11-03
---

# F-FX-013 — The sound of the broken stream — an audio codec damaged with the picture

## Essence

The owner (2026-11-03): Datamosh needs its paired audio filter too. Datamosh breaks a real
inter-frame video stream (dropped I-frames, a repeated P-frame, a frozen reference, lost
macroblocks). The stream it breaks is a muxed stream: its audio packets ride beside the video
packets and are broken by the same events — where the picture moshes, the sound is a broken audio
frame at the same moment, decoded by a real decoder that conceals what it lost.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | no | datamosh is done outside (avidemux) or by third-party looks; the audio is copied untouched |
| Premiere Pro | 26 | no | datamosh plugins (third-party) are video-only |
| DaVinci Resolve | 21 | no | no codec-damage effect in Fairlight |
| Final Cut Pro | 12 | no | no |
| After Effects | 2026 | no | datamosh plugins work on frames; no audio |

**Outside the core.** The sound of a broken codec lives in audio plugins; they show whether a codec
is simulated or imitated.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Goodhertz Lossy | 3.x | Lossy | its manual says it is "not based on any particular lossy codec or standard": spectral thinning, "packet loss" gaps and "packet repeat" fills — an imitation |
| Avidemux | 2.8 | Video: Copy, Audio: Copy | the classic I-frame deletion keeps the audio stream untouched — the sound never moshes |

## The trick behind it

1. **An audio codec has its own "I-frames": the predictor resets.** AAC Main predicts each
   spectral line from the two previous decoded frames (backward-adaptive, no side information)
   and resets the predictors on a schedule so that a decoder can join. A segmented stream (HLS,
   DASH) aligns those random-access points with the video's IDR frames. Drop the video I-frame and
   the audio decoder misses the reset with it: the encoder predicted from a fresh start, the
   decoder from what it heard before — the residual lands on the wrong spectrum and the previous
   sound "moshes" into the new one until the next reset.
2. **A lost frame breaks the next one through the bit reservoir.** Frames borrow unused bits of
   earlier packets (the MP3 `main_data_begin` mechanism): a frame whose data starts in a lost packet
   cannot be decoded either, so one loss is often heard as two.
3. **A flipped bit in entropy-coded coefficients is not one wrong number.** Variable-length codes
   lose their alignment: every coefficient after the error decodes as garbage until the frame ends,
   and the decoder's sanity check either catches it (concealment: the previous spectrum repeated
   with randomised signs) or lets a burst of noise through.

## Pros

- The sound and the picture break at the same frames because both read the same gates and the same
  GOP grid.
- The breakages are a codec's, so they sound like a damaged stream (smearing, metallic repeats,
  holes), not like a bitcrusher.

## Cons

- The codec is our own small AAC-Main-like codec (MDCT 2048 with a sine window, per-line
  prediction, Rice-coded coefficients, a bit reservoir): not bit-compatible with any real one.
- The audio stream starts at the event; when the picture's stream starts `before` frames earlier
  in the event before, the audio decoder has nothing of that event's sound to carry over.

## What users say

NO QUOTES: searched "datamosh audio", "datamosh avidemux audio copy", "Lossy plugin packet loss"
(2026-11-03). The results were tutorials (FLOSS Manuals: Video Copy, Audio Copy — the audio left
untouched) and vendor manuals; no thread with a verbatim opinion about the sound of a datamosh. The
demand is the owner's own (`docs/ktprompts/kkt-26.11.03b.txt`).

## How it fits Kinotrain

CAPABILITY: openspec/specs/audio-fx/

### What already exists

The broken stream's schedule `src/plugins/fx-video-codec/src/mosh.rs` (`Sched::sched_i`,
`dropped`, `pkt_pos`, `frozen_ref` — pure functions of the rows and the positions), the pairing
mechanism of F-FX-012, the frozen math `kt_plugin_api::math`.

### What is missing

An audio codec. Using the system FFmpeg's AAC encoder was considered and rejected: the audio render
thread may not open a codec (invariant 5 and the cpal-callback rule), its output is not frozen
across FFmpeg versions (invariant 11), and a real decoder cannot be told which packets to lose
mid-stream from inside our chain.

### What hinders

The stream schedule lives in video positions on the project grid; the audio state learns the grid
and its clip's timeline origin through the host (the same origin F-FX-012 needs).

### Which invariants it touches

1, 2, 11 (the MDCT and the predictor in frozen math, the same bytes on every platform).

## Estimate

| What | Answer |
|---|---|
| Scope | M |
| Where the main work is | an own codec in `kt-plugin-api` (shared with the camera's microphone codec), the audio state in `src/plugins/fx-video-codec` |
| Project format | does not touch it: new rows written only off their default |
| Tests that will have to be written | MDCT perfect reconstruction; a dropped I-frame at frame N breaks the audio frame at N's time; bloom repeats; bit-error concealment; determinism; bypass identity |
| Depends on | F-FX-011, F-FX-012 |
| Breaks compatibility | no |

## Verdict

Take: a small own codec (determinism and a decoder we control) whose damage is driven by the
datamosh rows and schedule, as the paired audio half of `datamosh`.

## Sources

1. Goodhertz Lossy manual — https://manuals.goodhertz.com/3.11/lossy (accessed 2026-11-03)
2. FLOSS Manuals, "Encoding explained" — https://archive.flossmanuals.net/a-look-at-open-video/encoding-explained (Avidemux: Video Copy, Audio Copy)
3. ISO/IEC 13818-7 (MPEG-2 AAC), section on prediction: backward-adaptive second-order predictors per line, predictor reset groups (secondary summary)
4. ISO/IEC 11172-3 (MPEG-1 Layer III): `main_data_begin`, the bit reservoir (secondary summary)

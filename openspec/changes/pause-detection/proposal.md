# Proposal: pause-detection

> DESIGN ONLY (spec session, 2026-11-24). Registry `v26.10-index.md` N4, the part `subtitles-and-text-editing` left out.
> Note F-AUD-008. Tests: t4204–t4209; t3466 moves here from `subtitles-and-text-editing`.

## Why

The owner, `docs/ktprompts/v26.10.txt` line 89, whole: "помимо базовых функций и дакинга - можно и whisper использовать и
прочие функции базирующиеся на нем, учитывая что он есть в ffmpeg - как минимум поиск по тексту, обрезка/отметка
пауз(кстати, по звуку можно тоже анализ пауз сделать отдельно, но надо учесть и про шумное аудио)"

**Reading, checked.** The line holds two things. The first, whisper and what rests on it (text search, cutting or marking
pauses), is `subtitles-and-text-editing`. The second, in brackets: the analysis of pauses BY SOUND, separately, and it must
take NOISY sound into account. The subtitles design answered it with one line — a dB threshold over the peak pyramid
(its D4). A fixed threshold is exactly what noisy sound breaks: in a recording with a fan the floor lies above any
threshold that a clean recording needs, so either nothing is a pause or everything is.

Today (`286c406c`): no detector anywhere. The peak pyramid drops the last 0–1023 frames of a file (`index.rs:866`, whole
bursts only), which F-AUD-008 recorded as the defect "a detector misses the end of the file".

## What Changes

- **Pauses by sound, without a recognizer**: a detector that learns the floor of THIS recording, moment by moment and band
  by band, and calls a pause what does not rise above it. A fan, a hum, a street or a changing gain do not move it.
- **Show first, then act** (F-AUD-008: every editor that shipped this arrived there): "Pauses…" on chosen audio events, a
  track or the region opens a small window; the pauses found are drawn over the events while it is open, and they move
  as the rows move. Three actions: **Mark** (range markers), **Split at the edges**, **Delete** with ripple (the rule of
  `ripple-and-trim-modes`; disabled with its reason until that is built).
- **Four rows**: sensitivity, the shortest pause, the margin before and the margin after speech, and "ignore sounds
  shorter than" (the merging of neighbours). Defaults that work are the point: sensitivity in the middle, 400 ms, 120 and
  200 ms, 150 ms.
- **Several tracks**: a pause is where ALL the chosen tracks are silent (two speakers), or per track by a choice.
- **An honest "no"**: music under the speech is not noise. When the sound never falls to its floor for most of the time,
  the window says so and proposes the speech's own track.
- **The end of a file** is in the pyramid (the defect closed with its test).

## Capabilities

### New Capabilities

- `material-analysis`: pauses by sound (the adaptive floor). `edit-analysis` adds beats, motion and cuts to it.

### Modified Capabilities

- `media-index`: the peak pyramid covers the file's last, partial burst.
- `ui-reach`: the "Pauses…" window.

## Impact

- kt-media: `pauses.rs` (the detector over `AudioSeries` and the pyramid's level 0); `PeakPyramid::build_until` reads the
  last partial burst (the PEAK kind's version rises: the caches rebuild once).
- kt-app: the window, the overlay, the three actions as batches; `AnalysisKind::Series(MediaId)` on the analysis lane
  when a file's series is not in the cache folder yet; i18n ×5.
- `subtitles-and-text-editing`: its "Mark pauses" becomes this; its fillers stay.
- Format: none (markers, splits and deletes are ordinary edits; nothing regrows, rule 20). OTIO: nothing new to lose.
- Idle: nothing runs but on request; the overlay is one mesh per (result, zoom).

## Questions to the owner

None.

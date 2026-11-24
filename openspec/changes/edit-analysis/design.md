# Design: edit-analysis

> DESIGN ONLY, revision 1 (2026-11-24), base `286c406c`.

## Context

- `AudioSeries` + `kt_media::listen::{SeriesStore, analyze_until, series_for}` + `kt_engine::listen` (a process registry by
  media path, `ensure` before an export): a complete path for ONE measure — computed by an offline job, stored, read by
  a frame, part of the frame's key. It is the mould.
- The cache folder keeps entries per content id with a kind and a version (`index.rs:26–37`).
- Frames for an analysis: an offline source (invariant 5's three conditions) with a `Quality` of 160×90 (by the form),
  read forward in spans (`read_video_span`).
- Links (R.61): `CurveSource::Link { path }`, `ParamOwner`, curves compiled once per revision; between events the timeline
  tick is the denominator.
- `snap_targets` (`timeline.rs:1877`) builds one `Vec` per gesture call: too dear for thousands of beats.
- Sources of `point-tracker`: the same class of job, a result per material frame.

## D1. A measure

```text
MeasureDecl { id: "motion", version, cadence: Burst | Hop512 | VideoFrame, reads: &[MeasureId], bytes_per_record }
```
A measure is a pure function of the file's decoded material and of the measures it reads; records are bytes or i16, one
kind per measure in `media/<content id>/` (`m-<id>`). `audio` is today's series with its kind and version untouched. The
pass over a file computes every requested measure of one kind of material together, so one decode serves all; a measure
computed only from others needs no decode. A dependency graph, a cycle refused at registration. Plugins may add one when
`wasm-plugin-host` is in: the family `measure` is closed to built-ins till then.

| Measure | Reads | Record |
|---|---|---|
| `audio` | sound | exists |
| `onset` | sound | one byte per 512 frames (10.7 ms) |
| `beats` | `onset`, `audio` | a list: tick, strength, bar start; tempo per beat; confidence |
| `luma` | picture | per frame: mean, sharpness, a 32-bin histogram |
| `motion` | picture | per frame: object motion, its centre, camera dx, dy, raw SAD |
| `cuts` | `luma`, `motion` | a list: tick, confidence |
| `speech` | `audio` | `pause-detection`'s score |

## D2. Sound

`onset`: FFT of 1024 every 512 frames (the frozen FFT), the half-wave rectified sum of band rises in dB. `beats` (Ellis'
dynamic programming, f64 with the project math): a tempo curve by the autocorrelation of `onset` over 8 s windows, 60–200
BPM, weighted toward 120 and continuous from window to window by a Viterbi pass, so a tempo that drifts is followed; then
the beat sequence maximising onset strength minus the square of the log deviation from the local period; each beat is
moved to the onset's peak by the pyramid's 5.3 ms blocks within ±20 ms. Bars: the phase among four whose beats have the
largest low-band level (4/4; another number by the correction). Confidence = the mean onset at beats over the mean; under
its floor no grid is drawn and the window says "no steady beat heard" (as `sync` refuses).

The class of error every editor has is the octave (Audacity #6425: 200 read as 100). It is not hidden by a better guess;
it is a correction of one click (D6).

## D3. Picture

Per frame of the source, luma at 160×90 (integer Rec.709): 
- **camera**: the shift (dx, dy) within ±8 px minimising the SAD of the row and column projections between frames;
- **motion**: 16×9 blocks of 10 px, the SAD of each block at the camera's shift, the mean over the blocks above the
  noise (the 25th percentile of the blocks over ±1 s), and their centre of weight; the raw SAD beside it;
- **cuts**: a candidate where the histogram distance and the raw SAD exceed four times the median over ±12 frames; a
  flash is a candidate whose frame after it is nearer to the frame before it than to itself — not a cut (the Premiere
  complaint: cuts on light flicker); a dissolve is found as a run of a steady histogram drift and reported as a span.
Integer arithmetic: the same bytes everywhere, whatever the decoder (the decoder's major is in the entry's salt as for
the copies). A variable-rate source: records are per decoded frame with its tick.

Cost: the decode dominates. The job reports its own speed against real time; it is background class, paused while
playing, and stops at an edit of nothing (it reads a file, not the edit). Sources longer than a screen of decode are
computed from the playhead's region first, the rest behind.

## D4. Seen

A header switch per track, "Beats" on sound tracks, "Cuts and motion" on picture tracks, remembered in `UiState`, off by
default. Turning it on asks the measures of the files on that track (a direct request, so invariant 8 stands: scroll and
zoom ask nothing). Drawn per event from its material window through its map (reverse, warp): beats as ticks of the
waveform (bars heavier, thinned by zoom), cuts as ticks, motion as a thin curve along the filmstrip's lower edge. One
mesh per (event, measure revision, zoom step), replayed on hover frames.

Snapping: shown beats and cuts join through an index of the visible window (a sorted slice per event, found by bisection
in `snap`), not through `snap_targets`' vector; off when the switch is off.

## D5. Acts

All of them: a preview overlay first, then one batch, one undo; nothing regrows.
- **Mark beats / Mark cuts**: markers (beats: named by bar.beat), the tick identity rule respected (a beat on an existing
  marker is skipped and counted).
- **Split at cuts**: splits of the chosen events; the linked sound too, unless told otherwise (Premiere's known
  surprise: it cuts the sound silently; here the tick box is on the window, off by default).
- **Cut on the beat**: chosen butt-joined events of one track; each joint goes to the nearest shown beat of the chosen
  sound track within half a beat by ROLL (`ripple-and-trim-modes`' roll batch; the material's end stops it and the joint
  is left and named). Or "split on every N-th beat / bar" of one event.
- **Still stretches**: the picture on top (the plan's opaque layer rule) for which `motion` in its mapped window stays
  under the row for at least N s; warp 0 (a freeze) is a stretch by definition. Overlay, then range markers.
- **Find moments**: the panel ranks spans of the chosen sources (bin selection, or the sources of the edit): each measure
  is scaled by the file's own percentiles, weights are four rows (motion, loud sound, speech, steady camera), a span is a
  local maximum of the smoothed sum grown to the wanted length (2–10 s), no two overlapping by half. A row: the thumbnail,
  the span, bars of what made it. Click scrubs the bin card (bin-scrub), drag places it with the moment under the mouse
  (bin-drag-moment). "Instead of this" on a selected event: the wanted length is the event's; sources synced with it
  (a link group, a sync offset) come first at the same moment, since that is the other camera; a row's "Replace" puts it
  over the event as a new event on the track above (nothing is lost).

## D6. Corrections

`MediaRef.beats: Option<BeatFix { tempo: ×½ | ×1 | ×2, shift: Tick, beats_per_bar: u8, anchors: Vec<Tick> }>` — project
content by content (a person's word, not a cache). The grid read by anyone is `beats ∘ fix`. An anchor ("the beat is
here", a click on the waveform with the switch on) bends the sequence between neighbouring anchors.

## D7. Linked

`ParamOwner::Material`: `"<event>/material/<channel>"`, channels `level`, `bass`, `onset`, `beat` (1 at a beat, falling to 0
over 250 ms or to the next beat), `bar`, `motion`, `motion_x`, `motion_y`, `cut` (as beat), `brightness`. The value at a
timeline tick is the channel at the event's material tick (through its trim, reverse and warp), linear between records,
0 outside the event. The compiled curve's key takes the measure's content hash and the fix, so a measure that arrives
changes exactly the moments that read it; until it arrives the channel is 0 and the moment is reported short
(`Behind`-like, as a listening generator's feed is silence): an export calls `ensure` for every measure its links read
before the first frame, so its bytes never depend on a background job (invariant 11). kt-cli does the same.

## D8. Class walk — "an analysis locked inside its first consumer"

| Analysis | Consumer today | Here |
|---|---|---|
| peaks | waveforms, sync | an input of `beats`; its own format stays |
| loudness of a file | bin hint | a number, not a series: unchanged; Find moments reads `audio` |
| `AudioSeries` | reactive generators | measure `audio`, bytes unchanged |
| onsets in `sync.rs` | sync | stays: pyramid-based by design (no decode) |
| pauses' speech score | `pause-detection` | measure `speech` |
| tracker samples | `point-tracker` | per event and by hand, not per file: its own entity; the job runner is shared |
| the packet index's key frames | seeking | not content |
| transcript | `subtitles-and-text-editing` | words; Find moments may read "speech" from it later |
| Datamosh's motion vectors | the effect | the picture after effects, per moment: not a file's |
| picture check (`picture-check`) | the render | measures the COMPOSITE; frozen spans by still stretches' twin on the output |
| thumbnails | filmstrip | `luma` could feed a poster choice: not taken |

Second mechanism, "a picture judged by the raw frame difference" — cuts called on flashes, motion called on a pan: only
new code; D3 separates the three.

## D9. Idle

No request, no job. A link is a request made by the project: its measure is computed once per file, background class,
and kept in the cache folder. The overlays are meshes; the panel is static text; `Find moments` recomputes its ranking
only when a row moves (bytes in memory, no decode).

## D10. Stages

A1 the measures, store, `audio` moved, `onset`, `beats`, the overlay, snapping, corrections, Mark beats.
A2 `luma`, `motion`, `cuts`, their overlay, Mark/Split at cuts, Still stretches.
A3 Find moments, Cut on the beat (needs `ripple-and-trim-modes`' roll).
A4 links.
B faces: a detector through `download-library` and the dlopen door of the recognizer; measure `face` (boxes per frame),
`expression` (the motion left inside the box held still), a row in Find moments. Its values MUST NOT drive a picture
parameter (a neural result differs between machines, invariant 11) — they only rank.

## Rejected

- **A beat button writing markers** (the example as the request).
- **The codec's motion vectors as motion** — an encoder's choices, absent in intra codecs; SAD is the owner's own word.
- **Measures of the edited result** — they would change with every edit; what one looks for is in the material.
- **libavfilter's `scdet`** — a sixth FFmpeg library for what 14 400 pixels a frame answer.

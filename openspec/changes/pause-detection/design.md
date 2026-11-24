# Design: pause-detection

> DESIGN ONLY, revision 1 (2026-11-24), base `286c406c`.

## Context

- `AudioSeries` (`kt-plugin-api/src/audio_series.rs`, gen-audio-reactive): per 1024-frame burst (21.3 ms) the RMS of each
  channel, the peak, 16 log bands 40 Hz–16 kHz over a 72 dB range, one byte each; a pure function of the decoded audio;
  kept in the cache folder (`media/<id>/series`); about 9 MB per hour; computed by a short-lived source
  (`kt-media/src/listen.rs` `series_for`).
- The peak pyramid, level 0: 256 frames (5.3 ms), in memory for the waveforms; tail missing (`index.rs:866`).
- Marker identity is its tick, names share one namespace (F-TXT-004): marks from two passes may collide.

## D1. The floor of this recording

For each band `b` in 260 Hz–3.6 kHz (bands 5–11, where speech lies):
- `N_b(i)` = the 10th percentile of the band's byte over the records within ±5 s of `i` (a sliding histogram of 256 bins,
  O(1) per record); in the first and last 5 s the window is what there is. Speech leaves gaps between words, so the low
  percentile is the noise even inside a long sentence; a fan switched on lifts it within the window.
- the score `S(i)` = the mean of the three largest `max(0, level_b − N_b)` in dB. Three bands: a voice is harmonic, a
  broadband rise of the noise moves all bands little.
- a file that never reaches −60 dBFS has no speech: an absolute guard, so digital silence is one pause.

## D2. Decision and edges

Hysteresis: sound begins when `S ≥ on` for two records, ends when `S ≤ off` for three; `on = 9 dB`, `off = 5 dB` at the
middle of the sensitivity row, both shifted together by ±6 dB over it. Sound shorter than the "ignore" row between two
pauses is absorbed; then pauses shorter than the row are dropped; then the margins shrink each pause (before speech at
its end, after speech at its start); then again the length test. The edges are refined inside their record by the
pyramid's level 0: the first block whose half-range exceeds the neighbouring floor block by 3 dB.

Integer arithmetic and bytes only: the result is the same on every machine.

## D3. Where the sound is judged

The FILE, in material time, then mapped to the timeline by the event's map (`clip_timeline_at`: trims, reverse, warp). The
event's effects are not heard: a gate or a denoiser may already have removed the noise, but judging the file is
independent of the edit and never recomputed by it. The window says "judged by the recording".

## D4. "All tracks silent"

The pauses of each chosen track (its events' mapped pauses plus the gaps between its events) are intersected; the margin
rows apply after the intersection. "Each track apart" acts on each track's own events. A mark is placed once per merged
pause, so two tracks never place two markers at one tick.

## D5. The window

A small window (registry row) over the selection: the four rows, the mode, a line "37 pauses, 1 min 12 s", and the three
actions. While it is open the pauses are drawn as bands on the events, replayed by the event paint like the region; a
click on a band excludes it (drawn hollow), so a wrong finding costs one click, not a retune of everything. Delete and
Split run one batch, one undo; a split shorter than `MIN_CLIP_DURATION` is impossible by the length test. The rows are
remembered by the user store.

"The sound never falls to its floor": when sound covers more than 97 % of the range and the mean of `N_b` lies within 6 dB
of the mean level, the line reads so in the hint style and names the likely cause, music on this track.

## D6. The pyramid's tail

`build_until` reads `ceil` bursts; the last burst's frames past the file's end are cut by the known length, so a block is
never made of zeros that are not there. The PEAK kind's version rises to 2, and `complete` is rewritten; the old peaks
are a miss and rebuilt in the background (a cache, invisible to the bytes of anything rendered).

## D7. Class walk — "a fixed level judges a recording whose floor differs"

| Place | Judges by | Here |
|---|---|---|
| pauses (subtitles' D4) | a dB threshold on peaks | this detector |
| the gate effect | a threshold row, per take | stays a row: an effect's parameter is the person's; "from this recording" (the floor + 6 dB) is a note for later |
| the keyed compressor (R.65), `auto-ducking-by-role` | sensitivity over the key's level | the ducking design's content rule reads the level of the KEY tracks; its sensitivity could read `S` — noted there, not taken |
| sync by sound | onsets, normalized | floor-free already |
| BS.1770 gates | −70 LUFS and relative | a standard, left |
| a silent channel in the report (`loudness-norms`) | −70 gate | a dead channel is digital silence or noise: judged by the standard's gate, left |
| the caption splitter ("pause ≥ 0.5 s") | words' times | no level, left |
| the peak pyramid's tail | nothing | D6 |
| `analyze_until` (series) | — | reads `ceil` bursts already (`listen.rs`) |

## Rejected

- **A threshold in dB with a "noise" checkbox** — the noise changes within one recording.
- **A neural voice detector** — a downloaded component for what the floor decides; whisper's words remain for those who
  have it.
- **Marks placed by a background pass** — rule 20.

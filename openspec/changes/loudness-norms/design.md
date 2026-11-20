# Design: loudness-norms

> DESIGN ONLY, revision 1 (2026-11-20), base `ea0aff6f`. Q1–Q2 in `proposal.md`; defaults recommended.

## Context

- The meter: `kt_fx::audio::loudness` (`Loudness { lufs, lra, dbtp }`, BS.1770 gates, ×4 true peak), fed by the
  export pass (export spec "Loudness is measured in the export pass, without master gain"; t403–t407).
- The render window's result (`render_dialog.rs`): one `ui.label` with the three numbers and a `ui.weak` "to −23".
- The picture check (change `picture-check`, D3): the render window's report list with jumps and "Mark all".
- Render settings: `RenderSetup` → `ExportOptions`, the text form `kt_engine::export_settings`, presets in
  `render-presets.json` (render-settings).

## D1. Norms

A closed table `LoudnessNorm { id, target_lufs, tolerance_lu, ceiling_dbtp, lra_max: Option }`: `ebu_r128`
(−23, ±0.5, −1, LRA ≤ 20 guidance), `atsc_a85` (−24, ±2, −2), `streaming` (−14, ±1, −1), `podcast` (−16, ±1, −1).
The id is a render setting (`loudness_norm=streaming` in the text form; an unknown id in a preset is listed and kept,
the presets rule). The row hover cites the source: a standard for EBU/ATSC, "measured by third parties" for
streaming and podcast.

## D2. Per channel, in the same pass

The accumulator keeps, besides the mixed block energies it has, the per-channel K-weighted energies per 400 ms block
and the per-channel ×4 true peak (the FIR runs per channel already — it is kept per channel instead of maxed). From
them: L/R peak, L/R integrated loudness (the same gates applied per channel), the balance = L − R in LU. The mixed
integrated loudness is computed exactly as before (t403–t407 unchanged, bytes of `Loudness` equal).

## D3. Findings

A per-burst record (1920 ticks: per-channel peak and short RMS) feeds a reducer:
- **over ceiling**: bursts whose true peak > the norm's ceiling, merged into spans;
- **clipped**: samples at |x| ≥ 1.0 in the finished mix (counted, spans);
- **silent channel**: one channel below −60 dBFS RMS for ≥ 0.5 s while the other is above −40;
- **imbalance**: |L − R| short-term loudness beyond 6 LU for ≥ 1 s.
The spans enter the shared report list of `picture-check` (kind, time, the event under the playhead's first
audio track at that time, note). The same render gives the same findings (invariant 11).

## D4. The Sound table

| Row | Value | Norm (streaming example) | State |
|---|---|---|---|
| Integrated | −17.2 LUFS | −14 ±1 | outside (red) |
| True peak | −0.3 dBTP | ≤ −1 | outside |
| Loudness range | 7.1 LU | guidance | within |
| Left / right peak | −0.3 / −12.8 dBFS | — | (balance row decides) |
| Balance | +9.4 LU | within ±2 | outside |
| Silent channel | R 38 % of the time | 0 % | outside |
| Clipped samples | 0 | 0 | within |

States are three theme colours with a text mark (✓/!/✕ drawn as Phosphor glyphs, the font rule); the section
header reads "Sound — 4 outside the norm". The offset line: "to the target: +3.2 dB" and, when the peak + offset
crosses the ceiling, "— the peaks would reach +2.9 dBTP" in the error style (rule 16). The norm picker sits in the
render settings and repeats in the result header (changing it re-judges the same numbers, no new render).

## D5. Class walk — "a measured number nobody notices"

| Place | Number | Emphasised? | Here |
|---|---|---|---|
| render result: loudness | LUFS/dBTP/LRA | no | D4 |
| render result: picture | (picture-check) | yes, its list | shared list |
| render result: frames/audio counts (T13) | 517/516 | no | out of scope (T13 row) |
| meters panel | peak, LUFS-S | colour zones exist | unchanged |
| Diagnostics: underruns | count | threshold warnings exist | unchanged |

## D6. Idle

All measurement is inside the export pass; the table draws a finished report; the norm switch recomputes states
only (a few comparisons).

## Rejected

- **A second analysis pass after the render** — the numbers would not be the ones of the written file's pass and
  would cost the render's time again.
- **Per-channel LRA** — misleading on short material (F-AUD-005, cons); per-channel peak, loudness and silence
  answer the owner's case.

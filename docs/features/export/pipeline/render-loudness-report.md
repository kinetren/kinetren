---
id: F-EXP-010
title: The render's loudness report — each value against its norm, per channel
status: done
priority: B
cost: S
category: export/pipeline
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-fx, kt-app]
invariants: [1, 11]
ui_rules: [9, 13, 16]
round: features-slow
point: change render-loudness-norms
tests: [t3106, t3107]
depends_on: [F-AUD-005]
trackers: []
services: []
checked: 2026-11-18
---

# F-EXP-010 — The render's loudness report — each value against its norm, per channel

## Essence

A finished render prints its loudness (integrated LUFS, true peak, range) as one grey line and "to
−23 LUFS" under it. A render with one channel far louder than the other, or one channel silent, or a
peak over the ceiling looks exactly like a good one: the numbers are there but nothing says which is
wrong or what "right" is. The report shows each value against its norm and colours the one that is
out, with the left and right channels side by side.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Loudness log at render | an optional text log of the render's loudness values next to the file |
| Premiere | 26 | Media Encoder: Loudness Normalization, Write Loudness Report | the export normalizes to ATSC A/85, EBU R128 or a custom ITU BS.1770 target, an optional true-peak limiter; the report is an XML file with the loudness and true peak |
| Resolve | 21 | Fairlight Loudness meter (the target standard chosen) | the meter shows integrated, short-term, range and true peak against the chosen standard's target, with an over/under indication |
| Final Cut Pro | 12 | no | the Loudness effect normalizes a clip; no report of the export |
| After Effects | 26 | no | — |

## The trick behind it

- Every editor that judges loudness names the STANDARD first, and the numbers mean nothing without
  it: ATSC A/85 is −24 LKFS with a −2 dBTP ceiling, EBU R 128 −23 LUFS with −1 dBTP, streaming
  platforms −14 LUFS. Media Encoder lets the target be changed only for the generic BS.1770 entry —
  the named standards are fixed, which is what makes them norms.
- The integrated value over both channels hides a channel problem: BS.1770 SUMS the channels'
  energies, so a mix with the right channel silent measures only 3 LU quieter than a balanced one and
  passes as "a bit quiet". A per-channel level is what shows it.

## Pros

- The measurement already runs on the export's pass (F-AUD-005); per-channel energy and peak are two
  more accumulators on the same samples — no second pass, the audio unchanged (invariants 1, 11).

## Cons

- The integrated target differs by delivery (broadcast −23, streaming −14); the report names both
  norms instead of judging against one the person did not choose. A target choice that normalizes is
  F-AUD-005's later stage.

## What users say

NO QUOTES: the owner's own line is the signal (`v26.10` line 56: "на цифры вообще внимание не
обратишь такие. а еще не понятно что является нормой у каждого параметра"); Adobe's help pages and
blog describe the report, no user statement read within the search budget.

## How it fits Kinotrain
CAPABILITY: openspec/specs/export/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
`LoudnessMeter` gives each channel's level over the summed gate's blocks and its own true peak; `render_dialog::loudness_rows` judges each value against its norm and the result block draws them as a table, the bad value in the error colour.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-fx `loudness.rs` (per-channel), kt-app `render_dialog.rs` (the block) |
| Project format | does not touch |
| Tests that will have to be written | the meter's per-channel values on known signals; the block's colours and norms in the harness |
| Depends on | F-AUD-005 |
| Breaks compatibility | no |

## Verdict

Take: the result block becomes a small table — integrated (norm: −23 LUFS EBU R 128 · −14 streaming),
true peak (≤ −1 dBTP), range (informative), left and right (balance within 3 LU, neither silent) —
with the bad value in the error colour. A target choice that also normalizes stays with F-AUD-005.

## Sources

- https://helpx.adobe.com/media-encoder/desktop/encoding-and-exporting/automatic-loudness-correction.html (access date 2026-11-18)
- https://www.adobe.com/learn/audition/web/broadcast-loudness-standards (access date 2026-11-18)

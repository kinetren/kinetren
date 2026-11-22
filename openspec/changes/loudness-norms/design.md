# Design: loudness-norms

> DESIGN ONLY, revision 2 (2026-11-22), base `9c26a478`: the owner's B6 — the project level. Revision 1 put the norm in
> the render settings. Q1 in `proposal.md`; the default is recommended.

## Context

- The meter (`kt_fx::audio::loudness`, BS.1770-4 in f64, ×4 true peak) is fed by the export pass (spec export "Loudness
  is measured in the export pass, without master gain"; audio-render "Loudness — BS.1770 / EBU R 128").
- The player's master gain is monitoring only; the mix ends at the master sum; the R.48 limiter (look-ahead, PDC) exists
  as an effect.
- Background analyses run on the analysis lane (`analysis.rs`): loudness, sync.

The professional path, read 2026-11-22:
- Resolve: the target loudness in Project Settings → Fairlight (default −23 LUFS, −2 dBTP), clips analysed by BS.1770-4
  and normalized to a target with a true-peak target; the Deliver page's "normalize to standard" scales until the
  loudness or the peak limit governs. (Blackmagic forum threads "Audio normalization ignores BS1770 target loudness",
  "17.2.1 Audio normalization and fairlight meter disagree", "Loudness Normalisation not standard"; davinciresolveclub
  "audio levels LUFS"; Hollyland "How to Normalize Audio in DaVinci Resolve".)
- Premiere: Essential Sound Auto-Match sets each clip's gain so clips of a type reach a loudness (−23 LUFS reported);
  short clips are miscounted (Adobe community "premiere pro does not automatch loudness correctly", "auto match volume
  is simply lowering volume", "auto match loudness problem when cutting dialogue"; Adobe help "create audio mix").
- [NOT VERIFIED against the manuals: the sources are forums and vendor blogs.]

## D1. The project target

`ProjectSettings.loudness`: `standard` (EBU R128, ATSC A/85, Streaming, Podcast, Custom), `target_lufs`, `ceiling_dbtp`
(−1 by default; −2 for ATSC), `offsets` per role (Dialogue 0, Music −8, Ambience −8, Effects −4, Title sound −4 LU).
Not written at its default (EBU R128, −23, −1, the default offsets).

## D2. Match loudness of clips

For each chosen event: its integrated loudness through its own chain (an offline render of the event alone, the
export's meter), then `gain_db += target + offset(role) − lufs`, clamped to the gain's passport. Events shorter than
3 s on one track from one source are grouped into one material (their renders concatenated) and get one shared
correction — the gated average needs ≥ 3 s. An event with no number (silence) is left and named. One batch; the gains
are ordinary levels.

## D3. The program stage

`Project.program`: `gain_db` (−24…+24), a true-peak limiter at `ceiling_dbtp` (the R.48 limiter, look-ahead 1.5 ms,
release 50 ms, ×4 detection) always on unless switched off, after the master sum and before the output — in the
player's render and the export's alike (one mix). "Match the program": a background offline pass of the whole mix
WITHOUT the stage measures the integrated loudness; `gain_db = target − lufs`; `measured = (lufs, revision)`; after an
edit `revision` differs and the stage shows "changed since measured". The player's monitoring volume stays after it and
out of every measurement.

## D4. The report

Revision 1's table (each value with the project's norm and a state; per-channel peak, loudness, balance, silence; timed
findings in the picture check's list), now judged against D1's target.

## D5. Class walk — "a level decided per file instead of per project"

| Level | Today | Here |
|---|---|---|
| program loudness | a grey reference line at render | D3 |
| clip loudness by type | by hand | D2 |
| background under speech | Auto-ducking | `auto-ducking-by-role` |
| true peak ceiling | the limiter as an effect | D3 on the master |
| render presets | carried the norm (revision 1) | carry none |

## D6. Idle

Analyses run only on request on the analysis lane, paused while playing; the program stage is one gain and the limiter
in the mix.

## Rejected

- **The norm in render presets** (revision 1) — the owner: "на уровне проекта … а не в пресетах рендера".
- **Normalizing the rendered file afterwards** — another encode; the stage is in the mix.

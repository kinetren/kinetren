# Proposal: loudness-norms

> DESIGN ONLY (spec session), revision 2 (2026-11-22) after the owner's answer (`kkt-26.11.20b` B6). Registry
> `v26.10-index.md` T14, round R.74. Note F-AUD-005. Tests reserved: t3919–t3923, t3966–t3971.

## Why

The owner, `docs/ktprompts/v26.10.txt` line 56, whole: "в окне рендера нет акцента у loudless и второй строки to если я
например прорендерю отрывок с сильным неприятным gain на одном канале или например на тишине у другого или всё
вместе..., на цифры вообще внимание не обратишь такие. а еще не понятно что является нормой у каждого параметра.. ну и
оформить это получше надо, а не просто текстом насрать на отъебись"

His answer, 2026-11-22 (`kkt-26.11.20b.txt` line 17), whole: "п.1. мне кажется что это должно быть на уровне проекта
нормализация а не в пресетах рендера. типо наверное же есть проффесиональный путь нормализации громкости дорожек?
как-то это не тупо как в п.1. должно быть все таки"

The professional path (sources in `design.md`):
- DaVinci Resolve keeps the **target loudness in the project settings** (Fairlight: target −23 LUFS by default, true peak
  −2 dBTP); its Fairlight page **analyses clips by BS.1770 and normalizes each clip** to the target with a true-peak
  limit; its Deliver page can **normalize the finished mix to a standard**, scaling until the loudness or the peak limit
  governs;
- Premiere's Essential Sound **Auto-Match** sets each clip's gain so that clips of one TYPE (dialogue) reach one loudness
  (−23 LUFS), and is known to miscount SHORT clips (too little material for a gated average);
- a **true-peak limiter on the master** keeps the ceiling whatever the gain.

Today: the project has no loudness target; the render window compares with a hard-coded −23 LUFS in grey text; the
master gain lives only in the player (monitoring) and by the export spec must not exist in the project.

## What Changes

- **The project's loudness target** (Project settings → Sound): a standard (EBU R128 −23 LUFS, ATSC A/85 −24, Streaming
  −14, Podcast −16, or a number) and a true-peak ceiling. The meters, the render report and every match below read it;
  render presets carry no target.
- **Match loudness of clips** (Premiere's Auto-Match, Resolve's Normalize Audio Levels): on selected events, a track, or a
  role ("all Dialogue"), each event's gain is set so its own integrated loudness (BS.1770, gated, through its chain)
  reaches the role's target — dialogue at the project target, music and ambience at a set offset below it (default
  −8 LU), effects −4 LU. Short events (< 3 s) of one track and source are measured TOGETHER as one material, so a short
  line is not over-raised (Premiere's known flaw). One undo step; the gains are ordinary event levels, editable.
- **The program stage on the master**: a project-level stage after the mix — "Program loudness": a gain and a
  true-peak limiter at the ceiling (look-ahead, the R.48 limiter). "Match the program" measures the whole edit in the
  background and sets the gain so the integrated loudness hits the target; the stage shows the measured value and a
  "changed since measured" mark after edits. Preview and render play the same stage (one mix).
- **The render report** (revision 1 kept): a table of measurements against the project target with states, per-channel
  peak, loudness, balance and silence, timed findings joining the picture check's list.
- The player's monitoring volume stays out of the project and out of every measurement.

## Capabilities

### Modified Capabilities

- `audio-render`: the program stage (gain + true-peak limiter) in the mix; clip matching by role with short clips
  measured together.
- `export`: the loudness measured after the program stage against the project target; the report table and findings.
- `ui-reach`: the target in Project settings, "Match loudness", the program stage, the report.
- `project-format`: the target, the role offsets and the program stage by content.

## Impact

- kt-model: `ProjectSettings.loudness { standard, target_lufs, ceiling_dbtp, offsets }`, `Project.program: ProgramStage
  { gain_db, limiter_on, measured: Option<(lufs, revision)> }`; a `MatchLoudness` batch of `SetGain`.
- kt-engine: the program stage after the master bus in the audio render (the same in export); the loudness analysis job
  (the export's meter on an offline pass of the mix, background lane) for "Match the program"; per-event analysis
  through the event's chain.
- kt-fx: per-channel accumulators (revision 1).
- kt-app: Project settings → Sound, "Match loudness…" on events, tracks and roles, the program stage in the meters
  column (gain, limiter light, the measured value), the render report; i18n ×5.
- Format: the next free `.kt` version by content. OTIO: a planned row (a foreign editor gets the event gains, not the
  program stage).
- Idle: analyses run on request on a background lane, paused by playback.

## Questions to the owner

Decided (2026-11-22, B6): normalization at the project level, not in render presets, by the professional path.

**Q1. Does the program stage re-measure by itself after edits?** Your words above: "нормализация … на уровне проекта".
Resolve normalizes the mix when you ask (Fairlight analysis, or the Deliver page), not continuously.
- (a) **On request: "Match the program" measures and sets the gain; after edits the stage says "changed since measured";
  the limiter always holds the ceiling** (*recommended*: the loudness does not shift under your hands while you mix).
- (b) Automatically: after edits the program is re-measured in the background and the gain follows.

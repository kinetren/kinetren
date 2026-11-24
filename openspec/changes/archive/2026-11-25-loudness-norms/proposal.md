# Proposal: loudness-norms

> Revision 3 (2026-11-24): re-read against stage 24's tree (`286c406c`), no open question.
> Revision 2 (2026-11-22) followed the owner's answer (`kkt-26.11.20b` B6). Registry `v26.10-index.md` T14. Note
> F-AUD-005; the report's note is the branch's F-EXP-009 renumbered at the merge. Tests: t3919–t3923, t3966, t4200–t4203; t3106, t3107 come with the branch
> `render-loudness-norms`.

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

Today (`286c406c`): the project has no loudness target; the render result prints one label and one weak line against a
hard-coded −23 LUFS (`render_dialog.rs:1682–1694`); `Loudness` has no per-channel value (`kt-fx/src/audio/loudness.rs`);
the master gain lives only in the player (monitoring) and by the export spec must not exist in the project.

**A finished half lies unmerged.** The branch `render-loudness-norms` (53fe3a33, 6eec28ce, 2026-11-18, features-slow's
report 5) built the per-channel meter (t3106) and the result table with norms and colours (t3107, live
`render_loudness`), archived its own change — and no merge ever took it: it is not in `orch-26-11-01`. Its norms are
constants (−23 or −14); the owner's B6 puts them in the project. This change starts from that branch.

## What Changes

- **The project's loudness target** (Project settings → Sound): a standard (EBU R128 −23 LUFS, ATSC A/85 −24, Streaming
  −14, Podcast −16, or a number) and a true-peak ceiling. The meters, the render report and every match below read it;
  render presets carry no target.
- **Match loudness of clips** (Premiere's Auto-Match, Resolve's Normalize Audio Levels): on selected events or a whole
  track, each event's gain is set so its own integrated loudness (BS.1770, gated, through its chain) reaches a target the
  person picks in the same small window: Dialogue (the project's target), Music (8 LU below), Effects (4 LU below) or a
  number. Short events (< 3 s) of one track and source are measured TOGETHER as one material, so a short line is not
  over-raised (Premiere's known flaw). One undo step; the gains are ordinary event levels, editable. Roles
  (`auto-ducking-by-role`) are not needed; when they exist, an event's role preselects its row.
- **The program stage on the master**: a project-level stage after the mix — a gain and a TRUE-PEAK limiter at the
  ceiling. A project has none until the person switches it on or presses "Match the program", and without it every byte
  is today's. "Match the program" measures the whole edit in the background and sets the gain so that the integrated
  loudness AFTER the limiter hits the target; the stage shows the measured value and "changed since measured" after an
  edit that is heard. Preview and render play the same stage.
- **The render report**: the branch's table, judged against the project's target and ceiling instead of constants, plus
  timed findings (over the ceiling, a silent channel, an imbalance) — a list of their own until `picture-check` adds its
  kinds to it.
- The player's monitoring volume stays out of the project and out of every measurement.

## Capabilities

### Modified Capabilities

- `audio-render`: the program stage (gain + true-peak limiter) in the mix; clip matching by role with short clips
  measured together.
- `export`: the loudness measured after the program stage against the project target; the report table and findings.
- `ui-reach`: the target in Project settings, "Match loudness", the program stage, the report.
- `project-format`: the target, the role offsets and the program stage by content.

## Impact

- kt-model: `ProjectSettings.loudness { target_lufs, ceiling_dbtp, standard }`, `Project.program: Option<ProgramStage
  { gain_db, ceiling: follows the settings, measured: Option<{lufs, dbtp, sound: u64}> }>`; commands `SetLoudness`,
  `SetProgram`; a match is a batch of the existing `SetGain`.
- kt-fx: `audio::program` (the stage's kernel: gain, true-peak limiter); per-channel accumulators (the branch).
- kt-engine: the stage at the OUTPUT of the mix, after the kept mix (design D3), in `player.rs` and
  `export::render_raw_audio_range`, which takes it as an argument (its other callers pass none); two analysis jobs.
- kt-app: Project settings → Sound (`settings_window.rs`, the card `settings.audio`), the stage in the meters column
  (`meters.rs`), "Match loudness…" in the event and track menus, the report (`render_dialog.rs`), `AnalysisKind` grows
  (t774's lock is extended by name); i18n ×5.
- Format: the next free `.kt` version by content. OTIO: the planned row stands.
- Idle: the jobs run on request on the analysis lane, paused by playback; the stage costs only while sound plays.

## Decided

- **The project, not the presets** (B6).
- **When the gain is measured: on request.** Put as a question in revision 2, decided here: B6 asks for the professional
  path, and Resolve normalizes when asked (the Fairlight analysis, the Deliver page), never behind the mixer's back. A
  gain that follows every edit would move the level of everything the person is mixing. The stage says "changed since
  measured", and the render report judges the file either way.

## Questions to the owner

None.

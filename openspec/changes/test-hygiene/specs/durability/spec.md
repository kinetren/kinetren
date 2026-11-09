## ADDED Requirements

### Requirement: A live try starts clean and a red try keeps its evidence
Before each try of a scenario the live bench SHALL empty the scenario's folder, so nothing an earlier
run left reaches this one, and the stand SHALL drop its dump's companions (`.crash.json`, `.load.json`)
when it starts. The lone rerun of a red scenario SHALL keep the red try whole in
`<scenario>/first-try/`, and every try SHALL keep the lines it printed in its folder (`said.txt`); the
report names where the first try is. The program's geometry dump and the bench's command file
SHALL live on tmpfs, emptied when a program starts, and be copied beside the screenshots when the
scenario ends; a red scenario of the stand gets the stand's journal and trace beside it.
Why: `renderregion` went red twice on a `geom.json.crash.json` a build of the day before had left
(found-defects 165), and the rerun overwrote the red try's screenshots, dumps and log, so a red that went
green alone left nothing to read (found-defects 163). On the working disk a write of the dump stood
10–20 s while builds synced it, and `space_hold` read a head standing still for 17 s of a playback
that ran.

#### Scenario: A stale file does not reach the run, and the retry keeps the first try
- **WHEN** a run of `_flaky` (red the first time) and `_red` (red always) starts on folders holding a
  file of an earlier run
- **THEN** the earlier file is gone before the first try; `_flaky` passes on the retry; both folders hold
  the first try in `first-try/` (its mark, its lines, its screenshot) beside the retry's own, and the
  report names `first-try`
- **TEST** t2210 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-check.sh` on stand-in
  scenarios; without Xvfb the test is skipped with the named reason)

# Tasks

The "before" numbers are taken by measurement on 5c7dc09 (the table is in `design.md`, "Context"): live scenarios
1012 s, the workspace 259 s, build 61 + 43 s, kt-gpu 10 s, SIMD ~2 s. Each stage of the round
ends with a commit in Russian (the auto-commit rule), and all the work ends with `finish.sh`.

## 1. Timing and budget: numbers first, then acceleration

- [x] 1.1 Wrap EVERY stage of `src/scripts/finish.sh` in a measurement and print a table "stage — seconds" plus the total; do not change the stages' verdicts (they remain with cargo). Check: `finish.sh` prints a time line for all executed stages, `grep` over the output finds the total.
- [x] 1.2 Create `src/scripts/finish-budget.py` (accounting of stages, the 300 s budget, a flag on the most expensive stage when over budget) with a `--selftest` mode on synthetic data. Check: t800 (`src/apps/kt-app/src/reach_tests.rs`) calls the guard and fails if a script stage has been left without a measurement or the overrun is not flagged.
- [x] 1.3 Make the workspace run a stage of `finish.sh` and remove the `cargo test -p kt-gpu` stage as covered by it; keep the SIMD stages (their own environment, 2 s for all). Check: t800 sees the workspace stage in the table; substituting a failing `cargo` makes the stage red (the scenario "Stage verdict from cargo").
- [x] 1.4 Record in the report the first full timing AFTER this stage as the reference point (expected ~21 min). Check: the stage table in the report, the numbers agree with `target/finish-times.txt`.

## 2. Build: mold

- [x] 2.1 Create `src/.cargo/config.toml` with mold for `x86_64-unknown-linux-gnu`; measure the cascade from `touch libs/kt-model/src/lib.rs` (`cargo test --workspace --no-run`) and the build of kt-app+kt-cli before/after. Check: the numbers in the report against 61 s and 43 s; `cargo test --workspace` is green.
- [x] 2.2 Check that the delivery is intact: `scripts/release-build.sh` builds in the container, `ldd … | grep -c libav` == 0, the GLIBC_ ceiling is in place. Check: a run of `scripts/release-build.sh` (invariant No. 10, capability media-index).

## 3. Live stand: readiness channel, frame, one process

- [x] 3.1 Add to kt-app, under an environment variable, the test channel of the stand ("open a project", "bring to a clean state", "report readiness") and the number of the drawn frame next to `KT_GEOM_DUMP`; without the variable the channel is dead. Check: t805 (`reach_tests.rs`) — the channel is absent without the variable, the frame number grows only when drawing.
- [x] 3.2 Teach `kt_live.py` to wait for the OBSERVABLE event (`wait_frames`, the appearance of a widget in the dump, a pixel change) and to type a line in full; `focus()` — only on a change of window and on the first gesture. Check: t803 plus the live `src/scripts/live/nestparam.py` — the verdict is the same, the scenario time is half of 136 s or less.
- [x] 3.3 Create the guard `src/scripts/live-sleeps.py --check`: a pause without a reason in words — red; the remaining pauses are listed with reasons (the double-click threshold, autosave by the clock, the drag step). Check: t803 (`reach_tests.rs`) — removing the reason from a pause makes the guard red.
- [x] 3.4 Move the harness to one kt-app process per worker with proof of cleanliness before the first gesture (an empty list of open windows, the project address) and the "its own process" flag on the startup scenarios (`recover.py`, `unsaved.py`, `oldfile.py`, `import_old.py`, the lock check). Check: t805 and the live `recover.py`, `level.py`; a separate comparison run "stand against cold starts" — the verdicts of all 50 scenarios match.
- [x] 3.5 Measure the live run after stage 3 (expected no more than 400 s against 1012 s) and record the number in the report. Check: the `finish.sh` stage table.

## 4. Worker parallelism

- [x] 4.1 Distribute scenarios to workers in `src/scripts/live-check.sh`: their own Xvfb+openbox, their own `XDG_DATA_HOME`/`KT_USER_STORE_DIR`, their own directory in `target/live-out`, reduced software-rendering threads; the number of workers — from the cores and `KT_LIVE_WORKERS` (1 = as before). Check: t802 (`reach_tests.rs`) on a substitute scenario directory — the screens and storages are different, the worker limit is observed.
- [x] 4.2 Re-run a red in parallel ALONE; count only a repeated failure as red; name in the report those who passed on the second attempt. Check: t801 (`reach_tests.rs`) — a substitute scenario that fails once gives a green run with a flag; one that fails twice — red with the name and the path to the screenshots.
- [x] 4.3 Run all 50 scenarios in parallel three times in a row and make sure the re-runs do not repeat on the same scenarios. Check: three runs of `./scripts/live-check.sh` — **594, 600 and 604 s, 50 of 50 green each time, zero re-runs**.

## 5. Revision of the set: coverage report and move to headless

- [x] 5.1 Create `src/scripts/live-coverage.py --check`: the registry windows `WINDOWS` and the fields of the geom dump against the gestures of the scenarios; print the places not clicked EVEN ONCE, and their number; the total is printed by `finish.sh`. Check: t804 (`reach_tests.rs`).
- [x] 5.2 By the report the candidates were marked and **six scenarios** moved (`nest`, `nestenter`, `nestopen`, `nestzero`, `fxfield`, `curvesrc` — 175 s of the 1677 s sequential live run): for each, the gesture had already been clicked by another scenario, and the assertion was about the model or the file. The addresses are in the machine-readable registry `MIGRATED` (`src/scripts/live-coverage.py`); the guard fails both on the remaining file and on a non-existent address. The only gesture that lived only in `curvesrc` (the "unlink" button) moved live into `formula.py`. Check: `openspec validate --strict`, the guards PATH/sleep/coverage and the full `cargo test --workspace` are green; not a single spec scenario disappeared — none of the six was named by a spec, and the number of untouched places did not grow (69 touched before and after).
- [x] 5.3 The holes of the report were closed for **23 places out of 105**: a new live scenario `trackverbs.py` (the row menu — "Properties…", "Mute", "Solo", "Lock", "Effects…"; the track window — three checkboxes, the blend mode, "Cancel"; test t806 and a `PATH` line in the spec `ui-reach`), plus live steps in the existing scenarios: the raster, the frame rate and all four conform rules in the settings window and the collapsing of the "Frame" group (`conform.py`), the form of the leading "Geometry" node — the scheme, a row of rules, "Reset" (`geomctl.py`), "Delete curve" and "Done" in the effects window (`formula.py`), the solo warning in the render window (`track.py`). The report also stopped lying in its own favor: the aim by coordinate from the dump and the aim by prefix (`startswith`) are counted as aim. Check: t804 — 82 untouched at a declared limit of 82 (`UNTOUCHED_LIMIT`, a ratchet); all live scenarios are green.
- [x] 5.4 Measure the workspace run and decide the question of `cargo-nextest` by measurement: it is taken only if the full set stays green and the gain is measurable; `cargo test` remains the reference. Check: two runs (`cargo test --workspace` and nextest) — the matching list of passed tests, the numbers in the report.

## 6. Final

- [x] 6.1 Add the "why" to `openspec/specs/durability/design.md` and `openspec/specs/ui-reach/design.md`: the revision of the parallelism ban with its reason, 96 % sleep, the live/headless criterion, the budget as a printout and not as a knife. Check: `openspec validate --strict`, the sections are readable without reference to the change.
- [x] 6.2 `./scripts/finish.sh` in full (a rerun after stage 5, version `kt26.09.28-be45d563`): **643 s** — live scenarios 424 s (66 %, 45 of 45 green), workspace tests 215 s (33 %), build 2 s, SIMD 2 s, cleanup 0 s. The 300 s budget was NOT met, the overrun is 343 s; the cause is named by the script itself and analyzed with numbers in `openspec/specs/durability/design.md`: the floor with the current mechanics is 215 s of workspace plus ≈115 s of fixed costs of live scenarios (≈10 s per scenario even without a single gesture), that is ≈330 s, and the revision of the set does not move it by construction. All three guards (`specs-user-path.py`, `live-sleeps.py`, `live-coverage.py`) are green, the list of the step screenshots is printed. Check: the `./scripts/finish.sh` run itself.

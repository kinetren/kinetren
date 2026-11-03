# Tasks

## 1. The contract: fork and the echo row

- [ ] 1.1 `kt-plugin-api`: `AudioState::fork` (default `None`), `CurveRead::clone_box` + `Clone for Box<dyn CurveRead>`, `Clone` on the `dsp` primitives, `Decl.echo_row`; builds (`cargo check --workspace --all-targets`)
- [ ] 1.2 Every audio plugin state (`src/plugins/fx-audio-*`) and kt-fx's `OwnedCurve` derive `Clone` and implement `fork`; t1330 (every built-in kind forks and continues byte for byte)
- [ ] 1.3 `pitch_shift` passport: the choice row `echo_before` = cut | ring (`omit_default`), `Decl.echo_row`; t973/t973c and t322 stay green (the default writes nothing, sounds the same)

## 2. The processor: rings on

- [ ] 2.1 `ClipProcessor`: the effective choice (`ring`: context applies, the `ctx` slot's echo row is on, a node before `ctx` rings); the node loop factored so the main stream and the twin run the same code
- [ ] 2.2 D2: a ringing prefix node without look-ahead is reset where its content crosses the in-point continuously (the call split at the crossing); t1332
- [ ] 2.3 D3: the fork at the input's out-point, the twin fed silence, the gate merge, the promotion and `ringing`; reset/seek/seam rules; t1331, t1334, t1339
- [ ] 2.4 Joins and splits in ring mode: t1333, t1335 (one piece == pieces, `wanted ⊇ read`, the loop laps); t1105–t1114 and GOLDEN_AUDIO green; measure the edge (the echo level right past the out-point, foreign energy = 0) and record the numbers in this tasks file

## 3. The file

- [ ] 3.1 `project_file`: format 3 by content (`Project::has_ringing_pitch` over every reel's events, tracks and morpher patches), reads 1–3, refusal of a newer version; t1336 (incl. the OTIO round trip); t811/t812/t1260 green

## 4. The interface

- [ ] 4.1 `fx_editor.rs`: the pitch panel's echo line (combo + hint), one undoable base edit; catalogs ×5 in the owner's words; `python3 scripts/subset-fonts.py`; t1337 (PATH), i18n/font tests, reach/layout tests green
- [ ] 4.2 Live scenario `src/scripts/live/pitchtail.py` on our fixture (a step screenshot per action, the saved `.kt` checked); registered for `live-check.sh`; t1338 run with `KT_LIVE_DISPLAY=:230` if :99 is busy

## 5. Docs and wrap-up

- [ ] 5.1 Specs merged on archive; `openspec/specs/audio-fx/design.md` and `audio-render/design.md` get the why (D2–D5); kt-fx CLAUDE.md lookahead rules get the ring bullet; the crate notes in kt-plugin-api if any
- [ ] 5.2 `docs/guide/whats-new.md` `## kt26.11` line; `docs/ktprompts/kkt-26.11.01-index.md` K15 row moved; `docs/features/audio/processing/pitch-shift.md` verdict line; `features-index.py --check`, `ktprompts-index.py --check`, `specs-user-path.py --check`
- [ ] 5.3 `cargo clippy --all-targets`, `cargo test --workspace` green; archive the change; `git merge master`; `./scripts/finish.sh` and the version in the report

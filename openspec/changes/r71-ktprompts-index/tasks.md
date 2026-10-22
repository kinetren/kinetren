# Tasks

The change does not change the program's code: its product is the registry, the guard and the owner's answers. The stages
are verifiable separately; each ends with a commit in Russian.

## 1. Source and registry

- [ ] 1.1 Registry `docs/ktprompts/v26.10-index.md`: the table from `design.md` §3 is carried over as is (all items G/U/T/W/A/L/N/C/P/F/R/O/X/M/Q/K/S/E/MW/TD/FUT), columns: number, claim verbatim (brief), state from the closed set, address, stage; section "what OTIO export loses" (empty for now — the rule of stage 3). Check: each numbered item of `docs/ktprompts/v26.10.txt` has a row (the guard of stage 2).
- [ ] 1.2 Section R.71 in `docs/user-feedback.md` is appended with questions 1–11 from `design.md` §4 (the owner's own words are already recorded at the proposal). Check: the section exists, the questions are numbered the same way as in the registry.

## 2. Guard

- [ ] 2.1 `src/scripts/ktprompts-index.py --check`: reads the source (numbered items by sections `== … ==` and blocks), reads the registry, fails on an item without a row, a state outside the set, `done` without `fn tNNN_` in the sources of `src/`, a `question` without text; prints the count by state. Check: t823 (`src/apps/kt-app/src/reach_tests.rs`) — the whole registry is green, three corruptions (a deleted row, a non-existent test, an empty question) are red with the item's address.
- [ ] 2.2 The guard in `src/scripts/finish.sh` next to `features-index.py --check` and `specs-user-path.py --check`. Check: `./scripts/finish.sh` prints its line.

## 3. Rule for agents

- [ ] 3.1 In the root `CLAUDE.md` — the rule: a model change that the OTIO exchange file does not express is recorded in the "what export loses" section of the claims registry (the owner's requirement, CORE section); in `openspec/specs/project-format/design.md` — a reference to the registry from the R.70 section. Check: `openspec validate --all --strict`.

## 4. Owner's answers and the nearest plan

- [ ] 4.1 Questions 1–11 are asked to the owner (AskUserQuestion one or two at a time — he asked to discuss each point separately); the answers verbatim — in `docs/user-feedback.md` R.71, the registry's `question` rows are moved to decisions. Check: the guard is green, not one `question` row without an answer or without the mark "waiting".
- [ ] 4.2 Following the answer to question 11 — the change of the next round is opened (`/opsx:propose`) for the first stage of the registry; in the registry its items have the round number set. Check: `openspec list` shows the new change, the registry refers to it.

## 5. Finish

- [ ] 5.1 `./scripts/finish.sh` in full (the code has not changed — only checked that the guard is built in and green); version `vYY.MM.DD-hash` in the report.

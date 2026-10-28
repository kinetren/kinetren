# Tasks

Test numbers t984–t989 are reserved by this change. The update line of the What's new panel reads
the state of `update-check` (stage 2 of that change); everything else does not wait for it. Every
stage ends green and is committed at once (English message, first line = the stage and what was
done).

## 0. Records before code

- [x] 0.1 The owner's words (item 8 of `kkt.txt`; the answers "a Guide window", "a welcome screen", "highlight the guide on the welcome screen"; the request of 2026-10-28 about the intro window) are recorded in `docs/user-feedback.md`, round of 2026-10-27/28; this proposal recorded
- [x] 0.2 Feature notes: F-HLP-028 carries the dated section "Revised by the owner, 2026-10-28" and the status `accepted`; F-HLP-027, F-HLP-023 and F-HLP-029 carry dated revision paragraphs naming this change
- [ ] 0.2b The README summary-table row of F-HLP-028 (status `rejected` → `accepted`) updated by hand from `features-index.py --table`. Verify `python3 src/scripts/features-index.py --check` and `openspec validate welcome-window-and-tour --strict` green; commit stage 0
- [ ] 0.3 The open question 1 of design.md (the accent frame) is answered by the owner and recorded in `docs/user-feedback.md`; until then the frame is not built

## 1. Probes before code

- [ ] 1.1 Probe: can egui 0.36 open a `menu_button` popup of the menu bar programmatically (memory popup id of the File menu) — a throwaway harness test in `src/apps/kt-app/src/reach_tests.rs` that opens File by code and finds "Create a generator…" in the accessibility tree of the next frame. Record the answer here; if no, adjust the t988 PATH to the fallback of design (Risks) before stage 4
- [ ] 1.2 Probe: render one poster frame of a fixture project through the reel-preview offline path at 320×180 on release, timing it (`cargo build --release -p kt-app`; a `#[ignore]` timing test); record the time here

## 2. Records in the user store

- [ ] 2.1 `user_store.rs`: `recent.json` (bound 12, newest first, missing kept), `templates.json`, the tour progress and the start choice, each with the `.broken` and future-version rules; seed from `last_project` once; posters folder ownership. Write t985 and t987 first (red), then green
- [ ] 2.2 Commit stage 2 (`cargo test -p kt-app`)

## 3. The welcome window

- [ ] 3.1 `welcome.rs`: `StartFacts`, `primary()` and t984 (every case of the spec, incl. FFmpeg missing and recovery); green
- [ ] 3.2 The window `kt_welcome` (registry entry in `WINDOWS`, form contract t779), template tiles, recent cards, the tour tile, the Guide link, What's new from `docs/guide/whats-new.md` (new file, English, `## kt26.10`), "Show at start"; `run` start order of design D4; View checkbox; Help menu. Write t986 with its PATH in `reach_tests.rs`; verify t986, t757, t779, t83, t20 green and `src/scripts/specs-user-path.py --check` (t799) green
- [ ] 3.3 The poster job on save/close (design D5); verify t985's poster half and that no file appears in the project folder; the release timing of 1.2 recorded against a Ctrl+S
- [ ] 3.4 "Save as template…" in the Project settings window; verify t987 and t780 green
- [ ] 3.5 The update line from the update-check state (when that change is applied); a harness case in t986 with a state file naming a newer release
- [ ] 3.6 Catalogs `[welcome]`, `[help]` ×5, `python3 scripts/subset-fonts.py`; verify t100, t125, t128, t171 and `font_tests.rs` green; commit stage 3

## 4. The lessons

- [ ] 4.1 Move `demo_project` from `src/apps/kt-cli/src/main.rs` into the shared builder in its generator-only form; kt-cli `demo` calls it. Verify t151 (`src/apps/kt-cli/tests/cli.rs`) green and that the lesson project renders with `kt-cli render` without a missing source
- [ ] 4.2 `lessons.rs`: the lesson table, `Predicate`, `Target`, `done()`; the side panel; "Show"; progress; "Start over". Write t988 (both scenarios, incl. `no_teaching_layer_over_the_ui` over the closed list of layers above `Order::Middle`) with its PATH; verify t988, t83, t20 (no repaint at rest with the panel open) green
- [ ] 4.3 Catalog `[lessons]` ×5 within the budget; verify t100, t128; commit stage 4

## 5. The Guide

- [ ] 5.1 `guide.rs`: the compiled page list (guard: equals `docs/guide/*.md`), the subset renderer with `<kbd>`, the `[guide.index]` entries with anchors, search; the window `kt_guide` in `WINDOWS`. Write t989 with its PATH; verify t989, t757, t779 and `readme-check.py --check` (t826) green
- [ ] 5.2 Commit stage 5

## 6. Live and records

- [ ] 6.1 Live scenario `src/scripts/live/welcome.py` (own process, own `XDG_DATA_HOME`): first start → the window with "Start the tour" primary → a template → Help → Welcome window… → a recent card → Help → Guide… → search "loop" → Help → Start the tour → step 1 closes by adding a title; `kt_live.step` screenshot per step. Verify `./scripts/live-check.sh welcome` green
- [ ] 6.2 `src/apps/kt-app/CLAUDE.md`: the start order and the three new modules; commit stage 6

## 7. Finish

- [ ] 7.1 Run `./scripts/finish.sh` from `src/`; record the version `vYY.MM.DD-hash` and the live screenshots of 6.1 in the report
- [ ] 7.2 After archiving: F-HLP-027/028 verdict lines name `CAPABILITY: openspec/specs/ui-reach/`; append the "why" (D1, D2 revisions, D3 in-viewport, D5 posters outside the tree, D8 the lesson sandbox) to `openspec/specs/ui-reach/design.md` and `openspec/specs/durability/design.md`

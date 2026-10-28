# Tasks

Test numbers t924–t929 are reserved by this change. Every stage ends with a commit (English
message, first line = the stage and what was done). Stage only this change's files.

## 0. Records before code

- [x] 0.1 The owner's items 4–5 of ktprompts 26.10.18 are recorded in `docs/user-feedback.md` (round "The owner's list of 2026-10-27 and ktprompts v26.10.18") and name `selection-copy-target`; the feature note `docs/features/timeline/assembly/copy-target-and-region-over-event.md` (F-TL-013) is written. Verify `python3 src/scripts/features-index.py --check` reports no error for F-TL-013 and `openspec validate selection-copy-target --strict` is green
- [ ] 0.2 Update the `timeline/` row ("written / planned") and the summary table of `docs/features/README.md` for F-TL-013 (rule 10). Verify `features-index.py --check` is green

## 1. Tests first

- [ ] 1.1 Write t924, t925, t926 in `src/apps/kt-app/src/layout_tests.rs` (region + one selected A/V event; several / none / no region; range-marker double-click with three and with one crossing event), asserting the clipboard block, the region left in place and the status-line note. Verify: t924 and the one-event half of t926 are RED, t925 and the three-event half of t926 are green (today's rule), record the first failure lines here
- [ ] 1.2 Write t927 and t928 in `layout_tests.rs` (harness step 0.02 s like t206): triple click on an event body with and without another FX window open; on a reel instance; on the level line at −6 dB. Verify they are RED (no region from a triple click; the FX window stays open)
- [ ] 1.3 Write t929 in `src/apps/kt-app/src/reach_tests.rs` (three ways, one region; the hint and the chord column promise the triple click). Verify it is RED and t799 accepts the PATH lines of all six scenarios

## 2. Copy precedence

- [ ] 2.1 `copy_selection` in `src/apps/kt-app/src/app.rs`: the resolver of design D1 (count link groups of `selected_with_groups`), the region untouched by rule 1. Verify t924, t925, t926 green and t205, t250 still green
- [ ] 2.2 Three copy notes in all five `i18n/*.toml` replacing `msg.clips_copied` (D2), the old key removed. Verify t125, t100 and t128 are green

## 3. Triple click

- [ ] 3.1 `src/apps/kt-app/src/timeline.rs`: the one-frame `pending_double` record set by the double-click branch (FX opened / reel toggled / level reset), the `triple_clicked()` branch calling `region_from_event` and undoing the view effect (D3). Verify t927, t928 green and t206, t207, t636 still green
- [ ] 3.2 Catalogs: `hint.clip` names the triple click, `tl.menu.region_from_event_accel` shows the triple click, all five languages; run `python3 scripts/subset-fonts.py` if a new CJK/Arabic glyph appears. Verify t929, t125, t100, t128 and the hint guards of `ui-reach` (t83 family) are green
- [ ] 3.3 Run `reach_tests.rs` and `layout_tests.rs` whole, then commit stages 1–3 (`cargo test --workspace --no-fail-fast` green, `cargo clippy -p kt-app --all-targets` without new warnings)

## 4. Docs and live

- [ ] 4.1 `docs/guide/shortcuts.md`: the Ctrl+C row states the three rules in order ("the selected event when exactly one is selected; otherwise what lies in the region; otherwise the selected events"), a new row "triple-click an event — region over the event and its linked partners", the Shift+double-click row kept; `docs/guide/timeline.md` gesture table gets the triple click. Verify the guide guard t826 and t869 (`src/apps/kt-cli/tests/readme_guard.rs`, `src/scripts/readme-check.py`) are green
- [ ] 4.2 Live scenario `src/scripts/live/select.py` (extend it): region by ruler drag, click one event, Ctrl+C, Ctrl+V at empty time — one event pasted whole; triple-click another event — region band over it, no FX window. Verify `./scripts/live-check.sh select` green, screenshots listed here
- [ ] 4.3 Commit stage 4

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` and the live screenshots here
- [ ] 5.2 After archiving: set F-TL-013 to `done` with its frozen verdict and `CAPABILITY:`, and append to `openspec/specs/ui-timeline/design.md` the "why" of D1 (link groups counted, the range-marker consequence, Vegas/Premiere rejected) and D3 (no delayed double click)

# Tasks

Test numbers t924–t929 are reserved by this change. Every stage ends with a commit (English
message, first line = the stage and what was done). Stage only this change's files.

## 0. Records before code

- [x] 0.1 The owner's items 4–5 of ktprompts 26.10.18 are recorded in `docs/user-feedback.md` (round "The owner's list of 2026-10-27 and ktprompts v26.10.18") and name `selection-copy-target`; the feature note `docs/features/timeline/assembly/copy-target-and-region-over-event.md` (F-TL-013) is written. Verify `python3 src/scripts/features-index.py --check` reports no error for F-TL-013 and `openspec validate selection-copy-target --strict` is green
- [x] 0.2 Update the `timeline/` row ("written / planned") and the summary table of `docs/features/README.md` for F-TL-013 (rule 10). Verify `features-index.py --check` is green
  - The row and the counts were already in place from stage 0; `features-index.py --check` green. The
    status stays `candidate` until archive (5.2): `accepted` freezes "How it fits" to 5 lines.

## 1. Tests first

- [x] 1.1 Write t924, t925, t926 in `src/apps/kt-app/src/layout_tests.rs` (region + one selected A/V event; several / none / no region; range-marker double-click with three and with one crossing event), asserting the clipboard block, the region left in place and the status-line note. Verify: t924 and the one-event half of t926 are RED, t925 and the three-event half of t926 are green (today's rule), record the first failure lines here
  - The tests were written together with the code; the red proof is taken by disabling the rule
    (`one_event && false`): t924 "the clipboard must hold the clicked event and its partner" and
    the one-event half of t926 RED, t925 green (today's rule), as designed.
- [x] 1.2 Write t927 and t928 in `layout_tests.rs` (harness step 0.02 s like t206): triple click on an event body with and without another FX window open; on a reel instance; on the level line at −6 dB. Verify they are RED (no region from a triple click; the FX window stays open)
  - RED with the triple-click branch disabled (t927: "the triple click set no region", t928: the
    reel's region missing). Found while writing t928: after the level reset the line jumps to 0 dB
    under the pointer, so the third click lands on the BODY — `DoubleEffect::LevelReset` makes the
    third click of that gesture do nothing.
- [x] 1.3 Write t929 in `src/apps/kt-app/src/reach_tests.rs` (three ways, one region; the hint and the chord column promise the triple click). Verify it is RED and t799 accepts the PATH lines of all six scenarios
  - RED with the branch disabled; t799 accepts the PATH lines.

## 2. Copy precedence

- [x] 2.1 `copy_selection` in `src/apps/kt-app/src/app.rs`: the resolver of design D1 (count link groups of `selected_with_groups`), the region untouched by rule 1. Verify t924, t925, t926 green and t205, t250 still green
  - `copy_selection`: one link group (or one ungrouped event) → `copy_clips`, the region untouched;
    else region; else selection. t924–t926, t205, t250 green.
- [x] 2.2 Three copy notes in all five `i18n/*.toml` replacing `msg.clips_copied` (D2), the old key removed. Verify t125, t100 and t128 are green
  - `msg.copied_event` / `copied_region` / `copied_events` in five catalogs, `clips_copied` removed;
    t125, t100, t128 green.

## 3. Triple click

- [x] 3.1 `src/apps/kt-app/src/timeline.rs`: the one-frame `pending_double` record set by the double-click branch (FX opened / reel toggled / level reset), the `triple_clicked()` branch calling `region_from_event` and undoing the view effect (D3). Verify t927, t928 green and t206, t207, t636 still green
  - `KtApp::pending_double` + `timeline::DoubleEffect { OpenedFx, ToggledNest, LevelReset }`; the
    triple-click branch after the double-click one (both may arrive in one frame or in two). t927,
    t928, t206, t207, t636, t392 green.
- [x] 3.2 Catalogs: `hint.clip` names the triple click, `tl.menu.region_from_event_accel` shows the triple click, all five languages; run `python3 scripts/subset-fonts.py` if a new CJK/Arabic glyph appears. Verify t929, t125, t100, t128 and the hint guards of `ui-reach` (t83 family) are green
  - `tl.hint.clip` names the triple click and `tl.menu.region_from_event_accel` is "3×click" in
    five catalogs; no new glyphs (the CJK/Arabic words already existed in the subsets). t929, t125,
    t100, t128, t83 green.
- [x] 3.3 Run `reach_tests.rs` and `layout_tests.rs` whole, then commit stages 1–3 (`cargo test --workspace --no-fail-fast` green, `cargo clippy -p kt-app --all-targets` without new warnings)

## 4. Docs and live

- [x] 4.1 `docs/guide/shortcuts.md`: the Ctrl+C row states the three rules in order ("the selected event when exactly one is selected; otherwise what lies in the region; otherwise the selected events"), a new row "triple-click an event — region over the event and its linked partners", the Shift+double-click row kept; `docs/guide/timeline.md` gesture table gets the triple click. Verify the guide guard t826 and t869 (`src/apps/kt-cli/tests/readme_guard.rs`, `src/scripts/readme-check.py`) are green
  - The Ctrl+C row states the three rules and that the status line names the one taken; a
    "triple-click an event" row, Shift+double-click kept as "the same region"; timeline.md's
    gesture table and the region paragraph. `readme-check.py` and `readme_guard` (t826, t869) green.
- [x] 4.2 Live scenario `src/scripts/live/select.py` (extend it): region by ruler drag, click one event, Ctrl+C, Ctrl+V at empty time — one event pasted whole; triple-click another event — region band over it, no FX window. Verify `./scripts/live-check.sh select` green, screenshots listed here
  - `select.py` steps 05–08: a region dragged over both events, one clicked, Edit → Copy (the
    virtual screen has no system clipboard — the menu calls the same body), the clipboard holds one
    event; Edit → Paste at 12.5 s (the view holds ~13 s; a click at 14 s went off-screen) puts it
    whole (4 s); a triple click on the second event — the region exactly 8–12 s, the FX window its
    second click opened is gone. `kt_live.triple_click` reuses `double_click(n=3)`, so the declared
    sleep stays 11.18 s. Screenshots `target/live-out/select/05-step-region-over-both,
    06-step-copied-one, 07-step-pasted-whole, 08-step-triple-region`. Green.
- [x] 4.3 Commit stage 4

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` and the live screenshots here
- [ ] 5.2 After archiving: set F-TL-013 to `done` with its frozen verdict and `CAPABILITY:`, and append to `openspec/specs/ui-timeline/design.md` the "why" of D1 (link groups counted, the range-marker consequence, Vegas/Premiere rejected) and D3 (no delayed double click)

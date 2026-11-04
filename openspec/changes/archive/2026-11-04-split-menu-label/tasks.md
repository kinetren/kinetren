# Tasks

Test number t870 is reserved by this change. Every stage ends with a commit (English
message, first line = the stage and what was done, no `Co-Authored-By` or other agent lines).
Stage only this change's files: other sessions work in the same checkout.

## 0. Records before code

- [x] 0.1 Append to `docs/user-feedback.md`, before "What the user repeated more than once", the section "Split menu label (2026-10-26)". It records the finding (Edit → "Split / cut region" promises a cut-out that neither it nor `S` does; found by `guide-shortcuts-reference`) and the answer (AskUserQuestion, 2026-10-26): *Rename to "Split at playhead"*, keeping `S` and the action (item 5). Verify the entry names `split-menu-label` and has no Cyrillic
- [x] 0.2 Commit stage 0 with the change's artifacts (`openspec validate split-menu-label --strict` green)

## 1. Test first

- [x] 1.1 Write t870 in `src/apps/kt-app/src/layout_tests.rs` as in design D3, on `harness_three_clips_dt`. Set a region over part of event 1, the playhead inside it and nothing selected. Edit menu → the item labelled `format!("{}\t{}", tr(k::tl::menu::SPLIT), "S")` → click. Assert one more event, both halves meeting at the playhead, the same region and the same total duration. Undo restores the ranges; `S` gives identical ranges. Then, for every `Lang::ALL`, assert the split label has no chord. Verify t870 is RED on the current code, and record the failure message here. Recorded: `the Edit menu has no “Разрезать по курсору (S)\tS” item` — visible `Разрез / вырез региона\tS` (Russian harness, before the fix)
- [x] 1.2 Confirm no live scenario or test names the old label: `grep -rn "cut_region\|Split / cut\|CUT_REGION" src/scripts src/apps/kt-app/src` lists only `toolbar.rs:452`. Record the result here. Recorded: only `toolbar.rs:452` and t870's own doc comment use the key or the old text. Found in passing: `tb.cut` = "Split / cut region (S)" in all five catalogs, the toolbar scissors hint over the same `cut_action`. The owner said to fix it here (AskUserQuestion, 2026-10-26), see 2.2b

## 2. One name, chord in the chord column

- [x] 2.1 `toolbar.rs:452`: `Self::menu_btn(ui, tr(k::tl::menu::SPLIT), "S")`. Remove `cut_region` from `[menu.edit]` in `en.toml`, `ru.toml`, `es.toml`, `zh-Hans.toml` and `ar.toml`. Verify `cargo build -p kt-app` succeeds (the catalog build accepts the removal in all five)
- [x] 2.2 `tl.menu.split` without the chord in all five catalogs (en "Split at playhead", ru "Разрезать по курсору", es "Dividir en el cursor", zh-Hans "在播放头切分", ar "تقسيم عند المؤشر"). In `timeline.rs:5891`, draw the item with `crate::widgets::menu_item(ui, tr(k::tl::menu::SPLIT), Some("S"))`, keeping the select-then-`cut_action` body. Verify t870 is green
- [x] 2.2b `tb.cut` in all five catalogs = the new split label + its key: en "Split at playhead (S)", ru "Разрезать по курсору (S)", es "Dividir en el cursor (S)", zh-Hans "在播放头切分（S）", ar "تقسيم عند المؤشر (S)". t870 checks per language that it contains the split label and names `S`. Verify t870 green
- [x] 2.3 Run the mandatory kt-app guards for a UI text change: `i18n_tests.rs` (t125 unused keys, t128 lengths, t171), `font_tests.rs` (t100), `reach_tests.rs` (incl. t799 for t870's PATH line), `layout_tests.rs`. Verify all green, except failures that belong to other open changes; name any such failure here. Recorded: green; the one red is t799 on `openspec/changes/ffmpeg-9/specs/ui-reach/spec.md` (that change's scenario without a PATH line); t870's own PATH line passes
- [x] 2.4 Commit stages 1–2 (`cargo test --workspace --no-fail-fast` and `cargo clippy -p kt-app --all-targets`: no new warnings). Recorded: 1132 passed, 1 failed (t799, ffmpeg-9); clippy 27 warnings before and after, none in the changed lines

## 3. Finish

- [x] 3.1 Run `./scripts/finish.sh` from `src/` (the script resolves its helpers by `$0`, so it must be started from there). Verify it is green and record the version `vYY.MM.DD-hash` here. The report names the new label in each language and any other menu label found during 1.2 that carries a chord in its text (as proposals, not edits) Recorded: version kt26.10.26-1b830567 (debug); workspace 1132/1133, the one red is t799 on the ffmpeg-9 change's spec; live scenarios 47, green (formula, nestparam, conform, corner on the second try, the live run 909 s against 562 s earlier: parallel load, none touches the split); "uncommitted changes" = the fixture index DB and `src/kinetren/`, untouched by this change. Other menu labels with a key in their text (`[tl.menu]`, drawn by `ui.button`, not by the chord helper; out of this change's scope, reported as proposals): `copy` "Copy (Ctrl+C)", `paste` "Paste (Ctrl+V)", `remove_morpher` "Delete morpher (Del)", `delete_in_region` "Delete clips in the region (Del)", `region_from_event` "Region over this event (Shift+double click)". Hints with keys (`tb.*`) are allowed by rule 3.

# Tasks

Test number t871 is reserved by this change. Every stage ends with a commit (English message,
first line = the stage and what was done, no `Co-Authored-By` or other agent lines). Stage only
this change's files.

## 0. Records before code

- [x] 0.1 Append to `docs/user-feedback.md`, before "What the user repeated more than once", the section "Menu labels without keys (2026-10-26)". Record the request ("do this task here") and the answer (AskUserQuestion, 2026-10-26): the gesture goes to the chord column, translated like "Enter, 2×click". Verify the entry names `menu-chords-out-of-text` and has no Cyrillic
- [x] 0.2 Commit stage 0 with the change's artifacts (`openspec validate menu-chords-out-of-text --strict` green)

## 1. Test first

- [ ] 1.1 Write t871 in `src/apps/kt-app/src/reach_tests.rs` (not `layout_tests.rs`: another session has uncommitted edits there, and this checkout cannot stage single hunks) as in design D3 (catalog scan, the two menus in the harness, `morpher_del_accel`). The function does not exist yet, so add a stub that returns `None` for step 3. Verify t871 is RED on the current catalogs, and record the message here

## 2. Labels and chords

- [ ] 2.1 Remove the key from `tl.menu.copy`, `paste`, `remove_morpher`, `delete_in_region` and `region_from_event` in all five catalogs. Add `tl.menu.region_from_event_accel` (en "Shift+2×click", ru "Shift+2×клик", es "Shift+2×clic", zh-Hans "Shift+双击", ar "Shift+نقرة مزدوجة"). Verify `cargo build -p kt-app` succeeds
- [ ] 2.2 In `timeline.rs`, draw the five items through `crate::widgets::menu_item` with the chords of design D1, and implement `morpher_del_accel`. Verify t871 is green
- [ ] 2.3 Run the UI-text guards: `i18n_tests.rs` (t125, t128, t171), `font_tests.rs` (t100), `reach_tests.rs` (t253 Delete morpher reach, t799 for t871's PATH line), `layout_tests.rs`. Verify all green, except failures that belong to other open changes; name any such failure here
- [ ] 2.4 Commit stages 1–2 (`cargo test --workspace --no-fail-fast`; `cargo clippy -p kt-app --all-targets` with no new warnings)

## 3. Finish

- [ ] 3.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version here

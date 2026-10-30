# Tasks

Round R.72 of the claims registry (`docs/ktprompts/v26.10-index.md`, stage R.72). Each section ends with
its tests green and a commit in English. A gesture item also needs its live scenario
(`src/scripts/live/*.py`, `kt_live.step` per step). The rows of the registry move to `done R.72, tNNN`
when the section closes.

## 0. Before code

- [ ] 0.1 Append the round start to `docs/user-feedback.md` (R.72: the owner's items U2–U7, T1–T4b, T6, T11, M5, S1 quoted from the source, his R.71 answers 7–9 referenced) and run `git log -S SPACE_HOLD_MS -S RAIL_DB_SPAN -S LEVEL_GRAB_PX` to confirm none of the touched constants was set at his request (design.md §1, §3). If one was, stop and ask him. Check: the section exists; the log result is noted in it.

## 1. Space bar (transport)

- [ ] 1.1 The press state machine: tap / hold-from-pause / hold-during-play, the Ctrl modifier read at press, `SPACE_HOLD_MS = 300` with its reason comment. Check: t1000 and t1001 (`layout_tests.rs`) green; the existing space tests (`t629_space_boost_from_small_base`, layout_tests 1332–1460) still green.
- [ ] 1.2 Ctrl+Space reverse while held, and the release back to +base. Check: t1002.
- [ ] 1.3 The ease-in and the release decay (analytic, a quarter of the rise's time constant), with dt 1/60, 1/30, 0.1. Check: t1003.
- [ ] 1.4 `docs/guide/shortcuts.md`: hold Space from pause, Ctrl+Space; the live scenario `space_hold.py` with steps. Check: `readme-check.py --check` green, the scenario green with screenshots.

## 2. Timeline gestures (ui-timeline)

- [ ] 2.1 `event_zone`: one function for press and hover, trims before level, level by row height and width. Check: t1008, t1009; t157 (top strip) and the trim/fade tests stay green.
- [ ] 2.2 Event gain to the floor (upper half unchanged), the menu field floor shown as `mute`, the chip after the source name; `audio-render`: the floor is ×0. Check: t1010, t1011; t255 (truncation) green.
- [ ] 2.3 The range-marker end: nearest hit, hover hint `tl.hint.marker_end`, snap without its own edges. Check: t1012.
- [ ] 2.4 The vertical zoom anchor over real row heights, and `scroll_max` from real heights. Check: t1013; t47/t50 (geometry) green.
- [ ] 2.5 Reproduce live where the left jump zone was lost to the badges (screenshots before), then: the scrollbar grab (12 px, no-resize, a click pages), the right zone ends before it, the left zone whole on badge rows. Check: t1014; t101 green.
- [ ] 2.6 In a jump zone a click jumps over an event, and the second quick click is a jump; a drag from the zone still moves the event. Check: t1015; the live scenario `jump_zone.py`.
- [ ] 2.7 Remove `tl.lane.add_morpher_here` / `tl.lane.add_camera_here` from all five catalogs and from `empty_svc`; the guard. Check: t1016; t125 (unused keys) and t100 (glyphs) green.

## 3. Reach and transport commands

- [ ] 3.1 Track reorder cue: grab cursor and `tl.hint.lane_reorder` over the gutter's free area. Check: t1017 drags from the name and from above the badges.
- [ ] 3.2 Frame step: `frame_step(dir)` for ← / → and Ctrl+click on Back/Forward; not while a text field has focus; shortcuts.md rows. Check: t1004; t82 (stop points) green.
- [ ] 3.3 The engine records one stall per episode and none at rate 0; the application posts one warning per episode to the bell. Check: t1005 (`kt-engine/tests/flight_stall.rs`), t1006.
- [ ] 3.4 "Insert frame hold (freeze frame)" in the event menu: three pieces, one batch, length kept, the last piece from the frozen frame, a curve case. Check: t1007; t252 (no orphan commands) and t253 (verbs) green; the live scenario `freeze_frame.py`.

## 4. Registry and finish

- [ ] 4.1 Move the 17 R.72 rows of `docs/ktprompts/v26.10-index.md` to `done R.72, tNNN` (U7 keeps the open item split as `partial` with the screenshots for the owner). Check: `python3 src/scripts/ktprompts-index.py --check` green, every named test exists.
- [ ] 4.2 `cargo test --workspace` and `cargo clippy --all-targets` green. Check: both exit 0.
- [ ] 4.3 `./scripts/finish.sh` — the version `vYY.MM.DD-hash` and the list of live steps in the report; `python3 scripts/target-prune.py` after it.

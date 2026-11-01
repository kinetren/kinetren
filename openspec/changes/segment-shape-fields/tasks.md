# Tasks: segment-shape-fields

## 1. Shared control (widgets.rs)

- [ ] 1.1 `shape_param_bounds` — one table of tension/periods bounds; `shape_dragged` (automation.rs) uses it
- [ ] 1.2 `shape_fields` over a set of segments: mixed display, typed = absolute, drag = delta, pick keeps same-kind parameters, gesture-only writes (`clamp_existing_to_range(false)`); `shape_all_control` becomes a thin call over all segments — t1120 (red on the old row), existing t696b/t697 guards stay green

## 2. Selection in the widget (automation.rs)

- [ ] 2.1 `SegSel` keyed by left-key ticks replaces `sel_seg`; formula field reads "one formula segment"
- [ ] 2.2 Gestures: right click (selection-aware), Ctrl+click toggle, Shift+click run, sweep drag on empty space, plain click clears, curve switch clears; tint of selected segments; sweep band — t1118
- [ ] 2.3 Segment menu acts on the selection, header with the count — t1119
- [ ] 2.4 Selection row in `curve_block` under the "all" row, caption, "×" clear — t1115, t1116, t1117, t1121
- [ ] 2.5 Hover hint over a segment names the gestures; new keys in all five catalogs — t1122

## 3. Live and docs

- [ ] 3.1 Live scenario `src/scripts/live/segfields.py` (steps with screenshots, our fixture), registered in live-check
- [ ] 3.2 `docs/guide/whats-new.md` line (kt26.11); registry row K10 closed (`python3 src/scripts/ktprompts-index.py --check`)
- [ ] 3.3 `cargo test -p kt-app`, `cargo clippy --all-targets`, full `cargo test --workspace`; `git merge master`; `src/scripts/finish.sh` — version here

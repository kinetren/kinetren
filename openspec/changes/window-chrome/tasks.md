# Tasks: window-chrome

> DESIGN ONLY until the owner answers Q1 and the orchestrator's go. Tests: t3935–t3941 (t3942 spare).

## 0. Notes

- [x] 0.1 proposal, design (D1–D8, class walk), deltas `ui-timeline`, `ui-reach`; `openspec validate window-chrome --strict`
- [ ] 0.2 the owner's answer folded in

## 1. Layers (W1, T9)

- [ ] 1.1 the under-windows layer; the glow as one mesh per glowing thing; the status hint and the title moved; t3936
- [ ] 1.2 the guard of `Foreground`/`Tooltip` uses with reasons; t3935

## 2. Window gestures (T5, U10, U11)

- [ ] 2.1 the WM's maximized state at start; restore to a normal size before a drag; t3937, live `max_drag`
- [ ] 2.2 the toolbar's wheel offset; t3938
- [ ] 2.3 drag a window by the preview where no gesture claims it; t3939

## 3. Event windows (W3, T8, T7)

- [ ] 3.1 `event_title`, the number in the head row; i18n ×5; t3940
- [ ] 3.2 unindented folds; icon copy/paste with hints (Phosphor glyphs checked); t3941

## 4. Close

- [ ] 4.1 live `window_over_glow`, a screenshot per step; idle unchanged
- [ ] 4.2 What's new; registry U10, U11, T5, T7, T8, T9, W1, W3 moved
- [ ] 4.3 `src/scripts/finish.sh` (orchestrator), version in the report

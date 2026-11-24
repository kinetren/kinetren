# Tasks: ripple-and-trim-modes

> Q1 decided (A9, check C9); Q2 and Q3 asked (two defaults, D1 `refs` and D3's triggers). Revision 2 implementation-ready at
> `229c694b` (2026-11-24). Code starts on the orchestrator's go, after `multicam` (TIMELINE-UI). Tests: t3415–t3432, t4239–t4242.

## 0. Notes

- [x] 0.1 F-TL-001, F-TL-002, F-TL-008; proposal, design (D0–D8), deltas `ui-timeline`, `ui-reach`, `morpher`;
      `openspec validate ripple-and-trim-modes --strict`
- [x] 0.2 his answer Q8 (kkt-26.11.20 line 14)

## 1. The model (kt-model)

- [ ] 1.1 `RippleScope`, `ops::ripple_plan` over `Indexed`: the vertical line, link groups once, one reel, locked tracks, the
      spanning refusal, markers ordered, ranges and areas stretched, morphers, cameras; t3415–t3418, t4239, t4240
- [ ] 1.2 `MoveForeign`, `SetTimelineEnd`, `Slip` (with `target_rows`, `touches_render_plan`, telemetry's label); t3415, t3426
- [ ] 1.3 rippled delete with `restore_handles` first; the rippled region cut by `Split` + `TrimEnd`; t3419, t3420, t3421
      (`kt-engine/tests/ripple_render.rs`)
- [ ] 1.4 slide and roll batches with their clamps; `ops::close_gaps`; t3428, t3429, t3430
- [ ] 1.5 the reference-layer guard; t3432
- [ ] 1.6 `TimelineModel::amend_batch`; t4241

## 2. The interface (kt-app)

- [ ] 2.1 `Cmd::{RippleTracks, RippleAll, AutoRipple, CloseGaps}` (the next free numbers, append only; `CMD_COUNT`), `OWN`: F and
      Ctrl+Shift+F, t1700 and the key list; Edit menu, the event's menu; t3431
- [ ] 2.2 the footprint, the disabled item with its reason; t3422
- [ ] 2.3 the toolbar button, the red outline and playhead, the mode's hint; t3423, t3424
- [ ] 2.4 slip, slide and roll: modifiers at the press, live drawing, red stops; refusals highlight the obstacle and say
      where it is (rule 16); t3427, t4242
- [ ] 2.5 i18n ×5, font subsets, `shortcuts.md`; live `ripple` with a screenshot per step (t3425)

## 3. Close

- [ ] 3.1 What's new ("closing a gap: a button that shifts the rest after every cut, one key to do it afterwards; slip, slide, roll,
      close gaps"); F-TL-001/002/008 verdicts; P1 ★1, v26.10 N2/U9 and A9 moved; the region cut's and slip's rules in kt-model
      `CLAUDE.md`; C9 put to him with the release
- [ ] 3.2 idle with the mode on (live `idle`); `finish.sh` (orchestrator)

# Tasks: scopes

> Decided 2026-11-20 (A8). Implementation-ready at `229c694b` (2026-11-24). Code starts on the orchestrator's go.
> Tests: t3400–t3405, t3410–t3412.

## 0. Notes

- [x] 0.1 F-CLR-005 refreshed; proposal, design (D0–D8), deltas `scopes`, `ui-reach`; `openspec validate scopes --strict`
- [x] 0.2 his answers Q6–Q7 in `kkt-26.11.20.txt` line 13, row A8

## 1. The tap (kt-engine `video_plan.rs`, `compose_queue.rs`; kt-gpu `engine.rs`)

- [ ] 1.1 `PresentOptions.tap`, `TapFrame`, `VideoGpuExec::take_tap`; two buffers, the copy in `present` and `present_kept`,
      the ledger row; t3400 (`kt-gpu/tests/preview_display.rs`)
- [ ] 1.2 the compose thread hands it on (`on_display`, bytes, the last tap on a pause), `SCOPE_TAP` in the trace; t3402, t3404
      (`kt-engine/tests/compose_queue.rs`)

## 2. The paused frame (kt-app `fx_thumbs.rs`, `scopes.rs`)

- [ ] 2.1 `SceneCtx::frame` extracted, the picker on it (its tests green); the `scopes` lane and its key; t3401

## 3. The instruments and false colour

- [ ] 3.1 `scopes.rs`: counts, images, the service thread, `Work::ScopeTap`/`ScopeCount`; t3403
- [ ] 3.2 `display.wgsl` mode 1 and its CPU twin; t3405 (`kt-gpu/tests/preview_display.rs`)

## 4. The window

- [ ] 4.1 the window, its `WINDOWS` row, View and the preview menu (one body), `Workspace.scopes`, the badge and the Esc
      step; i18n ×5, font subsets; `docs/guide/shortcuts.md` untouched (no key); t3411; `layout_tests` (the head does not jump)
- [ ] 4.2 live `scopes` with a screenshot per step (t3412); `idle.py`'s step (t3410); `KT_PREVIEW_READBACK=1` and
      `WGPU_BACKEND=gl` runs of `scopes`

## 5. Close

- [ ] 5.1 What's new ("the program shows a waveform, parade, vectorscope and histogram of the picture, and paints
      clipped places over it"); F-CLR-005 verdict; P1 ★8 and A8 moved; kt-gpu and kt-app `CLAUDE.md` (the tap never waits;
      a new display path hands its picture to the tap)
- [ ] 5.2 idle measurement with the window open on the heavy showcase in the report; `finish.sh` (orchestrator)

# Proposal: class-walk-revision

## Why

The owner, 11.11 (`docs/ktprompts/kkt-26.11.11.txt` L3; earlier kkt-26.11.10b K4 "а ты когда фиксы
всякие делал - мы по другим местам проблемы проверяли ти или нет??"): "п.3 - надо проверять по другим
местам может где забыли еще".

Every change of stages 9–10 kept a class walk in its `design.md`, yet later runs found places each one
missed: one-scheduler made the save async but not its callers ("Next lesson", "Save and continue") and
parked the audio thread without walking StallWatch; memory-ledger left the frames cache on a constant;
stream-layout sized the memo by the loop only; stream-history-on-device called the ring wait
"intended"; local-cost-queue kept the weakest record. The walks listed the SUBSYSTEMS where the
mechanism lives (threads, tenants, kernels), not the CALLERS and CONSUMERS that relied on the old
behaviour.

Mechanism of the miss: a walk enumerates the places that HAVE the mechanism, and stops; the code that
waits for, reads, sizes from or bypasses what the change moved is not enumerated, so its old assumption
survives.

## What Changes

- Revise the class walks of the eleven archived changes of 2026-11-09 … 2026-11-11 against the code:
  for each, its mechanism in one line, then a grep of its functions, types, constants, flags and trace
  kinds across `src/` for every caller, consumer and sibling path; the table goes into this change's
  `design.md`.
- Fix the misses that are the same defect, each with a regression test red on the old code
  (t2450–t2459):
  - the run prewarm warms the pieces a frame asks (a run longer than 32 stages);
  - the stream memo's view applies only to the renderer that set it (a reel, a strip, a poster read
    their walk back);
  - a view-state edit made while Ctrl+S writes keeps its asterisk;
  - closing the window while playing lets the autosave and the poster the close waits for run;
  - the reel strip's envelope pauses with the frames; the strip raises generator sources;
  - the mask pen and the search window count every press of a slow frame; the preview's single click
    waits egui's widened double-click window from the frame start;
  - the stream memo and the frames cache size against the ledger's room, not the bare available;
  - the preview bench, `kt-cli preview` and `pause-scan` name a frame behind its stream history;
  - `kt-cli stream-cost` sizes the memo as the player does; `pause-scan` opens decoders by the
    render's thread rule; the export's stream memo size is one constant.
- Fix found-defects 463 (t2230 raced when several workers freed at once).
- Record the rest as found-defects rows 510–524.
- `docs/engine-audit.md`: a section "Class-walk revision 2026-11"; `docs/orchestrator.md` "Briefs for
  executors": every brief asks for the CALLERS walk explicitly.

## Capabilities

### Modified Capabilities
- `video-plan`: run prewarm by pieces, the stream view per renderer, the strip's generators and paused
  envelope, the memo sized by the room, the bench names a catch-up.
- `durability`: a view edit during a write keeps its asterisk; closing while playing.
- `ui-timeline`: every press of a slow frame in the pen and the search; the preview click window.

## Impact

`kt-gpu` engine.rs (prewarm only — not the arbiter), `kt-engine` video.rs / video_stream.rs /
prefetch.rs / preview_bench.rs / export*.rs, `kt-time` mem.rs and the sched test, `kt-app` app.rs /
reel_preview.rs / maskctl.rs, `kt-cli` main.rs / streamcost.rs. No format change, no OTIO loss.

# Proposal: render-result-check

> DESIGN ONLY (spec session), base orch `5986f08e`. What T13 of `v26.10-index.md` still owes. Test t3998.

## Why

The owner's item, whole, `docs/ktprompts/v26.10.txt` line 55: "13. в окне рендера может быть в конце video 517/516 audio
412672/413364 - не понятно как так, фреймов на 1 больше, а аудио меньше и при этом оно уже отрендерилось."

The video half was fixed (`render-crash-and-recovery`, t1070: the total is the rendered frames). The audio row left the
window, so its half was hidden, not proved: nothing checks that the samples written are the samples of the range, nor
that the FILE holds what the window says. Both numbers end at their totals or the result says why.

## What Changes

- The export's result carries what it announced beside what it wrote (frames and samples), and a difference is a named
  line of the result (in the window's final block and in the telemetry journal), never a silent pair of numbers.
- The journal's "done" line names both pairs.
- The live scenario `renderregion` (an off-grid region, the case of his numbers) reads the finished window, the journal
  and the file: the frame count and the sound's length by `ffprobe`.

## Capabilities

### Modified Capabilities
- `export`: the finished render's numbers equal the announced ones and the file's.

## Impact

kt-engine (`export.rs` `ExportStats` gains the two totals), kt-app (`render_dialog.rs`: the line when they differ),
live `renderregion`; i18n ×5 (one line). No format change. Idle: nothing.

## Questions to the owner

None.

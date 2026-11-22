# What the owner is asked to check

The orchestrator's requests to the owner, in one place (owner, 2026-11-20: "свои просьбы запиши в
отдельный список того что мне надо проверять и для себя там же пиши связи чтобы помнить куда пойти
и что чинить по моим замечаниям").

Each row has two parts:
- **Check** — what to do and on which release. It is written for him.
- **Where to go** — the orchestrator's own links: the claim, the found-defects row, the change, and
  the executor. When his answer arrives, the work starts from there.

When his answer or trace arrives:
1. Register it (`docs/ktprompts/`, `docs/user-feedback.md`).
2. Move the row to "Answered" with a pointer to where it went.

Rows nobody has answered stay open across releases. A newer release replaces the release named in
a row; it does not close the row.

## Open

| # | Since | Check (for him) | Where to go (for the orchestrator) |
|---|---|---|---|
| C1 | 2026-11-19, kt26.11.20-ea0aff6f | **problem_region loop.** Play the loop on `problem_region` on the new release and send the `<data>/trace` folder. Is it complete earlier than lap ~15? Do the processor and the card work? | kkt-26.11.18 T2 → ahead-kept (t2930–t2932); `kt-cli trace loop`; found-defects 741, 776; executor mosh-sharp (prerender/hw) |
| C2 | 2026-11-19, kt26.11.20-ea0aff6f | **Simple region render speed.** Render a simple region and send the trace. Is it still ~1× real time? | kkt-26.11.18 T3 → export-readback (t2960–t2962); found-defects 740, 741 (VAAPI surface handoff unmeasured on his card); `kt-cli trace … export` |
| C3 | 2026-11-19, kt26.11.20-ea0aff6f | **Close while catching up.** If closing ever hangs again, send the trace; `kt-cli trace … shutdown` names the wait. | kkt-26.11.18 T1 → close-while-busy (t2950–t2953, live `close_busy`); found-defects 751, 791 |
| C4 | 2026-11-19, kt26.11.20-ea0aff6f | **Datamosh Sharp.** Datamosh at 144p with "Sharp" on: is the break kept and the picture sharp at the project's size? New instances start with Sharp on. | kkt-26.11.18b U1, U2 → mosh-sharp (t2970–t2975, live `datamosh_sharp`); F-FX-015; found-defects 761, 767, 768 |
| C5 | 2026-11-19, kt26.11.20-ea0aff6f | **Disk prerender.** After a restart, does a range he already played come back without recomputing? Is the hardware encoder found on his machine? The log names it. | kkt-26.11.18b U4 → render-cache-disk; found-defects 770, 773–776; executor mosh-sharp (hw decode, branch `hw-segment-decode`) |
| C6 | 2026-11-19, kt26.11.20-ea0aff6f | **Effects graph.** Does the graph in the effects window and the morpher (v1a–v1c) look and work the way he wanted? Are sliders in nodes and Camera ISP compact? Does it fit without scrolling? | kkt-26.11.18b U3, U7, U8 → change `fx-graph` (design F-UI-008); executor fx-graph |
| C7 | 2026-11-19, kt26.11.20-ea0aff6f | **Small features.** Check these: scrubbing and dragging from the media bin, cutting and copying a region, marker notes, the kind galleries, curve keys with Shift, the eyedropper, and the effects centre on the preview. | kkt-26.11.18b U6, U9; v26.10 A1, MW1 → executor features-slow |
| C8 | 2026-11-20, kt26.11.20-ea0aff6f | **Shapes generators.** Polygon, star, arrow, border frame, lower-third plate, progress bar: does each look right over footage (the frame around a shape is transparent)? | F-TXT-009 → gen-shapes (t3709–t3710, live `gen_shapes`); executor features-slow |
| C9 | 2026-11-20 | **Ripple across an edit point** (his answer: "запиши на будущее чтобы я проверил"). When ripple editing ships: delete a piece on track 1 while a music bed on track 3 spans the cut — the program refuses, highlights the bed and says "split it or ripple affected tracks only". Does it read right? | kkt-26.11.20 A9 → change `ripple-and-trim-modes` Q1 a |
| C10 | 2026-11-20 | **D3D11VA on a real Windows card.** Under wine the Windows path compiles and refuses with its reason; a real card is needed to prove the hardware decode. If he runs the Windows zip: does the log name D3D11VA? | found-defects 775 → branch `d3d11va-segment-decode` (merged); executor fx-graph |
| C11 | 2026-11-22 | **Render through dma-buf on his card** (when A13 lands). Render a region with `KT_EXPECT_HW=vaapi` and send the trace: does the frame go into VAAPI without a copy through memory? There is no VAAPI device on the build machine, only a fake driver. | kkt-26.11.20 A13 → change `export-host-copy`; executor mosh-sharp |

## Questions waiting for his decision

None open (2026-11-20: Q1–Q54 answered in chat, see Answered).

## Answered

| # | Asked | Answer | Went to |
|---|---|---|---|
| — | 2026-11-18 | fx-graph questions (layout, sliders, boxes, sound) | kkt-26.11.18b lines 26–29, U8 |
| — | 2026-11-19 | A1 (curve keys) and MW1 (bin drag) | kkt-26.11.18b lines 31–32, U9 |
| Q1–Q54 | 2026-11-20 | answered in chat; verbatim `docs/ktprompts/kkt-26.11.20.txt`, rows A1–A26 | `docs/ktprompts/kkt-26.11.20-index.md`; the questions themselves in git history of this file (c5b9eb30) |
| 2nd round | 2026-11-20..22 | answered in chat; verbatim `docs/ktprompts/kkt-26.11.20b.txt`, rows B1–B12 | `docs/ktprompts/kkt-26.11.20b-index.md` |

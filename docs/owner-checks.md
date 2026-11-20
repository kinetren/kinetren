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
| C1 | 2026-11-19, kt26.11.19-732d7d7f | **problem_region loop.** Play the loop on `problem_region` on the new release and send the `<data>/trace` folder. Is it complete earlier than lap ~15? Do the processor and the card work? | kkt-26.11.18 T2 → ahead-kept (t2930–t2932); `kt-cli trace loop`; found-defects 741, 776; executor mosh-sharp (prerender/hw) |
| C2 | 2026-11-19, kt26.11.19-732d7d7f | **Simple region render speed.** Render a simple region and send the trace. Is it still ~1× real time? | kkt-26.11.18 T3 → export-readback (t2960–t2962); found-defects 740, 741 (VAAPI surface handoff unmeasured on his card); `kt-cli trace … export` |
| C3 | 2026-11-19, kt26.11.19-732d7d7f | **Close while catching up.** If closing ever hangs again, send the trace; `kt-cli trace … shutdown` names the wait. | kkt-26.11.18 T1 → close-while-busy (t2950–t2953, live `close_busy`); found-defects 751, 791 |
| C4 | 2026-11-19, kt26.11.19-732d7d7f | **Datamosh Sharp.** Datamosh at 144p with "Sharp" on: is the break kept and the picture sharp at the project's size? New instances start with Sharp on. | kkt-26.11.18b U1, U2 → mosh-sharp (t2970–t2975, live `datamosh_sharp`); F-FX-015; found-defects 761, 767, 768 |
| C5 | 2026-11-19, kt26.11.19-732d7d7f | **Disk prerender.** After a restart, does a range he already played come back without recomputing? Is the hardware encoder found on his machine? The log names it. | kkt-26.11.18b U4 → render-cache-disk; found-defects 770, 773–776; executor mosh-sharp (hw decode, branch `hw-segment-decode`) |
| C6 | 2026-11-19, kt26.11.19-732d7d7f | **Effects graph.** Does the graph in the effects window and the morpher (v1a–v1c) look and work the way he wanted? Are sliders in nodes and Camera ISP compact? Does it fit without scrolling? | kkt-26.11.18b U3, U7, U8 → change `fx-graph` (design F-UI-008); executor fx-graph |
| C7 | 2026-11-19, kt26.11.19-732d7d7f | **Small features.** Check these: scrubbing and dragging from the media bin, cutting and copying a region, marker notes, the kind galleries, curve keys with Shift, the eyedropper, and the effects centre on the preview. | kkt-26.11.18b U6, U9; v26.10 A1, MW1 → executor features-slow |

## Questions waiting for his decision

| # | Since | Question (with context) | Where to go |
|---|---|---|---|
| Q1 | 2026-11-20 | **Generator from the Edit menu and the media bin.** On a track, the right-click menu now has one item ("Create a generator…"). On a video track it places a solid colour, on an audio track a tone, and the kind is changed in the generator window. The Edit menu and the media bin have no track, so they still show the list of kinds. Should they also have one item? If so, which kind: a solid colour, or by the selected track? | v26.10 G1 → branch `generator-from-track`; executor features-slow |
| Q2 | earlier | **Rename the menu item "Рендер в MP4…" to "Рендер…"?** The render writes more than MP4. | export UI; nobody has taken it yet |
| Q3 | 2026-11-20 | **Graph v2, order of delivery.** v2 lets a chain branch and merge (blur only the highlights and lay them back, parallel compression). (a) recommended: picture branches first; a branch on sound refuses with a reason until the sound stage; ships sooner. (b) picture and sound together; nothing ships until the sound graph is ready. | `openspec/changes/fx-graph-dag` (design b61bef6e), stages v2a–v2d; executor fx-graph |
| Q4 | 2026-11-20 | **Graph v2, inputs from outside the layer.** Today an event's chain sees only its own picture. (a) recommended: not in v2; a Merge mask comes only from its own picture; loading, coverage bars and keys stay as they are. (b) a mask from another track (Track Matte in Premiere/AE): the event then waits for that track's frame, prefetch reads both, an edit there re-renders this event; a step toward compositing between layers (v3). | `fx-graph-dag` design, question 2; executor fx-graph |
| Q5 | 2026-11-20 | **Graph v2, Slowed / Datamosh / pitch inside a branch.** Each runs once per layer today (Slowed picks the source frame, Datamosh decodes its own stream with history, pitch reads past the event's edges). (a) recommended: only before the first split; the editor refuses and says why; one read per layer. (b) allowed in a branch at the cost of a second read: the layer's decode and cache cost double, the bars show two reads, Datamosh keeps a second history. | `fx-graph-dag` design, question 3; executor fx-graph |

## Answered

| # | Asked | Answer | Went to |
|---|---|---|---|
| — | 2026-11-18 | fx-graph questions (layout, sliders, boxes, sound) | kkt-26.11.18b lines 26–29, U8 |
| — | 2026-11-19 | A1 (curve keys) and MW1 (bin drag) | kkt-26.11.18b lines 31–32, U9 |

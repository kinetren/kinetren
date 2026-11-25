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
| C1 | 2026-11-19, kt26.11.24-af14db35 | **problem_region loop.** On the new release: set the region, press Loop and Play on `problem_region` for about 20 laps, then Вид › Диагностика… › «Открыть папку трассы» and send that folder. Is it complete earlier than lap ~15? Do the processor and the card work? | kkt-26.11.18 T2 → ahead-kept (t2930–t2932); `kt-cli trace loop`; found-defects 741, 776; executor mosh-sharp (prerender/hw) |
| C2 | 2026-11-19, kt26.11.24-af14db35 | **Simple region render speed.** Файл › Рендер… with a region set; render it and send the trace folder (Диагностика › «Открыть папку трассы»). Is it still ~1× real time? | kkt-26.11.18 T3 → export-readback (t2960–t2962); found-defects 740, 741 (VAAPI surface handoff unmeasured on his card); `kt-cli trace … export` |
| C3 | 2026-11-19, kt26.11.24-af14db35 | **Close while catching up.** Close the program while the bars are still filling. If it ever hangs again, send the trace folder of that run (Вид › Диагностика… › «Открыть папку трассы» on the next start); the orchestrator's `kt-cli trace … shutdown` names the wait. | kkt-26.11.18 T1 → close-while-busy (t2950–t2953, live `close_busy`); found-defects 751, 791 |
| C4 | 2026-11-19, kt26.11.24-af14db35 | **Datamosh Sharp.** Add Datamosh, «Высота кодирования» 144, «Резко: картинка в разрешении проекта» on (it is on for a new one): is the break kept and the picture sharp at the project's size? | kkt-26.11.18b U1, U2 → mosh-sharp (t2970–t2975, live `datamosh_sharp`); F-FX-015; found-defects 761, 767, 768 |
| C5 | 2026-11-19, kt26.11.24-af14db35 | **Disk prerender.** The disk beside the preview quality must not stand at «Выкл». Play a heavy range, wait until its bars are bright, close and start the program, play it again: does it come back without recomputing? Is the hardware encoder found? Point at the «Кадры на диске» line of Вид › Диагностика…: the hint names the encoder; what reads the segments (the device's decoder) is in the trace folder. | kkt-26.11.18b U4 → render-cache-disk; found-defects 770, 773–776; executor mosh-sharp (hw decode, branch `hw-segment-decode`) |
| C6 | 2026-11-19, kt26.11.24-af14db35 | **Effects graph.** In an event's effects window and a morpher's window (double-click it): does the graph look and work the way he wanted, with branches too? Are the sliders in the nodes and Camera ISP compact? Does it fit without scrolling? | kkt-26.11.18b U3, U7, U8 → change `fx-graph` (design F-UI-008); executor fx-graph |
| C7 | 2026-11-19, kt26.11.24-af14db35 | **Small features.** Check these: scrubbing a bin card under the mouse and dragging it to the timeline; with a region set, «Вырезать интерьер региона» (C) and Ctrl+C; a marker's note (right-click the marker, «Заметка…»); the galleries of effects and transitions; curve keys (Shift+click adds to the selection); the eyedropper beside every colour; the centre of an effect dragged on the preview (the effect's window open). | kkt-26.11.18b U6, U9; v26.10 A1, MW1 → executor features-slow |
| C8 | 2026-11-20, kt26.11.24-af14db35 | **Shapes generators.** In the generator gallery, group «Фигуры»: Многоугольник, Звезда, Стрелка, Рамка, Плашка нижней трети, Полоса прогресса: does each look right over footage (outside the shape it is transparent)? | F-TXT-009 → gen-shapes (t3709–t3710, live `gen_shapes`); executor features-slow |
| C9 | 2026-11-20 | **Ripple across an edit point** (his answer: "запиши на будущее чтобы я проверил"). When ripple editing ships: delete a piece on track 1 while a music bed on track 3 spans the cut — the program refuses, highlights the bed and says "split it or ripple affected tracks only". Does it read right? | kkt-26.11.20 A9 → change `ripple-and-trim-modes` Q1 a |
| C10 | 2026-11-20 | **D3D11VA on a real Windows card.** Under wine the Windows path compiles and refuses with its reason; a real card is needed to prove the hardware decode. If he runs the Windows zip on a real card: send the trace folder (Вид › Диагностика… › «Открыть папку трассы»); it names the decoder of the disk cache's segments, D3D11VA expected. | found-defects 775 → branch `d3d11va-segment-decode` (merged); executor fx-graph |
| C11 | 2026-11-22 | **Render through dma-buf on his card** (when A13 lands). In Файл › Рендер… choose «h264_vaapi» in the video codec list (not Auto), render a region and send the trace folder (no environment variable is needed, that one is for the tests). The render window's log («Открыть журнал») and the trace say how many frames were handed to the encoder on the card, or why not. Look at the result: torn blocks or old frames inside new ones would be the one defect the program cannot see itself. There is no VAAPI device on the build machine, only a fake driver. | kkt-26.11.20 A13 → change `export-into-surface` D12; executor mosh-sharp |
| C12 | 2026-11-24, stage 24's release | **What the sound-reactive generators hear.** Waveform, Spectrum and Level rings read the FILE on the chosen audio track, not the track after its volume, effects and fades: a fade-out leaves the spectrum moving. Is that what he wants, or should the picture follow the sound as it is heard? (Following it makes a render compute its sound first.) | F-TXT-011 → archive `2026-11-21-gen-audio-reactive` design D1 (the orchestrator chose the file; its "Q55" was never put to him, Q55 in chat was the persistent cache) |
| C13 | 2026-11-25, when `hdr-sources-to-sdr` 1–4 lands | **HDR clip from a phone beside an ordinary one.** Put an HDR phone clip next to an SDR one of the same scene: on the preview and in a render: is white as bright as it should be, are the highlights kept, does skin keep its colour? | change `hdr-sources-to-sdr` (spec-engine, 365afcf0) |
| C14 | 2026-11-25, stage 24's release | **The persistent cache on his card.** The disk icon beside the preview quality, «Как в превью» (its sizes and folders are in Настройки программы… › Постоянный кэш): do heavy 4K files get a copy by themselves at rest (the card of the bin shows the mark), does play get smoother after it, does the processor load bother him? Measured only on a software renderer with libx264 copies. Send the trace folder. | kkt-26.11.22 D1, A10, B1 → archived `persistent-cache`; executor mosh-sharp |
| C15 | 2026-11-25, stage 24's release, on wine or Windows | **The frozen loop on Windows.** Start the Windows zip (under wine, as before), set a region over an event, press Loop and Play: does the picture move at once, with no click to wake it? If it freezes, send the trace folder. | kkt-26.10.27 K1 → change `loop-play-latency-hold` (tasks 1.3, 3.5, 4.2); found-defects 16 |

## Questions waiting for his decision

Asked in chat together, his items quoted whole. Numbers start at Q66: the branch `claude/youtube-channel-specs`
(not merged, another session's) holds its own Q55–Q65.

| # | Since | Question (the options and what each changes are in the proposal) | Change |
|---|---|---|---|
| Q66 | 2026-11-24 | `detached-windows` Q1: which windows stand outside (three options, counted) | `openspec/changes/detached-windows/proposal.md` |
| Q67 | 2026-11-24 | `detached-windows` Q2: Wayland places windows itself | the same |
| Q68 | 2026-11-24 | `presets-everywhere` Q1: "взять кривую/пресет … из другого события(ссылка)": a copy or a live link | `openspec/changes/presets-everywhere/proposal.md` |
| Q69 | 2026-11-24 | `presets-everywhere` Q2: which built-in presets first | the same |
| Q70 | 2026-11-24 | `project-scale` Q1: the budget at a million events (sizes computed) | `openspec/changes/project-scale/proposal.md` |
| Q71 | 2026-11-24 | `project-scale` Q2: references for chains, or for whole events too | the same |
| Q72 | 2026-11-24 | `picture-check` Q1: findings as markers | `openspec/changes/picture-check/proposal.md` |
| Q73 | 2026-11-24 | `picture-check` Q2: the flashing check | the same |
| Q74 | 2026-11-24 | `auto-ducking-by-role` Q1: what counts as a title section (four options, an example counted; the recommended one changed on 2026-11-25) | `openspec/changes/auto-ducking-by-role/proposal.md` |
| Q75 | 2026-11-24 | `flatpak-flathub` Open Questions 1–6 | `openspec/changes/flatpak-flathub/` |
| Q76 | 2026-11-24 | `claude/youtube-channel-specs` (five designs of 2026-11-20, Q55–Q65 inside): merge it? Its `loudness-target` keeps the norm in the render settings, against his B6 | spec-features' report of 2026-11-24 |
| Q77 | 2026-11-25 | `download-library` Q4: where the signed list of downloads lives and who keeps its key (holds stage 5 only) | `openspec/changes/download-library/proposal.md` |
| Q78 | 2026-11-25 | `ripple-and-trim-modes` Q2: do markers and named areas move with the "track" mode (A9 answered Q1 only) | `openspec/changes/ripple-and-trim-modes/proposal.md` |
| Q79 | 2026-11-25 | `ripple-and-trim-modes` Q3: does dragging an event ripple while the mode is on | the same |

## Answered

| # | Asked | Answer | Went to |
|---|---|---|---|
| — | 2026-11-18 | fx-graph questions (layout, sliders, boxes, sound) | kkt-26.11.18b lines 26–29, U8 |
| — | 2026-11-19 | A1 (curve keys) and MW1 (bin drag) | kkt-26.11.18b lines 31–32, U9 |
| Q1–Q54 | 2026-11-20 | answered in chat; verbatim `docs/ktprompts/kkt-26.11.20.txt`, rows A1–A26 | `docs/ktprompts/kkt-26.11.20-index.md`; the questions themselves in git history of this file (c5b9eb30) |
| 2nd round | 2026-11-20..22 | answered in chat; verbatim `docs/ktprompts/kkt-26.11.20b.txt`, rows B1–B12 | `docs/ktprompts/kkt-26.11.20b-index.md` |
| Q55–Q58 | 2026-11-22..23 | answered in chat; verbatim `docs/ktprompts/kkt-26.11.22.txt`, rows D1–D5 | `docs/ktprompts/kkt-26.11.22-index.md` |

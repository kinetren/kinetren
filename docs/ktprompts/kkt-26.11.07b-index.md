# Registry of the owner's list `kkt-26.11.07b`

Source — [`kkt-26.11.07b.txt`](kkt-26.11.07b.txt) (verbatim, the owner's second message of 07.11.2026,
on build `kt26.11.07-61414dba`). Columns and states — as in [`v26.10-index.md`](v26.10-index.md). The
round is in `docs/user-feedback.md` ("2026-11-07 — his run of 61414dba"). His files: the folder
`/media/e/developer/kt.9/fails/26.11.07` (`load-report.txt`, `app.ron`, `kt-app.log`, `haski.kt`);
he sent the project after all (line 15); it is his, its media are not on this machine, it is not copied into the tree. Line 19: his media are in
`/media/e/developer/kt.9/sources/untitled2_v26.09.08/sources` (read only); a copy relinked to them is
`/media/e/developer/kt.9/repro/26.11.07/haski-relinked.kt` (27 of 30 files found; Morandi,
`accompaniment.wav`, `vocals.wav` are missing).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| G1 | 2 | on a pause, on an event, moving the mouse over the preview costs 25 % of the processor (not prefetch) | done ui-frame-cost, t1910–t1914 + live `ui_frame`: a frame's work grew with the project (document copies in the bin and the windows, a speed map per event for the tail hatching, the speed curve laid out per pixel column); a frame now asks what is in view. His project, release: 6.1 → 1.6 ms a frame over the preview, 10.1 → 5.2 ms zoomed out, the effects window open 291 → 5 ms (with input-without-work); Diagnostics shows the frame's cost by stage | `ui-frame-cost` | ui-frame-cost |
| G2 | 3 | playing a short rendered range (400 ms): 16 V-fps, the picture stutters, 45 % | done cached-range-plays, render-ahead, t1934, t1936, t2920: a played range plays from what is kept; the card's figure is his checks C1, C5 | `cached-range-plays` | done |
| G3 | 4 | the sound crackles in that playback | done cached-range-plays, t1935, kept-mix-settle t2372: the render does not fall behind and an edit during playback no longer stops the sound; the clicks were the tape's own dropouts (his answer 07.11) | `cached-range-plays` | done |
| G4 | 4 | the cache does not work: a rendered range does not play from it | done cached-range-plays, t1930, t1934, t1936: the bars show what plays without work | `cached-range-plays` | done |
| G5 | 5 | the effects window open: every mouse move makes prefetch take a whole core | done input-without-work: t1871, t1874, t1880, live `idle`; on his project (event 2132, VHS selected, 10 s of the pointer) prefetch 69 % / audio-render 68 % / 41 engine builds → 0 / 0 / 0, release, llvmpipe | `input-without-work` | done, to merge |
| G6 | 5 | the report is written late and has no clock time: he cannot say what moment is what | done telemetry-journal: t1960, t1966, live `journal`; each record stamped at its event with clock time (the report went with load-report-into-trace: its header is in the trace, t2571, t2572) | `telemetry-journal` | done, to merge |
| G7 | 6-7 | VHS tape + ISP off on video and audio, prefetch still runs; a mouse move still loads; another effect selected in the chain, the load stops: the VHS tape panel triggers it. "ТРЕБУЮ ПОЛНУЮ РЕВИЗИЮ" | done input-without-work: t1870, t1872, t1873, t1875, t1876, t1877; the class walk in its `design.md` D3 (number fields, colours, level drag, speed drag, thumbnails, the UI renderer, the load watch) | `input-without-work` | done, to merge |
| G8 | 9 | VHS tape on video is tolerable; added on audio, everything breaks: neither cache nor optimized code | done audio-fx-cost, per-sample-costs, t1981, t1982, t2684; the cache half cached-range-plays t1930 | `audio-fx-cost`, `cached-range-plays` | done |
| G9 | 11 | does he need to send files; what is `app.ron` | question: "Нужно ли мне какие-то файлы скинуть?" Answered 07.11: no; nothing left | — | answered |
| G10 | 12-13 | one telemetry journal: load, the playback journal and his actions in one, with time; telemetry is not usage tracking (posthog) | done telemetry-journal: t1961-t1965, t1969, live `journal`; note F-HLP-035; `telemetry-journal.log` holds load, playback, notes, edits, windows, effect selection, open/save and his marks (Ctrl+Alt+M, Diagnostics); costs nothing at rest (measured, design) | `telemetry-journal` | done, to merge |
| G12 | 17 | the Diagnostics log shows only when the window is made taller: its tail is under the fold of the scrolling body | done telemetry-journal, windows-fill-their-area, t1968, t1127, t1125: the journal pane stands outside the scroll; the class across windows is the filling rule and its guard (`window-content-reach` was never opened) | `telemetry-journal`, `windows-fill-their-area` | done |

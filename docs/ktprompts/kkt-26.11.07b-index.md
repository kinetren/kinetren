# Registry of the owner's list `kkt-26.11.07b`

Source — [`kkt-26.11.07b.txt`](kkt-26.11.07b.txt) (verbatim, the owner's second message of 07.11.2026,
on build `kt26.11.07-61414dba`). Columns and states — as in [`v26.10-index.md`](v26.10-index.md). The
round is in `docs/user-feedback.md` ("2026-11-07 — his run of 61414dba"). His files: the folder
`/media/e/developer/kt.9/fails/26.11.07` (`load-report.txt`, `app.ron`, `kt-app.log`, `haski.kt`);
he sent the project after all (line 15); it is his, its media are not on this machine, it is not copied into the tree.

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| G1 | 2 | on a pause, on an event, moving the mouse over the preview costs 25 % of the processor (not prefetch) | no: defect, `transport`; in his report the UI thread stands at ~100 % of a core while it draws, up to 0.5 s per frame | `ui-frame-cost` | on his word |
| G2 | 3 | playing a short rendered range (400 ms): 16 V-fps, the picture stutters, 45 % | no: defect, `transport`, `video-plan` | `rendered-range-play` | on his word |
| G3 | 4 | the sound crackles in that playback | no: defect, `audio-render` | `rendered-range-play` (own report) | on his word |
| G4 | 4 | the cache does not work: a rendered range does not play from it | no: defect, `media-index` | `rendered-range-play` (own report) | on his word |
| G5 | 5 | the effects window open: every mouse move makes prefetch take a whole core | no: defect; in his report prefetch ~90 % and audio-render ~85 % for 20 s with no audio rendered, the counters say "at rest" | `vhs-panel-rebuilds` | on his word |
| G6 | 5 | the report is written late and has no clock time: he cannot say what moment is what | no: defect, `telemetry` | `telemetry-journal` | on his word |
| G7 | 6-7 | VHS tape + ISP off on video and audio, prefetch still runs; a mouse move still loads; another effect selected in the chain, the load stops: the VHS tape panel triggers it. "ТРЕБУЮ ПОЛНУЮ РЕВИЗИЮ" | no: defect, `audio-fx`, `video-fx`, `transport` | `vhs-panel-rebuilds` + the revision of every panel and every path that wakes prefetch | on his word |
| G8 | 9 | VHS tape on video is tolerable; added on audio, everything breaks: neither cache nor optimized code | no: defect, `audio-fx` | `vhs-audio-cost` | on his word |
| G9 | 11 | does he need to send files; what is `app.ron` | question: "Нужно ли мне какие-то файлы скинуть?" Answered 07.11: no more files needed; `app.ron` is the window and settings state, not needed | — | answered |
| G10 | 12-13 | one telemetry journal: load, the playback journal and his actions in one, with time; telemetry is not usage tracking (posthog) | no: feature, `telemetry`, no note, change `telemetry-journal` (note in `docs/features/` first) | `telemetry-journal` | on his word |

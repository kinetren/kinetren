# Registry of the owner's list `kkt-26.11.08`

Source — [`kkt-26.11.08.txt`](kkt-26.11.08.txt) (verbatim, two messages of 08.11.2026). Columns and
states — as in [`v26.10-index.md`](v26.10-index.md). The first message was a run of the OLD build
kt26.11.07-61414dba (its `kt-app.log` and `load-report.txt` say so); the second, of
kt26.11.08-b614203c. His files: `/media/e/developer/kt.9/fails/26.11.08/` and `…/new/`.

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| H1 | 3 | the effects window with VHS Tape selected lags everything, prefetch hammers on a pause | partial: input-without-work (the field at its floor wrote an edit per frame); his run was the old build; on b614203c "стало лучше" | `input-without-work` | merged in orch |
| H2 | 4 | the lag depends on the place: moving right, when another event appears it lags hard | partial: likely the same field on other VHS events, not proved on this scenario | `input-without-work` | merged in orch |
| H3 | 7 | "Ты точно там что-то исправил?!?!? Отвечай." | question: "Ты точно там что-то исправил?" Answered 08.11: he ran 61414dba; the fixes are in b614203c | — | answered |
| H4 | 9 | the debug build falls on SIGSEGV often | no: defect, `durability`, change `segv-crashes` | `segv-crashes` | on his word |
| H5 | 10 | the crash report is scanty | no: defect, `durability`, the report must carry the faulting thread's stack; change `segv-crashes` | `segv-crashes` | on his word |
| H6 | 11 | overall performance could be faster on his machine | no: defect, `transport`; his journal: an edit makes UI frames of 330 ms for seconds, short loops play at V-fps 0, 2.5 and 15.5, video cache hit 0 %; changes `edit-without-rebuild`, `loop-picture-throughput`; speed is judged on a release build | `edit-without-rebuild`, `loop-picture-throughput`, `decode-request-storm` | on his word; `decode-request-storm` in branch (t2040–t2045) |
| H7 | 12 | "Что дальше?" | question: "Что дальше?" Answered 08.11 with the plan | — | answered |
| H8 | 14 | is the dead loop picture tied to a heavy effect he changed? (new2, release build 360b50e6) | question: "а loop-picture-throughput может связан с тяжелым эффектом?" Answered 08.11: yes for the V-fps 0 loops after he selected Datamosh (1570 decode requests a second, 15-100 decoded); the UI blocks of up to 4.5 s at 25-50 % processor are a wait on the GPU | — | answered |
| H9 | 14 | on his computer everything could run much faster | partial: `gpu-pass-cost` — the journal records each pass's time on his card (t2020–t2022, live `gpu_record`); Camera ISP −25 % a frame, Datamosh's residual a workgroup per block, no resource created per frame (t2023, t2024); his card's numbers await his next run. `ui-never-waits`, `decode-request-storm` open | `ui-never-waits`, `gpu-pass-cost`, `decode-request-storm` | on his word |

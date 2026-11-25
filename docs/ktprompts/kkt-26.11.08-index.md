# Registry of the owner's list `kkt-26.11.08`

Source — [`kkt-26.11.08.txt`](kkt-26.11.08.txt) (verbatim, two messages of 08.11.2026). Columns and
states — as in [`v26.10-index.md`](v26.10-index.md). The first message was a run of the OLD build
kt26.11.07-61414dba (its `kt-app.log` and `load-report.txt` say so); the second, of
kt26.11.08-b614203c. His files: `/media/e/developer/kt.9/fails/26.11.08/` and `…/new/`.

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| H1 | 3 | the effects window with VHS Tape selected lags everything, prefetch hammers on a pause | done input-without-work, t1870, t1871, t1874: the field at its floor wrote an edit per frame | `input-without-work` | done |
| H2 | 4 | the lag depends on the place: moving right, when another event appears it lags hard | done input-without-work, frame-work-by-edit, t1873, t2660: a frame asks what its moment holds; the place no longer matters | `input-without-work` | done |
| H3 | 7 | "Ты точно там что-то исправил?!?!? Отвечай." | question: "Ты точно там что-то исправил?" Answered 08.11: he ran 61414dba; the fixes are in b614203c | — | answered |
| H4 | 9 | the debug build falls on SIGSEGV often | done segv-crashes, gl-context-holder, one-process-tests, t2086, t2087, t2088, t2740 | `segv-crashes` | done |
| H5 | 10 | the crash report is scanty | done segv-crashes, t2080–t2085: a native crash's report names the signal, the faulting thread, its stack with functions, files and lines, the other threads and the run's last journal lines; the crash window shows the one-line summary and those lines; kt-cli prints the same | `segv-crashes` | segv-crashes |
| H6 | 11 | overall performance could be faster on his machine | done edit-without-rebuild, decode-request-storm, render-ahead, t2000, t2040, t2920 (`loop-picture-throughput` was never opened: its claim is those and `gpu-throughput`) | `edit-without-rebuild`, `decode-request-storm`, `render-ahead`, `gpu-throughput` | done |
| H7 | 12 | "Что дальше?" | question: "Что дальше?" Answered 08.11 with the plan | — | answered |
| H8 | 14 | is the dead loop picture tied to a heavy effect he changed? (new2, release build 360b50e6) | question: "а loop-picture-throughput может связан с тяжелым эффектом?" Answered 08.11: yes for the V-fps 0 loops after he selected Datamosh (1570 decode requests a second, 15-100 decoded); the UI blocks of up to 4.5 s at 25-50 % processor are a wait on the GPU | — | answered |
| H9 | 14 | on his computer everything could run much faster | partial: `gpu-pass-cost`, `gpu-throughput`, `render-ahead`, `persistent-cache` are in (t2020, t2251, t2920, t3440); his card's figures are C1, C2, C14 | `ui-never-waits`, `gpu-pass-cost`, `decode-request-storm` | awaits his checks |
| H10 | 16 | a new engine instead of this one, as a breaking change, given all the defects collected? | question: "может нам ... придумать новый движок? а этот выкинуть просто как breaking change?" Answered 08.11: no rewrite from zero; the audit's data (`docs/engine-audit.md` §1, §4): every architectural mechanism is in the coordination layer (rebuild per edit, waits on the UI/render threads, producers without back-pressure, resources without an arbiter, budgets without a ledger), none in the computation; replaced in place under the spec-named tests, no `.kt` change, one additive plugin-API change | `engine-audit` | answered, audit archived |
| H11 | 18 | run the audit of defects by mechanism; the scheduler for CPU too, not only GPU; consider JIT of math and computation | done engine-audit and its ordered plan, t2062, t2000, t2230, t2250, t2294: all five changes of the plan and `pipeline-prewarm` are archived | `builder-cost-guard`, `edit-without-rebuild`, `one-scheduler`, `gpu-arbiter`, `memory-ledger`, `pipeline-prewarm` | done |
| H12 | 21 | JIT was meant not only for shaders | done compiled-curves, kernel-specialization, stream-layout-per-build, fused-pointwise-runs, t2100, t2173, t2270, t2190: the audit's T1 and T2 are in; run-time machine code stays rejected with its reason (F-EXT-005) | `builder-cost-guard`, `compiled-curves`, `kernel-specialization`, `stream-layout-per-build`, `pipeline-prewarm`, `fused-pointwise-runs` | done |
| H13 | 22 | GPU priorities must not wait for his journal: that tunes the program to one machine | done gpu-arbiter, t2250: priorities are measured at run time on whatever machine; the rule is in `docs/orchestrator.md` | `gpu-arbiter`, `fused-pointwise-runs` | done |
| H14 | 23 | в.1 "Да": run `builder-cost-guard` now | done builder-cost-guard, t2062, t2065, t2067; rows 152–157 of found-defects carry their own states | `builder-cost-guard` | done |
| H15 | 24 | в.2 "Да": the scheduler may pause background services (thumbnails, analysis, index) during playback | done one-scheduler: thumbnails, effect cells, reel strips, posters, analysis, the index, the media watch and autosave are lanes of one scheduler; a playing player and a running export vote them paused, they resume on a pause + t2231, t2238, live `sched_pause` | `one-scheduler` | done |
| H16 | 25 | в.3 "Согласен": fused effect chains may shift goldens by at most 1 LSB per former boundary | done fused-pointwise-runs, t2190, t2459: the runs' boundary rounding is inside the tolerance he allowed | fusion | done |
| H17 | 26 | в.4 "переводи": fast-forward master to the stage | done: master fast-forwarded by the orchestrator (the stage of ui-never-waits, decode-request-storm, gpu-pass-cost, engine-audit at kt26.11.08-a3ee2f53; t985 known red, row 132) | — | done |
| H18 | 28 | start `compiled-curves` and `pipeline-prewarm` | done compiled-curves, pipeline-prewarm, t2100, t2120 | `compiled-curves`, `pipeline-prewarm` | done |
| H19 | 30 | every change reaches master only by a merge commit: one commit per change on master, the intermediate commits on a side loop | partial: a standing rule, kept since (`git log --first-parent master`) | — | standing |

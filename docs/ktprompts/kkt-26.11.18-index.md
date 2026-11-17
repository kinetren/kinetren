# Registry of the owner's list `kkt-26.11.18`

Source — [`kkt-26.11.18.txt`](kkt-26.11.18.txt) (verbatim, one message of 18.11.2026 after stage 19,
release kt26.11.17-1f77e6bf; his traces in `fails/26.11.18/{new,old,render_simple_region}`).
Columns and states — as in [`v26.10-index.md`](v26.10-index.md).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| T1 | 1 | closing the program while a loop is catching up does not work; he has to kill the process | done close-while-busy: every wait of the close bounded and named in the trace (`kt-cli trace shutdown`), the exit bounded after the recovery write (t2950–t2953; live scenario `close_busy`: paused, playing and File → Quit) | `durability`, `transport`, `telemetry` | 20 |
| T2 | 2 | problem_region: the processor and the card barely work, the loop is complete only from about lap 15 | no → `ahead-kept` (his trace `new`: device 32–46 %, up to 2–3 moments in flight, most moments composed ahead are never kept) | `video-plan`, `telemetry` | 20 |
| T3 | 3 | even the simplest region renders at about 1× real time | no → `export-readback` (his trace: 0.94×, readback 94.5 % of the wall, encode 3.7 %) | `export` | 20 |
| T4 | 4 | the render log goes into the telemetry, not a separate file | no → `export-readback` (render.log into the structured trace) | `export`, `telemetry` | 20 |

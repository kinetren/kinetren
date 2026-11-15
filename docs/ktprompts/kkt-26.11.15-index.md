# Registry of the owner's list `kkt-26.11.15`

Source — [`kkt-26.11.15.txt`](kkt-26.11.15.txt) (verbatim, one message of 15.11.2026 after stage 14,
with his folder `kt.9/fails/26.11.14`: `1/` a crash report of kt26.11.10-4275c0f8, `2/new` traces of
the new version, `2/old_for_e.g._more_fast_at_rendereg_and_behind` a trace and load report of
kt26.11.10-4275c0f8). Columns and states — as in [`v26.10-index.md`](v26.10-index.md).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| N1 | 3 | a crash of the old version (double click on the window title while a region loops: `Surface::configure` "Failed to wait for GPU to come idle" panics) — check it in the new one | partial: being checked | `kkt-26.11.15` stage 15 | 15 |
| N2 | 4–5 | the performance problems we tried to solve are not solved and got worse: the real-time preview of a piece that played in real time in kt26.11.10 is very bad in the new one at the same system load | partial: diagnosed from his traces, then made FASTER than kt26.11.10 (not merely restored) | `kkt-26.11.15` stage 15 | 15 |
| N3 | 6 | the final render is very slow in every version, slower than the preview | no → after N2 | — | later |
| N4 | 7 | the processor mostly idles while the program works | no: FORBIDDEN to start until N2 is done and faster than before (line 9) | — | after N2 |
| N5 | 8 | the final render's settings are very scant | no: feature, no note yet (to write in `docs/features/` first) | — | later |

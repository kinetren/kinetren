# Registry of the owner's list `kkt-26.11.15`

Source — [`kkt-26.11.15.txt`](kkt-26.11.15.txt) (verbatim, one message of 15.11.2026 after stage 14,
with his folder `kt.9/fails/26.11.14`: `1/` a crash report of kt26.11.10-4275c0f8, `2/new` traces of
the new version, `2/old_for_e.g._more_fast_at_rendereg_and_behind` a trace and load report of
kt26.11.10-4275c0f8). Columns and states — as in [`v26.10-index.md`](v26.10-index.md).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| N1 | 3 | a crash of the old version (double click on the window title while a region loops: `Surface::configure` "Failed to wait for GPU to come idle" panics) — check it in the new one | done gpu-errors-survive, live `resize_loop` (red on the old code with his panic) — the preview's thread submitted on the window's device while the UI thread reconfigured its surface, and no device of ours caught a wgpu error; every device now writes its errors instead of panicking, a frame's error is its error (t2720, t2721), the display yields to a refused reconfigure (t2722) | `gpu-errors-survive` | 15 |
| N2 | 4–5 | the performance problems we tried to solve are not solved and got worse: the real-time preview of a piece that played in real time in kt26.11.10 is very bad in the new one at the same system load | done loop-laps-regression (t2517, t2433) on `rendereg_2` (`kkt-26.11.16b` Q1: the loop plays from kept from its 5th lap); before that partial: diagnosed from his traces, then made FASTER than kt26.11.10 (not merely restored). Stage 15 fixed the far-walk (row 660) and the readback wait (661), but his run of 16.11 (`kkt-26.11.16` P1, P2) shows it still slower; line 12: the piece WITHOUT Datamosh worries him even more than Datamosh — both are examined | `kkt-26.11.15` stage 15 | 15 |
| N3 | 6 | the final render is very slow in every version, slower than the preview | no → `render-speed`, started on his word of 16.11 (`kkt-26.11.16b` Q2) | `render-speed` | 17 |
| N4 | 7 | the processor mostly idles while the program works | no → `cpu-idle-work`: N2 accepted on his `rendereg_2` run (`kkt-26.11.16b` Q1), started on his word | `cpu-idle-work` | 17 |
| N5 | 8 | the final render's settings are very scant | no → `render-settings`, no note yet: the note in `docs/features/` first | `render-settings` | 17 |

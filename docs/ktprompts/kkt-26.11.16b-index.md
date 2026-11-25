# Registry of the owner's list `kkt-26.11.16b`

Source — [`kkt-26.11.16b.txt`](kkt-26.11.16b.txt) (verbatim, one message of 16.11.2026 after stage 16,
release kt26.11.16-1afe406e; his folder `kt.9/fails/26.11.16/rendereg_2/{new,old}`: a trace of
kt26.11.16-8c4ca513 with a 22.8 s loop, a later trace file of kt26.11.10-4275c0f8 (its first file is
not in the folder) and an old load report). Columns and states — as in [`v26.10-index.md`](v26.10-index.md).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| Q1 | 1 | check `rendereg_2` | done loop-laps-regression (t2433; its arbiter test was replaced by t2923 in render-ahead): `trace loop` of the new run — laps 3–6 still compose (295, 216, 131, 69 frames) while loop-fill keeps 116–151 a lap, lap 7 makes nothing, from lap 8 every lap serves ~860 kept frames: the loop plays from kept from its 5th played lap (his impression of kt26.11.10 on `problem_region_26_11_09`: about the 11th; stage 15: about the 25th). The first lap shows as many frames as kt26.11.10's first lap on `rendereg` (323 against 315), and a filled lap matches the old filled lap (849 shown, ~38 fps, 7–11 % load against 898, 5–10 %). The old fill laps cannot be counted: the old folder lacks the first trace file | — | 16 |
| Q2 | 2 | if it satisfies — do the rest without him | done render-speed, cpu-idle-work, render-settings, t2791, t2334, t2801 | `render-speed`, `cpu-idle-work`, `render-settings` | done |

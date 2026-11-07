# Design: idle-cpu

## Context

The owner's words are in `docs/ktprompts/kkt-26.11.07.txt`, item E1. This machine has no graphics card, so wgpu
runs on llvmpipe or lavapipe and a frame costs 80 ms of processor. Percent measures that, so the
measure is WORK: `kt_time::work`, relaxed atomics that steer nothing.

The project is `kt-cli showcase --heavy`: ten releases of a composition carrying Slowed, VHS Tape and
Camera ISP, a dense edit of 1350 events under them.

## D1. What ran on a pause

| Cause | Counters on the pause (1357 events) | Since |
|---|---|---|
| The preview service evicted a live release; the frame asked again | 3.7 reel jobs, 66 submits, 15 UI frames a second; 236 % of a core against 46 % | R.59 (cap of 6 from R.56, one key per instance from R.59) |
| One sidecar row per reel | every release but the last rendered on each opening | R.59 |
| The tick of 4 Hz | 4 UI frames a second, 36 % of a core in software | v1.4 |
| An incomplete frame recomposed on every UI frame | each mouse move pays the chain; `last_compose_retry` was written and never read | R.53 |
| The meters' cap falling at 60 fps after a seek | 10 s of full frames from a level under the cursor | R.16 |

After the change, release build, the same project: every counter 0 over 30 s of a pause, on the first
opening and on the second, and 0.73 % of a core over 60 s of a plain run with no bench attached, of
which 0.40 % is the audio output's threads and 0.33 % the load watch. A software rasterizer does not
change this number: nothing is drawn.

The old logic prints the mechanism: one job per UI frame. The request runs in the frame, so whatever
drew a frame drove the circle, and the jobs filled the pause between frames.

Rejected: a larger cap. Any number is a project away from the same circle; the cap's job is memory,
and what an event shows has to stay.

## D2. At rest nothing polls

The frame was the program's scheduler. `wake` replaces it: a background thread says it has something,
a clock asks for its moment (`KtApp::rest_wake`). No heartbeat: a missed wake is a defect to find, and
a heartbeat would hide it at the price of frames forever. `wake::soon` carries a stream of small
results as one frame per 120 ms, because egui keeps the earliest repaint asked for.

The bench needed the same: `live-ctl` watches its file, and a throttled dump asks for one more frame.

## D3. An incomplete frame waits for news

`BurstCache::on_next_insert` hears the next insertion; `compose_news` then checks that nothing is
missing before paying for a composite. A timer that doubles from 250 ms to 8 s remains for a
`complete = false` that comes from something other than a missing layer. Past it the frame waits for
an edit, a seek or an insertion: it costs nothing to leave it.

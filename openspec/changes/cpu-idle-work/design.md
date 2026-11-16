## Context

See proposal.md — Why. The diagnosis is from the owner's traces of build kt26.11.16-8c4ca513
(`fails/26.11.16/{rendereg_2,problem_region_26_11_09,rendereg}/new`, read only) with
`kt-cli trace … during|loop|gpu|dump`, a release kt-cli of this branch.

**What his machine does while the program has work it cannot keep up with** (Ryzen 7 7700X, 16 logical
processors, iGPU RADV RAPHAEL as the display, offline and compute device):

| trace | seconds | process load, mean | load at most | card per second |
|---|---|---|---|---|
| `rendereg_2/new` (VHS Tape loop) | 271 | 12.8 % of ONE core | 343 % (opening) | ~20 frames × 15 ms ≈ 30 % |
| `problem_region/new` (Datamosh + Camera ISP) | 272 | 11.6 % | 234 % (opening) | ~4 frames × 60 ms ≈ 24 % |
| `rendereg/new` (no Datamosh) | 90 | 25.6 % | 575 % (opening decode) | ~20 × 15 ms |
| `26.11.14/2/new` | 768 | 29.5 % | 244 % (hevc decode) | — |

So the processor holds about 1 % of the machine, and the card is idle two thirds of each second. During
a loop's first laps the LOAD lines read `kinetren 6, audio-render 4, preview-compose 2, sched 1-2`: the
work is device work (the effects are WGSL passes), and the device is fed by chains that run one at a time.

**The mechanism** (one fill moment of `rendereg_2`, +40.09 s, `job:loop_fill` on `sched-3`): 181.8 ms of
wall for ~20 ms of device time. The moment is cut into seven pieces; each is admitted after the one before
(`gpu_admit` waits 0, 0, 0, 62.8, 4.2, 22.9, 20.2 ms), and after the last the readback stands 70 ms. All
660 fill moments of the run took 107.8 s (163 ms each), and only one was ever in flight: the fill is one
lane, its pump composes one moment, and it starts none while the display composes (`display_busy`). The
display's own composites are one at a time too (the compose thread, latest-wins by design). A pipeline of
round trips delivers what it holds in flight divided by the round trip (Little's law): one moment in flight
over 165 ms is ~6 moments a second, whatever the card and the processor could do. That is the class:
**work that waits on a round trip, run one at a time, leaves both the processor and the device idle.**

## Goals / Non-Goals

**Goals:** the loop fill keeps several moments in flight where that pays (a hardware device), sized from
the machine and the device at run time; nothing changes at rest; the software rasterizer keeps its
measured rule.

**Non-Goals:** the arbiter's bound and its three lower pieces (the owner's latency limit, loop-laps-regression
D2); the export pipeline (`render-speed`); the display's own compose thread (one frame at a time is its
contract: a newer ask replaces a waiting one); a CPU executor of device work beside a card (see Open Questions).

## Decisions

### D1. The fill runs `width` moments at once, one engine each

`LoopFill` starts eight `near` lanes (`loop-fill`, `loop-fill-1` … — lanes cost nothing until kicked: no
thread, no timer). `set_exec` makes the executor's `sibling_exec()` up to the width (same device, ring and
compiled pipelines, its own lock and frame state — a composite holds its engine's lock through its
admissions and its readback, so two moments on one engine would serialize there). Lane `i` composes on
executor `i`; a moment is claimed under the fill's lock (`claimed`), so no two lanes take one; a lane that
starts a composite kicks the next lane. The prefetch thread still kicks lane 0 every pass while a loop plays.

`width(software, workers, forced)`: a software device 1; a card `workers − 1` (the scheduler's pool,
`sched::workers()` = a quarter of the cores, 3 to 6 — one worker stays for a person's answer, since `near`
is not a capped class), at most `RING_LOWER_PIECES` = 3, kt-gpu's `arbiter::AHEAD_PIECES` (more moments
only wait in admission). On his 16 processors: 4 workers → width 3; on 8: 3 → 2; on 32: 6 → 3.
`KT_LOOP_FILL_WIDTH=N` forces it (1 to 8), for an A/B on his machine.
Rejected: a fill lane per core (the ring admits three lower pieces; the rest wait there); a second
headless device for the fill (rejected in scheduler-followups: a device and its pipelines against an arena
and pools — a sibling is the arena and pools).

### D2. On a card the fill does not wait for the display's composite

**Revision** of scheduler-followups D5 ("it starts nothing while the display composes"), for hardware
devices only — reason: the gate was measured on llvmpipe, where the fill and the display share the
processor. On a card the ring's arbiter already orders them (the frame on screen is class `now`, the fill
`near`, and a lower class leaves at most the bound in front of the display's next present). With the gate,
his fill could start only in the third of each second the display was not composing. On a software device
the gate stays, and the width is 1.

### D3. An incomplete moment narrows the fill for the lap

A moment that comes out incomplete (a stream stage short of its history) sets `narrow`: lanes above 0 take
nothing until the lap key moves (versions, quality, region, build, the playhead falling back). On his
Datamosh region 86 of 92 fill moments of a lap came out incomplete: three lanes would burn three times
the device time on them, taken from the history walk the display needs. The skip-for-the-lap rule is kept.

### D4. Measured

*The card case is not reproducible here* (a VM with no card: every device is llvmpipe, and llvmpipe gives
each device a ring and a queue thread of its own, found-defects 692). It is held by t2860 with an executor
that reports a hardware device and takes a 150 ms round trip per composite (the shape of his fill moments):
width 3 filled 18 moments in 1.00 s against 2.81 s at width 1, three composites in flight at every step,
while a display composite was counted in flight all the time (the gate would have stopped it).
His next trace reads it: `kt-cli trace loop` ("fill: N kept"), `trace gpu` (the device's busy share), the
LOAD lines (`sched` threads busy at once).

*The software case* (release kt-cli, `preview heavy-showcase 26 30 --loop 45 --size 640x360`, llvmpipe,
32 processors shared with a sibling session's export — load average 29–78; runs interleaved):

| run | width | all-kept from lap | V-fps | incomplete | process |
|---|---|---|---|---|---|
| w1-1 | 1 (default) | 7 | 14.5 | 26 | 9.6 cores |
| w1-2 | 1 (default) | 7 | 13.8 | 3 | 10.8 cores |
| w3-1 | 3 (forced, no gate) | never in 11 laps | 9.1 | 353 | 12.3 cores |
| w3-2 | 3 (forced, no gate) | never in 12 laps | 5.2 | 172 | 12.1 cores |

Per thread: `llvmpipe` 553–936 %, `av:vp9` 27–99 %, everything of ours under 20 %. Forced wide on a
software device takes more of the processor and fills slower, and the display loses frames: the device IS
the processor, and the display and the fill compete for it. The default keeps software at width 1 with the
gate — the rule scheduler-followups measured (all-kept from lap 9 against 18 with the gate).

*At rest*: nothing new runs. The lanes are scheduler lanes (no thread, no timer); a lane runs only when
kicked, and only the prefetch thread kicks while a loop PLAYS (t2335: a paused player starts no `near`
job). The siblings are made once when the fill's executor arrives, not per frame.

## The class walk: work that waits on a round trip, one at a time

| subsystem | what bounds its parallelism | found | done here |
|---|---|---|---|
| loop fill | one lane, one moment, the display gate, one engine lock | his 660 moments at 163 ms each, one in flight | D1–D3 |
| display composite (`preview-compose`) | one frame at a time, latest-wins | by contract (a newer ask replaces a waiting one; the UI never waits) | no change |
| slow-chain route | the offline engine, one frame | the same contract as the display | no change |
| prefetch look-ahead decode | a MediaWorker per source (invariant 5), libavcodec 4 threads, RGBA 4 (`memory::decode_threads`/`rgba_threads`, measured constants) | his decode bursts reach 507 % (`av:vp9`); decode never stands in front of the loop in his traces | no change; row 694 (constants not from the machine) |
| stream history walk (Datamosh) | the prefetch thread walks one stage run at a time; compute jobs on one ring | `problem_region`: compute jobs `near` buffers back p50 607 ms after the device ended | row 693, not fixed (stream-round-trips' D9 area) |
| audio render | one thread (invariant 2) | 4–5 % of a core in his loop | no change (invariant) |
| thumbnails, fx cells, reel strips, posters, analysis, index | one job per lane, pool a quarter of the cores (3–6), paused while playing | `sched` one core for 1–2 s at the opening; nothing while playing | no change |
| compute host | one device, ring admission | part of the stream walk | no change |
| export | `render-speed`'s change | — | not touched |
| GPU arbiter | the bound, three lower pieces | the width is capped by the same three | no change |

Callers of what moved: `VideoGpuExec::sibling_exec` (new, default `None`; kt-gpu's engine implements it);
`Player::set_fill_gpu`/`fill_gpu_setter` (kt-app's offline-engine thread, kt-cli preview, tests) — the
siblings are made on the caller's thread, which in the app is the offline-engine thread, never the UI;
`LoopFill::kick` (prefetch thread). `Player::fill_width` and `composing_for_tests` are new (bench, tests).

## Risks / Trade-offs

- [The display on a card loses frames to wider fill] → the arbiter keeps the frame on screen first and at
  most three lower pieces of half the bound in front of it; `KT_LOOP_FILL_WIDTH=1` restores one at a time
  without a rebuild; his next trace's `trace gpu` shows the display's present wait.
- [Siblings cost memory] → each holds an arena and pools at the preview's size (640×360 on his machine);
  the memory ledger accounts device buffers per engine. Three, not one per core.
- [A sibling made on GL] → `sibling` takes the GL gate; the fill's executor is the side engine, never the
  window's device.

## Open Questions

- A CPU executor beside a card: his llvmpipe adapter (16 processors) could compose fill moments while the
  iGPU composes the display, which is the literal answer to "the processor idles". Its frames differ from
  the card's within the stored-frame tolerances, so a covered loop would show moments from two devices.
  That is the owner's call (asked in the report), not taken here.

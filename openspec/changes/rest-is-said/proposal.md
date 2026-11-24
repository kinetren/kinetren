# Proposal: rest-is-said

> DESIGN ONLY (spec-engine, 2026-11-24, orch `286c406c`; revision 2 of the same day against `0c2a4065`: `durable-moment-key`
> fixed 966, 967 and 969 one by one, and what it did is cut from here), asked by the orchestrator: read the rows of class "red once
> under load" together and say whether one mechanism stands behind several. Tests t4117–t4124.

## Why

Twelve rows of `docs/found-defects.md` (822–828, 958, 960, 961, 966, 967) were read together, with the fixed ones of the
same family (141, 750, 764, 772, 782, 783, 817, 950) and the open ones (293, 373–375, 424, 650). Three of the list are not
load reds: 823 (fractional frame rates), 826 (the prerender's order) and 960 (a defect of the product). **One mechanism
stands behind the rest: the observation is anchored to something other than the program's own progress.** Four shapes of
it, and each row is one of them:

| Shape | What the observer does | Rows |
|---|---|---|
| M1, the window opens early | starts measuring "nothing moves" at a flag the program sets BEFORE its last designed step | 828 (b), 950, 958 (`idle`), 961 (t2142), 967; fixed before: 141, 764, 782 |
| M2, the window closes by the clock | ends the experiment or the wait after N seconds and reads whatever is there, or a wait gives up and returns as if it had not | 822, 824, 966, **961 (t2140)** |
| M3, two clocks | paces a stimulus by its own sleep against a program timer | 828 (a), likely 958 (`node_graph`, `datamosh`); fixed before: 783, 817 |
| M4, two durations from two moments | compares wall times taken at different loads | 825, 827; open: 374, 375 |

`distribution` already says both halves of the rule ("a test waits for the code's last designed step", "a test's clock is
on the list or not in the test"). Each red was then cured alone, with a new flag in the dump or a new helper in one test
file: `sound_paused`, `rc_settled`, `posters_busy`, `settled`, the writer's `S` in `/proc`. The next actor has none, so the
next test of it is red. Two things keep the class alive: the program has no one statement of "I am at rest", and a wait
that gives up does not say so. A third makes it costly: four of the twelve rows could not be read because the failure's
message was gone with the run.

## What Changes

- **The program says its rest**: every background actor declares whether it works, is parked, or has a wake due; one
  question answers "at rest, or who is not". Tests and live scenarios open a quiet window only from that answer.
- **A wait that gives up says it**: `trace::flush` answers whether the records are written; the tests' form waits to
  the guard and names the writer.
- **A bench that gives up says so.** Playing on until a frame is shown is built (966, `first_shown_by`); its other ends (a start that
  never sounds) still return as if the range had been played.
- **The early window is made to fail on purpose**: a stretch at the actors' last step (a test knob) makes every M1 test red
  in one run, instead of one at a time over months.
- **A red keeps its message**: the test run writes its failures to a file that the next run does not overwrite.
- **Row 960**: the prefetch's near part on a pause waits for a frame instead of passing for ever.

## Capabilities

### Modified Capabilities
- `distribution`: how a test observes rest, how a wait ends, what a red leaves behind.
- `transport`: a paused preview whose sources do not answer runs no passes.

## Impact

kt-time (`rest.rs`, `trace.rs` `flush`), the actors' park points (kt-engine `player.rs`, `worker.rs`, `compose_queue.rs`,
`render_cache.rs`; kt-app `audio_out.rs`; `sched.rs`), `preview_bench.rs`, `.config/nextest.toml`, `scripts/finish.sh`,
`scripts/live/idle.py` and its users, the kt-app dump. No format change, no model change, no pixel. At rest: an actor
writes one atomic as it parks.

## Questions to the owner

None.

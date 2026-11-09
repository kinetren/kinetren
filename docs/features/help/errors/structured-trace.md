---
id: F-HLP-036
title: A structured trace of one run — why a frame waited, answered by a query
status: in-progress
priority: A
cost: L
category: help/errors
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, perfetto-2026, tracy-0.12, unreal-insights-5.6]
we_have: partial
crates: [kt-time, kt-engine, kt-gpu, kt-cache, kt-app, kt-cli]
invariants: [4, 6, 11, 12]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-HLP-035]
trackers: []
services: []
checked: 2026-11-09
---

# F-HLP-036 — A structured trace of one run — why a frame waited, answered by a query

## Essence

The person meets "Loading" for five seconds on a paused frame and sends one file from his run. Whoever
reads it must answer "what happened and why" from that file alone: which frame was asked, what it
waited for (a decode, a cache entry another job evicted, a history position of a stream effect, a
compute job, a GPU pass, a pipeline compile, a lock), in what order, how long each wait was, and what
the person did just before — without another build, a bench or his machine.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | no | a crash dialog and a text log; nothing records why playback dropped frames |
| Premiere | 26 | Debug Monitor / log files | text logs per session (`Premiere Pro Log.txt`), playback dropped-frame indicator; no cause chain |
| Resolve | 21 | Help › Create Diagnostics Log on Desktop | a zip of text logs and system info; the GPU/driver facts are the useful half; no per-frame causes |
| Final Cut Pro | 12 | no | the system's unified log; no per-frame record |
| After Effects | 26 | no | text logs and an info panel; no per-frame record |

**Outside the core.** All five core lines are "no" for a cause chain, so the device comes from
profilers and engines.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Perfetto | v50 line | track events + trace processor | slices on tracks (a thread, an async track), FLOW ids that join a slice on one thread to its effect on another, counters as their own tracks; the file is queried with SQL, not read |
| Tracy | 0.12 | zones, frame marks, lock zones, plots | a zone costs a few nanoseconds: a timestamp into a lock-free per-thread queue drained by a worker; a lock wait is recorded as its own event so contention is seen |
| Unreal Insights | UE 5.6 | `.utrace` channels | typed events of fixed layout whose schema is announced once inside the file; channels switched on per need; CPU, GPU and asset-loading tracks on one clock |

## The trick behind it

- **The cause is an id carried, not a sentence written.** Perfetto's flows and Unreal's event ids: a
  frame's id rides with every request it makes (a decode request, a compute job, a cache lookup) to
  the other thread that serves it, so "why did frame 812 wait" is a join, not a guess from
  timestamps that happen to be near.
- **A record is a fixed typed layout, its names interned once.** Unreal's announced event layouts and
  Perfetto's interned strings: a hot record is a few integers, a name (an effect, a pass) is written
  once per file and referred to by number, so the file stays small and a reader needs no text parser.
- **Recording is a store into a ring, the disk is someone else's job.** Tracy's queue: the thread
  that does the work never formats, allocates or locks; a writer drains in batches and sleeps when
  nothing comes. The same holds the cost at rest at zero.

## Pros

- A complaint about one run is answered from that run: no rerun with a different build, no bench.
- The same file answers new questions later: the query grows, the record does not.

## Cons

- Every subsystem that waits must say what it waits for, with ids; a subsystem that forgets is a
  blind spot that looks like "nothing happened".
- A bounded ring forgets the oldest: a complaint about minute one of a two-hour busy run is lost.

## What users say

NO QUOTES: this session did not search forums; the note is written from the vendors' documentation
(perfetto.dev, Tracy's manual, Unreal's Insights pages, Resolve's diagnostics menu). The owner's own
words carry the demand: "у тебя телеметрия должна быть такая, чтобы всё было ок а не возня в ASCII
логи которые вообще мало что говорят" (kkt-26.11.09 line 8).

## How it fits Kinotrain

CAPABILITY: openspec/specs/telemetry/

### What already exists

`src/apps/kt-app/src/tele_journal.rs` — one text journal stamped at the event (F-HLP-035);
`src/libs/kt-time/src/work.rs` — work counters; `src/libs/kt-gpu/src/pass_time.rs` — GPU pass time;
`src/libs/kt-engine/src/flight.rs` — the playback black box. None carries a frame's id or a cause.

### What is missing

Typed records with ids across the compose thread, the decode worker, the prefetch thread, the
stream stages, the compute host, the executor's memo and the audio render thread; a query tool.

### What hinders

Invariant 6: the audio callback records nothing, the render thread only into a lock-free ring.
Invariant 12: nothing may wake at rest.

## Estimate

| What | Answer |
|---|---|
| Scope | L — one round, two stages (the trace, then the defect it explains) |
| Where the main work is | kt-time (the ring and the file), kt-engine/kt-gpu (records), kt-cli (the query) |
| Project format | does not touch |
| Tests that will have to be written | ring and file round trip, cost at rest, a frame's chain across threads, the query answers |
| Depends on | F-HLP-035 |
| Breaks compatibility | no |

## Verdict

Take: the owner refused benches and version bisects (kkt-26.11.09 I5–I8); a typed trace with ids and
a query is the only way a run on his machine explains itself.

TEST: src/libs/kt-time/src/trace.rs — t2140–t2143 hold the ring, the file and the cost at rest; the query's answers are t2146–t2149 in src/apps/kt-cli/src/trace_query.rs

## Sources

- https://perfetto.dev/docs/instrumentation/track-events (access date 2026-11-09, by documentation, not checked live)
- https://github.com/wolfpld/tracy (manual, access date 2026-11-09, by documentation)
- https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-insights-in-unreal-engine (access date 2026-11-09, by documentation)

# Design: structured-telemetry

Reconnaissance: `docs/features/help/errors/structured-trace.md` (F-HLP-036): Perfetto's slices joined
by flow ids and queried with SQL, Tracy's lock-free per-thread queue drained by a worker, Unreal
Insights' typed events whose layout travels in the file. Resolve's "Create Diagnostics Log" gives the
machine facts and text logs; no core editor records why a frame waited.

## The mechanism the owner named

kkt-26.11.09 I5–I8. The journal says THAT work ran ("Datamosh 13–24 compute jobs/s, 1–6 composites/s")
and never WHICH frame asked for it, what that frame waited for on another thread, or why a cache
missed. So a run on his machine cannot explain itself and every complaint became a reproduction on
ours — the bench and the bisect he refused. The trace below records causes with ids; the query asks
the questions; the journal stays for people.

## D1. Where: `kt_time::trace`

Every crate that does the work depends on kt-time (the reason `work` lives there); a crate of its own
would add a dependency edge to all of them for the same zero-dependency code. The format, the writer
and the reader are std only. The questions live in `kt-cli trace` (`trace_query.rs`).

## D2. A record is a fixed typed layout

`Rec`: 64 bytes — the time since the process started (ns), a duration for a span, `id`, `cause`,
three numbers and a 32-bit `aux`, whose meaning each `Ev` documents. Text — names (an effect, a
pass, a kernel, a thread), the machine, the run, the journal lines — goes on a side channel (a mutex
and a `Vec`, never from the audio render thread's loop) and into the file as length-prefixed items.
Names are interned by a 32-bit FNV hash in a lock-free table: the first use writes the string once.

Rejected: JSON lines (the owner's complaint is text the agent must parse; a JSON line per record is
~6× the bytes and a format call per record on hot threads); a crate (`tracing`, `perfetto` SDK):
they allocate per span or need a daemon, and the audio thread may not.

## D3. Recording never waits; at rest nothing wakes

A bounded MPMC ring of 65 536 slots (Vyukov: a sequence per slot, CAS on the head). A producer
writes its slot and publishes it; a full ring drops the record and counts it (the writer records
`Dropped`). No lock, no allocation after a thread's first record (which names the thread), no
system call but one `unpark` of the writer per burst: the writer parks after a drain that found
nothing; while records come it drains every 25 ms. The cpal callback records nothing (invariant 6).
Tests: t2142 (no context switch at rest), t2143 (a full ring drops and says so; recording does not
allocate, a counting allocator in its own test binary).

## D4. Bounded ring files

`<data>/trace/<start ms>-<pid>-<seq>.ktt`, 8 MiB segments, all runs together within 128 MiB (oldest
deleted at each new segment). A segment repeats the names, threads, machine and run records, so a
segment read alone resolves everything (t2141). A busy second of the owner's session made ~2 000
records (128 KB): 128 MiB is about 17 minutes of continuous heavy playback, hours of editing.

## D5. Ids join cause and effect

- A preview frame's id is the compose queue's ask number, process-wide (`trace::next_id`). The
  compose thread sets it as its cause (`with_cause`) for the whole composite: the decode requests it
  submits, its stream stage's run, its compute jobs carry it.
- A cache key's id is a hash of (source, size, index) with the top bit set (`key_id`): the frame's
  wait, the request, the decoded entry and the eviction name the same number; an eviction's cause is
  the key whose insert pushed it out.
- A prefetch pass, a recheck of kept moments, a job: an id of its own, set as the cause of what it
  does. t2144 holds the chain frame → request → decode across the compose and worker threads.

## D6. The class walk: every subsystem that waits or keeps

| Subsystem | Records |
|---|---|
| Preview composite (compose queue) | `FrameAsk` (tick, flags, the ask it replaced), `FrameDrop` (superseded, the model moved on, no picture, never taken), `FramePlan` (layers, missing in the cache, route, kept key), `FrameWait` (decode key, lock ≥ 1 ms, the device finishing the frame before), `FrameExec` (kept-composite hit/stored, stream frame held, slow device, cold pipelines), `FrameLayer` (each layer's content — what the kept key is made of), `FrameDone` |
| UI | `FrameShow` (the frame taken, complete, new, on the display), `Loading` (the scrim on/off), `UiFrame` (a frame of 50 ms or more only: hover frames record nothing) |
| Playback | `Play`: play, pause, seek, region, wrap, direction, reset, stall, underruns, start held for its renderer or sound — every push of the black box, repeats included |
| Decode queue (MediaWorker) | `DecReq` (key, priority, cause), `DecWithdraw` (withdrawn, cancelled by a seek), `DecSpan`, `Decoded` |
| Burst cache | `CacheEvict` (key parts, unread, still wanted, by which insert), `CacheInvalidate` |
| Stream stages (Datamosh) and their memo | `StreamRun` in a composite (position, positions lacked, rendered / held / short / source missing), `StreamAdvance` on the prefetch thread (from how far short, positions computed, stopped by a missing source), `StreamMemoEvict` |
| Compute host | `ComputeJob` (kernel, dispatches, device time, wait for its buffers) |
| GPU executor | the kept composites: `MemoStore`, `MemoEvict`; pipelines: `Warm`, `ColdDraw`; the pass times stay in the journal's GPU record |
| Audio render | `AudioFill` (frames live / from the kept mix), `MixChunk` (replayed, kept, not clean, and why the store could not hand it out: never kept, another scene, kept in part, in which context), `AudioBuild` |
| Prefetch | `Job` prefetch pass (asked, history pending), renderer rebuilds (`Invalidate` renderers) |
| Jobs | `Job`: thumbnails, effect thumbnails, reel previews, posters, analysis, export (whole and per frame), video renderer builds, the kept-moment recheck |
| Edits | `Edit` (the player's project version), `Invalidate` (renderers, kept mix, kept moments dropped, a source's entries) |
| Load watch | `Load`, `Work` (each counter that moved in a busy second); the journal lines mirrored as text on the same clock |
| Machine | cpu, cores, memory, OS, kernel, arch; per device a role opens (display, headless, export, compute): adapter, type, backend, driver and driver info, PCI ids, timestamps, pipeline cache, all features, limits |

Checked and left without records: the cpal callback (invariant 6 — its counters reach the trace
through the black box's underrun snapshot); the usage telemetry (nothing local happens there).

## D7. The query

`kt-cli trace [dir|file] [--run N] <question>`, the program's data directory by default:
`summary`, `loading [from to]`, `loop`, `during <from> <to>`, `slow [n]`, `frame <id>`, `machine`,
`dump` (JSON lines), `chrome <out.json>` (Perfetto / chrome://tracing). Times are seconds since the
process started, the journal's `+193.201`. t2146–t2149 hold the answers on synthetic runs.

- `loading`: the stretches — the UI's own scrim records when present, else from the first ask of a
  tick until a complete frame of it (≥ 180 ms) — and for each the waits ranked by time: the decodes
  the frames waited for (and whether they were evicted again inside), the stream history (composites
  short, advances, positions computed, stopped by a missing source), compute by kernel with device
  time, the stream memo's evictions, plans, the executor, locks, the device, warm-ups, builds,
  prefetch passes; then the critical path of the last composite and the journal around it.
- `loop`: laps from the playback records (region, play, wrap, pause); per lap frames, kept, composed,
  incomplete, the kept mix's replays and why the rest were live; for every frame composed again on a
  later lap the reason — evicted from the kept composites, the frame's key changed (naming the layer:
  decoded again, or another stream frame), a held stream frame, the slow device, incomplete, never
  kept before.

## D8. The journal stays, mirrored

The journal is what a person reads and what the load report exports (telemetry-journal); its lines go
into the trace as text records on the same clock (the journal's clock now starts at the trace's).
The load seconds are also typed (`Load`, `Work`). Nothing of the journal was removed.

## D9. Cost at rest

Nothing records at rest: no composite, no decode, no playback, no UI frame of 50 ms. A pointer over a
still place records nothing (hover frames are short). Measured on the heavy showcase — section
"Measurements".

---

## Stage 2: the cache defect (I8, I2/I5)

### What the trace showed (our fixture, `scripts/live/heavy_loop.py --build`)

The showcase's longest footage event under the owner's chain of event 2149 (Camera ISP → Datamosh gop
66, 720 lines, 8 px blocks, search 64, rotated vectors on a zigzag curve → VHS Tape), a 0.7 s loop,
`kt-cli preview … --loop 30 --trace`. `kt-cli trace loop` before:

- laps 1–14 composed again: "a stream stage showed a held frame" and "the frame's key changed since
  the last lap" — the stage ran behind playback, showed its latest held frame, and that composite is
  not the moment's own, so nothing was kept until the history caught up (lap 15, 16 s);
- every kept composite still cost a full plan: 706 stream-stage runs for 706 composites, 15.1 ms of
  plan per composite while the executor's replay took 0.3 ms — `FramePlan` 8.7 s against `FrameExec`
  0.4 s over the run.

The mechanism: **a kept composite was reached only through the work that made it.** The executor's
memo is keyed by the plan's key, and the plan of a stream layer carried the key of the frame the stage
RENDERED — so the plan ran the stage first. Where the stage's memo had dropped the history (the owner's
256 MB memo against 720-line positions of 5.5 MB) or the stage was behind (playing, slow rate), the plan
came out short or held, the kept composite was never found, and the frame showed "Loading" or was
composed again — on every lap, a hundred laps or not. The same held for a layer's decoded frame: the key
carries its content serial, and a frame evicted and decoded again got a new serial, so its kept
composite was lost too; and the slow-chain route (bytes from the offline device) kept nothing at all.

### The fix, as a class

1. **The key of a history stage is computed from its inputs** (`StreamKernel::key`, Datamosh:
   `mosh::key` — the decoded key of the chain from the rows and the source addresses, no frame, no
   memo). A new stream kernel that gives no key keeps the old path (the stage runs).
2. **The preview asks the kept composites first** (`PreviewParts::show_kept`): a probe plan
   (`VideoRenderer::probe_plan`) keys every stream stage without running it; if the display's executor
   holds that key, `present_kept` shows it with the display pass alone — no decode asked, no stage, no
   chain. Its key equals the full plan's whenever the full plan renders the stage's own frame; a probe
   can only hit what a full plan stored. The kept-moment recheck uses the probe plan too.
3. **A re-decoded entry keeps its serial** (`BurstCache::past`): an unchanged source gives the same
   bytes for the same key (invariant 11), so the entry takes back the serial it had when evicted;
   `invalidate_source` forgets them (new bytes at an old address keep getting a new serial, R.53 item 409).
4. **The slow-chain route keeps its composites** (`VideoGpuExec::keep`): a complete composite made as
   bytes on the offline device is written into the display executor's memo; the next lap is shown on
   the display from it (the route is not re-timed by a replay).
5. **The look-ahead skips kept moments** (`video_prefetch_wants_except`, `stream_advance_ahead`): a
   moment the bars show as kept asks neither decodes nor history — the video twin of the audio
   look-ahead skipping what the kept mix holds.

Walked for the same mechanism (a kept result reached only through the work that made it):
- the kept final mix (audio): replays by scene hash without rendering — not affected; the trace showed
  laps ≥ 2 fully replayed (34/0);
- frame blending and nested reels: their keys are already computed from addresses and revisions;
- pause, slow rate, loop, scrub: all go through `compose_via`, so all take the kept path first;
- export: composes every moment once and keeps nothing — not affected;
- thumbnails, reel strips, posters: their own memos keyed by content hashes — not affected.

### Cold pause

A paused frame never composed before still waits for its history: on our fixture 51 positions, 1.4 s
on llvmpipe (`kt-cli trace loading`: decodes done in 110 ms, then one advance on the prefetch thread
computing the positions serially, search 568 ms of device time). That is the effect's own cost, not a
cache's; the owner's run of this build will split it per position into device time, the job's wait
and the processor (the `ComputeJob` and `StreamAdvance` records).

## Measurements

(filled at the end of the change)

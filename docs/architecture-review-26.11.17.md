# Kinetren architecture review (2026-11-17, read-only)

Asked by the owner (kkt-26.11.17 S3): "я уже немного устал это делать каждый раз для тебя — ты подумай
отдельным агентом — может там фундаментально breaking changes какие-нибудь нужны? может архитектура у
нас не верная?"

Sources read: `CLAUDE.md`, `docs/architecture.md`, `docs/engine-audit.md` (the 2026-11-08 audit that
answered "a new engine?" with "no"), `openspec/specs/video-plan/design.md`, the archived designs of
`gpu-arbiter`, `gpu-throughput`, `cpu-idle-work`, `loop-laps-regression` and `compute-kernels-render`,
`docs/found-defects.md` (rows 690–712), the registries `kkt-26.11.10` to `kkt-26.11.17`, and
`docs/user-feedback.md` (the 2026-11-04 acceleration round). The owner's traces were queried with
`src/target/debug/kt-cli trace` (kt26.11.16 build) under
`/media/e/developer/kt.9/fails/26.11.16/{rendereg_2,problem_region_26_11_09}/new`. No file in the
repository was changed and nothing was built.

---

## 0. Verdict in one paragraph

The computation layer is not the problem: the frame plan, the WGSL kernels, the effects, the model,
the `.kt` format and the audio clock are sound. The 2026-11-08 audit was right about that.

The problem is the layer that decides when GPU work is submitted and waited for. It is built for
latency on a machine that has no card:
- it composes frame by frame, on demand;
- it holds at most half a frame period of work in flight;
- it spreads the work over several wgpu devices on one physical GPU, joined by host round trips;
- it adds a heuristic for each case that comes up.

His traces show the outcome directly. During the slow laps **his iGPU is busy 4–30 % of the time
and his 16-thread CPU about 1 %**, and every frame waits on coordination.

This layer has been re-tuned in about 45 changes since 2026-11-03. Every rule in it was designed and
accepted on llvmpipe in a VM with no card. The designs say the card case "is not reproducible here"
(cpu-idle-work D4, found-defects 692). That is why the owner has become our measuring instrument.

So the answer is yes, a breaking change is needed, but a targeted one, not a rewrite:
1. Replace demand-driven preview composing with a render-ahead pipeline: the export pipeline running
   ahead of the playhead, with many frames in flight, on one device per card.
2. Add a persistent render cache.
3. Get a machine with a real card of his class into the loop. Without it, every future rule is
   tuned blind again.

---

## 1. The actual shape of the playback pipeline

```
cpal callback (master clock) ◄─ ring ◄─ audio-render thread ◄─ BurstCache (pre-FX audio)
                                                     ▲
MediaWorker ×source (one decoder each, priority queue, generation cancel) ──► BurstCache (decoded frames)
                                                     │
prefetch thread (100 ms pass): look-ahead decode orders; Datamosh history walks → compute host jobs
                                                     │
preview-compose thread: ONE composite at a time, latest-wins ──► display device (egui's)   ┐
loop-fill lanes (sched "near"): up to 3 moments (cpu-idle-work D1) ──► sibling execs, same │ one physical
offline ("slow-chain") device: heavy chains                                                │ GPU ring,
compute host device: Datamosh encode/decode jobs, front passes                             │ admitted by
export devices                                                                             ┘ kt-gpu arbiter
kept composites (PresentMemo, RAM, memory-ledger budget) ──► the display shows kept frames on later laps
```

### Serialisation points (each verified in code or design)

1. **The display composes one frame at a time.** The compose thread is latest-wins: a newer request
   replaces a waiting one. This is its contract, kept unchanged in `cpu-idle-work` ("display
   composite … by contract").
2. **The arbiter's bound.** On a ring that carries the display, at most half a frame period of
   *predicted* device time may be in flight (`kt-gpu/src/arbiter.rs:46`, `bound()`). That is about
   16.7 ms at 30 fps. Lower-class work is cut into pieces at pass boundaries, with at most 3 pieces
   ahead (`AHEAD_PIECES`). In `gpu-arbiter` D4 each piece waited for the one before; this was
   later relaxed to admission by the predicted drain.
3. **The engine lock.** "A composite holds its engine's lock through its admissions and its
   readback" (cpu-idle-work D1). Parallelism therefore needs sibling executors.
4. **Several wgpu devices on one card.** The trace's machine facts show `gpu.display`,
   `gpu.headless` and `gpu.compute` all on RADV RAPHAEL. Results cross between devices through host
   memory:
   - found-defects 663: "the codec's picture still crosses the bus to the composite, required while
     the devices differ";
   - found-defects 693: Datamosh job buffers come back late.
5. **`poll(wait_indefinitely)` in the composite and compute paths** (`kt-gpu/src/engine.rs:1274,
   1825, 2524`; `compute.rs:400, 469`). Each one waits for every submission on that device, not
   just the caller's. Row 661 fixed one instance of this class.
6. **A stateful stream (Datamosh) walks history sequentially.** The prefetch thread walks one stage
   run at a time. The history has an inherent ordering; the rest of the pipeline adds no ordering
   that the effect itself needs.
7. **Decode.** There is one decoder per source (invariant 5). With long-GOP sources (vp9, hevc) a
   dense edit cut from one file decodes **five pictures per picture used**, with 128–157 backward
   reopens per 10 s (found-defects 712). The traces show decode is not what the loop waits for
   today (row 694), but it costs CPU and disk.

### What the owner's traces say (kt26.11.16-8c4ca513, Ryzen 7 7700X, 16 threads, iGPU RAPHAEL only)

His machine has **no discrete card**. Adapters: RADV RAPHAEL (an integrated GPU with 2 RDNA2 compute
units), llvmpipe, and GL radeonsi. The program picks LowPower, which means the iGPU.

**`rendereg_2/new`: a 22.8 s loop with VHS Tape, no Datamosh, preview at 640×360**
- `trace loop`:
  - lap 3 composed 295 frames and kept 30;
  - laps 3–6 still had frames that were never kept;
  - from lap 7 everything played from kept frames (~160 s).
- `trace during 28.5 51.3` (lap 4), from the journal:
  - each second: 19–22 frames composed at **10.6–13.1 ms of device time per frame**, "span" 36–44 ms;
  - LOAD 13–18 % of ONE core;
  - `sched-3` was busy the whole 22.8 s with 131 `loop_fill` jobs, about **174 ms of wall for each
    moment against ~12 ms of device time**;
  - `gpu_admit` waited 14.9 s out of the 22.8 s window.
- Whole run: the process load median is 89 % of one core (p90 149 %) out of 1600 % available.
- The device is busy about 12 ms × 22 per second, roughly **26 % of the card**.
- At 12 ms a frame the card could compose more than 60 frames per second at 360p. This loop is
  therefore **compute-feasible in real time on lap 1**, yet it took 7 laps.

**`problem_region_26_11_09/new`: Datamosh plus Camera ISP**
- `trace gpu`:
  - "pieces on the device 1764: **busy 10 035 ms**, idle between consecutive pieces 27 601 ms, of it
    25 035 ms with nothing queued", over a 272 s run. **The device was busy about 4 % of the run.**
  - Compute jobs ran p50 5.8 ms on the device, but their "buffers after end" took **p50 607 ms, p95
    1990 ms**.
  - Admission waits came to 346–486 % of the compute jobs' wall.
- `trace loop`: 14 laps of 11 s, and lap 14 still had 54 incomplete frames.

**What this proves:** the bottleneck is neither compute nor decode. It is the
submit → wait → admit → map → next cycle. By Little's law, throughput equals work in flight divided
by the round trip, and the bound plus the one-at-a-time stages keep the work in flight near one
frame.

"Device finishing the frame before: 78 ms" against 12 ms of measured pass time (`trace slow`) also
means part of each round trip is time **the program cannot see**:
- fence or poll latency;
- the desktop compositor sharing the iGPU;
- vsync.

None of this can be observed on llvmpipe.

**"The final render is slower than the preview"** (kkt-26.11.15 N3, found-defects 711) follows from
the same facts:
- the preview composes at 640×360, and the export at 1080p, which is 9× the pixels;
- the export runs on one device of the same 2-CU iGPU (`export_engines`: "a card gets one",
  `kt-gpu/src/lib.rs:244`);
- the export gets "only the slack" of the arbiter (loop-laps-regression D2).

Nothing in the UI tells him this. He sees it as a defect.

**"The CPU idles"** (N4) is true and structural:
- `compute-kernels-render` deleted every CPU video kernel (owner "удаляй", 2026-11-04);
- so on his machine all video math has to go through the weakest device he owns;
- even that device is fed at 4–30 %.

---

## 2. Candidate structural causes: confirmed or rejected

| Candidate | Verdict | Evidence |
|---|---|---|
| One decoder per thread through a priority queue (inv. 5) | **Rejected as the cause of the slow laps.** Partly confirmed for decode waste. | The traces show decode is not what the loop waits for (row 694, cpu-idle-work class walk). The real cost is long-GOP sources in dense edits: 5× wasted decode (row 712). The fix is intra-frame "optimized media", not a new threading model. |
| GPU as the only executor, with stateful history effects forcing sequential frames | **Partly confirmed.** | Datamosh history is inherently sequential, but its device work is ~6 ms per position while the wall per position is about 100× that (map latency, admission). The sequential nature costs little; the coordination around it costs the rest. "Only executor" matters on HIS hardware: a 2-CU iGPU next to 16 idle threads. Deleting the CPU composite cost the dev machine ~40 % (UF 2026-11-04: 96 s CPU against 134 s llvmpipe), and its cost on his machine was never measured. |
| Cache keyed per quality/scale | **Rejected.** | It is correct by design (video-plan design "decode quality is part of the request"). No trace row points at it. |
| Preview and export share one path but differ in parallelism | **Confirmed. This is the core.** | Invariant 7 unified *what* is composed, not *how it is scheduled*. The export is a throughput pipeline: bounded queues, chunks held, pipelined readback (`export_pipeline.rs`). The preview is a latency system: a latest-wins compose thread, fill lanes, gates, an arbiter bound. Two scheduling models need two sets of heuristics, and only the export's can be benchmarked faithfully offline. |
| No background render / prerender-to-disk model (VEGAS Selectively Prerender, Resolve Smart Cache, Premiere render-in-to-out) | **Confirmed as a missing feature with high payoff.** | Kept composites live only in RAM under the memory-ledger budget. They exist only after laps of a loop *played*, and are lost on restart, eviction and edits. There is no `docs/features` note for a render cache. Every heavy region is paid again in every session, by playing it repeatedly. |
| Telemetry that needs the owner to send traces | **Confirmed. The root is the missing card, not the telemetry.** | The trace is excellent: it answered every question above. The burden comes from every card-dependent rule being unverifiable here (rows 291, 406, 492, 560, 561, 692, 693; cpu-idle-work D4 "The card case is not reproducible here"). |
| Several wgpu devices on one physical GPU | **Confirmed (new, not in the candidates).** | It forces a host-side arbiter (`arbiter.rs` header: "A hardware card runs every `wgpu::Device` … through ONE graphics ring"), cross-device readbacks and uploads (row 663), and map/poll latency (row 693). |
| A constraint mislabelled as the owner's | **Confirmed (new).** | The half-frame bound is cited as "the owner's latency limit" (video-plan design: "Rejected: a larger bound (the owner's latency limit)"; loop-laps-regression D2). Its origin, `gpu-arbiter` D3, calls it "a general rule … the owner may want to revisit" (Open Questions). No owner line in `user-feedback.md` or `ktprompts` sets it. This rejected bound is the knob that caps the work in flight. |

### Why the defects keep coming back (the meta-mechanism)

1. **Rules are tuned on the wrong machine.**
   - Every performance change since 2026-11-03 was measured on llvmpipe in a VM, where "device =
     processor" and every device has its own queue thread.
   - On his iGPU the opposite holds: one ring, a separate fence/poll latency, a desktop compositor
     on the same ring.
   - Rules that are right on one are wrong on the other. cpu-idle-work D2 had to revert
     scheduler-followups D5 for exactly this reason.
2. **Heuristic layering.** The layers now in place:
   - the arbiter: classes, bound, pieces, drain prediction, `AHEAD_PIECES` = 3, quarter-bound floors;
   - loop-fill: width, narrow mode, display gate;
   - `FAR_STALLS`;
   - stream pacing ("a quarter of the cache");
   - memory-ledger orders;
   - about 16 `KT_*` A/B switches in kt-gpu/kt-engine alone.

   Each was right for one trace. Together they set throughput through their interaction, which no
   unit test holds. "Faster than kt26.11.10" regressed twice (kkt-26.11.15 N2, kkt-26.11.16 P1).
3. **Acceptance is by mechanism, not by utilisation.** A change is done when the mechanism found in
   his trace is gone and a test holds it. None requires "the card is ≥ X % busy while frames are
   missing", which is the property he actually notices.

---

## 3. Diagnosis burden: what would let us stop needing his traces

In order of payoff:

1. **A card of his class on our side.** This could be an AMD Ryzen 7000/8000 mini-PC or desktop with
   an RDNA2/3 iGPU, Fedora/Mesa RADV, as a self-hosted runner. Every card-only open row becomes
   reproducible (291, 406, 492, 560, 561, 692, 693). This is not "tuning to one machine" (owner
   H13): today we have *no* card, and the H13 rule is about designing for one specific box.
2. **A nightly real-time bench on that box**, built from tools that already exist:
   - `kt-cli showcase --heavy` and `kt-cli preview <range> --loop N --size 640x360`;
   - `kt-cli export --time`;
   - `kt-cli trace … loop|gpu`.

   It gates on:
   - laps until all frames are kept;
   - V-fps on lap 1;
   - device-busy share while frames are missing;
   - process CPU;
   - export × real time.

   It runs A/B against the previous release and against kt26.11.10. A regression is then found
   before he runs the build.
3. **A replica of his project shape.** `kt-cli replicate <project.kt>` would write a project with
   the same structure from synthetic sources:
   - event density;
   - cuts from the same file;
   - codec and GOP (vp9 long-GOP, generated by the ffmpeg binary as fixtures are);
   - effect chains with their parameters;
   - morphers and reels.

   His `.kt` without media is enough. `showcase --heavy` is a generic approximation, and row 712 /
   711 show it misses his shapes.
4. **In-program self-diagnosis.** The trace already holds the facts. The program should state the
   verdict itself.
   - Per played region, it records one of:
     - "device-bound: needs N ms/frame, budget M";
     - "decode-bound";
     - "**waiting**: device X % and processor Y % while frames were missing".
   - The third class is always a defect of ours. It would be written as a NOTE in the journal and
     counted in telemetry (PostHog already exists: `telemetry` spec), so it reaches us without him
     sending anything.
   - The Diagnostics window shows the same verdict in words.
5. **One-click report.** A "Send slowdown report" button packs:
   - the last N minutes of trace, journal and load report;
   - the `.kt` skeleton, without media;
   - the machine facts.

   It produces one archive, or uploads it on his consent. Today he picks folders by hand
   (`fails/26.11.16/rendereg_2/{new,old}`).

---

## 4. Recommendations, ranked by expected payoff

Sizes use the `docs/features` scale: S = one stage, M = 2–3 stages, L = a round.

### Breaking / architectural

**R1. A reference card machine and a utilisation gate** (process and infrastructure; payoff: highest,
because it fixes how every later change is judged)
- **What changes:**
  - a self-hosted runner with an AMD RDNA iGPU on RADV;
  - the nightly bench from §3.2;
  - perf changes are accepted on "device-busy share ≥ 70 % while frames are missing, and lap-1 V-fps
    ≥ X" on the card, not only on llvmpipe.
- **What breaks:** nothing in code. Orchestrator rules change (`docs/orchestrator.md`): a perf change
  is not done until it is measured on the card.
- **Cost:** hardware plus S to wire the runner and the bench.
- **Evidence it worked:** the card-only rows close here, and he is not asked for a trace for a
  regression the bench would have caught.

**R2. Playback as a render-ahead pipeline on one device per card** (payoff: very high on his
complaints 2 and 4 and the loop laps)
- **What changes:**
  - While playing (and in a loop), the preview runs the **export pipeline**: decode a chunk ahead →
    compose with K frames in flight (K from measured device time, not a half-frame bound) →
    keep in the frames memo.
  - The display only *presents* finished frames. The on-demand latest-wins composite stays only for
    paused, scrubbing and seeking.
  - Loop fill, the display gate, fill lanes, narrow mode and the arbiter's piece-cutting for preview
    work all collapse into one producer with one ordering: playhead first, then ahead, then the
    loop's rest.
  - **One wgpu device per physical adapter**, shared by display, compose and compute (Datamosh), so
    that:
    - textures and buffers pass between stages on the device (removes rows 663 and 693);
    - priority becomes the order of command-buffer submission from one submit thread, not admission
      across devices;
    - `wait_indefinitely` is replaced by per-submission fences.
- **What breaks:**
  - internal contracts: compose-thread latest-wins during play, `gpu-arbiter` multi-device rings,
    sibling executors, the `cpu-idle-work` lanes, and many tests that pin them
    (t2433, t2517, t2860, t2020–t2025 …);
  - the spec text of `video-plan`, `transport` and `telemetry` for those paths.

  Invariants 1–12 stay: 6 (audio clock), 7 (one plan, GPU executor), 11 (the same plan → the same
  bytes on one device) and 12 (no work at rest) all hold. The owner's rule "no stale or partial
  picture" (kkt-26.11.11 L2) holds: a frame not ready is not shown, and Loading names "rendering
  ahead N %". The "latency limit" must be put to him as a question with context (it was never his).
  The UI shows playback starting a little later on a cold heavy region, to fill K frames ahead.
- **Cost:** L. Suggested order:
  1. one device per card, and on-device hand-off from compute to composite;
  2. the render-ahead producer for play and loop;
  3. remove the fill lanes and the preview's arbiter cuts;
  4. trace and bench gates.
- **Evidence it worked:**
  - on `rendereg_2`'s shape (VHS, 360p, ~12 ms device per frame): real time **from lap 1**;
  - device busy ≥ 70 % while catching up, against today's 26 %;
  - on Datamosh: "buffers after end" p50 from 607 ms to under 10 ms, and device busy from 4 % to a
    majority while history is pending.

**R3. A persistent render cache, a "prerender"** (payoff: high; makes slow regions stop mattering)
- **What changes:**
  - Kept composites get a disk tier, keyed by the existing plan key plus the adapter/driver identity.
    This is the same as the frame memo's key and the pass-time disk cache's key.
  - "Render region to cache" (VEGAS Selectively Prerender) is an explicit action.
  - Optionally, background fill of regions the self-check marks "not real-time". This needs his
    decision against "the program at rest costs nothing"; he said in L4 "я не против чтобы оно
    наперед считало".
  - The coverage bars already show kept composites (invariant 4).
  - The export may reuse cached frames of the same device and size (bytes equal on one device,
    invariant 11).
- **What breaks:** nothing in `.kt`. It needs disk budget and eviction (a ledger tier), and a feature
  note in `docs/features/` first (project rule).
- **Cost:** M. It also benefits from R2, because the producer writes the cache.
- **Evidence it worked:** a heavy loop is real time from lap 1 after a restart; a cached range
  exports at encoder speed.

**R4. The processor as a second executor, by a deterministic split** (payoff: medium, uncertain on
his machine; **needs an owner decision on invariant 11**)
- **What changes:** export and the render cache (not the live display) may compose some *segments*
  on an llvmpipe device on the CPU's 16 threads, beside the iGPU. The split is by a rule that does
  not follow the load, for example alternate events or GOP-aligned chunks.
- **What it amends:**
  - invariant 11 ("byte for byte between runs on one device" becomes "on one machine, per segment
    rule");
  - video-plan design "Export does not mix backends". A seam at a cut is invisible; inside a shot it
    is forbidden.
- **Cost:** M.
- **Gate first:** measure llvmpipe on a Zen4 against the Raphael iGPU per effect on the reference box
  (R1). If llvmpipe is less than ~30 % of the iGPU, drop it.
- **Evidence it worked:** export × real time on his class of machine; CPU above 50 % during export.

### Incremental (in place, no contract change beyond the spec text)

**R5. Lift the half-frame bound now, as an A/B** (S, payoff: possibly large, cheap)
- `KT_GPU_ARBITER=off` already exists.
- Make the bound a function of the *measured* display frame cost, and give `near` K frames of
  in-flight work.
- Run on the reference box. If no box yet, ask him once with context: what the bound is, that it is
  not his rule, and what to watch.
- Fix the mislabel in `video-plan/design.md` and the `loop-laps-regression` design.

**R6. Replace device-wide waits with per-submission fences** (S)
- The `poll(wait_indefinitely)` sites listed in §1.5.
- One poller thread per device, so map callbacks fire without a waiter (row 693).

**R7. Intra-frame "optimized media" for long-GOP sources in dense edits** (M; the decode half of
"render slower")
- The verdict in `docs/features/media/proxies/proxy-toggle-workflow.md` was "take it when material
  shows". Row 712 is that material.
- In-process transcode (invariant 9 holds), used by preview and export alike, so the bytes are the
  same.
- An alternative is a decode-order memo per file for many cuts, which is the running `decode-waste`
  change.

**R8. Make the render's cost visible** (S)
- The render dialog predicts time from the pass timer's measured device ms × output pixels, and
  states "your preview runs at 640×360, the render at 1920×1080 (9× the work)". This closes row 711
  as understanding, not a bug.
- Optionally an export at preview size for drafts.

**R9. Self-diagnosis and a one-click report** (§3.4 and §3.5; S–M)
- The "waiting" class (device and CPU both idle while frames are missing) becomes a counted defect
  in telemetry, reaching us without his traces.

**R10. `kt-cli replicate`** (§3.3; S–M) so agents run his project's shape without his media.

### Not recommended
- **A new engine as a whole.** The 2026-11-08 audit's table still holds: `.kt`, the plugin API, the
  frame plan, the kernels and ~1358 spec tests are not where the defects are.
- **More heuristics in the arbiter, lanes or gates.** Each round adds one, and the interaction is the
  defect.
- **Re-adding a CPU copy of every kernel.** If the CPU is wanted, R4 gets it through llvmpipe without
  a second kernel text, and only after measurement.

---

## 5. Suggested order

1. **R1 and R5** at once: the box plus the cheap A/B. Within days this shows how much of his
   slowness is the bound alone.
2. **R6:** small; removes the 600 ms map waits.
3. **R2:** the structural fix, stage by stage, judged on the box.
4. **R3:** the render cache on top of R2's producer.
5. **R7, R8, R9, R10** alongside, on other files.
6. **R4** only if R1's measurement says llvmpipe adds real capacity on his class of machine.

## Questions to put to the owner (with context, per his rule)
- **The half-frame bound.** It was never his limit. Raising it may let background work delay a
  displayed frame by up to one frame. Yes or no?
- **Background render-cache fill at rest (R3).** This conflicts with "the program at rest costs
  nothing" unless it is explicit. Should it be automatic, a button, or both?
- **Invariant 11 per-segment split (R4)** for export only.
- **Buying or providing a reference machine** with an AMD iGPU (R1).

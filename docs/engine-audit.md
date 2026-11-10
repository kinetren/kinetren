# Engine audit — defects by mechanism, the core question, schedulers and JIT

Change `engine-audit` (2026-11-08), registry `docs/ktprompts/kkt-26.11.08-index.md` H10–H11.
The owner asked (line 16 of `kkt-26.11.08.txt`): «может нам учитывая все огрехи которые насобирали
за всю историю - придумать новый движок? а этот выкинуть просто как breaking change?», then (line 18):
«запускай разбор дефектов по механизмам ... по поводу планировщика - возможно даже не только GPU но и
CPU планировщик. И может быть задуматься JIT математики и вычислений?»

This is an analysis. It changes no product code. Numbers come from the archived changes, the specs'
design records, `docs/found-defects.md`, the executors' hand-over notes, or the measurements in §0.
A number with no source is marked **guess**.

Contents:
- §0 What was measured for this audit
- §1 Defects by mechanism (M1–M11), with the architectural/local verdict
- §2 Replacing the core or targeted fixes: (a) incremental engine, (b) one CPU+GPU scheduler
- §3 JIT of math and computation, every computation: CPU and GPU (the note is
  `docs/features/plugins/kernels/fused-kernels.md`)
- §4 Recommendation and the answer to "a new engine as a breaking change?"
- Appendix: every performance row of `found-defects.md`, mapped to its mechanism

## 0. Measurements made for this audit

Setup:
- Binary: release `kt-cli` kt26.11.08-7d22d084, the `ui-never-waits` branch tip (merged with
  `decode-request-storm` and `gpu-pass-cost`).
- Device: llvmpipe. Machine load 29–56 on 32 threads, because `finish.sh` was running alongside.
- Every run was capped with `systemd-run --user --scope -p MemoryMax=12G -p MemorySwapMax=0`.
- Projects: the relinked copy `repro/26.11.07/haski-relinked.kt` (1974 events, 8 morphers, 2 reels;
  9 audio files are missing on this machine) and our `showcase --heavy` (1357 events, 104 s).

| `kt-cli probe … --time` | haski-relinked (4 ranges) | showcase --heavy (3 ranges) |
|---|---|---|
| `effective_clips` | 1987 clips in 9 ms | 1357 clips in 1 ms |
| `AudioRenderer::new` | **393, 466, 466, 488 ms** | **1, 1, 1 ms** |
| `VideoRenderer::new` | 19, 25, 22, 36 ms | 6, 6, 13 ms |
| audio render, warm, linear 4 s | 0–4 ms (≤ 0.1 % of real time) | 0–15 ms for 2 s (≤ 1 %) |
| compose (decoded cache, 1080p, llvmpipe) | p50 0.17–14.2 ms | p50 52–475 ms, p95 985–2298 ms (VHS, ISP, Slowed stacks) |

**Finding (new, row 151 of `found-defects.md`): the whole-engine rebuild is dominated by one
quadratic function.**
- How it was sampled: perf is not permitted here (`perf_event_paranoid` = 4). Instead, gdb was
  attached and interrupted at 80, 200 and 320 ms into `AudioRenderer::new` on haski.
- All 3 of 3 stacks stood in `kt_model::warp::segments`, called from `warp::sign_knots`
  (`src/libs/kt-model/src/warp.rs:405-460`). That is reached from `kt_fx::audio::chain_addr_knots`
  (`src/libs/kt-fx/src/audio/mod.rs:1399`), called per clip in `build_reel`
  (`src/libs/kt-engine/src/audio.rs:738`).
- Why it is quadratic: a speed curve with a shaped segment or a formula layer is sampled on a 5 ms
  grid (`rate_profile_keys`, `RATE_GRID_TICKS` = 450, change `slowed-reverb-is-an-event` D2), so a
  shaped segment of tens of seconds becomes thousands of points. `sign_knots` then calls
  `dir_after`, `consumed_abs` and `source_progress` for every point. Each of those rebuilds the
  whole segment list (`segments`, `abs_segments`, a fresh `Vec` each time), so the cost is
  O(K²) per clip.
- Where the dense curves are: on haski, five events carry such curves (event 281: 360 s with 3 shaped
  segments; 2132/2133: 83 s; 2145/2146: 32 s). The showcase has none, and it builds the whole audio
  engine in 1 ms.
- **Guess:** a linear `sign_knots` (one sweep, or the prefix sums the warp map already keeps at
  `warp.rs:~520`) brings haski's `AudioRenderer::new` from ~450 ms to the showcase's order (a few ms).
  To be measured by the change that fixes it.
- Consequence for §2: most of the 412–447 ms "whole rebuild per edit" that rows 110 and 117 report is
  this one function, not the size of the project. The incremental engine is still needed (§2a), but
  it is no longer the only way to make an edit cheap.

## 1. Defects by mechanism

Walked:
- every row of `docs/found-defects.md` (open and closed);
- the archived performance changes `idle-cpu`, `input-without-work`, `ui-frame-cost`,
  `cached-range-plays`, `telemetry-journal`, `audio-fx-cost`, `ui-never-waits`, `gpu-pass-cost`,
  `decode-request-storm`;
- the older changes `video-prefetch-thrash`, `sidecar-fsync-stall`, `preview-throughput`,
  `export-throughput`, `cold-start-audio`, `morpher-event-grid-cost`, `slowed-reverb-bimodal-cost`,
  `render-speed-no-card`, `export-pipeline-determinism`, `compute-backends`, `compute-kernels`,
  `compute-kernels-render`, `gpu-render-hardening`, `test-stability`, `r68-verification-speed`,
  `r69-live-gesture-mechanics`;
- the design records of `transport`, `audio-render`, `video-plan`, `video-fx`, `media-index` and
  `compositions` (R.44–R.46, item 301, the prefetch history);
- the owner's claims in `docs/ktprompts/` (C1, C2, E1–E11, G1–G12, H1–H11) and `docs/user-feedback.md`.

`hover-static-load` was never a change of its own: its code shipped inside `idle-cpu`, and its rest
is row 107.

**ARCHITECTURAL** means the structure produces the mechanism again wherever new code is written: a
new effect, panel or service meets it with no one deciding to. **LOCAL** means a fix at its site
stays fixed.

### M1. The whole engine is rebuilt per edit, by every consumer — ARCHITECTURAL

What happens:
- A model edit hands the player a full project snapshot (`KtApp::push_project_to_player` →
  `model.document()`, a clone on the UI thread).
- Each consumer then builds its own engine from it:
  - the UI thread builds a `VideoRenderer` (row 150), and possibly a second one through
    `sync_generators`;
  - the `audio-build` thread builds an `AudioRenderer`;
  - prefetch builds its own `AudioRenderer` and `VideoRenderer`;
  - the probe (meters, loop head) builds another `AudioRenderer`.
- One edit therefore costs 2 audio and 2–3 video whole-project builds (code map,
  `player.rs:416-567`, `:2309-2318`).
- Every derived quantity a new feature adds to `build_reel` is paid per edit, by every consumer.

| Defect | Cost measured | State |
|---|---|---|
| region edit cloned the project into 1961 events and stopped the stream (transport design) | — | closed: `set_loop_region` |
| prefetch cloned the project every 100 ms (transport design) | "lag at pause" | closed: version-gated |
| a no-op input made an edit; every edit rebuilt the engine (input-without-work D1) | 41 builds and 12 swaps in 10 s; prefetch 69 %, audio-render 68 % | closed for no-op edits (t1870–t1880) |
| swap on the audio render thread (cached-range-plays D4) | `AudioRenderer::new` 412 ms against a 341 ms ring; 1.2 M chain-tail walks per swap 244–425 ms → 0.6 ms | closed: builder thread (t1935) |
| row 110: a real edit rebuilds the whole audio engine | 447 ms on two threads per edit (release, his 1974 events); 393–488 ms in §0 | **open** (mostly row 151) |
| row 111: the first meter read after an edit builds on the UI thread | (a build) | **open**, later reads moved to `probe-build` |
| row 117: setting a loop region rebuilds on the UI thread | 412 ms per region change | **open** (the first probe) |
| row 150: every edit builds a `VideoRenderer` on the UI thread | 23 ms debug; 19–36 ms release in §0 | **open** |
| R.59 owner rule "a simple project must not lag; one heavy source must not weigh the whole project" (UF:1184; "video with curves clones the chain per frame", UF:1193) | — | the rule stands, kept by hand |
| `invalidate_source` per keystroke (media-index, t497) | — | closed: index |

Why architectural: nothing in the structure says what an edit touched. Every new derived datum is
paid per edit, in every consumer, and the only thing that would notice is a measurement (`probe`
prints the build cost since `ui-never-waits`). Row 151 shows it: a feature added on 2026-11-04
(shaped speed curves) multiplied the edit cost of a project that uses it by ~400×. Compare 1 ms on the
showcase with ~450 ms on haski.

What still holds well: the products of a build are already content-keyed where it matters. That
covers the kept final mix (scene hash of the events heard, cached-range-plays D2), the kept composites
(`plan_key(plan)`, `kt-gpu/src/engine.rs:1576`) and FX state (diffed by clip id and fingerprint,
invariant 3; the per-clip `fp` is computed at `audio.rs:855-875`). An edit elsewhere therefore keeps
the cache warm. **The waste is the build, not the caches.**

### M2. An O(whole) helper inside a loop over its parts — LOCAL, recurring

What happens: a per-item loop calls something that rescans or rebuilds the whole structure, so the
cost is O(N·K). It is invisible on hand-made data and explodes on generated data (a 5 ms grid, a
2 s window walked per frame, a project of thousands of events).

| Defect | Cost | State |
|---|---|---|
| row 151 (new): `sign_knots` rebuilds the segment list per point | ~450 ms per build on haski (§0) | closed by `builder-cost-guard`: 368–572 → 11–12 ms, bytes identical, counting guard t2060–t2067 |
| row 144: prefetch calls the stage's `needs` per frame of its 2 s window | 160–280 ms a pass against a 100 ms target | the stream's share 12–20 → 2.2–3.6 ms (`stream-layout-per-build`); the rest of the pass to read in his trace |
| LFO walks the rate-curve keys per sample (audio-fx-cost walk) | O(keys) per sample | **open**, not in found-defects before; listed there now under 151 |
| morpher grid: 360 identical tiles computed per sub-sample (morpher-event-grid-cost) | 8.3 M sub-samples a frame; 274 → 2.1 ms CPU, 12.0 → 3.4 ms llvmpipe | closed (t942–t946) |
| `merge_burst_ranges` (media-index, t287) | 885 coverage requests under a mutex → 1 | closed |
| raw reverse eternal miss (media-index, t285) | up to 1024 frames per skipped burst | closed |
| overlap weights by hash per frame (audio-render design) | 40 % on small projects | closed: arrays by track |
| ui-frame-cost: a frame's work grew with the PROJECT (a document copy per question, a walk of every event) | 6.1 → 1.6 ms a frame; effects window 291 ms → 5.0 ms | closed (t1910–t1914) |
| row 107: a hover still lays events out | 58 of 111 frames over a composition | **open** |

Why local: each site is fixed once. It recurs because builders and per-frame paths have no cost
guard. A count-based test (calls of `segments` per build, passes per hover frame) catches the next one
without a clock (t1671's rule).

### M3. A thread waits for work whose length it does not control — ARCHITECTURAL

Where it happens: the UI thread, the audio render thread or a sidecar holder blocks on a composite, a
GPU job, an fsync, a build, a lock held through a build, or a file open. The defect class behind
"interface frozen" and "cannot Undo" (owner E8: «я просто БУКВАЛЬНО НЕ МОГУ ОТМЕНИТЬ СВОЙ ЖЕ ЭФФЕКТ»).

| Defect | Cost | State |
|---|---|---|
| ui-never-waits: composite on the UI thread; Datamosh `poll(wait_indefinitely)` per job | UI blocked 1019, 4509, 3704, 3343, 3232 … ms (his journal, RADV) | closed: `compose_queue` (t1840–t1848) |
| locks through builds: project lock, `workers` guard through a decoder join, `natives` through `VideoRenderer::new_with_facts` | lock hold 385–412 ms | closed site by site |
| preview-throughput D2: the UI waited on the device each frame | 11.5–13.2 ms wait + 3.5–4.1 copy per composite | closed |
| sidecar-fsync-stall: SQLite lock held across fsync | one fsync 137 s | closed (t1460) |
| auto-ducking on the UI thread (media-index history) | tens of seconds | closed |
| true-peak on every opening (audio-render) | 4.4 s for an hour-long file | closed |
| row 98: Ctrl+S writes and syncs twice on the UI thread | 6.7 s at load 76 | **open** |
| row 148: source opens and decoder joins on media gestures (UI thread) | — | **open** |
| rows 111 and 117: the first probe build on the UI thread | 412 ms | **open** |
| row 147: a composite the UI has left runs to its end (no cancellation point in `StreamKernel::render`) | 4 cores for seconds after a pause | closed: `StreamInput::stop`, the compose, prefetch and pre-roll stop checks (`gpu-arbiter`, t2257–t2259) |
| rows 105 and 112: interface stalls of 10–30 s at 0 % processor | — | **open**, one cause removed (geometry dump) |
| rows 99 and 100: two Space presses or an Esc inside one slow frame | — | **open**: inputs judged on the frame clock |

Why architectural:
- The `ui-never-waits` walk ended with a list of what is still on the UI thread: `compose_news`,
  `init_video_gpu`, `park_offline_engine`, `VideoRenderer::new_with_facts`, source opens, Ctrl+S
  fsync, autosave's `document()`, Open/recovery joins.
- Each fixed site was moved by hand. Nothing stops the next panel from calling a blocking API: no
  type, lint or counter.

The structural answer is that the UI and the render thread may only post jobs and read results
(§2b), plus a counter of blocking calls on those threads that a test reads.

### M4. Work is ordered faster than it is consumed — ARCHITECTURAL

What happens: a producer sizes its demand by itself, not by what its consumer (a cache, a decoder, a
device) can hold or do. Requests are decoded or computed and evicted unread, then ordered again in a
circle.

| Defect | Cost | State |
|---|---|---|
| video-prefetch-thrash: window by ONE frame, not by stacked layers | 22 025 / 22 960 drops unread → 0; V-fps 13.9 → 25.7 | closed: byte window, refresh, withdraw (t1416–t1418b) |
| decode-request-storm: a stream stage ordered its whole history (600 positions × 8 MB vs a 512 MB cache) | ~1570 requests/s in his journal; 88 653 requests for 2 026 decoded in 30 s → 270/269 in 60 s; a composite of 8 970 ms → 111 ms | closed: `StreamPacing` (t2040–t2045) |
| export-throughput: chunk gate "two chunks ahead" | two of 16 workers busy, 337 s → 95.6 s with a byte budget | closed |
| export-pipeline-determinism: look-ahead past the cache, lost layers 5–10 per run | — | closed: holds (t1600–t1605) |
| cold-start-audio: ring filled before the decoders | 98 511–126 903 underrun frames each start → 0 | closed: start grace (t1537) |
| prefetch not folded into the loop (transport design) | "dozens of orders for one burst" | closed: `lookahead_windows` |
| idle-cpu D1: the preview service evicted a live reel release and ordered it again | 236 % of a core against 46 %; 66 submits/s | closed: previews settle (t1830) |
| media-index t69: an unpublished failure made the UI order again every frame | opens grow with FPS | closed |
| row 114: thumbnail and effect-cell caps go round (384 / 256) | — | **open** |
| row 134: ~45 GPU submits/s from Datamosh jobs prefetch runs far ahead | 35–49 submits/s; 12–13 jobs/s on our mosh showcase, all the look-ahead's, with and without the rings | ordered (near behind now) and stopped when left by `gpu-arbiter`; the count is two jobs per history position — **open** for `stream-layout-per-build` |
| row 143: the Datamosh memo (256 MB) holds ~20 positions at 1080 lines | a long loop recomputes history every lap | **open** |
| `thumbs` queue is LIFO and unbounded, with no pause while playing (code map A2) | — | **open**, not measured |

Why architectural: each fix sized ONE producer against a cache it does not own. That covers the
prefetch window (½ of the video cache), `StreamPacing` (¼ of it) and the export holds. The producers
that remain (the thumbnail service, the reel strip, posters, fx-thumbs, the next stream effect) meet
the same cache with their own rules. Separately, there are ten queue disciplines (see §2b).

### M5. Two subsystems share one resource with no arbiter — ARCHITECTURAL

| Defect | Cost | State |
|---|---|---|
| one decoder shared under a mutex (media-index history) | seek ping-pong | closed: invariant 5, `MediaWorker` |
| export D9: one file at two places → one decoder ping-pong | 238 reopens; 101 009 → 9 190 pictures | closed (t1455) |
| preview D1: one moment decoded once per size | 42 654 → 10 934 pictures, 661 → 113 reopens | closed |
| preview D5: the Playback request for the frame on screen stood behind the window | 38 026 underrun frames at 50–53 s → 0 | closed |
| sidecar `open_gate`: twelve openers, one to three failed after 5.7–6.2 s; t872 SIGBUS deleted a live file | — | closed |
| compute host: jobs from prefetch, compose, reel/poster pre-roll and export on one queue, no priority, no lock across a job (code map §4) | each poll waits for the other's work | closed: jobs admitted by the caller's class, cut between dispatches while a higher class waits (`gpu-arbiter`, t2255) |
| offline device shared by the slow preview route, fx-thumbs, reel strips and posters under one `inner` mutex (code map §4, inference) | not measured | the ring part closed (`gpu-arbiter`); the mutex stays: row 290 |
| row 149: on his RADV the display and offline devices share one graphics ring; one long submission holds the display | reproduced on ONE llvmpipe device (t2254) | closed: one ring per card, lower classes cut into measured pieces (`gpu-arbiter`, t2250–t2254) |
| row 94: render jobs set the process-global `DECODE_THREADS` / `RGBA_THREADS`; a preview source opened meanwhile takes them | — | closed: settings scoped to the render's opening thread (`gpu-arbiter`, t2262) |
| gpu-render-hardening: each extra device +1.3 GB at 1080p; "eight devices took 6 GB more and gained nothing" | — | closed: ceiling cores/8, 2–4 |

Why architectural: every resource added since v1 got its own owner and its own queue. The resources
are decoders, three kinds of GPU device, the compute host, the sidecar and threads of conversion.
Priority between owners exists only inside `MediaWorker`.

### M6. Work at rest: a timer or the UI frame instead of an event — partly ARCHITECTURAL

| Defect | Cost | State |
|---|---|---|
| idle-cpu: "one job per UI frame — the frame was the scheduler"; 4 Hz tick; `last_compose_retry` never read; meters' cap at 60 fps | 234–244 % → 0.73 % of a core (0.40 audio output + 0.33 load watch) | closed (t1830–t1836) |
| transport: the render thread spun 500 times a second | — | closed: condvars |
| R.31: a freeze every ~30 s, "cpu audibly straining" | — | closed long ago |
| row 122: load watch wakes once a second | ~0.4 % of a core | **open** |
| row 140: ~70 repaints a second after a pause, nothing composing | llvmpipe 500 % (bench only) | **open**, check outside the bench |
| timers that still run at rest (code map §9): prefetch 100 ms, index-service 200 ms, audio-render 250 ms, media-watchdog 900 ms, load watch 1 s, fx-thumbs 1 s, thumbs 2 s; compose_queue busy-polls the device every 1 ms after a present | each small, not measured one by one | **open** |

The rule "the program at rest costs nothing" (CLAUDE.md) is enforced today by the live `idle`
scenario and a count. Each new service brings its own `wait_timeout`. **Architectural** in that
sense; each timer is a local fix.

### M7. Per-element recomputation of per-frame / per-block constants — LOCAL, recurring

| Defect | Cost | State |
|---|---|---|
| audio-fx-cost: a glibc call per sample (`floor`/`ceil`/`round` on the SSE2 baseline, 15 sites in VHS sound); `pow`/`exp` of unchanged values | VHS sound 52.8 → 22.2 reference kernels; gate 5.8 → 0.6; limiter 6.0 → 1.2; distortion 4.0 → 0.35; bitcrusher 3.4 → 0.4; his chain 83.7 → 51.4 (3.9 → 2.5 % of a core) | closed (t1980–t1982) |
| row 128: Phaser recomputes sin/pow/tan per sample | 6.2 reference kernels, ~0.3 % of a core | owner's decision (it is the sweep's sound) |
| row 129: Compressor takes log per sample | 4 reference kernels, ~0.2 % | **open** |
| gpu-pass-cost: Camera ISP re-queried `textureDimensions` and divided per neighbour | 115–129 → 92–97 ms a frame (llvmpipe) | closed (t1217) |
| gpu-pass-cost: VHS head read re-queried the width per clamp | −10 % | closed |
| row 137: radial blur spin computes 16 sin/cos per pixel | 4 of 26 ms the pass (1080p llvmpipe); a device table cost more (27.8 ms), a CPU table moves bytes | closed: not the cost (local-cost-queue D2) |
| row 136: Camera ISP behind a K× barrier computes every sensor pixel k² times | K = 2, 1080p frame: camera passes 388.8 → 120.7 ms | closed: sensor passes at the sensor's size, same bytes (t2310, t2311) |
| render-speed-no-card D3: a VHS loop llvmpipe could not unroll | s7 32.3 → 10.4 ms, s1 11.0 → 5.1 | closed |
| slowed-reverb-bimodal-cost: the test stimulus matched per sample | 104 vs 415 ms per 10 s; every older `synth:` benchmark inflated | closed (t991) |

Why local: each kernel is written by hand, so each was fixed by hand. This is the class where
specialization and JIT live (§3). The measured gains show what is at stake: for ALU, 2–10× on single
audio kernels, and −25 % on one GPU simulation.

### M8. Allocation or resource creation per frame, compile in the frame — LOCAL

| Defect | Cost | State |
|---|---|---|
| preview D2: premultiply into a new allocation each frame | (part of 11–17 ms) | closed |
| gpu-pass-cost: no resource after warm-up (t2023); pass timer buffers grew with device lag (t2025) | — | closed |
| row 138: a bind group and a uniform write per pass on the composing thread | 6.7 + 2.2 µs a pass (llvmpipe); composing thread 1.62 → 1.29 ms a 1080p frame | closed: uniforms staged per submission, bind groups kept a frame (t2313, t2314) |
| row 139: a new geometry shape compiles its pipeline inside the frame | a hitch; llvmpipe `glow:resample` 2 s at its first draw | closed: the interpreter runs the frame, the shape compiles beside (pipeline-prewarm, t2121) |
| row 64: the first Datamosh frame — device open + shader compile, no disk cache | 6.6 s under load; 62–209 ms open + 247–690 ms intra with an empty Mesa cache | closed: prewarm at project open, compute shaders named and prepared (pipeline-prewarm, t2120, t2122) |
| export memory: glibc's dynamic mmap threshold | 16.2 / 14.9 GB → 7.3 GB | closed (t1458) |
| cached-range-plays D6: nest streams per placement | the test process reached 55 GB; 1.3 GB a second → 172 MB | closed (t1937) |
| row 96: upload makes two copies | ~12 ms a frame (software device) | **open** |

Why local: guarded by t2023/t2024 once fixed. The compile part is a cache question (§3 c).

### M9. A cache keyed or ordered by something other than what determines the result — partly ARCHITECTURAL

| Defect | Cost | State |
|---|---|---|
| LRU ranked a prefetched frame by insertion (video-prefetch-thrash) | see M4 | closed: refresh |
| reads refreshed dead frames ahead of live ones (export-pipeline-determinism) | 5–10 missing layers per run | closed: holds |
| post-FX memo in export evicted neighbours' pre-frames (video-plan design) | black render with rare frames | closed: memo preview-only |
| serials per cache: one project's frame taken for another's (t1574); `UploadCache` keyed by the base serial (t250) | wrong pictures | closed |
| reel preview: a cancelled job left nothing on disk; a starved frame stored under the key (compositions) | rendered anew at every opening; "a black reel forever" | closed |
| cached-range-plays: a covered range composed and mixed again every lap (composite memo only on the readback path; post-FX mix not kept) | V-fps 13.6 → 50.5; device time 72 → 3.3 ms a frame; 23 → 2.5 cores | closed (t1930–t1937) |
| loop pre-roll on every lap (audio-render) | ≈1.25 s per chain → ≈0.3 ms | closed |
| row 118: the first loop pass composes every frame in full | 72 ms a frame (llvmpipe) | **open** |
| row 119: a frame evicted and decoded again misses its kept composite | — | closed: the serial returns (structured-telemetry, t2152), held through the executor (memory-ledger, t2294) |
| one budget per cache, no ledger across them: live BurstCache 512 + 64 MB, PresentMemo 512 MB per engine, Upload 256, Readback 64, transient pool 128, StreamMemo 256, MixStore 64, compute pool 1 GiB, reel 64, poster 16 (code map §7) | rows 10 (export 7.3 GB peak), 85 (4K unproven), 96 (8.5 against 7.3 GB), 310 (a loop past 582 composites at 640×360 composed every lap) | closed: one ledger (`kt_time::mem`), growth from half the free memory measured at run time in a stated order, the present memo sized by the loop, an export's claim and frames-in-flight uploads (memory-ledger, t2290–t2301); the bases stay (row 330) |

Each key defect was LOCAL and is closed. What remains structural is the **budget**. Ten caches each
pick their own size, and nobody sees the sum or evicts by the value of a byte across them.
memory-ledger (2026-11-09) closed it: every keeper reports to one ledger, growth above the proven
bases comes from one run-time budget in the order kept composites → stream history → decoded frames,
and an export takes it back while it runs.

### M10. Work nobody measured — a missing or wrong counter — LOCAL per site, now mostly instrumented

| Defect | State |
|---|---|
| "thrash" counted every unread drop; an empty memo lookup counted as a miss (41 % / 57 %); underruns in two units (24 000 vs 24); empty passes before the first frame | closed |
| rebuilds had no counter, so the load watch called 196 % "at rest" (input-without-work) | closed: `engine_builds`, `model_edits`, `project_swaps` (t1880) |
| no compose-time counter (morpher grid) | closed: `FrameCost` |
| "work the device does that nobody measured" (gpu-pass-cost) | closed: pass timer, `GPU` journal record (t2020–t2022) |
| notes stamped late or in UTC (telemetry-journal) | closed: one journal (t1960–t1969) |
| row 53: dropped-frame counter on the CPU path | open (the CPU path is deleted; to recheck) |
| row 108: no load number proved on a real card | **open**: a confirmation, not an input to the design — every rule in §2b and §3 is general or measured at run time on the machine that runs it, and proved on our benches |

Why it matters for §2: the next structure should count by itself. A scheduler counts every job it
runs, and at rest it counts zero.

### M11. Tests that depend on the wall clock or on load — LOCAL, structurally guarded

What happened and was fixed:
- r68: 96 % of a live run was sleep. r69: 387 s of 1582 s were gestures that slept.
- test-stability: bounds of 20–60 s fired on correct code, because one fsync took 44 s. Guard t1671
  now refuses any clock limit not on its list.

What escapes:
- row 126: t1914 measures wall-clock frame cost, 8.5 against 4.2 ms at load ~60;
- row 101: `space_hold` runs on 300 / 400 ms windows;
- rows 106, 127, 130, 133, 141 and 146 are single reds under load;
- row 89: `nextest` hides a process-wide lock.

Local per test. Row 126 belongs under t1671.

### What the walk does NOT find

No defect was traced to the computation itself being built wrongly. The computation is
`frame_plan` and the GPU executor, the audio chain and FX state, the decoders, the BurstCache and the
export pipeline. Its defects were local costs inside kernels (M7) or keys (M9), each closed under a
test. Every architectural mechanism (M1, M3, M4, M5, the budget part of M9, the timer part of M6)
lives in the **coordination layer**: who builds what, when, on which thread, for whom, and with
whose resources.

## 2. Replacing the core or targeted fixes

### (a) A persistent incremental engine (`edit-without-rebuild`, t2000–t2019 reserved)

**What it would buy, by the numbers we have:**
- Today one edit pays, on haski: ~450 ms audio build × 2 consumers (render, prefetch) plus a probe
  build on the next meter read, 19–36 ms × 2–3 `VideoRenderer` builds (one on the UI thread), and a
  document clone on the UI thread (not measured alone; project copies were 53 % of `draw_all` before
  ui-frame-cost).
- An incremental engine makes an edit cost what it touched. A slider step on one event of 1974 then
  rebuilds one event's derived state. **Guess:** well under 1 ms. That would allow 60 edits a second
  in a drag on any project size.

**What a targeted fix buys first:**
- Row 151 alone removes most of the 450 ms (§0). The showcase, with 1357 events and no dense speed
  curve, builds its audio engine in 1 ms and its video engine in 6–13 ms.
- So after a linear `sign_knots`, a whole rebuild of a ~2000-event project is likely a few ms of
  audio plus ~20 ms of video (**guess**, to measure).
- The remaining per-edit cost is then the multiplicity (2 + 2–3 builds), the UI-thread video build and
  clone, and the next quadratic someone adds.

**Verdict:**
- Do it in three steps, all in place, under the existing tests:
  1. The quadratic fix plus a count-based cost guard on the builders.
  2. **One build per project version, shared** (`Arc<AudioRenderer>`, `Arc<VideoRenderer>`) by the
     render thread, prefetch and the probe, built off the UI thread. The UI thread passes an
     `Arc<Project>` (or a revision) instead of cloning.
  3. **Per-event derived state memoized by the existing per-clip fingerprint `fp`**, so a rebuild
     reuses every unchanged event's warp knots, chain context, run predecessor and preroll. This is
     already the shape of `sync_fx_since`, which diffs by `fp`.
- A full persistent incremental structure (an edit patches the index in place) is step 4, worth doing
  only if a 10× synthetic project still misses the budget after step 3.
- None of this changes the `.kt` format, the plugin API or a test's expectation.

### (b) One scheduler for CPU and GPU work

**What exists (code map, `src/` as of 76dc3576):**
- Engine, one set per `Player`:
  - `media-worker-<id>`: one thread per source; priority queue `Playback < NearPrefetch <
    FarPrefetch < Thumbnail` (only the first two are ever submitted); generation cancel plus
    `withdraw_video`.
  - `audio-render`: ring of 16 bursts; sleeps 8 ms playing, 250 ms paused.
  - `audio-build`: one build in flight, edits coalesced.
  - `prefetch`: a 100 ms poll; also advances Datamosh stream stages, so it submits compute jobs.
  - `preview-compose`: latest-wins, one composite at a time; slow chains go to the offline device.
  - `probe-build`.
  - The export pipeline: bounded FIFO channels, its own devices.
- App services, each with its own discipline:
  - `thumbs`: LIFO, unbounded, no generation, no pause on play.
  - `fx-thumbs`: LIFO plus generation, offline device, 25 ms sleep while busy.
  - `kt-posters`: FIFO-replace, offline device, pauses.
  - `reel-preview`: one BgJob at a time, offline device, pauses.
  - `index-service`: FIFO, 200 ms poll.
  - `analysis`: dedup FIFO, no pause.
  - `autosave`, `media-watchdog` (900 ms), `load-watch` (1 s), telemetry, journal.
- GPU: the display device (egui plus preview-compose); one offline device per process (slow
  preview, fx-thumbs, reel strips, posters); one compute host per process (Datamosh jobs from
  prefetch, compose, pre-roll and export, no priority, `poll(wait_indefinitely)`); export devices.
  No wgpu `PipelineCache`; every new device recompiles every pipeline.
- About ten queue disciplines and seven timers at rest. Pause-on-play is uneven, and there are
  four entry points to one compute queue.

**Where they contend (measured or recorded):**
- row 134: ~45 submits/s from prefetch's Datamosh look-ahead;
- row 147: a composite the UI left runs to its end;
- row 149: one RADV ring for display and offline;
- row 144: the prefetch pass at 160–280 ms because it advances a stage per frame;
- row 143: the stage memo against the loop length;
- row 94: global decode thread settings;
- rows 114 and 122 plus the timers at rest;
- M5: offline-device sharing (inference).

**Sketch.** One `Scheduler` owns resource pools and job classes. It decides WHEN and WHERE, never WHAT.

*Not scheduled (unchanged):*
- The cpal callback (invariant 6: lock-free ring, no allocation).
- The audio render thread's ring fill. It stays a dedicated thread; the scheduler only gives its
  decode requests the top class.
- Decoder ownership: one owner thread per source (invariant 5). The scheduler orders each owner's
  queue, which generalizes `MediaWorker`'s priority model.

*Job classes, in priority order:*
1. **Audible / visible now**: Playback audio decode; the frame on screen (its decode, its stream
   positions within `compose_steps`, its composite); the new engine version's build.
2. **Answer the person**: probe/meter values, an opened window's previews, a gesture's file open.
   Never on the UI thread.
3. **Near**: the prefetch window (audio first), stream history ahead of the cursor, the loop's next
   lap.
4. **Visible background**: filmstrip/bin thumbnails on screen, fx-thumbs of an open picker, reel
   strips on screen.
5. **Background**: posters, analysis (LUFS, peaks, index), far prefetch.
6. **Offline**: export, as its own pool with its own devices, registered for cores and memory.

*Resources:*
- A CPU worker pool of N cores, minus the audio render and UI threads.
- Decoder owners.
- Per GPU device, one submission queue with class priority. Each submission is bounded by
  measured device time, using the pass timer from gpu-pass-cost; a long chain is cut at pass
  boundaries, which answers row 149.
- The compute host, with jobs tagged by the class of whoever asked.
- A memory ledger across every cache (M9).

*Rules:*
- Every job carries a generation and a cancellation token. A seek or an edit bumps the generation of
  classes 1–3.
- `StreamKernel::render` takes a stop check between history frames. This is the one plugin-API change
  (row 147).
- Producers ask the scheduler for room before ordering, so back-pressure comes from consumer
  capacity (M4).
- Every job is counted per class. "Zero work at rest" becomes "no runnable job ⇒ no thread awake", and
  the counters feed the journal (M6, M10).
- Classes 4–5 pause while playing or rendering, all of them (today: some).

*Determinism (invariant 11):*
- The scheduler changes timing only. Every product is a function of its plan and inputs, as today.
- Offline paths already hold their inputs and never depend on timing.
- A fallback result (an incomplete frame, a frame from a not-yet-specialized pipeline) is shown and
  never stored: the existing rule of the kept composites and the reel cache.

**What it buys, honestly:**
- The measured gains of past local fixes in this class are large:
  - decode-request-storm: 88 653 → 270 requests;
  - video-prefetch-thrash: 22 025 → 0 drops;
  - preview-throughput: V-fps 11.3 → 21.0 and underruns 193 987 → 0;
  - idle-cpu: 236 % → 0.73 %;
  - input-without-work: prefetch 69 % → 0.
- The scheduler would not repeat those gains. It makes them the default for the next producer.
- The open rows it addresses: 94, 98, 114, 122, 134, 140, 143, 144, 147, 148, 149, and the timers.

**No machine decides the design** (owner, H13: «ты весь софт под конкретную машину пытаешься
оптимизировать?»; rule in `docs/orchestrator.md`):
- Priorities are a fixed general order: the classes above.
- Budgets and limits are measured at run time on the machine that runs the program:
  - the length of one GPU submission, from the pass timer every device already has (t2020–t2022);
  - the cores free for the CPU pool;
  - the memory the ledger may use.
- Each rule is proved on our benches. Row 149 (one ring for the display and the offline work) is
  reproduced on llvmpipe by running the preview and the offline route on ONE device, and the bench
  reads the display's frame latency with and without the bounded submission.
- His journal confirms that a fix reached his machine. Nothing in this plan waits for it.

### Why not a new engine as a breaking change (the core question)

| What a rewrite from zero would touch | Needed for M1–M11? |
|---|---|
| `.kt` project format | No. The format is the model; the engine reads it. (The `.kt` → `.ktren` rename is the owner's separate decision, row 17.) |
| plugin API | Only two extensions: a stop check in `StreamKernel::render` (scheduler), and a point-wise kernel form if fusion is taken (§3). Both are additive. |
| `frame_plan`, the GPU executor, FX kernels, transitions, generators | No. They carry invariants 7 and 11, the stored frames (t181…), and the M7 fixes already made. |
| decoders, BurstCache, MediaWorker, export pipeline | Generalized in place by the scheduler, not replaced. |
| tests | ~1358 test ids are named in the capability specs. A rewrite re-earns every one of them; an in-place change keeps them as the referee. |

**What breaking would cost:** the defects behind the owner's complaint are the coordination layer
(M1, M3, M4, M5, the M9 budget). A rewrite replaces the computation layer too. That layer is not
where the defects are, and it carries most of the tests.

**What a rewrite would not fix by itself:** M2, M7 and M8 recur in any engine written by hand.
Only guards fix them: count-based cost tests, the scheduler's counters, a pipeline cache.

## 3. JIT of math and computation — every computation, not only shaders

The owner (H12): «про JIT я вообще-то имел ввиду не только шейдеры». The first version of this section
judged only shaders and today's audio cost. This one walks every computation the engine does and
weighs it against what the measured defects show costs on the CPU.

The reconnaissance of other programs is the note `docs/features/plugins/kernels/fused-kernels.md`
(F-EXT-005). The code facts below come from a read of `src/` at 8770e6b2.

### 3.1 Three kinds of "compiling", and the limits they must keep

**T1. Compile the DATA: build an evaluator once, use it per element.**
- What it does: a curve, a speed profile, a formula or a schedule is turned once per build into a
  structure answered in O(1) or O(log n), instead of being re-walked per sample, frame or pass.
- Its parts:
  - a cursor for in-order reads;
  - prefix sums for integrals (the warp map already has one: `Integral { segs, before }`,
    `kt-model/src/warp.rs:483-526`, used only by the video frame map and the UI);
  - formulas compiled into closures over numbered variable slots;
  - per-block constants hoisted out of the per-sample loop.
- Precedent: Halide's "schedule", Faust's per-diagram `compute()`, Core Image's recipe-then-program.

**T2. "JIT without a JIT": specialized Rust kernels chosen at run time.**
- What it does: the same Rust source compiled into several monomorphic variants. One variant is chosen
  per block, per frame or per build, by state (a parameter standing still, a weight of exactly 1, no
  curve) or by instruction set.
- We already do it in three places:
  - `cpu::fast!` is an AVX2 copy chosen at start (audio-fx-cost: VHS sound 40 → 22 reference
    kernels);
  - `dsp::Memo` skips work while a value stands still;
  - `geom_specialized` builds one pipeline per geometry shape.

**T3. Real run-time code generation** (Cranelift, LLVM, a Faust-like graph compiler).
- What it does: machine code emitted while the program runs.

**Limits any of the three keeps:**
- **Audio is byte for byte, always** (invariant 11). A specialized or generated path must perform the
  same IEEE operations in the same order as the general path.
  - Rust neither contracts `a·b+c` into FMA nor reassociates.
  - `kt_plugin_api::math` (pure-Rust libm) is the one source of transcendentals on every ISA.
  - So T1 and T2 can be made bit-identical and proved by the existing two-path test pattern
    (t1981: bytes on both paths).
  - T3 must emit exactly that sequence, and call the same math functions out of line. That is a
    second implementation held to the first by tests.
- **ARM:** aarch64's baseline already has NEON (128-bit), so rustc vectorizes our plain loops there
  with no second copy. `cpu::fast!` matters only on x86-64, where the shipped baseline is SSE2.
  Cranelift has an aarch64 back end. An LLVM JIT would add a large native dependency on every
  platform.
- **No invented language:** T1 compiles the formulas the program already has. T2 is Rust. T3 would
  generate from the same Rust or WGSL texts. Nothing new for a person to learn.

### 3.2 Area by area

What the code does is read at 8770e6b2. "Measured" cites a number. A **guess** is marked.

| Area | What costs (code read / measured) | Kind that fits | Expected gain | Risk | Size | Verdict |
|---|---|---|---|---|---|---|
| **Engine build: speed profiles, warp, links, morph pass** | **Measured:** `AudioRenderer::new` 393–488 ms on haski against 1 ms on the showcase (§0). Code: `sign_knots` O(K²) with allocations (row 151). `rate_profile_keys` builds a `BTreeMap` with three `String` keys and walks `curve_value` from the first key at every 5 ms grid point (`warp.rs:943-1000`). It is redone on every `clip_warp` (per piece in `clip_slice`, per trim helper) and in every one of the 4–5 morph passes per build (`project.rs:6440-6505`). Links: per frame-grid tick, a linear clip search plus a `BTreeMap` plus a tree eval, and a whole clip clone per pass (`links.rs:315-490`). | T1: the profile is sampled ONCE per curve revision and memoized by its content; warp questions go through the prebuilt prefix-sum map; `sign_knots` in one sweep; formulas compiled to closures; one morph pass per build | 450 ms → a few ms per build (**guess**, the showcase builds in 1 ms); it multiplies with M1's 2–3 builds per edit | low: the sums must keep their order (`source_progress` "repeats step by step… the same order of addition"); the prefix map stores exactly those partial sums | S–M | **take first**; `builder-cost-guard` (running) takes row 151 and 144; `compiled-curves` generalizes it |
| **Prefetch passes** | **Measured:** 160–280 ms a pass against a 100 ms target (row 144). Code: `wanted_audio_bursts_for` walks ALL clips each pass and, per FX clip, rebuilds `segments()` several times and clones `ParamValues` per slot, where the render side caches the same numbers in `src_ends` (`audio.rs:1813-1893`). `video_prefetch_wants` calls `layers_at` (all clips) per frame of a 2 s window. Datamosh `needs` builds a new `Sched` + `HashMap` + `Keys` per call, per frame of the window (`mosh.rs:140-417`). | T1: per-build tables (`src_ends` shared with prefetch; the bucket index instead of all clips; the stream layout compiled once per build) | 160–280 ms → ≤ 10 ms a pass (**guess**); fewer rows 134/143 side effects | low | S–M | **take** (with the scheduler's producer rule) |
| **Audio DSP per sample** | **Measured:** his chain 51.4 reference kernels after audio-fx-cost; compressor `log10`+`pow` per sample 4 kernels (row 129); phaser 6.2 (row 128, owner). Code: every LFO effect (tremolo, auto-pan, vibrato, chorus, flanger, phaser) computes its phase with `param_integral`, which walks the rate curve FROM THE FIRST POINT every sample (`kt-plugin-api/src/curves.rs:21-48`). A curve resolved on the frame grid makes that thousands of steps per sample, never measured. EQ and filter re-tune their biquads every 128 frames whether or not a value moved (`fx-audio-tone/src/lib.rs:224-250, 604-618`). Smoothers tick at rest. | T1: a prefix-integral cursor for the LFO; T2: "stands still" variants (the `Memo` pattern: skip a re-tune whose inputs are equal, a constant-parameter block path); x86 keeps the `fast!` copy, ARM gets NEON from the baseline | the LFO case is unbounded today and becomes O(1); the rest **guess** 51 → ~40 reference kernels on his chain (compressor below threshold, EQ at rest) | low: bit-identical by construction, two-path tests | S | **take** (`kernel-specialization`); row 128 stays the owner's |
| **Audio chain as one compiled graph (Faust/Cmajor, Cranelift/LLVM)** | Code: a `dyn` call per node per 1024-frame chunk (`audio/mod.rs:1319-1355`) ≈ 47 calls a second per node; kernels are already monomorphic inside. Source: Faust's `-vec` mode splits fused loops again because fusion stops vectorization; a Cranelift DSP log found dispatch "isn't the bottleneck" and scalar Cranelift slower than rustc. | T3 | nothing measurable from dispatch (**guess** < 1 %); worse vectorization without a SIMD back end | high: a second implementation of every kernel held byte for byte; W^X memory on macOS needs an entitlement | L–XL | **reject** — T1/T2 give the gains without a second implementation |
| **Mixdown weights** | Code: per sample `fade_weight` (branches + `sqrt`) and `overlap_at` with a binary search, times `gain`, also across stretches where the weight is exactly 1 (`audio.rs:176-192, 548-585, 1381-1396`). Not measured alone; the whole render is ≤ 1 % on our ranges, 2.5–4.4 % on his densest (§0, audio-fx-cost). | T2: split a block into constant-weight runs (a plain multiply or copy) and ramp runs | **guess** small; grows with overlaps | low (same values) | S | **take inside `kernel-specialization`**, after the above |
| **Automation per frame (video)** | Code: `AutoPlan::apply` reads each curve with the linear `curve_value` (`project.rs:4419-4436`). Resolved link and formula curves have a key per frame, so one frame costs O(frames of the clip) and a clip O(n²). The chain is cloned per animated layer per frame (`video.rs:1864-1871`), and again per stream position (`rows_at`). | T1: a compiled curve per build (binary search or a cursor); parameters patched into a reused chain | not measured; quadratic in clip length | low (same interpolation) | S | **take** (`compiled-curves`) |
| **Formulas** | Code: a boxed-tree interpreter, a `BTreeMap<String,f64>` per evaluation, a `Vec` per call node (`formula.rs:109-293`); `SegShape::Formula` also locks a global `Mutex`, clones the text and hashes it per evaluation (`project.rs:3910-3915, 3998-4008`). Reached at build (rate profiles, links: per grid tick) and in UI curve drawing. | T1: compile to closures over slot-indexed variables once per text; T3 (Cranelift) only if closures still show in a profile | **guess** 10–50× per evaluation (no allocation, no string compares) | low: same operations, same `math` functions | S | **take closures; T3 not needed** |
| **Frame plan and geometry (CPU side)** | **Measured:** plan 0.02 ms, encode 0.1–0.2 ms a composite (ui-frame-cost). Code: `build_plan` scans all clips per frame (`video.rs:2028-2120`); `plan_chain` allocates per layer per frame; the mask key and `plan_key` hash a `format!("{:?}")` per frame; plugin predicates search the registry by id string. | T1: a plan template per (layer, chain revision) compiled at build, patched per frame; the bucket index | small today; grows with the project | low | S | **take later**, with `compiled-curves` |
| **Datamosh / stream stages, CPU side** | **Measured:** part of row 144; row 135: residuals and the rebuilt frame read back as 32 bits a sample (~25 MB a 1080p frame). Code: `Sched`/`Keys` rebuilt per call; `rows_at` clones the chain + `AutoPlan::apply` per position; scalar BT.601 conversion on the CPU (`codec.rs:77-125`); the audio half builds a `Sched` + `HashMap` per 1024-frame codec frame (`sound.rs:187-232`). | T1: the stream layout compiled once per build; packing; move the colour conversion into the compute pass (the scope: math on WGSL) | **measured** −66 % readback, the stage's thread −60…−75 % | medium: t1239's pinned bytes | M | **done** (`stream-layout-per-build`) |
| **Analysis: LUFS, true peak, peaks, sync** | Code: `LoudnessMeter::feed` scalar f64 biquads per sample; true peak does `tail.rotate_left(1)` every sample and a `OnceLock::get` per tap (`loudness.rs:163-314`); `PeakAcc` scalar. It runs in every export and on every first opening of a file. Not measured. | T2: `fast!` copy on x86 (NEON is baseline on ARM); a ring index instead of the rotate; the kernel hoisted | **guess** 2–4× on the meter | low: f64, same order; loudness rounding (row 6) unaffected | S | **take** (`kernel-specialization`) |
| **Export, CPU side** | **Measured:** RGBA conversion 64 s against 17 s decoding before four converter threads (render-speed-no-card D2). Code: `sws_to_rgba` allocates a zeroed frame each time (`source_impl.rs:1301-1320`); the audio chunk is copied per burst. FFmpeg's `swscale` already picks hand SIMD at run time. | not JIT: buffer pools; frame blending (row 86, scalar `dissolve_into`) moves to a WGSL pass by the acceleration scope | small per frame | low | S | **take as housekeeping**, not as JIT |
| **Shaders: pipeline prewarm + disk cache** | **Measured:** row 64: first Datamosh frame 6.6 s under load (62–209 ms open + 247–690 ms intra with an empty Mesa cache); row 139: a geometry shape compiled inside a frame; llvmpipe `glow:resample` 2 s at its first draw. Each new device recompiles everything (`cache: None`). | prewarm at project open; wgpu `PipelineCache` on Vulkan | the first-use hitch goes on every machine | none (bytes unchanged) | S | **take** (`pipeline-prewarm`) |
| **Shaders: fused point-wise runs** | **Measured** (llvmpipe, gpu-pass-cost D2): point-wise effects 2–3 ms each at 1080p, composite 1.1 ms a layer. A device-independent count: every pass reads and writes a full frame. | generated plain WGSL per run, Core Image's colour/warp/general classes; a run cut where a neighbourhood kernel stands | k − 1 frame round trips and dispatches fewer per run, on every device; on our bench ~(k−1)×2 ms (**guess** until the bench runs) | the plugin API grows a point-wise form; references move ≤ 1 LSB per former boundary (**owner agreed**, H16); offline paths never take an unfused fallback | L | **take** — a general rule, proved on our bench by bytes moved per frame and time per frame; the run-length cap is measured at run time (a fused pipeline slower than its parts on this device is split) |
| **Shaders: specialization by constants** | Code: wgpu substitutes WGSL `override` in naga before the driver compile, so each value is a full compile. Precedent: gpu-pass-cost's hand hoisting, Camera ISP −25 %. | discrete values only (mode, tap count, radius class) | per heavy effect | compile per value if misused | S each | **take by hand, per effect** (row 137) |

### 3.3 Verdict

- **JIT is taken, mostly as T1 and T2.**
- **T1 (compile the data) carries the largest measured CPU costs:** the build (~450 ms an edit,
  row 151), the prefetch pass (160–280 ms, row 144), per-sample LFO walks, per-frame automation,
  formulas, the Datamosh schedule.
- **T2 (specialized Rust chosen at run time)** extends what audio-fx-cost proved: the AVX2 copy and
  the "stands still" memo, now for the remaining kernels and the analysis.
- **T3, real code generation, is rejected for now.** Nothing measured on the CPU is bound by
  dispatch or by interpretation that closures cannot remove. Generated machine code would be a second
  implementation of byte-exact audio. If a profile after T1 still shows formula evaluation, Cranelift
  for formulas is the one candidate; it has an aarch64 back end.
- **On the GPU:** prewarm and the pipeline cache now; fused point-wise runs as generated WGSL, decided
  by a general rule and proved on our bench; specialization by hand.
- **Nothing here waits for any one machine's numbers.**

## 4. Recommendation

The changes are named by mechanism. Sizes use the `docs/features` scale: S = one stage, M = 2–3 stages,
L = a round. Test numbers are allocated by each change; t2000–t2019 are reserved for
`edit-without-rebuild`.

Every limit below is a general rule or is measured at run time on the machine that runs the program,
and is proved on our benches (llvmpipe, the showcase, the relinked copy). No step waits for a
journal from one machine.

| # | Change | Mechanism | What it fixes | Size | Risk | Tests that hold it |
|---|---|---|---|---|---|---|
| 1 | `builder-cost-guard` (**running**, owner H14) | M2, the cheap half of M1 | row 151 (linear `sign_knots`), row 144 (prefetch `needs` per frame), the LFO key walk; a count-based guard on builders and per-frame paths (row 107) | S | low: bit-identical knots (t17, t292, the warp tests) | new count tests; probe build cost on haski before/after |
| 2 | `compiled-curves` (JIT T1) | M2, M7 | rate profiles sampled once per curve revision and memoized by content; every warp question through the prefix-sum map; formulas compiled to closures (`formula.rs`, `SegShape::Formula`); `resolve_one`/`resolve_derived` and the morph pass once per build; `AutoPlan::apply` on compiled curves, no per-frame chain clone; LFO phase by a prefix-integral cursor | M | low: the same sums in the same order, two-path byte tests | t17, t292, t645, the automation and warp tests; new "same bytes, compiled and interpreted" tests |
| 3 | `edit-without-rebuild` | M1 | rows 110, 111, 117, 150: one build per project version shared by render, prefetch and probe, off the UI thread; an `Arc` snapshot instead of a UI-thread clone; per-event derived state memoized by `fp` | M | medium: FX state continuity (invariant 3), the scene hash | t2000–t2019; t1875, t1935, t1930–t1937 |
| 4 | `one-scheduler` (stage 1) — **done** 2026-11-09 | M3, M4, M6, M10 | job classes, counters, cancellation tokens; the background services on one executor with event wakes, all paused while playing (**owner agreed**, H15); rows 114, 122, 140, the seven timers; the UI-thread waits (rows 98, 148, the first probe) become jobs; a counter of blocking calls on the UI and render threads | L | medium; live scenarios hold it | live `idle`, `ui_wait`, `cached_range`, `journal`; t1830–t1836, t1840–t1848, t2040–t2045 |
| 5 | `gpu-arbiter` (scheduler stage 2) — **done** 2026-11-09 | M5, M4 | one ring per card (per device on a software rasterizer), admission by class; on a ring with a display a lower-class frame or compute job is cut into pieces by the pass timer's measured times, bound half a frame period (row 149: on ONE llvmpipe device, display wait p50/p95/max 32/322/764 → 27/50/73 ms, offline rate kept); compute jobs tagged by class (row 134: 12–13 jobs/s, all the look-ahead's, ordered behind the frame on screen; the count is `stream-layout-per-build`'s); `StreamInput::stop` (row 147); per-render decode settings (row 94); the present and a paused export wait without timers (rows 272, 271) | M | medium | t2020–t2025, t1239; new bench: display latency under a Datamosh history, shared device |
| 6 | `pipeline-prewarm` | M8 | rows 64 and 139: prewarm at project open; wgpu `PipelineCache` on Vulkan | S | low | a count of in-frame pipeline creations (t2023's style) |
| 7 | `kernel-specialization` (JIT T2) — **done** 2026-11-09 | M7 | "stands still" variants (biquad re-tune only on change, compressor below threshold, smoothers at rest, constant-weight mixdown runs); `fast!` copies for the loudness meter, peaks and mixdown; true peak without a per-sample rotate; rows 129 (128 stays the owner's). Measured: filter 0.52 → 0.34, EQ heavy 0.90 → 0.55, compressor on quiet material 1.46 → 0.58 ref; meter 50 → 25 ms, peaks 7.2 → 4.1 ms per minute; his chain (reverb, VHS, Datamosh sound) unchanged — nothing there stands still, the "51 → ~40" guess does not hold | S–M | low: two-path byte tests (t1981's pattern) | t1980–t1982, t2170–t2176, loudness tests |
| 8 | `stream-layout-per-build` — **done** 2026-11-09 | M2, M7 | the Datamosh schedule compiled once per build (video and sound halves); packed readbacks (row 135); colour conversion inside the compute pass; the memo against loop length (row 143); an advance converts no picture. Measured (release, his relinked region, 720 lines, `kt-cli stream-cost`, same load): the stage's thread 10.4–17.2 → 4.3–4.4 ms a frame, readback 18.3 → 6.2 MB a frame, the prefetch's 2 s window question 12–20 → 2.2–3.6 ms; a loop of the region (667 positions) held in a 1.6 GB memo, laps 2 and 3 run no job; t1239's bytes unchanged | M | medium: t1239's pinned bytes | t1239, t2040–t2045, t2270–t2278 |
| 9 | `memory-ledger` (scheduler stage 3) — **done** 2026-11-09 | M9 | one ledger (`kt_time::mem`) across BurstCache, PresentMemo, StreamMemo, MixStore and the pools; growth above the bases from half of (available + held), cgroup-aware, in the order kept composites → history → frames; the present memo sized by the loop (row 310), a loop it cannot hold keeps a part, a card's refusal bounds it; an export claims the growth; export devices keep only the uploads in flight; row 119 held. Measured (release, his relinked region, 640×360, 240 s, same load): laps 14–21 composed ~52 frames each → laps 10–21 compose none; composed 1935 → 1172, kept shown 5407 → 8943, V-fps 28.9 → 40.4, 11.8 → 6.5 cores | M | medium: eviction order is correctness for export (holds stay) | t1600–t1605, t1934, t1570, t2290–t2301 |
| 10 | `fused-pointwise-runs` (JIT, shaders) — **done** 2026-11-09 | M7 | point-wise runs as one generated WGSL pass per run shape, prewarmed; each stage keeps its own pass's rounding, so fused = unfused byte for byte (stricter than the owner's ≤ 1 LSB per boundary, H16) and the run-time trial (pass timer, interleaved, minima) may split a slower fused pass in halves without moving a frame. Measured on llvmpipe at 1080p: the chain's bytes k × 16.6 → 16.6 MB, device 3.1/4.7/7.2/7.5/9.3 → 1.8/1.8/2.3/2.1/2.2 ms for 2–6 stages; the heavy showcase holds one two-stage run in ~1070 ms frames — no visible change there | L | medium: plugin API | t2190–t2197 |
| 11 | `local-cost-queue` — **done** 2026-11-10 | M7, M8, M9 | Camera ISP's sensor passes at the sensor's size behind K× (row 136); uniforms staged per submission and bind groups kept a frame (138); audio: VHS sound's FIR cosine table and stepped dropout cache (311), the camera mic's window/µ-law tables and kept buffers (211, 312), WSOLA's window table (313), the codec without per-frame scratch (314). Measured (release, llvmpipe, minima, same bytes): camera at K = 2 388.8 → 120.7 ms; the composing thread 1.62 → 1.29 ms a 1080p frame; VHS sound with a curved head 42.9 → 36.1, camera mic 29 → 25, WSOLA 19.9 → 19.0 reference kernels. Closed by measurement, not changed: 137 (the trig is 4 of 26 ms; a table costs more), 210 (a memo is 3 % slower). 118: the heavy showcase's first lap is decode-bound, nothing local left; 119 held by memory-ledger; 128 the owner's | S each | low | t1217, t2023, t2310–t2317 |
| 12 | `test-hygiene` — **done** 2026-11-09 | M11 | rows 47, 99, 101, 126, 133, 158, 163, 165, 166, 170: the bench keeps a red try (`first-try/`) and starts clean; keys and clicks read by a slow frame keep their meaning (a frame's commands were a set; Space and egui's double click judged by frame times); a start's audio pass reaches the ring whole (t1935's underruns were the product's, its trace showed the device on time); t1671 sees processor clocks; scenarios read consequences. rows 133, 170: the timeline polled for the recheck of kept moments every frame (product, t2217). Row 47 not reproduced: its red now names its cause | S | low | t1671, t2210–t2217 |

**Order and why:**
- 1 is running. 2 extends it from the sites found to the whole class.
- 3 then makes one build per edit.
- 4 and 5 are the scheduler, which schedules those builds. 5 no longer waits for anything: it is
  proved on the shared-device bench.
- 6 and 7 are independent and small; they can run beside 2–5 on other files.
- 8 follows 2 (it uses the compiled curves).
- 9 follows 4.
- 10 last of the large ones, because its plugin-API change should meet a settled scheduler.

**Explicit answer:** no new engine as a breaking change.
- The data puts every architectural defect in the coordination layer: rebuild per edit, waits on the
  UI/render threads, producers without back-pressure, resources without an arbiter, budgets without a
  ledger.
- The CPU "JIT" opportunities are compiled data and specialized kernels inside the existing engine.
- All of it is replaced in place, under the ~1358 spec-named tests, with no `.kt` change.
- The plugin API grows twice, both additive: a stream stop check, and a point-wise kernel form.
- What a rewrite would break:
  - every engine-internal test would have to be re-earned;
  - every M7 fix would have to be redone;
  - the stored-frame references would move;
  - the plugins would be rebuilt;
  - M2/M7/M8 would recur in the new code as in the old.

## Appendix — performance rows of `docs/found-defects.md` by mechanism

| Mechanism | Rows (open in bold) |
|---|---|
| M1 rebuild per edit | **110**, **111**, **117**, **150** |
| M2 O(whole) inside a loop | **151**, **144**, **107**, 127 (closed by t1939) |
| M3 a thread waits | **98**, **105**, **112**, **147**, **148**, **99**, **100**, **47** (Ctrl+S under load, cause unknown), 121 (closed), **54** (one-off composite stalls, watch) |
| M4 ordered faster than consumed | **114**, **134**, **143**, **16** (loop freeze on wine, unverified) |
| M5 no arbiter | **149**, **94**, **72** (render time on a busy machine), **52** (llvmpipe `queue.submit` 30 ms in a window) |
| M6 work at rest | **122**, **140** |
| M7 per-element constants | **128** (owner), **129**, **96** (VHS s5 22 ms, alpha composite 9 ms), **145** (Datamosh encoder on llvmpipe), **135** (32-bit readback), 37, 42, 43, 136, 137, 210, 211, 311–314 (closed) |
| M8 allocation / compile in the frame | **64**, **139**, **96** (two upload copies), 138 (closed) |
| M9 cache key / budget | **118**, **119**, **10**, **85**, **143**, **12** (gallery near a cut) |
| M10 not measured | **108**, **53**, **55** (RGBA one thread per file), **56** (cold disk), **86** (blending on the CPU, not measured), **93** (preview on VHS unsteady, cause unknown), **68** (hardware encode unproven) |
| M11 wall-clock tests | **126**, **101**, **106**, **141**, **146**, **133**, **89**, 130 (closed), **70**, **102**, **31**, **32** (build/CI time) |
| Owner question, not a defect | **142** (VHS 1135×576 grid in the preview) |

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
- §3 JIT of math and computation (the note is `docs/features/plugins/kernels/fused-kernels.md`)
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
| row 144: prefetch calls the stage's `needs` per frame of its 2 s window | 160–280 ms a pass against a 100 ms target | **open** |
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
| row 147: a composite the UI has left runs to its end (no cancellation point in `StreamKernel::render`) | 4 cores for seconds after a pause | **open**, needs a plugin-API change |
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
| row 134: ~45 GPU submits/s from Datamosh jobs prefetch runs far ahead | 35–49 submits/s | **open**, re-read in his next journal |
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
| compute host: jobs from prefetch, compose, reel/poster pre-roll and export on one queue, no priority, no lock across a job (code map §4) | each poll waits for the other's work | **open** |
| offline device shared by the slow preview route, fx-thumbs, reel strips and posters under one `inner` mutex (code map §4, inference) | not measured | **open** |
| row 149: on his RADV the display and offline devices share one graphics ring; one long submission holds the display | — | **open** |
| row 94: render jobs set the process-global `DECODE_THREADS` / `RGBA_THREADS`; a preview source opened meanwhile takes them | — | **open** |
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
| row 137: radial blur spin computes 16 sin/cos per pixel | (22.8 ms the pass) | **open** |
| row 136: Camera ISP behind a K× barrier computes every sensor pixel k² times | — | **open** |
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
| row 138: a bind group and a uniform write per pass on the composing thread | ≈ µs a pass (CPU) | **open** |
| row 139: a new geometry shape compiles its pipeline inside the frame | a hitch; llvmpipe `glow:resample` 2 s at its first draw | **open** |
| row 64: the first Datamosh frame — device open + shader compile, no disk cache | 6.6 s under load; 62–209 ms open + 247–690 ms intra with an empty Mesa cache | **open** |
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
| row 119: a frame evicted and decoded again misses its kept composite | — | **open** |
| one budget per cache, no ledger across them: live BurstCache 512 + 64 MB, PresentMemo 512 MB per engine, Upload 256, Readback 64, transient pool 128, StreamMemo 256, MixStore 64, compute pool 1 GiB, reel 64, poster 16 (code map §7) | rows 10 (export 7.3 GB peak), 85 (4K unproven), 96 (8.5 against 7.3 GB) | **open** |

Each key defect was LOCAL and is closed. What remains structural is the **budget**. Ten caches each
pick their own size, and nobody sees the sum or evicts by the value of a byte across them.

### M10. Work nobody measured — a missing or wrong counter — LOCAL per site, now mostly instrumented

| Defect | State |
|---|---|
| "thrash" counted every unread drop; an empty memo lookup counted as a miss (41 % / 57 %); underruns in two units (24 000 vs 24); empty passes before the first frame | closed |
| rebuilds had no counter, so the load watch called 196 % "at rest" (input-without-work) | closed: `engine_builds`, `model_edits`, `project_swaps` (t1880) |
| no compose-time counter (morpher grid) | closed: `FrameCost` |
| "work the device does that nobody measured" (gpu-pass-cost) | closed: pass timer, `GPU` journal record (t2020–t2022) |
| notes stamped late or in UTC (telemetry-journal) | closed: one journal (t1960–t1969) |
| row 53: dropped-frame counter on the CPU path | open (the CPU path is deleted; to recheck) |
| row 108: no load number proved on a real card | **open: the decisive unknown of this audit** |

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
  Their cost on his card is unknown until his next journal with the `GPU` record (row 108).
- **Guess:** on his journal's symptoms (45 submits/s, V-fps 9, UI blocks now gone), the GPU arbitration
  and the stream pacing matter more than the CPU pool.

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

## 3. JIT of math and computation

The reconnaissance note is `docs/features/plugins/kernels/fused-kernels.md` (F-EXT-005).

The owner's fixed scope (UF:2547-2577, 2026-11-04):
- all math, on plain WGSL compute through wgpu/Vulkan;
- the byte reference dropped;
- ARM needed;
- no invented language;
- audio stays on the CPU (registry C2).

| Kind of JIT | Who does it | Fits the scope? | Gain on OUR measured costs | What it breaks | Verdict |
|---|---|---|---|---|---|
| (c) pipeline prewarm + disk pipeline cache | UE5 PSO precaching, Dolphin, Metal/driver caches; wgpu `PipelineCache` (Vulkan only) | yes | Removes first-use hitches. Row 64: first Datamosh frame 6.6 s under load; 62–209 ms open + 247–690 ms intra with an empty Mesa cache. Row 139: geometry shape compiled in the frame. llvmpipe `glow:resample` 2 s at first draw. Every new device (offline, export) recompiles everything | nothing (keyed by device + driver + text hash; bytes unchanged) | **take**: small |
| (a) fuse a run of point-wise effects into one generated WGSL pass (colour/warp kernels; neighbourhood kernels cut the run) | Core Image (kernel concatenation, FCP); Photoroom PhotoGraph; Halide (with tiling); our own `geom_fused` already absorbs point-wise taps | yes: plain WGSL, generated text, no new language | llvmpipe: the point-wise effects cost 2–3 ms each at 1080p and the composite 1.1 ms a layer (gpu-pass-cost D2), so a run of k saves ~(k−1)×2 ms (**guess**). On a card these passes are bandwidth-bound (~0.13 GB a 1080p RGBA8 round trip, **guess** ~0.1–0.3 ms each): the gain grows on ARM/iGPU bandwidth. The two heaviest costs are not point-wise and are NOT helped: Camera ISP 92–97 ms (12 passes), VHS 65–72 ms (14 passes) | plugin API (a point-wise effect gives a pure WGSL function besides or instead of its pass); stored frames: an 8-bit texture between effects disappears, so outputs move by ≤1 LSB per boundary (FrameTol, t181 references; the owner allowed updating references, UF:2553/2567); invariant 7 holds (the generator is part of the one executor); invariant 11 holds only if offline paths always wait for the fused pipeline and the unfused fallback is never stored | **take later, measured**: after his card's `GPU` record shows the point-wise share |
| (b) specialization by constants | Metal function constants, Vulkan spec constants, WGSL `override` | yes, narrowly | In wgpu an `override` is a full naga + driver recompile per value, so only discrete values (mode, tap count, radius class, booleans) qualify. gpu-pass-cost already did the hand version: ISP −25 % by hoisting per-pass constants; radial-blur spin's per-frame sin/cos (row 137) is the next | nothing if per discrete value; a compile per frame if misused on a continuous value | **take per heavy effect, by hand or by `override`**: no framework |
| (d) JIT the audio chain (Cranelift/LLVM, Faust/Cmajor style) | Faust, Cmajor/SOUL, Max gen~, synfx-dsp-jit | no: audio stays on the CPU by decision; LLVM is a heavy dependency | Audio render is 0.1 % of real time on the measured ranges (§0) and 3–4.4 % on his densest chain (audio-fx-cost: 881 → 597 ms for 20 s). A 1024-frame block costs ~47 dispatches per node per second: nothing to recover. Cranelift does not auto-vectorize; rustc + `cpu::fast!` already does (VHS sound 40 → 22) | invariant 11 (audio byte for byte; a second implementation must equal the first, `kt_plugin_api::math`) | **reject**: precompute per block in Rust (rows 128, 129) |
| (e) CPU SIMD JIT for video | BlinkScript CPU path, ISPC | no: the CPU video path was deleted by the owner's decision | none | — | **reject** |

**JIT verdict.** Take the pipeline cache and prewarm now (S). Take fused point-wise runs later, as
generated plain WGSL, after his card's numbers (L). Keep specialization manual and discrete. No audio
JIT and no CPU JIT. None of it is a new engine: (a) is a code generator inside the existing executor,
fed by `plan_chain`, which already segments chains into fusable runs and barriers.

## 4. Recommendation

The changes are named by mechanism. Sizes use the `docs/features` scale: S = one stage, M = 2–3 stages,
L = a round. Test numbers are allocated by the change; t2000–t2019 are reserved for
`edit-without-rebuild`.

| # | Change | Mechanism | What it fixes | Size | Risk | Tests that hold it |
|---|---|---|---|---|---|---|
| 1 | `builder-cost-guard` | M2 (and the cheap half of M1) | row 151 (linear `sign_knots`), row 144 (prefetch `needs` per frame), the LFO key walk per sample; a count-based guard on every builder and per-frame path (calls of `segments` per build; passes per hover frame for row 107) | S | low: same numbers, bit-identical knots (t17, t292 and the warp tests hold) | new count tests; probe build cost on haski before/after |
| 2 | `edit-without-rebuild` | M1 | rows 110, 111, 117, 150: one build per project version shared by render, prefetch and probe, built off the UI thread; an `Arc` snapshot instead of a UI-thread clone; per-event derived state memoized by `fp` | M | medium: FX state continuity (invariant 3) and the scene hash must not move | t2000–t2019; t1875, t1935, t1930–t1937, invariant 3 tests |
| 3 | `one-scheduler` (stage 1) | M3, M4, M6, M10 | job classes, counters, cancellation tokens; the background services (thumbs, fx-thumbs, posters, reel preview, analysis, index, watchdog) moved onto one background executor with event wakes and uniform pause-on-play; rows 114, 122, 140, the seven timers; the remaining UI-thread waits (rows 98, 148, first probe) become jobs; a counter of blocking calls on the UI and render threads | L | medium: many services; behaviour must stay; live scenarios hold it | live `idle`, `ui_wait`, `cached_range`, `journal`; t1830–t1836, t1840–t1848, t2040–t2045 |
| 4 | `gpu-arbiter` (scheduler stage 2) | M5, M4 | one submission queue per device with class priority and bounded submissions (row 149); compute-host jobs tagged by class (row 134); `StreamKernel::render` stop check (row 147, plugin API); per-render decode settings instead of globals (row 94) | M | medium; needs his `GPU` journal first (row 108) | t2020–t2025, t1239, new cancellation test |
| 5 | `memory-ledger` (scheduler stage 3) | M9 | one budget across BurstCache, PresentMemo, StreamMemo, MixStore and the pools; rows 10, 85, 96, 143 | M | medium: eviction order is correctness for export (holds stay) | t1600–t1605, t1934, t1570 |
| 6 | `pipeline-prewarm` | M8, JIT (c) | rows 64 and 139: prewarm the known pipelines at project open, wgpu `PipelineCache` on Vulkan | S | low | a count of in-frame pipeline creations (t2023's style) |
| 7 | `fused-pointwise-runs` | M7, JIT (a) | point-wise runs as one generated WGSL pass | L | high: plugin API, references | t181 tolerances, t183, new fused-equals-unfused test |
| 8 | local cost queue | M7, M9 | rows 129, 135, 136, 137, 138, 118, 119; row 128 only on the owner's word | S each | low | t1980–t1982, t1217, t2023 |
| 9 | test hygiene | M11 | row 126 under t1671; rows 101, 133 | S | low | t1671 |

**Order and why:**
- 1 first: smallest, and it removes most of the measured edit cost.
- 2 next: the architectural half of M1. It must land before the scheduler, because the scheduler
  schedules builds.
- 3 to 5 are the scheduler in three stages, each shippable. 4 waits for his card's journal.
- 6 is independent and cheap; it can run beside 1.
- 7 only on numbers from a real card.

**Explicit answer:** no new engine as a breaking change.
- The data puts every architectural defect in the coordination layer: rebuild per edit, waits on the
  UI/render threads, producers without back-pressure, resources without an arbiter, budgets without a
  ledger. It puts none in the computation layer.
- The coordination layer can be replaced in place, piece by piece, under the ~1358 spec-named tests,
  without touching the `.kt` format. The plugin API gets one additive change (the stream stop check),
  plus a second one only if fusion is taken.
- What would break if we rewrote anyway:
  - every engine-internal test would have to be re-earned;
  - every M7 fix would have to be redone;
  - the stored-frame references would move;
  - the plugins would be rebuilt against a new contract;
  - meanwhile M2/M7/M8 would recur in the new code exactly as in the old.

## Appendix — performance rows of `docs/found-defects.md` by mechanism

| Mechanism | Rows (open in bold) |
|---|---|
| M1 rebuild per edit | **110**, **111**, **117**, **150** |
| M2 O(whole) inside a loop | **151**, **144**, **107**, 127 (closed by t1939) |
| M3 a thread waits | **98**, **105**, **112**, **147**, **148**, **99**, **100**, **47** (Ctrl+S under load, cause unknown), 121 (closed), **54** (one-off composite stalls, watch) |
| M4 ordered faster than consumed | **114**, **134**, **143**, **16** (loop freeze on wine, unverified) |
| M5 no arbiter | **149**, **94**, **72** (render time on a busy machine), **52** (llvmpipe `queue.submit` 30 ms in a window) |
| M6 work at rest | **122**, **140** |
| M7 per-element constants | **128** (owner), **129**, **136**, **137**, **96** (VHS s5 22 ms, alpha composite 9 ms), **145** (Datamosh encoder on llvmpipe), **135** (32-bit readback), 37, 42, 43 (closed) |
| M8 allocation / compile in the frame | **64**, **138**, **139**, **96** (two upload copies) |
| M9 cache key / budget | **118**, **119**, **10**, **85**, **143**, **12** (gallery near a cut) |
| M10 not measured | **108**, **53**, **55** (RGBA one thread per file), **56** (cold disk), **86** (blending on the CPU, not measured), **93** (preview on VHS unsteady, cause unknown), **68** (hardware encode unproven) |
| M11 wall-clock tests | **126**, **101**, **106**, **141**, **146**, **133**, **89**, 130 (closed), **70**, **102**, **31**, **32** (build/CI time) |
| Owner question, not a defect | **142** (VHS 1135×576 grid in the preview) |

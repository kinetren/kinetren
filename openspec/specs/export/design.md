# Export and headless rendering — why it is this way

## What hurt and for whom

The old engine kept two composition codes: one for the preview and another for export, with its own pool and a different Quality — the file contained something other than what the editor showed (defect n). Export wrote raw rgb24+s16le into temporary files (tmpfs → gigabytes in RAM) and then muxed them with an external `ffmpeg` (defect o). The owner hurt concretely over the course of the rounds: "the render now shows a black picture" (a chunk did not fit into the cache), "the render is broken" (the bottom track was lost on a multi-layer edit with FX), "missing sources must not be a blocker", a file name without the project and region, and above all the complaint "here the sound is from another event", the analysis of which took hours, because there was no way to ask the engine "what are you reading at this point".

## Decisions and why

- **Export = an offline run of the preview.** `render_raw_audio`/`render_raw_video` call the same `AudioRenderer`/`VideoRenderer` as the player; the difference is `DirectFetcher` ("clock by readiness") and the project's Quality. That is why "preview == export" is proven rather than promised: t11/t67/t479/t485/t184 compare bytes.
- **The muxer is in-process; the encoder is chosen by a probe of opening.** Candidates are tried until the first `avcodec_open2 >= 0`; the list in the program is CANDIDATES, not a promise (the set of encoders is a property of the machine's system FFmpeg). This is better than the "trial encoding" of other editors: there a trial is run by someone else's binary from a package, and its verdict lies; here the same process that writes the file does the probing, and there is no cacheable verdict at all.
- **Golden — a hash of the streams BEFORE the muxer.** The bytes of an .mp4 will not be identical for two users (the encoder is a property of the machine), so the reference guards the raw streams; t25 forces `libx264` only for the roundtrip and does not touch the encoder reference. The title is a separate `GOLDEN_TITLE_VIDEO`, so that a change of font does not touch the shared reference.
- **One backend per file.** A GPU error in the middle of an export is an export error: the GPU↔CPU tolerances (`FrameTol`) legitimately put a difference at the seam, and mixing backends would produce a visible seam. The `GpuRequired` refusal was removed when the CPU path became complete (Pan3d via the inverse homography): without an adapter the export is slower but possible (t301).
- **The chunk is bounded by memory, layers and decode quality.** A batch ≤ half of the budget, divided by `max_concurrent_layers`; the frame size — by `max_decode_quality` (decode scale step S gives S² memory). Otherwise LRU evicts the pre-frames before the composition, the starvation branch fires, and the export bytes depend on cache timing — a violation of determinism, not only of quality.
- **Post-FX in export is not memoized.** In the preview, memoization saves the loop; in export each t is composed exactly once, and full-size post-FX frames only evict the pre-frames of neighbouring tracks (t66c).
- **Loudness — on a pass that is needed anyway.** The meter is fed by the ready chunk buffer; the master gain is not measured, because it is not in the project — it is the monitoring loudness. There is no "normalize" button: the target (−23/−14/−27) is chosen by a person, and applying it after the fact would mean re-encoding the file.
- **Placeholders instead of refusal in the GUI, refusal instead of silence in kt-cli.** The owner's decision for the GUI is "notify and render", with a banner showing the file name (you see at once WHAT is missing). kt-cli has no dialog, so there it is a refusal and an explicit `--allow-missing`.
- **`probe` is computed by the model; `probe --time` goes through `BurstCache`.** Diagnostics must repeat the engine's wiring: the cost of a place on the loop is constant (the cache holds the source; the reverb is a streaming automaton), and it must be measured in REPEAT mode, since a linear measurement of the dependence on the region's length does not see it.
- **The cross-render is an external oracle of the edit.** melt (the engine of Kdenlive/Shotcut, headless); the samples are committed copies with a pinned SHA ("from the internet" is about provenance, not delivery). The comparison is colour + duration; audio is deferred (equal-power versus linear mix inevitably diverge). A real film was added at the owner's suggestion and immediately found a hole with NTSC frequencies.

## Rejected and why

- **Joining chunks after a parallel render (F-COL-040) — rejected.** All the industrial assemblers (`ffmpeg -f concat`, `mkvmerge`, a job-manager assembler) are external processes, excluded by invariant 9 by contract, not by cost; an in-process assembly requires a path "package from file → into the muxer without decoding", which does not exist. The second reason: determinism is local — chunks from different adapters differ legitimately, and the difference would land at the seam. The third: "a chunk can be computed more cheaply" is a second composition path. Not a reason: the absence of a network, the complexity of concat, the amount of code. The equivalent: N regions → N independent files. Reopen only after smart render (F-EXP-003).
- **Cloud render (F-COL-041) — rejected.** The document is kilobytes, the material is gigabytes, and our export reads it three times (a packet scan on opening, the video pass, the audio pass over the second set of sources); sending the job to where there is no material means moving the material. A second executor outside the process is a second product. Reopen as a question about proxies and the material address by content, not about rendering.
- **The region in kt-cli — "we do not do it"** (a direct decision of the owner): the divergence from the GUI is deliberate.
- **"Let's speed up the preview, it's only for display"** — forbidden by the verdict F-RES under invariant 7: the cost "the preview is not faster than export on the same frame" is named in advance; the only permitted way to speed up is to reduce Quality.
- **Streaming to the pipes of an external `ffmpeg` (stdin + mkfifo)** — an interim decision, replaced by the in-process muxer: without an external process the system FFmpeg's hardware encoders became available, and invariant 9 was strengthened to "not only without temp files, but without processes".
- ~~**Explicit SIMD in export** — no: the CPU path is the byte reference, and auto-vectorization would change the f64 sums and the determinism between machines; the speed-up is the GPU.~~ **Revisited (export-throughput D10):** the premise is gone — the AVX2 twins (R.32 and the barriers' `simd.rs`) are byte-exact by rule and run in the export already. What the export lacked was cores, not lanes. The rule that stands: SIMD in any kernel only as a byte-exact twin of its scalar text.

## Non-obvious details

- `export_chunk_frames` (`src/libs/kt-engine/src/export.rs`) clamps the batch to 1…120 and divides half of the budget by `layers`; `max_concurrent_layers` in `src/libs/kt-engine/src/video.rs` counts a blending layer as TWO frames, and the clip's blending flag must travel in four places (`clip_frame_keys`, `max_concurrent_layers_in`, `reel_coverage`, `nest_memo_key`) — otherwise the budget and the order would diverge (t583).
- The "black render" regression shows only at a large Quality: at 320×180 a chunk of 120 frames fits into the cache. An export test checked on a small frame guards nothing — t116 (`src/libs/kt-engine/tests/frame_order.rs`) sets a large Quality deliberately.
- Export reads frames as a span (`read_video_span`), not frame by frame (`src/libs/kt-engine/src/export.rs`, `render_raw_video_range_with`): frame-by-frame reading, when the project fps > the source fps, went into a seek plus decoder re-creation on every second frame and diverged from the preview's sampling semantics — judder in the finished file.
- Natives and procedurality of sources are taken by export from the OPEN sources (`SourceFacts`, `VideoRenderer::new_with_facts` in `src/libs/kt-engine/src/video.rs`): `VideoRenderer::new` does not know them (step ≡ 1). An export that builds a renderer without facts would diverge from the preview in the decode scale step — the order and the cache keys would become different.
- The generator sources are opened BY the export itself via `gen_sources(project)` (`src/libs/kt-engine/src/export.rs`): their address is derived from the event, and the caller has nowhere to get them; there must be no second enumeration.
- The muxer is written by ONE writer (`src/libs/kt-engine/src/export_pipeline.rs`, `write_all`): video frame `i`, then the audio up to that frame's end (export-throughput D2; until 2026-11-03 the whole audio pass followed the video pass); the interleaving of packets is done by `av_interleaved_write_frame`; `audio_sources`/`video_sources` are TWO independent sets, both opened by the caller (`open_all()` twice in `src/apps/kt-cli/src/main.rs` and `src/apps/kt-app/src/render_dialog.rs`) — one export is two full passes over the material.
- The export pause is a cooperative 40 ms wait in the progress wrappers (`src/libs/kt-engine/src/export.rs`), cancellable; the ETA and speed are computed by the UI — wall-clock in the engine is forbidden.
- During encoder probes the FFmpeg log level is raised to FATAL and lowered back (`src/libs/kt-ffmpeg/src/muxer_impl.rs`, `Muxer::open`): failed candidates are normal.
- `ExportError` (`src/libs/kt-engine/src/export.rs`) no longer contains `GpuRequired`; `FfmpegFailed(ExitStatus)` is a remnant of the pipe path and is not produced by live code.
- `project_range` (`src/libs/kt-engine/src/export.rs`) — `max(material, timeline_end)`; `timeline_end` is written to the file ONLY when its length exceeds the material, otherwise our own file would accumulate a field and break the byte stability of the roundtrip (t151).
- Loudness: `ExportStats.loudness` is computed in `render_raw_audio_range` (`src/libs/kt-engine/src/export.rs`) by the same meter `kt_fx::audio::loudness` (`src/libs/kt-fx/src/audio/loudness.rs`) as the source analysis; the reason for f64 is the SUMS of squares (0.37 LU of drift over 10 minutes in f32), not the biquad coefficients (those give ≤ 0.002 LU) — t403b guards exactly this claim.
- `render_file_name` and `sanitize_name_part` (`src/apps/kt-app/src/render_dialog.rs`): a region gives a segment only for a marker range that MATCHES the region — the name does not lie.
- kt-cli `open_all` (`src/apps/kt-cli/src/main.rs`): pseudo-paths are parsed ONLY by `kt_media::factory` — a second, case-sensitive copy of the `synth:` parsing had already lived there and diverged (`synth:Ramp` counted as a missing file). The media path is already resolved by `kt_otio::load_project`; a second join of the base made `sub/sub/clip.mp4` (t307).
- `probe --time` (`src/apps/kt-cli/src/main.rs`, `probe_range`) prints a linear run AND a repeat of the region (`render_looped_with`): the wrap is a seam that resets the resampler buffer, and a short region's reverb tails cost twice as much as a long one. The numbers from a dev build are inflated by 6–9 times relative to release — judge only by `cargo build --release`.
- `probe` prints for a reel instance the ACTUAL content window (`warp.used()`, `REVERSED`), not the length on the timeline — for a stretched one they do not coincide; the literal `is a NESTED REEL` is held by t452b (`src/apps/kt-cli/tests/cli.rs`).
- Cross-render (`src/apps/kt-cli/tests/cross_render.rs`): the classification threshold is 32 with a palette spread ≥ 96 (D > 2T — provable separability); the probe grid is built from OUR model — transition windows, ±1 frame around splices and placeholder zones are excluded (the fade curves in ours and in melt differ by definition); EDL clips from the reel's hour are rebased to zero (`rebased_clips`) — for a single-colour source the entry point into the picture has no effect; melt without `real_time=0` drops frames and the test is non-deterministic.
- The real film (`src/apps/kt-cli/tests/oc_render.rs`): the start timecode is stripped to zero ONLY on the MLT side — our render gets the file as is, otherwise the test would stop checking our behaviour; an offset of up to two frames is allowed as drift of 24 versus 23.976 (the 90 kHz grid cannot express the NTSC series exactly).
- `nested_example.otio` remains in the smoke tier of t158 (`SAMPLES` in `src/apps/kt-cli/tests/cross_render.rs`) for a reason OPPOSITE to the former one: nested Stacks are dropped by melt (its render is shorter by the sum of their durations), not by us.

## Deferred

- Scaling of FX pixel parameters (blur/glow/vhs) by `quality / project` and the test "the resolution fraction does not change the picture" — the remainder of F-RES-006 (preview and export are equal only at an IDENTICAL Quality).
- A rational fps (24000/1001) in the model, time and export — the residual NTSC drift of 0.1 %.
- Sound in the cross-render (the mlt adapter is limited on audio).

## The render window at the level of v1 (render-window-v1-parity, archived 2026-11-04)

The owner asked for at least v1's information (two labelled forms, the speed chart, the log, Open
file / Open folder); we took v1's rows, chart and log but not its fixed settings string, which was
printed before anything was known. The window now learns the encoder, pixel format, options and
backend from the run itself, and freezes at the end.

- **D1. Events, not state.** `export_mp4_with_backends` takes `impl Fn(ExportEvent)`, where
  `ExportEvent = Opened(EncoderReport) | Progress(ExportProgress) | Log(ExportLog)`. `ExportProgress`
  stays `Copy` (it gains `video_bytes`/`audio_bytes`); `ExportLog` is an enum of facts, not preformatted
  text, so the UI translates it and t955 compares values. Rejected: a non-`Copy` progress carrying an
  `Option<Arc<EncoderReport>>` (every step would carry a pointer set once — a state, not an event), and a
  second callback for logs (two channels whose relative order is lost). `BgJob::poll_with` visits every
  message because `poll` kept only the last: a log line between two progress steps was lost.
- **D2. Packet bytes from the muxer.** `Muxer::open` keeps every candidate that exists in the build and
  failed, with its code (only `last_err` survived), and the opened candidate's options; `drain` counts
  `pkt.size` per stream before `av_interleaved_write_frame`. Packet bytes, not `avio_tell` and not
  `metadata(out).len()`: they are per stream (the bitrate row is the VIDEO bitrate), do not jump at the
  `+faststart` rewrite, mix in no container overhead, and are available to `kt-cli` and tests. The sum is
  at most the file size (t954).
- **D3. The UI owns every clock** (invariant 11: no wall clock in the offline path). The engine reports
  counts; elapsed stops at the result (`finished_at`) — before, it kept growing after Done; remaining,
  speed samples (≥ 0.5 s), size so far, estimated final size (after 2 % of the frames) and the current
  bitrate (a ratio of counts) are the window's. A `Clock` seam lets t957 draw frames at 11 s and 15 s
  without sleeping.
- **D8. The overwrite question is always ours** (`Ask::OverwriteRender`: Replace / Choose another name /
  Cancel). Asked even where the native save dialog already asked: the native question is not guaranteed
  (the `KT_FILE_PICK` seam, portals that do not confirm), ours names size and date, and the behaviour is
  one on every OS and in the live rig. Before it, a failed render over an old file deleted the old file
  too (`cleanup_on_err`). Cost accepted: a double question where the native dialog confirms.
- **D5–D7, D9.** The window follows the R.66 form contract (`window_body` + `window_actions`; Close is far
  from Cancel); the final block replaces the run rows and nothing animates in the final state; Open file /
  Open folder use the one OS helper `util::open_in_file_manager` (invariant 9 is about the export path,
  not the UI); the log is a ring of 800 lines in the job with two files on disk (`render.log` rotated one
  step).

## `kt-cli showcase` (readme-showcase, 2026-10-21)

A separate subcommand, not a `demo` flag: `demo` is synthetic by contract (t388). `showcase` cuts the README project from real footage, with media referenced by absolute path and never copied, byte-identical for the same folder (t824). Refusals (t825): a missing folder, no video, or no audio anywhere — the music bed needs audio. Text files next to the footage are skipped, because FFmpeg's text demuxer would read `CREDITS.txt` as a video. Background and the screenshot pipeline: `openspec/specs/showcase/design.md`.

## Windows (windows-build, 2026-10-26)

- **Encoder cascade per OS** (`src/libs/kt-ffmpeg/src/encoders.rs`, t858): Windows tries
  nvenc, amf, qsv, libx264, mf, mpeg4 — without `h264_amf`/`h264_mf` an LGPL FFmpeg fell back to
  `mpeg4` on AMD/Intel; `h264_mf` comes after libx264 (worse quality). QSV is fed NV12 (it did not
  open with YUV420P); vaapi only on Linux.
- **Revision of "references on linux/x86_64":** there were references for one platform because
  there was one delivery. Now the gated tests (t11, t485) compare against a table per (os, arch)
  with provenance, and t859 keeps a row for every delivered platform. Measured on windows-latest:
  the export AUDIO differs in the last bits (msvcrt's transcendentals), video and the title equal
  Linux's; the ungated FX/geometry hashes matched as they are.
- **mingw's `round`** returned 1 for `round(0.49999999999999994)` — on real Windows, not only
  wine — and every `.round()` of the CPU reference went through it. `src/libs/kt-time/src/
  libm_fix.rs` defines correct `round`/`roundf` for `x86_64-pc-windows-gnu` (t198).

## Every exported frame has a duration (ffmpeg-9, 2026-10-27)

- **What hurt — on every major, unseen until FFmpeg 9.** The muxer sent frames to the encoders
  without a duration, and packets came back without one. The last video sample of every MP4 had
  zero duration (`stts` …, 1×0; a 3150-frame export at 30 fps lasted 104.967 s, not 105.000 s).
  FFmpeg 6–8 kept the frame only because the movie timescale was 1000 and the edit list was
  rounded UP to the next millisecond; FFmpeg 9 dropped the muxer's filling of missing packet fields
  (`FF_API_COMPUTE_PKT_FIELDS2`) and writes the edit list at the track's exact timescale, so it ended
  at the last frame's START and the demuxer marked that frame discarded — t33 decoded 59 of 60.
- **The fix** is in the shared `src/libs/kt-ffmpeg/src/muxer_impl.rs`, not a version branch:
  every frame carries its duration (one tick of 1/fps, `frame_size` samples), and a packet that
  still comes back without one gets it before the time-base rescale. On FFmpeg 6 every packet's
  pts/dts/size/flags stayed the same (compared packet by packet); only the track length is now
  exact. A behaviour removed without an API change is caught only by running the major (t33 in
  `make ffmpeg-test`).

## ci-green-again (2026-11-01): one math for the frozen paths, and a named foreign encoder

**Frozen math (invariant 11 across platforms).** The references were printed with glibc, and the
kernels called the platform libm, which rounds some inputs differently on Windows (t973c, t1019,
t1048 red on windows-latest). The owner chose our own math over per-platform tables: "own math,
close to Linux, where Linux is the more accurate one". `kt_plugin_api::math` (`FrozenMath`) is the
one place, on the pure-Rust `libm` crate; the `f32` functions are the `f64` ones rounded once —
correctly rounded but for ~2⁻²⁸ of inputs — because libm's own `f32` routines were a ulp off glibc
on a tenth of `expf`/`powf` inputs (t1065 measures every function against glibc on a sweep;
t1066 refuses a std call in the frozen crates). The warp's sinc table and dot product are ours too:
rubato built the table on the platform's `sinf`/`cosf` and chose AVX+FMA, SSE or scalar at run
time, so Slowed+Reverb depended on the OS and the CPU (t1067 holds the AVX path bit-equal to the
portable `fmaf` one). Eight t973c values and t11's audio moved — each to EXACTLY the value
windows-latest had printed: on those inputs glibc was the one an ulp off and msvcrt correctly
rounded; nothing else moved. Rejected: interposing `sinf`/`powf` for the whole binary (link order
decides, and egui's and wgpu's math would change silently); per-platform tables (a third CRT would
need a third). Residual: rustfft's twiddles are `f64` cosines rounded to `f32` (~2⁻²⁹ per twiddle).

**A forced encoder outside the cascade** (`h264_vaapi` on Windows) ended with "last error: " and
nothing after it: the loop skipped every candidate. The muxer now names it with the cascade before
the loop (t1063 forces the other system's encoder, so the Linux job proves the path; t962 expects
that error where VAAPI is not in the cascade). Details: `openspec/changes/archive/2026-11-01-ci-green-again/design.md` (D4, D6–D8).
## The announced frame total is the loop's count (render-crash-and-recovery)

The progress total was `(r1 − r0) / step`, floored, while the loop renders from the frame that
CONTAINS `r0` (the frame the preview shows there, invariant 7) up to `r1`: on an off-grid region it
rendered one or two frames more than announced, the window's fraction passed 1, the remaining time
came out negative and `Duration::from_secs_f64` panicked on the UI thread — the owner's crash of
2026-10-31. One function, `video_frames_in`, now gives both numbers; the loop itself did not move
(starting at `align_ceil` would drop a frame the person sees). The window still clamps the fraction
and builds display durations with `try_from_secs_f64`: a counter mismatch costs an estimate, never
the program. The FFmpeg download ETA had the same class (a tiny rate overflows) and the same guard.

### The same crash a second time: the owner's build predated the fix (render-finish-crash, 2026-11-02)

The owner's report of 2026-11-01 (kkt-26.11.01b item 2, build 26.11.01-a04b201d: "it reaches 100 %
and the program folds … even 0.2 s anywhere") came with no crash text — the tests had rotated his
journal away (durability). Reproduced instead: his build predates e72ea7a8, the fix above; his region
`rend` starts at frame 195.51 of the 24 fps grid (477 announced, 478 rendered), and any 0.2 s region
off the grid announces 4 frames and renders 5 or 6. The fraction passes 1 when the audio phase
reports the rendered count, so the UI thread died after the video was written — "it may even save the
file". On master his project renders to the end through the real window. t1070/t1071 held the two
halves separately with scripted numbers; t1312 drives a REAL export of the owner's case through the
window frame by frame to Close (red on the old code with his panic text), and the live scenario
`renderregion.py` checks that the supervisor opened no crash window — with the supervisor a UI panic
no longer ends the process a scenario started, so "the process is alive" alone would pass a crash.

## The final render is pipelined (export-throughput, 2026-11-03)

- **What hurt.** The owner (kkt-26.11.03 C1): "our final render is VERY slow, slower even than the
  preview; it seems not parallel but strongly sequential". One thread did decode → composite →
  GPU readback (a full wait per frame) → RGBA→YUV → encode; libx264 was opened without threads;
  the audio pass started after the last video frame. Measuring it found two decode defects that
  cost more than all of that (media-index design, "The decoder reads what it is asked, once"),
  and a crash: the showcase export died in 3 s (`RGBA_TAIL_PAD`, t1450).
- **One file, its stages concurrent** (`export_pipeline::run`): decode a byte budget ahead with a
  thread per source, the GPU on one thread with three frames in flight (`submit`/`collect`) or
  CPU frame workers (one scratch each, over the shared renderer), the conversion on the frame
  workers, one writer, the audio on its own thread. The encoder receives exactly the serial
  export's frames and samples (t1451); `ExportOptions.serial` keeps the former export as that
  reference and as the "before" of `kt-cli export --serial --time`. Kept rejected: chunks rendered
  in parallel and joined (F-COL-040) — one file, one encoder, one order.
- **Nothing written depends on timing**: the audio interleave is a fixed rule, the progress counts
  are the writer's (t955 compares two exports' events element by element; the first version
  reported the audio thread's own count and broke it).
- **The look-ahead is a byte budget** (`LOOKAHEAD_BYTES` = 4 serial budgets under a cache of 5): a
  chunk being composed is never evicted (R.15 item 1), and at 4K decode sizes a chunk is ONE frame
  — the first version, two chunks ahead, kept 2 of 16 CPU workers busy (337 s).
- **The encoder's threads** (`threads=auto`, in the open report, t1456). A machine writes the same
  file run after run; two machines differ, as they did before (the encoder is the machine's).
- **The export's GPU device is HighPerformance; a software rasterizer composes on the CPU.**
  LowPower stays for the UI (invariant 12). llvmpipe/lavapipe rasterize one frame at a time through
  one queue; the CPU workers compose a frame each: 133.9 s against 95.6 s on the showcase.
  `KT_VIDEO_GPU=1` takes the adapter anyway (`kt_gpu::export_engine`).
- **A stream stage keeps the serial export**: its pre-roll decodes on the compose thread.

### Measured

Release, the showcase (104.0 s, 2600 frames of 1080p25, libx264 veryfast crf 23, seven VP8/VP9
files up to 3840×2160), 32 cores, no GPU (llvmpipe), media in the page cache, output in
`/dev/shm`, other sessions' builds on the machine (load 10–50). Busy seconds per stage summed over
the stage's threads; "decoded" is pictures out of the decoders for the 5,644 the export uses.

| run | wall | peak RSS | decode | compose | readback | convert | encode | pictures decoded |
|---|---|---|---|---|---|---|---|---|
| master + the RGBA pad, GPU (llvmpipe), one thread | 324.0 s | 4.6 GB | — | — | — | — | — | ≈101,000 |
| master + the RGBA pad, CPU, one thread | 860.9 s | 3.7 GB | — | — | — | — | — | ≈101,000 |
| `--serial`, GPU (llvmpipe): the decode fixes only | 206.7 s | 7.9 GB | 34.8 | 120.9 | — | 8.2 | 40.6 | 9,190 |
| `--serial`, CPU: the decode fixes only | 634.4 s | 5.0 GB | 34.9 | 551.3 | — | 7.4 | 38.6 | 9,190 |
| pipelined, GPU (llvmpipe, `KT_VIDEO_GPU=1`), 4 workers | 132.6 s | 14.9 GB | 55.1 | 117.1 | 10.3 | 14.2 | 5.1 | 9,083 |
| pipelined, CPU (the default here), 16 workers | **95.0 s** | 16.2 GB | 63.8 | 1190.6 | — | 14.0 | 7.7 | 9,190 |
| the same, with frame buffers returned to the system (D11) | 115.7 s¹ | **7.3 GB** | 89.9 | 1343.7 | — | 14.6 | 8.6 | 9,190 |
| GPU (llvmpipe) with D11 | 191.0 s¹ | 7.3 GB | 75.8 | 132.0 | 50.9 | 15.1 | 5.1 | 9,083 |

¹ load 55–80 from other sessions' builds during these two runs (10–45 during the others): the wall
times are not comparable with the rows above; the RSS is. Audio and audio encode: 1–3 s busy in
every run. On this machine the export went from 324 s (the program's default then, llvmpipe) to
95 s; the CPU path from 861 s to 95 s. Where the time went before: the decoders made 101,009
pictures for 5,644 used (D8, D9) and one core composed. Now the composite is the bottleneck,
spread over 16 workers (1190 s busy in 95 s); the next gain is the composite itself (the owner's
C2, `compute-backends`), not the plumbing.

- **Freed frames stay with glibc's arenas unless large allocations are mapped** (D11,
  `kt_media::frame_alloc`): the dynamic mmap threshold rises to 32 MB after the first large free,
  and decoded frames freed on another thread stayed in per-thread arenas — 9 GB of the 16.2 GB.
  The binaries fix the threshold at 1 MiB first thing (t1458); `MALLOC_ARENA_MAX=2` alone gave
  10.2 GB.

## Hardware encoding meets its driver outside the render (hw-encode-context, 2026-11-04)

**The crash could not be seen, so it is contained.** The owner: "if you choose h264_vaapi the app
just crashes at render" (ktprompts v26.10 T16). No machine here has a render node, so which fault it
was stays unproven. The device step (no device → no `avcodec_open2`) removes one candidate, and it
also made things worse: with a render node every Auto export now loads the VA driver, where until
then Auto got −22 and never touched it. A driver that faults gives no return code, so no cascade can
fall down on it. `kt_ffmpeg::hw_trial`: our own binary started once more opens the device, opens
the encoder and encodes three frames into `/dev/null`; only its `ok` lets the driver into the render
process. On the code before this, a fake driver that aborts ended `kt-cli export` with SIGABRT in
6 runs of 6, forced and on Auto; t1551 holds the opposite.

**What the trial does not cover.** A driver that passes three frames and faults later ends the
render in the render's own process; what is left then is the supervisor's window and the autosave.
A refusal in the middle of the render is an error with no partial file (t1554): the stream cannot
move to another encoder half way.

**A device candidate is one step** — device → frames → open, and what fails before the open never
reaches `avcodec_open2`. The surfaces are per export, and each NV12 frame the workers convert is
uploaded by the one writer thread (`encode_video_frame`), so the pipelined export needs nothing
of its own for hardware: the converters do not know about surfaces.

**Everything hardware-only is an optional table** (media-index): a build without `hwcontext` keeps
working and loses the candidate.

**QSV has no device context.** `qsvenc` makes its own session from system NV12 frames; a derived
device would only choose the node on a two-GPU machine, and could not be run anywhere. It has the
trial, because on Linux that session reaches the same VA driver. Not measured on an Intel machine.

**What is proven is the path, not a driver.** `fixtures/fake_va` is a VA driver under the real libva
and FFmpeg that fails in every known way and, healthy, encodes I_PCM from the surface, so the file
decodes to what was uploaded: t1551–t1556 and the live `hwencode` run on any machine, a CI runner
included. `make hw-test` (`KT_EXPECT_HW=vaapi`, t963 made loud) stays for a machine with a real
render node. Whether Intel's or AMD's driver opens, its quality and its speed have never been
measured, and t963's bound of 6/255 is the proposal's, not a measurement.

## An export's frames do not depend on LRU (export-pipeline-determinism, 2026-11-05)

**The promise was true only while nothing evicted.** "A frame being composed is never evicted" rested
on the look-ahead fitting the cache. That decides how much fits, not what LRU drops. The cache stays
full during an export, and a composite's read refreshes a frame never needed again, so such frames
outlive frames decoded and not yet read. On `kt-cli demo` each run lost a different set of layers (10,
2, 6 on release), while `--serial` lost none. Now every chunk not yet written HOLDS the frames it
names, found or decoded, and the look-ahead is the bytes held. Memory does not grow: the live set
always had to fit, and dead frames still go first.

**Why neither t1451 nor the showcase saw it.** t1451's 90 frames never fill the cache. The loss needs a
look-ahead far larger than the frames in flight at the workers, so that frames wait decoded and
unread, and it is not a plain ratio of budget to frame size: a quarter of the budget at 1080p lost
nothing, 640×360 at a quarter lost 4–8 layers on every run. t1601's values are proven on the old
code: red 8 of 8, a change to them has to prove itself red again. A scaled-down version chosen by
reasoning passed on the broken code 6 of 6.

**A second way to differ from serial.** The serial export runs `stream_preroll` before every frame,
which re-reads any missing frame, so a span that fails half way heals one frame later. The pipeline had
no such step: one failed span cost 46 frames (t1605). A key a span did not give is asked again
alone, before anything composes it. So a layer that can be had is waited for, and only one no decoder
can give is reported (`ExportLog::IncompleteFrames`, t1602): a render goes on and says where.

**"Equal to serial" means the streams, not the file.** `threads=auto` against the serial's `1` makes a
different x264 stream (t1456, on purpose), and the serial export writes the audio after the video.
At the same thread count (`kt-cli export --encoder-threads 1`) the video and the audio packets of the
two files are identical: t1603, the demo, and the 104 s showcase.

### Measured

Release, the showcase (2600 frames of 1080p25), libx264, 32 cores, no GPU, output in `/dev/shm`, other
sessions' builds on the machine, nothing to be done about them. **Six pairs started together**, each
binary on its own copy of the project, so both meet the same load by construction:

| pair | load | before: pipeline wall | after: pipeline wall | after − before |
|---|---|---|---|---|
| 1 | 23–98 | 170.5 s | 171.6 s | +1.1 s |
| 2 | 98–108 | 181.8 s | 181.7 s | −0.1 s |
| 3 | 108–95 | 182.5 s | 180.0 s | −2.5 s |
| 4 | 95–174 | 177.1 s | 177.1 s | 0.0 s |
| 5 | 174–163 | 166.1 s | 168.1 s | +2.0 s |
| 6 | 163–126 | 204.4 s | 204.4 s | 0.0 s |

Median difference 0.0 s, worst +2.0 s, peak RSS 6.93–7.03 GB on both sides. Every file of both
binaries has the same md5, zero missing layers. Alone at load 50–70, the best runs were 102.1 s
before and 105.3 s after. The published 95 s was taken at a lower load; this machine did not offer
one, and the two binaries are level here, so that figure was neither confirmed nor lost.

The serial export of the same showcase took 895 s.

What this measurement cannot say. Alternating runs gave a median of 112 s against 140 s for the same two
binaries: at load up to 267 the spread (102–204 s) is larger than anything being measured. Two
processes started at the same instant on ONE project file can split widely, and that is not the
code: identical binaries gave 204 against 153 s and 257 against 275 s, and old against new 130
against 180 s, 248 against 250 s, 220 against 219 s. An earlier series of four such pairs gave
old against new 194/194, 104/226, 107/276 and 137/185 s; its stage logs were overwritten, the two
widest gaps exceed the 51 s seen between identical binaries, and they remain unexplained. In none of
the 13 same-instant pairs was the old binary the slower by more than 3 s, though the process started
first was the slow one in most of the wide gaps, and the new binary had been started first in
several. The cause of the split was not found (the two exports do not wait for each other: both run
from the first seconds). The six pairs on separate project copies did not split.

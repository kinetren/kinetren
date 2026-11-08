# Design: ui-never-waits

## Context

The owner's words: `docs/ktprompts/kkt-26.11.07.txt` row E8, `kkt-26.11.08` row H9 (and found-defects
121). His journal: `/media/e/developer/kt.9/fails/26.11.08/new2/load-report.txt`, release build
kt26.11.08-360b50e6, Ryzen with an integrated Radeon (RADV RAPHAEL_MENDOCINO, Vulkan). Here only
llvmpipe runs; what that cannot prove is said in D5.

## D1. The mechanism, measured

His journal, 06:38:35–06:39:09, playback over events he had just given a Datamosh:

- `NOTE the UI thread was blocked for N ms inside one frame`: 1019, 4509, 232, 296, 825, 2255, 3232,
  1917, 3704, 3343, 2502, 1608 … ms; the note measures `draw_all` alone, so the wait was inside the
  interface's own frame, not in egui's paint or the swapchain.
- `LOAD` 22–57 % of ONE core for the whole process; `composites` 1 a second, `device_submits` ~45 a
  second; `ui_frames` 1–3 a second. `PLAY … V-fps 9.0`.

The UI thread did not compute, it waited. One composite a second, but 45 submits a second. The
submits are the compute host's (`kt-gpu/src/compute.rs`, one device, jobs serialised): a Datamosh slot
is a STREAM stage (`video_stream.rs`), which runs inside `frame_plan` (`layer_src` → `stream::run` →
`StreamKernel::render`) on the thread that composes, and its kernel decodes the event's history, one
compute job per frame, each read back with `poll(wait_indefinitely)`; the prefetch thread ran the same
jobs ahead of the cursor (found-defects 134, `decode-request-storm`, merged beside this change, which also
limits a composite's own share of a history to two positions). The composite ran in the UI frame
(`draw_preview` → `Player::present_preview`), so the UI thread stood through its own jobs and behind the
prefetch's on the one compute device.

Reproduced on our fixture (`kt-cli showcase --heavy`, Datamosh at the far end of its range on the
file events of the first ten seconds: search 64, gop 600, before 300, mosh 1), debug build, llvmpipe,
640×360: `kt-cli pause-scan` composes as a paused preview does, synchronously — single composites of
3739, 2282, 552, 2087, 1046, 281, 689 ms. That is what the UI thread waited for.

The rule: the UI thread never blocks on the GPU or on a lock another thread can hold for long; a
frame that is not ready shows the last one; Undo and every window stay responsive while an effect
takes seconds a frame.

## D2. The walk: every place where the UI thread waits for what it does not control

Found by reading every UI-thread path (`apps/kt-app/src/**` from `KtApp::ui`, gestures, menus,
windows) and the `Player` methods it calls. Each row: what waited, on what, and what this change does.

**GPU and the composite**

| Place | What it waited for | Now |
|---|---|---|
| `draw_preview` → `present_preview` / `compose_preview_into` | the whole composite: plan (a stream stage's compute jobs), resolve, upload, the executor | on the preview's thread (`compose_queue`); the UI asks, takes an answer within 3 ms, else shows the last frame (t1840, t1842, t1848) |
| `PreviewDisplay::sync` → `display_target()`, `glow_view()` | the engine's `inner` lock, held by the preview's thread through `present` | published in their own slot at the end of `present` (t1845) |
| `coverage_runs` → `final_video_runs` | `exec.holds()` per kept moment (the engine's lock) and up to 32 `frame_plan`s (a stream stage's kernel) | the check runs on the preview's thread between frames; the UI reads the map; the executor is asked with the map free (t1843) |
| the display queue | a slow chain on egui's device: the swapchain waits behind it | a chain whose EXECUTOR time passes 150 ms moves to the offline device; the plan's time (a stream stage, on the compute host's own device) does not count |
| `fx_editor` → `compute::unavailable()` per frame of a Datamosh slot | the first call opens the compute device (`OnceLock`), or waits for another thread opening it | `compute::unavailable_now()`: unknown before the open, which goes on a thread (t1847) |
| `compose_news` → `missing_layers` → `layers_at` | a stream kernel's `needs`, under the stream memo | checked: a lookup in the memo, no job; kept |
| `init_video_gpu` (first frame of each project) | `GpuFxEngine::new` compiles every pipeline | kept: once per opening, before any picture; the offline device is already built on a thread |
| `on_exit` → `park_offline_engine` | the offline engine's last job | kept: exit, by design (gpu-render-hardening D6) |

**Locks another thread holds through long work**

| Place | Held by | Now |
|---|---|---|
| `Player::set_project` (every edit, undo, a level drag), `update_gen`, `set_media_path` → `project` lock | the audio render thread's `AudioRenderer::new(&project.lock().clone())`: the guard lived through the whole build, 385–412 ms on the owner's project | the clone is a statement of its own on the render thread (both builds) |
| `add_media`, `replace_media`, `remove_media`, `Drop` → `workers` write lock | `shutdown()` joined the decoder INSIDE the `if let` on the guard: the render, prefetch and preview threads waited too | the guard ends with its statement; the join follows |
| `remember_native`, `remove_media` → `natives` write lock | the prefetch thread held the read guard through `VideoRenderer::new_with_facts` | the facts are cloned out first (also the UI's own two builds) |
| `ThumbService::add_media`, `retune_media` (every generator value edit) → `shared.media` | the thumbnail worker held it through `FfmpegSource::open_indexed` (the guard in an `and_then` chain) | the path is cloned in a statement of its own |
| `with_probe` (meters on a pause, the loop's region) → `levels_probe`, `project` | its own `AudioRenderer::new` on the UI thread, once per version (found-defects 111, 117) | a later version is built on a thread of its own while the one before answers; the loop head is pinned again when it lands; the first is built where it is asked (t1844, t1875) |
| `BurstCache` inner (coverage, epoch, `contains_video`) | decoder inserts | checked: short holds; kept |
| `video` renderer pointer, `shown`, `video_gpu`, `video_cost`, `loop_region`, `preview_quality`, `gen_states` | — | checked: field reads and pointer clones |

**Decode, files, builds on the UI thread**

| Place | What | Now |
|---|---|---|
| render window, resolution combo → `max_source_resolution_cached` | opened EVERY file of the project, keyed by the revision: again after each edit while the window was open | keyed by the files, found on a thread, the value before answers meanwhile |
| `Player::set_project` → `VideoRenderer::new_with_facts` | a whole-project build per edit | measured: 23 ms on the owner's 1974 events (debug, `kt-cli probe` now prints it); kept, it is `edit-without-rebuild`'s (found-defects 110) |
| `sync_media_sources`, `add_media_clip_at`, `place_media_at`, `build_player` | open a source (FFmpeg + sidecar) | kept: a gesture that brings a file in; listed below |
| Ctrl+S, presets → `write_atomic` (fsync) | the disk | kept: the owner's decision (2026-11-07, answer 9: Ctrl+S writes next to the project, with fsync) |
| autosave | writes on a thread; takes `model.document()` on the UI thread | kept |
| Open, recovery, `replace_with` (old player's `Drop` joins render and prefetch) | parse, joins | kept: gestures that replace the project |
| `journal::log` on `warn` | a synchronous append | checked: one short write per notice |
| `dump_geom` under `KT_GEOM_DUMP` (the live harness) | a file write and rename up to 10×/s: 10–20 s on a busy disk (D6) | written by a thread of its own |
| clipboard | reads on a thread already (`envclip`) | — |
| reel previews, posters, effect thumbnails | the offline engine | on their own threads already; the UI never takes the offline engine's lock |

## D3. The started work: kept, and what changed in it

`claude/ui-never-waits-wip` (f8ac500c, cut before cached-range-plays and preview-throughput's merges)
moved the composite to a thread of its own and sent a slow chain to the offline device. Its approach is
right: a composite cannot be stopped inside the device, so the only way the UI does not wait is not to
be the thread that composes; and a slow chain on egui's own device makes the swapchain wait for it, so
a thread alone is not enough. Kept, with these changes:

- merged over the kept composites: the preview's thread records what it presented, and the recheck of
  those moments moved there (above);
- the route to the offline device is judged by the EXECUTOR's time: the WIP timed the whole composite,
  so a Datamosh frame (its time is the stream stage's, on the compute host's own device) sent every
  frame to the offline device with a readback, for nothing;
- the last composite's bytes stay in `preview_buf` (two harness tests read the picture);
- the UI's composite stage of `frame_cost` now measures what the UI spends on the picture: the ask and
  the take.

Rejected: cancelling a composite. A frame inside the device runs to its end; "cancelled" means nobody
reads it. Rejected: a fixed timeout after which the UI gives up waiting: any wait inside the frame is
the defect, and a timeout is a wait.

## D4. A new project state goes to the offline device first

The first frame of each project version is composed on the offline device and timed (the WIP's
rule): an edit that adds an anomalous parameter would otherwise put a frame of seconds on egui's queue
before anything knows it is slow. The price: one frame per edit is read back and uploaded by the bytes
path. A slider drag is an edit per step, so a drag shows its frames through the bytes path; on the
heavy showcase that costs the UI a few ms a frame here. Accepted: bounded, against an unbounded wait.

## D5. What llvmpipe cannot prove

- On llvmpipe each device is its own set of threads, so the offline device runs in parallel with the
  display. On his RADV both devices share the GPU's graphics ring; the kernel driver interleaves at
  submission granularity. Datamosh submits a job per frame of history (short submissions), so the
  display's frames get between them; ONE long submission of a slow chain on the offline device would
  still hold his ring for its length. Splitting a slow chain's passes into separate submissions, and
  timing them, is `gpu-pass-cost`'s.
- His UI frames after this change were not measured on his card; the journal's notes are the proof he
  can bring: `NOTE the UI thread was blocked …` should be gone during playback.

## D6. The harness measured itself

The first release runs on a scratch copy of the owner's relinked project still noted UI frames of
0.1–1.3 s, and on a cold sidecar 10–20 s, at about a millisecond of processor time a frame: a wait.
The journal's note did not say where, so it now lists the frame's stages by wall time (t1846). It named
the timeline, and inside it the live harness's own geometry dump (`KT_GEOM_DUMP`): the UI thread wrote
the file up to ten times a second, and on a disk busy with the sidecar's fsyncs a write stood for
seconds. The owner's program never writes it, but every live measurement of the interface's waits
measured the harness, and the flaky live `idle` (found-defects 112: hover frames lost 3–10 s at 0 %
processor) has the same shape. The dump is now written by a thread of its own; under `cfg(test)` it
stays synchronous, because the harness reads it right after its frame.

## D7. Numbers

- Before: his release run, single UI frames of 0.3–4.5 s while playing over Datamosh. Here, the
  synchronous composite the old UI frame stood in (`kt-cli pause-scan`, our fixture, 640×360,
  llvmpipe): 0.22–1.16 s a frame on the release build, 0.28–3.7 s on the debug build (D1).
- After, release, a scratch copy of the owner's relinked project (1974 events, his Datamosh event at
  2619 s), cold sidecar: 91 s of playback over the Datamosh event with its effects window opened and
  closed — the journal noted NO UI frame of 100 ms or more; the UI drew 40–80 frames per quarter
  second throughout; V-fps 0.6 (the stream kernel on llvmpipe's compute).
- After (live scenario `ui_wait`, the same fixture, debug, llvmpipe): playback over the moshed events,
  the effects window opened by a double click in 0.3 s, a Datamosh added from the gallery and removed by
  Ctrl+Z in 0.3 s, the window closed by Done in 0.5 s; the journal noted NO UI frame of 100 ms or more.
  Meanwhile `preview-compose` held 3–4.5 cores on the stream kernel and one new frame came out in
  3.9 s of playback (V-fps 0.3): the slow effect cost frames, not the interface.

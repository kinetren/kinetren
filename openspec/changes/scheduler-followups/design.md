# Design: scheduler-followups

## Context

See proposal.md for the four rows. What the code does now (`orch-26-11-01` at 6a7b0255):

- **Open (270).**
  - `KtApp::open(path)`: `load_project_file` reads and parses the `.kt` on the caller's
    thread. `open_loaded` → `build_player` then calls `open_source_or_placeholder` per media
    (FFmpeg probe plus the sidecar, counted as `FILE_OPEN`/`SIDECAR` by `kt_time::block`) and
    `Player::new`.
  - `Player::new` builds the first version on the caller's thread: `build_version`, counted as
    `BUILD` in `incremental.rs`. The kt-engine CLAUDE.md names this as the one exception: "The
    opening is the one build on the caller's thread".
  - `replace_with` does `*self = app`, so the old `Player` drops on the UI thread. That joins the
    build, render and prefetch threads and runs `MediaWorker::shutdown` per source (`JOIN`).
  - The callers (menu Open, Recent and the start window, drop, import, the live rig, lessons) are
    UI-thread code.
- **Save (252).** `save()` posts the write and both fsyncs on the `save` lane (one-scheduler, row
  98, t2236). Row 252 was observed before that.
- **Loop (251).**
  - Only the compose thread composes the preview, one ask at a time, newest first.
  - A moment is kept on the display executor's present memo when it was presented, or when the
    slow route made it (`compose_via` → `VideoGpuExec::keep`).
  - The prefetch thread tells the memo the loop's size (`tell_loop` → `keep_loop`). It skips kept
    moments in its look-ahead (`video_prefetch_pass_except`) and sizes the frames cache to hold
    the loop (frames-cache-growth).
  - So after a lap the loop's frames are usually decoded; what is missing is composites of moments
    the display never asked for.
- **Offline engine (290).**
  - `kt_gpu::offline_engine()` is one `GpuFxEngine` on a headless device. It is shared by the
    slow route (compose thread, class `now`), effect cells (`fx-thumbs` lane, `visible`), reel
    strips (`reel-preview`, `visible`) and posters (`posters`, `background`).
  - Every frame takes `inner` (arena, pool, memos) for its whole resolve, submit and readback.
    gpu-arbiter cuts a frame into pieces on a ring that carries a display. On a card the offline
    device shares the display's ring; on a software rasterizer it is a ring of its own, which
    neither bounds nor cuts.
  - So the slow route waits for the whole side frame twice over: on the mutex, and on llvmpipe
    on the unbounded ring.

## Goals / Non-Goals

**Goals:**
- `kt_time::block` reads zero on the UI thread across File → Open, a recent card, a dropped project
  and the import, including the release of the project being closed.
- A loop that plays on a display slower than its frames becomes all-kept. That is, from some lap
  on, the compose thread shows only kept moments wherever the frames cache and the present memo
  hold the loop.
- The slow-route frame's wait for side work on the offline device is bounded by one ring piece and
  is visible in the trace.
- Nothing new at rest: the fill runs only while a loop plays, and the load only while the project
  opens.

**Non-Goals:**
- The program's start (`eframe` creation) still reads its start project before the first window
  frame. The window does not exist yet, so nothing stands in it. Its sources and build go off the
  UI thread like any open.
- Row 334 (the heavy showcase's loop is never kept on llvmpipe because ~886 composites a lap are
  incomplete) is not this change. The fill composes only moments whose frames are decoded, so it
  cannot help an incomplete frame. It is kept as a row with what the fill measured there.
- No change to what a frame is (its plan, key or bytes).

## Decisions

### D1. The player builds its first version on its own thread (`Player::new_deferred`)

- The constructor publishes a build of an empty copy of the project: the same settings, with no
  clips, media, reels or tracks. That costs microseconds, so `Builds` always has a `Built` to hand
  out.
- It then hands the real project to `engine-build` as version 1, the same path as an edit.
- Consumers already handle a want ahead of the build:
  - the render thread holds the next start until the build lands (`build_holds_start`, time
    stands before the first frame);
  - the compose thread waits for its state's build (`wait_current`), so the preview shows
    Loading;
  - prefetch reads `latest()`.
- `Player::new` (kt-cli, kt-engine tests, exports) keeps the synchronous first build: an offline
  caller wants a built player at once, and it is not on a UI thread.

Rejected: a `KtApp` with `Option<Player>`. Hundreds of call sites read `self.player`, and a
`None` window is a new state for every one of them to get wrong.

### D2. The sources open on the `media` lane, through the existing sync

- `open_loaded` sets `media_paths` to the project's media without opening anything. It posts one
  job on the `media` lane (class `answer`, FIFO). The job sets the decode threads from the memory
  it measures, opens every source (`open_source_or_placeholder`) and adds it through
  `Player::sources()`, which is the add body off the UI thread. It answers each source's
  `(id, label, error, length)`.
- `take_media` lands the answer as `MediaThen::Load`: the placeholder set, the missing-file
  warnings and their count, and `note_media_length`. These are the same lines the constructor
  wrote before, only later.
- The lane is FIFO, so a sync posted by an edit during the load (a media path changed, a file
  added) runs after the load's job and replaces or adds on top of it.
- The thumbnails, index and media-watch services already start as lanes and open nothing on the
  caller's thread.
- Under `cfg(test)` the constructor ends with `take_media(ANSWER_GRACE)`, where the grace is the
  test's bound. A harness test that opens a project then sees its sources, as `COMPOSE_GRACE`
  composes in the frame. In the product the grace is 3 ms: a small project lands in the same
  frame and a large one in a later one.

### D3. The project file is read on the `media` lane; the app is swapped when it answers

- Every UI open path posts `KtApp::open_job(path, how)` and keeps an `Opening { answer, then }` on
  the app. The job runs on the `media` lane and is wrapped in `block::wait(FILE_OPEN)` so an
  inline call would count. It does `load_project_file`, and for an Open (not an import) it also
  does `project_lock::acquire`.
- `take_opening` runs in the frame next to `take_media`. When the answer lands it builds the app
  from the loaded parts (`open_loaded`, which now opens nothing), applies the lock state,
  `replace_with`, and runs the caller's tail (the import's save hint and note, the rig's
  recovery flags).
- A second open while one is in flight replaces it (the newest wins), as a newer ask replaces a
  waiting frame.
- `KtApp::open(path)` stays for the start and for tests: the synchronous read plus D1 and D2.
- The status line says "Opening <file>…" while the read is in flight. It is the same hint slot
  the media open uses (rule 10, one place for hints).

### D4. The closed project is released on a thread of its own

- `replace_with` moves the old app's `Player` and sound output into a short-lived `app-drop`
  thread. Dropping a player joins its threads and decoders, and that is exactly the "finite and
  cancellable" work invariant 5 allows off the queue.
- It is a thread and not a lane job: a decoder's last span may take as long as the disk likes, and
  a lane worker held by it would delay the next open's `media` job.
- The rest of the old `KtApp` (egui textures, maps) drops on the UI thread. That is memory only.

### D5. The loop fill: a `near` service on the player, an engine of its own, kept on the display

- `loop_fill.rs` in kt-engine is a scheduler service lane `loop-fill` of class `near`. `near` is
  not paused by the player's vote (only `visible` and `background` are), and the rings admit it
  behind `now`. It holds `PreviewParts` and an executor injected by `Player::set_fill_gpu`.
- It is kicked by the prefetch thread when it tells the loop (`tell_loop`) and on every pass while
  a loop plays and the fill is not done. Each pump is one moment:
  1. Check that a loop region exists, the transport plays, a display executor exists and the fill
     executor exists. If any of these is missing, go idle.
  2. Walk the loop's moments from `playhead + lead` forward, wrapping, and take the first moment
     that is:
     - not held by the display executor (by the probe plan's key, as `show_kept` does);
     - not skipped this lap;
     - warm: every layer of `layers_at` is in the cache, so the fill asks no decode and withdraws
       no request of the frame on screen.
     The `lead` is the fill's own last frame time × 2, at least one frame. The moment just ahead
     of the playhead is the compose thread's to make, and composing it twice is waste.
  3. Compose it with `compose_via(Some(fill), None, t, Bytes)`. That is the slow-route path: a
     complete, not held composite is `keep`-ed on the display executor and noted as kept (the
     bars show it).
  4. If the frame came out incomplete or held, or the display executor does not hold it after the
     keep (the memo refused or is full), mark the moment skipped until the next lap. If nothing is
     left this lap, go idle until the next kick.
- The skip set is cleared when the lap number moves, on an edit (project or content version) and
  on a quality change.
- `KT_LOOP_FILL=off` disables the service, for an A/B on one binary.
- Trace: `Ev::Job` with `job::LOOP_FILL` per moment (moment, outcome), and the compose's own
  records under the fill job's id as its cause.
- The fill executor in kt-app is `kt_gpu::side_engine()` (D6). In `kt-cli preview` it is the same,
  so the bench is the app's arrangement.

Rejected: composing fill moments on the compose thread between asks. On a slow display there is no
time between asks. On a card the fill's value is the GPU's idle time between display frames, which
only a separate submitter at a lower class can use.

Rejected: decoding for the fill. A fill that asks decodes competes with the look-ahead for the frame
on screen. The frames cache is already sized to hold the loop (frames-cache-growth), so after a lap
they are there.

### D6. A second engine on the offline device for side work; the slow route bounds its ring

- `GpuFxEngine::sibling(&self)` builds an engine on the same device and queue that shares the
  device's `Compiled`, so there is no compile and no shadow. It has its own `Inner` (arena,
  pools, memos). Comparing devices by id (`new_kept`'s rule) is not involved: the `Compiled` is
  handed over directly.
- `kt_gpu::side_engine()` is the offline engine's sibling, made once. `fx_thumbs`, `reel_preview`
  and the posters ask for it. `park_offline_engine` parks both.
- The slow route keeps `offline_engine()`. When kt-app installs it as the slow route, it calls
  `serve_preview()`, which registers the engine's ring as carrying the preview (`Ring` gains a
  `previews` count in `bounded()`/`cut_before`).
  - On a card that ring already carries the display, so nothing changes.
  - On a software device the offline ring from then on cuts a lower-class frame when the slow
    route waits, and bounds its flight.
- So the slow-route frame waits on the ring for at most one piece, never on a mutex held by an
  effect cell.

Rejected: routing side work to a second headless device. That is another device and its pipelines
(1.3 GB at 1080p on llvmpipe by `memory::engines_for`'s measure, and a compile). The sibling costs
an arena and pools.

### D7. The class walk

The mechanism: a thread that must answer (the UI thread, the compose thread for the frame on
screen) takes something another job holds for its whole length: a lock, a file, a build, a join,
a device frame. Every place was read in the code at 6a7b0255. Verdict per place:

| Place | Waits on | Verdict |
|---|---|---|
| `KtApp::open*` → `build_player` | FFmpeg probe, sidecar, first build | fixed: D1, D2 |
| `load_project_file` in the open paths | the `.kt` read | fixed: D3 |
| `claim_project` → `project_lock::acquire` | lock-file open, write | fixed: D3 (in the job) |
| `replace_with` → old `Player::drop` | joins of threads and decoders | fixed: D4 |
| `insert_dropped_as_sequence` | `load_project_file` of the dropped file | fixed: the same lane job |
| `KtApp::new_with_project` (New, templates, lessons) | first build | fixed: D1 (no sources) |
| `create_generator_at` → `open_source` | a factory source (no file) | not a wait: a generator opens no file |
| Ctrl+S (`save`) | write and fsyncs | already on the `save` lane; proved under a busy disk (t2333) |
| autosave | write | on the `autosave` lane |
| `on_exit` waits for the save and the posters (3 s) | the lanes | allowed: the window is closing |
| `draw_preview` → `take_composed` | the compose thread | bounded by `COMPOSE_GRACE` |
| UI → `GpuFxEngine` `inner` | a frame | none: `shown` is read without `inner` (ui-never-waits) |
| compose thread, slow route → offline `inner` | an effect cell, reel strip or poster | fixed: D6 |
| compose thread → offline ring (software) | a whole side frame | fixed: D6 (`serve_preview`) |
| compose thread → display ring | look-ahead and compute pieces | bounded (gpu-arbiter) |
| compose thread → `Builds::wait_current` | the build of its state | by design: the frame of a state needs its build |
| compose thread → `workers.read()` | a writer adding a source | short: the add takes the write lock only to insert |
| prefetch → `tell_loop` → display `inner` | a composite | allowed: not a thread a person waits on |
| fx thumbs, reel strips, posters → side `inner` | each other | allowed: the same class band, all background |
| loop fill → side `inner` | a poster | allowed (near behind background on the mutex, by design small) |

| `replace_with` → `note_recent` → `StartStore::write` | the recent list's tmp + rename | left: found-defects 370 (no fsync, page cache) |
| `Ask::OverwriteRender` drawing | a `stat` of the target per frame | left: found-defects 370 |
| `save_flight_log`, `offer_recovery` (read_dir), `forget_recovery`, `drop_own_recovery` | a write, a folder read, unlinks | left: found-defects 370 |
| `run` (the start) → `KtApp::open` | the start project's read and parse | left: found-defects 371 (before the window exists) |
| loop fill vs compose thread on one renderer | `stream_held` was one flag per renderer | fixed: per thread (D5) |

Rows found and not fixed go to `docs/found-defects.md` rows 370–389 (370–372 used).

## Measured

- **Open (270)**: live `open_heavy` (debug, llvmpipe, Xvfb): File › Open of the heavy showcase (605
  events) from a small project — the events on the timeline at once, the File menu opened during the
  load, `block.ui` 0 and `block.render` 0 over the whole run. t2330 red on the former synchronous path
  (`file_open ×1` counted), green on this one.
- **Save (252)**: t2333 — three Ctrl+S while a thread writes and syncs 32 MB in a loop in the same
  folder: no blocking call on the UI thread, every save lands in a later frame. Nothing left to fix.
- **Slow route behind side work (290)**: t2337 on llvmpipe (one software device, 1080p): beside heavy
  side frames of 178–184 ms (VHS Tape + Camera ISP), the route's light frame waited 119–153 ms when the
  side work shared its engine (the former arrangement) and 3–26 ms on the side engine with the ring
  bounded (30–32 cuts per run), two runs.
- **A loop on a slow display (251)**: `kt-cli preview heavy.kt 30 32 --size 640x360 --loop 90`
  (the heavy showcase, release, llvmpipe, 50 moments a lap), with the fill and `KT_LOOP_FILL=off`
  one after the other, each run's `kt-cli trace … loop`:
  - first version (the fill ran whenever there was work): all-kept from lap 17 against lap 16, V-fps
    16.6 against 19.5 — on a software rasterizer the fill and the display share the processor, so it
    only moved work off the display's own frames. Hence D5's gate below.
  - with the gate (load 30–60 on the machine): **all-kept from lap 9 against lap 18**, V-fps 20.6
    against 18.4, kept composites shown 1859 against 1624; the fill kept 30 moments, the display
    composed 53 against 52.
  - `trace loop` counted the fill's composites as the display's frames (no `FrameAsk`, tick 0) and
    blamed "layer 0 reads another frame": `lives` now takes only the frames the display asked for,
    and a lap names the fill apart ("fill: N kept of M composed").

### D5 revised: the fill takes only the time the display leaves

A display composite that runs a plan (not a replay of a kept moment) is counted while it runs
(`PreviewParts::composing`); the fill starts no moment while one is. On a card that is the device's
time between frames; on a software rasterizer it is the time the display spends replaying kept
moments, which grows lap by lap, so the fill accelerates the convergence instead of slowing the frames
the person sees.

## Risks / Trade-offs

- [A harness test reads the player right after an open and sees the empty build] → The compose
  thread already waits for the build. A test that reads `renderer()` directly waits for
  `built_seq` through a test helper. The kt-app suite is run whole to find them.
- [A Play pressed during the load plays silence] → `build_holds_start` holds the start until the
  build lands, and the sources open first on the lane. A source added after the build bumps a
  rebuild and holds the start too. Covered by t2331.
- [The fill and the display compete for a software rasterizer's threads] → The fill runs one
  moment at a time, on another ring, at `near`. When the loop is kept, the display's own
  composites stop, so the total work after the fill falls. Measured on llvmpipe, before and after.
- [The present memo cannot hold the loop] → The fill stops at the first moment the memo does not
  keep (D5 step 4). The ledger's grant, not the fill, decides how much of a loop is kept.
- [A sibling engine doubles the offline device's pools] → Bounded by the pools' own caps (memory
  ledger accounts). The side engine is opened only at the first effect cell, strip or poster.

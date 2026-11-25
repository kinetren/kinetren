# Design: gpu-waits-named

## D1. The cause of row 835

Read from the evidence (`live-evidence-24/render_honest-b`, the first try's `stand-trace`) and the code; the
wait itself was not recorded by the build that stood, so the last step is an inference.

- The window is on GL, so the preview composes on its own GL device and the render-ahead producer on a
  sibling of it: one `Compiled`, one `gl_gate`, one ring (id 1).
- The producer's job (sched-4) starts at +370.7702. Its `gpu_frame` record (the harvest of the frame before)
  and its `gpu_admit` both stand at +375.5989, 1 µs apart. Between a frame's start and those two records the
  executor only resolves the plan, uploads and harvests; the harvest hears the passes of the frame before
  (a size first measured then) and calls `seeds::note`, which WROTE `pass-times-v1.tsv` in place: a
  file, a rename over the old one, under the table's lock, inside the device's gate. The first measurement
  of a process and then one each 30 s write.
- The preview's frame (+370.7824) waits for the gate and is admitted at +375.6016, right after the
  producer's `gpu_exec` (+375.6004).
- No other disk call exists on that path: the gate, the engine's lock, the frame counter, the ring's lock
  and the error scopes are memory. Mesa's own disk cache is read at a pipeline's creation or its first
  draw; no `warm`, `cold_draw` or creation record stands in the window.

t3250 reproduces it: the file is a pipe nobody reads, and the former code never came back from `note`.

## D2. What is recorded

`holds::hold` already wraps the calls that hold a device; its end now records the call. `holds::lock` takes
the locks of a frame's path (`try_lock` first: a free lock costs no clock), `holds::file` this crate's
files. Threshold 500 µs: a shorter one is the call itself, and a record per uniform write would drown the
trace. A frame that stands for good has no record until it ends; `holds::in_flight` names it then (the panic
report). Nothing records at rest.

## D3. The pass times

A frame asks and tells the table in memory. A thread of a few milliseconds reads the file (asked at
`set_cache_dir`, before the first frame) and writes it when due; a write never precedes the read, so the
file's other lines survive, and what the process measured stands over the file's. Until the file is read a
pass is unmeasured: a cut moves, no byte does. The exit's flush still waits for the file (the close is
bounded by `exit_watch`).

## D4. The displayed frame and the disk tier

`show_kept` opened the store, looked up the segment and waited for the reader's decode on the compose
thread: the same shape as row 835 on another file. The read now runs on a lane of its own (class `Now`);
the frame waits 250 ms while playing and 2 s at rest, then composes. A read given up on stays in flight:
until it ends the frames after it ask nothing (or each would wait the bound again), and what it brings is
kept under its key. Rejected: no wait at all (a covered loop would compose every frame beside its
read); a frame period (a segment's first open and a cold decoder take longer, and the composite it buys
is the slow one).

## D5. Class walk: where a wait can stand on a disk, or on a lock held across one

The preview's compose thread:

| Place | Was | Now |
|---|---|---|
| `seeds::note`/`predict` from `harvest`, `cut_point`, `Piece` | file under the table's lock, inside the device's gate and the engine's lock | D3 |
| `compute.rs` `run`: `last` lock held over `seeds::predict`/`note` | the same | memory only |
| `compute.rs` `pipe`: one lock over every `create_compute_pipeline` | a job waited for another kernel's compile (the driver reads its cache in it) | a lock per text |
| `show_kept` → `restore_from_disk`: store open (manifests), segment open, decode | on the thread | D4 |
| `DiskStore::contains` → `drop_segment` (unlink) on a changed `made` | on the thread | on the lane |
| `set_video_gpu`: the old engine dropped under `video_gpu` | the preview waited for its gate | dropped with the lock free |
| `RenderCache::set_config`: the store dropped under its lock, joining a reader that may read | both | on a lane |
| `builds.wait_current` | the build derives from memory, opens no file | clean |
| `workers` map | the old decoder is joined with the map's lock free (ui-never-waits) | clean |
| `video_gpu`, `shown`, `on_disk`, `ix`, `cfg` | memory under them | clean |
| cache folder `inner` (`missed`, `used`, `take_dropped`) | never held over a file: the walk, the drops and the stamps read and write with it free | clean |
| `errors::emit`, the journal | a queue and a thread (found-defects 833) | clean |
| the trace's writer | takes the texts out, then writes | clean |
| compute host, stream front, side and offline engines: built at first use behind a `OnceLock` | a device opens (the loader and the driver read files) | prepared off the thread; a first frame that needs one still waits for its build — it has no other way to a picture |
| Mesa's/the driver's own cache, read at a pipeline's creation or first draw | in the driver | off the frame by `pipeline-prewarm`; on GL a warm draw holds the gate for one draw, the shadow device having compiled it. Named now (`gl gate`, `create pipeline`) |

The prerender and the producer (their own threads: they may stand, nobody may wait for them inside a lock):
`store.begin/push/commit/make_room` hold no lock of the store over a file; `rc.store` opens under `opening`,
which only the threads that read take; the reader is one thread behind a channel. A standing disk can hold
scheduler workers (the producer's restore, the reads' lane, a media open): three on a small machine, and then
background answers wait. They wait for the disk, and no frame does.

The UI thread: the welcome window asked `exists()` of every recent card and the overwrite question asked
`metadata` in every drawn frame: asked once now. Left, each one call at a gesture, in found-defects 911: the
fsync of an effect or render preset, the presets' first read, the project's lock file, the recovery
list, the removal of a poster or a recovery file, the flight log.

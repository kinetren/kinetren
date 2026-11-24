# Design: durable-moment-key

## What happens today (read in orch `887c2972`)

A composite kept in the display's memo is found by `plan_key` (`kt-gpu/src/engine.rs:4336`). For every layer it hashes:
- `LayerSrc.content`, the cache entry's serial; `layer_src` (`kt-engine/src/video.rs:2569`) puts **0** when the frame is
  not in the cache;
- the input's presence: tag 0 for `Frame(None)`, else tag 1 with alpha, width and height (`engine.rs:4384`).

`show_kept` (`player.rs:1878`) asks the memo by a probe plan BEFORE anything is decoded. For a moment whose frames were
evicted, the probe's key is another key, the memo answers "not held", and the moment is decoded and composed again.

**Correction to row 965's cause.** It says the frames "decoded again take new serials". They do not: `Inner::past`
(65,536 entries, t2152) gives an evicted entry its serial back; the run evicted 3,808. After the decode the composite
is found again (t2294 holds exactly that). What fails is the moment before the decode, which is the whole use of a
kept composite. So a longer memory of serials fixes nothing. This is from the code, not from a new run; t4108 is
written first and must be red on the old build for this reason.

The disk key (`durable_hash`, `render_cache.rs:639`) already has neither. It is not simply usable for the memory:

1. **It can name an old picture by new values.** It takes a generator's `src_rev` from the renderer, which is new at
   once; the source is retuned later, on its worker (`worker.rs:363`). Between the two the cache holds OLD frames. The
   present key is safe there (the serial is the old one); the durable key would keep the old picture under the new
   name, for good. On every drag of a title's slider this window is open. On disk it is closed only by the 1.5 s of
   quiet, unless the worker is inside a longer span (row 1063).
2. **It stats every file of the project at every new build** (`RenderCache::files`, `:915`), on the compose or
   prefetch thread: a slow disk would stall the picture. And the answer is the file as it lies on disk now, while the decoder
   still has the old one open (row 1062).
3. **`durable_hash` and `plan_hash` are two copies** of one walk over the plan ("a new field is hashed here too").

## D1. A source has an identity, and the cache holds it

`SourceIdent { key: u64, durable: bool }` (kt-cache), from a new `MediaSource::identity(&self) -> Option<SourceIdent>`
(default `None`):

| Source | Key | Durable |
|---|---|---|
| `FfmpegSource` | xxh3 of (file name, size, mtime in ns), read once at `from_inner`, on the opening lane | yes |
| `GeneratorSource` | the text `src_rev` hashes now (`video.rs:1519`) plus `GenFeed::key()`, taken at open and after every retune, on the worker | yes |
| `SyntheticSource`, `CheckerSource` | its pseudo-path | yes |
| `PlaceholderSource`, any source with no identity | a number from `NEXT_SERIAL` | no |

- The file NAME, not the path: the store lies in the project's folder (`project-cache-folder`) and must still hit
  after that folder is moved. The media id is hashed beside it (`put_src` hashes `source.0`). A failed stat gives a
  process number, never a guess.
- It is the file AS OPENED. `seek_and_reopen` (`kt-ffmpeg/src/source_impl.rs:1126`) reopens the decoder, not the file,
  so the identity stays true until the source is replaced.
- Not the content id of `project-cache-folder` (xxh3 of the whole file): it is known only after the index lane read
  the file, and the key would move in the middle of a session.

In `BurstCache`, under its one lock:
- `adopt(source, ident)`: drops the source's entries and remembered serials when the identity differs, sets it. Called
  by `MediaWorker::spawn_with_stats` (one place for `Player::new`, `add_media`, `replace_media`) and by the worker
  after an applied retune. The identity and the drop are one step.
- `insert_video_span` and `insert_audio` of a worker carry the identity they decoded from; one that is not the source's
  current identity is refused. `add_media` swaps the worker before it stops the old one (`player.rs:2220`), so a late
  insert of the old decoder is possible today.
- `get_video_leaf(source, q, idx) -> (Option<(Arc<VideoFrame>, u64)>, Option<SourceIdent>)`: the frame, its serial and
  the identity in one look.
- `invalidate_source` keeps the identity (it drops bytes; t2145 and t2978 use it to empty the cache). The two callers that
  mean "other content" use `adopt` (retune, replace) or `forget` (a generator gone, `player.rs:2286`). The `stale` list
  of a retune holds reel addresses, which have no entries: unchanged.

**The invariant that makes the key safe:** every entry of a source was decoded from the source's current identity. A
plan therefore takes the identity from the CACHE, never from the renderer. While a retune waits, the cache answers the
old identity and the old frames, and the composite goes under the old name; after it, both are new.

## D2. One key

`LayerSrc.leaf: u64` and `LayerBlend.leaf: u64`, beside `content`:
- a file or generator layer: mix(identity key, decode width, height); the identity is returned whether or not the
  frame is there;
- a stream layer: the kernel's key (it names the sources it reads, D3);
- a reel or a track stage: 0 — its plan is hashed whole;
- a source the cache knows no identity of (a private cache filled by `insert_video`: exports, thumbnails, strips, cell
  previews, hand-written plans of tests): **0**.

`kt_engine::moment_key(plan) -> u64` is `durable_hash`'s walk with one rule per layer: `leaf != 0` → hash the leaf and
nothing of `content`, of the presence or of the frame's size; `leaf == 0` → hash `content` and the input as `plan_hash`
does today. The second branch is needed: an engine outlives a cache, and two private caches give the same media id,
index and size to different files (t1457, t1574, t1607).

- kt-gpu: `plan_hash` goes; `plan_key(plan) = moment_key(plan) ^ forms_revision` (memory only: which form a run takes
  is the process's).
- Disk: `disk_key(plan) -> Option<DiskKey>` = `moment_key` and the rows as the curves gave them (a process
  cannot change its curve arithmetic, so the memory leaves them out), salted as `project-cache-folder` says; `None` when a leaf is 0 or not durable, so a placeholder's
  picture is never written. `FORMAT`, the decoder major, the kernels and `COMPOSITE_PATH_VERSION` are added by
  `project-cache-folder` (3.2) to `disk_key`, not to `moment_key`. `FramePlan` gains `durable: bool`, folded while the plan
  is built, sub-plans included.
- Deleted: `Leaves`, `with_leaves`, `note_leaf`, `probe_durable` (the probe plan is enough), `FileIds`,
  `RenderCache::files`, `restored_key`. A restored moment is kept under its present key. Where a composed one comes
  later it replaces it (`PresentMemo::insert` already does).
- `content` (the serial) stays for the GPU's upload cache and held pictures (`engine.rs:3591`): those are copies of
  bytes, so they are named by the bytes.

**Can an incomplete picture get in under the key?** A starved plan now has the key of the whole one. Only complete
frames are kept (`execute`, `present`, `ahead`, `keep`), and `present` of a starved plan whose key is held shows the kept
composite — wanted, it is what `show_kept` does.

## D3. The stream stage

`ClipStream::ident` (`video_stream.rs:684`) mixes the media id, pts, size and `src_rev`. It gives the same name to
another file relinked under the same media id, and `StreamMemo` and the front memo are cleared by nothing (row 1061).
- `ClipStream::new` takes a snapshot of the identities of the sources it can read (the event's, the event before's,
  the second inputs') from the cache; `ident` mixes the snapshot in place of `src_rev`. No identity known → `src_rev` as today.
- `with_frame` and `on_device` read by `get_video_leaf` and refuse a frame whose identity is not the snapshot's
  (a retune applied in between): the history is missing, as any absent frame.
- `key_only` receives the cache for the snapshot and reads no frame of it.
- Memo keys move once; no picture does.

**Found while building it (t4116):** the COMPILED SCHEDULE of a stage (`PlanCell`, one per stage and size) holds
the names of its positions too. A replaced file rebuilds no renderer (the plan is indifferent to the source), so the
stage kept the old file's names after the identities had changed, and read part of the old history. The cell is keyed
by the identities as well; two stand per size while a retune is between the renderer and the worker.

## D4. The held moments leave the window

- `size_frames_cache` is given the moments held in memory or on disk (`kept_now`), not `on_disk` alone
  (`player.rs:3583–3632`).
- **The cost of counting.** `loop_frame_bytes_except` walks the loop at every change of the held count; in the first
  lap the count changes every pass, 10 a second. It is counted again when the count has moved by a sixteenth of the
  loop, when it FELL, and at a lap's end (`sizing.held`); t4114 holds the number of counts.
- **An edit must not start a decode storm.** A kept moment counts only at the current versions; every edit, also of a
  title elsewhere or of a volume, moves them, and `recheck_shown` confirms 8 moments a pass on the compose thread.
  Today the window asks for the sources of all the unconfirmed ones; they are in the cache, so it costs nothing.
  With them evicted it would decode two seconds at every edit. So a moment waiting for its recheck counts as held for
  the window and the size until the recheck answers, at most 200 ms after the edit; the prefetch thread nudges the recheck
  itself (today only the bars ask), and `recheck_shown` goes from the playhead outward, not from the project's start.
  The on-screen moment is unaffected: the compose path asks its layers at `Playback`.
- The floor is the cache's base (512 MB): it is never shrunk below it, and what was last used stays there.

**What this costs, plainly.** An effect edited inside a loop that was playing from its kept composites used to find
every source decoded. Now the sources past the base are decoded again for the moments the edit changes. The frame
on screen is asked first. The measurement below reports it. Rejected: keep the paused moment's own sources decoded —
"a kept moment needs no decode" is held for a pause too (t2145, t2978).

## D5. What is NOT changed

Admission and eviction of the present memo, the ledger's order, the probe plan's cost (one lock more per layer is
replaced, none added), the bars' rule, the export (composes without the memo), the decode sizes (the owner's D7).

## D6. Class walk: where a key contains "is it decoded now"

| Place | Today | After |
|---|---|---|
| `present`, `execute`, `ahead`, `keep` (`plan_key`) | serial and presence | `moment_key` |
| `show_kept`, `kept_picture`, the recheck, render-ahead's look (`render_ahead.rs:365`) | probe key, 0 when evicted | the same key always |
| `restore_from_disk`, `scan`, the prerender `one` | probe + leaves + stat | `disk_key(plan)` |
| read-back memo of the bytes path | `plan_key` | follows |
| thumbnails, strips, posters, cell previews (private caches) | serials | serials (leaf 0) |
| reel previews' disk tier | reel revision + media fingerprint | no change: no serial |
| the GPU upload cache, held pictures | serial | serial: named by bytes |
| stream memo, front memo, carrier sources (`src_key`) | address + `src_rev` | + the identity (D3) |
| the kept final mix | scene hash, no serial | no change |
| `cached-final-render` (designed): an export's private cache | `Leaves` | it adopts the identities before it asks a key (noted there) |
| `simulated-elements` (designed): the atlas | "content serial / leaf" | the leaf; its task 2.x reads this change |
| `track-stream-stage` (designed): leaves per segment, `moment(k)` | `Leaves` | the track stream's `ident` mixes identities as D3; t3386 tests the key |
| trace `frame_plan`/`frame_exec` key, `trace loop` "the key changed" | moves on eviction | moves only on a real change |

## Measurement (the executor's, before and after on one machine, the same `heavy.kt`)

`kt-cli showcase --heavy`, footage in `/dev/shm`, `kt-cli preview heavy.kt 20 24 --size 640x360 --loop 60 --trace`, then
`trace memory`, `trace loop`. The audit's debug run is the known "before"; repeat it on a RELEASE build.

| | Before (debug, `cache-cuts`) | Accept |
|---|---|---|
| V-fps | 20.4 (21.1 before the cuts; 1.9 with kept frames left out) | not below before |
| incomplete composites | 455 | not above before |
| frames tenant after the third lap | 1158 MB | at or below the base (512 MB, 576 MB as its want is rounded) |
| decode requests per lap, laps 3+ of the moments kept | — (report) | 0 |
| `trace loop`: "the frame's key changed since the last lap" | 86–112 a lap in row 965's run | 0 with no edit |
| process peak | 3873 MB | lower by the frames' difference, report |

And three more: 1920×1080 linear 20–80 s (no change expected: nothing is held there — say so); an edit in the loop (one parameter
of the heavy event, ten times): time to the first complete picture, p50 and max, before and after; an unrelated
edit in the loop (a title outside it): decode requests in the next second, must be 0.
The idle guard (`idle` live scenario) and a pause with the mouse moving are unchanged.

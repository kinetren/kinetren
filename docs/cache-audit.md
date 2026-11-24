# Cache audit (kkt-26.11.22 D2)

The owner, answering Q55 (kkt-26.11.22 line 5): "у тебя что, после каждой цепочки есть отдельный кэш?!?!? а не
дохера ли мы данных сейчас в таком случае храним??"

This file lists every cache the program keeps today, in memory and on disk. For each one it says what the cache
holds, its key and its bound, the bytes measured on the heavy showcase, and whether a cache of FINAL preview
frames (the owner's D1) would make it redundant. The code was read at orch `e7b19346`; nothing in it was changed.

## The answer (for the owner)

**No: no cache sits after every chain.** The picture path keeps no result in the middle of a chain or of a layer.
The GPU executor keeps only FINAL composites: the present memo in video memory and the read-back memo for the
bytes path. The intermediate textures go back to a pool after each frame. The one exception is Datamosh, a
stream stage. It keeps the codec's own history (reference pictures, the encoder state) and the output of the
effects in front of it, because a codec cannot make the next picture without the previous ones. The
`layers/` kind that `persistent-cache` had planned on disk is withdrawn by D1.

**Yes, we hold too much in one place, and it is not effect results.** It is the decoded SOURCE frames, the
burst cache. On the heavy showcase after a minute of play at a 1920×1080 preview:
- the decoded frames held **7.9 GB**;
- everything else held about 0.3 GB;
- the process's resident memory was 10.7 GB.

Their bound follows the look-ahead. The program decodes two seconds ahead, and on this dense edit that is a
4 GB window: many layers a moment, each decoded at 2578×1080 because a zooming chain asks the larger size. The
cache asks the memory ledger for twice the window, so it took 8 GB. At a 640×360 preview the same minute held
1.8 GB of decoded frames.
With Datamosh behind every styled cut, the stream stage's history and its device buffers added 0.6 GB (251 MB
of history, its bound full, and 379 MB of compute buffers).

**What to cut:** the decoded frames' window, first of all. A moment whose final frame the program already
holds needs none of its source frames, so with D1's store those frames need not be decoded or kept. The list is
at the end.

## How it was measured

- **Binary:** the debug `kt-cli` of orch at `a2018bf3` (2026-11-22). No build was made for this audit.
- **Project:** `kt-cli showcase heavy.kt --sources <showcase-media> --heavy`: 9 sources, 1,357 events,
  13 transitions, 104 s, under Slowed, VHS Tape and Camera ISP. The footage was in `/dev/shm`.
  `--mosh-behind` puts Datamosh behind every styled cut; that was a third run.
- **Run:** `kt-cli preview <project> 20 80 [--size 640x360] --render-cache DIR --trace DIR`, a minute of play
  against a wall-clock device, on llvmpipe (no card in this machine).
- **Memory:** each kind's bytes are the ledger's `mem_used` trace records, written once a busy second, and the
  resident memory is `mem_sample`. Disk is `du` of the folders.
- **Debug caveat:** a debug build composes slowly, so few moments complete. At 1080 no frame of the minute
  completed (V-fps 0.0); at 360, 16 did. So the result caches (present memo, disk prerender) are
  UNDER-measured here and the source caches are not. Their bounds are given beside them. Debug judges bytes,
  not speed.

| Run | Preview | Resident (peak) | Ledger sum | Frames shown |
|---|---|---|---|---|
| heavy | 1920×1080 | 10.7 GB (11.1 GB by `time -v`) | 8.2 GB | 0 of 1500 |
| heavy | 640×360 | 5.6 GB (5.9 GB) | 2.1 GB | 16 of 1500 |
| heavy `--mosh-behind` | 640×360 | 7.1 GB (7.5 GB) | 2.8 GB | 3 of 1500 |

Where the resident memory exceeds the ledger's sum, the rest is the decoders inside FFmpeg (eight open
pictures each), llvmpipe's own memory and the code. No cache names it.

## Every cache

**Holds** says where in the frame's path the content sits:
- **source** — decoded input, before any effect;
- **stage** — inside a chain;
- **final** — the composite or the heard mix;
- **work** — reusable scratch that keeps no content between frames.

**Final-frame store** answers whether D1's store (final preview frames on disk, the present memo in memory)
makes the cache redundant.

### Memory: picture

| Cache | Where | Holds | Key | Bound | 1080 | 360 | mosh 360 | Final-frame store |
|---|---|---|---|---|---|---|---|---|
| Burst cache, video | `kt-cache` `BurstCache` | source: decoded frames at the decode size | (source, quality, frame index) | 512 MB base + the ledger's `Frames` grant; it asks twice the 2 s look-ahead window (`frames_cache_want`, `prefetch.rs:194`) | **7.9 GB** | **1.8 GB** | 1.8 GB | **partly**: a held moment needs no decode. The window already skips moments the present memo holds (`kept_moments`, `player.rs:3569`); it does not skip moments on disk, and it sizes its want before that skip |
| Upload cache | `kt-gpu` `UploadCache` | source: the same frames as textures | content serial + size | 256 MB an engine | 140 MB | 264 MB | 312 MB | partly, as above. On llvmpipe "video memory" is RAM, so this is a second copy of the burst cache's bytes |
| Present memo (kept composites) | `kt-gpu` `PresentMemo` | **final** | plan key | 512 MB + the `Kept` grant (a loop: every moment of it + an eighth) | 16 MB | 18 MB | 4 MB | it IS the final-frame cache in memory |
| Read-back memo | `kt-gpu` `ReadbackCache` | **final**, as bytes | plan hash | 64 MB | 0 | 1 MB | 0 | **yes** for the preview on a card (the present memo holds the same moments); still needed for the bytes path (GL preview off the display, strips, posters) |
| Transient pool | `kt-gpu` `TexturePool` | work: free intermediate textures | size | 128 MB an engine | 96 MB | 35 MB | 37 MB | no: scratch, nothing kept between frames |
| Compute pool, spares | `kt-gpu` `compute.rs`, `held.rs` | work: free buffers; with Datamosh also the stage's buffers kept on the device (the live tier) | size, usage | 1 GB, 192 MB | 0 | 0 | **379 MB** (Datamosh's buffers kept on the compute device) | no for the scratch; the stage's kept buffers as the stream memo below |
| Stream memo | `kt-plugin-api` `StreamMemo` (`video_stream::stream_memo`) | **stage**: a stream stage's history (Datamosh's reference pictures, the clean encoder's state) | the kernel's key of what determines the entry | 256 MB + the `History` grant; a live tier of 128 MB on the compute device | 0 | 0 | **251 MB** (of a 320 MB bound: full) | **for playback, yes**: a final frame needs no history. A moment not yet held still needs the history from its cut, so the memo stays as the computing state, not as a store of results |
| Front memo | `video_stream::front_memo` | **stage**: the output of the effects in front of a stream stage | stream address | 64 MB + grant (counted as `History`) | 0 | 0 | (in history) | yes for playback, as the stream memo |
| Picker and strip engines' caches | `fx_thumbs.rs`, `reel_preview.rs` | source: a job's own decoded frames | as the burst cache | 64 MB / 16 MB a job, dropped with it | — | — | — | no: offline jobs, gone at rest |

### Memory: sound

| Cache | Where | Holds | Key | Bound | Measured | Final-frame store |
|---|---|---|---|---|---|---|
| Burst cache, audio | `BurstCache` audio pool | source: decoded audio | (source, burst index) | 64 MB base | in the `Frames` sum (≈ 20 MB) | no (a picture store holds no sound); the kept mix spares it where it holds |
| Kept final mix | `kt-engine` `MixStore` | **final**: the heard mix per burst | (context, burst) by the scene hash | 64 MB + the `Mix` grant (a loop: + an eighth) | 21 MB (a minute is 22 MB) | it IS the sound's final cache |
| Effect state | `kt-engine` `fxstore` | the effects' running state | clip id | the chains | — | no: it is the effects' state (invariant 3), not a cache |

### Memory: interface and small memos

| Cache | Holds | Bound | Note |
|---|---|---|---|
| Timeline thumbnails (`thumbs.rs`) | JPEG-decoded 320×180 textures | 384 (≈ 88 MB at the ceiling) | source frames for the filmstrip; not touched by D1 |
| Reel previews (`reel_preview.rs`) | a reel's strip, frames and peaks | 6 reels × 48 frames (≈ 66 MB) | **final** of a reel at thumbnail size |
| Effect cells (`fx_thumbs.rs`) | 160×90 cells | 256 (≈ 14 MB) | **final** of one effect on one frame |
| `BuildMemo` (`incremental.rs`) | derived events per edit | the events of the current build | CPU structures, no picture |
| `ChainMemo` (`video.rs:181`) | a chain resolved at the last few ticks | a few entries | no picture |
| UI caches (coverage, warp maps, names, bidi, graphs) | per revision | one each | no picture |
| Plugin memos (`dsp::Memo`, the tape's `DropCache`) | last values of pure functions | scalars | no picture |

### Disk

| Cache | Where today | Holds | Key | Bound | Measured | Final-frame store |
|---|---|---|---|---|---|---|
| Disk prerender (`render_cache.rs`) | `<data>/render-cache/v2-<build>/<device>/` | **final** composites, hardware-encoded segments | `durable_hash` + build salt | 2 GiB for all projects | 12 KB after each run: it writes only at rest, and a debug run never rests. render-cache-disk measured 30 MB for 54 heavy moments (≈ 0.55 MB a moment) | it IS the final-frame store; `project-cache-folder` moves it beside the project |
| Media index (sidecar) | `<project>.ktindex.db` (SQLite) | source facts: peaks, packet index, thumbnails, posters, loudness, sync | media path | none | 1.1 MB after the runs | no: waveforms and thumbnails are drawn from it (invariant 8) |
| Reel previews (disk tier) | the same sidecar | **final** of a reel at thumbnail size | (reel, revision, media fingerprint) | 48 frames a reel | — | no: thumbnail size |
| Compiled pipelines, pass times | `<data>/shader-cache/` | the driver's pipeline blobs, measured pass times | adapter, driver | 64 MB, 8192 lines | — | no: per machine, not results |
| Planned: Datamosh history on disk (`durable-history-and-mix`) | `kinetren-cache/results/…/history/` | **stage**: LZ4 of exact stream entries | durable key | the folder's cycle | not built | partly: a moment held final needs no history. The history serves only a seek or an edit into a moment not yet held |
| Planned: mix runs (`durable-history-and-mix`) | `kinetren-cache/mix/` | **final** sound | durable key | the folder's cycle | not built | no (sound) |
| Withdrawn: layer results (`persistent-cache` `layers/`) | — | stage | — | — | — | removed by D1 |

## What a final-frame store makes redundant, and what to cut

In order of bytes saved.

1. **The decoded frames' window counts moments the program already holds final.** It skips moments the present
   memo holds, not moments on disk, and it sizes its want (twice a 2 s window, 7.9 GB here) before the skip.
   - Cut: the window skips every moment held final, in memory or on disk, and sizes its want after the skip.
   - A held stretch then decodes nothing and keeps nothing; the bytes go where the person has not been yet.
   - This keeps the owner's J4 decision (the cache may grow when the window needs it). It changes what the
     window counts, not the cap.
2. **The decode size above the preview's.** At a 1920×1080 preview the sources decoded at 2578×1080. A zooming
   chain asks for the larger size (R.46), and every layer's frame then costs 1.8× its pixels in the window.
   - A held final frame needs neither, so cut 1 removes most of it.
   - A cap of the decode step at the preview's size plus what the visible zoom needs is a second, separate
     question. It is not cut here: R.46's step is held by tests (t309, t632).
3. **The upload cache on a software rasterizer.** On llvmpipe its 256 MB an engine (140–264 MB measured) is a
   second copy of the burst cache in RAM.
   - Cut: size it by the frame being resolved and the next, on a device of type CPU only.
   - On a card it saves a re-upload and stays.
4. **The read-back memo on the display path.** It is redundant where the present memo holds the same moment.
   - Cut: the preview on a card does not fill it.
   - It stays for the bytes path (GL preview off the display, reel strips, posters, the slow route).
   - The saving is small (64 MB bound); it is listed because it is a second final-frame cache of the same moments.
5. **Datamosh history kept on disk (`durable-history-and-mix`, planned).** With the final frames on disk, a
   played stretch needs no history. The history is needed again only when the person seeks or edits into a
   moment not held.
   - Not cut: the owner ordered it built (A14, "build now").
   - It is a question for him: keep the disk history, or keep only the final frames and recompute the history
     from the cut on a miss. Recomputing costs the walk from the cut.
6. **Keep:**
   - the present memo, the kept final mix, the disk prerender: the final caches D1 asks for;
   - the stream and front memos in memory: the computing state of a stage that cannot run without its past;
   - the media index, the thumbnails, the reel and cell previews: thumbnail-sized, and the timeline draws them;
   - the scratch pools: no content kept.

## Not measured here

- The kept composites and the disk prerender at their working size: a release build on this machine, or the
  owner's trace, would fill them. Their bounds are 512 MB plus the loop's grant, and 2 GiB.
- Memory on a card: the present memo and the upload cache sit in video memory there, not in RAM. On llvmpipe
  they are RAM.

## Cuts made (branch `cache-cuts`, 2026-11-23)

Measured with the debug `kt-cli preview` of orch `d613b228` (before) and of the branch (after), one after
the other on the same `heavy.kt` (`showcase --heavy`, footage in `/dev/shm`), llvmpipe; the ledger's
`mem_used` peaks and `mem_sample` by `kt-cli trace … memory`.

| Run | Frames | Upload | Transient | Caches | Process peak | V-fps |
|---|---|---|---|---|---|---|
| 1920×1080, 20–80 s linear, before | 7919 MB | 140 MB | 96 MB | 8192 MB | 10710 MB | 0.0 |
| same, after | 7917 MB | 102 MB | 44 MB | 8092 MB | 10615 MB | 0.0 |
| 640×360, loop 20–24 s for 60 s, before | 1158 MB | 265 MB | 32 MB | 1546 MB | 4194 MB | 21.1 |
| same, after | 1158 MB | 28 MB | 32 MB | 1309 MB | 3873 MB | 20.4 |

- **Cut 1** (the decode window skips moments held on disk, the want counted without them): no change in
  these runs. A debug build at 1080 completes no moment, and the disk prerender writes only at rest, so no
  moment was on disk. It acts where the prerender holds moments: the owner's machine after a rest.
  The kept composites in MEMORY are not cut: their key is the decoded frames' content serials, and leaving
  their frames out of the want lost them (found-defects 965: 21 V-fps fell to 2). The 7.9 GB window on a
  1080 linear play is the frames of moments nothing holds yet; this cut does not touch it.
- **Cut 3** (a CPU device keeps two frames of uploads): 265 → 28 MB on the loop, 140 → 102 MB on the
  linear run.
- **Cut 4** (the window's engine on a card fills no read-back memo): not measurable here (llvmpipe is a CPU
  device, where the memo stays); its bound was 64 MB.

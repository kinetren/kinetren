# Design: decode-waste

Found-defects row 712 (render-speed, kkt-26.11.15 N2–N4: the preview, the final render and the processor use
are too slow). The owner's standing word of 16.11: "делай другие сам без меня".

## D0. What was measured

Release build, this machine (i9-13900KF, 32 threads, llvmpipe), the heavy showcase (`kt-cli showcase …
--heavy`, 1,357 events in 104 s, seven VP9/VP8 files of 1080×1080 to 3840×2160) and its footage in
`/dev/shm`. Load 18–60 from sibling sessions throughout: the counts below are a function of the requests
(the export's exactly, the preview's within a few per cent), the wall times are not comparable between runs.

Every decoder run was logged (file, keys, decoder, cursor before, reopens, pictures) and the request
sequences were replayed through a cost model of the decoder rules (a run reached by decoding on costs the
frames between; otherwise the frames from the keyframe before it). The model gave 12,163 pictures and 157
reopens for the logged run's 12,289 and 157, so policies were compared on it before they were written.

| stretch | what | pictures decoded | used (to RGBA) | reopens | per used |
|---|---|---|---|---|---|
| 20–30 s, 640×360, export | before | 12,289 | 1,924 | 157 | 6.4 |
| | after | 10,214 | 1,924 | 120 | 5.3 |
| 20–30 s, 640×360, preview (two runs each) | before | 10,309 / 10,280 | 1,878 / 1,899 | 136 / 135 | 5.5 |
| | after | 9,444 / 9,419 | 1,939 / 1,916 | 117 / 115 | 4.9 |
| whole project, 640×360, export | before | 117,679 | 21,187 | 1,536 | 5.6 |
| | after | AFTER_ALL | 21,187 | AFTER_ALL_R | AFTER_ALL_P |

The export's file is byte for byte the same before and after (md5 of the 20–30 s piece and of the whole
project). The export's wall did not move here (51.5 / 49.6 s at load ~15; 117.0 / 116.7 s side by side at
load ~60): on llvmpipe the device bounds it (gpu wait 370 % of the wall, decode 60 % busy). The decode
thread's busy time is what moved; on a machine whose device keeps up, decode is in front of the frame.
Peak RSS of the piece's export 4.03 → 4.35 GB (the smaller files' extra decoders).

## D1. The mechanism

Two causes, one of them the material's:

1. **A cut's lead-in.** The showcase footage has a keyframe every 2.6–4.9 s (VP8 Jupiter 4.3 s, cobe 4.3 s);
   the heavy edit cuts every 0.3–0.5 s to a new place of a file. A decoder reaches a place in the middle of a
   GOP only by decoding from the keyframe before it: about half a GOP, 50–70 pictures, for a cut that uses
   10–15. Every seek landed on the keyframe right before its target (checked against the packet index for
   every reopen: 11,273 decoded where the walks from the keyframes were 11,015). The union of all walks of
   the 20–30 s stretch — each region of each file decoded once — is 8,609 pictures, 4.5 per used: no
   decoder order inside the stretch gets under it.
2. **Decoders fighting over places of one file** — the part this change removes:
   - keys of one request closer than 2 s (`RUN_GAP`) were ONE run on one decoder. The montage and its
     picture-in-picture, or two layers of one file, ask places 1–2 s apart in every chunk: the decoder walked
     from the first place to the second (a 4K picture per frame between) and the next chunk sought back to
     the first and walked its GOP again;
   - three places of one file read together had two decoders (`MAX_CURSORS`), so one of them was sent back
     to a keyframe every chunk;
   - when no decoder reached a run, the least recently used went — often the one standing right behind the
     NEXT run of the same request, whose reopen then sent another away (a cascade);
   - a span at ONE size (`read_video_span`: the reel strip, the effect thumbnails, a stream stage's
     history, the export's retry of a lost key, the worker's native-size spans) sought only before its first
     key and decoded every picture up to a far one (the D8 fix of export-throughput was in the
     multi-size path only), and a frame (`read_video_frame`: the filmstrip thumbnails) always used the
     first decoder, wherever it stood.

Across the whole project the union of walks is 31,469 pictures for 117,679 decoded: the same GOPs are
walked again by cuts tens of seconds apart on the timeline. Keeping the walked frames until those cuts
come would hold thousands of pictures (the model: a horizon of 1,000 requests, 11,566 frames held at once
for 90,770 decoded) — see "Not done".

## D2. A run goes on while its decoder is the nearest

`FfmpegSource::read_runs` is the one body of every video read. A run starts at the decoder `cursor_for`
gives its first key and takes the next key while `goes_on`: the run's decoder, standing at the run's last
key (`kt_ffmpeg::Source::video_reach_from`, the same reach rule as `video_reach`), reaches it, and no other
open decoder already stands nearer. Keys of the same moment at other sizes stay in the run (D7 of
export-throughput: one picture, several conversions). Two places of one file in one chunk then keep a
decoder each: the first chunk walks from one to the other once, the next opens a decoder for the place
left behind, and from then on each decoder decodes on.

Rejected: a smaller fixed gap. Any constant is wrong for some speed (a 4× event's keys are 4 frames apart,
a slowed one's a fraction); "the nearest decoder" is the question the gap approximated.

Rejected: "seek forward when a keyframe lies between the cursor and the target" (decoding on only within
a GOP). On the model it saved 2.5 % of the whole project's pictures and cost 20 % more reopens, each a
decoder rebuild with its threads; not worth a change of the forward rule all reads share.

## D3. How many decoders, which one goes

- **A fresh decoder only when none is free to go**: every open one stands AHEAD of the key (a place behind
  the others), has read a key of this request (a place this request is reading), or is the one a later key
  of this request would be given. Otherwise the free one standing longest unused goes — for a key ahead of
  it that is a seek forward, so a sweep (the filmstrip, one frame per request; a jump far ahead) keeps one
  decoder. Without this rule `read_video_frame` through the choice opened up to four decoders per file for
  a filmstrip's keys.
- **Which one goes**: the least recently used of those no later key of this request would be given
  (`nearest` for every later key). "Reaches by decoding on" was too wide — a decoder 2 s behind a later key
  "reached" it, and every decoder looked wanted.
- **Decoders per file by its picture** (`cursors_for_size`): two decoders of a 4K picture's pixels make the
  budget, at least two, at most four (`MAX_CURSORS`). A decoder's frame pool (its references and its
  frame threads' pictures) grows with the picture, so a 1080p file's four decoders hold what a 4K file's
  two do. On the model a third decoder for every file gave 9,726 pictures (RSS +0.8 GB), the size rule
  10,061 (RSS +0.3 GB), four for every file 9,685; the misses were mostly in the small files (Jupiter
  1080×1920, cobe 1080×1080, coffee-run 1920×804 decoded 1.5–1.9× their walks' union, the 4K files 1.0–1.3×).

## D4. One rule for every read

`read_video_frame` and `read_video_span` go through `read_runs` with their one size (`None` is the
native size). Every key goes through `prepare_for` (`read_video_span_sizes`), so a one-size span seeks per
key as the multi-size one did since export-throughput D8. The frame a key gets does not depend on which
decoder read it (`frame_for`: the greatest pts at or before the target; t1453–t1455 and the new tests
compare with a fresh source), so the preview, the export and every offline path keep their bytes
(invariant 11; the export's md5 above).

## Class walk: every reader of a file and every caller of what moved

Found by grep of `read_video_frame`, `read_video_span`, `read_video_span_qualities`, `cursor_for`,
`MAX_CURSORS`, `RUN_GAP`, `video_reach`, `decode_debug_stats` across `src/`.

| path | reads through | before | now |
|---|---|---|---|
| preview playback, near/far prefetch, cut lead keys (`MediaWorker::worker_loop`, kt-engine `worker.rs`) | `read_video_span_qualities`; native size `read_video_span` | runs by gap, two decoders; the native span on the first decoder, sought once | D2–D4 |
| loop fill (`loop_fill.rs`) | composes only what the cache holds, asks no decode | — | unchanged |
| export, serial and pipelined (`chunk_requests` → `decode_requests`, `export.rs`) | `read_video_span_qualities` per source per chunk | as above | D2–D3 |
| export retry of a key a span lost (`decode_requests`) | `read_video_span` of one key | the first decoder, wherever it stood | the nearest decoder |
| stream stage pre-roll (`stream_preroll`: export, reel strip, poster) | `read_video_span` per (source, size) batch | sought once per batch | per key |
| filmstrip and bin thumbnails (kt-app `thumbs.rs`) | `read_video_frame` per key | the first decoder | D3: a forward sweep keeps one decoder (t2884) |
| effect thumbnails (`fx_thumbs.rs`), reel strip (`reel_preview.rs`) | `read_video_span` of the missing keys | sought once | per key |
| kt-cli `probe` frame cost, `stream-cost`, `gpu-share`, the decode bench | `read_video_span` | sought once | per key |
| analysis (LUFS, sync, ducking), peaks | audio only: `read_audio_burst` on its own demuxer | — | unchanged |
| generators, placeholder, synthetic and checker sources | their own `MediaSource` | — | unchanged (the trait's defaults) |

- **Invariant 5**: every decoder of a file lives inside its `FfmpegSource`, opened and used on the thread
  that owns the source (a `MediaWorker`, an export's decode thread, an offline job's own source). Nothing
  here shares one; live playback still owns no source.
- **The MediaWorker's own cursor** (`cursor_video` in `pick_next`, anti ping-pong) is one number per media
  while the source holds up to four decoders. It orders the queue (forward of the last key read first); the
  source then gives each run its decoder. Not changed: the preview's count fell with the source's rule alone,
  and a queue order per decoder would need the source's cursors in the engine (the trait does not expose
  them).
- **Memory**: decoders are not in the memory ledger (`kt_time::mem`); their bound is `cursors_for_size`.
  The export's peak RSS rose 8 % on the showcase piece.
- **Diagnostics**: `decode_debug_stats` sums every decoder of the file (unchanged); `kt-ffmpeg::iostats`
  counts every decoder's pictures and reopens (unchanged) — the numbers above are from them.
- **Docs that held the old rule**: kt-media `CLAUDE.md` ("keys more than 2 s apart start a new run",
  "fewer than `MAX_CURSORS` (2)") and the media-index design's export-throughput entry — updated.

## Not done (found-defects)

- **A GOP walked again by cuts far apart on the timeline** (whole project: 31,469 pictures would do,
  ~AFTER_ALL are decoded). Removing it needs the walked pictures kept until their cut comes — a
  decoded-picture tier, or intra-frame proxies of the footage ("optimized media" in Resolve, proxies in
  Premiere and Vegas). New row.
- **The MediaWorker's queue order knows one cursor per media** (above). Not a measured cost.

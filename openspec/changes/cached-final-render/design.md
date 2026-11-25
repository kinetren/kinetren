# Design: cached-final-render

> IMPLEMENTATION-READY (spec-engine, revision 3, 2026-11-24), read against orch `229c694b` (durable-moment-key, project-cache-folder,
> render-result-check merged). Code is named by function and file.

## Context (what exists)

- **The one key** is `moment_key(plan)` (kt-engine `moment_key.rs`); the disk tier's is `disk_key(plan, made) = DiskKey { key: FORMAT + moment_key +
  put_rows, salt: Made::salt }`. The salt is what makes the pixels (the decoder's major, `COMPOSITE_PATH_VERSION`, the plugins that run), not the build.
- **A frame is named by its source's identity**, which the CACHE holds (`BurstCache::adopt`). An export's private cache adopts nothing, so its leaves have no
  names and its plans no `disk_key`.
- **The moment key is not a key of the picture** (this reading): `put_plan` hashes the timeline tick; `fx_hash` (`fx_chain_hash`) the event's start, length and `source_in`;
  `u` the progress over the length; a reel's subtree its whole `content_hash`, a track stage its whole `revision`. So a trim of an event's tail renames every frame of the event, and a
  ripple everything after it. For the preview that costs a recomposition (found-defects 1069); for X1 ("мелкая оплошность (склейка кривая обнаружена)") it is
  the whole feature: the two renders would share only what lies before the fix, and t3328's "another position" could never occur.
- **The export**: `export_pipeline::run` (chunks over the range, `decode_chunk`, the compose thread, `write_all`), the serial reference in `export.rs`; `write_all` counts a frame where
  the sink took it and `ExportStats` carries announced and written (render-result-check). `Muxer` (kt-ffmpeg) has one video encoder for the file, `drain` writes its packets straight to
  `av_interleaved_write_frame`.
- **The preview's tier**: `render_cache::DiskStore::get(DiskKey)`; `RenderCache` of the player; its readers live on `render-cache-read`.

## D1. Every offline cache adopts its sources' names (found-defects 1068; needed here, right by itself)

`kt_engine::adopt_sources(&BurstCache, &HashMap<MediaId, Box<dyn MediaSource>>)`: `adopt(source_id(m), src.identity().unwrap_or_else(SourceIdent::once))`. Called right after the
cache is made, in `export.rs` (serial), `export_pipeline::run`, `reel_preview.rs` (both), `fx_thumbs.rs` (as it opens each source), kt-cli `framecost`, `streamcost`, `gpushare`. The renderer
gets `set_source_idents(cache.ident_reader())`, as the player's. No frame's bytes change: an identity is part of names only.

It closes row 1068: `ClipStream::ident` without an identity is (media id, pts, size, front), and the FRONT memo is the process's: an export of another project
whose media has the same id, or of the same one after its file was replaced, reads the former's pictures through a Datamosh with a slot in front.

## D2. The picture key

`moment_key::picture_key(plan) -> Option<u64>`: what the kernels of a frame are GIVEN, with no place in it. `None` unless every leaf is named and durable. Hashed, in this order:

1. `out_w`, `out_h`; per layer its role, `alpha_k`, `scale_alpha`, fade (`k`, mix, incoming) or dissolve (`k`, mix): `put_mix` as today;
2. per source: the leaf's key and `cache_idx` (the source's own time), the blend pair the same way; the RESOLVED chain (each slot's `Debug`: the
   values the curves gave), the wiring, the reads recursively, the evaluated masks, `src_scale`, the lead, `cam3d`, `blend_mode`, the matte (track-stream-stage);
3. a subtree: `blend`, `nest_scale`, its plan recursively. Not its `seq`, `content_hash`, `inner_t`;
4. `u`, only where a slot of the chain reads progress; the plan's `t`, only where a slot or a transition reads time.

Not hashed: `fx_hash`, the tick, start/length/`source_in`: what they decide is in 2 and 4 already.

**Who reads time or progress** is a fact of the plugin: `Plugin::reads_time()` and `Plugin::reads_progress()`, both `true` by default (safe); a plugin whose kernel's parameters do not use `ctx.burst_index` / `u`
answers `false`. Geometry and the colour families are `false`; noise, the camera, the tape, VHS stay `true` (the files that name `burst_index` today: `fx-video-camera`, `fx-video-stylize`,
`fx-video-tape`). t3335 holds it: for every plugin that answers `false` the cooked parameters, taps and geometry primitives are byte-equal at three ticks and three progresses over
the passport's corners. `CamMotion` (core) reads progress. A stream stage's key holds its absolute positions (its losses are seeded by them): it does not move, and that is said.

`moment_key` is not touched: the preview's and the disk tier's keys stay.

## D3. The entry of a frame, and the fingerprint of a file

**A frame**: `FrameId { picture: u64, salt: u64 }` (16 bytes), the salt `Made::salt(plan)`.

**A file**: one xxh3 over a text of `key=value` lines (the text is kept in the index's first SEI too, so a mismatch names its line): container, codec, encoder name, `avcodec` major.minor, profile and
level as opened, pixel format, size, frame rate, rate mode and its number, maximum, `keyint_tenths`, B-frames, speed, depth, matrix, range, the composing device's class (the string
the disk tier's folder is of), `COMPOSITE_PATH_VERSION` (it holds `yuv.rs` and the composite path), and `cached=1`. Not the program's build: an update that changed no pixel keeps
its renders; a changed kernel is in the salt of the frames it runs in. Not `threads`: it decides no compatibility.

## D4. The GOP plan: a function of the timeline

Before anything is decoded, `gop_plan(video, cache, range, q) -> Vec<Gop { first, len, ids: Vec<FrameId> }>` asks every frame's `probe_plan` (keys without frames: a stream stage by `key_only`).
A GOP starts at frame 0, at every frame whose `shape(plan)` differs from the frame's before, and every `keyint` frames FROM that start. `shape` hashes the layers' roles, sources, reels, the
plugin ids of the chains, the wiring and which transition is running: not values. Counting from the shape's start, not from 0, keeps the GOPs of a shot that moved.

Cost: a plan per frame; measured on the showcase in the trace (a `Job` span `cached_plan`), done on the export's thread, cancellable between frames. A frame without a picture key (a placeholder,
a leaf without a name) has `FrameId::NONE`: its GOP is never copied and never offered.

**The encoder under this mode**: `Muxer::set_gops(starts)`: the frame at a start gets `pict_type = I` and the encoder its forced-IDR option (`forced-idr=1` for libx264, libx265, nvenc;
`forced_idr=1` for qsv and amf; VAAPI takes an I picture as IDR); `gop_size` becomes the longest planned GOP, scene cut off (`sc_threshold=0` / `no-scenecut=1`), closed GOPs (libx265 `open-gop=0` in
`x265-params`). An encoder that gives no IDR at a planned start is found by the writer (D5) and ends the mode for this file with the reason, not the render.

## D5. The writer of a cached render: GOPs, not packets

In this mode `drain` hands the video packets to a `GopWriter` (kt-ffmpeg, pure over bytes but for the mux call) that holds one GOP:
1. NAL units are read from Annex B (what every encoder of ours gives) by `kt_ffmpeg::nal` (new, pure: split, length-prefix ↔ Annex B, emulation prevention, types per codec);
2. a planned start without SPS/PPS (VPS) in its IDR gets them from the encoder's current extradata;
3. when the GOP is whole (the next planned start's packet, or the end) its hash is xxh3 over its NAL units except our SEI, in decode order; the SEI is built and put into the IDR's access unit after the
   parameter sets (H.264 type 6 payload 5; HEVC prefix SEI, type 39);
4. the packets go to the muxer as today.

Sample entries: H.264 `avc3`, HEVC `hev1` in MP4/MOV (the parameter sets stand in the stream). **MKV is the default container of a cached render**; MP4/MOV are offered
with the note that Apple's players refuse `hev1` (`format.rs` writes `hvc1` without this mode, D8 of render-settings). At most one GOP of packets is held: 2 s.

**The SEI** (version 1): our UUID, `u8` version, `u32` first frame, `u16` frames, frames × 16 bytes, `u64` hash; in the first GOP also the fingerprint's text. About 0.8 KB for 2 s at 25 fps. x264's own SEI has another UUID and is
never touched.

## D6. Where earlier renders are found

`renders/index.tsv` in the project's cache folder (a new `cache_folder::Tenant::Render`, appended): path, size, mtime, fingerprint; and `renders/<hash of path>.gops`, the file's GOP table as written, by `Folder::write`
(versioned, checksummed). A listed file whose size or mtime differs is not a candidate. The SEI is the truth for any other file: "Look in folder…" seeks each key frame by the container's own index and reads ONE
packet (no decode, no full read), and writes the table. A table evicted from the folder is read from the SEIs again.

## D7. Matching and copying

`reuse(gops, candidates) -> Vec<Span>`, pure: spans `Render(first..end)` and `Copy { file, their_first, first..end }`. A planned GOP is a copy when ONE candidate GOP with an equal fingerprint has the same number of frames
and the same ids in order. Candidates are ordered by fingerprint text, then by the table's hash: first match wins, never time or disk order. Neighbouring copies from
one file merge.

**Verification**: before the first frame is composed, every GOP to be copied is read whole and its hash compared (a `Job` span `cached_verify`); one that differs becomes a `Render` span and the summary says so.

**Copying**: `Muxer::copy_gop(&CopiedGop)`: drop the old SEI, write ours with the NEW first frame; pts/dts in frames: `pts' = pts − their_first + first`, the same for dts (the source's dts are decode order minus the delay of its B-frames; the
fingerprint has the same B-frames, so the delay is the same and dts stay ascending across spans: the writer asserts it and errs otherwise). Before a copy the encoder is
flushed (`send_frame(NULL)`, drained to EOF); after it a fresh one is opened by `open_encoder(spec, …)`, the part of `Muxer::open` that makes the context, taken out; a VAAPI context keeps its device and frames.
The reopened encoder's pts continue on the file's grid.

## D8. The pipeline

- Chunks are made over the `Render` spans only, with the frames' real indices; `decode_chunk`, the look-ahead and the compose thread do not change. A stream stage is pre-rolled at the first frame of
  EVERY rendered span (today: of the export).
- `write_all` iterates frames; at a `Copy` span it calls `sink.copy_span(span, gop)` per GOP: `frames += gop.len; gate.set(end); video_done(end)`, then the audio up to the GOP's end by its fixed rule. The counts of render-result-check
  are what the file took: a copied frame is counted when the muxer took its GOP's packets, and a failed copy ends the render short and SAID (t3998's path).
- The serial export does the same (t1451's law gains a cached pair).
- `export-into-surface`: a rendered span goes by surface as any; a span boundary flushes the encoder and the pool is the same frames context.
- The audio is rendered whole, as always.

## D9. The window and kt-cli

A section "Cached render" of the render setup: the switch; the candidates with their verdict ("12 of 60 s", "other bitrate: 8000 / 12000", "file changed", "other device"); the strip over the range (a copied GOP is drawn bright only
after D7 verified it is a candidate GOP: what it shows IS copied, invariant 4's honesty); the estimate. Plan and match run on the window's job thread, cancelled by any change; nothing at rest. The finished summary: "copied N of M s from F files". "Remove the render index": a
copy of a file without our SEIs, packets re-muxed, no decode (`nal` drops them). `kt-cli export --cached [--look DIR]`, `kt-cli strip-index <file> <out>`.

Offered for H.264 and HEVC; the section says "needs H.264 or H.265" otherwise.

## D10. The draft render (the owner's Q20 b), and why the final never reads the prerender (found-defects 773)

The tier holds the preview's size and quality, lossy, possibly in a run form the export does not use. A final render made of it would be worse than its own frames and not byte-stable. The draft is another mode:
- `DraftSource::get(t) -> Option<Picture>`: the plan of `t` at the PREVIEW's quality (a renderer of the export's project and its adopted names), `disk_key`, `RenderCache::read` of the player in the program (its own reader thread); kt-cli opens a
  `DiskStore` of the folder for the device class it would preview on (read only: no write, no eviction).
- A held moment is scaled to the draft's size (`fit_into`); every other moment is composed at the preview's quality and scaled the same way.
- The size is the preview's by default; the project's size with the warning "frames are upscaled".
- Title "Draft — Kinetren", metadata `kinetren_draft=1` (both containers take it as a tag); no index; never a candidate. Its bytes are not stable run to run, and the window says so. The audio as in any export.
- The summary and `kt-cli export --draft` say the seconds taken.

## D11. Invariant 11

A cached render's bytes are a function of (project, settings, the verified files). The same inputs give the same bytes (t3330), for a render of ONE composing device (several software devices give frames equal to one's, t1573). With the option off no
file moves (t3324, t2805).

## D12. Class walk

| Place | Relation |
|---|---|
| every offline `BurstCache` (7 places) | adopts names (D1, row 1068) |
| `moment_key`, `disk_key` (preview, tier) | unchanged; row 1069 stays the preview's |
| `Plugin` trait, ~90 built-ins | two defaulted facts; only the `false` ones are touched |
| wasm/OpenFX plugins (designed) | the default `true` |
| `render_cache` `plugins_of`/`put_rows` | a new layer input (track-stream-stage's matte) joins both and `picture_key` |
| `Muxer::open`, `drain`, `finish` | `open_encoder` taken out; `GopWriter` only in this mode |
| the encoder cascade and its trial | unchanged; a reopened encoder is the same candidate, no second trial |
| `encoders.rs` options | the forced-IDR names per encoder, in this mode only |
| `probe_output` | names `avc3`/`hev1` |
| `export_pipeline`, the serial export | spans (D8) |
| `ExportStats`, `report`, `render_dialog` | announced = the whole range; copied frames counted where taken |
| `hdr-sources-to-sdr` | its `Made.pixels` step renames every frame it changes |
| `persistent-cache` copies | an export reads no copy: the names are the originals' |
| `loudness-norms` | audio: rendered whole |
| Windows | the same; `h264_mf`, `*_amf` forced IDR checked by t3325 where a machine has them: an encoder that gives none ends the mode with its name |
| OTIO, `.kt` | untouched |

## D13. Order

1. D1 alone (row 1068). 2. The picture key and its facts. 3. `nal`, the SEI, the GOP plan, `GopWriter`; the tables. 4. Match, verify, copy, the pipeline. 5. The window, kt-cli. 6. The draft.

## Risks

- **A plugin that answers `false` wrongly** brings back a stale frame: t3335 runs every one; the default is `true`.
- **An encoder that cannot restart or force an IDR**: found at the first GOP, named, the render goes on without the mode.
- **`hev1`/`avc3` in MP4**: MKV is the default; the note says who refuses.
- **64-bit keys**: a GOP needs all its frames in order.

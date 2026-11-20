# Design: disk-copies

> DESIGN ONLY, revision 2 (2026-11-20), base `5e7a8ea9`: the owner's A10 (adaptive, a switch "as in the preview",
> never "proxy"); renamed from `proxies`. Revision 1 was on `ec30cb02`. Q1–Q2 in `proposal.md`; the defaults
> follow the recommended options. "Copy" below always means the disk copy of a media file.

## Context

Read on `ec30cb02`:

- `Player::replace_media(media, source)` (`src/libs/kt-engine/src/player.rs`) swaps the worker behind a
  `MediaId` and refreshes the builds with the new source facts; `retune_media` is for sources that can
  retune (a file cannot). `SourceId == MediaId`; a swap clears that source's cache only.
- Natives come from the opened source (`SourceFacts`); the decode step (`clip_decode_quality`) is a clip
  property derived from the native's margin over Quality (kt-engine CLAUDE.md, R.46). The preview's
  quality is a decode SIZE asked of `sws_scale` after a full `avcodec` decode — it does not reduce
  decode work.
- The disk render cache (`render_cache.rs`) keys by `durable_hash` + leaves noted by `note_leaf` (a
  file's path, size, mtime). The kept final mix is cleared on `SourceFacts::epoch`.
- Exports open their own sources: `render_dialog::open_all` (`src/apps/kt-app/src/render_dialog.rs`) and
  kt-cli by `MediaRef.path`; reel strips and posters are offline frame jobs with their own renderers.
- `MuxConfig { fps: u32, … }` (`src/libs/kt-ffmpeg/src/lib.rs`): an integer grid; `Probe.fps` is a
  rational. A grid-written copy of 23.976 material drifts (F-MED-002 "What gets in the way").
- Offline sources are allowed under invariant 5 when not shared, promising no tempo, finite and
  cancellable (`BgJob`, `src/apps/kt-app/src/bg.rs`).

## Goals / Non-Goals

**Goals:** a decode as cheap as a JPEG per frame while editing; the copy's size follows the preview; switching changes
sharpness only; the export bytes never depend on copies; no change to the project file.

**Non-Goals:** full-size intermediates; copies of generators, reels or stills (no decode cost to save); a copy as the
unit of sharing (F-COL-005).

## D0. The adaptive size (the owner's A10)

- The **wanted size** of a file's copy: in mode "As in the preview", the file's decode step at the current preview
  quality (`clip_decode_quality` over its events, the largest), clamped to at most 1/2 and at least 1/8 of the
  native; in mode 1/2 or 1/4, that fraction; in mode Off, none.
- A copy whose size is at least the wanted size is used (the decode reads it smaller through `sws_scale`); a copy
  smaller than wanted is used meanwhile AND a job makes the wanted size; the old copy is deleted after the swap.
  So raising the quality renders a better copy, lowering it costs nothing.
- At a preview quality of 1/1 in "As in the preview", the original plays (the clamp of 1/2 means a copy would be
  softer than asked).
- **Costly files** (Q2 a): a file is costly when its video codec is long-GOP (the probe's GOP length > 1) and
  either its native exceeds 1920×1080 or the player measured its decode slower than real time at the wanted size
  (the decode-time counters the performance panel reads). Only costly files get a copy without a click.

## D1. The copy file

- Container MKV (accepts any audio codec for stream copy; MP4 refuses PCM and multichannel layouts, the
  Premiere trap of F-MED-002 trick 2).
- Video: H.264 `keyint=1`, `bframes=0`, CRF 23, yuv420p 8-bit, size from D0 rounded to even;
  the encoder cascade's software x264 first (all-intra on a hardware encoder is not universally
  supported), hardware allowed when it opens with `gop_size=1`.
- Timestamps: each decoded picture is written with its own pts in the source's time base
  (`MuxConfig.time_base: Option<(i32, i32)>` + `push_frame_at(pts)`); VFR material stays VFR.
- Audio: every audio stream's packets copied (`avcodec_parameters_copy`, no decoder, no encoder) — the
  decoded samples are the original's; the sidecar seek index is per FILE identity, so the copy gets its
  own.
- A file without video gets no copy (the menu item is disabled with the reason).

## D2. Generation job

`kt_media::disk_copy::make(src_path, dst_path, frac, cancel, progress)`: its own short-lived `FfmpegSource`
(invariant 5's offline conditions), its own muxer, written to `dst.part` and renamed on success; a
cancel deletes the part. Scheduled as a `Background` lane job (one-scheduler): the player's `bg_pause`
vote pauses it while playing; an export's claim pauses it. One job at a time per machine (decode +
encode saturate cores). Progress = pts written / duration. On success the binding row is written in the project
cache folder's index (`project-cache-folder`), and the media is swapped at once (D3). The files land in
`<project cache folder>/copies/<identity>-<w>x<h>.mkv`.

## D3. The swap and the natives

`CopySource` wraps the file source of the copy and reports `MediaInfo` with the ORIGINAL's `width`,
`height`, `sar`, `fps`, `duration`, `has_alpha = false` and a new field `decoded_scale: u32` (2 for a
1/2 copy). The engine's decode step treats a request of Quality `q` as a read at `q` clamped to the
copy's real size: `clip_decode_quality` = min(as today, native/decoded_scale) — so a 1/1 preview of a
1/2 copy gets a 1/2 picture upscaled by the chain's existing `Resample` stop (`FrameTol::BILINEAR`), and
a 1/4 preview decodes the copy at 1/4. Conform, geometry and masks see the original raster (they read
`natives`, which are the original's). `Player::set_disk_copies(mode)` replaces each media whose best ready copy changes through `replace_media`, on the sources thread (`Player::sources()`,
never the UI thread).

## D4. Keys: nothing made from a copy is kept for the original

- **Frames cache**: `replace_media` clears the source — the warmth of the other mode goes (by design,
  invariant 4: one truth; no copy axis in the key).
- **Content serials**: a new source → new serials → the present memo and readback memo miss (they key by
  serial).
- **Disk render cache**: `note_leaf` notes the OPENED file (the copy's path, size, mtime) — so a copy
  composite and an original composite have different durable keys. Without this, a restart with copies
  off would replay copy-sharp frames from disk (the class item of D7).
- **Kept final mix**: the source's epoch moves on replace (`SourceFacts::epoch`), the store clears; the
  audio is bit-identical anyway (stream copy), but the rule stays one.

## D5. Export never reads a copy

All export bodies open sources by `MediaRef.path` through `open_all`, which never consults the copy
table; the GPU export plan is built with the original natives. Guard: t3445 renders a project with
disk copies ON in the preview and compares the export with the export of the same project in a fresh process
with no copy files present — byte-equal.

## D6. A camera's copy

`Use a camera's copy…` opens a file chooser; the candidate is probed: duration within one frame of the
original, frame count equal, the first and last 20 frame pts equal after time-base conversion, audio
duration within one burst. A mismatch is refused, the message names the field (rule 16). A camera's copy
need not be all-intra; it is used as given.

## D7. Class walk — "a substitute picture must never reach a final product"

| Place | Opens sources how | Verdict |
|---|---|---|
| export (both bodies), kt-cli export | `open_all` by path | safe; t3445 |
| render window's estimate / cost map | the player's | reads the preview — labelled "preview" already |
| disk render cache (render-cache-disk) | preview composites | keyed by opened file (D4) — t3446 |
| present memo / readback memo / render-ahead | serials | new source, new serials |
| reel strip, poster, bin thumbnail | offline jobs, own sources by path | originals (thumbnails come from the sidecar of the original) |
| scopes (change `scopes`) | presented composite | measures the copy picture while playing; the paused full-frame measurement (scopes D2) is offline → original; the label says "disk copy" when the presented frame came from one |
| stream stage (Datamosh) | decodes the event's stream from the cache | runs on copy frames in the preview; the export decodes originals — the preview/export difference is the decode size, as today at 1/4 |
| eyedropper | screen pixels | reads what is shown; nothing to change |
| analysis (LUFS, sync) | audio | identical samples |

## D8. Format, OTIO, idle

- `.kt`: unchanged (`UiState.disk_copies`, not written at its default). The binding is in the project cache
  folder's index (`project-cache-folder`); a lost index forgets bindings, and the files are found again by their
  names `<identity>-<w>x<h>.mkv`.
- OTIO: a later stage may write the copy as a second media reference (OTIO's own key for it); until then nothing
  of the edit is lost.
- Idle: no job → no work; a finished job leaves nothing running; the badge is static. The decode-cost probe
  reads counters the player keeps while playing; it never runs a decode of its own.

## D9. The name

No string, catalog key, menu item, hint, log line or file name says "proxy" (the owner, A10). A guard scans the
catalogs (`i18n/*.toml`) for `proxy`/`прокси` and fails on a hit. The final name is Q1's; the working name is "disk
copies" (`disk_copies` in code).

## Rejected

- **A copy axis in the cache key** — two truths of coverage (invariant 4, F-MED-002).
- **Copy audio re-encoded** — the samples would differ from the original; stream copy keeps them.
- **An export checkbox "use copies"** — Premiere's trap; nothing to forget if it cannot be done.
- **A copy at the full native size** — all-intra at 4K costs more disk than the original; at 1/1 the original plays.
- **Copies kept in the program's data folder** — lost when the project moves; the owner put caches in the project's
  folder (A14).
- **Grid-timed copy** — drifts on fractional rates.

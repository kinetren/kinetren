# Design: proxies

> DESIGN ONLY, revision 1 (2026-11-20), base `ec30cb02`. Q1–Q3 in `proposal.md`; the defaults follow
> the recommended options.

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
  rational. A grid-written proxy of 23.976 material drifts (F-MED-002 "What gets in the way").
- Offline sources are allowed under invariant 5 when not shared, promising no tempo, finite and
  cancellable (`BgJob`, `src/apps/kt-app/src/bg.rs`).

## Goals / Non-Goals

**Goals:** a decode as cheap as a JPEG per frame while editing; switching changes sharpness only; the
export bytes never depend on proxies; no change to the project file.

**Non-Goals:** optimized media (full-size intermediates); proxies of generators, reels or stills (no
decode cost to save); proxies made by the GPU encoder of the render cache segments (a later option); a
proxy as the unit of sharing (F-COL-005).

## D1. The proxy file

- Container MKV (accepts any audio codec for stream copy; MP4 refuses PCM and multichannel layouts, the
  Premiere trap of F-MED-002 trick 2).
- Video: H.264 `keyint=1`, `bframes=0`, CRF 23, yuv420p 8-bit, size `native / 2` rounded to even (Q2);
  the encoder cascade's software x264 first (all-intra on a hardware encoder is not universally
  supported), hardware allowed when it opens with `gop_size=1`.
- Timestamps: each decoded picture is written with its own pts in the source's time base
  (`MuxConfig.time_base: Option<(i32, i32)>` + `push_frame_at(pts)`); VFR material stays VFR.
- Audio: every audio stream's packets copied (`avcodec_parameters_copy`, no decoder, no encoder) — the
  decoded samples are the original's; the sidecar seek index is per FILE identity, so the proxy gets its
  own.
- A file without video gets no proxy (the menu item is disabled with the reason).

## D2. Generation job

`kt_media::proxy::make(src_path, dst_path, frac, cancel, progress)`: its own short-lived `FfmpegSource`
(invariant 5's offline conditions), its own muxer, written to `dst.part` and renamed on success; a
cancel deletes the part. Scheduled as a `Background` lane job (one-scheduler): the player's `bg_pause`
vote pauses it while playing; an export's claim pauses it. One job at a time per machine (decode +
encode saturate cores). Progress = pts written / duration. On success the sidecar row is written, and
when proxies are on, the media is swapped at once (D3).

## D3. The swap and the natives

`ProxySource` wraps the file source of the proxy and reports `MediaInfo` with the ORIGINAL's `width`,
`height`, `sar`, `fps`, `duration`, `has_alpha = false` and a new field `decoded_scale: u32` (2 for a
1/2 proxy). The engine's decode step treats a request of Quality `q` as a read at `q` clamped to the
proxy's real size: `clip_decode_quality` = min(as today, native/decoded_scale) — so a 1/1 preview of a
1/2 proxy gets a 1/2 picture upscaled by the chain's existing `Resample` stop (`FrameTol::BILINEAR`), and
a 1/4 preview decodes the proxy at 1/4. Conform, geometry and masks see the original raster (they read
`natives`, which are the original's). `Player::use_proxies(on)` replaces each media that has a ready
proxy (on) or a proxy open (off) through `replace_media`, on the sources thread (`Player::sources()`,
never the UI thread).

## D4. Keys: nothing made from a proxy is kept for the original

- **Frames cache**: `replace_media` clears the source — the warmth of the other mode goes (by design,
  invariant 4: one truth; no proxy axis in the key).
- **Content serials**: a new source → new serials → the present memo and readback memo miss (they key by
  serial).
- **Disk render cache**: `note_leaf` notes the OPENED file (the proxy's path, size, mtime) — so a proxy
  composite and an original composite have different durable keys. Without this, a restart with proxies
  off would replay proxy-sharp frames from disk (the class item of D7).
- **Kept final mix**: the source's epoch moves on replace (`SourceFacts::epoch`), the store clears; the
  audio is bit-identical anyway (stream copy), but the rule stays one.

## D5. Export never reads a proxy

All export bodies open sources by `MediaRef.path` through `open_all`, which never consults the proxy
table; the GPU export plan is built with the original natives. Guard: t3445 renders a project with
proxies ON in the preview and compares the export with the export of the same project in a fresh process
with no proxy files present — byte-equal.

## D6. Attach

`Attach proxy…` opens a file chooser; the candidate is probed: duration within one frame of the
original, frame count equal, the first and last 20 frame pts equal after time-base conversion, audio
duration within one burst. A mismatch is refused, the message names the field (rule 16). A foreign proxy
need not be all-intra; it is used as given.

## D7. Class walk — "a substitute picture must never reach a final product"

| Place | Opens sources how | Verdict |
|---|---|---|
| export (both bodies), kt-cli export | `open_all` by path | safe; t3445 |
| render window's estimate / cost map | the player's | reads the preview — labelled "preview" already |
| disk render cache (render-cache-disk) | preview composites | keyed by opened file (D4) — t3446 |
| present memo / readback memo / render-ahead | serials | new source, new serials |
| reel strip, poster, bin thumbnail | offline jobs, own sources by path | originals (thumbnails come from the sidecar of the original) |
| scopes (change `scopes`) | presented composite | measures the proxy picture while playing; the paused full-frame measurement (scopes D2) is offline → original; the label says "proxy" when the presented frame was proxy |
| stream stage (Datamosh) | decodes the event's stream from the cache | runs on proxy frames in the preview; the export decodes originals — the preview/export difference is the decode size, as today at 1/4 |
| eyedropper | screen pixels | reads what is shown; nothing to change |
| analysis (LUFS, sync) | audio | identical samples |

## D8. Format, OTIO, idle

- `.kt`: unchanged (`UiState.proxies: bool`, not written when false). The binding is in the sidecar
  (`CREATE TABLE IF NOT EXISTS proxy`, no `SCHEMA_VERSION` bump — the `poster` precedent). A lost sidecar
  forgets bindings; proxy files remain on disk and are found again by name `<identity>.mkv`.
- OTIO: a later stage writes `media_references["proxy"]` with the active key `DEFAULT_MEDIA`; until then
  nothing of the edit is lost.
- Idle: no job → no work; a finished job leaves nothing running; the badge is static.

## Rejected

- **A proxy axis in the cache key** — two truths of coverage (invariant 4, F-MED-002).
- **Proxy audio re-encoded** — the samples would differ from the original; stream copy keeps them.
- **An export checkbox "use proxies"** — Premiere's trap; nothing to forget if it cannot be done.
- **Grid-timed proxy** — drifts on fractional rates.

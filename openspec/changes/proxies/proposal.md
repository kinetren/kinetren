# Proposal: proxies

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3440–t3459.

## Why

The owner took candidate ★18 on 2026-11-01 (`docs/ktprompts/kkt-26.11.01-candidates.md`, registry
`kkt-26.11.01-index.md` P1): "4K HEVC 10 бит с телефона тормозит на скрабе. Хочу «сделать прокси» в
фоне, переключатель прокси/оригинал одним кликом, индикатор, а рендер — всегда с оригиналов."

The preview quality (1/16…1/1) saves the chain and the composite but not the DECODE: a long-GOP 10-bit
HEVC is decoded whole, frame by frame, at any preview size, and a scrub backwards re-decodes a GOP per
frame. Only a light intra-frame copy of the file removes that cost. Note F-MED-002
(`docs/features/media/proxies/proxy-toggle-workflow.md`) did the reconnaissance: Vegas fuses proxies into
the quality control and never renders from them; Premiere keeps an export checkbox that people forget;
the swap machinery (`Player::replace_media`), the sidecar DB and the muxer exist.

## What Changes

- **Make a proxy** (bin card right-click, or a selection of cards): a background job reads the file
  through its own short-lived source and writes `<proxy folder>/<identity>.mkv` — H.264 **all-intra**
  (every frame a keyframe: a scrub costs one picture) at a fraction of the native size (Q2), the
  ORIGINAL's frame timestamps copied one for one (no fps grid: the proxy is as long as the original to
  the tick), and the original's audio packets **stream-copied** (no re-encode: the same samples).
  Progress on the card; paused while playing; cancellable; the job survives closing the window.
- **Attach a proxy** made elsewhere (a camera's proxy): checked against the original — duration within
  one frame, the same frame timestamps, the same audio length; a mismatch is refused with what differs.
- **One switch "Proxies"** next to the preview quality (Q1): on — every media with a ready proxy decodes
  it; off — originals. A card shows a badge (ready / making N % / none), the switch says how many of
  the project's files have proxies.
- **The proxy is a decode step, not another picture geometry**: the source opened on the proxy reports
  the ORIGINAL's native size and frame rate; frames arrive at the proxy's size and enter the chain as
  a decode at a lower quality does today. The event's conform, geometry, masks and the frame plan are
  computed against the original's raster, so switching changes sharpness only, never framing.
- **Never rendered**: the export, the render window, kt-cli, the reel strips, the poster and the disk
  render cache never open a proxy: they open sources by `MediaRef.path`. A composite made from a proxy is
  keyed apart from one made from the original (the disk tier's leaf names the opened file), so no
  proxy frame is ever kept for the original.
- **The binding lives in the sidecar index** (`<project>.ktindex.db`, a new table), keyed by the
  original's identity — the `.kt` file does not change.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `media-index`: the proxy as a decode substitute behind the same `MediaId`; generation as an offline
  job; the binding table; matching rules for attached proxies.
- `export`: an export never reads a proxy, guarded by test.
- `ui-reach`: make/attach/remove from the bin card, the switch next to the quality, badges.

## Impact

- kt-ffmpeg: `MuxConfig` takes the source's time base and per-frame timestamps (today `fps: u32`, the
  grid that would make a 23.976 proxy 4 s short per hour, F-MED-002); an MKV path with audio stream copy;
  `keyint = 1`.
- kt-media: `ProxySource` (opens the proxy file, reports the original's native facts), the sidecar
  table `proxy(media_identity, path, w, h, made_by, created)`, the transcode loop.
- kt-engine: `Player::use_proxies(bool)` swaps sources through `replace_media`; `note_leaf` names the
  opened file; `SourceFacts` epoch bump (the kept mix of the old source is not replayed).
- kt-app: bin card menu, badge, switch, job list, i18n ×5.
- Project format: no change (the binding is in the sidecar; the switch is `UiState.proxies`, not
  written off). OTIO: export may write the proxy as a second `media_references` entry (key `proxy`) —
  a later stage; nothing of the edit is lost meanwhile, so no row in "What OTIO export loses".
- Idle: nothing runs unless a proxy job was asked; a job pauses while playing (scheduler class
  `Background`).
- Feature note: F-MED-002 refreshed.

## Questions to the owner

**Q1. One switch, or fused into the preview quality?** Your words: "переключатель прокси/оригинал
одним кликом, индикатор". Vegas fuses them (Draft/Preview quality = proxy); FCP has a separate
"Proxy Preferred / Original" choice.
- (a) **A separate "Proxies" toggle next to the quality, with a count** (*recommended*: you see which
  one plays; the quality keeps its meaning).
- (b) Fused: a quality of 1/2 or lower uses a ready proxy automatically; 1/1 always the original.

**Q2. Proxy size and where the files lie.** Your disk is short (2026-10-30, 2026-11-01). A 1/2-size
all-intra H.264 of 4K is about 15–25 % of a phone HEVC's size per minute (intra frames are larger than
long-GOP ones).
- (a) **1/2 size, in a folder next to the project (`<name>.proxies/`)** (*recommended*: travels with the
  project; a person sees and deletes it).
- (b) 1/4 size (smaller files, softer picture), same place.
- (c) In the program's data folder under a size limit, oldest removed first (never fills the disk, but
  a moved project loses its proxies).

**Q3. Offer proxies by themselves?** Your rule 20: nothing happens by itself.
- (a) **Never by itself; when a file decodes slower than real time while playing, the status line says
  so once with "Make proxy" in the card's menu** (*recommended*).
- (b) Make them on import for files above a size (Premiere's ingest).

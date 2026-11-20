# Proposal: disk-copies

> DESIGN ONLY (spec session, 2026-11-20), revision 2 after the owner's answers (`kkt-26.11.20` A10). Renamed from
> `proxies`: the owner — "не называй это прокси никогда … просто кэш тоже нельзя - не понятно что он не в озу". The
> working name "disk copies" stands until his answer to Q1. Tests reserved: t3440–t3459.

## Why

The owner took candidate ★18 on 2026-11-01 (`kkt-26.11.01-index.md` P1): "4K HEVC 10 бит с телефона тормозит на
скрабе", a light copy made in the background, a one-click switch, an indicator, the render always from originals.
On 2026-11-20 (`kkt-26.11.20.txt` lines 12–15) he reshaped it: "адаптивный? как играем в превью так и храним? и если
выбрали лучшее качество то рендерим лучшее чем есть. ну и не мешает сделать рядом переключатель где будет "как в
превью" или чет такое", and the name must say the copies are on disk.

The preview quality (1/16…1/1) saves the chain and the composite but not the DECODE: a long-GOP 10-bit HEVC is
decoded whole at any preview size, and a scrub backwards re-decodes a GOP per frame. Only a light intra-frame copy
of the file removes that cost. Note F-MED-002 (`docs/features/media/disk-copies/disk-copies-of-media.md`) holds the
reconnaissance.

## What Changes

- **The copy follows the preview** ("как играем в превью так и храним"): a file's disk copy is made at the size the
  preview decodes it (its decode step at the current preview quality), never larger than 1/2 of the native size;
  at a preview quality of 1/1 the original plays. Choosing a higher preview quality makes a better copy in the
  background (the smaller one keeps playing until it is ready, then is deleted); a lower quality reads the copy
  there is at the smaller size, with no new job.
- **One switch beside the quality** with the choices **As in the preview** (the default), **1/2**, **1/4** (a fixed
  copy size) and **Off** (originals only); it shows how many of the project's files have a ready copy and their
  total size on disk.
- **Which files get one** (Q2): with the switch on, every video file whose decode is costly (a long-GOP codec, or
  measured slower than real time at the preview's size while playing) gets a copy by a background job, paused while
  playing or exporting; "Make a disk copy" on a bin card asks for one by hand; "Remove disk copies" frees the space.
- **The copy file**: an MKV, H.264 with every frame a keyframe, the original's own frame timestamps one for one
  (fractional and variable rates stay exact), the original's audio packets copied without re-encoding.
- **Where**: the project's cache folder (change `project-cache-folder`, the owner's A14: "подпапка в папке проекта
  с разными кешами"), subfolder `copies/`, named by the original's identity.
- **The copy changes sharpness only, never framing or sound**: a source opened on a copy reports the original's
  native size, aspect, rate and duration; conform, geometry, masks and the frame plan are the original's.
- **Never rendered**: the export, kt-cli, the reel strips, the poster and the disk render cache never open a copy;
  a composite made from a copy is never stored under the original's key.
- **Use a camera's copy…** (a camera's low-resolution file): checked against the original, refused naming what
  differs.
- Nothing in the interface says "proxy".

## Capabilities

### Modified Capabilities

- `media-index`: the disk copy as a decode substitute behind the same `MediaId`; the adaptive size; generation as an
  offline job; the binding; checks for a camera's copy.
- `export`: an export never reads a disk copy.
- `ui-reach`: the switch and its choices, badges, the card's items.

## Impact

- kt-ffmpeg: `MuxConfig` takes the source's time base and per-frame timestamps (found-defects 823); MKV with audio
  stream copy; `keyint = 1`.
- kt-media: `CopySource` (opens the copy, reports the original's natives and its `decoded_scale`), the binding table
  in the project cache folder's index (`project-cache-folder`), the transcode loop.
- kt-engine: `Player::set_disk_copies(mode)` swaps sources through `replace_media`; `note_leaf` names the opened
  file; the decode-cost probe that marks a file as costly.
- kt-app: the switch, card badges and items, the job's progress, i18n ×5.
- Project format: the switch is `UiState.disk_copies` (a convenience, not written at its default). No `.kt` version.
  OTIO: nothing of the edit is lost; no row.
- Idle: no job → no work; jobs are `Background` lane jobs paused by playback and export.

## Questions to the owner

**Q1. The name.** Your words: "не называй это прокси никогда … пусть будет постоянный кэш или что-то такое более
компактное. просто кэш тоже нельзя - не понятно что он не в озу." What it is: a light copy of each heavy file, kept
on disk in the project's cache folder, made at the size the preview plays.
- (a) **"Копии на диске" / "Disk copies"** (*recommended*: says what and where in two words; the switch reads
  "Копии на диске: как в превью").
- (b) "Постоянный кэш" / "Persistent cache" (your own wording; "кэш" alone is what people call memory too).
- (c) "Лёгкие копии" / "Light copies" (says why, not where).

**Q2. Which files get a copy without asking.** Your words: "как играем в превью так и храним". A copy costs disk
(a 1/2 copy of a 4K phone clip is about a fifth of the clip; your disk is short) and a background job while paused.
- (a) **With the switch on, only files whose decode is costly — long-GOP camera and phone codecs, or measured slower
  than real time when played; the rest are not copied** (*recommended*: the copies go where they help).
- (b) Every video file of the project.
- (c) None without asking: only "Make a disk copy" on a card, with a one-time hint when a file plays too slowly.

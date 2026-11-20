# Proposal: memory-breakdown

> DESIGN ONLY (spec session, 2026-11-20). Registry `v26.10-index.md` Q1–Q3 (no). Note F-RES-011 (refreshed).
> Tests reserved: t3962–t3967.

## Why

The owner, `v26.10` lines 143–145: "Счеткик ОЗУ внизу основного окна не равен счетчику ОЗУ в окне диагностики … процесс
kt вообще может занимать 4гб или 10, а счетчик отражает только 500мб … внизу окна хочется компактный счетчик видеть, а в
окне диагностики все-таки какой-то раскрытый список-1d-граф заполнения по секциям - что где и чего, и я не про тупое
чтение procinfo, мы должны не только базовую procinfo вывести но еще и распределить в ней информацию по нашему
управлению памятью и постоянной памятью проекта, тот же ktindex."

Since then both counters read the process's resident memory (frames-cache-growth), and memory-ledger prints the caches'
tenants in one Diagnostics line. What is missing is his item 3: the whole resident memory split into OUR sections, as a
bar, and the project's memory on disk.

## What Changes

- **Every large holder reports into the ledger.** Besides the cache tenants, report-only accounts (they are never
  granted, only counted): the project model and its undo history, the timeline's waveform peaks and thumbnails in
  memory, the reel previews, the interface's textures (egui), open decoders (their frame pools), the audio render's
  state (effects' buffers, the ring), glyph atlases and fonts. Each account is opened by its owner and kept current
  by it (a byte count it already knows; the model's by a cheap estimate per revision).
- **The Diagnostics "Memory" section draws one bar** of the resident memory: one coloured segment per section, the
  sections ordered by size, "not counted" as the rest (resident minus the accounts); hovering a segment names it with
  MB and what it is; under the bar the same in a list. A second bar shows the GPU's memory (the device pools and
  textures kt-gpu holds), labelled as video memory.
- **The project on disk**: a third bar, the project's stores: the sidecar index (`<project>.ktindex.db`: peaks,
  thumbnails, packet index), the project's cache folder (prerender, stream history, disk copies — change
  `project-cache-folder`), the recovery file; sizes read when the section opens and every 5 s while it is open.
- **The status bar keeps one compact number** (resident MB); its hover shows the top three sections and the video
  memory (Q1).

## Capabilities

### Modified Capabilities

- `telemetry`: report-only ledger accounts; the breakdown's arithmetic (sections + not counted = resident).
- `ui-reach`: the Memory section's bars and list; the status counter's hover.

## Impact

- kt-time (`mem.rs`): report-only tenant kinds (`Model`, `Undo`, `Peaks`, `Thumbs`, `ReelPreview`, `UiTextures`,
  `Decoders`, `AudioState`, `Fonts`), `report(kind, bytes)`; the snapshot carries them; `GROWING` unchanged.
- kt-model: a byte estimate of a project and of the undo stack, updated per revision (summed field sizes, no
  serialization).
- kt-engine, kt-media, kt-app, kt-text: their holders report.
- kt-gpu: the device's bytes by pool for the second bar.
- kt-app: the Diagnostics section, the status hover, i18n ×5.
- No format change. Idle: the counters are numbers kept by their owners; the disk sizes are read only while the
  section is open.

## Questions to the owner

**Q1. The compact counter at the bottom.** Your words: "внизу окна хочется компактный счетчик видеть". Today it shows
the process's resident memory ("memory: 3200 MB").
- (a) **Resident memory only; its hover lists the three largest sections and the video memory** (*recommended*: one
  number that matches the Diagnostics bar's total).
- (b) Resident and video memory side by side ("3.2 GB · VRAM 1.1 GB").

**Q2. "Постоянная память проекта" — which disk stores.**
- (a) **The project's own: the sidecar index, the project's cache folder, the recovery file** (*recommended*: what
  this project costs on disk and can be cleared).
- (b) Also the program's own data (the library of downloads, logs, the telemetry journal).

# Proposal: memory-breakdown

> DESIGN ONLY, revision 3 (2026-11-24, base `0c2a4065`): made implementation-ready against today's code (design D0);
> revision 2 had the owner's B12. Registry `v26.10-index.md` Q1–Q3. Tests: t3962–t3965, t4232–t4234. No open question.

## Why

The owner's items, whole:

- `v26.10.txt` lines 143–145:
  > 1. Счеткик ОЗУ внизу основного окна не равен счетчику ОЗУ в окне диагностики, они действительно разные вещи считают или что?
  > 2. Даже если считают разные вещи - процесс kt вообще может занимать 4гб или 10, а счетчик отражает только 500мб, что тоже странно
  > 3. впринципе внизу окна хочется компактный счетчик видеть, а в окне диагностики все-таки какой-то раскрытый
  > список-1d-граф заполнения по секциям - что где и чего, и я не про тупое чтение procinfo, мы должны не только базовую
  > procinfo вывести но еще и распределить в ней информацию по нашему управлению памятью и постоянной памятью проекта, тот
  > же ktindex.
- `kkt-26.11.20b.txt` lines 25–26 (B12): the counter — "к текущему добавить vram, и убрать лишнее, ну и в gb показывать, а
  при наведении - раскрывать что-то еще показывать"; the project's disk — "Только проекта (Recommended)".

**The reading, checked.** "К текущему добавить vram, и убрать лишнее": the current counter is two items, "frames: V/cap MB"
and "memory: P MB". The memory stays, VRAM joins, the frames pair is what goes (revision 2 dropped only the word
"memory"). "При наведении раскрывать": by the standing rule there are no pop-ups (t83, the owner's R.8), and the one
hint's place is the status line; the counter's own hint expands there, and a click opens the section. "Тот же ktindex": since
project-cache-folder (merged 2026-11-24) `ktindex.db` is no more, its content is the `media/` part of the cache folder, so
the third bar is the folder's parts. Items 1 and 2 were closed by frames-cache-growth: both numbers are the process's.

## What Changes

- **Two new kinds in the ledger**, counted and never granted: open decoders' picture pools and the audio render's state.
- **A "Memory" section in Diagnostics**: one bar of the process's memory by section, the rest as "not counted"; a bar of
  video memory; a bar of the project's disk. A list under each. It takes the place of the one line "Caches: …".
- **The status bar**: "3.2 GB · VRAM 1.1 GB" alone; its hint expands the largest sections; a click opens the section.
- **Video memory is measured**, by the graphics library's own counters of every device the program opened.

## Capabilities

### Modified Capabilities

- `telemetry`: the sections, their arithmetic, the video memory's measure.
- `ui-timeline`: the requirement "The memory numbers say what they count…" (replaced by B12).
- `ui-reach`: the section's bars, the counter's hint and click.

## Impact

- kt-time `mem.rs`: `Tenant::{Decoders, AudioState}`, `KINDS` 10; `Tenant::on_device`.
- kt-gpu: wgpu's `counters` feature, `memory::video()`; kt-ffmpeg/kt-media: the decoders' account; kt-engine `fxstore.rs`:
  the audio state's.
- kt-model: `Project::approx_bytes`, `TimelineModel::memory`.
- kt-app: `mem_view.rs` (new), `app.rs`, i18n ×5.
- No format change. OTIO: nothing. Idle: nothing is read while neither the counter's hint nor the section is on screen, but
  the two numbers of the bar in frames drawn anyway, once a second, as today.

## Questions to the owner

None. Q1–Q2 as asked are in the git history of this file (800f7b45).

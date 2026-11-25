# Proposal: subtitles-and-text-editing

> DESIGN, revision 2 (2026-11-25), base `fc60a5d9`: implementation-ready (stage A with nothing unmerged but `shared-events`). The owner
> answered Q1–Q3 (A11). Code starts on the orchestrator's go. Tests: t3460–t3465, t3467–t3474, t4210, t4250–t4252.

## Why

Three claims of the owner, one feature:

- ★14, taken 2026-11-01 (`kkt-26.11.01-candidates.md`, registry P1): "Нужна дорожка субтитров: SRT/VTT
  туда-обратно, вшивка стилем текста, анимация по словам как в CapCut. … в панели текста ищу фразу,
  удаляю слова — режется видео, паузы и «эээ» — маркерами."
- B5 (`kkt-26.11.01b.txt`, 2026-11-01): "нету speech to text, и сразу забегаю в перед, мне не нужно
  топорное окно, где мне выводится весь текст в событии, мне нужна возможность "ходить" по этому
  тексту, нужна навигация по нему... в том числе в поиске, нужно удобство". Taken as (user-feedback,
  "the second list"): the transcript is text tied to time — the word under the playhead lit, a click on
  a word moves the playhead, a search steps through hits and marks them on the timeline, a selection
  of words is a region; recognition runs locally (a Whisper-class model downloaded on a click, as
  FFmpeg on Windows), nothing goes to a cloud.
- `v26.10` N4: "whisper: text search, pauses; analysis of pauses by sound".

Today: titles exist (kt-text, F-TXT-001: vendored Noto Sans, harfrust shaping), no subtitle object, no
SRT/VTT, no recognition, no transcript. Note F-TXT-004 rejected bundled model weights; the owner's
"downloaded on a click" revises that verdict (weights are not shipped, they are fetched on request).

## What Changes

- **Transcript of a media file**: words with their MATERIAL time (`t0`, `t1` in material ticks, like
  automation keys — a trim, a move, a cut or a reverse never desyncs the text), confidence, and
  speaker-less paragraphs. Made by a local recognizer (Q1) as an offline job on the media's audio;
  stored in the project (the person corrects words; corrections are content).
- **The Text panel** (Q2): the project's text in timeline order — each event shows the words of its
  material window, joined across cuts. The word under the playhead is lit and kept in view while
  playing; a click on a word moves the playhead; typing in the search box steps through hits (Enter /
  Shift+Enter) and marks them on the ruler; dragging over words makes the region; Delete with a region
  of words cuts it with ripple (change `ripple-and-trim-modes`: one rule); a word can be retyped.
- **Fillers**: "Mark fillers" (words in a per-language list: "эээ", "uh", "um", …) puts range markers; nothing is cut by
  itself (rule 20). Pauses are the change `pause-detection` (by sound, noisy sound included); the Text panel opens it.
- **A subtitle track**: a video track that holds caption items (start, duration, text with line
  breaks) in timeline time; "Create captions from text" splits the transcript into items by a maximum
  line length and duration; import and export SRT and WebVTT; a track style (font of the vendored set,
  size, colour, outline, box, position, safe margin) and per-word highlight ("karaoke", CapCut) as the
  burned-in animation.
- **Burn-in through the one plan**: a visible subtitle track is a procedural layer in `frame_plan` drawn
  by kt-text (the title's rasterizer — no second text path, UI rule 17), above the video tracks by its row.
- **Export**: a sidecar `.srt`/`.vtt` next to the render (a checkbox in the render window), and burn-in
  per track. An embedded `mov_text` stream is a later stage.

## Capabilities

### New Capabilities

- `subtitles`: the subtitle track and caption items, SRT/VTT, style, burn-in, sidecar export.
- `transcript`: words in material time, local recognition as a downloadable component, the Text panel,
  search, word selection as region, text deletion through ripple, filler marking.

### Modified Capabilities

- `project-format`: transcripts and the subtitle track raise the file version by content; OTIO export
  carries them only in `metadata["kt"]` and writes a sidecar SRT.
- `ui-reach`: the Text panel, the recognizer download, captions from text, the subtitle track's menus.

## Impact

- kt-model: `Project.transcripts`, `Track.sub`, captions and their commands, `GeneratorKind::Captions` (internal), `TlEntity::Caption`.
- kt-media: `subtitles_io`, the captions' frame; new `kt-speech` (dlopen whisper.cpp). kt-text: `highlight`.
- kt-engine: `VideoClip.holes`; nothing else. kt-app: the row, the in-place editor, the Text tab, the render window, i18n ×5.
- kt-otio: `metadata["kt"]`, `<name>.srt`. Format: the next free version by content. Idle: as asked by a person only.

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20.txt` line 19, A11, whole): "Q14–Q16 subtitles: "п.1. и на будущее - все что у нас качается -
надо переносить и делать архитектурно в виде некоторой библиотеки, типо как плагины чтоли их же тоже можно будет качать. вот и
сделат библиотеку плагинов."" — Q1–Q3 (a): whisper.cpp pinned by sha256, the Text tab beside the bin, font packs on a click; every download
through `download-library`. Read again on 2026-11-25 with his v26.10 line 89 ("можно и whisper использовать … учитывая что он есть в ffmpeg"):
"п.1" chose the pinned library over FFmpeg's own filter (option b), which needs an FFmpeg built with it. No open question.

The questions as asked:

**Q1. Which recognizer is downloaded?** Your words (2026-11-01): "speech to text … мне нужна возможность
"ходить" по этому тексту". Taken: local, a Whisper-class model downloaded on a click as FFmpeg on
Windows. Today: nothing. FFmpeg 8 has a `whisper` filter only when built with whisper.cpp — most Linux
distributions do not build it, and using it means loading libavfilter (the frame rule of kt-text forbids
a sixth FFmpeg library for pictures; for analysis it would be allowed but rarely present).
- (a) **The whisper.cpp library + a model file downloaded on a click, pinned by version and sha256,
  loaded at run time like FFmpeg** (*recommended*: works on every machine; the binary links nothing).
- (b) FFmpeg's `whisper` filter when the system FFmpeg has it; otherwise the feature is absent.
- (c) A recognizer compiled into the program (a Rust inference crate): no runtime download, only the
  model; a larger build and slower CPU inference than whisper.cpp.
Model size choice in the download card: base (~140 MB), small (~470 MB), medium (~1.5 GB).

**Q2. Where does the Text panel live?** Your words: "мне не нужно топорное окно". UI rule 22: the
place is sought in the order chain strip → a section of an existing window → a panel → a window.
- (a) **A tab beside the media bin in the left column** (*recommended*: it lives next to the timeline,
  the bin keeps its width limit, R.13).
- (b) A dockable window (a registry row).
- (c) Under the preview, sharing its height.

**Q3. Subtitles in Chinese, Japanese, Korean, Arabic.** Today the frame font is Noto Sans only
(kt-text rule 1: a frame's font is vendored, never a system font). A CJK font is ~16–19 MB per weight.
- (a) **Vendor Noto Sans CJK and Noto Naskh Arabic as downloadable font packs on a click, pinned by
  sha256** (*recommended*: the release stays small; the frame stays deterministic).
- (b) Ship them in the release (+~40 MB).
- (c) Burn-in only for scripts the vendored font covers; other scripts export as sidecar text only.

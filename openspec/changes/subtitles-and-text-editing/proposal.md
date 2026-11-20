# Proposal: subtitles-and-text-editing

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3460–t3484.

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
- **Pauses and fillers**: "Mark pauses" (silence ≥ N ms from the peak pyramid, no decode) and "Mark
  fillers" (words in a per-language list: "эээ", "uh", "um", …) put range markers; nothing is cut by
  itself (rule 20).
- **A subtitle track**: a fifth track kind holding caption items (start, duration, text with line
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
  search, word selection as region, text deletion through ripple, pause and filler marking.

### Modified Capabilities

- `project-format`: transcripts and the subtitle track raise the file version by content; OTIO export
  carries them only in `metadata["kt"]` and writes a sidecar SRT.
- `ui-reach`: the Text panel, the recognizer download, captions from text, the subtitle track's menus.

## Impact

- kt-model: `Transcript { media, lang, words }`, `Word { t0, t1, text, conf }`, `LaneKind::Subtitle`,
  `Caption { id, start, duration, text }`, `SubStyle` on the track; commands; `normalize`; the format.
- kt-media: `transcribe` job (decode audio offline → 16 kHz mono → the recognizer), SRT/VTT parse/write,
  the subtitle procedural source (kt-text frame of the captions active at a tick).
- kt-engine: the subtitle track as a layer source (`SUB_TAG | track id`), prefetch-free (procedural).
- kt-text: per-word highlight as an animation input (a cluster range and a colour), no time inside.
- kt-app: the Text panel, search, the download card (as the FFmpeg download), menus, i18n ×5.
- kt-otio: `metadata["kt"]` carries transcripts and the subtitle track; the export writes `<name>.srt`.
- Fonts: CJK/Arabic subtitles need glyphs; kt-text vendors Noto Sans only (Latin/Cyrillic/Greek). Q3.
- Format: the next free `.kt` version by content (`has_transcripts() || has_subtitle_track()`);
  rows in "What OTIO export loses".
- Idle: recognition runs only when asked (background lane, paused while playing); the panel follows the
  playhead only while it moves; a static panel costs nothing.

## Questions to the owner

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

# Design: subtitles-and-text-editing

> DESIGN ONLY, revision 2 (2026-11-25), base `fc60a5d9`. Q1–Q3 decided (A11). Implementation-ready: stage A needs nothing unmerged;
> stage B takes the Library's `speech-*` rows and the font packs from `download-library` (its stages 2 and 4).

## D0. What revision 1 said and the code contradicts

1. **`LaneKind::Subtitle`, a fifth kind.** 65 places match the four kinds; mute, solo, lock, order, opacity, effects, masks, copy of rows,
   `SetLaneOrder`, bands, OTIO rows all exist per VIDEO track. A subtitle track is a video track with captions in place of events (D5): it has
   its row in the composite order, its opacity, its chain and its masks with no new code, and a person sees a track like any other.
2. **`SUB_TAG`, a new kind of source.** A generator event has an address, a state, a retune lane, an identity on disk and procedural time.
   The captions of a track resolve to ONE synthetic generator event (D7); the one thing added is "nothing at this tick".
3. **"One word per line … `{"t0":…}`".** The file is serde's pretty JSON: an object is a line per field, 6 000 words would be 36 000 lines. A word is
   one STRING (D1).
4. **The download card pattern, 2.2 and 2.6**: the Library (`download-library`) now; a feature's `library_line`.
5. **Markers**: pauses are `pause-detection`; fillers stay here.
6. **`kt-text` "gets a cluster range"**: the reveal weights exist per cluster (`lib.rs:518–530`), the highlight joins them.
7. **The Text panel "beside the bin"**: the bin is one column (`draw_bin_column`) with its header; a tab strip is added there, the width limit untouched.
8. Since `frame-format-variants` (designed) titles in another shape wrap and keep their short side: captions use the same `wrap_lines`.

## Context

`GeneratorKind::Text` and `GenState` (`genmedia.rs`), `GeneratorSource` (`generator.rs`), `gen_sources` (`video.rs:3031`), `Player::update_gen`, kt-text
(`render_rgba`, clusters, reveal), `Track` (`project.rs:3111`), `resolve_reel`, `links::resolve_derived`, `clip_material_at`/`clip_timeline_at`,
`AudioSeries` jobs and the `analysis` lane, `pause-detection`, `ripple-and-trim-modes` (`ripple_plan`, `amend_batch`), `download-library`.

## Goals / Non-Goals

**Goals:** walk the text of the edit as text (B5); captions as a track with SRT/VTT both ways; burn-in by
one plan; text cuts in one undo step; nothing in a cloud.

**Non-Goals:** speaker diarization (later); translation; CEA-608/708 (inside the encoded video,
F-TXT-004 trick 1); `mov_text` embedding (stage 4); recognition of reels as a whole (a reel's text is
its leaves' text).

## D1. The transcript belongs to the media, in material time

```text
Project.transcripts: Vec<Transcript>                         // empty = not written
Transcript { media: MediaId, lang, engine: String, words: Vec<Word> }
Word { t0, t1: Tick /* material */, conf: u8, para, edited: bool, text }
```
In the file a word is a string: `"1234500 1267800 92 p Привет"` (`p` paragraph, `e` edited): one line each, a retyped word is one line of `git diff`.
An event shows the words of its material window through `clip_timeline_at` (trims, reverse, warp): no edit of an event touches a word. A morpher that swaps
the source shows the swapped file's. An instance of a composition shows its leaves' text (the panel reads the resolved reel). Commands `SetTranscript`,
`SetWord`, `MergeWords`, `SplitWord`, `RemoveTranscript`; content, undo, one media one transcript per language.

## D2. Recognition (Q1 a)

- Component: whisper.cpp's shared library (`speech-runtime`) and a ggml model (`speech-model`), items of the Library (`download-library`), loaded
  with `dlopen` by a new crate `kt-speech` (the kt-ffmpeg loader pattern; the binary links nothing). `KT_SPEECH_DIR` gives a folder to tests and
  to a person with a build of their own. Absent → the `library_line` with the size and the button.
- Job: an offline source of the media (invariant 5's offline conditions), audio decoded and resampled to
  16 kHz mono (swresample), fed in 30 s windows with word timestamps on; results mapped from sample
  index to material ticks (`ticks = samples × 90000 / 16000`, exact: 45/8). On the `speech` lane, class `Background`, paused
  while playing, cancellable, progress per window. Threads: half the cores.
- Recognition output is DATA once stored: invariant 11 is about renders, and a stored transcript renders
  the same everywhere. Re-running recognition replaces the words only after confirmation if any word was
  edited by hand (a word carries `edited: true`).

## D3. The Text panel (Q2 a)

A tab "Text" beside the bin. Content: the timeline text — for each event of the audio tracks (the lowest
audio track with a transcript first; a track selector in the head), in timeline order, its visible words;
a cut between events shows as a thin bar, a gap as "· · ·" with its length.

- **Follow**: the word under `video_playhead()` is lit; while playing the panel scrolls to keep it in
  view (repaint only while the playhead moves, invariant 12).
- **Click** a word → seek to its timeline start. **Drag** over words → `Project.region` = their span.
- **Search**: a box in the head; hits are counted ("3 / 17"), Enter/Shift+Enter step and seek, hits are
  drawn as ticks on the ruler (one mesh, rebuilt only when the query or the revision changes).
- **Delete** with words selected → the region cut with ripple (`ripple_plan`, its scope that of the mode, else the track), one undo step; until
  `ripple-and-trim-modes` is in, Delete cuts with a gap and says so.
- **Retype**: double-click a word edits its text (a transcript edit, not a cut).
- Layout uses the shared text widgets; a 40-minute interview (~6 000 words) is laid out once per revision
  and per width into galleys cached by (revision, width), drawn by clipping to the visible rows.

## D4. Pauses and fillers → markers

Pauses are the change `pause-detection` (2026-11-24): a detector that learns the recording's floor, since the owner asks
for noisy sound too (v26.10 line 89); the Text panel's "Pauses…" opens its window. "Mark fillers": words matching a per-language
list (in the i18n catalogue, editable in Settings) as range markers "filler". Marker names are made
unique by `unique_name` (the area/marker namespace rule). Nothing is cut; a "Cut all pause markers" item
is an explicit ripple of the named ranges.

## D5. The subtitle track and captions

```text
Track.sub: Option<SubTrack>                                    // legal on a VIDEO track with no events
SubTrack { style: SubStyle, captions: Vec<Caption> }           // sorted, not overlapping
Caption { id: CaptionId, start, duration: Tick, text, words: Vec<(u16 /* cluster */, Tick, Tick)> }
SubStyle { family, font_pack, bold, size, color, outline, box, align, pos_y, margin, highlight: Option<Color>, max_chars, max_lines }
```
Timeline-timed items: a caption is an edit decision. `validate`: events and captions do not share a track; captions do not overlap. The row of the track
draws caption boxes; selection, drag, trims, Delete and copy act on them as on events (`TlEntity::Caption`: t253's table demands its verbs). Ripple moves them
(`ripple_plan` gains `Caption`; the other way round if it lands second), a region crops copies of them, a split of the track's neighbours does not touch them.
`Create captions from text` makes them from the words visible in a range: a break at a paragraph, at a pause ≥ 0.5 s, at `max_chars` (42) × `max_lines`
(2), at 6 s; lines broken by `kt_text::wrap_lines` at 0.9 of the width; each word's cluster and timeline times are kept for the highlight. Re-creating replaces
nothing: a new track.
Commands: `AddCaption`, `SetCaption`, `RemoveCaption`, `SetSubStyle`, `SetTrackSub` (make and unmake), with the row's lock.

## D6. SRT / WebVTT

Parse: SRT (index, `hh:mm:ss,mmm --> …`, text, blank line) and VTT (header, cues, `<b>/<i>` dropped to
plain text with a note, cue settings ignored). Times rounded to the nearest tick (ms × 90 exact). Write:
from the track's captions, times as `round(ticks / 90)` ms, `\n` line breaks, UTF-8 without BOM.
Import puts a new subtitle track; a file with overlapping cues imports into as many tracks as needed
(never merged silently).

## D7. Burn-in through the one plan

`resolve_reel` adds ONE synthetic event per audible-by-eye subtitle track: kind `GeneratorKind::Captions(Arc<CaptionSet>)` (internal: not in the list of
kinds, not in the file), start = the first caption, duration to the last, lane = the track's, id = `caption_event(track)` (a reserved range, as
`slice_id` has one; `validate` refuses a model id in it). So the composite order, the track's opacity, blend, chain and masks, the prefetch, the address
`GEN_TAG | id`, the retune of a typed letter and the identity on disk are the generator's.
- `kt_media::generator::render_at(Captions, pts)`: the active caption by bisection → `text_params` of its style → kt-text, with `highlight: Option<(cluster
  range, colour)>` beside `reveal` (the branch without it is the former bytes); cached by (caption, highlighted word, size).
- **Nothing between captions**: `VideoClip.holes: Option<Arc<[TickRange]>>` from the set; `active_at` is false in a hole, so there is no layer and no upload.
- In a format of another shape (`frame-format-variants`) the size follows the short side and the margin the style's fraction.
- An entered composition has its own subtitle tracks, composed inside it.

## D8. Export

The render window: per subtitle track "burn in" (default on when the track is visible) and "sidecar"
(`.srt` / `.vtt` next to the output, written after the muxer closes, same base name). OTIO: captions and
transcripts in `metadata["kt"]`; the OTIO export also writes `<name>.srt` per subtitle track beside the
`.otio` so a foreign editor can import them.

## D9. Format, versions

`.kt`: `transcripts`, `Track.sub` and a style's font pack raise the file to the next free version by content. A project without them is byte-identical.
kt-otio envelope: the next `KT_META_VERSION` by content. Rows in "What OTIO export loses".

## D10. Class walk — "a long list drawn every frame"

| Place | Risk | Rule |
|---|---|---|
| Text panel | 6 000 words per frame | galleys cached by (revision, width), clip to view |
| ruler search ticks | thousands of marks | one mesh per (query, revision) |
| snap targets (`snap_targets`) | captions as snap points would allocate per call | captions join the snap set through the timeline's event index, not a per-call `Vec` |
| subtitle track row | hundreds of caption boxes | an index per track (bisection), the `EventPaint` replay on hover frames |
| marker band with pause markers | hundreds of range markers | existing band, its own cost (checked by the hover-static live step) |
| the subtitle source | a rasterization per frame | memo by (caption id, highlight word, size); a static caption is drawn once |

## D11. Idle

Recognition and pause marking are jobs asked by a person; at rest nothing runs; the panel's follow moves
only with the playhead; search ticks are a cached mesh. The live `idle` scene adds the Text panel open
with a long transcript and the mouse moving over it: no work counter but hover frames.

## Rejected

- **Captions as title events on a video track** (Vegas 2026): re-generation stacks duplicates, text edits
  invalidate a frame source (F-TXT-004 trick 2).
- **Captions in markers** — tick identity and a shared name space.
- **Words in timeline time** — every trim would desync them.
- **A cloud recognizer** — the owner: nothing goes to a cloud.
- **libass / drawtext for burn-in** — kt-text rule 4.

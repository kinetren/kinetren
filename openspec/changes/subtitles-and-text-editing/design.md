# Design: subtitles-and-text-editing

> DESIGN ONLY, revision 1 (2026-11-20), base `ec30cb02`. Q1–Q3 in `proposal.md`; the defaults follow
> the recommended options.

## Context

Read on `ec30cb02`:

- **Titles** (`GeneratorKind::Text`, R.52/R.59): values on the event, rendered by kt-text — a pure
  function (params, W, H) → RGBA with the vendored Noto Sans (`src/libs/kt-text/assets/`), harfrust
  shaping, no time inside (animation arrives as `reveal` / `offset` numbers). The source of a generator
  is addressed `GEN_TAG | event id` and lives under a MediaWorker (law P, kt-engine CLAUDE.md).
- **Markers** are identified by tick (`normalize` dedups by `t`), names share one namespace with areas
  (`DuplicateName`): unfit for a thousand caption lines (F-TXT-004).
- **Tracks** are objects (R.62 B); `LaneKind` = Video | Audio | Morph | Camera.
- **Keys glued to material** (`AutoKey.t` = material tick, R.33): the rule a word's time follows.
- **The peak pyramid** in the sidecar (`docs/media-index.md`): silence detection without decode
  (invariant 8).
- **Downloads on a click**: the FFmpeg download on Windows (F-MED-007,
  `src/apps/kt-app/src/ffmpeg_setup.rs`): pinned URL + sha256, a card, a progress bar, no network
  otherwise.
- **Ripple**: change `ripple-and-trim-modes` (`ops::ripple_plan`) — text deletion uses it.

## Goals / Non-Goals

**Goals:** walk the text of the edit as text (B5); captions as a track with SRT/VTT both ways; burn-in by
one plan; text cuts in one undo step; nothing in a cloud.

**Non-Goals:** speaker diarization (later); translation; CEA-608/708 (inside the encoded video,
F-TXT-004 trick 1); `mov_text` embedding (stage 4); recognition of reels as a whole (a reel's text is
its leaves' text).

## D1. The transcript belongs to the media, in material time

```text
Project.transcripts: Vec<Transcript>          // empty = not written
Transcript { media: MediaId, lang: String, engine: String, words: Vec<Word> }
Word { t0: Tick, t1: Tick, text: String, conf: u8, para: bool }   // t in MATERIAL ticks
```

An event shows the words whose `[t0, t1)` lies in its material window (the warp's span for a slowed
event: `clip_material_at` maps both ways). A split, trim, move, reverse or speed change needs no edit of
words — the REPRESENTATION keeps them in sync, as for keys (R.33). A morpher layer that swaps the
source shows the swapped media's words. Editing a word's text = `SetWord { media, index, text }`;
merging/splitting words keeps the times. In the `.kt` file one word per line (git-friendly, the R.70
rule): `{"t0":…,"t1":…,"w":"…"}`.

## D2. Recognition (Q1 a)

- Component: whisper.cpp shared library (pinned version, per-platform build with sha256) and a ggml
  model, downloaded on a click into the data dir, loaded with `dlopen` (the kt-ffmpeg loader pattern; the
  binary links nothing, invariant 10's spirit). Absent → the menu item opens the download card.
- Job: an offline source of the media (invariant 5's offline conditions), audio decoded and resampled to
  16 kHz mono (swresample), fed in 30 s windows with word timestamps on; results mapped from sample
  index to material ticks (`ticks = samples × 90000 / 16000`, exact: 45/8). Class `Background`, paused
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
- **Delete** with words selected → the region cut with ripple (one rule, `ripple_plan`), one undo step;
  the scope is the auto-ripple scope or "affected tracks" when the mode is off — the text gesture is an
  explicit command, so rule 20 holds.
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
LaneKind::Subtitle                          // the fifth kind; rows like any track
Track.sub: Option<SubTrack { style: SubStyle, captions: Vec<Caption> }>
Caption { id: CaptionId, start: Tick, duration: Tick, text: String, words: Vec<(Tick, Tick)> }
SubStyle { family, size_px, color, outline, box, align, pos_y, margin, highlight: Option<Color> }
```

Captions are TIMELINE-timed items (not material): a caption is what is shown, an edit decision. They
move with ripple like any time-anchored object (`ripple_plan` gains them). `Create captions from text`
builds them from the transcript words visible in a range: break at a paragraph, a pause ≥ 0.5 s, the
max characters per line (default 42) and max 2 lines, max 6 s; words' timeline times kept in `words` for
the karaoke highlight. Captions on one subtitle track do not overlap (validate), adjacent ones may abut.

## D6. SRT / WebVTT

Parse: SRT (index, `hh:mm:ss,mmm --> …`, text, blank line) and VTT (header, cues, `<b>/<i>` dropped to
plain text with a note, cue settings ignored). Times rounded to the nearest tick (ms × 90 exact). Write:
from the track's captions, times as `round(ticks / 90)` ms, `\n` line breaks, UTF-8 without BOM.
Import puts a new subtitle track; a file with overlapping cues imports into as many tracks as needed
(never merged silently).

## D7. Burn-in through the one plan

A visible subtitle track is a layer of the frame plan at its row position: a procedural source
`SUB_TAG | track id` (a new address range beside `GEN_TAG`, the `const _` order guard extended), whose
frame at tick `t` is kt-text's rasterization of the active caption(s) with the style, and the highlight
fraction of the word under `t` (passed as a cluster range + colour — kt-text gets no time). The source is
procedural (`MediaInfo.fps == None`), so it is not prefetched, has no frame grid floor, and its content
serial changes with the track's revision. Preview and export compose it by the same plan (invariant 7).
A caption-free tick has no layer (no cost).

## D8. Export

The render window: per subtitle track "burn in" (default on when the track is visible) and "sidecar"
(`.srt` / `.vtt` next to the output, written after the muxer closes, same base name). OTIO: captions and
transcripts in `metadata["kt"]`; the OTIO export also writes `<name>.srt` per subtitle track beside the
`.otio` so a foreign editor can import them.

## D9. Format, versions

`.kt`: `transcripts` and subtitle tracks raise the file to the next free version by content
(`version_for`: `has_transcripts() || has_subtitle_tracks()`). A project without them is byte-identical.
kt-otio envelope: the next `KT_META_VERSION` by content. Rows in "What OTIO export loses".

## D10. Class walk — "a long list drawn every frame"

| Place | Risk | Rule |
|---|---|---|
| Text panel | 6 000 words per frame | galleys cached by (revision, width), clip to view |
| ruler search ticks | thousands of marks | one mesh per (query, revision) |
| snap targets (`snap_targets`) | captions as snap points would allocate per call | captions join the snap set through the timeline's event index, not a per-call `Vec` |
| subtitle track row | hundreds of caption boxes | the existing `EventPaint` replay on hover frames |
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

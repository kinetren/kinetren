---
id: F-TXT-004
title: Subtitle track, SRT/VTT and text-based editing
status: candidate
priority: B
cost: L
category: titles/subtitles
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-otio, kt-engine, kt-ffmpeg, kt-media, kt-app]
invariants: [7, 8, 9, 10, 11]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: [F-TXT-001]
checked: 2026-09-07
---

# F-TXT-004 — Subtitle track, SRT/VTT and text-based editing

## Essence

The editor gets not a caption on the picture but a LIST of lines: each has a start,
an end and a text. The list lives on a separate timeline row, is edited as text,
is checked for line length and reading speed, and goes outward in three different
ways — as a file next to the video, as a stream inside the container, or burned into the pixels. Over the past two years automatic transcription has grown on top: the list appears by itself,
and in Vegas/Premiere/Resolve editing the TEXT of a line starts changing the EDIT.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Speech to Text / Insert Subtitles | there is no separate subtitle TRACK: transcription puts the result as title events — "Vegas Pro creates Titles & Text events on a new track in your timeline". Text editing is two-way: "Vegas Pro removes the selected text and the corresponding section from the timeline", and back — "Vegas Pro updates the corresponding events on the timeline to match the transcript edits". Offline, 7 models from "Tiny - Fast (~78MB)" to "Large v3 - Latest (~3100MB)", 100+ languages, SRT export. Subscription only: "This feature is available only with a Vegas Pro subscription" |
| Adobe Premiere | 26.x | Text panel (Transcript + Captions), Text-Based Editing | the "Transcript" and "Captions" panels at the clip and sequence level, a subtitle track in the timeline; transcription is local (Adobe told Larry Jordan that they moved to local processing "sometime in 2022"); by default SRT, plus CEA-608, CEA-708, EBU, Teletext, Australian; the line width is set in characters. In 26.3 (June 2026) word-by-word captions were added: "Premiere can now generate single-word captions directly from a transcript" |
| DaVinci Resolve | 21 (checked against the documentation for 20.x and the product page) | Subtitle track / Create Subtitles from Audio | a REAL data track: "Subtitles appear in the timeline above of your video tracks and can be moved and trimmed like any other media". Import of "timed text TTML, SRT, XML and embedded MXF/IMF subtitles", export "rendered into the final video or exported as separate TTMLs, SRT or VTT files". Animation attaches to the HEADER of the track: "Drag one of the five animation options into the caption track header". STT only in Studio; IntelliScript assembles the edit from a script, matching it against the transcription |
| Final Cut Pro | 12.3 | Closed captions (roles CEA-608 / iTT / SRT), Generate Captions | captions lie "in a closed caption lane at the top of the timeline", each format has its own role and sub-role per language. Automatic transcription is limited by hardware and language: "The Generate Captions feature requires a Mac with Apple silicon", "available in U.S. English only", the result is ordinary titles ("standard title clips created using the default Subtitle title") in the Subtitles role. On import a reference point is chosen: "Relative to Timeline" or an absolute one |
| After Effects | 26.x | none | there is no separate subtitle track: text is an ordinary text layer, timing is keys and markers. Import of SRT as a list of lines is not confirmed by the documentation — `helpx.adobe.com` did not open from the environment, see "Sources" [NOT VERIFIED] |

## The trick behind it

**1. One and the same text has three DIFFERENT fates on export, and the editor
shows only one of them.** A sidecar (SRT/VTT/TTML) is a file next to it,
the viewer turns it on and off, the search engine indexes it. An embedded stream is not
"the same text inside the file": CEA-608/708 travel in the user data of the picture, that is,
inside the ENCODED video ("CTA-708 captions are injected into MPEG-2 video
streams in the picture user data"), and therefore survive re-encoding
only together with it. Burn-in is not text at all, but pixels. A formal test for the
difference is given by FCP: CEA-608 "can be imported or exported as tracks embedded
in a media file or as separate files", while iTT and SRT "can't be embedded in an
output media file the way CEA-608 closed captions can". Resolve drives it home visually: the five caption animations live only in the burned-in variant —
"Captions exported as a separate file can not be animated". So the nice
presentation that the editor saw in the preview exists in exactly one of the three
outputs, and in the other two he delivers bare text and learns about it from the client.

**2. Half the market does not have a "subtitle track" — there it is titles, and this
is visible from the side effects.** Vegas 2026 officially puts the transcription as
"Titles & Text" events on a new track. Hence exactly those complaints that do not happen
with a list of lines: repeated generation does not replace the earlier lines, it puts second ones on top ("competing subtitles… they will superimpose upon each
other"), text edits in the panel do not hold, and generation breaks AFTER the clips have been cut. For a list of lines, editing the text is editing a field of a record; for titles, editing the text is editing the SOURCE of the frame, that is, redrawing and invalidating the cache.

**3. Text editing is not a subtitle feature, but a change of what counts as the source of the edit.** Vegas's wording is verbatim: "Vegas Pro removes the
selected text and the corresponding section from the timeline", and back —
editing the timeline rewrites the transcript. That is, the transcript and the edit have become
two projections of one structure, and its unit is a WORD with its own time.
That per-word time is really stored is proven by Premiere 26.3: captions "one word at a time" are made from the transcript already available, without re-running recognition. The non-obvious price: the word must be tied to the tick
of the MATERIAL, otherwise the first trim/move/reverse will desynchronise the text from the
picture — the same choice of domain as for the automation keys (F-AUT-002), but
for a thousand words instead of a dozen keys. And second: "deleted a word" is a ripple
of the track to which the transcript belongs; what happens at the same time to
B-roll and music above it, each decides for himself, while undo must roll back the text and the
edit in ONE step, otherwise they drift apart.

## Pros

- A line is a data record, not a source of a frame: editing the text does not touch
  the render graph and does not invalidate the frame cache, so proofreading forty lines
  costs forty field edits, not forty redraws.
- One video render serves N languages: sidecar files can be attached
  in any number, and the viewer switches between them. Burning in multiplies the renders by
  the number of languages.
- Text as data allows DELIVERY to be CHECKED: 42 characters per line, no more than two lines, up to 20 characters per second (children's programmes — 17). These are checkable numbers from the broadcast regulations, not matters of taste; such a check cannot be done by pixels at all.
- A word-by-word transcript makes the edit addressable as text: "cut out the stumble" is
  one operation on a range of words instead of a trim plus a shift of the neighbours.
- A separate data track does not take part in the composite, so its scrolling and
  editing generate neither decode nor redrawing of the picture.

## Cons

- Three outputs — three different truths. Larry Jordan explicitly notes for Resolve a mismatch between what is shown and what is burned in, because of which the decision about a line break
  has to be made blind; FCP advises stripping formatting on SRT export, because it "may not appear correctly in your intended
  player".
- The timing of lines lives in milliseconds (SRT/VTT), the edit — in frames. At 23.976/29.97 the boundary of a line never falls on a frame boundary, and
  "off by a frame" is not an import bug but an unavoidable rounding. The visible symptom is the choice in FCP on import: "Relative to Timeline" (add the start of the project) versus an absolute one.
- Subtitles-as-titles clutter the media pool with one object per line, and cannot be replaced on repeated generation (see the quotes).
- Text editing is rigidly tied to the state of the timeline on which the transcript was made: cut the events — generation stops working, the transcript is rebuilt, the proofreading is lost.
- Automatic transcription means weights: from 78 MB to 3.1 GB per model, separately per language, and the quality directly turns into hours of proofreading. The limitations can be harsher than they seem: FCP "requires a Mac with Apple silicon" and "U.S. English only", Vegas — subscription only.
- CJK/RTL run into not the logic but the font and shaping: in Vegas Korean text comes out as question marks and breaks the presets, and this is a complaint about the titling system, not about recognition.

## What users say

> I just now tried AI Speech to Text–WORTHLESS!! Testing this on both the
> Small Good Quality model AND on the Large V2 model on several events, I
> either get only a couple of words detected, or, on one transcription, I even
> got what looked to be JAPANESE CHARACTERS!!
> — richkut, Boris FX forum (Vegas Pro), 19.04.2026,
> https://forum.borisfx.com/t/ai-speech-to-text-disappointing/22602

> But, you have to make sure that you have deleted all the previous title
> cards in Project Media, or else you'll have competing subtitles (they will
> superimpose upon each other).
> — kevin.vonduuglasittu, Boris FX forum, 21.04.2026,
> https://forum.borisfx.com/t/problems-editing-ai-speech-to-text-subtitles-using-vegas-pro-23/22688

> I'll double click on words in 'Subtitle' view and edit them…and then 2
> minutes later they have reverted.
> — kevin.vonduuglasittu, same place, 21.04.2026,
> https://forum.borisfx.com/t/problems-editing-ai-speech-to-text-subtitles-using-vegas-pro-23/22688

> when I tried to edit my first TikTok video with the software, the subtitle
> generation didn't work after I cut my different clips.
> — AlexandreTHEGAMER, Boris FX forum, 11.04.2026,
> https://forum.borisfx.com/t/my-subtitle-generation-doesnt-work-on-vp-2026/22213

> I am using speech to text to create subtitles. The spoken text in the video
> is korean and the machine I am working on is a german windows PC.
> — walter.k, Boris FX forum, 16.05.2026 (further in the same post: some of the
> characters are shown as question marks),
> https://forum.borisfx.com/t/speech-to-text-missing-characters-shown-as-question-marks/23543

> At the moment, VEGAS Pro effectively does not have a modern title system
> that fully supports Korean text properly.
> — storyyou, Boris FX forum, 08.05.2026 (in the Boris FX reply: a prototype of a new
> titler existed, and Title Studio was temporarily switched on instead),
> https://forum.borisfx.com/t/about-vegas-pro-text-systems-concerns-from-a-korean-user-protype-titler-titles-text-and-bcc-title-studio/23289

> I chose Premiere for its accuracy and ease of correction. Final Cut is
> simply inadequate and Resolve goes out of its way to make the process
> excessively cumbersome.
> — Larry Jordan, a comparison of captioning in FCP 11 / Premiere 2025 /
> Resolve 20, 16.08.2025,
> https://larryjordan.com/articles/comparing-captioning-in-final-cut-pro-11-premiere-pro-2025-resolve-20/

What I did NOT manage to find: a link "switched to another editor because of text editing" and
complaints about burn-in of the kind "burned it in and could not fix it" — r/editors,
r/premiere and discussions.apple.com do not open from this environment, search of the
Blackmagic forum (`search.php`) returns 403 to a guest, and creativecow had no working search URL.
Everything found live is from the Boris FX forum (Vegas), where the complaints concern automatic transcription and the titling system, not the
subtitle track as such.

## How it fits Kinotrain

### What already exists

- **Timed named records and their round-trip.** `Marker { t, duration,
  name, color }` and `MarkedArea { name, start, duration, lanes, color }` —
  `src/libs/kt-model/src/project.rs`. Markers go into OTIO as `Marker.2`
  (`src/libs/kt-otio/src/convert.rs`), areas — into the `metadata.kt` envelope
  (`src/libs/kt-otio/src/meta.rs`, `KT_META_VERSION = 4`), tests 89/90.
- **A ready UI precedent, the "strip of labelled ranges"**:
  `marker_band_rect`, `pick_range_marker`, inline editing of the name
  (`marker_edit`), dragging of the edge (`Drag::MarkerEdge` →
  `Command::SetMarkerDuration`) — `src/apps/kt-app/src/timeline.rs`, test
  170. We already know how to draw and drag a strip of lines.
- **A deterministic text rasteriser INSIDE the engine already exists** —
  `src/libs/kt-media/src/placeholder.rs`: a built-in 3×5 pixel font,
  deliberately independent of both system fonts and egui, "so the frame is
  deterministic and identical in the preview, the export and the tests" (test 208). This is
  also the precedent of "a source that draws the frame itself" — the placeholder is an ordinary
  `MediaSource` and lives under the same MediaWorker (invariant 5).
- **RTL experience**: `src/apps/kt-app/src/i18n/bidi.rs` (reordering of the glyph atoms by UAX#9, test 126) and 5 catalogues `apps/kt-app/i18n/`.
- **The muxer**: `src/libs/kt-ffmpeg/src/muxer_impl.rs`. The types `AVSubtitle` /
  `AVSubtitleRect` are ALREADY generated by bindgen (`allowlist_type("AV.*")` in
  `src/libs/kt-ffmpeg/build.rs`, the headers are vendored for FFmpeg 6/7/8).

### What is missing

- A text rasteriser with real fonts — shaping, line breaking,
  RTL, outline. This is F-TXT-001, and it must not be opened separately (rule 17
  of the checklist: two ways of putting text on screen is a forbidden pair).
- A list of lines as a model: right now there is none at all, and markers are not one
  (see below).
- A fifth kind of track: `LaneKind` is exactly `Video | Audio | Morph | Camera`
  (`src/libs/kt-model/src/project.rs`), they are known to `normalize_lane_order`
  (test 49), the order of rows goes through undo, and `MarkedArea.lanes` stores
  semantic keys `(LaneKind, u32)`.
- A layer drawn by us: `LayerSrc` in
  `src/libs/kt-engine/src/video_plan.rs` rigidly holds `SourceId` +
  `cache_idx` in the burst cache; there is no branch for "a layer from a ready RGBA picture".
- A parser/writer of SRT and VTT and a subtitle stream in MP4: in `api.rs` there is no
  `avcodec_encode_subtitle`, and `avformat_new_stream` is called in the muxer exactly
  twice — for video and audio.

### What gets in the way

- **Honestly about markers: as UI they are a precedent, as data they do not fit.**
  `Project::normalize()` does `markers.sort_by_key(t)` and
  `markers.dedup_by_key(t)`: the identity of a marker is its tick, therefore two lines
  on one tick do not occur and command addressing goes by time
  (`Command::MoveMarker { from, to }`). The names of range markers and areas
  lie in ONE namespace with a uniqueness check
  (`ProjectError::DuplicateName`) — two subtitles "Yes." would violate it. The name
  is single-line, without line breaks, style or position, and is drawn on the time-label row. Plus, markers are the targets of the magnet (`snap_targets` in
  `src/apps/kt-app/src/timeline.rs` builds a `Vec<i64>` per call): a thousand
  lines of a forty-minute film would turn this into an allocation per frame, which is directly
  forbidden by invariant 12. Conclusion: lines are their own collection with an id and their own
  track, not an extension of `Marker`.
- **Fonts are the main obstacle, and an unexpected one.** The shipped CJK font is
  a SUBSET of the interface catalogues: "codepoints 522 (from i18n/*.toml)"
  (`src/apps/kt-app/assets/SUBSET-MANIFEST.txt`, built by
  `src/scripts/subset-fonts.py`, test 100). Any Chinese subtitle outside the UI vocabulary will
  produce squares — that is, exactly the defect that Korean users of Vegas complain about. System fonts cannot be taken:
  invariant 11 requires byte-for-byte video against a committed reference, and a system font makes
  the reference machine-dependent. So either the full font goes into the repository
  (Noto CJK ~19 MB), or the burn-in is honestly limited to the shipped set.
  Today's single rasteriser inside the engine solves this in its own way: `placeholder.rs` draws everything outside the table as "?".
- **Invariant 7 (the composition code is one).** Burn-in must NOT be done "in a GPU
  shader, and differently on the CPU": this is exactly the class of divergence because of which the preview and the export would show different things. A precedent for the decision already exists —
  item 250 (blending of frames is done in the COMMON code of the plan, not in the executors),
  and from it one can see that it is logical to rasterise a line once into RGBA and
  hand the same layer to both backends. The concrete method is a matter for the round.
- **Invariants 9 and 10 at export.** The sidecar SRT/VTT we write ourselves — it is an
  ordinary file, with no dependencies. The embedded stream must go through the
  dlopen-ed system libavformat: in MP4 from text subtitles practically the only
  choice is `mov_text` (3GPP Timed Text, `tx3g`); one needs either a new symbol in
  the vtable of `src/libs/kt-ffmpeg/src/api.rs`, or assembly of the samples by hand.
  Whether FFmpeg's mp4 muxer will accept a stream without an encoder description — [NOT
  VERIFIED]. CEA-608/708 drop out of this list entirely: their place is inside the
  encoded video, and our encoder is chosen automatically from five
  candidates (nvenc/vaapi/qsv/x264/mpeg4), and inserting its own
  side data into each one is a separate large story.
- **Speech-to-text.** Weights from 78 MB to 3.1 GB per model, separately per language,
  plus the inference runtime. A project that does not even link FFmpeg but loads it
  through dlopen (invariant 10) cannot embed this in the binary. There are two realistic
  options: (a) import of an SRT made outside — precisely what is advised on the Vegas forum in place of built-in transcription; (b) an optional external
  engine on the same principle "if it is in the system — we use it". We will not have our
  own model, and this must be said directly, not implied.
- **UI rules 13 and 20.** The subtitle track is one more row in an already dense
  timeline; it must not grow by itself (rule 20) and must not take up room when there are no subtitles
  in the project (rule 13).

### Which invariants it touches

7 (burn-in is the common code of the plan, not a second path), 8 (the strip of lines is data,
it must not generate decode), 9 and 10 (we write the sidecar ourselves; the stream — only through
the dlopen-ed FFmpeg, no new libraries are to be added), 11 (determinism of the raster →
the font is vendored, system ones are not taken), 12 (a thousand lines in the draw path and in
`snap_targets` — without allocations and with culling by the viewport). UI checklist
rules: 13 (room), 17 (titles and subtitles must not become a pair of ways
to do the same thing), 20 (the track does not grow by itself).

## Estimate

| What | Answer |
|---|---|
| Volume | L — a whole round, and even without automatic transcription |
| Where the main work is | kt-model (the collection of lines + a fifth `LaneKind` + commands/undo), kt-app (the timeline row, the text editor, line-length checks), kt-engine (the burn-in layer in `video_plan`), kt-ffmpeg (the subtitle stream), kt-otio (saving), kt-media (font/raster on the model of `placeholder.rs`) |
| Project format | needs `KT_META_VERSION` v5: a new collection + a new kind of track in `lane_order` (now v4, `src/libs/kt-otio/src/meta.rs`) |
| Tests that will have to be written | round-trip SRT/VTT (millisecond ↔ tick is exact: 90000/1000 = 90, divisions without remainder); quantisation of the line boundary to the fps grid (that very "one-frame error"); a golden frame of a burned-in subtitle (CPU reference + GPU tolerances, invariant 7); an otio round-trip with a fifth track; `lane_order` with the new kind (an extension of test 49); a harness test of the subtitle row and glyph coverage (an extension of test 100) |
| Depends on | F-TXT-001 (titles and the text rasteriser) — nothing can be done without it |
| Breaks compatibility | no: old projects open, new ones with subtitles will not open in the old build (the check `check_kt_version`) |

## Verdict

We take it, but cut up and in the right order. First — the LIST OF LINES as an independent collection with an id and its own track, plus SRT and
VTT import/export: this is value that needs not a single pixel of text, and it honestly
rests on the timing records and the OTIO envelope already in place. Second — burn-in, and only on top of the F-TXT-001 rasteriser, with one
plan code (invariant 7) and with a vendored font (invariant 11). Third — an embedded
`mov_text` in MP4, when there is demand for it.

Automatic transcription and text editing in this form we do NOT take: the model weights
(78 MB…3.1 GB) are incompatible with the principle of "we do not drag anything inside", and without
our own recognition text editing degenerates into "import someone else's SRT and
cut along it" — which is precisely what should be supported in the first step. Range markers
cannot be reused for lines: deduplication by tick and a shared unique
namespace with areas are not a minor limitation, but a different model of
identity.

## Sources

- https://support.apple.com/guide/final-cut-pro/intro-to-captions-ver00e40835d/mac (accessed 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/generate-subtitles-automatically-verd6d52c4e8/mac (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/import-closed-captions-ver4185ef95a/mac (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/export-closed-captions-ver245c7581b/mac (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/8-design/speechtotext.htm (2026-09-07)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/1-before/whatsnew.htm (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/edit (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/whatsnew (2026-09-07, the Resolve 21 page)
- https://community.adobe.com/announcements-727/what-s-new-in-adobe-premiere-26-3-june-2026-1628369 (2026-09-07)
- https://larryjordan.com/articles/comparing-captioning-in-final-cut-pro-11-premiere-pro-2025-resolve-20/ (2026-09-07)
- https://larryjordan.com/articles/create-animated-captions-using-davinci-resolve-20/ (2026-09-07)
- https://partnerhelp.netflixstudios.com/hc/en-us/articles/217350977-English-Timed-Text-Style-Guide (2026-09-07) — the numbers of the regulations: 42 characters per line, at most two lines, 20 (17 for children's programmes) characters per second
- https://en.wikipedia.org/wiki/CEA-708 (2026-09-07) — how embedded captions travel inside the picture
- https://en.wikipedia.org/wiki/MPEG-4_Part_17 (2026-09-07) — `tx3g` in MP4
- The Boris FX forum (Vegas Pro), threads 22602, 22688, 22213, 23543, 23289 (2026-09-07)

Caveats. The Premiere row is compiled from the official 26.3 announcement in the Adobe community and from the comparative review by Larry Jordan: `helpx.adobe.com`,
`adobe.com/products/premiere*` and `blog.adobe.com` did not open from this environment, so the wording about local transcription and the list of
formats is a secondary source, not an Adobe help page. The After Effects row is the absence of the feature by indirect data, not confirmed by the help [NOT
VERIFIED]. Resolve was checked against the product page and the "What's New" of the 21st
version; "AI Animated Subtitles" under that name could not be found on the Blackmagic site — the description of animated subtitles is taken from the Larry Jordan review of
Resolve 20 [NOT VERIFIED as the official feature name]. None of the five
editors was run live. Search: the `WebSearch` budget in the session is exhausted, direct URLs and search engines through `WebFetch` were used; DuckDuckGo returned a CAPTCHA,
Brave/Ecosia/Yandex — 403/429/CAPTCHA.

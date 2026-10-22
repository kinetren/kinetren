---
id: F-MED-006
title: Several audio streams in one file
status: candidate
priority: B
cost: M
category: media/sources
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-ffmpeg, kt-media, kt-model, kt-otio, kt-cache, kt-app]
invariants: [4, 5, 8, 10, 11]
ui_rules: [4, 13, 18, 20]
round: null
point: null
tests: []
depends_on: []
trackers: [losslesscut, kde-bugzilla, kde-invent, olive, flowblade, pitivi, otio]
checked: 2026-09-08
---

# F-MED-006 — Several audio streams in one file

## Essence

One file carries not one audio track but several INDEPENDENT ones: each has
its own codec, its own channel layout, its own language, its own microphone. A
gameplay recording from OBS — the game in the first stream, the microphone in the
second; a broadcast MXF — eight mono tracks; a film — the dub and the original. The
editor needs to say which stream they are listening to, and sometimes to listen to
two at once. This is not channel layout (its owner is F-AUD-006): there a pair is
ASSEMBLED from the channels of one stream, here the stream itself is chosen first.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Stream (a property of the EVENT) | RMB on the event → Stream → a submenu of streams. "When you add a multistream audio file, you can choose which stream to use. Right-click the event, choose Stream from the shortcut menu, and select a stream from the submenu." One event = one stream; two streams at the same time = two events of the same file on two tracks |
| Adobe Premiere | 26.x | no separate mechanism — the streams are merged into the channels of the master clip | For MXF/MOV all streams are shown as one flat list "Media Source Channel" in Modify → Audio Channels; for MKV they are not shown at all — the first one is taken (Adobe community, 2020–2025). The official workaround on the forum is to extract the tracks to WAV and sync them by hand |
| DaVinci Resolve | 21 | Clip Attributes → Audio, the sources are called "Embedded Channel N" | the streams of a file are unfolded into a FLAT list of embedded channels; the editor assigns which embedded channel feeds which clip track, and edits this on several clips of the media pool at once. The word "stream" does not appear in the interface |
| Final Cut Pro | 12.3 | Audio Configuration → audio components | the same flat model: "Final Cut Pro automatically groups channels into audio components", "The number of audio components you see corresponds to the number of channels you've configured in your source clip". Separately: MKV is not imported at all, so the most widespread consumer source of multi-stream files does not reach FCP |
| After Effects | 26.x | none | one stream is taken, there is no choice; the official recommendation of the Adobe community is to extract the tracks in Premiere or Audition and import them as separate files |

**Outside the core.** Without Kdenlive this feature cannot be described: it is the only
publicly traceable program where the stream is chosen AS A STREAM, rather than dissolved
into a list of channels — and the only one where one can see what it cost. Without
OpenTimelineIO one cannot describe "What gets in the way": there is no stream addressing
in the format itself in which we write the project.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 | Clip Properties → Audio tab, "Audio streams" | a list of the file's streams, each switched on separately; the setting "Automatically import all streams in multi-stream clips" — "If checked, all video and audio streams of a clip are imported… If unchecked, the first video and audio streams are imported". The documentation is forced to open its section with the phrase "There is a difference between audio streams and audio channels" |
| OpenTimelineIO | 0.18.x | none | `ExternalReference` addresses the file as a whole (`target_url`) and cannot name a stream inside it; a request for this has been open since 09.08.2017 (#145, 26 comments) |

## The trick behind it

**First: by default the "best" stream is taken, not the "first" one — and the best one
by someone else's heuristic that does not exist in the project.** All the FFmpeg-based
editors (Kdenlive, Shotcut, Olive, LosslessCut and we) call `av_find_best_stream`, and
its own header describes the choice this way: "The best stream is determined
according to various heuristics as the most likely to be what the user
expects". That is, the winner is not the order in the file but the internal scoring of
libavformat: the `DEFAULT` disposition, the bitrate, the presence of a decoder. Hence two
things that are not in any feature list. The first: the Kdenlive documentation promises
"the first video and audio streams are imported", while the library beneath it promises
"best" — and on a file where the second stream is marked `DEFAULT`, the second one wins.
The second: the result depends on the library version, not on the content of the project.
There is nothing to notice the substitution by — the sound is there, it is simply not the
right one, and Premiere admits this directly: "There is no
straightforward way to verify the audio track structure within Premiere
itself" — people open the file in VLC to find out what is inside.

**Second: two incompatible devices, and the question itself splits along them.**
In Vegas a stream is a property of the EVENT (RMB → Stream), so "hearing both microphones
at once" is expressed by two events of one file on two tracks, without re-importing and
without a second item in the bin. In Premiere, Resolve and FCP there is no concept of a
"stream" in the interface at all: all streams are laid out in advance as a flat list of
channels ("Media Source Channel", "Embedded Channel N", "audio components"), so "both at
once" comes for free, while the question "which stream is this?" has no answer in
principle. This is exactly the fork because of which the request "let me choose the audio
stream" is closed with a link to channel mapping (kde-invent #382 → #754) in some editors,
and in others it is not even formulated: a user of a flat list does not know that they
have streams at all.

**Third: unfolding all streams at once is not a gift but a debt, and this shows in the
default of the only program that can do both.** In Kdenlive "Automatically import all
streams in multi-stream clips" is OFF by default: a recording from OBS with six tracks
would otherwise turn into six clips on six tracks on every drag. And the second half of
the same story: the flagship request "[Feature Request] MultiTrack Audio Support"
(kde-invent #382, 41 comments) was not finished — it was REWORDED:
#754 was opened 11.07.2020, #382 was closed 28.07.2020, and the checklist of five
items that moved to #754 has not a single tick mark six years later.

## Pros

- Choosing a stream does not require a new subsystem: `av_find_best_stream` has
  `wanted_stream_nb` as its very first parameter, which today is everywhere −1.
  The mechanism already exists; only the place where the number is stored is missing.
- A stream is independent as a whole: its own codec, its own sample rate, its own channel
  layout. Therefore choosing a stream and channel layout (F-AUD-006) are
  SEQUENTIAL operations, not competing ones: first "which of the six", then
  "how to mix its channels into a pair". Neither cancels the other.
- In Vegas a stream on an event gives what the flat channel model does not give at all:
  two copies of one file with different streams live side by side on the timeline, and the
  second one does not cost a second bin item.
- The flat model (Premiere/Resolve/FCP) instead gives "everything at once and without
  questions": there is never a "re-import with a different checkbox", because everything
  is imported.

## Cons

- "Took the wrong stream" is indistinguishable by ear from "took the right one": there is no
  silence, no error, the meters move. The class is discovered at delivery, not at import —
  and therefore does not generate a stream of duplicates in the trackers (see the numbers
  below), which is why it looks rare and is not fixed.
- Unfolding all streams clutters the timeline exactly in proportion to their number, so
  even where it exists it is switched off by default, and the user gets "silently take one"
  in a program that can do better.
- The stream choice is state that has to be stored somewhere, and it is lost at every
  boundary: on re-import, on re-linking the file, on passing the project to another tool.
  The request "remember my choice" comes to LosslessCut twice, eight years apart (#85, 2018 → #2990, 2026).
- Everything derived from the sound (waveform, peak pyramid, track thumbnails) is computed from the
  SELECTED stream, but stored per file. Changing the choice must reset them; when this is not
  done, we get kde-bugzilla #501280 "Audio thumbnail wrong for second audio track in mkv
  with two audio tracks".
- The word "stream" has lost to the word "track" to such an extent that the requests
  are phrased about something else, and parsing a request costs more than fulfilling it.

## What users say

> Files with multiple audio tracks only use 'the first' one and there is no
> way to select an alternate audio track.
> — Flowblade #325, opened 01.02.2017, closed with label `wontfix`, last edit
> 13.03.2026, https://github.com/jliljebl/flowblade/issues/325

> I record the desktop audio (e.g. from a DAW) into track 1, and voice over
> from my mic into track 2. […] both tracks will (incorrectly) play the
> first audio track.
> — Olive #1754, 30.09.2021, label High Priority,
> https://github.com/olive-editor/olive/issues/1754

> I would like to have a select option for video sources with multiple audio
> streams, to enable or disable the streams is want to use.
> — Olive #225, 31.12.2018, https://github.com/olive-editor/olive/issues/225

> Video-File with multiple Audio Streams uses only the first one regardless
> of choice
> — Kdenlive, kde-bugzilla #475311, 07.10.2023, closed as DUPLICATE of
> #475233, https://bugs.kde.org/show_bug.cgi?id=475311

> I record my microphone and the game's audio in two different audio tracks,
> and every time I edit a new clip I need to select both audio tracks to
> make sure I don't cut out any important audio.
> — LosslessCut #2990 "[feature request] remember audio track selection",
> 31.07.2026, open, https://github.com/mifi/lossless-cut/issues/2990

> Dropping an .MKV video clip with multitrack audio embedded onto the new
> item icon creates a sequence that propagates the video track and all of
> the audio tracks that were recorded.
> — Adobe Premiere, official community, 10.09.2025,
> https://community.adobe.com/t5/premiere-pro-discussions/is-there-a-preferences-fix-for-mkv-files-with-multitrack-audio-yet/m-p/15499109

> it will likely be necessary to identify a specific stream within a file,
> or even a specific channel within an audio stream.
> — OpenTimelineIO #145 "Media Reference Stream/Channel Specification
> support", opened 09.08.2017, 26 comments, last edit 21.09.2023,
> https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/145

SIGNAL: choosing an audio stream inside a file — 9 tasks in losslesscut, `multi.?track audio|multiple audio (track|stream)|(second|other|alternate|another) audio (track|stream)|audio (track|stream)s? (select|switch|chooser|picker)|select(ing)? (the )?audio (track|stream)`; share 0.5% against 0.2% in kde-bugzilla

SIGNAL: the word "stream" has lost to the word "track" — 72 tasks in kde-bugzilla, `audio tracks?\b`; share 1.1% against 0.1% for the filter `audio streams?\b` in the same tracker (8 tasks) and 0.2% in pitivi

The first number is read as follows, and it does NOT read in Kdenlive's favor. LosslessCut
does not edit anything at all — it cuts without re-encoding and keeps all the streams.
The topic costs it twice as much as a full-fledged editor, because there the user HEARS
the substitution immediately: the file keeps six tracks, while the player plays one, and the
discrepancy between "what I hear" and "what I will get" is loud. Kdenlive has the mechanism
built (a list of streams in the clip properties, "separate channels", MLT `audiomap`) — and
its share is half as LOW not because it is better, but because in an editor the wrong stream
is discovered later. Confirmation: the shape of Kde's 14 tasks — three of them were closed as duplicates within 26
days (#475233, #475311, #476349 → #474895, autumn 2023), that is, the stream of complaints
arises only when the mechanism has BROKEN. While it simply "takes one", three people do not
independently arrive.

The second number concerns why the requests on this topic are phrased wrongly. In the
same kde-bugzilla the word "track" describes 72 tasks, the word "stream" — 8,
a difference of nine times; across ten trackers — 189 against 19. Because of this the
request "choose the audio stream" and the request "reassign the channels" arrive
in identical words, and maintainers pay for this: kde-invent #382
(about streams) and #754 (about channels) have lived as one chain for six years, and the
Kdenlive documentation is forced to open its section with the phrase "There is a
difference between audio streams and audio channels". We stand on the same
FFmpeg and will get the same confusion in feedback if we do not separate the words in the
interface ourselves — this is an argument for UI rule 18, not an ornament.

As for the requests themselves, they are the same for everyone and have not changed for eight years:
Olive #225 (2018) — "enable and disable the needed streams", Flowblade #325
(2017) — "no way to select an alternate track", LosslessCut #1074
(2022) — "it would be good to choose which track to play". Three of the four
open-source projects closed this; Flowblade closed it over nine years with the label
`wontfix`.

## How it fits Kinotrain

### What already exists

Our choice of audio stream happens in EXACTLY ONE place and without user involvement. When a
file is opened, the scan context calls
`av_find_best_stream(scan, AVMEDIA_TYPE_AUDIO, -1, -1, …)`
(`src/libs/kt-ffmpeg/src/source_impl.rs:161`); the same call is repeated
as a fallback in `resolve_stream_index`
(`src/libs/kt-ffmpeg/src/source_impl.rs:31`), which re-checks the index
on its own context — this function appeared not for the sake of choice but for the sake of
regression item 26: a foreign `stream_index` on a lazy ogg demuxer hit
`av_assert0` inside libavformat and killed the process. There is a test for this
(`src/libs/kt-media/tests/ffmpeg_conformance.rs:162`,
`t10_ogg_cover_art_no_abort_and_decodes`), and it is the only one in the tree that
deals with an extra stream in a file at all — and even that one is about cover art, not
sound.

The stream number does not go out and is not accepted in:
`kt_ffmpeg::Source::open(path)` (`src/libs/kt-ffmpeg/src/lib.rs:249`) and
`FfmpegSource::open(path)` (`src/libs/kt-media/src/ffmpeg.rs:28`) take
only the path. `MediaInfo` (`src/libs/kt-media/src/lib.rs:16`) knows exactly one bit about
sound — `has_audio`.

One MediaWorker per `MediaId` (`Player::add_media`,
`src/libs/kt-engine/src/player.rs:445`) — invariant 5 holds on its own, and the cache key
`BurstKey { source: SourceId(media.0), … }`
(`src/libs/kt-cache/src/lib.rs:44`, `src/libs/kt-engine/src/fetch.rs:18`)
already distinguishes bin items. The mechanism for resetting derived data when the source
changes is also ready and tested: `Player::invalidate_source`
(`src/libs/kt-engine/src/player.rs:467`, t211).

Separately: the model already allows "one file twice in the bin".
`add_media_clip_at` (`src/apps/kt-app/src/app.rs:3181`) each time yields a
new `MediaId` and does not look for an existing item by path, and
`Command::AddMedia` simply appends a `MediaRef` to the list
(`src/libs/kt-model/src/model.rs:761`).

### What is missing

1. A place where the stream number lives. `MediaRef { id, path }`
   (`src/libs/kt-model/src/project.rs:1777`) and its envelope in the format
   `KtBinMedia { id, path }` (`src/libs/kt-otio/src/meta.rs:138`) carry nothing about the
   file except the path; the envelope version is currently
   `KT_META_VERSION = 5` (`src/libs/kt-otio/src/meta.rs:17`).
2. Knowledge that there are several streams. `Probe`
   (`src/libs/kt-ffmpeg/src/lib.rs:55`) returns `has_audio: bool` and does not
   return the number of streams, nor their languages and codecs. Until this exists, the UI
   cannot even say "there are six in the file", so the impossibility of choosing
   cannot be shown as an error (UI rule 16).
3. Layouts of one file across several clips. `place_media_at`
   (`src/apps/kt-app/src/app.rs:3239`) creates EXACTLY ONE audio clip,
   because it decides by `info.has_audio` — a boolean.

### What gets in the way

**The interchange format.** We write the media path into the standard
`ExternalReference.target_url` (`src/libs/kt-otio/src/convert.rs:603`), and there is nowhere
to put the stream number: OTIO cannot do this, and request #145 has been open since
2017. So the number will leave in `metadata["kt"]` — and a project opened
by a foreign OTIO tool will silently fall back to the "best stream". This is not our
shortcoming, it is the price of the format, and it must be stated in a note, not
discovered in a round.

**The sidecar DB is keyed by PATH, not by bin item.** `Index::ensure_media`
looks up the row `WHERE path=?1` with `path TEXT NOT NULL UNIQUE`
(`src/libs/kt-media/src/index.rs:151`, the schema is there too, lines 101–122), and
the peak pyramid lives in `audio_peaks(media_id, level, channel)` keyed by this same
`media_id`. The waveform reader goes the same way:
`db.ensure_media(Path::new(&path))` (`src/apps/kt-app/src/util.rs:115`), and
a repeated build is suppressed by the `complete` flag. The consequence is exact: two bin
items pointing to one file with different streams will get DIFFERENT
`SourceId`s in the cache (invariant 4 holds) and ONE `media_id` in the sidecar DB —
the waveform for both will come from whichever stream was computed first. This is verbatim
kde-bugzilla #501280, built into the design in advance.

**The cost of "adding the file twice".** Each `MediaId` has its own MediaWorker thread
and its own `Source`, and a `Source` holds TWO independent demuxers (audio and
video, `src/libs/kt-ffmpeg/src/source_impl.rs:255`). Six streams of one
file as six bin items means six OS threads and twelve demuxers per file. The Vegas model (the
stream on the event) does not pay this price, but it requires a field in the cache key:
`BurstKind::Audio` (`src/libs/kt-cache/src/lib.rs:19`) has no parameters, and adding
`Audio { stream }` is exactly the trade-off that F-AUD-006 already worked through
for channel layout and rejected.

**Nothing to verify it with.** There is no fixture with two audio streams in the tree
(`src/fixtures/gen.sh`, seven files, the most complex being an ogg with cover art),
so today even the current behavior is not covered: that we take the "best" one
is known from the code, not from a test.

### Which invariants it touches

- **Invariant 11 (determinism) — the main one.** Choosing the stream is now done by
  someone else's heuristic ("various heuristics", the libavformat header), not by the
  project file. Combined with invariant 10 (FFmpeg 6/7/8 is loaded via dlopen, and the major
  version is determined on the user's machine), this means: the same `.otio` on two machines
  may decode DIFFERENT sound, and no golden test will see this, because all our audio
  fixtures are single-stream. Writing the stream number into the project is not
  "convenience" but restoration of an invariant.
- **Invariant 10 (we do not link to libav*).** It also suggests the solution:
  since the library version is someone else's, we cannot rely on its choice; our choice
  must be a number in the project, and `wanted_stream_nb` is the place where this
  number is substituted.
- **Invariant 4 (the cache is the only truth about coverage).** Not touched,
  IF the stream remains a property of the bin item: `SourceId(media.0)` already
  distinguishes items. It is touched at once if the stream becomes a property of the clip
  (the Vegas model) — then a new key discriminator is needed.
- **Invariant 8 (waveforms from the pyramid, thumbnails from the sidecar).** Directly touched:
  the pyramid is keyed by path, not by bin item (see "What gets in the way").
  Without changing this place, changing the stream will show the old waveform — and that
  will not be "cosmetics" but a violation of the single source of truth.
- **Invariant 5 (one MediaWorker owns the decoder).** Not touched:
  one bin item = one stream = one worker. Several streams of one file at the same time means
  several workers for one file, which the invariant allows (ownership of the decoder, not of the file).
- **Invariant 1 (the path is always stereo).** Not touched at all: the stream choice
  happens BEFORE the mixdown point `av_channel_layout_default(2)`
  (`src/libs/kt-ffmpeg/src/source_impl.rs:491`), and what goes out is still
  only f32 48 kHz stereo.
- **UI rule 4 (the default is useful).** The default "take one stream"
  stays: unfolding six clips on every drag is not allowed
  (see "The trick behind it", item 3). But the default must be the FIRST stream, not
  "the best by someone else's count" — because the first is reproducible, while the best
  depends on the library version.
- **UI rule 20 (nothing grows by itself).** Automatic
  unfolding of all streams into clips is literally "growing by itself", and the rule forbids
  making it the default. It is admissible only as an explicit command over a selection.
- **UI rule 18 (the name says WHAT and WHERE).** "Stream 2" is a poor
  label. It must carry the language and the layout ("Stream 2 · rus · stereo"),
  otherwise we will reproduce for ourselves the very confusion of stream and channel whose
  cost is counted above.
- **UI rule 13 (do not waste space).** A separate media properties panel is not
  created for this: there is room on the bin card and in its context menu.

ADDRESS: `src/libs/kt-ffmpeg/src/source_impl.rs:161` — the class "took the first stream" is literally impossible for us: we take not the first but the "best" (`av_find_best_stream` with `wanted_stream_nb = -1`), and this is WORSE, not better, because the choice stops being a function of the file and becomes a function of the libavformat version on the user's machine — with invariant 10 (dlopen of FFmpeg 6/7/8) this is a direct gap in invariant 11, and not a single golden test catches it, since all seven fixtures in `src/fixtures/gen.sh` are single-stream. One more class is possible and not caught by anything — kde-bugzilla #501280 ("waveform from someone else's stream"): `Index::ensure_media` (`src/libs/kt-media/src/index.rs:151`) keys the peak pyramid BY PATH (`WHERE path=?1`, `path … UNIQUE`), and the waveform reader (`src/apps/kt-app/src/util.rs:115`) goes to the same place and suppresses the rebuild with the `complete` flag — so two bin items for one file with different streams will get one pyramid between them, and invariant 8 ("the strips come from the pyramid") will turn this into a silently wrong picture.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 round stages |
| Where the main work is | kt-ffmpeg (`wanted_stream_nb` instead of −1 and the number of streams in `Probe`), kt-model + kt-otio (a field on `MediaRef`/`KtBinMedia`, the envelope version), kt-media (the sidecar DB key must distinguish the stream), kt-app (the choice on the bin card, the label per rule 18) |
| Project format | needs version v6: the stream number is a new field of `MediaRef`; absence of the field = today's behavior. It does not fit into the standard part of OTIO (#145), it goes into `metadata["kt"]` |
| Tests that will have to be written | a fixture with two DIFFERENT audio streams (there is none today); "stream N selected — stream N is decoded" on all three FFmpeg majors; the choice travels through the model and OTIO round-trip; changing the stream invalidates the audio strip of the cache AND the peak pyramid; two bin items for one file give DIFFERENT waveforms |
| Depends on | nothing. F-AUD-006 (channel layout) is the next operation on the same sound, not a competitor: first "which stream", then "how to mix its channels". Both touch `source_impl.rs`, so doing them in a row is cheaper than separately |
| Breaks compatibility | no: absence of the field = current behavior. But the current behavior itself is non-deterministic across FFmpeg versions, and fixing "first stream" as the default CHANGES it — deliberately |

## Verdict

**We take it — and we take it first not as a feature but as a repair of invariant
11.** Today the audio stream number is not stored anywhere, and it is chosen by the heuristic
of someone else's library, whose version is determined on the user's machine
(invariant 10). This means that the same project on two machines may sound
different, and no golden test will catch this: all the audio fixtures are single-stream.
The cheapest and most urgent part of the note is a fixture with two different streams and a test of
"which stream do we take" — it can be done before the feature is accepted as a whole.

**The stream is a property of the bin item, not of the clip.** Not to taste: `SourceId` in the
cache key is `MediaId` (`src/libs/kt-engine/src/fetch.rs:18`), so the
Premiere/Resolve model costs us nothing, while the Vegas model (the stream on the
event) would require `BurstKind::Audio { stream }` and would touch invariant 4.
The same trade-off was already worked through by F-AUD-006 for channel layout and rejected for
the same reason — two independent analyses converged, and this makes the conclusion stable. The
price of our choice is named honestly: "hearing two streams at once" is expressed only by
two bin items for one file, that is, by two workers and two pairs of demuxers.

**Unfolding all streams is a command, not a default** (UI rule 20).
The only program that can do both keeps this setting off by default, and its tracker explains why. Our default is the
FIRST stream: it is reproducible, while "the best" is not.

**And before all of this, the sidecar DB is fixed.** The peak pyramid is keyed by the file
path, not by the bin item; as long as this is so, any choice of stream
immediately gives a silently wrong waveform (kde-bugzilla #501280) and breaks
invariant 8. This is a condition, not a wish: without it the feature cannot be considered
done.

## Sources

Trackers (the registry and the way of export — `docs/features/_trackers.md`; dumps in
`~/.cache/kinotrain/trackers/`, not committed to the repository):

- https://invent.kde.org/multimedia/kdenlive/-/work_items/382 — "[Feature
  Request] MultiTrack Audio Support", opened 19.10.2019, CLOSED 28.07.2020,
  41 comments, labels Audio + Feature Request
- https://invent.kde.org/multimedia/kdenlive/-/work_items/754 — "Audio
  Routing | Channel Mapping", opened 11.07.2020, updated 22.06.2025,
  checklist of five items without marks (the owner of the channels topic is F-AUD-006)
- https://bugs.kde.org/show_bug.cgi?id=409739 (2019, FIXED) · =474140
  (FIXED) · =474141 ("User cannot change the audio stream a clip uses",
  UNCONFIRMED since 2023) · =474895 · =475233 · =475311 · =476349 (three duplicates
  within 26 days in autumn 2023) · =428607 (segfault on a clip with no enabled
  streams) · =494291 (UNCONFIRMED, updated 05.06.2026) · =501280
  ("Audio thumbnail wrong for second audio track", FIXED 2025)
- https://github.com/olive-editor/olive/issues/225 · /1754
- https://github.com/jliljebl/flowblade/issues/325
- https://github.com/mifi/lossless-cut/issues/85 · /1074 · /936 · /2990
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/145

Both numbers of the `SIGNAL:` lines are reproduced with `tracker-rank.py --query` with the same
filters. **State of the export as of 08.09.2026:** `blender-vse` and
`ffmpeg-trac` are empty, `openshot` has an incomplete dump (the state file shows
a restart from the first page with 4516 tasks already written) — therefore openshot does not take part in
the `SIGNAL:` lines; only trackers with `done: true` were used (losslesscut 1920, kde-bugzilla 6346, pitivi 2672).

Editors' documentation (access date 08.09.2026):

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/4-media/adding_mediafilesproject.htm
  — VEGAS Pro 2026, "Add media files to your project" (the quote about
  multistream and RMB → Stream is taken from here)
- https://support.apple.com/guide/final-cut-pro/audio-channels-and-components-verdcd140a/mac
- https://docs.kdenlive.org/en/project_and_asset_management/project_bin/clip_properties.html
  and https://docs.kdenlive.org/en/getting_started/configure_kdenlive/configuration_misc.html
- https://community.adobe.com/questions-729/importing-clip-with-multiple-audio-tracks-premiere-only-sees-one-1372047
  (the message of 21.11.2020 "only the first audio track is detected by
  Premiere"; replies of 26–27.08.2024 with workarounds)
- https://community.adobe.com/t5/premiere-pro-discussions/problem-re-exporting-mxf-op1a-xdcam-with-separate-mono-tracks/td-p/10798974
  — MXF OP1a "separate mono tracks" as a requirement of broadcast delivery and
  a remark that there is nothing to check the track structure with inside Premiere

The `av_find_best_stream` header is quoted from the VENDORED header in our tree,
not from the website: `src/libs/kt-ffmpeg/vendor/ffmpeg-8/include/
libavformat/avformat.h:2230`.

**What could not be verified.** The Resolve manual page
`steakunderwater.com/VFXPedia/__man/Resolve18-6/…/part1095.htm`, referenced by F-AUD-006, returns 404 from this environment, and the Blackmagic forum — 403; the Resolve row is compiled from secondary descriptions of Clip Attributes →
Audio ("Embedded Channel N"), not checked live. The Premiere and
After Effects rows are compiled from the official Adobe community and secondary
sources: `helpx.adobe.com` still opens from this environment only intermittently (the same limitation is
recorded in `_editors.md` for the first wave). `vegaspro.helpmax.net` did not open ("proxy refused the
connection") — the official Boris FX documentation for
VEGAS Pro 2026 was used instead.

---
id: F-AUD-006
title: Audio channel routing and layout
status: candidate
priority: B
cost: M
category: audio/mixing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-media, kt-ffmpeg, kt-cache, kt-otio, kt-app]
invariants: [1, 3, 4, 11]
ui_rules: [4, 18]
round: null
point: null
tests: []
depends_on: []
trackers: [kde-invent, kde-bugzilla, olive, losslesscut, otio]
checked: 2026-09-08
---

# F-AUD-006 — Audio channel routing and layout

## Essence

The editor says which channel of the SOURCE goes to which channel of the output. This is neither
volume nor panning: panning moves a finished pair left and right, whereas here the pair is first
ASSEMBLED from what was recorded. Three situations for whose sake the mechanism exists: the camera
has a lavalier on channel 1 and a shotgun microphone on channel 2 — only the first is needed, in
both ears; a mono file must sound in the center, not in the left ear; a five-channel source must be
reduced to stereo by an understandable rule, not by a silent default of the library.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Channels (a property of the EVENT) | right-click on an audio event → Channels. For stereo: "Both", "Left Only" (creates a MONO from the left), "Right Only", "Combine" ("creates mono by mixing channels with amplitude divided by two"), "Swap". For multichannel: "Channel x" — the event becomes mono from the selected channel, "Channels x/y" — stereo from the selected pair. "VEGAS Pro allows you to perform audio channel conversion nondestructively" |
| Adobe Premiere | 26.x | Modify → Audio Channels (a property of the MASTER CLIP in the bin) | in the project panel: Clip → Modify → Audio Channels. Set by the Clip Channel Format (Mono / Stereo / 5.1 / Adaptive), the number of clips and a matrix of checkboxes "Media Source Channel → output". The layout determines not only the sound but also how many audio tracks the clip occupies when placed in a sequence |
| DaVinci Resolve | 21 | Clip Attributes → Audio tab (a property of the MEDIA POOL clip) | right-click on a clip in the media pool → Clip Attributes → Audio: the format that Resolve determined itself, the number of tracks of the clip and which source channel feeds each one; a source channel can be set to Mute. Edited at once on several selected clips of the bin |
| Final Cut Pro | 12.3 | Audio Configuration (inspector, a property of the clip) | Channels: Mono / Stereo / Dual Mono / 5.1 surround. The layout generates "audio components": a stereo clip is one component, Dual Mono is two, and each has its own volume, panning, effects and role. More than six channels are automatically reduced to 5.1 on playback |
| After Effects | 26.x | none (only the Stereo Mixer effect) | footage has no layout — Interpret Footage knows nothing about channels. Reassignment is done by the Stereo Mixer EFFECT (Left Level / Right Level / Left Pan / Right Pan) on a layer, that is, it lives in the effects chain and travels with it, not with the source |

## The trick behind it

**First: the layout is a property of the SOURCE, and this is not a trifle of convenience but a
choice of who edits it.** Premiere and Resolve keep it on the bin clip ("master clip", "media pool
clip"), Vegas — on the timeline EVENT. The difference is visible on an interview: one hour-long file
with two microphones, cut into forty lines. In Premiere/Resolve the editor corrects the layout once
in the bin BEFORE cutting, and forty events inherit it. In Vegas each event is corrected; therefore
Vegas also demands doing the conversion before cutting, and if one notices after — the work is redone
forty times. The reverse side in Premiere: a clip already placed in a sequence remembers the old
layout, and after the master clip is edited the timeline and the bin diverge.

**Second: in FCP the price of the layout is written in the help text in plain words.** "Changing the
channel configuration resets all adjustments and edits made to audio components (including applied
effects, disabled ranges, keyframe adjustments, and role assignments)" — that is, changing the layout
erases effects, keyframes and roles. This is a direct consequence of the structure: a component is
derived from the layout, the layout changed — the components no longer exist, and together with them
everything that hung on them. Such a thing is not written in the list of features on the website.

**Third: the law of downmixing is nowhere named with a number.** The only place in the whole five
where it is said aloud is VEGAS's "Combine": "creates mono by mixing channels with amplitude divided
by two". For the rest, the downmix of multichannel to stereo is a library default which the editor
silently inherits. The default of libswresample, on which we also stand, does no normalization at all
for float output: L = FL + 0.707·FC + 0.707·BL, LFE is discarded. Measured on our own FFmpeg build —
a source whose peak on each of the six channels is 0.9998, after downmixing to stereo gives a peak of
**2.41** (+7.6 dBFS), and the user sees not a single line about it. It is exactly this, and not the
fact of downmixing itself, that is complained about in the trackers.

## Pros

- The layout of the source survives the cutting: an edit in one place in the bin rewrites all the
  events of this file, because the events do not store it — they INHERIT it. Forty lines of an
  interview against forty undo steps.
- Vegas's "Left Only" gives MONO, not "the left in the left ear": selecting a channel and spreading
  it over both outputs is one operation, because the layout describes the output as a whole, rather
  than muting half of the input. Panning cannot do this in principle — it can only reduce.
- Since the layout sets the number of outputs, it also sets how many tracks the clip occupies when
  placed (Premiere, Resolve). One mechanism answers both for the sound and for the placement — there is
  no need for a second setting of "how to lay out across tracks".
- The matrix "input × output" expresses everything at once: channel selection, channel duplication,
  channel swap and muting. The separate effects "Fill Left with Right", "Swap Channels", "Mono Downmix"
  are one and the same matrix, broken down into buttons.

## Cons

- An N×M matrix is unreadable exactly from the moment N > 2. That is why everyone hides it behind
  presets (Mono / Stereo / 5.1 / Adaptive), and a preset is a lie by default: it guesses the intent by
  the number of channels and guesses wrongly on two microphones (both tracks are "stereo", and the editor
  hears a mix of the lavalier with the shotgun).
- The layout BREAKS everything derived from it. FCP admits this openly and resets effects, keyframes and
  roles; the others stay silent, but the waveform, the peak pyramid and any cached samples go stale after
  a change of layout, equally for everyone.
- The property of the source does not survive the replacement of the source: "relinked the file — the
  layout returned to the guessed one" is a separate class of complaints, because the layout lives on the
  bin clip, and the bin clip refers to a path.
- The law of downmixing is not "the right one" but a chosen one. Any choice breaks someone's expectation:
  without normalization — clipping; with normalization — "the sound got quieter after export to mono".

## What users say

> Implement very simple functionality to re-map audio channels (e.g. left
> channel to left+right channel, swap channels, etc.)
> — Olive #1356, 18.11.2020, https://github.com/olive-editor/olive/issues/1356

> This is a common situation, you've recorded audio with a mono microphone
> onto a stereo track. This only fills one of the two stereo channels,
> commonly the left channel.
> — Olive #277, 08.01.2019, https://github.com/olive-editor/olive/issues/277

> MLT Audiomap is able to manage up tl 32 channels (same as all the
> professional applications).
> — Kdenlive, kde-invent #754 "Audio Routing | Channel Mapping", opened
> 11.07.2020, last edited 22.06.2025, 21 comments, labels Audio +
> Feature Request, https://invent.kde.org/multimedia/kdenlive/-/work_items/754

> It would be useful to be able to disable/remove audio channels for preview
> and export, in my case mostly to extract a background audio or a voice from
> a video file with 6 channels (5.1 audio) for example.
> — LosslessCut #2314, 05.02.2025, closed as `not_planned`,
> https://github.com/mifi/lossless-cut/issues/2314

> Inform the user that volume won't match the timeline when Downmixing to
> mono (happens when rendering a 2-channel timeline using a 1-channel preset)
> — Kdenlive, kde-bugzilla #516398, 20.02.2026, severity major, still
> UNCONFIRMED, https://bugs.kde.org/show_bug.cgi?id=516398

SIGNAL: the channel layout of the source — 28 tasks in kde-bugzilla, `(audio|sound).*(channel|stream)|channel.*(map|rout|swap|remap)|down.?mix|(mono|stereo).*(channel|mix)`; share 0.4% against 0.2% in olive

SIGNAL: "mono turned out only in the left" — 4 tasks on four different trackers, `(mono|1.?channel).*(left|right)|(left|right) channel only|fill (left|right) with|clone mono|empty stereo channel`; share 0.1% in losslesscut against 0.0% in kde-bugzilla

These numbers must be read like this. The first share is not "Kdenlive is worse": Kdenlive HAS channel
layout (selection of audio streams, "separate channels", MLT `audiomap`), and the 28 tasks are the score
for a built mechanism, of which 7 are still open, and among the closed ones half are closed by real fixes
(11 of 21). Olive has half the share with three tasks, and all three are closed `completed`: remapping
there was done at once in a single function (#1356), after which the stream of requests stopped. That is,
what is expensive is not "to have a layout" but to have it in pieces — exactly the state in which kde-invent #754 lives: the request has been open for six years, with a checklist of five items, none checked.

The second share is more important than the first in its form: one and the same complaint exactly once
in four trackers out of ten — Kdenlive (#491072, "1-channel wav (mono) audio is only played back on the left
channel", fixed), kde-invent (#301, the balance mutes the left channel), Olive (#277) and LosslessCut (#2390,
"clone mono audio to second channel to make it stereo", closed `not_planned`). Only once each — because this
is not a topic with a stream of duplicates, but a FORK OF THE DEFAULT: whoever treats mono as a double mono
has no complaints at all; whoever treats it as "mono into the left" gets the complaint once, and it lives
until the default is changed.

## How it fits Kinotrain

### What already exists

Downmixing to stereo exists with us, and it is exactly one for the whole program —
`AudioPath::open_decoder` in `src/libs/kt-ffmpeg/src/source_impl.rs:489`: the output layout is set as
`av_channel_layout_default(2)`, the input layout is taken from the decoder as is, and everything else is
done by the default matrix of libswresample. The comment there honestly names the reason — "invariant
No. 1: only this format goes outward". Further along the channel path there is no longer any concept of
it: `AudioBurst` is `[f32; 1024*2]` (`src/libs/kt-media/src/lib.rs:39`), the mix and FX see only the pair.

The choice of a STREAM (not a channel) also exists, but not as a decision of the user:
`resolve_stream_index` (`src/libs/kt-ffmpeg/src/source_impl.rs:31`) calls `av_find_best_stream`, and this
is the only way a file with two audio tracks enters the project. Neither `kt_ffmpeg::Source::open`
(`src/libs/kt-ffmpeg/src/lib.rs:249`) nor `FfmpegSource::open` (`src/libs/kt-media/src/ffmpeg.rs:28`)
accepts the stream number from outside.

Panning exists and is not routing: `FxInstance::Gain { db, pan }` (`src/libs/kt-model/src/project.rs:2182`)
is applied in `src/libs/kt-fx/src/lib.rs:828` by the linear law `pan_l *= 1−p`, `pan_r *= 1+p` with
clipping at 1.0 (item 228). The multiplier never exceeds one, therefore `pan`
can only REDUCE a side: "the left channel into both ears" is inexpressible with it, and "the left channel
in the left ear" is the only thing that can be obtained. We have only two audio effects (`Gain` and
`Slowed+Reverb`, `src/apps/kt-app/src/fx_registry.rs:52`) — there is nowhere to put routing even as an effect.

A place for the property of the source is ready and empty: `MediaRef { id, path }`
(`src/libs/kt-model/src/project.rs:1777`) and its envelope in the format `KtBinMedia { id, path }`
(`src/libs/kt-otio/src/meta.rs:138`) carry nothing about the file except the path. The bin panel
(`src/apps/kt-app/src/bin_panel.rs`) is a ready place for editing such a property, including editing
several selected elements at once, as in Resolve.

### What is missing

1. The number of channels of the source is shown nowhere: `MediaInfo` (`src/libs/kt-media/src/lib.rs:16`)
   knows `has_audio`, `fps`, `width`, `height` — and not a single field about sound except "present/absent".
   Until it exists, neither the UI can say "this file has 2 microphones", nor a check before render can
   warn about downmixing.
2. The layout itself as data: no field in `MediaRef`, no version of the format for it (now
   `KT_META_VERSION = 5`, `src/libs/kt-otio/src/meta.rs:17`).
3. The choice of the audio STREAM is half of the same task (kde-invent #754 and kde-bugzilla #474141
   "User cannot change the audio stream a clip uses" — it is about this), and with us it is closed by the
   same `av_find_best_stream`.

### What gets in the way

**The cache key.** `BurstKey { source, kind: BurstKind::Audio, idx }` (`src/libs/kt-cache/src/lib.rs:44`),
and `source` is `SourceId(media.0)` (`src/libs/kt-engine/src/fetch.rs:18`), that is, the identifier of the
BIN element, not of the clip. The audio bursts of one file are common to all its clips by construction.
A direct consequence follows, which must be accepted consciously: the layout with us is a property of the
SOURCE (the Premiere/Resolve model) not because it is more convenient, but because it is not expressible
otherwise without a new field of the key; a clip-level layout (the Vegas model) would cost a splitting of
the audio cache and would hit invariant 4.

**Data derived from the channels.** The peak pyramid is computed from already downmixed bursts
(`src/libs/kt-media/src/index.rs:379`) and lies in the sidecar DB as the table `audio_peaks(media_id, level,
channel)` (`src/libs/kt-media/src/index.rs:116`) — by media_id. A change of layout must invalidate it,
otherwise the waveform will show the previous layout (invariant 8: waveforms come from the pyramid, not from
decoding). The mechanism for this already exists and is proven on the replacement of a source —
`Player::invalidate_source` (`src/libs/kt-engine/src/player.rs:467`, t211).

**Export.** The muxer opens the encoder rigidly on `AUDIO_CHANNELS` (`src/libs/kt-ffmpeg/src/muxer_impl.rs:181`).
This does not hinder the feature — it concerns the INPUT — but it closes the temptation of "since we're
touching the channels anyway, let's make 5.1 on output": that is another note and another price.

### Which invariants it touches

- **Invariant 1 (audio always f32 48 kHz stereo).** The feature does NOT REQUIRE widening it, and this is the
  main conclusion of the section. There are two ways. The first — widen the path to N channels and downmix at
  the output: this rewrites `AudioBurst`, the cache, the mix, FX, the meters and the muxer, that is XL and a
  break of invariant 1 at once. The second — keep the downmixing point where it is now, and replace its rule:
  instead of the default libswresample matrix — a matrix chosen by the user, via `swr_set_matrix` or by manual
  downmixing right after decoding in `src/libs/kt-ffmpeg/src/source_impl.rs`. The second way is cheaper by an
  order of magnitude, does NOT touch invariant 1 at all (outward still only 48 kHz stereo comes out), and covers
  all three situations from "Essence". The only thing it does not give is 5.1 ON OUTPUT; nobody asked for that.
- **Invariant 3 (a model edit does not kill FX state).** In FCP a change of layout resets effects and keyframes;
  this is forbidden for us. The layout lives on the source, not on the chain, so the diff of chains by clip id
  does not see it — but a `reset()` of buffers on a change of layout is needed just as on a seek: past the
  downmixing point the signal changes, and the reverb tail from the previous layout is an artifact.
- **Invariant 4 (the cache is the only truth about coverage).** A change of layout = invalidation of the audio
  band of the source, not a separate bookkeeping.
- **Invariant 11 (determinism).** The downmix matrix must be part of the project, not a property of the machine:
  otherwise GOLDEN_AUDIO depends on which version of libswresample is installed on the builder's machine. This,
  by the way, is an argument for computing the matrix OURSELVES, rather than handing it to swr.
- **UI rule 4 (the default must be useful).** The default for mono is "into both ears", and with us it is already
  like that; for a two-channel file the default "both channels as stereo" also remains, because "two tracks = two
  microphones" is a guess, not a fact.
- **UI rule 18 (the name says WHAT and WHERE).** "Left Only" is a bad name: it does not say that the result will be
  centered. Our item must sound like "Channel 1 into both ears", otherwise we reproduce kde-bugzilla #491072 at the
  level of the label.

ADDRESS: `src/libs/kt-ffmpeg/src/source_impl.rs:489` — the class "mono sounds only in the left" (kde-bugzilla #491072, olive #277, losslesscut #2390) is structurally impossible with us, because the mono stream does not reach the engine at all: `swr` spreads it over both channels already inside the source with coefficient 1.0 (verified on our fixture `src/fixtures/out/sine44k_mono.m4a`: max|L−R| = 0.0). But the class of kde-bugzilla #516398 IS possible with us, and NOTHING catches it: the same point downmixes 5.1 by the libswresample matrix without normalization for float output (L = FL + 0.707·FC + 0.707·BL, LFE discarded), the peak after downmixing reaches 2.41 with an original 0.9998, and there is not a single `clamp` in the audio path between this point and the muxer — neither in `src/libs/kt-engine/src/audio.rs` nor in `src/libs/kt-ffmpeg/src/muxer_impl.rs`. There are no tests for a multichannel source either: the only audio fixture more complex than stereo is mono (`src/libs/kt-media/tests/ffmpeg_conformance.rs:51`, t10), and it checks only channel 0, so even the rule "mono → double mono" with us holds on the behavior of someone else's library, not on our test.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages of a round |
| Where the main work is | kt-ffmpeg (the downmixing rule at the swr point), kt-model + kt-otio (a field on `MediaRef`/`KtBinMedia` and a version of the envelope), kt-app (editing in the bin), kt-media (the number of channels in `MediaInfo`, invalidation of the pyramid) |
| Project format | a v6 version is needed: the layout is a new field of `MediaRef`, read by the passthrough rules (a file without the field = today's behavior) |
| Tests that will have to be written | "mono → double mono" (today NOT covered; a fixture and a check L == R are needed); a multichannel fixture + the law of downmixing as a number; the layout travels through the round-trip of the model and OTIO; a change of layout invalidates the audio band of the cache AND the peak pyramid; the matrix is ours, not swr's — byte-for-byte between FFmpeg versions (invariant 11) |
| Depends on | nothing. The neighbor in the subcategory of F-AUD-004 (buses and track FX) concerns the level BETWEEN the clip and the master, this one concerns what the input itself is made of; the order between them is free |
| Breaks compatibility | no: the absence of the field = the current behavior |

## Verdict

**We take it, by the second way from the section on invariant 1: we downmix at the input by the rule chosen
by the user.** The downmixing point with us is exactly one (`source_impl.rs:489`), the rule in it now is the
libswresample default, and replacing the default with the user's choice costs an order of magnitude less than
widening the path to N channels. Invariant 1 is not touched at all: outward still only f32 48 kHz stereo comes
out, and neither the mix, nor FX, nor the meters, nor the muxer will learn of the existence of six channels.
Widening the path (the first way) is rejected not because it is "expensive", but because "there is nothing to
pay for": none of the three situations of "Essence" about multichannel OUTPUT requires it.

**The layout is a property of the source, and with us this is not a matter of taste.** The audio cache is keyed by
`SourceId(media.0)`, that is, by the bin element; a clip-level layout in the Vegas style would require a new field
of the key and would hit invariant 4. The Premiere/Resolve model is not merely closer to us — it is already half
built; what remains is to put a field into `MediaRef` and show it in the bin panel.

**Separately and before everything else, close the test on mono.** The rule "mono sounds in both ears" is exactly
the default that splits editors in half and produces the complaint once in four trackers. With us it is correct and
protected by nothing except the behavior of someone else's library: t10 reads the mono fixture and checks only
channel 0. This is the cheapest thing in the note and the only thing that can be done before the feature is accepted
as a whole.

## Sources

Trackers (the registry and the method of download — `docs/features/_trackers.md`; the dumps are in
`~/.cache/kinotrain/trackers/`, they do not go into the repository):

- https://invent.kde.org/multimedia/kdenlive/-/work_items/754 — "Audio
  Routing | Channel Mapping", opened 11.07.2020, updated 22.06.2025
- https://invent.kde.org/multimedia/kdenlive/-/work_items/382 — "MultiTrack
  Audio Support", 41 comments, the parent request to #754
- https://bugs.kde.org/show_bug.cgi?id=491072 · =516398 · =420905 · =474141
- https://github.com/olive-editor/olive/issues/1356 · /277 · /575 · /225
- https://github.com/mifi/lossless-cut/issues/2314 · /2390 · /507
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/145 —
  "Media Reference Stream/Channel Specification support", open since 09.08.2017:
  addressing a channel inside a stream is not solved even in the format in which we
  write the project

Both numbers of the `SIGNAL:` rows are reproduced by `tracker-rank.py --query` with the same
filters. The download of the trackers was running in the background at the time of writing:
`blender-vse` and `ffmpeg-trac` were empty, `openshot` and `flowblade` had their denominator grow
during the work — the numbers were taken 08.09.2026 at 00:27 UTC with ten non-empty dumps.

Documentation of the editors (accessed 08.09.2026):

- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/working_with_channels_for_audio_files.htm
- https://support.apple.com/guide/final-cut-pro/configure-audio-channels-verc1fab5f6/mac
- https://support.apple.com/guide/final-cut-pro/audio-channels-and-components-verdcd140a/mac
- https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part1095.htm
  ("Assigning Audio Channels in the Media Pool"; for Resolve 21 the mechanism is confirmed by a
  Blackmagic forum — https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=188164)
- https://www.storyblocks.com/resources/tutorials/premiere-pro-stereo-to-dual-mono
  and https://community.adobe.com/questions-729/premiere-pro-combine-left-and-right-microphone-channels-into-one-mono-track-1418091
- https://helpx.adobe.com/ca/after-effects/using/audio-effects.html (Stereo Mixer)

`helpx.adobe.com` still opens from this environment only intermittently (the Premiere page about audio
channel mapping did not open — "proxy refused the connection"), therefore the Premiere row was assembled
from secondary sources and the official Adobe community; this is the same limitation that is recorded in
`_editors.md` for the first wave.

The measurements of the downmixing law were made locally on the system FFmpeg of this machine
(`ffmpeg -i … -ac 2 -f f32le`), and not taken from the documentation: mono → max|L−R| = 0.0 on
`src/fixtures/out/sine44k_mono.m4a`; a synthetic 5.1 whose every channel has a peak of 0.9998, after
downmixing to stereo gives a peak of 2.4136, which coincides with 1 + 2·(1/√2) to the fourth digit.

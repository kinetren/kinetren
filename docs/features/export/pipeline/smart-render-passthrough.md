---
id: F-EXP-003
title: Smart render — copying untouched pieces without re-encoding
status: candidate
priority: C
cost: M
category: export/pipeline
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-ffmpeg, kt-media, kt-engine, kt-app]
invariants: [1, 5, 7, 9, 10, 11]
ui_rules: [9, 13, 16, 19]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-EXP-003 — Smart render — copying untouched pieces without re-encoding

## Essence

The editor took an hour-long file, cut two minutes off the beginning, touched nothing else —
and waits forty minutes while the editor re-encodes what is already encoded. Smart render removes this wait: a piece of the timeline that exactly matches a piece of the source is not decoded and not
encoded, but copied in packets into a new file. It is fast because there is no work at all; it is lossless because the bytes are the same.
The price is one and it is strict: one can copy only from a key frame,
so "smart" rendering is smart exactly up to the first trim that is not on a GOP boundary.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | **"no-recompress rendering (smart rendering)"** — it has its own name, there is no separate switch | it has worked since the DV era, growing with formats: the version history has the line "Added support for no-recompress rendering (smart rendering) of XAVC Intra MXF video" (VP 13 build 290, 17.04.2014). It triggers ITSELF when the render template matches the parameters of the source — there is no checkbox "enable smart rendering" in the dialog, and therefore the user learns that the feature works only from the render time. The term "render only where needed" does not exist in the product (see "Sources") |
| Adobe Premiere | 26.x | **Smart Rendering**, plus a separate checkbox **"Use Previews"** in the export dialog | It works NOT for "any h264", but by the list of intermediates in two wrappers: MXF (AVC-Intra, DNxHD/HR, XDCAM, JPEG 2000, XAVC-Intra) and QuickTime (ProRes, DNxHD, GoPro CineForm, Animation, uncompressed RGB). The condition is formulated strictly: "Smart rendering works only if the source codec, size, frame rate, and bit rate match the export settings". Clips with effects must be PRE-rendered to previews — then the export copies the preview files instead of recalculating |
| DaVinci Resolve | 21 (the mechanics were checked from forum descriptions of 17.x–20.x) | **"Bypass re-encode when possible"** — a checkbox on the Deliver page | Copies when a ready material in the needed form already exists for the piece of the timeline: an optimized/pre-rendered clip or the source itself. Practically limited to intra codecs (ProRes, DNxHD/DNxHR). A separate condition that nobody else has: **the project's colour management must be disabled** — "This feature only works on certain codecs (DNxHD should work) with deactivated color management", "any afaik in Rec709 only. Other color spaces in project settings will result in reencoding" |
| Final Cut Pro | 12.3 | no marketing name of its own — this is the behaviour of render files | When the codec of the export matches the codec of the render files, the frames are copied: "the render files will be used. That is, the frames are simply copied to the final file". The historical ancestor (FCP 7) was called differently and was done by hand: Export → QuickTime Movie → Current Settings + Make Self-Contained gave a "non recompressed movie" |
| After Effects | 26.x | no | A compositor, not an editing application: it has no notion of an "untouched piece of the source" — every layer goes through the transformations of the composition, and Render Queue always computes the frame. **[NOT VERIFIED against the primary source]** — a conclusion from the structure of the product; I did not search the help for an explicit denial |

## The trick behind it

**1. "Smart render" is TWO different mechanisms under one name, and they are constantly confused.** The first is copying the packets of the SOURCE ("do not re-encode what did not change"); this is how VEGAS does it and how it is described in its version history with the word "no-recompress". The second is reusing the already computed PREVIEW FILES ("do not compute a second time what is already computed"); this is exactly what Premiere hides behind the "Use Previews" checkbox, and it is exactly what FCP's render files do. The difference is practical: in the second mechanism the source does not matter at all — what matters is that the codec of the PREVIEW matches the export codec, and a clip WITH EFFECTS is also copied if it was rendered in advance. That is, Premiere accelerates a project where everything is
loaded with effects, and Vegas accelerates a project where there are no effects at all. These are opposite scenarios, and a user who has read about "smart render"
in one editor arrives at another with wrong expectations.

**2. The hybrid output runs into not the codec but the CONTAINER HEADER.** In MP4/MOV the parameters of the video stream (for h264 — SPS/PPS in the `avcC` box) lie in the header as ONE set for the whole file and are fixed at the moment it is written.
So in a file where some pieces are copied and some are encoded, the encoder must output a bitstream compatible with this single set: the same profile, the same level, the same pix_fmt, the same resolution, the same type of entropy coding. Hence — and only hence — comes what in the help pages looks like an arbitrary list of supported formats: for Premiere it is literally a list of MXF and QuickTime wrappers, for Resolve — ProRes/DNxHD, for Vegas — DV/HDV/XDCAM/XAVC-Intra. All of them are either intra-only,
or have a rigidly defined stream structure, that is, ones where "output a compatible stream" is a feasible requirement. An arbitrary h264 from a phone never falls into this list: it has its own profile, its own SPS, often a variable frame and B-pyramid.

**3. A key frame is a necessary condition for a cut, but NOT a sufficient one.**
One can copy from an I-frame, but "I-frame" and "a safe splice point" are
different things. With an open GOP "a B frame in one GOP is able to reference
a frame in a different GOP" (Wikipedia, Group of pictures) — that is, the frames after an I-frame legitimately reference the previous group, and a cut
exactly at the I-frame gives a broken picture in the first frames of the piece. Plus the
output order is not equal to the decoding order: "cut by the pts of the key frame" and
"cut by the packet boundary" are also not the same thing. Therefore editors usually require not just a key frame but a closed GOP,
and that is also why intra formats are convenient: there every frame is a boundary.

## Pros

- **The only path without generation loss.** For us (and for any editor with an RGB pipeline) even "I did not change anything" means decode to RGB, composite and encode again: a 10-bit or 4:2:2 source is GUARANTEED to degrade. A copy of the packets is not "a bit better" but the only branch where there is no degradation in principle.
- **Export time stops depending on the length of the clip and starts depending on the volume of edits.** An hour of material with two trims is delivered in seconds, and this is not an optimization by percentages but a different order of magnitude.
- **A copy is more deterministic than encoding.** The same input bytes always give the same output bytes, whereas the output of libx264/nvenc depends on the build, the version and the number of threads. The byte equality "copy == original" is a stronger statement than any hash of an encoded file.
- **Mechanism No. 2 (reusing previews) pays back the pre-render.** Where it exists, the editor renders a heavy section once during editing, and the export no longer computes that section — the work moves from "waiting at the end" to "background during work".

## Cons

- **Smart only up to the first trim.** An entry point not on a key frame — and the piece either is re-encoded entirely or slides to the nearest I-frame. That is, the price of speed is either no speed or a loss of editing precision; on a long GOP (0.5–2 s) this is half a second of miss.
- **The output file becomes non-uniform.** In one video there are pieces with the source's bitrate side by side with pieces with our CRF: the quality and bitrate jump at the joins, and this is visible both to the eye and to an analyzer. "Without loss of quality" is true only of the copied part.
- **A list of formats instead of a rule.** The user cannot predict whether the feature will trigger: in Premiere it is a table of wrappers and codecs, in Vegas — a match with the render template. Hence all the negative feedback of the kind "turned it on, and it did not get faster" — the feature silently did not trigger.
- **The absence of a visible control works against it.** Vegas has no switch at all — the user does not know whether anything was copied or not; Premiere has the checkbox, and then the class of complaints appears "the checkbox is ticked, yet it still encodes".
- **The hybrid makes the join dependent on an encoder that you did not choose.** On a machine with nvenc and on a machine with libx264 the "compatible stream" is different bitstreams, and the same project may assemble on one and not on another.
- **The feature is killed by any global setting of the frame, not only by an edit of the clip.** In Resolve it is switched off by enabled project colour management: "Other color spaces in project settings will result in reencoding". The user did nothing with the clip — he set the colour policy of the project once, and the whole smart render silently switched off on the entire timeline. For us this is a direct warning: our pipeline is itself a colour conversion (decode to RGBA8 and back to YUV420P), and as soon as the project gets any colour setting at all, the right to a copy will have to be removed entirely.

## What users say

> The "Use Previews on Export" tick box no longer works. So even if you have
> all ProRes 422 Preview rendered files ready to go - Premiere simply ignores
> them when you export the Media.
> — JonesVid, 29.04.2024, https://community.adobe.com/questions-729/faq-what-is-smart-rendering-1355162

> It needs a re-encode, not a rewrap.
> — RogerS, 12.04.2026, https://forum.borisfx.com/t/bugs-in-vegas-pro-2026/22222 (post 5)

> There are video codecs that do not use temporal compression… those are often
> referred to as "all intra" frame videos, or "All-I" frame.
> — Fr0sty, 12.04.2026, https://forum.borisfx.com/t/bugs-in-vegas-pro-2026/22222 (post 7)

> File/export/quicktime movie… then select "make self contained" and use
> "current settings". this will result in a non recompressed movie.
> — Jerry Hofmann, 11.05.2008, https://creativecow.net/forums/thread/final-cut-pro-export-without-re-encoding-clipsae/

The first quote is the most valuable kind of feedback: the feature is declared, the control
is in place, and there is no behaviour, and the person learns this from the export time. The second and third are from a thread about "bad for editing" files: there they also explain
why a long GOP gets in the way and why intra formats are more convenient for editors, that is,
exactly the mechanism on which smart render stands. The fourth
shows the age of the topic: with Final Cut it was a manual operation back in 2008.

## How it fits Kinotrain

### What is already there

**Half of the prerequisites lie ready, and this is a rare case for this codebase.**

1. **The muxer is already ours and already in-process.** `Muxer` in
   `src/libs/kt-ffmpeg/src/muxer_impl.rs` itself opens the `AVFormatContext`, creates the streams, writes the header and pushes packets through
   `av_interleaved_write_frame` (line 377) with `av_packet_rescale_ts` already next to it. Passthrough is exactly the same call,
   only the packet comes not from `avcodec_receive_packet` but from `av_read_frame` of someone else's file. Invariant No. 9 here does not hinder but is a PREREQUISITE: an editor that shells out to a `ffmpeg` binary
   would have to build `-c copy` and would lose control of the join.

2. **The keyframe index is already built and already lies in SQLite.** The table
   `packet_index` (`docs/media-index.md`, schema v2) holds one row per stream:
   `stream='video'` — the pts of all packets with `AV_PKT_FLAG_KEY`,
   `stream='audio'` — the pts of all audio packets, an i64 LE blob, sorted. It is written by `src/libs/kt-media/src/index.rs` (`put_packet_index` /
   `packet_index`), read by `FfmpegSource::open_indexed`
   (`src/libs/kt-ffmpeg/src/source_impl.rs`, lines 190–226), and the ready answer already sticks out outward — `MediaSource::keyframe_before(pts) ->
   Option<Tick>` (`src/libs/kt-media/src/lib.rs`, line 73; the implementation is
   `source_impl.rs`, lines 284–295, with recomputation from the stream's timebase into ticks and back). That is, the question "where is the nearest key frame to the left", for which foreign editors keep a separate index, is ALREADY answered by us by a `partition_point` over an array in memory, without a single read of the file. The binding of the index to the file is also solved: `PrebuiltIndex`
   (`src/libs/kt-ffmpeg/src/lib.rs`, lines 89–97) explicitly stipulates
   "valid only for byte-for-byte the same file", and identity is checked by size + mtime + xxh3-128 in `MediaIndexDb::ensure_media`.

3. **There is a way to decide "is this frame an untouched copy?"** `FramePlan` (`src/libs/kt-engine/src/video_plan.rs`) — the composition structure of the moment, built by ONE code before the backend is chosen. The sign of a "trivial frame" is read from it entirely and cheaply: `layers.len() ==
   1`, a branch `PlanLayer::One` with `alpha_k == 4096`, `fade: None`, an empty
   `fx`, `blend: None`, `cam3d: None`, `has_3d == false`, the size of the layer
   equals `out_w × out_h`. This is important architecturally: the decision about passthrough is taken by the same plan builder as everything else — a second traversal of the model with its own understanding of "what counts as an edit" is not created, and invariant No. 7 remains literally true.

4. **The test fixture is already generated.** `src/fixtures/gen.sh` makes
   `testsrc_h264.mp4` — 320×180, 30 fps, `-g 15`, yuv420p, libx264. A project
   320×180@30 with one such clip without FX is exactly the degenerate case that passthrough must copy entirely, and the boundaries at multiples of 0.5 s fall exactly on key frames.

### What is missing

- **No one above kt-ffmpeg sees packets.** The `MediaSource` trait
  (`src/libs/kt-media/src/lib.rs`, lines 64–94) is an abstraction over
  DECODED data: `read_audio_burst` returns f32 48k, `read_video_frame`
  returns RGBA. There is no raw packet in it, and by its meaning there should
  not be one. It is also implemented by `SyntheticSource` and `PlaceholderSource`
  (`src/libs/kt-media/src/placeholder.rs`), which have nothing to answer to "give me
  an h264 packet". A SEPARATE optional copier interface is needed, not an extension
  of the trait.
- **The muxer has exactly two inputs**: `write_video(rgba)` and
  `write_audio(&[f32])`. There is no packet input.
- **The source codec is recorded nowhere.** `MediaInfo`
  (`src/libs/kt-media/src/lib.rs`, lines 16–26) holds
  `duration/has_video/has_audio/fps/width/height`, and that is all: no codec_id, no
  profile, no level, no pix_fmt, no colour characteristics, no
  `extradata`. The condition "same codec and same profile" cannot be CHECKED right now,
  because there is nothing to compare against; `probe_json` in the index DB would have to be
  extended along with it.
- **Not enough dlopen symbols.** `src/libs/kt-ffmpeg/src/api.rs` loads
  `av_read_frame`, `avcodec_parameters_from_context`,
  `av_packet_rescale_ts`, `avformat_write_header` and
  `av_interleaved_write_frame`, but there is NO `avcodec_parameters_copy`,
  `av_packet_ref`, or the `av_bsf_*` family (bitstream filters; they are mandatory for
  the Annex B ↔ AVCC conversion). They must be added to the loader's table and
  run through bindgen over the vendored headers in
  `src/libs/kt-ffmpeg/vendor/ffmpeg-{6,7,8}` (invariant No. 10). The work is
  mechanical, but it is triple.

### What gets in the way

**1. Our output is one by construction, and it is ours.** `Muxer::open` takes the
`codecpar` of the stream from OUR encoder (`avcodec_parameters_from_context`,
`muxer_impl.rs` lines 204–205) and writes the header right away with `+faststart`
(lines 210–220). After `avformat_write_header` the set of stream parameters is
fixed for the whole file. A hybrid requires taking `codecpar` from the SOURCE and
forcing our encoder to emit a bitstream compatible with it.

**2. And our encoder is chosen automatically, on someone else's machine.**
`VIDEO_ENCODERS` = nvenc → vaapi → qsv → libx264 → mpeg4, with a real
open-probe (`muxer_impl.rs`, lines 95–160), plus we hard-set
`gop_size = fps*2`, `max_b_frames = 0`, `pix_fmt = YUV420P`. There is no way to combine
this with a random h264 from a phone, however hard one tries. One can only forbid
mixing and allow a copy where no encoding is needed AT ALL.

**3. Audio goes first.** Invariant No. 1: inside the engine, sound is always f32
48 kHz stereo, and the muxer's AAC encoder accepts only that.
Audio passthrough is a violation of No. 1 (packets are not decoded at all). And if
only video is copied while the sound is encoded, a hybrid container appears, with the
AAC priming delay and a desync at the junction.

**4. The export pipeline is per-frame, and its invariants rest on this.**
`render_raw_video_range_with` (`src/libs/kt-engine/src/export.rs`,
lines 190–275) runs in chunks along the fps grid, warms the cache with `read_video_span`,
computes `max_concurrent_layers`, and calls `emit(&frame_buf)` exactly
`total_video` times. The segment to be copied must be CUT OUT of this loop
entirely, rather than substituting frames inside it. Otherwise progress, statistics and
`missing_video_layers` will start counting frames that do not exist.

**5. Our keyframes are pts, not "where to cut".** The index stores
`p.pts` of packets with `AV_PKT_FLAG_KEY` (`source_impl.rs`, lines 213–218), and nothing
else: no dts, no sizes, no sign of a closed GOP. With B-frames the output order is not
the decode order, and an open GOP is also marked with `AV_PKT_FLAG_KEY` (see "The trick behind it", item 3). The consolation: the scanning pass itself already reads EVERY packet of the file (`av_read_frame` in a loop,
lines 204–221), so adding to the index what the copier lacks is a question of blob size,
not of a second pass over the file.

### Testability: does the feature undermine the foundation

I analyse this separately, because it is the main question about the feature.

**Invariant No. 11 (determinism) the feature does NOT touch.** `golden_export.rs`
(test 11) hashes the RAW streams `render_raw_audio`/`render_raw_video` on
`SyntheticSource`. There is no muxer in it at all, and the synthetic source has no
packets. GOLDEN_AUDIO/GOLDEN_VIDEO do not touch passthrough by a single byte. Moreover, a copy is more
deterministic than encoding: the output of libx264 depends on the build and on the number of
threads. That is exactly why our reference stands on raw streams and not on mp4.

**Invariant No. 7 the feature does not violate, but it takes a piece of the file out from under
verification.** The formulation "on one backend, preview == export byte-for-byte" is about
RENDERING. A copied segment is not rendered, so there is nothing to compare: our preview path
for the same moment gives decode → RGBA8 → composite → YUV420P
at CRF 23, that is TWO colour-space reconversions and a generation loss, while the copy has none. The picture at the output will be better than the preview and
certainly different. This is not a violation of the letter, but it is a hole in verifiability.

**However, it can be verified MORE strictly than encoding, with exact equalities rather than tolerances**
(ad-hoc epsilons are already forbidden for us):

- the copy equals the original byte for byte: demux the source and the output and compare the
  payloads of the packets over the segment (`assert_eq` on `Vec<u8>`);
- the decode of the output equals the decode of the source, with the same `FfmpegSource` as in
  `src/libs/kt-media/tests/ffmpeg_conformance.rs` (tests 10/34);
- the grid has not drifted: the output duration equals `project_range`, with exactly
  `total_video` frames;
- a refusal is honest: on a clip with FX, a transition, a level, a warp, a reverse, a second layer,
  a foreign fps or a foreign size, passthrough MUST fail to engage. This is a separate negative test
  of the "t247e" class (enumerate all kinds of edits and make sure each one removes the right to a copy).

So the foundation is undermined not by passthrough, but by a HYBRID: at the junction where a copy
borders our encoding, byte equality cannot be checked in principle. There one can only look at
decodability and at the absence of artifacts, and that is exactly the zone where all the
smart render bugs of other editors live.

### Which invariants it touches

- **No. 9** (export is in-process): not touched, but relied upon: the copy goes through
  the same `av_interleaved_write_frame`.
- **No. 10** (no linking with libav*): touched mechanically: new symbols
  in `api.rs` plus the bindgen wrapping of ffmpeg-6/7/8.
- **No. 1** (audio always f32 48k inside the engine): audio passthrough goes
  beyond it. Either we copy only the video, or the invariant is reworded
  ("sound that the engine PROCESSES is always f32 48k").
- **No. 7** (the composition code is one): not violated, as long as the "trivial frame" flag is
  computed FROM `FramePlan`. It is violated at once if a separate project analyser with its own
  notion of "edit" is created.
- **No. 11** (determinism): not violated (see above), but the zone without a reference
  is widened.
- **No. 5** (one MediaWorker owns the decoder): the copier MUST open the
  file with its OWN short-lived handle, as the index worker already does
  (`docs/media-index.md`, "Who writes"), and not go to someone else's decoder.
- **UI rules 9, 13, 16, 19.** There must be no separate "smart render" checkbox.
  The dialog already has a "Video codec" combobox with an "Auto" item
  (`src/apps/kt-app/src/render_dialog.rs`, lines 285–300), and "Copy
  without re-encoding" is one more item of it (item 13 "do not waste space", item 9
  "no checkboxes out of place"). And when a copy is impossible, this MUST be an ERROR with a
  named reason, not a silent slide into re-encoding (item 16). Otherwise we would
  reproduce with our own hands the thread "the box is ticked, but it encodes anyway", quoted above.

## Estimate

| What | Answer |
|---|---|
| Size | **M** for the degenerate case ("the whole export is a copy of one untouched clip"): 2–3 round stages. The hybrid output is a separate feature at **XL** and with a different verdict |
| Where the main work is | kt-ffmpeg (packet copier, new dlopen symbols, bindgen 6/7/8, packet input of the muxer), kt-media (probe: codec_id/profile/pix_fmt/extradata in `MediaInfo` and `probe_json`), kt-engine (predicate over `FramePlan`, export branch bypassing the per-frame loop), kt-app (codec combobox item + refusal reason as an error) |
| Project format | not touched (the decision is made at export; nothing is stored in the model) |
| Tests to write | conformance on `src/fixtures/out/testsrc_h264.mp4`: byte equality of the copied packets, decode of the output == decode of the source, number of frames and duration; negative test "any edit removes the right to a copy" (FX, transition, level, warp, reverse, second layer, foreign fps, foreign size); tests 25/33 (`export_options.rs`) are not to be weakened |
| Depends on | nothing (extending probe is useful on its own) |
| Breaks compatibility | no |

## Verdict

**We take it, but only the degenerate case, and only as an explicit choice in the
codec combobox: "copy without re-encoding".** The copy is justified by the fact that it is the only
export branch without generation loss. Right now even an untouched clip goes through YUV → RGBA8 → YUV420P
at CRF 23. Half of the work (keyframe index, own muxer, single plan builder) has
already been done for other tasks.

**The hybrid output is rejected until a separate decision by the owner.** The reason
is not the size but the construction: the MP4 header carries one set of SPS/PPS per file, while our encoder is
chosen automatically from five candidates on someone else's machine. So we can guarantee neither a
"compatible junction", nor can we check it by byte equality. The right form of refusal is an error with a
named reason ("the source is h264 High@4.0, the export will open as
libx264 Main@3.1 — a copy is impossible"), not a silent re-encoding.

## Sources

- Vegas Pro Release History (topic on the official Boris FX forum,
  author Nick Hope, 22.06.2026): the line "Added support for no-recompress
  rendering (smart rendering) of XAVC Intra MXF video" in the section Vegas Pro
  13 build 290 (17.04.2014) —
  https://forum.borisfx.com/t/vegas-pro-release-history/24284
  (access date 2026-09-07)
- Larry Jordan, "Premiere Pro: Export Faster with Smart Render",
  05.12.2019 — the condition "Smart rendering works only if the source codec,
  size, frame rate, and bit rate match the export settings", the list of
  MXF/QuickTime wrappers, the "Use Previews" checkbox —
  https://larryjordan.com/articles/premiere-pro-export-faster-with-smart-render/
- Adobe Support Community, "FAQ: What is Smart Rendering?", Kevin J.
  Monahan, 03.10.2019, and comments from 2024 —
  https://community.adobe.com/questions-729/faq-what-is-smart-rendering-1355162
- Larry Jordan, "FCP X: Render Files, Exporting and Image Quality",
  22.11.2015 — "the render files will be used. That is, the frames are
  simply copied to the final file" —
  https://larryjordan.com/articles/fcp-x-render-files-exporting-and-image-quality/
- Creative COW, thread "Final Cut Pro – export without re-encoding clips?",
  May 2008 —
  https://creativecow.net/forums/thread/final-cut-pro-export-without-re-encoding-clipsae/
- Boris FX Forum, thread "BUGs in Vegas Pro 2026", 12.04.2026 (posts 5 and 7)
  — https://forum.borisfx.com/t/bugs-in-vegas-pro-2026/22222
- Wikipedia, "Group of pictures" — definitions of an open and a closed GOP —
  https://en.wikipedia.org/wiki/Group_of_pictures
- Blackmagic Forum, threads on "Bypass re-encode when possible" (available in
  this session only through search engine output, see caveat 3) —
  https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=101369 and
  https://forum.blackmagicdesign.com/viewtopic.php?f=33&t=211623
- Project code and documentation: `src/libs/kt-ffmpeg/src/muxer_impl.rs`,
  `src/libs/kt-ffmpeg/src/source_impl.rs`, `src/libs/kt-ffmpeg/src/api.rs`,
  `src/libs/kt-ffmpeg/src/lib.rs`, `src/libs/kt-media/src/index.rs`,
  `src/libs/kt-media/src/lib.rs`, `src/libs/kt-engine/src/export.rs`,
  `src/libs/kt-engine/src/video_plan.rs`, `src/apps/kt-app/src/render_dialog.rs`,
  `src/fixtures/gen.sh`, `docs/media-index.md`.

**Caveats about the verification.**

1. **The wording of the task, "in Vegas it is called render only where
   needed", was not confirmed.** A search for the exact phrase (Yahoo, full-text
   search over the official Boris FX forum through its Discourse API) gave not a single hit related to video editors: the output goes to
   Blender and Maya. Vegas's own term is "no-recompress rendering
   (smart rendering)", and it is confirmed by the version history. I consider the premise of the task
   mistaken and do not reproduce it in the note.
2. **The Vegas forum has moved.** `vegascreativesoftware.info/us/forum`
   returns a 301 to `https://forum.borisfx.com/` (Discourse, section
   `/c/vegas-pro/55`). Full-text search is convenient through
   `https://forum.borisfx.com/search.json?q=…`, and a specific post is taken
   as `/t/<id>/<number>.json`. This is worth remembering for the next waves:
   ordinary search engines in this session returned a captcha and 403.
3. **The Resolve line is the weakest.** The option name ("Bypass re-encode
   when possible", the checkbox on the Deliver page) and both quotes about codecs and
   colour management are taken from the SEARCH ENGINE OUTPUT over the Blackmagic forum
   (`forum.m.blackmagicdesign.com/viewtopic.php?f=21&t=101369`,
   `forum.blackmagicdesign.com/viewtopic.php?f=33&t=211623`). The threads themselves in
   this session returned 403 or a certificate error, and I did not open the Resolve Reference
   Manual. I have no authors and dates of the posts, so these phrases are NOT moved to the section
   "What users say". They appear only in the table, as a description of the mechanics. The whole line is **[NOT VERIFIED against the primary source]**,
   and at the next wave it must be re-checked first.
4. **The After Effects line is a reasoning, not a quote** (see the note in
   the table). `helpx.adobe.com` does not open from this environment.
5. The Vegas and FCP lines describe the mechanics from the version history, the forum and
   the analysis by Larry Jordan; they were not checked live on the 2026 version.

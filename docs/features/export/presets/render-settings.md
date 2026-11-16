---
id: F-EXP-006
title: Render settings — what a person can set for the final file
status: in-progress
priority: A
cost: L
category: export/presets
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-ffmpeg, kt-engine, kt-app, kt-cli]
invariants: [1, 9, 10, 11]
ui_rules: [4, 13, 16, 19, 23]
round: kkt-26.11.15
point: "N5 — change render-settings"
tests: []
depends_on: [F-EXP-001, F-EXP-004]
trackers: []
services: []
checked: 2026-11-16
---

# F-EXP-006 — Render settings — what a person can set for the final file

## Essence

The edit is done; now a person decides what FILE leaves the program: which container and codec,
how big, how sharp, which colour it claims to be, which sound, which piece of the timeline. The
owner's claim (kkt-26.11.15): "очень скудные настройки финального рендера прям очень". This note
is about the SET of options in one render window and which of them are honest with us; naming
and keeping a set is F-EXP-001, which encoder actually opens is F-EXP-004.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 b66 | Render As → template → Customize Template | a template is bound to ONE format (MAGIX AVC/AAC MP4, HEVC, ProRes, …); Customize opens its tabs: frame size, frame rate, field order, profile, bit rate mode CBR/VBR with maximum and average, Two-pass, encode mode (MainConcept / NVENC / QSV / AMF), audio tab, project tab (render quality); the window adds "render loop region only" and the output name; a customized template is saved under a name. Build 66 added colour information for ProRes templates (see F-EXP-001 trick 1) |
| Adobe Premiere | 26.x | Export mode (Media Encoder for the queue) | Format → Preset → Video: profile and level, bitrate encoding CBR / VBR 1-pass / VBR 2-pass, target and maximum bitrate, hardware or software encoding, key frame distance; Audio: codec, sample rate, channels, bitrate; Range: entire / in-out / work area / custom; Metadata (XMP); "Estimated File Size" under the settings |
| DaVinci Resolve | 21 | Deliver page → Render Settings | Format (MP4, MOV, MXF, …) and codec, encoder (native or NVIDIA/Intel/AMD/Apple), quality Automatic or restrict to a bitrate, encoding profile, key frames Automatic or every N, frame reordering, passes; Advanced: Data Levels Auto/Video/Full, Color Space Tag, Gamma Tag, field rendering, subtitles; Audio tab; File tab with a name template; render range: entire timeline or In/Out. In the 18.6 manual the H.264 defaults are quality Automatic, profile Main, single pass, key frames Automatic with frame reordering on |
| Final Cut Pro | 12.3 | Share → Export File (Compressor for the rest) | deliberately short: format, video codec (H.264 faster / better quality, HEVC 8- or 10-bit, the ProRes family), resolution, the colour space follows the project and the codec; no bitrate field at all. Everything else (data rate, multipass, frame controls, custom sizes) goes through a Compressor setting sent from the same Share menu |
| After Effects | 26.x | Render Queue → Output Module | format (H.264 native since 2023, QuickTime, image sequences), codec options, channels and depth, colour management of the output, resize and crop, audio output; anything finer is handed to Media Encoder through the queue |

**Outside the core.** The rate-control and two-pass questions are formulated most plainly by
transcoders, where nothing but the encode is on the screen; they get no editor key and carry no verdict.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| HandBrake | 1.9/1.10 line [version NOT VERIFIED] | Video tab | Constant Quality (RF) OR Avg Bitrate; the 2-pass checkbox exists only under Avg Bitrate; encoder preset slider, tune, profile, level (Auto by default); container MP4/MKV/WebM with the codec list following it; the Summary tab shows the result as tags |
| Shutter Encoder | 19.x [version NOT VERIFIED] | function list + "Advanced features" | every FFmpeg switch as a checkbox: colour space and range conversion, GOP, B-frames, hardware acceleration per vendor, metadata, name suffixes; the codec list follows the "function" (H.264, H.265, AV1, VP9, ProRes, DNxHR, …) |

## The trick behind it

**1. Range and colour tags are TWO controls in one decision, and the classic "washed-out export"
is born exactly where they disagree.** Resolve keeps "Data Levels" (how values are WRITTEN:
Auto/Video/Full) apart from "Color Space Tag"/"Gamma Tag" (what the file CLAIMS); a guide on the
subject puts it as "Data levels changes the encoded range; Color Space Tag and Gamma Tag are
metadata". A full-range conversion tagged as limited (or the reverse) shows lifted blacks or
crushed shadows in every player, while the program's own viewer looks right — the person
"fixes" the grade instead of the tag. A file with NO tags is worse than a wrong one: the player
guesses, and the common guess depends on the frame height (BT.601 for SD sizes, BT.709 for HD)
[the guess rule is player behaviour, NOT VERIFIED per player]. That is our case today: the muxer
writes no `color_*` field, so a custom 720×576 export would change its colours on playback
with nothing in the window to say why. Hence the rule of the verdict: a tag is written from the
SAME choice that drives the RGB→YUV conversion, never as a separate field.

**2. Two-pass and the size estimate are properties of a SOFTWARE bitrate mode, not of export.**
HandBrake shows the 2-pass checkbox only under Avg Bitrate, and its own documentation says
Constant Quality "can produce the same quality as a 2 pass encode" at about half the time.
Hardware encoders do not do it at all: in Vegas the box stays ticked and "For NVENC, Vegas
ignores 2 pass"; in Premiere the same 25 Mbit/s target came out at 22 Mbit/s in software and
8.2 Mbit/s on hardware (quote below). Premiere's "Estimated File Size" is target × length, so it
is honest only for CBR and a ceiling for VBR, and meaningless for a quality mode. Beyond F-EXP-001
trick 2 (a bitrate is a request): the request is honoured differently by each encoder of the
same cascade, so an estimate must say which mode it trusts.

**3. Level is a ceiling the ENCODER checks, and a manual level is the commonest way to make a
working export refuse.** H.264/HEVC levels cap frame size, frame rate and bitrate together;
4.1 does not admit 3840×2160, and forum advice for failed 4K exports is "set level 5.1".
Resolve and HandBrake default to Auto for exactly this reason, and Adobe's hardware path has its
own ceilings (a 4096-pixel frame, about 75 Mbit/s per an Adobe community answer) that silently
send the job to software. The profile follows the bit depth (High vs High 10, Main vs Main 10)
— a person picks 10-bit, never "High 10".

## Pros

- A quality mode (CRF/CQ) needs no knowledge of the content: the same number gives the same
  look for a talking head and for confetti, and one pass suffices — the case two-pass exists for
  (hit a size) is the bitrate mode's.
- Container-first lists (HandBrake, Resolve) make impossible pairs unreachable: ProRes is not
  offered in WebM, so the "codec not supported by the container" error is never produced.
- Writing range and matrix as tags from the conversion itself makes the file's colour the same
  in every player that reads tags, independent of frame size.

## Cons

- Every extra codec is another cascade to keep honest: each hardware encoder has its own
  preset names, rate-control words and pixel-format wishes (QSV wants NV12), and a field shown
  for one encoder can be meaningless for the next one in the cascade.
- A long settings column (Shutter Encoder, Vegas Customize) is where people stop understanding
  what they changed; presets (F-EXP-001) are the answer the others found, not fewer options.
- Templates bound to one format (Vegas) cannot move a setting across formats: changing MP4 to
  MOV means a different template and re-entering everything.

## What users say

> What can explain such a huge gap between the target bitrate and end result during hardware encoding?
> — Adobe Community, Premiere Pro discussions, 2022-03-11, https://community.adobe.com/t5/premiere-pro-discussions/confused-about-export-bitrates-software-vs-hardware-encoding/m-p/12807245/highlight/true

> When exporting the material to H265, the colors are washed out. No matter whether you set data level full or video.
> — Blackmagic forum, "Is it a bug? My fix/workaround for washed out HDR export", https://forum.blackmagicdesign.com/viewtopic.php?p=1011695 (read 2026-11-16 from the search index: the page answers 403 to direct fetches, post date not read)

> For NVENC, Vegas ignores 2 pass
> — VEGAS forum, "2 pass only does one pass", https://www.vegascreativesoftware.info/us/forum/2-pass-only-does-one-pass--135019 (read 2026-11-16 from the search index; the forum now redirects to forum.borisfx.com, post date not read)

> очень скудные настройки финального рендера прям очень
> — the owner, `docs/ktprompts/kkt-26.11.15.txt`, 2026-11-15

## How it fits Kinotrain

CAPABILITY: openspec/specs/export/

### What already exists

- `src/libs/kt-ffmpeg/src/muxer_impl.rs` — `Muxer::open`: the container is the literal
  `"mp4"`, the audio encoder the literal `"aac"`, `gop_size = fps * 2`, `max_b_frames = 0`,
  no `color_range`/`colorspace`/`color_primaries`/`color_trc` is set anywhere.
- `src/libs/kt-ffmpeg/src/encoders.rs` — the per-OS H.264 cascade (nvenc → vaapi → qsv →
  libx264 → mpeg4 on Linux; nvenc → amf → qsv → libx264 → mf → mpeg4 on Windows) with
  `quality_options`/`bitrate_options` per encoder; libx264 is fixed to `preset veryfast`,
  NVENC to `p4`. The open check is real (F-EXP-004).
- `src/libs/kt-engine/src/export.rs` — `ExportOptions`: `quality`, `video_bitrate`,
  `audio_bitrate`, `force_encoder`, `resolution`; the frame rate is `project.settings.fps`.
- `src/apps/kt-app/src/render_dialog.rs` — `RenderSetup` and `draw_render_setup`: encoder,
  CRF 14–32 or 0.5–80 Mbit/s, AAC 128–320 kbps, resolution (project / 4 presets / max of
  sources). The range (region else whole project) is applied but not shown. Nothing survives a
  restart (ui rule 23). The name template `kt-<project>-<region>-<len>s.mp4` is the owner's
  decision and stays; only its extension follows the container.
- `src/apps/kt-cli/src/main.rs` — `export <in.kt> <out.mp4> [encoder]`: only the encoder name.

### What is missing

Everything in the TAKE line of the verdict: other containers and codecs, VBR/CBR, GOP and
B-frames, a speed scale, 10-bit, colour tags, custom size, frame-rate override, other audio
codecs, a visible range, metadata, presets and remembered settings, a size estimate — and the
same set reachable from kt-cli, or the GUI and the CLI render different files from one preset.

### What hinders

- Invariant 9: no temporary files — two-pass needs a stats file and is rejected below.
- Invariant 10: one binary for FFmpeg 6–9 loaded at run time; which of libx265/libaom/libsvtav1/
  libvpx/prores_ks a given build carries is known only by opening — each codec is a cascade
  with the F-EXP-004 open check, and an absent one is shown as unavailable with the reason.
- Invariant 1: the engine is 48 kHz stereo f32 to the output, so sample rate and layout are
  not options; PCM/FLAC/Opus differ only in the encoder of that one stream.
- Invariant 11: the export is byte-reproducible on one machine; every new option is part of
  the determinism key (encoder threads are already pinned for it), and a frame-rate override
  must come out of `frame_plan`, not a frame dropper after it (invariant 7).
- The composite is RGBA 8-bit and progressive, built at the output size: 10-bit output carries
  8-bit content in a 10-bit stream (fewer banding steps after the encoder, no new precision).

### Which invariants it touches

1 (audio format fixed), 7 (one composition path — size and fps come from the plan), 9
(in-process, no temp files), 10 (dlopen, per-build encoder availability), 11 (determinism).

## Estimate

| What | Answer |
|---|---|
| Scope | L — a whole round (change render-settings, point N5 of kkt-26.11.15) |
| Where the main work is | kt-ffmpeg (containers, codec cascades, colour tags), kt-app (the window, presets), kt-cli (the same options) |
| Project format | does not touch (settings live between projects) |
| Tests that will have to be written | per container/codec a written-and-probed file; tags equal the conversion; GUI ≡ CLI for one preset; determinism of each new codec on one machine |
| Depends on | F-EXP-001 (presets), F-EXP-004 (open check) |
| Breaks compatibility | no |

## Verdict

**TAKE**, each item as the owner's claim asks:
- Container MP4 / MOV / MKV / WebM; the file extension follows it.
- Video codec H.264 / H.265 / AV1 / VP9 / ProRes 422 HQ, each with its own hardware-first cascade and a real open check; an option the loaded FFmpeg cannot open is shown as unavailable with the reason.
- Rate control Quality (CRF/CQ) / VBR (average + maximum) / CBR.
- Keyframe interval in seconds; B-frames 0–4.
- An encoder speed scale of five steps, mapped onto each encoder's own presets.
- Bit depth 8/10 where the codec has a 10-bit profile; the profile follows it, the level stays automatic.
- Colour matrix BT.709/BT.601 and range limited/full, written as tags AND used in the RGB→YUV conversion (today no tags are written).
- Frame size presets plus a custom W×H; a frame-rate override.
- Audio codec AAC / PCM 16 / PCM 24 / FLAC / Opus / none; bitrate where lossy.
- Range whole project / region, shown in the window.
- Title and comment metadata.
- Named presets kept between projects; the last settings remembered.
- An estimated size where the rate is known (CBR exact, VBR as a ceiling, none for Quality).

**REJECT**, with reasons:
- Two-pass — needs a temporary stats file (invariant 9) and a second full render of an export the owner already calls slow; the Quality mode reaches in one pass what pass 2 serves (trick 2).
- Manual level — the encoder picks the lowest level that fits; a manual one too low makes it refuse (trick 3).
- Tune — x264/x265-only words, meaningless on the hardware encoders of the same cascade.
- 4:4:4 chroma — the composite is RGBA 8-bit, and delivery players do not decode 4:4:4.
- A scaling filter — the composite is built at the output size; there is no scaler stage (invariant 7).
- Interlaced field order — the composite is progressive.
- 44.1 kHz or another sample rate, channel layouts other than stereo — invariant 1: 48 kHz stereo all the way to the output.
- Manual VBV buffer — derived from the maximum rate.

Factory presets are named by what they are (codec, size, rate), not by a platform: platform
requirements change without a release (F-EXP-001 trick 2).

## Sources

- https://community.adobe.com/t5/premiere-pro-discussions/confused-about-export-bitrates-software-vs-hardware-encoding/m-p/12807245/highlight/true (access date 2026-11-16)
- https://forum.blackmagicdesign.com/viewtopic.php?p=1011695 (search index only, 403 on fetch; 2026-11-16)
- https://davinciresolveclub.com/davinci-resolve-export-looks-washed-out/ (page dated 2026-10-27; access date 2026-11-16)
- https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part3934.htm (Resolve 18.6 manual mirror, H.264 defaults; search index, 2026-11-16)
- https://handbrake.fr/docs/en/1.2.0/technical/video-cq-vs-abr.html (search index only, handbrake.fr not fetchable from the environment; 2026-11-16)
- https://www.vegascreativesoftware.info/us/forum/2-pass-only-does-one-pass--135019 (search index; redirects to forum.borisfx.com; 2026-11-16)
- https://community.adobe.com/questions-729/premiere-ignores-my-bitrate-settings-1415185 and https://community.adobe.com/t5/premiere-pro-discussions/export-size/m-p/13438715 (estimate = target × length; 2026-11-16)
- https://forum.blackmagicdesign.com/viewtopic.php?p=239891 (H.264 output resolution limits; 2026-11-16)

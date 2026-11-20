---
id: F-MED-010
title: Decoding the timeline's sources on the video card
status: candidate
priority: B
cost: M
category: media/sources
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-ffmpeg, kt-media, kt-engine]
invariants: [4, 5, 10, 11]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-EXP-008]
trackers: []
services: []
checked: 2026-11-20
---

# F-MED-010 — Decoding the timeline's sources on the video card

## Essence

A camera or phone file (H.264, HEVC) is decompressed by the video card's decode block instead of the
processor, so a 4K timeline plays with the cores free for effects and the mix. The editor notices it
only as "the processor is no longer at 100 % on plain footage", and as the rare file the card decodes
wrongly or not at all.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Preferences → File I/O → "Hardware decoder to use" (Off / Auto / Intel QSV / NVDEC …) | one global choice for every file; the list depends on the machine and does not always match what is used; NVDEC is reported to take nearly all video memory and to collide with OFX plugins |
| Premiere | 26 | Preferences → Media → "Enable hardware accelerated decoding" (restart), a checkbox per device under H.264/HEVC | the user may pick which GPU decodes; supported "flavours" (bit depth, chroma) are a table per vendor and per release; Debug Monitor shows what is actually decoded in hardware |
| Resolve | 21 | Preferences → Decode Options → "Decode H.264/HEVC using hardware acceleration" (+ Intel Quick Sync / NVIDIA under it) | Studio only; users report it is used only when enough video memory is left beside the image processing |
| Final Cut Pro | 12 | none | VideoToolbox decodes silently; no choice, one platform |
| After Effects | 2026 | Preferences → Import → "Enable hardware accelerated decoding" | the same implementation as Premiere (shared media core) [NOT VERIFIED against the primary source] |

## The trick behind it

- **The decode block's memory is the scarce resource, not its speed.** Resolve falls back to the
  processor when video memory is short; Vegas's NVDEC eats the memory OFX plugins wanted. A decoder
  holds a whole pool of reference surfaces (a 4K H.264 stream: 17+ surfaces of 12 MB), so "every
  decoder on the card" is a memory decision.
- **Support is per FLAVOUR, not per codec**: 4:2:2 and 10-bit 4:2:2 HEVC from cameras, or H.264 High 10,
  are not decoded by most consumer blocks; Premiere and Puget publish tables per vendor and release.
  An editor that says "H.264: hardware" lies for a part of the files.
- **Nobody says whether the picture is the same.** None of the five promises that a frame decoded on the
  card equals the processor's frame. For H.264/HEVC/VP9/AV1 the standard makes the decoded samples
  exact, so the only legitimate difference is the colour conversion after it — which is where an
  editor that wants a byte-stable render has to look.

## Pros

- Plain long-GOP footage stops costing cores: on the owner's machines the processor is the preview's
  bottleneck (kkt-26.11.18b: "у нас datamosh лагает как нехуй делать").
- The disk prerender (F-EXP-008) already decodes on the device; the same path for every source keeps
  one rule.

## Cons

- A decoder on the card is a bounded pool of surfaces; a dense edit opens many decoders, so some must
  stay on the processor.
- Carrying each picture back to the processor's memory costs a copy; on some integrated chips reading
  the surface is slow, so the gain must be measured on the machine, not assumed.

## What users say

> "Hardware Decoder NVIDIA NVDEC uses all video RAM"
> — VEGAS Creative Software forum, thread title, https://www.vegascreativesoftware.info/us/forum/hardware-decoder-nvidia-nvdec-uses-all-video-ram--117890

> "Scrubbing uses CPU on PC with Nvidia but GPU when using Intel Qsync"
> — VEGAS Creative Software forum, thread title, https://www.vegascreativesoftware.info/us/forum/scrubbing-uses-cpu-on-pc-with-nvidia-but-gpu-when-using-intel-qsync--131483

## How it fits Kinotrain

CAPABILITY: openspec/specs/media-index/

### What already exists

`kt_ffmpeg::Source::open_hw` (`src/libs/kt-ffmpeg/src/source_impl.rs`, hw-segment-decode) decodes on
VAAPI/D3D11VA and carries NV12 back; `FfmpegSource::open_hw` (`src/libs/kt-media/src/ffmpeg.rs`) opens
every decoder of a file that way — today only for the disk tier's segments (`src/libs/kt-media/src/segment.rs`).

### What is missing

The preview's and the export's sources, and the guarantee that a device picture equals the
processor's: today NV12 and yuv420p go through different swscale paths (mean difference 1.46, t3129).

### What hinders

Invariant 4 (the cache key does not name the decoder), invariant 5 (decoders owned by one thread;
each `Source::open_hw` opens a device of its own), invariant 11 (the export's bytes on one device).
Change `device-decode-sources` designs it.

### Which invariants it touches

4, 5, 10 (one more optional FFmpeg symbol), 11.

## Estimate

| What | Answer |
|---|---|
| Scope | M — two stages |
| Where the main work is | kt-ffmpeg (layout, shared device, eligibility), kt-media (decoder kind per decoder), kt-engine (budget, trace) |
| Project format | does not touch |
| Tests that will have to be written | device picture equals the processor's on the fake VA driver; budget refusal changes no byte; export bytes with and without the device |
| Depends on | hw-segment-decode (done) |
| Breaks compatibility | no |

## Verdict

Take, without a user-facing checkbox by default (FCP's silence, Resolve's memory check): eligible
flavours decode on the device when its surface budget allows, and the picture is made byte-equal to
the processor's so the choice can never show in a frame or an export.

## Sources

- https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part123.htm (Decode Options, access date 2026-11-20)
- https://www.pugetsystems.com/labs/articles/What-H-264-H-265-Hardware-Decoding-is-Supported-in-Premiere-Pro-2120/ (access date 2026-11-20)
- https://www.pugetsystems.com/labs/articles/What-H-264-and-H-265-Hardware-Decoding-is-Supported-in-DaVinci-Resolve-Studio-2122/ (access date 2026-11-20)
- https://www.vegascreativesoftware.info/us/forum/hardware-decoder-nvidia-nvdec-uses-all-video-ram--117890 (access date 2026-11-20)
- https://www.vegascreativesoftware.info/us/forum/scrubbing-uses-cpu-on-pc-with-nvidia-but-gpu-when-using-intel-qsync--131483 (access date 2026-11-20)

---
id: F-CLR-009
title: HDR and wide-gamut sources in an SDR edit — tone and gamut mapping at decode
status: candidate
priority: A
cost: L
category: color/management
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-ffmpeg, kt-media, kt-gpu, kt-engine]
invariants: [4, 7, 10, 11]
ui_rules: [4, 16]
round: null
point: change hdr-sources-to-sdr
tests: []
depends_on: [F-CLR-008, F-MED-010]
trackers: []
services: []
checked: 2026-11-20
---

# F-CLR-009 — HDR and wide-gamut sources in an SDR edit — tone and gamut mapping at decode

## Essence

A phone films HDR by default: an iPhone writes 10-bit HEVC in BT.2020 primaries with the HLG transfer,
and many Android phones and action cameras write HLG or PQ. In an SDR edit such a clip must be TONE-mapped
(highlights rolled into the SDR range) and GAMUT-mapped (BT.2020 colours into BT.709), or it looks grey
and washed out next to the SDR clips. That is the most common colour complaint about every editor that got
it wrong.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | ACES / HDR project in 32-bit float; "View transform" | an 8-bit project shows HLG/PQ untouched (washed out); correct mapping needs the 32-bit float project with ACES and a view transform |
| Adobe Premiere | 26 | Sequence Settings → "Automatically Tone Map Media"; per-clip "Override Media Color Space" | an SDR (Rec.709) sequence tone-maps colour-managed HDR clips; a clip Premiere does not colour-manage is NOT tone-mapped, and the fix is to override its media colour space by hand |
| DaVinci Resolve | 21 | Color Management → Input DRT tone mapping: None / Simple / Luminance Mapping / DaVinci | the timeline space is chosen by the person; input tone mapping is a project choice of method; colourists disagree which one to use |
| Final Cut Pro | 12.3 | Library Color Processing (Standard / Wide Gamut HDR); HDR Tools effect | a Standard (Rec.709) library converts HDR clips automatically; the HDR Tools effect adjusts the conversion per clip |
| After Effects | 26 | Project colour settings, OCIO/ACES | a working space and a display transform; HDR needs a 32-bit project |

## The trick behind it

**1. The conversion must happen before the picture is quantized to the working bits.** A PQ signal spends
half its code values above 100 cd/m². Squeezed to 8 bits FIRST and tone-mapped after, the SDR range keeps
only a few dozen levels, and gradients band. Every editor that tone-maps in an 8-bit pipeline does it at
the input, from the source's 10 bits. One that runs a float working space can do it later. So in an 8-bit
pipeline "where" is not a style choice.

**2. Premiere's failure mode is the silent one: a clip it does not recognize is not tone-mapped at all.**
The fix the forums repeat is to override the clip's colour space by hand. A tag we read is the trigger, and
an untagged HDR file stays washed out unless a person can say what it is. That is the override on the FILE
from F-CLR-008, trick 2.

**3. The METHOD of the tone map is a matter of taste, and the professionals disagree.** Resolve offers
four methods, and colourists argue for "None", "DaVinci" and "Luminance". A non-colourist wants one that
looks right by default. The standards' recommendation (ITU-R BT.2408 for the HDR reference white, BT.2390's
roll-off for highlights) is the defensible default.

## Pros

- iPhone and action-camera footage looks as it does on the phone's SDR share, next to SDR clips, with no
  setting.
- Deterministic: one integer pipeline, the same bytes in preview and export, on the processor and on the
  device.

## Cons

- The HDR is gone from the edit: the working space stays SDR BT.709 8-bit, and an HDR export needs a
  higher-precision pipeline (F-CLR-008).
- One default look for every clip; a per-file override and a choice of method are follow-ups.

## What users say

> iPhone 14 Pro HDR footage in Premiere Pro looks washed out and I've tried everything
> — thread title, Adobe Premiere Pro community,
> https://community.adobe.com/questions-729/iphone-14-pro-hdr-footage-in-premiere-pro-looks-washed-out-andi-ve-tried-everything-1406993 (accessed 2026-11-20)

> iPhone Rec 2100 HLG footage becomes washed out in Premiere Pro on Windows
> — bug report title, Adobe Premiere Pro community,
> https://community.adobe.com/bug-reports-728/iphone-rec-2100-hlg-footage-becomes-washed-out-in-premiere-pro-on-windows-1635208 (accessed 2026-11-20)

## How it fits Kinotrain

CAPABILITY: openspec/specs/media-index/

Today `kt_ffmpeg::colour` (source-colour-matrix) reads the matrix, range and primaries, and swscale
converts with the BT.2020 MATRIX only (`src/libs/kt-ffmpeg/src/colour.rs`, `source_impl.rs`). The transfer
is not read, there is no tone or gamut map, and the 10 bits are cut to 8 at the same step (found-defects
724). The export writes BT.601 limited by default (`src/libs/kt-ffmpeg/src/format.rs`, found-defects 1054).

F-CLR-008's rule "the conversion lives in the plan" cannot hold for THIS half. The burst cache holds
8-bit RGBA (invariant 4), and by trick 1 the tone map must see the source's 10 bits. So it is the
decode's step, as the matrix already is. It is one integer pipeline: the processor decode runs it now,
and the device decode (F-MED-010) runs the same integer pass in WGSL, byte-equal (invariants 7, 11). The
FFmpeg symbols it needs are the colour tags already read (invariant 10).

## Estimate

| What | Answer |
|---|---|
| Size | L: the transfer and mastering tags, an integer pipeline with frozen tables, the device twin, export tags |
| Where the main work is | kt-ffmpeg (`colour.rs`, the decode's conversion), kt-media (facts), kt-gpu (the WGSL twin) |
| Project format | none |
| Tests that will have to be written | an SDR source is byte for byte as before; PQ and HLG fixtures map to stored frames on every CPU; the device twin equals the processor; the export default's tags |
| Depends on | F-CLR-008 (tags, the working space decision), F-MED-010 (the device decode) |
| Breaks compatibility | no: SDR sources and the goldens do not move |

## Verdict

**Candidate, designed** (change `hdr-sources-to-sdr`). It waits for the owner's answers on the working
space, HDR output and the export default.

## Sources

- Adobe community threads and bug reports on washed-out iPhone HLG footage (links above, 2026-11-20)
- PetaPixel, "Adobe Premiere Pro Gets Automatic Tone Mapping for HDR Footage", 2023-02-16:
  https://petapixel.com/2023/02/16/adobe-premiere-pro-gets-automatic-tone-mapping-for-hdr-footage
- DaVinci Resolve 18.6 manual (mirror), Input DRT tone mapping:
  https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part300.htm
- EBU tech-qual BPN 125, "High Dynamic Range (HDR) in NLE Systems — DaVinci Resolve":
  https://tech-qual.ebu.ch/publications/bpn125

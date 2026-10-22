---
id: F-CLR-008
title: Color management — colour space tags, working space, output
status: researched
priority: B
cost: XL
category: color/management
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-ffmpeg, kt-media, kt-engine, kt-gpu, kt-model, kt-app]
invariants: [7, 9, 11]
ui_rules: [4, 16, 19]
round: null
point: null
tests: []
depends_on: [F-CLR-005, F-MED-004]
trackers: []
services: []
checked: 2026-09-23
---

# F-CLR-008 — Color management — colour space tags, working space, output

## Essence

A phone shoots HLG BT.2020, a camera — log, a screenshot — full-range sRGB, an old DV — limited-range BT.601. Placed side by side without conversion they differ in colour and brightness, and an export without tags forces the player to guess. Colour management is three decisions: what the file means (tags), what we compute in (the working space), and what we output to (output with tags).

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Project Properties → Pixel format (8-bit / 32-bit full range / video levels), ACES Color Space | the working space is a property of the project; the interpretation of the file — Media Properties [NOT VERIFIED against the primary source] |
| Adobe Premiere | 26 | Lumetri Color → Settings; Sequence Settings → Working Color Space; Interpret Footage → Color Space Override | three levels: the file, the sequence, the output [NOT VERIFIED against the primary source] |
| DaVinci Resolve | 21 | Project Settings → Color Management: DaVinci YRGB / DaVinci YRGB Color Managed / ACES; Input and Output color space, per-clip Input Color Space | the rule belongs to the project, the override to the clip; with colour management off everything is computed "as is" [NOT VERIFIED against the primary source] |
| Final Cut Pro | 12.3 | Library → Color Processing (Standard / Wide Gamut HDR); Project → Color Space; Info inspector → Color Space Override | the working space is a property of the LIBRARY: Standard — "linear RGB with Rec. 709 color primaries, the color space that previous versions of Final Cut Pro have used"; the output is a property of the project: "the project's render files are generated in that color space, and your project is exported in that color space if the destination codec supports it" |
| After Effects | 26 | Project Settings → Color: Working Space, Linearize; Interpret Footage → Color Management | the working space belongs to the project, the interpretation to the footage [NOT VERIFIED against the primary source] |

## The trick behind it

**1. In FCP the working space is a property of the LIBRARY, not of the project.** There is one more level above the project, and it cannot be switched for a single edit; but two projects of one library cannot diverge in mathematics. In Resolve and AE the same choice sits on the project — and two timelines of one project also share it.

**2. The tag override is on the FILE, not on the event.** Color Space Override in FCP and Input Color Space in Resolve correct the interpretation of the media: one file — one value, no matter how many times it stands on the timeline. This is the opposite of the frame conform rule (F-MED-004), where the exception lives at the event: "what the file means" is a fact about the file, "how to place it" is a decision of the edit.

**3. The output "in the project space, if the codec supports it" is a silent condition.** FCP writes render files in the project's space and exports in that same space "if the destination codec supports it"; when it does not, it converts without asking. The output tag is part of the export preset, not of the project (kin of F-EXP-001).

## Pros

- One level above the project (FCP) — the projects of a library do not diverge in mathematics.
- An override on the file — one correction fixes all instances.
- Resolve's managed mode — log cameras and phones fall into one row without a LUT.

## Cons

- Enabling colour management changes the look of EVERYTHING that was assembled without it.
- A linear/f32 working space doubles the memory of intermediate buffers and breaks the 8-bit byte references.
- A wrong tag in a file (a frequent case) is visible only to the eye.

## What users say

> the project's render files are generated in that color space, and your project is
> exported in that color space if the destination codec supports it.
> — Final Cut Pro User Guide 12.3, "Use wide-gamut HDR color processing",
> https://support.apple.com/guide/final-cut-pro/use-wide-gamut-hdr-color-processing-ver1cd9629a5/mac
> (accessed 2026-09-23)

NO QUOTES: no living complaints were sought — the note was filed under R.62's decision as a record of a deferred item; the search budget of the round went to frame conform and track conform.

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-plan/

### What we already have

- The pipeline is 8-bit RGBA without tags: `to_rgba` (`src/libs/kt-ffmpeg/src/source_impl.rs`)
  calls `sws_getContext` without colour-space or range flags; the frame is computed "as is".
- The export — `MuxConfig` (`src/libs/kt-ffmpeg/src/lib.rs`) — without colour-space tags; the recipient's player guesses.
- Colour correction by event — ColorGrade/Duotone/Posterize (F-CLR-005), LUT (F-CLR-004) — everything in 8 bits on top of "as is".

### What is missing

- Tags in `MediaInfo` (`src/libs/kt-media/src/lib.rs`): the colour space, the transfer function, the range — kt-ffmpeg reads them for free, but does not expose them.
- A working space of the project and output tags in the export.
- A conversion on input (range 16–235 → 0–255 at least) — the cheapest and most frequent half.

### What gets in the way

- Invariant 11: any conversion on input moves the bytes; the golden references are taken on synthetic footage without tags — they must be enough to keep "no tags = as is" byte for byte.
- Invariant 7: the conversion must live in the plan, as a CPU + WGSL pair, not in the decoder (the lesson of F-MED-004: a decision in the decoder is outside the single code).

## Estimate

| What | Answer |
|---|---|
| Size | XL: tags in the facts of the source, the working space, the conversion as a CPU + WGSL pair, output tags, the project window |
| Where the main work is | kt-ffmpeg (reading tags), kt-media, kt-engine/kt-gpu (conversion), kt-model, kt-app |
| Project format | fields of the project and of the media, a round version |
| Tests | "no tags = as is" byte for byte; range 16–235 → 0–255 on a fixture with a tag; GPU conformance |
| Depends on | F-MED-004 (the project window, the facts of the source), F-CLR-005 |
| Breaks compatibility | no, given the default "as is" |

## Verdict

**Deferred (R.62).** The owner named "different colour spaces" in one claim together with the aspect ratio; the reconnaissance showed that this is a separate round the size of transitions (tags, working space, a pair of kernels, output). The first reasonable piece — only tags in the facts of the source and the range on input, with the default "as is" byte for byte. Take it after F-MED-004: the project window and the facts of the source appear there.

## Sources

- Final Cut Pro User Guide 12.3, "Use wide-gamut HDR color processing" and "Intro to wide color gamut and HDR":
  https://support.apple.com/guide/final-cut-pro/use-wide-gamut-hdr-color-processing-ver1cd9629a5/mac ,
  https://support.apple.com/guide/final-cut-pro/intro-to-wide-color-gamut-and-hdr-ver09be4f91f/mac
  (accessed 2026-09-23)
- Vegas / Premiere / Resolve / After Effects — from memory of the interface, the primary sources were not opened [NOT VERIFIED]

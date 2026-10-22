---
id: F-COL-011
title: Source timecode as a common address
status: candidate
priority: B
cost: M
category: collab/handoff
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: partial
crates: [kt-time, kt-otio, kt-model, kt-app]
invariants: []
ui_rules: [3, 14]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [frame-io, bm-cloud]
checked: 2026-09-08
---

# F-COL-011 — Source timecode as a common address

## Essence

Three people speak about one frame. The editor says "at 4:12 from the start", the reviewer — "at 00:04:12 in the sent reel", the colourist — "shot 14, the second frame after the splice". Rearrange one splice — and two of the three addresses are dead: everything to the right has
moved. Only one address does not die — the timecode of the SOURCE: it is embedded in the material before the edit even exists, and does not move from a rebuild, nor from a renaming of the file, nor from a change of editor.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Media Properties → the timecode of the media; the source timecode in the Trimmer | the address is read from the file and shown beside the edit time; on the timeline the ruler lives in its own coordinate system `[NOT VERIFIED]` |
| Adobe Premiere | 26.x | Source Timecode; Modify → Timecode of a clip; the Media Start column in the project; the Timecode effect for burn-in | two times are visible at once (source and sequence), and "Modify → Timecode" allows declaring to the material a different starting point, if the camera did not record it |
| DaVinci Resolve | 21 | Source TC / Record TC, Change Clip Timecode, Data Burn-In (Source Timecode), conform by XML/AAF/EDL | the timecode is a working key of the conform on a par with the reel name; the comparison of offline with online is done by the burnt-in numbers, not by positions |
| Final Cut Pro | 12.3 | the source timecode in the inspector and the viewer; a third-party burn-in | the library's structure addresses by events and clips, not by tracks and time, so the address exists, but it is not the centre of the pipeline — the source of the address's visibility is historically catching up |
| After Effects | 26.x | the starting timecode of the footage; the Timecode effect with the Source Timecode mode | the footage element has its own starting point, the layer remembers it, while the composition lives in its own time — the two times are separated more strictly than in an NLE |

**Outside the core.** Without OpenTimelineIO this feature cannot be described: all five editors
show the address to the user, but NOWHERE is it said in which coordinate system it lives
in the exchange file — and precisely this determines whether the project travels an hour out or not.
Our own mechanism comes from here.

| Programme | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18.x | `available_range.start_time`, `source_range`, `global_start_time` | there is NO separate "timecode" field: `source_range` lives in the same coordinate system as `available_range`, and its start IS the embedded timecode; `global_start_time` is the same, but for the whole edit |

## The trick behind it

**The address is not a number but a PAIR "reel + timecode", and the second half is embedded in the
material before the edit exists.** The timecode by itself is not unique: two cards of one shooting day start from one hour, and many cameras
start from zero at every power-on. The address is made unique by the reel name,
recorded at the dailies stage — and precisely for this reason it survives the renaming of the
file, whereas "the key by file name" breaks at the first foreign sort. Hence a practical conclusion
that is absent from every list of features: a conform that has converged "almost entirely" has usually converged incorrectly — it found the same
timecode on a different reel.

**The second layer: in the exchange file there is no timecode as a field at all.** There is a coordinate system:
the `source_range` of a clip is expressed in the same coordinates as the
`available_range` of the material, and its start IS the embedded hour. An editor that
internally counts the entry point from the ZERO frame of the file is obliged to subtract this
start on reading and return it on writing — otherwise it drifts into the file by an hour and
renders emptiness. This is exactly the detail that is not visible in any interface and
for which one pays with a conform that arrived black.

**The third layer: the edit too has an address, and it is FALSE.** Record TC by convention
starts at 01:00:00:00 and after one insertion shifts entirely — all remarks written against it, after a rebuild, point to the neighbouring shot, and look
correct. Therefore serious pipelines ask for remarks in SOURCE addresses,
and leave the edit time for conversation within one version. The difference between the two
addresses is not in accuracy but in who their author is: one was written by the camera, the other — by the last edit.

## Pros

- The address survives a rebuild: it is tied to the material, and the material is not changed by an edit of the montage. This is the only property for whose sake the mechanism exists at all.
- The address survives a change of programme: the timecode and the reel are read by everyone, including those who know nothing about the format of a foreign project.
- The address survives the renaming and moving of files — that is, precisely the two events that break all the other ways of linking an edit to the material.
- An address burnt into the picture requires nothing at all of the second side: neither a programme, nor a user account, nor access to the project. There is no cheaper mechanism in the industry for a conversation between two people.

## Cons

- **The address is not unique without a reel**, and not everyone writes reels. Material from a phone, a screen capture, an export from a foreign programme arrives without both fields — and the pipeline silently falls back to the file name.
- **Fractional rates.** 23.976/29.97/59.94 are not frame rates, but 24000/1001 and its neighbours; drop-frame counts labels, not frames, so one and the same frame has two legitimate notations (DF and NDF), differing by 3.6 s per hour. An error at this point looks like "a shift of a couple of seconds towards the end of the reel".
- The timecode overflows at a day: shooting across midnight gives two different frames with the same address on one reel.
- The address speaks about the material and is silent about the edit: "fix this here" in the addresses of the source does not answer the question at which place of the film this "here" stands, if one take is used twice.
- Burning the address into the picture means spoiling the picture; a separate version with burn-in is an extra render and an extra file, which are later confused with the final cut.

## What users say

The formulation for whose sake the note was opened is about the reason, not about convenience:

> Timecode is like a train track: it tells you where you are at a given moment in
> time. Matching source timecode is the number one reason that conforms work or
> not.
> — Frame.io Insider, "The Beginner's Guide to Conforming with DaVinci Resolve",
> 29.03.2019, https://blog.frame.io/2019/03/29/conforming-with-resolve/

And in the same place — about the second half of the address, the one that is forgotten:

> While tape-based workflows are more and more rare, reel names are still
> probably THE most important, underused part of conforming.
> — the same material, 29.03.2019

Both quotes are about one and the same thing: the address must be recorded BEFORE the edit, otherwise after the edit it cannot be obtained from anywhere.

## How it fits Kinotrain

### What already exists

The honest answer: we have made the MECHANICAL half and have not made the human one.

- The address does not get spoiled. The start of `available_range` of the active media reference is subtracted on
  reading and returned on writing (`src/libs/kt-otio/src/convert.rs`,
  `active_available_range`), therefore our `source_in` is counted from the first frame of the
  file, and a foreign editor gets its own timecode back in place. Fixed by the
  test t180 (`src/libs/kt-otio/tests/convert.rs`): a clip with an hour of timecode is read as
  "10 seconds from the start of the file" and after the round is written back as the
  hour plus 240 frames.
- This was found not by reasoning but by a live film: the cross-render t179
  (`src/apps/kt-cli/tests/oc_render.rs`, the material "Sol Levante" with exactly an hour
  of timecode) — without subtraction the engine drifted an hour into a 162-second file and
  rendered emptiness.
- Foreign fields survive the round in full: `available_range`, the metadata and the `extra`
  of the media reference lie in `Retained` and return to their place, even when we
  substitute only the file address.

And that is all. Beyond this begins what does not exist:

- **There is no address in the document.** `MediaRef` is `id` and `path`
  (`src/libs/kt-model/src/project.rs`); neither a timecode nor a reel name exists in the model,
  they live only in the preserved foreign part of the file.
- **There is no address in the time crate.** `src/libs/kt-time/src/lib.rs` — ticks,
  ranges, the frame grid and the audio bursts; neither a timecode type, nor drop-frame, nor a
  non-zero origin. Zero is hard-wired: `FpsGrid::frame_index` counts from zero
  ticks.
- **There is nothing to show the address with.** `format_timecode`
  (`src/apps/kt-app/src/util.rs`) prints `00:01:23.456 F12` — hours, minutes,
  seconds, MILLISECONDS and separately the frame number. This is not a notation of the industry, and it cannot be read aloud to a second person.
- **The address cannot be entered.** `parse_timecode` rejects a four-part input
  (`if parts.len() > 3 { return None }`), that is, the string `01:02:03:14` is garbage;
  the input mask of Ctrl+L is fixed to six digits HHMMSS (`TC_MASK_DIGITS = 6`),
  there are no frames in it at all, the precision of input is a second.
- We have no fractional rates by construction: 23.976/29.97/59.94 do not divide 90000, and
  `grid_fps` rounds them to the nearest whole number (item 178). So a drop-frame address
  is not expressed by our model of time — and this is not a trifle, but half of the broadcast
  world.

OFFLINE: REMAINS — `src/libs/kt-time/src/lib.rs` — the whole arithmetic of the address remains on one machine and without a second participant: the tick is integer, the frame grid divides 90000 exactly, "frame number N of the material" is counted exactly and is repeated run to run, and the identity of the material itself is already computed by a hash in the index database; what is missing is not the network but the ORIGIN OF THE COUNT — the crate has neither a timecode type nor a non-zero starting point, therefore the address exists as arithmetic and does not exist as a name.

NETWORK COST: both sides must have the same material with the same embedded timecode — the media address — the address does not carry a frame, it REFERS to it, and the second side must have a means of dereferencing the reference: without the common material "01:02:03:14 reel A003" is a string to which not a single frame can be attached.

### What is missing

- A starting point for the material in the document (today it exists only in a foreign part of the file and disappears for synthetic sources and for files without `available_range`).
- A recording of the media address in the industry form — `HH:MM:SS:FF` — and a second line in the time display: editorial time and media time side by side, as in all five editors.
- Entry of the media address ("go to 01:02:03:14 in this clip") and search by it: exactly what a second person uses when reading a letter.
- A reel name: the field exists neither in the model nor in the UI, and there is nowhere to take it from.

### What gets in the way

Displaying the address is cheap, entering it is not: the Ctrl+L mask is fixed at six digits and is typed right to left (the user's decision), while the media address needs a seventh and an eighth digit and a different reference point. This is not "add a field" but a second mode of the widget that already exists, and it has to pass UI rule 14 ("works repeatedly", t83/t98) and UI rule 3 (the width does not jump). The second obstacle is our fractional frame rates: while 29.97 is rounded to 30, we cannot show an honest drop-frame address, and we are obliged to say so aloud rather than show similar-looking digits.

### Which invariants it touches

None of the twelve — and this is the strong argument for "cheap": the media address touches neither the time model (the tick remains the unit), nor determinism, nor the cache, nor the composition. It touches two rules of the UI checklist: 3 (nothing jumps — the width of the timecode field is fixed by the longest variant) and 14 (the feature works repeatedly — the mask must hand back focus, which is its own historical trap).

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: the starting point of the material in the model and in the format; display and entry of the address |
| Where the main work is | kt-otio (the starting point from `available_range` into the model, not only into `Retained`), kt-model (`MediaRef`: the start and the reel name), kt-app (the second time line, the entry mode), kt-time (the `HH:MM:SS:FF` notation — the only place where it may live) |
| Project format | does not touch someone else's data: the address already lies in `available_range` and returns to its place (t180); our synthetic sources get an optional field — a version is needed only if the start becomes editable |
| Tests to write | "display and entry are one and the same point": a typed `01:02:03:14` leads exactly to the frame that the second line shows at that position; "the round trip does not move the address" (an extension of t180 to our own files); a layout test "two time lines do not grow the strip"; refusal for fractional frame rates instead of similar digits |
| Depends on | nothing; F-COL-010 (an assembled copy must carry the same address) and F-MED-005 (relinking by address, not by file name) rest on it |
| Breaks compatibility | no |

## Verdict

Take it, and take it entirely locally: a media address is useful to ONE person no less than to two — it survives even one's own re-assembly. The server half is not needed at all: for the address to work, what is needed is not a server but shared material.

An honest assessment of what has been done: we closed the part that usually breaks — someone else's timecode is not lost or shifted by us (t180, found by t179) — and did not do the part the mechanism exists for: the address can be neither seen in the industry notation, nor entered, nor named aloud. Today it lies in the file as a byte and does not exist as a name. This is a rare case where "partial" means not half the work but its invisible half.

Boundaries: portability of the project to another machine — F-PRJ-003; repair of a broken path and collection into a folder — F-MED-005; how much margin travels with the edit — F-COL-010 (where it is also analysed why trimming shifts the address and what to do about it); a timecode generator as a title — F-TXT-003.

## Sources

- https://blog.frame.io/2019/03/29/conforming-with-resolve/ — The Beginner's
  Guide to Conforming with DaVinci Resolve (accessed 2026-09-08)
- https://www.provideocoalition.com/source-timecode-burn-in-comes-to-fcpx-and-is-in-the-other-nles-too/
  — why an offline reference with burned-in timecode is used and why editors are asked for it (accessed 2026-09-08)
- The rows of the table "Who has it and how" were compiled from the composition of the interfaces and help pages (Source Timecode and Modify → Timecode in Premiere, Source/Record TC and Data Burn-In in Resolve, Media Properties in VEGAS, the Timecode effect in After Effects); not verified live — none of the five has a public tracker (`docs/features/_trackers.md`, "Whom we do NOT take"). The VEGAS row is marked `[NOT VERIFIED]` in the part that concerns the timeline ruler.
- The semantics of `available_range`/`source_range`/`global_start_time` — the reference library OpenTimelineIO 0.18.x (`docs/features/_editors.md`, `otio-lib-0.18`) and our own analysis in `src/libs/kt-otio/CLAUDE.md`, section "Starting timecode of the material (item 180)".
- Our code was checked by grep over `src/` on 08.09.2026: `kt-time/src/lib.rs`, `kt-otio/src/convert.rs` (subtraction and return of the `available_range` start), `kt-otio/tests/convert.rs` (t180), `kt-model/src/project.rs` (`MediaRef`), `kt-app/src/util.rs` (`format_timecode`, `parse_timecode`, `TC_MASK_DIGITS`), `kt-cli/tests/oc_render.rs` (t179).

---
id: F-COL-025
title: How a comment is addressed in someone else's player
status: candidate
priority: B
cost: M
category: collab/review
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-fx, kt-engine, kt-media, kt-model, kt-app]
invariants: [7, 11]
ui_rules: [8]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [frame-io]
checked: 2026-09-08
---

# F-COL-025 — How a comment is addressed in someone else's player

## Essence

The person to whom a video was handed over watches it not in the editor. They have a player on the phone, a browser tab, or a file forwarded in a messenger, and they write the remark in words: "at the fourth minute the face is too dark." For this remark to get into the edit, its phrase must contain an ADDRESS, and they have nowhere to take the address from except the picture itself. That is why the review copy differs from the master in exactly two things: a timecode and the name of the version are burned into the frame.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Timecode media generator | the text is placed as an EVENT on a track, so the burn-in lives inside the edit and travels into the master if one forgets about it — **[NOT VERIFIED]** against the 2026 product help |
| Adobe Premiere | 26.x | Timecode effect on a track; "Metadata & Timecode Burn-in" in Adobe Media Encoder | two different places and two different fates: the effect is visible in the preview and travels with the edit, the encoder's overlay does not exist in the preview at all and lives in the export job |
| DaVinci Resolve | 21 | Data Burn-In (Workspace), with a separate switch for rendering | a setting of the WORKING SPACE level, not of the clip: a set of fields (source timecode, edit timecode, file name, arbitrary text) is assembled once and switched on before output — **[NOT VERIFIED]** as to the exact set of fields in version 21 |
| Final Cut Pro | 12.3 | no native burn-in; timecode generator and third-party plugins | historically catching up: burning in the SOURCE timecode became possible here later than anywhere else, and it is done with third-party titles — **[NOT VERIFIED]** |
| After Effects | 26.x | Timecode effect with the Source Timecode mode | a layer above the composition, that is, the same "effect on a track" as in Premiere: visible in the preview, travels into the master if one is forgetful |

## The trick behind it

**1. An address drawn by the PLAYER does not survive forwarding — only pixels survive.** The review service shows the timecode next to the window, and while a person sits on its page, everything works. But the video gets posted to a messenger, forwarded on, downloaded and opened in a system player — and nothing of the address remains: the foreign player counts time from zero of the file, and the messenger on the way also re-encodes the picture. Burn-in is not decoration or a television habit, but the only part of the video that travels TOGETHER with the image.

**2. Two things are ALWAYS burned in, and the second is more important than the first.** The timecode answers "where," the name of the version answers "in what"; without the second the first is ambiguous, because "00:04:12" is in every version, and a remark that arrives a day later cannot be told apart from a remark about the previous round. Hence a rule that is in none of the lists of features: the name of the version in the corner is worth more than the timecode, although it looks smaller.

**3. There are two timecodes in the frame too, and they die at different times.** The source timecode is burned in by the camera and moves by nothing; the edit timecode shifts entirely from one insertion. Therefore a serious review copy carries both: by the first one finds the material, by the second one talks within one version — and as soon as the version has changed, the second address is dead while the first is alive.

**4. Mechanically, burn-in almost everywhere lives OUTSIDE the timeline — and this is a conscious choice with a price.** In Resolve it is a workspace setting, in Adobe it is an overlay in the encoder: that way it cannot be accidentally left in the master. The cost is exactly what the reviewer sees: one thing in the preview, another in the exported file. Where burn-in is made as an event on a track (VEGAS, a layer in After Effects), the picture is honest, but a forgotten title travels into the final cut — a known way to disgrace oneself.

## Pros

- Requires NOTHING of the other side: no program, no account, no access to the project. In the industry there is no cheaper mechanism for a conversation between two people.
- Survives forwarding, re-encoding and a change of player — that is, exactly the three events that kill all other ways of pointing at a frame.
- Works without a network and without a service: a file on a flash drive is addressable in the same way as a file by link.
- Makes a remark verifiable after the fact: the numbers are visible on the frame itself, not in the correspondence next to it.

## Cons

- It spoils the picture: the review copy stops being what is judged visually, and the conversation about color and framing goes on top of the caption.
- It requires a SEPARATE render: a second file appears, which is later confused with the final cut — a classic, and the reason burn-in was moved into the output settings.
- A forgotten switch works in both directions: either the master with the numbers, or a review copy without the address, and both errors are discovered by the addressee.
- The address in the frame is read by a person and carried over by hand: the return path into the edit is not solved by burn-in at all (F-COL-026).
- Small type and messenger compression: numbers squeezed into 720p and compressed twice stop being legible — the caption size turns out to be a parameter that nobody thinks about in advance.

## What users say

The wording of the reason — why it is the SOURCE timecode and not the time of the edit window:

> I want to send some smaller version of the clips to someone on DVD and have
> them make notes that reflect the original/source timecode rather than the
> timeline window numbers.
> — Bruce Schultz, Creative COW, "Source timecode burn in???", 30.04.2009,
> https://creativecow.net/forums/thread/source-timecode-burn-inaeaeae/

And the same from the side of a client, seven years later:

> Client wants burn-in source timecode, files names and record timecode, among
> other things.
> — olivac44, Adobe Community, "Premiere Pro CC and Burn In Timecode/Source
> Name," 28.07.2016,
> https://community.adobe.com/t5/premiere-pro-discussions/premiere-pro-cc-and-burn-in-timecode-source-name/td-p/8441009

Both remarks ask for the same thing, and neither is about convenience: without an address in the frame, a remark is not converted into an edit.

## How it fits Kinotrain

### What already exists

An honest answer: **we put text into pixels in exactly one place, and that place is the wrong one.**

- `src/libs/kt-media/src/placeholder.rs` — the stub for an unavailable file draws the FILE NAME with a built-in 3×5 pixel font. The font was introduced precisely for repeatability: "it depends neither on system fonts nor on the egui renderer, so the frame is deterministic and identical in preview, export and tests." That is, the hardest part of burn-in — deterministic raster text in a frame — is already written by us and closed by a test (t208).
- But this is a SOURCE, not an overlay: it replaces the content, rather than writing on top of it; its frame is 640×360 by default, and its glyph table is ASCII, where everything else is shown as "?".
- Time is turned into a caption by `format_timecode` (`src/apps/kt-app/src/util.rs`): "00:01:23.456 F12" — hours, minutes, seconds, MILLISECONDS and separately the frame number. This is not an industry notation (analysis — F-COL-011), and it goes to the painter of the toolbar.
- There are ten video FX (`video_fx_templates` in `src/apps/kt-app/src/fx_editor.rs`: Pan, Mirror, Blur, Greenscreen, Edges, Noise, VHS, Glow, Pan 3D, EventGrid), and not one accepts a string: an effect parameter is described by `ParamSpec` (`src/libs/kt-model/src/project.rs`) — `min`, `max`, `default`, `step` of type `f64`. A text parameter does not exist in this system by construction, and this is not an oversight: the spec simultaneously defines the automation range, and there is nothing to animate a string with.

OFFLINE: DOES NOT REMAIN — `src/apps/kt-app/src/util.rs:217` — the absence is visible exactly here: the only function that turns time into a caption gives the string to the toolbar painter, and there is no road from it into the frame — not one of the ten video FX accepts a string, and the parameter spec is numeric by construction. Today a person adds the address for the other player in words in a message next to the file, and at the first forwarding the file and the message diverge forever.

NETWORK COST: the name of the version in the corner of the frame must be issued by ONE source, not by each on its own — the user account — a burned-in "v4" costs exactly as much as the confidence that someone else's v4 is not a different video; our identifiers are issued as "maximum + 1," and the same rule on two machines gives two different videos with the same label.

### What is missing

- Overlaying text onto the frame: a compositing stage that writes on top of a finished frame, rather than instead of it.
- An industry notation of the address `HH:MM:SS:FF` and a starting point of the material — burning in "00:04:12.333 F07" is pointless, it is not read aloud (F-COL-011).
- The name of the version as data: today it cannot be obtained from anywhere, because the video's versions do not exist with us at all (F-COL-024).
- A "review copy" export mode: a single switch that makes the second file differ from the master and says so out loud.

### What blocks it

**The main obstacle is invariant 7, and it is also the main fork.** The industry puts burn-in into the EXPORT settings precisely so that it is not visible in the preview; for us this is impossible: layers, order, weights and levels are computed by one code (`VideoRenderer::frame_plan`), and the preview and the export must match byte for byte on one backend (t11/t67/t184). So burn-in with us is part of the frame plan and is visible in the window, and "show without the caption" becomes a separate conscious decision, not a side effect of the overlay living in the encoder. This does not prevent the feature, but it forbids copying its design from neighbors.

The second obstacle — the font. The 3×5 pixel table knows Latin, digits and punctuation; a version name in Chinese will come out as a string of "?", and our interface is translated into five languages. A real font stack in the engine, meanwhile, instantly clashes with invariant 11: a system font means a different raster on different machines and a red GOLDEN_VIDEO.

The third — input: a string parameter is not drawn by `params_grid`, and "just put a TextEdit" is forbidden by rule 8 of the checklist.

### Which invariants it touches

- **7 (one composition code).** Burn-in must pass through `frame_plan`; an overlay "only at export" cannot exist with us — that would be a second path of composition.
- **11 (determinism).** The raster of the caption must coincide between runs; the built-in pixel font of the stub is already a ready answer to this requirement, and the only one that does not drag in system fonts.

Of the UI rules, one is touched — 8: the field for the version name is drawn in the same grid of parameters as everything else, not as a separate widget "however it turns out."

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: text overlay in the frame plan (a shared raster font, moved out of the stub); sources of the caption — the address and the version name |
| Where the main work is | kt-fx (the overlay stage), kt-engine (the place in `frame_plan` and in export), kt-media (the font moves from the stub to a common place), kt-app (the review-copy switch) |
| Project format | a format version is needed if the caption is part of the document; as an output parameter it does not touch the format at all |
| Tests that will have to be written | "preview == export with burn-in" (an extension of t67/t184 — otherwise invariant 7 is broken); "the raster of the caption is byte-for-byte identical between runs" (in the spirit of t208); "a non-ASCII name does not turn the label into a fence of question marks — either a refusal or an honest transliteration" |
| Depends on | F-COL-024 (the version name as data), F-COL-011 (the address in industry notation), F-TXT-001 (titles — a neighboring but different task) |
| Breaks compatibility | no |

## Verdict

We take it, and we take it entirely locally: burn-in is the only feature of this wave that requires nothing at all of the other side — that is its very meaning. The server half is absent here not for lack of means, but by the nature of the mechanism: it is deliberately built to work in someone else's player without us.

An honest assessment of what has been done: we have the least obvious part — deterministic pixel text in a frame — and it stands at the input of the pipeline instead of at the output. We know how to draw a caption INSTEAD OF missing material and do not know how to draw it ON TOP OF existing material. The difference between these two sentences is the whole job.

The difference from F-TXT-001 (titles): there text is content, a person writes it, it is animated, and it is part of the work. Here text is a service caption, its content is taken from the document (address, version number), it cannot be edited frame by frame, it must be deterministic, and it must exist in the export path. The requirements are almost opposite, which is why there are two notes.

Boundaries: the address as such (source timecode, reel) — F-COL-011; the version of the video whose name is burned in — F-COL-024; what to do with a remark that came back — F-COL-026; titles and generators — F-TXT-001 and F-TXT-003.

## Sources

- https://creativecow.net/forums/thread/source-timecode-burn-inaeaeae/ — why the review copy needs the SOURCE timecode and not the time of the edit window (access date 2026-09-08)
- https://community.adobe.com/t5/premiere-pro-discussions/premiere-pro-cc-and-burn-in-timecode-source-name/td-p/8441009
  — the client's requirement: source timecode, file names and edit timecode in the delivered file (access date 2026-09-08)
- https://community.adobe.com/t5/premiere-pro-beta-bugs/nonsense-appearing-in-metadata-amp-timecode-burn-in-effect-using-ame/idi-p/15506146
  — confirmation that at Adobe the overlay also lives in the encoder, separately from the effect on the track (access date 2026-09-08)
- The rows for VEGAS, Resolve and Final Cut were compiled from the composition of interfaces and help pages; they were not checked live, the `[NOT VERIFIED]` marks stand in the table itself
- Our code checked by reading on 08.09.2026: `src/apps/kt-app/src/util.rs`
  (`format_timecode`), `src/libs/kt-media/src/placeholder.rs` (3×5 font, determinism, ASCII table), `src/apps/kt-app/src/fx_editor.rs`
  (`video_fx_templates` — ten effects), `src/libs/kt-model/src/project.rs`
  (`ParamSpec`), root `CLAUDE.md` (invariants 7 and 11)

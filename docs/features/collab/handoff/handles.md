---
id: F-COL-010
title: Handles: how much extra moves with the edit
status: candidate
priority: B
cost: L
category: collab/handoff
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-engine, kt-media, kt-cli]
invariants: [5, 9]
ui_rules: [16, 19]
round: null
point: null
tests: []
depends_on: [F-MED-005]
trackers: []
services: [bm-cloud, frame-io, fcp-shared-library]
checked: 2026-09-08
---

# F-COL-010 — Handles: how much extra moves with the edit

## Essence

The edit has gone away: to colour, to sound, to the client, to another machine. Together with it the
material must go too — but not all of it, otherwise these are terabytes, and not "exactly at the cuts", otherwise the receiving side cannot move a cut by a single frame. Between these two extremes
stands one number: how much surplus material to leave on each side of each
event. The industry answer is 24 or 48 frames, and every conform asks for it.
Without this number the gathering of material is useless, even when it is technically successful.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Save As → "Copy and Trim Media with Project", the field "Extra head and tail" | trimming is a side effect of saving the project; the margin is set in frames, but only DV AVI, uncompressed AVI and Sony YUV AVI are cut, everything else is copied as a whole file |
| Adobe Premiere | 26.x | Project Manager → "Consolidate and Transcode" / "Collect Files", the field "Include Handles: N frames" | a separate dialogue per project; trimming is performed only if all four parameters match (frame, rate, fields, PAR) and the codec yields to cutting |
| DaVinci Resolve | 21 | Media Management → Copy / Move / Transcode, "Used media and trim keeping N frame handles" | the only one that has it as an independent tool with its own page, not an item of "save as"; trimming is counted per event of the timeline |
| Final Cut Pro | 12.3 | Consolidate Project/Library Media | there are NO handles at all: one used frame drags the whole file into the library. In FCP 7 trimming with handles existed and did not return in FCP X — the niche is occupied by third-party utilities |
| After Effects | 26.x | File → Dependencies → Collect Files (+ Reduce Project) | copies the sources whole; Reduce Project throws away unused ELEMENTS, but never pieces inside an element — there is no concept of a "margin" in the compositional model |

## The trick behind it

**The number of handles is a REQUEST, not a guarantee, and the silent fallback is the same for all of them:
copy the source whole.** At Premiere the boundary runs along the structure of GOP:
Project Manager does not cut long-GOP H.264/H.265, because it does not parse where the reference
frames are and which neighbouring segments are needed to restore the intermediate ones —
and the same is done by time remapping and any mismatch of frame/rate/fields/PAR.
At VEGAS the same fallback is declared as a list of formats: DV AVI,
uncompressed AVI and Sony YUV AVI are trimmed. In both cases the user ordered 48 frames of
margin, received a folder of the original size and NOT A SINGLE message — he learned of it by the volume.
That is, the most expensive part of the transfer (how many bytes will travel) is determined not by the
setting but by the internal design of the codec, about which the dialogue is silent.

**The second non-obvious layer: trimming CHANGES ADDRESSES.** The trimmed copy has a different
first point, and the project after gathering lives no longer with the same numbers: the entry points
are rewritten for the new file. While the copy and the project travel together this is unnoticed; as
soon as someone sends a remark by the timecode of the SOURCE (F-COL-011), it lands on nothing —
the address has shifted by exactly the length of the head handle. Therefore reasonable pipelines preserve the
source timecode in the trimmed copy, and this is a separate job that the checkbox "handles" itself does not mention.

**The third: handles solve exactly one task — "move a cut" — and silently do not
solve the second.** To take another take, to return a discarded piece, to re-edit a
phrase — all of this requires material that is absent from the package by construction. A margin of
24 frames means "frame-level edits are allowed, decision-level edits are not", and this is the managerial meaning, hidden in a field for a number.

## Pros

- Handles make the transfer REVERSIBLE at the cheapest level: the receiving side moves a
  cut without requesting anything and without waiting for anyone. No server, no user account,
  no shared disk is needed for this — one agreed multiplier is needed.
- The number is one for the whole package, therefore it is carried in a letter and in the folder name.
  This is the only known agreement of the industry that does not require program support on the RECEIVING side.
- Trimming by events rather than by files compresses the transfer by an order of magnitude: long takes, of which eight seconds are used, stop travelling whole.
- Cutting on receipt is cheap: the recipient decodes nothing superfluous, and their caches warm up on short files.

## Cons

- **The fallback to the whole file is silent** — for two of the five editors it also
  depends on the codec, that is, on a property of the material that is absent from the dialogue.
- A trimmed copy is NOT the same material any more: a different size, a different hash, a different
  first point. The reverse loop (the recipient sent a correction, which has to be applied to the
  originals) converges only if the address is preserved, not the file name.
- Handles double the material at the joins: two events from one take with overlapping margins give either two files with a shared piece, or one glued file, and these are different answers in different programs — the receiving side does not know in advance how many files will arrive. `[NOT VERIFIED]` — the behaviour when margins overlap is described in the documentation of none of the five.
- Re-encoding during trimming is a generation loss. Where the codec cannot be cut without re-encoding, the choice is between "the whole file as is" and "trimmed, but no longer the master", and the dialogue calls both by one word.
- The number of handles cannot be checked after the fact: a package gathered with zero looks like a package gathered with 48 — the difference is visible only when the recipient has hit the edge and can no longer do anything about it.

## What users say

The request is formulated exactly as in the definition of the feature — "used pieces plus handles" — and the result is the reverse:

> When I try to use Project Manager to take the clips from a completed Sequence
> and transcode the use portions plus handles the system just exports the entire
> clip in full.
> — aizickg14671823, 28.12.2023,
> https://community.adobe.com/t5/premiere-pro-discussions/project-manager-not-reducing-clips-and-adding-handles/td-p/14322787

The answer in the same thread names the mechanics in plain words, and this is exactly the detail that is absent from every list of features:

> One notable limitation of Project Manager, is that they can't 'cut' any H.264/5
> long-GOP media down, because PM can't sort out where the iframes are.
> — R Neil Haugen, 28.12.2023, the same thread

At VEGAS the same thing is discussed under the title "Copy media with project – does not trim
the source media" (Creative COW, the VEGAS Pro forum,
https://creativecow.net/forums/thread/copy-media-with-project-does-not-trim-the-source/),
and the explanation there is not about a bug but about the list of supported trimming formats.

At Final Cut the same need is discussed as a missing feature: "Consolidate
export, trimming unused portions of each clip, with handles" (Creative COW, the Apple Final Cut Pro forum,
https://creativecow.net/forums/thread/consolidate-export-trimming-unused-portions-of-each-clip-with-handles/)
— in FCP 7 trimming with handles existed, in FCP X it does not, and the niche is occupied by third-party
utilities.

## How it fits Kinotrain

### What already exists

- The arithmetic of the margin is exact with us and is already written. `ClipEvent` carries `source_in`,
  `duration` and `reversed` (`src/libs/kt-model/src/project.rs`), and "how much material is consumed by the moment τ" is computed by the warp map
  (`src/libs/kt-model/src/warp.rs`, `output_time_for_source`) — that is, for an event with variable speed we know how to convert "24 frames at the output" into
  "that many ticks of material". This is exactly the case on which Premiere gives up and copies the clip whole.
- The rule of the slice is one per project: `slice_anchors`/`slice_source_in`/`clip_slice`
  (`src/libs/kt-model/src/project.rs`), and t273 stands on it — "there is no arithmetic on
  `source_in` outside a single rule". Any gathering must pass through it, otherwise the package will diverge from the edit.
- A check of "is there enough material" exists, but it is one-sided:
  `slice_fits_source` asks only about the LEFT edge (`source_in >= 0`), and `trim_max_end` limits the right edge exclusively for reversed events — for an ordinary one it returns `i64::MAX / 4`.
- Writing media is in-process and already works: `kt_engine::export.rs` via `kt-ffmpeg::Muxer`, without temporary files and without an external binary (invariant 9);
  the library can do a range (`src/libs/kt-engine/src/export.rs`).
- Paths in the file are relative, therefore a folder copied whole opens on another machine without editing the project (`src/libs/kt-otio/src/url.rs`).

### What is missing

- **The document does not know the length of the material.** `MediaRef` is `id` and `path`
  (`src/libs/kt-model/src/project.rs`), and `MediaInfo { duration, fps, … }` lives in
  `src/libs/kt-media/src/lib.rs` and appears only after the file is opened.
  So the question "are there 24 frames left beyond this cut" cannot be answered in principle from one project — the material must be at hand, which is exactly what the receiving side does not have.
- There is no gathering command: `enum Command` has neither `Collect`, nor `Consolidate`, nor `Relink` — and the last matters more than the first two, because a gathered copy must be re-addressed, and there is nothing to re-address it with (F-MED-005).
- There is no field for "how many frames of margin" anywhere: neither in the model, nor in the render window, nor in kt-cli.

### What gets in the way

Gathering is N output files and a reverse edit of `source_in` for each event,
whereas our whole export is built as ONE flat output. We cannot reuse
`export.rs` as a whole: there is layer composition there, and here we need a raw slice of
one source. Further — the `MediaInUse` guard: the path to material today cannot be substituted, it can only be deleted, and deleting occupied material is forbidden, so
without F-MED-005 a gathered folder remains a dead copy.

OFFLINE: DEGENERATES — `src/apps/kt-cli/src/main.rs` — on one machine the handles degenerate into the rule "the material is always whole": kt-cli has seven commands (version, new, convert, demo, roundtrip, render, export), export has three parameters — input, output, encoder — and not one form of delivery except a single flat file; the margin in frames can be computed, but there is no reason to, because there is nothing to cut and no one to re-address it to.

NETWORK COST: the receiving side needs BYTES, not a document — invariant 9 — cutting the margin is writing media files, and the only permitted writer we have is the in-process muxer kt-ffmpeg, which means gathering becomes a second export mode, not a separate utility next to the program.

### Which invariants it touches

Invariant 9 (export is fully in-process) — gathering must write media with the
same muxer; an external `ffmpeg` for cutting is forbidden, and this is rather a plus: a foreign
binary in the transfer pipeline is the main source of "it built for me, not for you".
Invariant 5 (one MediaWorker owns the decoder) — reading the sources for cutting
goes through its queue at a priority below playback, otherwise gathering would choke the preview.
UI rule 16 (impossibility is an error, not a faded hint) directly forbids us to repeat
someone else's fallback: "the margin did not fit" and "this codec cannot be cut" must
be errors before work starts, not a surprise in the folder size. UI rule 19
(the dialogue says where the decision can be changed later) — the gathering window must name the place
where the number of handles is changed.

## Estimate

| What | Answer |
|---|---|
| Size | L — a whole round: cutting, re-addressing, window, report |
| Where the main work is | kt-engine (writing a slice of the source through the muxer), kt-model (commands of gathering and re-addressing, the length of material in the document), kt-cli (a gathering subcommand), kt-app (window and report) |
| Project format | does not touch it: paths and entry points already exist; the length of material in the document is an optional field, and if it is introduced, it is by version |
| Tests to write | "a gathered copy renders byte for byte like the original" (golden on synthetics); "a margin of N frames yields exactly N frames, and where it did not fit — an error, not silence"; "an event with a warp map is gathered by `output_time_for_source`, not by the duration on the timeline"; "a reversed event takes its margin from the side from which it is read" |
| Depends on | F-MED-005 (without re-addressing a gathered folder does not open) |
| Breaks compatibility | no |

## Verdict

We take the local core, and only together with F-MED-005: "gather the used material with a margin of N frames" without re-addressing yields a folder that cannot be opened. The server
half we do not open — there is no second participant, and everything that makes this feature
meaningful is already local with us: the cutting is in-process, the address of the material in the
file is relative, the arithmetic of the margin is exact even at variable speed.

How we differ from the neighbours. **F-MED-005** (relinking and consolidation) is about the MECHANISM of binding for ONE person: the path broke, gather everything into one folder
so that nothing is lost. Here the subject is a NUMBER, which makes sense only when transferring to a second person: how much surplus material he will receive. Consolidation without handles
is technically successful and practically useless, and vice versa — handles without
consolidation are not expressed, so the notes are neighbouring, not one.
**F-EXP-003** (smart render) is about how NOT to re-encode what is untouched when outputting one file; here re-encoding during cutting is most often mandatory
(long GOP), and the output is not one file but one file per event. What they have in common is only
the words "do not touch what is extra".

Separately: the gathered copy must carry the source timecode of the material, otherwise
the remarks of the second side stop landing on the edit — see F-COL-011, where it is
analysed what we have already done about this.

## Sources

- https://community.adobe.com/t5/premiere-pro-discussions/project-manager-not-reducing-clips-and-adding-handles/td-p/14322787
  — Project Manager not reducing clips and adding handles (accessed
  2026-09-08)
- https://community.adobe.com/t5/premiere-pro-bugs/project-manager-consolidate-and-transcode-problems/idi-p/9260441
  — Project Manager: Consolidate and Transcode problems, a multi-year thread
  (accessed 2026-09-08)
- https://creativecow.net/forums/thread/copy-media-with-project-does-not-trim-the-source/
  — VEGAS Pro: "Copy media with project" does not trim the sources (accessed
  2026-09-08)
- https://creativecow.net/forums/thread/consolidate-export-trimming-unused-portions-of-each-clip-with-handles/
  — Final Cut Pro: trimming with handles as a missing feature (accessed
  2026-09-08)
- https://larryjordan.com/articles/final-cut-pro-consolidate-media-for-archiving-or-colllaboration/
  — what exactly Consolidate does in FCP (accessed 2026-09-08)
- The Resolve and VEGAS rows in the table are compiled from the wording of their help pages
  ("Used media and trim keeping N frame handles"; "Copy and Trim Media with
  Project" with the field "Extra head and tail"); not verified live — there is no public tracker for any of the five (`docs/features/_trackers.md`, "Whom we do NOT
  take").
- Our code, checked by grep across `src/` on 08.09.2026: `kt-model/src/project.rs`
  (`ClipEvent`, `MediaRef`, `slice_fits_source`, `trim_max_end`, `clip_slice`),
  `kt-model/src/warp.rs`, `kt-media/src/lib.rs` (`MediaInfo`),
  `kt-engine/src/export.rs`, `kt-cli/src/main.rs`.

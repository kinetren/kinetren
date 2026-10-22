---
id: F-TL-004
title: Nested sequences — an edit that behaves like a clip
status: done
priority: A
cost: XL
category: timeline/structure
editors: [premiere-26, fcp-12, resolve-21, vegas-pro-2026, after-effects-26]
we_have: yes
crates: [kt-model, kt-otio, kt-engine, kt-gpu, kt-cache, kt-media, kt-app]
invariants: [3, 4, 5, 7, 8, 11]
ui_rules: [13, 17, 20]
round: R.50, R.55, R.56
point: 378-395, 434-447, 448-452
tests: [t423, t424, t425, t426, t427, t428, t429, t430, t431, t432, t433, t434, t435, t436, t437, t438, t439, t440, t441, t442, t443, t444, t445, t446, t447, t448, t449, t450, t451, t452, t453, t454, t455, t456, t457, t458, t579, t580, t581, t582, t583, t584, t585, t586, t587, t598, t599, t600, t601, t602, t639, t640]
depends_on: []
checked: 2026-09-19
---

# F-TL-004 — Nested sequences — an edit that behaves like a clip

## Essence

A piece of the edit — several events on several tracks, with junctions, transitions and levels — collapses into ONE event. From then on it lives as an ordinary clip: it is trimmed, copied, gets an effect chain, a level, a speed. You can go inside and keep editing. The point is not to remove the mess from sight: nesting sets a BOUNDARY OF PROCESSING ORDER. Everything applied outside is applied to the already assembled picture (and to the overall mix), not to each piece separately — and that is the only way to express "an effect on the result" where effects live on events.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Adobe Premiere | 26.x | Nest… (and simply dragging a sequence into a sequence) | the selection is replaced by a sequence clip; the sequence itself is a full object in the project panel with ITS OWN settings (raster, fps); the same object can be placed in several places, and an edit inside changes all of them; effects on the nest clip are computed after assembly — the official advice "reorder the render by nesting" stands on this (see F-MPH-004) |
| Final Cut Pro | 12.3 | Compound Clip | the "parent" lies in the browser, the "children" stand on the timeline: "Every compound clip in the timeline has a “parent” compound clip in the browser". Edits to the content are inherited by all instances "in all projects"; an instance has ITS OWN duration, material beyond its edge is shown as a hatched zone and does not get into the frame; independence is done separately (an independent compound / project snapshot) |
| DaVinci Resolve | 21 (checked against 20.x and forums) | Compound Clip · Fusion Clip · a timeline inside a timeline — THREE different mechanisms | a compound is an editing container: "Open in Timeline" opens it as a timeline, "Decompose in Place" unfolds it back in place; a Fusion Clip is the same selected piece, but converted into a NODE composite (the editing of the junctions moves into the Fusion graph and is no longer editing in the back-and-forth sense); the third is dragging a finished timeline from the media pool into another timeline. On the Fusion page a compound is visible as one `MediaIn` — that is, the editing does not show through inside the node [NOT VERIFIED against the primary source: forum.blackmagicdesign.com and the manual did not open from this environment, see "Sources"] |
| VEGAS Pro | 2026.0 | Nested Timeline (also a nested project) | the nest is a SEPARATE `.veg` FILE on disk, not an object inside the project: "Nested timelines are technically just nested projects, but with a big improvement: To edit it, you don't have to open it in a separate instance of VEGAS Pro". The parent must be saved before the nest is created; the nested one is offered the name "<project>_Untitled Timeline"; entry is by a button, exit by "Open Parent Timeline" |
| After Effects | 26.x | Pre-compose / nested composition | the original source of the idea: a pre-comp is an ordinary composition with its own raster, fps and duration, inserted as a layer into another. When pre-composing one layer, there is a choice of whether to LEAVE the transform and effects outside or move them inside — that is, on which side of the boundary they end up. The boundary can be UNDONE back with the Collapse Transformations switch |

## The trick behind it

**1. The nesting boundary is a controllable object, and AE gives a button to cancel it.**
Nesting is usually described as "put into a folder". In fact it is a switch of the operation order, and in AE it is two-sided: the same little square that on a vector layer means "keep rasterizing" is called Collapse Transformations on a composition layer and means "this boundary does not exist". Walter Soyka on Creative COW: "For composition layers, the switch is actually called “Collapse Transformations.”" — and then the main point: "The downside is that effects applied to the precomp layer will no longer work quite the same", because an effect that stood ON the composite starts to apply to the layers inside, in their own coordinates. Hence a practical consequence that is not in any list of features: a switch set for the sake of sharpness, or so that 3D and blend modes "leak" outward, silently changes the meaning of effects already applied. One and the same switch with two different values is also a ready answer to the question "why does nesting need a separate checkbox" (the neighboring side of the same mechanics is analyzed in F-MPH-002).

**2. A nest has its OWN duration and its OWN raster — and both cut silently.**
FCP draws the boundary honestly: go inside a compound clip and the edges are visible as a "darker crosshatched area" — the material exists in that zone but does not get into the frame until you lengthen the PARENT instance. The raster behaves the same way, only without hatching: the nest is first assembled into an intermediate frame of ITS OWN size, and only that is scaled into the parent. Therefore the classic complaint "the nest gets soft at zero scale" is not a bug, but two successive resamples: 1080p material → 1080p raster of the nest → 720p parent, and each step honestly shows 100 %. It is exactly for this reason that the advice "stabilize on the nest" works and harms at the same time: the stabilizer needs the assembled piece and a margin beyond the frame edge — and the margin is taken from the raster of the NEST, not from the source (see F-MSK-006).

**3. Everyone has a "reference", but they reference different things — and the price differs.**
FCP: "Any changes you make to the internal parts of the compound clip are inherited by all of its child clips, in all projects" — a reference to an object inside the library; an independent copy is made by a separate command. Premiere: a reference to a sequence in the project panel. VEGAS: a reference to a `.veg` FILE on disk — that is, the nest travels with the project as media, is lost as media, and "is made independent" by copying the file. The difference is not cosmetic: in the first two cases "make a copy" is an editor's command, in the third it is the job of a file manager, but the nest can be handed to another editor separately. What all three share is one and the same inconvenience: by a clip on the timeline you CANNOT SEE how many other places will change together with it.

## Pros

- The only way to express "one effect on the composite of N layers" where effects live on events: without a boundary each layer is processed separately and the result will be different (the same fork as in F-MPH-007).
- An edit inside rewrites all instances at once: forty repeats of an opening title are fixed by one edit and one undo step, not forty.
- Internal time is independent of external time, so speed, retime and reverse apply to the ASSEMBLED piece — the chain "first edit, then slow down" is expressed without falling apart into piecewise speeds.
- A nest is an ordinary event, so trim, copy-paste, transitions, level and selection work on it with ready-made code: no new gestures are needed.
- Audio gets a free sub-mix: "one reverb for ten lines" is solved without buses and sends (F-AUD-004 names this explicitly as what FCP uses in place of the missing buses).
- In OTIO nesting is a native construct (`Stack` as a child of `Track`), so the file stays understandable to foreign programs without inventing our own.

## Cons

- An intermediate raster: two resamples instead of one, softness when sizes do not match and judder when fps do not match — and both show "100 %" and "no keyframes", that is, they look like correct work.
- Reference semantics are invisible on the timeline: an edit inside silently changes all instances; to get a copy one has to KNOW a separate command.
- Coordinate systems come apart: a mask or a track made inside does not know about the transform outside, and tracking done outside attaches to the already assembled picture — which is why tracking and masks regularly "break from nesting" (F-MSK-005, F-MSK-001).
- Exchange with foreign programs: what goes out is a mixdown, not tracks — for a sound department a nested edit is worse than a flat one.
- The cost grows non-linearly with depth: each level is its own composite, its own intermediate buffer and its own cache, and effects that depend on time deprive the inner levels of reuse between frames.
- Diagnostics: to understand where a frame came from one has to enter N levels and come back out; an error at any level looks the same from outside.
- A duration boundary: material beyond the edge of a nest exists but is not shown — the edit is made not where the problem is visible.

## What users say

On softness — and on what the workaround turns into:

> My main sequence is 720p. I have a ton of footage in 1080p. To enable warp
> stabilize, I nested the footage in a 1080p sequence. […] The scale of the
> original footage inside the 1080p nested sequence is at 100%. On the main
> timeline, the nested sequence is at 100% as well, so there is absolutely
> zero scaling. […] Despite this, I see a noticeable decrease in picture
> quality when playing from the main timeline.
> — JHVW, community.adobe.com, 05.09.2018, https://community.adobe.com/t5/premiere-pro-discussions/unscaled-nesting-lowers-resolution-quality/m-p/14219625

> Just wanted to say I've got the same problem! The only workaround would be
> exporting the nested sequence and then putting that exported video into the
> timeline.
> — dylans84529781, community.adobe.com, 08.11.2023, https://community.adobe.com/t5/premiere-pro-discussions/unscaled-nesting-lowers-resolution-quality/m-p/14219625

On the cost of nested editing for everyone who receives the project after you:

> I never advise editing with nests, as it makes sending anything out for
> separate work impossible…if not extremely challenging.
> — Shane Ross, Creative COW, 29.05.2015, https://creativecow.net/forums/thread/editing-with-nested-sequences-good-or-bad-ideaae/

On dissolving the boundary:

> The downside is that effects applied to the precomp layer will no longer
> work quite the same.
> — Walter Soyka, Creative COW, 04.05.2018, https://creativecow.net/forums/thread/collapse-transformations-continuous-rasterise-rem/

> Continuous Rasterize can definitely make things go wonky sometimes and
> should be avoided at the design stage whenever possible.
> — Gary Reisman, Creative COW, 03.05.2018, https://creativecow.net/forums/thread/collapse-transformations-continuous-rasterise-rem/

## How it fits Kinotrain
CAPABILITY: openspec/specs/compositions/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
A reel is a reference inside the project, one address space; the laws E (inline == expanded) and G (a level on a nest ≠ a level on each).

## Estimate

| What | Answer |
|---|---|
| Size | XL — a whole round: a recursive model, a plan tree, both executors, the format, the UI mode |
| Where the main work is | kt-model (schema, ids, cycle validation), kt-engine (the plan node + audio sub-mix), kt-gpu (executing the subtree), kt-otio (Stack on write and on read), kt-app (entry/exit, breadcrumbs, filmstrip) |
| Project format | a new version of the kt envelope is needed; a foreign nested Stack stops being a `ForeignEvent` — import behavior changes incompatibly |
| Tests that will have to be written | round-trip of the model with nesting; refusal on a cycle; preview == export on the tree (extension of t67/t184); CPU↔GPU tolerances on the subtree (t181/t182); idempotence of `.otio` with Stack (t144-t147); moving `nested_example` from smoke to the strict tier t158; undo of an edit inside a nest; no decode on scroll over a nest (invariant No. 8) |
| Depends on | nothing; depending on it are F-MPH-007 (the composite stage), F-AUD-004 (sub-mix instead of buses), F-TXT-002 (title templates), F-MSK-006 and F-MSK-005 (coordinates inside versus outside) |
| Breaks compatibility | yes — importing foreign files with nested Stacks starts giving a different model |

## Verdict

We take it, and we take it early — but not as "a grouping convenience" — as a revision of the composition plan. This is the most structural position of the base: F-MPH-007 (effect on the composite), F-AUD-004 (one chain per group of lines) and F-TXT-002 (templates) rest on it — and it also closes a known compatibility gap recorded in item 158 (`docs/porting-notes.md`): today we honestly read foreign nested compositions and honestly draw an empty space in their place.

The A priority is taken deliberately: all five have nesting, and there is no workaround that does not break non-destructiveness — the workaround is "render it and drop it in as a file", exactly as in the quote above. Before the round, TWO decisions of the owner are needed, and they determine half of the code: does a nest inherit the parent's settings (we are against it in all five, but then there is no softness and no judder), and is it a reference or a copy (and if a reference — to an object inside the project, as in Premiere and FCP, or to a file on disk, as in VEGAS).

**Addendum R.56 (items 448-452).** Three items from "outside the scope of the round" were closed by direct demands of the owner: blending of frames on the nest itself now exists (item 448, and together with it the internal time of the composition became continuous — otherwise an animation inside a slowed reel went in steps), and the reel event on the timeline acquired FRAMES and an envelope (item 452) — by offline rendering of the content by key (reel, edition), not by a thumbnail from the file. The note "a reel has no filmstrip and never will" was removed as a wrong formulation: what was impossible was one method, not the task. Texture memoization of the sub-composite on the GPU and a disableable boundary (Collapse Transformations) are still not done.

**Addendum R.58 (item 459).** The price of the "own raster" of a nest surfaced, which the reconnaissance had named in general terms: **the size of the sub-composite and the input of the instance's chain are different quantities**. One stage is assembled per reel (the content is shared, the cache keys must match), but not every instance chain can consume it — an empty one, one made of a single layer force, or of barriers without a resample cannot. Until this was separated, such an instance either brought down the debug build or SILENTLY disappeared from the frame. The very same fork stands behind the advice "stabilize on the nest" from the section "The trick behind it": the margin is taken from the raster of the NEST, and who spends it is a question separate from who creates it.

## Progress

**Wave A (R.50, items 378-383) is closed.** The owner answered both questions that the note called mandatory: a nest is a REFERENCE TO AN OBJECT INSIDE THE PROJECT (the Premiere/FCP path, not a file on disk), and it INHERITS THE PARENT'S SETTINGS (it has no fps/raster/rate of its own — so no intermediate resample arises). Beyond that, he decided to close the gap of item 158 at once: foreign nested `Stack`s are read as real reels already in this round, and inside a nest one can put everything, including morpher layers and cameras.

Done: the tree model with guards, commands and gestures for collapse/expand/unlink, the format (`Stack` as a child of `Track` + the sidecar `kt.sequences`, `KT_META_V6`), expansion in `effective_clips` and law E (the inline version is byte for byte equal to the expanded content), reading of foreign nests.

**Wave B (items 384-391) is closed.** The nest became a real node of the plan: not a new `PlanLayer` branch, but an INPUT of the layer (`LayerInput::Nest`) — it is a clip, it has a level, a fade and a camera. The chain on the nest is now computed on the ASSEMBLED picture (law G), and that is what everything was started for. Audio was made a virtual medium under the fetcher, so the order of f32 summation did not change and GOLDEN_AUDIO is intact. The GPU got its own parser and `Pipe::Unpremul`. Along the way a defect found by the reconnaissance before it appeared was cured: premultiplication at the nest boundary.

**Wave C (items 392-395) is closed.** Entering inside is done by an exchange of content: the interface did not change by a single line, but the document goes out. You can enter in four ways, and exit with Esc (last in the cascade) and with the path breadcrumb; the cursor is translated, the view returns entirely.

Left outside the scope of the round (beyond the plan's volume): a disableable boundary (Collapse Transformations in AE), the nest's own settings (the owner's decision is the opposite) and texture memoization of the sub-composite on the GPU.

**R.56 (item 448): the internal time of the composition is CONTINUOUS.** The floor on the grid is set by a SHEET, and only a file one; the procedural (titles, generators, automation, cameras) inside a slowed composition no longer goes in steps. Frame blending on the nest was done by the same item — the mode is inherited by the content, and the pair is assembled from the LEAVES, so the sub-composite per frame remains one.

**R.58 (item 463): the reel preview on disk.** The owner's claim: "Compositions do not seem to cache their preview on the timeline, and every project opening rebuilds it from scratch." The frames and the envelope of the reel now live in the ready-made sidecar `<project>.ktindex.db` (the owner's decision; the option "shared data folder" was rejected) — the tables `reel_strip`/`reel_frame`/`reel_peaks`, added additively, without a schema bump. The R.56 note "a sidecar is impossible — it is keyed by file identity" was a property of the `ensure_media` API, not of the schema: there are no foreign keys in the DB. The key on disk is (reel, edition `content_hash`, media fingerprint): the edition does not see a substitution of the source, so the third part of the key is xxh3 over (path, size, mtime) of all files of the reel's tree, computed in the task thread, not in the UI. A hit returns the strip and the peaks without a single decode; a miss renders and writes in one transaction after the peaks. A project without a path — RAM, as before. Along the way two holes of the service were closed: the LRU became honest (an access refreshes the record), the negative cache got a ceiling, and invalidation of the edition gives the reel a new attempt. Tests 639 (kt-media, the cycle and the strictness of the key) and 640 (the second opening of the project — zero renders; a changed mtime — a re-render).

## Sources

- https://support.apple.com/guide/final-cut-pro/intro-to-compound-clips-verbbd3496b/mac (accessed 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/edit-compound-clips-verca9e8672/mac (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/nested%20timelines.htm (accessed 2026-09-07)
- https://help.magix-hub.com/video/vegas/en/content/topics/nested%20timelines.htm (accessed 2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/unscaled-nesting-lowers-resolution-quality/m-p/14219625 (accessed 2026-09-07)
- https://creativecow.net/forums/thread/editing-with-nested-sequences-good-or-bad-ideaae/ (accessed 2026-09-07)
- https://creativecow.net/forums/thread/collapse-transformations-continuous-rasterise-rem/ (accessed 2026-09-07)
- https://davinciresolveclub.com/davinci-resolve-compound-clips/ (accessed 2026-09-07) — a third-party source, used out of necessity
- Project code and documentation: `src/libs/kt-model/src/project.rs`,
  `src/libs/kt-engine/src/{video.rs,video_plan.rs,worker.rs,fxstore.rs}`,
  `src/libs/kt-otio/src/{schema.rs,convert.rs}`,
  `src/libs/kt-media/src/lib.rs`, `docs/porting-notes.md` items 153, 158,
  `docs/file-format.md`, `docs/user-feedback.md` (R.24 item 5, R.25 item 4).

What could NOT be verified, and why. The Resolve line relies on forum headings and a third-party analysis: `forum.blackmagicdesign.com` returns 403 from this environment for any `viewtopic.php`, mirrors of the manual (manualzz) give "proxy refused", and Resolve has no official HTML manual. The claims about Fusion Clip and about the flag "refer to the source resolution" are marked `[NOT VERIFIED]` directly in the table. Adobe's help (`helpx.adobe.com`) and reddit are unavailable — the Premiere and AE lines are assembled from the official Adobe community and Creative COW; the text of the Pre-compose dialog ("leaves the properties and keyframes … in the original composition") is visible only in search results with a link to helpx, so in the table it is paraphrased, not quoted. The old VEGAS forum (`vegascreativesoftware.info`) redirects after the change of publisher to `forum.borisfx.com`, and the direct links to threads about nested projects are dead — so there are no live quotes from there in this note.

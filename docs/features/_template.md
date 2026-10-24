---
id: F-XXX-000
title: Short feature name
status: researched
priority: B
cost: M
category: category/subcategory
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-engine]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: []
services: []
checked: 2026-09-08
---

# F-XXX-000 — Short feature name

> This is a template. Copy it whole, fill in all sections; there must be no empty
> sections. The values of the header fields are dictionaries from `README.md`; there is no free text in them.

## Essence

Three to five lines: what this does, THROUGH THE EYES OF THE EDITOR. Without implementation, without "in the industry it is accepted". If the essence does not fit into five lines — these are two features, and two notes are needed.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 22 | Adjustment Event | an event on a track, acts on the composite of those below |
| After Effects | 2026 | Adjustment Layer | a layer without content, effects apply to the result below |

A line for EACH of the five core editors. If there is no feature — write exactly that: "no".

If the mechanism originates outside video editing — in a SECOND table, separately, so that the reader can see which lines carry the verdict and which explain the device. When it is mandatory and when superfluous — `_editors.md`, section "Software outside the core".

**Outside the core.** One line: why the feature cannot be described without this program.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Ableton Live | 12.4.5 | Info View | a fixed area, describes what is under the cursor |

## The trick behind it

**A mandatory section; the base exists for its sake.** One to three CONCRETE details that are absent from the list of capabilities on the product's website and that are visible only when you have worked with it.

Not "there are masks", but "the mask lives on the EFFECT, not on the clip, so the same blur is painted in two places of the frame with different strength". Not "there are curves", but "the key is stored in ticks of the MATERIAL, so trimming does not shift it, but hides the keys".

If there is nothing to write here — **do not open a note**: put a line in the coverage table of the README "checked, we have nothing of our own". The list of features is on Wikipedia; this base is not needed for it.

## Pros

- Each item is a consequence of the DEVICE, not an evaluation. "Fast" is not an item; "editing one layer rewrites 40 clips without 40 undo steps" is an item.

## Cons

- The same: what is paid for this device. "The layer does not know what is below it, therefore its automation is computed in TIMELINE time, not material time, and shifting the clip desynchronizes the effect".

## What users say

> A verbatim quote, no more than two sentences.
> — source, date, https://…

At least one quote with a link; "people complain" is not feedback. Most valuable is "switched to the new version BECAUSE OF this" and "turned it on once and turned it off forever, because…". Negative feedback is no less valuable than praise.

## How it fits Kinotrain

### What already exists

With paths and names: `Project::effective_clips()` in `src/libs/kt-model/src/project.rs`, item 241. At least one path of the form `src/…` is mandatory — a section written blindly is useless.

### What is missing

### What hinders

A concrete constructive point, not "a lot of code is needed".

### In the category `collab/` — two more mandatory lines (rule 13)

They are checked by the script. The anchor in backticks must point to a file that exists in the tree TODAY, and must not be repeated for two notes of the category:

```
OFFLINE: REMAINS — `src/libs/kt-media/src/index.rs` — we already compute the identity of the material: xxhash plus size and mtime in the media table, so "the same file moved to another folder" is recognized without any server.
OFFLINE: DEGENERATES — `src/apps/kt-app/src/app.rs:5036` — the multi-user lock degrades into a guard of a single writer: write_project_guarded compares the revision and does not overwrite a newer snapshot, but it knows exactly one person.
OFFLINE: DOES NOT REMAIN — `src/libs/kt-model/src/project.rs:1459` — a Marker has a time, duration, name and color, and there is NO author and NO state; today the editor transfers the client's remark from the letter into the marker name by hand.

NETWORK COST: merging two edits of one project — the edit log — our undo is snapshots of the whole project, there is nothing to merge: an edit has neither a name nor a base revision.
NETWORK COST: rendering on someone else's machine — invariant 9 — export is entirely in-process: no external process, no job queue, no agent.
```

The `DOES NOT REMAIN` branch is not an excuse: the anchor in it points to the PLACE where the absence is visible, and the tail answers in the present tense — what the person uses today to close this gap. A future-tense verb in these two lines is a sign that the note has drifted into a technical specification (rule 8).

### Which invariants it touches

Numbers from the root `CLAUDE.md` (1–12) and rules of the checklist from `src/apps/kt-app/CLAUDE.md` (1–20). None — write exactly that: this is a strong argument for "cheap".

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2 stages of a round |
| Where the main work is | kt-model (schema), kt-gpu (shader) |
| Project format | needs version v5 / does not touch |
| Tests that will have to be written | round-trip of the model; conformance CPU↔GPU; harness test of the window |
| Depends on | F-AUT-001 |
| Breaks compatibility | no |

A frame, not a specification: the note does not design the implementation — the design begins in the round.

## Verdict

One to three lines: take / do not take and WHY. For the status `rejected` the reason is mandatory and is checked by the script: it outlives the author, "not needed" is not a reason.

**In the category `help/` the verdict must carry a line of one of five forms** (checked by the script), and the tail must name an address — a test name, a path or a clause of `--check`:

```
TEST: reach_tests::function_name — today it fails, because …
CHECK: src/apps/kt-app/build.rs — the clause "…"
REJECTED: invariant 12 / rule 10 — …
ALREADY-HAVE: src/apps/kt-app/src/app.rs — reconnaissance clarified the wording as: …
DEFERRED: until F-XXX-NNN — …
```

Notes about hints have one more mandatory line. A pop-up must not be proposed; the SECOND SURFACE is proposed, which does not overlap and does not delay:

```
ONE LINE: overlap — does not touch | delay — does not touch | capacity — <what, and ON WHICH surface>
```

## Sources

- https://… (access date 2026-09-07)

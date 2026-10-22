---
id: F-COL-030
title: Conflict resolution dialog, empty fourth year
status: researched
priority: B
cost: S
category: collab/merge
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app, kt-model]
invariants: []
ui_rules: [16, 20]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [team-projects, bm-cloud, fcp-shared-library]
checked: 2026-09-08
---

# F-COL-030 — Conflict resolution dialog, empty fourth year

## Essence

The editor presses "publish my edits," and instead of publishing a modal window opens titled "Resolve Conflicts." Inside the window there is nothing: no list of divergences, no choice of version, no explanation. The only live button is "Stop Resolving," that is, cancel. Publication does not go through until the window is closed, and closing it means the edits were not published. This is how it has stood since 2020.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | the concept of "conflict" does not exist at all: divergence is neither prevented nor detected; the second save overwrites the first `[NOT VERIFIED live: the mechanism is absent both from the help and from the feature list]` |
| Adobe Premiere | 26.x | `Resolve Conflicts` (Team Projects) | a modal window in the path of publishing edits. The list of divergences is computed by the SERVER; the client only draws the rows; if no rows arrive, the window comes up empty, and there is nothing in it with which to tell "there are no conflicts" from "the list was not received" |
| DaVinci Resolve | 21 | there is no window | the conflict is prevented at entry: bins and timelines are occupied automatically, "read only" until the lock is released. Instead of resolution, a COMPARISON of two versions of the timeline by the human eye is offered (F-COL-031) |
| Final Cut Pro | 12.3 | there is no window | instead of a window — a refusal BEFORE opening: "The document [Library name] is already in use by [user] on [Mac].local". The impossibility is expressed as an error at the moment of the attempt, not as a surface after it |
| After Effects | 26.x | the same window | has no model of its own: both the mechanism and the window come from the service shared with Premiere `[NOT VERIFIED live]` |

## The trick behind it

**The window outlived a mechanism that never existed, and it survives not out of habit but because of the separation of duties between client and server.** The list of divergences in Team Projects is computed on the other side; the client is a drawer of rows that arrived over the network. From this follows a property that makes the defect unkillable: an EMPTY window has two different meanings — "there are no conflicts" and "the list did not arrive" — and the client cannot distinguish them in principle, because in both cases it holds the same thing, an empty array. Only the one who counted can say "there are no conflicts"; the client does not know this and therefore honestly draws emptiness.

**Second: the window stands on the path of writing, not beside it.** It is not an information panel that can be closed while one goes on working — it is a modal step of publication. The only working button inside ("Stop Resolving") cancels the operation, that is, an empty surface has exactly one outcome, and it is the loss of the result of the attempt. An empty window is therefore not "cosmetics": it is a dead end dressed up as a dialog.

**Third, which is why this note was opened: we have a machine check for exactly this class, and it looks in the OTHER DIRECTION.** t252 (`no_orphan_commands`) catches an orphaned COMMAND — a mechanism that the interface did not reach. Adobe's case is the mirror image: an orphaned SURFACE — a window with no mechanism behind it. Our "no dead ends" mechanism (R.38) was established on the claim "half of it is dead ends," but it checks one direction out of two, and the second is checked today by no one.

## Pros

- The very existence of the surface admits that divergence happens: the other four pretend there is no question.
- Modality on the path of publishing is the right form for an irreversible step: first sort it out, then write.
- The separation "the server counts — the client draws" allows changing the rule for counting conflicts without touching the application.

## Cons

- An empty state is ambiguous by construction, and there is nothing on the client with which to cure this — an answer "there are no conflicts" is needed as a separate message, not as the absence of rows.
- A dead end: the only exit from the window cancels the operation for which it was opened.
- A surface creates an appearance of possibility. For four years the user has been searching for how to "resolve conflicts," because the window promises that this is done here.
- The defect lives for years precisely because it is not a crash or an error: nothing crashes, the log is clean, and reproduction requires a second participant and a subscription.

## What users say

The defect was filed in 2020 and confirmed in 2024 — by the same symptom, in the same thread, which has never been marked as solved:

> When trying to share a team-project, I get a window that says 'resolve
> conflicts' but the window itself is empty.
> — devGuru411, "Premiere Pro Team Project - 'Resolve Conflicts' Empty Window,"
> Adobe Product Community, 26.08.2020,
> https://community.adobe.com/t5/team-projects-discussions/premiere-pro-team-project-resolve-conflicts-empty-window/td-p/11387956

The author specifies that the only thing one can press in the window is "Stop Resolving" — that is, cancel — and that until a fix arrives they cannot publish their edits. On 16.12.2024 lauren_2058 replied in the same thread with a question whether an answer had been found, and reported that she has the same problem; an Adobe employee (Udo_Pawlik) asked for the version and requested an attempt to reproduce the conflict; there is no solution in the thread.

`trackers:` is empty deliberately (rule 12): none of the five core editors has a public tracker, and the theme of shared work is absent from the corpus of editing programs as a class — the count on this subject belongs to F-COL-029, and repeating it here would mean counting one finding twice.

## How it fits Kinotrain

### What already exists

- `src/libs/kt-model/src/model.rs:336` — `CommandError::NothingToUndo`: the most degenerate refusal we have ("there is nothing to undo"), and it is NAMED. The model does not return emptiness: each refusal has its own variant with text —
  `ClipOverlap`, `MediaInUse`, `MarkerExists(Tick)`, `AreaUnnamed`
  (`src/libs/kt-model/src/project.rs:3204` and nearby).
- `src/apps/kt-app/CLAUDE.md` item 16 — "the impossibility of an action is shown as an ERROR, not as a faded hint": the obstacle is highlighted, the style is an error style, and it is said where to look if the cause lies outside the view. Closed by t172 ("obstacle as error").
- `src/apps/kt-app/src/reach_tests.rs:211` — t252 `no_orphan_commands`: an implemented command that no interface row of the application creates breaks the build. Introduced over a concrete loss — the named areas of R.35 existed in the model and were unreachable from the interface.
- `src/apps/kt-app/src/reach_tests.rs:459` — t253: for each timeline entity the full set of verbs is checked, and the table is exhaustive over `TlEntity` — a new entity will not compile until its row is filled in.

### What is missing

- A check of the reverse direction. t252 runs from the command to the interface; no test catches a surface for which there is no mechanism. A window with a title and without content would today build silently for us.
- A vocabulary of the empty state. An empty panel for us means "there is nothing here," and this is sufficient while the data source is local and synchronous: "no rows" and "rows have not arrived" are one case. As soon as the source becomes separate (the index DB is built in the background, thumbnails arrive later), the difference will appear, and there is no language for it.

### What gets in the way

Nothing constructive — only the direction of the already written check gets in the way. It is worth remembering where the price of the question came from: item 20 of the checklist ("nothing grows or is cut off by itself") forbids surfaces from appearing on their own, but prose breaks nothing — that is precisely why R.38 introduced `reach_tests`. The lesson of Adobe is that the reachability mechanism must be two-sided: a command without a surface is a lost feature, a surface without a mechanism is a dead end, and the second is worse, because it looks like a working program.

### In the category `collab/` — two mandatory rows (rule 13)

OFFLINE: REMAINS — `src/libs/kt-model/src/model.rs:336` — the antidote is wholly local and is already built: impossibility is expressed for us by a typed refusal with its own text, down to the most degenerate case "there is nothing to undo," rather than by a surface that the user has to interpret; the interface is obliged to show it as an error under item 16 of the checklist, and this rule is checked by t172 rather than resting on the good faith of the author of a window.

NETWORK COST: the content of the conflict window — project-as-database — the list of divergences can be compiled only by the side that stores both versions and knows how they differ; the client receives a ready array of rows, so an empty window is indistinguishable from "there are no conflicts." Ours, meanwhile, has both versions at once nowhere, and there is nothing from which to compile such a list today.

### Which invariants it touches

Of the twelve — none. Of the rules of the `src/apps/kt-app/CLAUDE.md` checklist — two, and both on substance: item 16 (impossibility must be an ERROR: an empty window is the extreme case of a "faded hint," it does not even speak with colour) and item 20 ("nothing grows or is cut off by itself" — a surface laid down under a mechanism that does not exist has grown by itself). The note does not propose a new window and does not touch the prohibition of pop-up surfaces.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage: the reverse direction of the reachability check and a vocabulary of the empty state |
| Where the main work is | kt-app (`reach_tests.rs` — the surface scanner; the texts of the empty states) |
| Project format | does not touch it |
| Tests to write | a window/panel without a mechanism breaks the build just as an orphaned command does (today it does not fail); an empty state caused by data that has not loaded does not look like "there is nothing here" |
| Depends on | nothing: both halves are local |
| Breaks compatibility | no |

## Verdict

We do not take the feature — there is nothing to take: the mechanism does not exist, not even for the author of the window. We take the lesson, and it is cheap. The direct conclusion for us: the "no dead ends" mechanism (R.38) must work in both directions. Today t252 catches a command that the interface did not reach, and does not catch an interface behind which there is no command — and precisely this case has lived at Adobe for four years and looks like a working program. The second conclusion: an empty state must have two different texts as soon as the data source stops being synchronous and local.

This is also the most vivid illustration of item 16 of the checklist: Final Cut in the same place says who exactly holds the library and on which machine, while Premiere shows an empty rectangle. The difference between them is not in the complexity of the mechanism but in the fact that the first answers an attempt with a refusal and a reason, while the second answers with a surface.

Boundaries: why there is no merging anywhere and where the window itself came from — F-COL-029; what can be shown instead of resolution (a comparison of two edits) — F-COL-031; a named failure instead of a silent one — F-COL-032; how an editor generally reports the impossibility of an action — F-HLP-010; empty states that teach — F-HLP-020.

## Sources

- https://community.adobe.com/t5/team-projects-discussions/premiere-pro-team-project-resolve-conflicts-empty-window/td-p/11387956
  — the thread of 26.08.2020, confirmed 16.12.2024, not marked solved (accessed 2026-09-08).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  locking of bins and timelines instead of resolving conflicts (accessed 2026-09-08).
- https://support.apple.com/en-us/HT207128 — the Final Cut message "already in use" as a form of refusal BEFORE opening (accessed 2026-09-08).
- The VEGAS and After Effects rows are marked `[NOT VERIFIED]` directly in the table.
- Our code read on 08.09.2026: `src/libs/kt-model/src/model.rs` (336),
  `src/libs/kt-model/src/project.rs` (3204),
  `src/apps/kt-app/src/reach_tests.rs` (211, 459), `src/apps/kt-app/CLAUDE.md`
  (items 16, 20 of the checklist).

---
id: F-TL-010
title: Transition between clips on one track
status: already-have
priority: B
cost: M
category: timeline/structure
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: yes
crates: [kt-model, kt-engine, kt-otio, kt-app]
invariants: [7, 11]
ui_rules: [16, 19, 20]
round: R.28
point: 162
tests: [135, 136, 137, 155, 159, 162, 163, 164, 165, 173, 176, 223, 225, 226, 227, 462, 463, 464, 465, 466]
depends_on: []
trackers: [kde-invent, kde-bugzilla, pitivi]
checked: 2026-09-16
---

# F-TL-010 — Transition between clips on one track

## Essence

Two neighboring events on ONE track pass into each other: the picture dissolves, the sound flows over. It does not matter to the editor how this is built inside; what matters is that for the sake of a second of dissolve one does not have to create a second track, drag a second clip there and align them by hand. This is exactly the subject of the live request kde-invent #796 "Same track transitions" — an editor whose track was a strict sequence added an overlap after the fact and for three years fixed the consequences.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Automatic Crossfades (Options); Fade Type / Transition via the right-click on the overlap zone | you place an event over the neighbor ON THE SAME track — the crossfade appears by itself, there is no separate command. A right-click on the overlap zone gives the fade curve (fast/slow/linear/smooth/sharp) and the replacement of the crossfade with any other kind of transition |
| Adobe Premiere | 26.x | Cross Dissolve at the cut point; Clip handles settings | the clips stay BUTT-JOINED, the transition eats material beyond the cut ("handles"). There is no material — the dialog "Insufficient media. This transition will contain repeated frames", and the transition is completed with repeated frames. An overlap on one track is impossible: the upper clip overwrites the lower one, and the request "make the crossfade an overlap" has been hanging open in Adobe Ideas since 2023 |
| DaVinci Resolve | 21 | Cross Dissolve at the cut point | the same device and the same failure: not enough handles — "not enough media", cured by trimming both clips at the junction by 15–30 frames so that the "hidden" frames appear. Dragging a clip onto its neighbor is an overwrite, not a transition |
| Final Cut Pro | 12.3 | a transition at the cut point of the storyline (Command+T) | the magnetic line is sequential BY DEFINITION: two clips cannot be overlapped in it, the second goes off as a connected clip one level up. The transition needs "extra media beyond clip edges"; there is none — the dialog suggests shortening the clips and the whole project |
| After Effects | 26.x | none: a transition as an object does not exist | a "track" is a layer, layers overlap by definition and always. A crossfade is made with Opacity keyframes on two layers. Therefore the question "a transition on one track" is not asked here — and there is nobody to ask "what kind of transition is this": there is no object |

**Outside the core.** This note could not be written without Kdenlive: it is the only publicly traceable case in which an editor with a sequential track added an overlap after the fact, and the tracker shows what it paid.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 | Mix Clips (since 20.12, "same track transition") | each track has TWO playlists INSIDE; a "mix" puts one of a pair of clips into the second playlist of the same track, and the overlap results where, in the MLT model, the track is still strictly sequential. Outwardly this appears as a separate subsystem with its own commands, its own dialog and its own crashes |

## The trick behind it

**1. A transition is ALWAYS an overlap; editors differ only in where they hide it.** The Apple help wording is direct: a transition needs overlapping video on both sides of the cut. There are exactly three hiding places, and each has its own failure. Beyond the cut, in trimmed material (Premiere, Resolve, FCP) — the failure "the handles ran out": the "insufficient media" dialog, repeated frames, or a suggestion to shorten the project. On a neighboring track (the MLT/GES lineage: Kdenlive before 20.12, Shotcut, Pitivi) — the failure "two tracks for the sake of a second", and it also breaks the order of clips: pitivi #573 from 2009 describes how the ban on overlap on a layer did not even allow swapping clips. On the track itself, as a visible overlap (VEGAS and us) — the failure "placed it and did not notice". There is no choice "without a price" in this list; there is a choice of which price to show the user.

**2. Kdenlive did not cancel the second track — it hid it inside the track, and we did exactly the same thing, only in the file.** The assert from the crash of kde-invent #1499 names the mechanism verbatim:
`m_playlists[sourcePlaylist].is_blank_at(position) &&`
`m_playlists[destPlaylist].is_blank_at(position)` in `trackmodel.cpp` — a track has two playlists, and the "mix" relocates a clip from one to the other. We have the same geometry living on the format boundary: in OTIO a clip has NO "start on the timeline" field, the position is set by the order of the track's children, so two simultaneously sounding clips in one Track cannot be expressed physically (the direct question of the user, R.28: "CAN'T WE JUST IGNORE THE STANDARD AND WRITE OVERLAPS?"). Our overlaps are written as adjacent Tracks plus the map `KtLayers { video: [n], audio: [n] }` in `metadata["kt"]`, and reading assembles them back. That is, we did not cancel the two-track workaround — we moved it to where the user does not see it, and taught the reader to parse it (t165). The difference from Kdenlive is not in the presence of a seam but in its place: they have the seam in the editing model and it crashes on undo of a mix resize; we have it in serialization and it is checked by a round-trip.

**3. A crossfade is not equal to two semi-transparent layers, and this is visible FROM BELOW.** The same defect was independently filed in two unrelated codebases: pitivi #2583 "Lower layers visible beneath crossfaded top layers" and kde-bugzilla #456830 "Mix Clips compositions transition applies to lower layers". The mechanism: the pair is put on the composite as two layers with alpha 0.5, and the lower track shows through at 0.5·0.5 = 25 % exactly in the middle of the transition. It is cured not by picking the alpha, but by the pair having to collapse into ONE layer BEFORE the composite. We do exactly this: `PlanLayer::Dissolve { a, b, k, alpha_k }` resolves the pair into one result `a·(1−k) + b·k` and only then multiplies it by the alpha — there is in principle nothing to show through from below. Sound answers the same question by another law: not 0.5/0.5, but `√(1−t)`/`√t`, because power must be preserved, not amplitude.

## Pros

- An overlap is a state, not a command. The move gesture under `overlap_layers` returns `MovePlan::Free` on any overlap: the batch `[MoveClip, TrimEnd, TrimStart, AddTransition]` is not assembled at all, and "dragging it back" returns exactly what was there, because nothing was cut (t163). The defect class "material lost under the crossfade" (R.26) is closed for us not by restoring material, but by not touching it.
- A foreign or old edit is imported one-to-one: 1952 clips, 5 video and 3 audio tracks, 76 overlaps — and not a single new track (item 164, t164). The former converter unfolded the same file into ~40 video tracks, that is, it itself produced the pain for whose sake F-TL-010 is opened.
- A transition is an independent object, not a property of a pair: 16 kinds (`TransitionKind`, t223), curves of the kind's PARAMETERS on top of them (`TransitionEvent.auto`, t225/t232), the audio crossfade curve separately (`AudioCurve`, t226). A fade at the edge of a track is the same object with one live end (`is_fade`, t155), so "fade" and "transition" do not diverge into two different pieces of code.
- The key of a kind's curve is stored in ticks from the START of the transition window, not in fractions of the window: an edit of the window's edge opens and hides the keys, like a clip trim, rather than stretching the animation.
- There is not a single "not enough material" dialog. The transition window takes the handles via `head_ext`/`tail_ext`, and when there are none — the missing half gets full weight, the frame is not repeated, the sound does not click and no underrun is raised (t137, t136).

## Cons

- **Overlap and transition are two mechanisms, and they never meet.** `check_transition_full` requires `a.start + a.duration == b.start`, otherwise `TransitionNotAtJoint`. So the 16 kinds, the curves and the adjustable window are available only on clips BUTT-JOINED, and on a real overlap only the audio crossfade. A user who overlapped clips (that is, did "as in Vegas") cannot choose a transition kind.
- **Video does not blend at all on overlap.** The Vegas rule puts the later clip on top as opaque (`sort_by_key((Reverse(lane), start))`, t162), so an overlap of the picture is a hard cut. The same gesture on the two halves of an A/V pair gives different results: the sound flows over, the picture clicks.
- **1/N with three or more is a break, not a continuation.** Two give the √ law (0.707 each at the middle), three give exactly 1/3 to each. In the frame where the third clip appears, the weight of the first two drops abruptly from ~0.7 to 0.33. This is a deliberate fidelity to the semantics of v1 (14 such zones in the user's project), but the price is there, and it is not smoothed.
- **An overlap is not marked on the strip in any way.** The painter draws the later clip over the earlier one as an ordinary rectangle; there is no hatching and no X-shaped crossfade curves, as in VEGAS. An accidental overlap is visually indistinguishable from a clip that simply starts there — while the tail of the neighbor under it SOUNDS.
- **There is no switch for `overlap_layers`.** The option is on by default, is read by the model, the engine and the converter, but across the whole `apps/` there is neither a command, nor a menu item, nor a badge — the field can become `false` only from a file that our own program cannot write.

## What users say

> Currently, there is a mechanism in place to prevent the user from
> overlapping clips onto a single layer, because the auto
> crossfading/transitions system is not implemented.
> — pitivi #573, 26.07.2009,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/573

The same report also names the side damage of the ban: clips cannot be swapped, because to do so they would have to be driven through one another.

> Without this functionality, I can't see how one can feasibly create a
> timeline with multiple clips with transitions between them.
> — pitivi #1318, 05.06.2013,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1318

> Checkerboard image is visible beneath the top layer during the
> crossfade. This occurs both in the preview and in the rendered video.
> — pitivi #2583, 16.10.2021,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2583

> For simplicity, the feature is currently called a Mix, can be changed
> before release.
> — kde-invent #796, 18.09.2020,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/796

The name was never changed, and eight months later an objection came in on exactly this:

> To avoid the word "Transition" in a video editing software is not a good
> idea.
> — kde-invent #1070, 07.05.2021,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1070

> The current way to add and manipulate transitions on the timeline is a
> sort of "guess & check" method, you've got to have the right amount of
> handle frames before you add it.
> — Adobe Ideas, «Add a crossfade by overlapping audio or video clips in
> the timeline», 24.01.2023, status Open for Voting,
> https://community.adobe.com/t5/premiere-pro-ideas/add-a-crossfade-by-overlapping-audio-or-video-clips-in-the-timeline/idi-p/13515690

SIGNAL: a transition on one track as a SEPARATE subsystem — 17 tasks in kde-bugzilla, `same.?track transition|single.?track transition|\bmix(ed|es|ing)? clips?\b|\bmix (zone|transition|duration|clip)|\bmix\b.{0,20}\b(track|timeline|transition)`; share 0.3% against 0.0% (none of 2672) in pitivi

SIGNAL: the lower layer shows through the crossfade — 2 tasks in pitivi, `(lower|underlying|background|other) (track|layer)s?.{0,30}(visible|bleed|show|appear)|bleed.{0,20}through|applies to lower`; share 0.1% against 0.0% (1 task of 6346) in kde-bugzilla

SIGNAL: the right to overlap clips as a separate request — 3 tasks in pitivi, `(allow|enable|let|don.?t block|permit|easy|easier).{0,25}overlap|overlapping clips?|overlap.{0,20}(clips?|events?).{0,20}(same|one|a) (track|layer)`; share 0.1% against 0.0% (1 task of 2217) in kde-invent

How to read the first number. The 17 Kdenlive tasks are NOT "users asking for a transition": the feature itself was closed in 20.12, and everything after is the count of its consequences. By year: 2016 — 1 (the old internal mix/composite, before the feature), 2021 — 8 (reversal of direction, sound against video, crashes on moving), 2022 — 4 (damage to the timeline, "not enough frames", the bleed-through of lower layers), 2023 — 1, 2024 — 3. Kdenlive paid for the overlap with three years of defects IN THE MECHANISM ITSELF; zero at Pitivi is not a sign of well-being, but a sign that there is no separate mechanism there and nothing to break (Pitivi pays its price with a different account — the second SIGNAL).

A lexicon trap found here, useful for the next note: the word `transition` gives in kde-bugzilla 1388 of 6346 = 21.9 %, but 1322 of them come from the `component` fields "Video Effects & Transitions" (1245) and "Audio Effects & Transitions" (77), not from the text of the task. `match()` in `tracker-rank.py` searches over "title + labels + component", so any filter made of one common word must be checked for a match against the component name, otherwise the note will report a finding the size of an entire section of the tracker.

## How it fits Kinotrain
CAPABILITY: openspec/specs/transitions/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Already there: a transition on the overlap of one track — the window = the overlap zone, without its own handles; without a transition the gesture changes only the position.
ADDRESS: src/apps/kt-app/src/timeline.rs — the class "placed it and did not notice" is POSSIBLE with us, and nothing catches it: the painter draws the later clip over the earlier one as an ordinary rectangle without marking the zone, `overlap_layers` has neither a command nor a menu item nor a badge (checked by grep: outside tests the field is mentioned across the whole `apps/` exactly twice — in `plan_move` and `plan_move_block`; there is no `Command` variant that changes it at all, so `false` can arrive only from a file that our own program cannot write), and `check_transition_full` in `src/libs/kt-model/src/project.rs` refuses a transition on an overlap (`TransitionNotAtJoint`) — that is, the 16 kinds are unavailable exactly where the crossfade already sounds.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: showing the overlap zone + a switch and a badge (S); a transition on top of an overlap (M, as a separate decision) |
| Where the main work is | kt-app (the strip, the context menu, the badge); kt-model — only if "butt-joined only" is removed |
| Project format | does not touch it: `KtLayers` and `TransitionEvent` are already in v3; a transition on an overlap will require an owner of the window and, probably, a field |
| Tests that will have to be written | harness: the overlap zone is marked and distinguishable from the neighboring clip; the switch changes the project and the badge names the place of return (rule 19); on removing "butt-joined only" — a new round-trip t165 and CPU↔GPU conformance on a pair whose window is set by bodies |
| Depends on | — |
| Breaks compatibility | no (items 1–2); a transition on an overlap — yes, it will require reading old files "as they are" |

## Verdict

We take it in the part of display and the switch, and do not take it in the part "a transition on top of an overlap" without a separate decision: there the price is not in the code but in the fact that the transition window acquires a second owner, and exactly at this point Kdenlive got three years of defects.

**Ed. R.51 (2026-09-16): a separate decision was ADOPTED by the owner** — a transition must work on an overlap too ("symbiosis of two events", a direct answer to AskUserQuestion). The question "who owns the window" is resolved so: the window of an overlapped transition is the overlap zone ITSELF `[b.start, a.end)`, the mode is not stored and is derived from the geometry of the pair, `in/out_offset` are not used in this mode; a transition is legal only on a clean pair in a "staircase" (a third clip in the window is a validation error, 1/N zones remain without transitions). The Kdenlive lesson is taken into account by the choice "the window has no handles of its own": it follows the trim of the clips, and no twin subsystem with its undo crashes appears. Implementation and tests — round R.51, see `openspec/specs/transitions/`.

ALREADY-HAVE: src/libs/kt-model/src/project.rs (`overlap_layers`, `TransitionEvent`) — the reconnaissance refined the wording: **we do not "support transitions on one track", we allowed OVERLAPS, and our transitions are a separate object at the JOINT, and these two things have not met so far.** The promises are different and their prices are different. "We support transitions on one track" is the promise of Kdenlive, and it cost it 17 tasks in bugzilla and 6 in invent over four years. Our promise is "an overlap is legal and cuts nothing", and its price is different: the overlap is invisible on the strip, sounds where the picture clicks, and cannot do any of the 16 kinds. A second refinement of the same order: we did not cancel the two-track workaround, we moved it into the file (`KtLayers`), because in OTIO two simultaneous clips in one Track are physically inexpressible — so "one track" is true about the screen and false about the disk.

## Sources

Trackers (dumps in `~/.cache/kinotrain/trackers/`, outside the repository; the numbers are reproduced by `python3 src/scripts/tracker-rank.py --query …`):

- kde-invent #796 "Same track transitions", 18.09.2020, closed 09.08.2024,
  33 comments: https://invent.kde.org/multimedia/kdenlive/-/work_items/796
- kde-invent #874, #875, #1070, #1499, #1504 — the track effect after a mix, the missing QtQuick.Shapes, the dispute over the name, an assert on undo of a resize,
  "mix clip stopped working"
- kde-bugzilla #365887, #436846, #440019, #440020, #441999, #442066,
  #442088, #442317, #445626, #453536, #453537, #456830, #457783, #469416,
  #483679, #484181, #493108 — the very 17
- kde-bugzilla #486562 "odd behavior when applying a transition on two
  clips on the same track", 04.05.2024, CONFIRMED — not included in the 17 (the title has no word mix), but describes the same joint:
  https://bugs.kde.org/show_bug.cgi?id=486562
- pitivi #398, #573, #1318, #1408, #1915, #2044, #2162, #2583 — the overlap as a ban, as a gesture and as the source of the bleed-through of lower layers
- The export date in the registry `docs/features/_trackers.md` (the columns "Dump from" and sha256 at the time of writing are not yet filled — the export of the wave was running in the background).

Documentation and communities (accessed 08.09.2026):

- VEGAS Pro 2026, "Automatic crossfades":
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/autocrossfades.htm
- Adobe, "Clip handles settings":
  https://helpx.adobe.com/lv/premiere/desktop/add-video-effects/apply-video-transitions/clip-handles-settings.html
- Adobe Ideas, "Add a crossfade by overlapping audio or video clips in the
  timeline", 24.01.2023:
  https://community.adobe.com/t5/premiere-pro-ideas/add-a-crossfade-by-overlapping-audio-or-video-clips-in-the-timeline/idi-p/13515690
- Adobe Community, "What does "Insufficient media. This transition will
  contain repeated frames" mean?":
  https://community.adobe.com/questions-729/what-does-insufficient-media-this-transition-will-contain-repeated-frames-mean-1376361
- Blackmagic Forum, "Add transitions insufficient handles":
  https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=83114
- Apple Community, "Not Enough Extra Media Beyond Clip Edges":
  https://discussions.apple.com/thread/8469386

Ours:

- `src/libs/kt-model/src/project.rs` (`overlap_layers`, `TransitionEvent`,
  `check_transition_full`, `TransitionKind`), `ops.rs` (`upgrade_legacy`),
  `src/libs/kt-engine/src/audio.rs` (`overlap_weight`, `overlap_at`),
  `video.rs` (the Vegas order, `TransAnim`), `video_plan.rs`
  (`PlanLayer::Dissolve`), `transitions.rs` (`TransMix::of`),
  `src/libs/kt-otio/src/convert.rs` and `meta.rs` (`KtLayers`),
  `src/apps/kt-app/src/timeline.rs` (`plan_move`, `plan_move_block`,
  `draw_order`, `corner_fade_len`), `src/scripts/live/overlap.py`.
- items 161–164, 177; `docs/user-feedback.md` R.24
  item 6, R.26 in full, R.28 items 5/6/9 and decisions 1–3.

Note on method: no foreign editor was run live. `helpx.adobe.com`, `discussions.apple.com` and `forum.blackmagicdesign.com`
were opened in this pass only through search snippets; only the Adobe Ideas page was read verbatim; the Resolve and FCP lines rely on help pages and communities, not on verification. All quotes from the trackers were read verbatim from the local dumps.

---
id: F-TL-011
title: Expanding a nested item IN PLACE
status: done
priority: A
cost: XL
category: timeline/structure
editors: [premiere-26, fcp-12, resolve-21, vegas-pro-2026, after-effects-26]
we_have: yes
crates: [kt-model, kt-app]
invariants: [3, 7]
ui_rules: [13, 14, 16, 17, 20]
round: R.55, R.56, R.58
point: 441-445, 449-451, 460-462
tests: [t566, t567, t568, t569, t570, t571, t572, t573, t591, t592, t596, t597, t634, t635, t636, t637, t637b, t637c, t638]
depends_on: [F-TL-004]
checked: 2026-09-20
---

# F-TL-011 — Expanding a nested item IN PLACE

## Essence

A composition is expanded RIGHT IN THE SAME timeline: its tracks slide open beneath the instance's row, rather than replacing the view. The editor adjusts what is inside and immediately hears and sees the result IN THE CONTEXT of the whole programme — with its neighbours, transitions and the chain of the nesting itself. The full entry (a view of the composition alone) remains a second mode, for working "entirely inside".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Avid Media Composer | 2026 | Expanded nesting (Alt/Option + Step In) | the only close analogue: the nesting is unfolded in layers RIGHT ON the timeline's tracks, neighbouring tracks shift. The ordinary Step In gives a "simple" nesting — a view of the content only |
| Adobe Premiere | 26.x | Open in Timeline (double-click on the nest) | there is no expansion in place: the sequence opens in a SEPARATE timeline tab. The programme's context is not visible at that point; switching between tabs is manual |
| Final Cut Pro | 12.3 | Open Clip (compound clip) | the view is REPLACED by the contents of the compound clip, with a breadcrumb path at the top; outside the trim the material is shown dimmed |
| DaVinci Resolve | 21 | Open in Timeline / Decompose in Place | two different gestures: open the nest in a separate timeline OR IRREVERSIBLY decompose it onto tracks (this is no longer an expansion but an unfolding — for us `expand_instance`) |
| VEGAS Pro | 2026 | Nested timeline | the nest is a reference to a `.veg` on disk; it opens in a separate tab. There is no expansion in place |
| After Effects | 2026 | Open composition | a composition always opens in its own timeline panel; "in place" cannot be expressed — AE has no single timeline into which one could slide it open |

**Outside the core.** Without a DAW the feature cannot be described: in video editing "expansion in place"
is a rarity, while in audio it is the norm, and there its cost and habits are visible.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| REAPER | 7.x | Folder tracks | a parent track folds its children; unfolding shows them RIGHT IN THE SAME window, and the parent continues to sum. Unfolding is a state of the view, not an edit of the project |
| Ableton Live | 12.4.5 | Group Track | the same: the group unfolds in place, the children are edited in the shared context, playback is not interrupted |

## The trick behind it

**1. Expansion is a state of the VIEW, not of the edit, and that is precisely why it can be
done "non-rigidly".** In Resolve two neighbouring items sit side by side — "Open in
Timeline" and "Decompose in Place" — and the second IRREVERSIBLY breaks the nest
down onto tracks. The difference between them is the whole feature: expansion must
collapse back without a trace in the project and without a history step. As soon as it
starts changing something in the document, it stops being an expansion and
becomes an unfolding, which we already have (`expand_instance`).

**2. In a DAW, unfolding a group does NOT interrupt playback — and this is not a
convenience but a consequence of the construction.** The children were always in the same mixer
graph; collapsing merely hid the rows. In video editing a nest is most often
a separate document, so "entering" = "switching the document", and switching the document
breaks playback. Whoever wants expansion in place must first have one address space and one
graph — and then the expansion comes almost for free.

**3. Trimming a nest and expansion quarrel, and it is settled not by drawing but by the
arithmetic of the trim.** In an expanded nest, material beyond the window of the instance remains at the sides. Showing it is not enough — one wants to ADMIT it by pulling the edge. In FCP such material is dimmed, but it cannot be pulled while expanded. For it to be possible, the trim must move `start` and `source_in` by ONE delta: then the content of the strip stands still on the screen while the window slides over it, like a blind. Move only one of the two in the trim — and the material would drift away from under the hand.

## Pros

- An edit inside is heard and seen AT ONCE in the context of the whole programme: neighbours,
  transitions and the chain of the nesting itself are in place, because the root is playing.
- Collapsing the expansion means forgetting about it, not rolling it back: no history step,
  no trace in the file.
- Snap and magnet work ACROSS the boundary: an event inside a composition can be aligned to a root-level event, which in a separate timeline is impossible
  in principle.
- The tails beyond the trim are visible and reachable, not "somewhere in there".

## Cons

- Two coordinate systems are on the screen at once, and "whose row this is" has to be shown, otherwise a person edits not what they think.
- Reference semantics become more dangerous: an edit inside the expansion changes ALL the places where the reel stands, yet it looks like an ordinary drag on the timeline.
- Track numbers stop being mere numbers: any insertion or deletion of a track has to know about the band.
- A deep nesting is unreadable: four levels of expansion are four levels of dimming and frames over two dozen rows.

## What users say

NO QUOTES: verbatim reviews about exactly "expanded nesting" in Avid and about expansion in place in video editing could not be found — network search is unavailable in this session, and a quote with a link cannot be made up (rule 4 of `README.md`). What is known without quoting: Premiere, VEGAS and AE have no such feature at all, and in Resolve the neighbouring item does an IRREVERSIBLE decomposition — and it is precisely this adjacency that is most often discussed. The section must be supplemented with quotes at the first wave with network access.

## How it fits Kinotrain
CAPABILITY: openspec/specs/compositions/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Expansion of the reel in place — a band in the parent view; expanding and collapsing are a byte-for-byte identity, and an edit from the band reaches the reel.

## Estimate

| What | Answer |
|---|---|
| Size | XL — the view of the band, the grab of the edge, the selection rules, gestures, the ×N warning, tests and a live scenario |
| Where the main work is | kt-app (`timeline.rs`, `app.rs`); the model is already done |
| Project format | NOT touched: expansion is a workplace, not work |
| Tests | t566-t569 are done; ahead — the view, the grab of the edge, the selection, gestures, ×N |
| Depends on | F-TL-004 (nested sequences) |
| Breaks compatibility | no |

## Verdict

**We take it, and take it as the mode BY DEFAULT** — a direct request of the owner
(R.55, complaint 9). The justification lies not in convenience but in the fact that for us
expansion is cheap where it is expensive for everyone: one address space and
one graph have existed since R.50, the engine already plays the root, and the trim already moves
`start` and `source_in` together. Of the five core editors the feature exists only in
Avid, which is not in the core — that is, we are not catching up, but doing what the compared ones do not have.

The full entry remains: "work entirely inside" is a legitimate scenario, and
it is precisely there that the reel itself plays from its own zero (R.55, item 434).

## Progress

**The model is done (R.55, items 441-442).** `Band` with two offsets,
`merge_reel`/`unmerge_reel` with byte-for-byte identity (t566), the working view
`bench` next to the canon, the bands in the undo snapshot, `Band::after_batch`
as an exhaustive `match` (t568), addressing of edits was not needed at all
(t569).

**The visible part is done (R.55, item 445).** The gestures (double-click and Enter —
expansion, the named menu item — full entry), the Esc cascade, the tone and labels
of the band's rows, the frame of the block, the veil over the tails, the grab of the window edge directly in the band
(`Drag::NestWindow`), the rule "selection within one reel", the ×N warning, tests t570-t573 (the live `nestopen.py` was moved into them entirely, R.68: its assertions were about the document and about what is playing).

**The band INSIDE the full entry (R.58, item 460).** The owner's complaint:
"when I open a composition I cannot expand a nested one inside it". The model did not forbid this (the canon after `swap_reel` is the current reel, the bases are computed from it); two application-level guards were removed, the Enter cascade inside is the same as at the root.
Tests t634 (application), t635 (model); the live `nestenter.py` was moved into
them in R.68 — step 4 is held entirely by t634.

**The bands survive a change of context (R.58, item 461).** The owner's decision —
a revision of R.56 item 5: EVERYTHING is restored, including the band through which one entered by the second Enter (`NestStep::band` — on entry, `KtApp::reel_bands` — on exit; a `ClipId` is stored, the band is recomputed). Test t636, t457
rewritten; step 5 of the live `nestenter.py` was moved into t636 (R.68).

**The zone down to zero inside the full entry (R.58, item 462).** An event moves to the left of
the reel's zero (`KtApp::time_floor`), is shown under the veil, and the cursor and playback do not go there (`CURSOR_FLOOR`); a negative `start`
goes into the file (`kt.start`, v8). Tests t637/t637b/t637c/t638 (the live `nestzero.py` was moved into them in R.68:
the drag of the event was clicked through live in `select`/`level`/`corner`).

**Left outside the round:** recursive expansion (the constant "one band at a time" stands), and four
snags found by the reconnaissance from the section "What gets in the way" — the track names when the context is exchanged, `trim_max_end` beyond the end
of the content, `overlap_layers` of different reels in one view, and the highlighting of the morpher
window across the band boundary.

## Sources

- `docs/features/timeline/structure/nested-sequences.md` (F-TL-004) — the reconnaissance
  of nested sequences on which this note stands.
- `docs/porting-notes.md`, items 378-395 (R.50) and items 434-443 (R.55).
- `docs/user-feedback.md`, section R.55 — complaint 9 verbatim and the owner's decisions.

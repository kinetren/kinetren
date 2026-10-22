---
id: F-HLP-012
title: Missing source: how the editor reports it
status: researched
priority: B
cost: M
category: help/errors
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-media, kt-app, kt-model, kt-engine]
invariants: [5, 11]
ui_rules: [13, 18, 19]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-HLP-012 — Missing source: how the editor reports it

## Essence

A file that the edit refers to has disappeared: the disk was disconnected, the folder was renamed, the client sent the material again into another directory. The editor asks four questions in a row — WHAT is missing, WHERE in the film the hole is now, whether one can keep working and rendering, and WITH WHAT it can be fixed. A poor editor answers all of them with one modal window on opening and a red fill for the rest of the time. A good one answers in four different places, because the questions are asked at four different moments.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Offline media / Resolve Offline Media | when an unavailable file is added, a list of offline files pops up with the statuses `Offline` / `Probable Match` / `Found`; buttons Smart Search (guess), Browse (point by hand), Leave Offline; an event with offline media stays on the timeline, but users run into trouble at render time |
| Adobe Premiere | 26.x | Media Offline / Link Media | a red plate "MEDIA OFFLINE" in the program monitor and in the export itself; the Link Media dialog: `Locate`, the checkbox "Relink others automatically" and the set `Match File Properties` (File Name, File Extension, Media Start) — the found file pulls the others along; export is not blocked but warns |
| DaVinci Resolve | 21 | Media Offline / Relink Media | an orange Relink Media icon in the Media Pool; the dialog shows the NAMES OF THE VOLUMES the material came from; `Relink Selected Clips` searches by name and timecode in the chosen folder and, on failure, offers "deep search"; `Change Source Folder` moves the whole path; Edit Index can "Show Offline Clips Only"; when placed in the Render Queue — a warning window "cancel / add anyway" |
| Final Cut Pro | 12.3 | Missing File | a red frame with a yellow triangle in the browser, the viewer and on the timeline; `File > Relink Files > Original Media`; the window by default shows only what is missing; `Locate All` / `Locate Selected`; at the bottom the count "3 of 3 files matched"; the replacement must match type, frame rate and audio channels; the relink command CANNOT be undone |
| After Effects | 26.x | Missing footage / Placeholder | in the Project panel the name is in italics with a `Missing` label, in the composition — a placeholder of colour bars; it is fixed by double-clicking the project row or with `File > Replace Footage > File`; effects applied to the missing layer stay intact; the colour bars go into the render |

A second table (software outside the core) is deliberately absent: the mechanism comes precisely from video editing, all five core rows are filled, and the "trick behind it" is taken from the core — by the criteria of `_editors.md` a row outside the core would be filler.

## The trick behind it

**The cargo that does not fit into a status-bar line is a FOREIGN NAME written into the FRAME ITSELF.** A status-bar line lives until the next hover and dies with the session; the frame travels into the video stream, the exported file, the client's screenshot and a still a year later. For us this cargo has already arrived: `render()` in `src/libs/kt-media/src/placeholder.rs` draws the file name with the built-in 3×5 font directly into RGBA. None of the five core editors writes the name into the frame.

1. **A stub that does not name itself can be fixed only from the project.** In AE the placeholder is literally "A still image of color bars": the same picture for any missing file. In Premiere and Resolve the words "MEDIA OFFLINE" stand in the frame, in FCP — a red frame with an icon, in Vegas — a black event. In all five the name lives SEPARATELY (italics in the Project panel, a row in the Media Pool, the Edit Index). The practical consequence shows only after some work: a screenshot of a damaged render sent by the client contains not a single bit of diagnostics — "you have stripes here", and that is all; they go to the project and look with their eyes for which of forty clips is red. For us the same screenshot names the file.

2. **Batch relink is not "the same folder", it is a GUESSED REGULARITY.** The strongest variant is in FCP: you point at `File1_A` instead of `File1` — and it goes looking for `File2_A`, `File3_A`, and if the sources lay in two neighbouring folders, it will look in the folders next to the chosen one. Resolve does it in two passes (first along the OLD paths as a hint — this is fast — then a "deep search" across all subdirectories) and can derive a relative volume mapping from one hit ("Automatically setup when relinking"). AE gets by with one phrase: "In most cases, you need to relink only one footage file". Common to all: what gets repaired is not a file but a HYPOTHESIS about how the whole folder moved.

3. **Partial matching is where the editors diverge most.** Resolve lets you choose the strictness by hand: `Loose Filename Match` ignores the extension (you can substitute the material with a file of ANOTHER format), `Tight Filename Match` requires the extension to match. FCP forbids instead: the replacement must match type, frame rate and audio channels, and the relink itself does not roll back via undo — the cost of an error is maximal. Vegas is the most honest about GUESSING: in the list, next to the status `Found` (chosen by a person) there is a separate status `Probable Match` (found by Smart Search). None of the five, however, promises that the substituted file will have the same LENGTH: the timecode and the tail of the event remain the editor's responsibility.

## Pros

- The stub is an ordinary `MediaSource`, not a special case: the engine does not know about it. Therefore it travels the same path as a real file — preview, prefetch, cache, morphers, FX, export — and invariant 5 (exactly one MediaWorker owns the decoder) is not touched by a single line.
- The stub's duration is 1000 hours, `fps: None`. The edit is not trimmed by the missing file, and the frame grid stays the PROJECT's. Compare with the AE manual's advice "set the placeholder to the same size, duration, and frame rate as the actual footage" — there a mismatch in length breaks the layer.
- The 3×5 font is built into the crate, so the frame is deterministic and depends neither on system fonts nor on egui: the name in the frame does not break invariant 11 (byte-for-byte equality of runs and reference), and preview == export.
- Paths are written RELATIVE to the project file (decision R.24 item 7, `src/libs/kt-otio/src/url.rs`). The most frequent cause of relink for all five — "the whole folder moved" — does not happen for us at all.
- Four messages stand at four points where a DECISION is made, not in one log: on opening, before the render starts, in the summary after it, when the file returns. None of them created a new strip in the interface — the warning and the counters live inside windows that already exist (rule 13).
- The split "by name versus summary" came out of the design itself: `warn()` overwrites the single `status` slot, so the status bar keeps the LAST message — the summary "unavailable sources: N" — while the by-name list lies whole in the bell.

## Cons

- There is no relink in any form. `kt_model::Command` has `AddMedia` and `RemoveMedia` and nothing that would change `MediaRef::path`, so the only way to repair is to put the file back in its old place; everything else is done by a text editor on the `.otio`.
- The message names the file and does not name an action. Rule 19 ("the dialog must say where the decision can be changed later") is not met — and cannot be met while no action exists.
- The placeholder is placed at exactly two entries — opening a project and starting a render. A file that disappears while a project is already open gets no placeholder: the guard will notice it and put a badge on the bin card, but the player keeps a live decoder, and there is no reverse replacement "real → placeholder" in the code.
- "Missing" is determined in two different ways: the warning BEFORE the start counts as missing what is not on disk (`exists()`), while the placeholder is given to what did not OPEN. A broken or unsupported file lies in place, triggers no warning and surfaces only in the summary AFTER the render.
- The reason is taken from `thiserror` in English and goes into the notification text bypassing the i18n catalogues: the Russian interface shows a Russian frame around an English middle.
- A non-ASCII name becomes `?????` (deliberately — for the determinism of the frame). For Cyrillic and CJK names precisely, the frame stops doing the one thing it was written for: carrying the name.
- The guard builds the list of paths ONCE, when a project is opened: media added later is not tracked, and it cannot come back from offline.

## What users say

> Exported video has 'MEDIA OFFLINE' plastered over random shots. Once again, those shots ARE THERE inside Premiere.
> — UK TV Professional, 11.08.2023, https://community.adobe.com/t5/premiere-pro-discussions/export-tells-me-quot-media-offline-quot-but-it-s-visable-in-premiere/td-p/13999862

> I export and then at some point in the exported file the media offline
> graphic pops up. Sadly i have now lost all trust in premiere.
> — David Payne, 23.03.2018, https://creativecow.net/forums/thread/media-offline-graphic-on-export-but-timeline-is-fi/

The warning BEFORE the start — the same thread, four days later, and this is exactly our second level:

> The new adobe media encoder actually warns me about an offline file. Now,
> this is GREAT...
> — David Payne, 26.03.2018, https://creativecow.net/forums/thread/media-offline-graphic-on-export-but-timeline-is-fi/

On the render block in Vegas (our item 10 of R.33 was written against exactly this):

> now I can't render the video
> — Martin Saur, https://creativecow.net/forums/thread/cant-render-video-because-of-offline-media/

And on the nameless stub of AE — the complaint is not that the name is absent but that the stub SHOUTS while telling nothing:

> the big loud colour bars are more of a hassle than a black frame which I
> may be able to get away with from time to time.
> — Mag_44, 06.12.2005, https://creativecow.net/forums/thread/can-you-toggle-off-the-missing-footage-colour-bars/

## How it fits Kinotrain

### What already exists

The primary source of the requirement is `docs/user-feedback.md`, R.33 item 10, verbatim:

> "During render and preview, a missing source must not be a blocker: notify the user and render anyway; for missing video a placeholder must be substituted." (Owner's request, R.33 item 10.)

And his answer to a clarifying question, in the same round:

> "The placeholder for missing video is a dark plate with the file name and an icon ('immediately visible WHAT is missing'); audio is silence. Notifications: the notice list when the project opens, a warning in the render dialog BEFORE the start, and counters of what was lost in the summary AFTER." (Owner's answer to a clarifying question, same round.)

Done in items 206-209 of `docs/porting-notes.md` (the stub, opening, render, return — four "halves" of one owner request):

- The stub itself — `src/libs/kt-media/src/placeholder.rs`: a dark frame with an orange border, a film icon, the file name in the 3×5 font, silence in audio, a duration of 1000 hours, `fps: None`; the frame is cached by `Quality`. Test 208 there: the frame is deterministic, does not depend on time, different names give different frames, non-ASCII is replaced by `?`.
- Level (a), opening the project: `src/apps/kt-app/src/util.rs:59` (`open_source_or_placeholder`) returns a pair "source + error", and `src/apps/kt-app/src/app.rs:684` sends a SEPARATE `warn()` for each file with the system cause (`media_missing`) and then one `note()` with the summary (`media_missing_count`). Before this round only the LAST error survived, and the clip remained without any source at all, and the preview hung in "Loading".
- Level (b), before the render: `src/apps/kt-app/src/render_dialog.rs:101` (`missing_sources`) and the warning in the settings window (the `missing_warn` string — "unavailable sources (N), they will get into the file as placeholders: …").
- Level (c), after the render: `render_dialog.rs:80` (`open_all`) accumulates the names, and the summary prints `missing_after` and `missing_layers` — `ExportStats.missing_video_layers` (`src/libs/kt-engine/src/export.rs:53`). Before this round it was not shown in the GUI at all, and an incomplete render looked like success.
- Level (d), the file returns: the guard `src/apps/kt-app/src/util.rs:183` polls existence every 900 ms, `KtApp::poll_returned_sources` (`app.rs:2131`) reopens the file IN THE BACKGROUND and swaps the source through `Player::replace_media` (`src/libs/kt-engine/src/player.rs:463`), after which `media_online` arrives. The order in the engine matters: stop the worker → invalidate its frames → bring up the new one.
- A fifth surface that is easy to overlook: a bin card of offline media gets a warning badge both in the thumbnail and in the caption (`src/apps/kt-app/src/bin_panel.rs:692` and `:783`), and hovering puts the full path into the status-bar line.
- Regression guards: t209 (a project with two missing files opens, BOTH are named, the preview is assembled), t210 (the render goes on and warns), t211 (a returned file replaces the placeholder) — `layout_tests.rs:3944`, `:3871`, `:3795`; the engine half of t211 — `kt-engine/tests/player_dynamic.rs:204`.

### What is missing

1. **Relink — none at all.** By grep over `src/`: `SetMediaPath` does not exist, `MediaRef::path` is immutable after `AddMedia`, and there is no dialog "point to a new path". This is the main gap of the note: in all five core editors this is the central command of the domain, while for us the only scenario of repair is "put the file back".
2. **kt-cli is still a blocker.** `src/apps/kt-cli/src/main.rs:56` and `:283` print an error and return 1 at the first file that does not open. The R.33 item 10 requirement is closed in the GUI and is not closed in the headless path that CI and any batch render use.
3. **There is no reverse substitution**: a live source → placeholder on the fly.
4. **The list of missing files has nowhere to click.** Resolve has "Show Offline Clips Only" in the Edit Index — a jump to the hole in the edit. For us the name lies in the bell as text, and the event can be found only by eye (Ctrl+F across the project searches media, but there is no link "notification → event").
5. **None of the four messages says HOW to repair** (rule 19).

### What gets in the way

Less than it seems. The UI half of the relink is already written and tested for another task: `crate::widgets::media_picker` / `media_picker_raw` (`src/apps/kt-app/src/widgets.rs:612` and `:657`) — the source field of the morpher with search, file DnD and a choice from the project's media (R.33 C2, test 218). What is missing is the model half: a command for changing the path with undo, and a decision about what to do with `ktindex.db` (the batch index, the peak pyramid and the thumbnails are tied to the media and go completely stale after a substitution).

Three structural constraints:

- **The guard lives with the list of paths taken at opening.** A path change must rebuild it, otherwise "repaired, and it did not let go".
- **`.otio` already supports several `media_references` with an active key** (`src/libs/kt-otio/src/convert.rs:50`) — that is also the place for future proxies (F-MED-002). Relink must change the ACTIVE reference rather than overwrite the others: the passthrough of foreign files is held by tests t144–t147.
- **The price of text.** Every new help line means five languages, a limit of 160 characters (`src/apps/kt-app/build.rs` fails the build), glyphs (t100), font subsets, bidi, t125/t128/t131; the details are not recounted here — F-HLP-033. Therefore "where to repair it" has to fit into ONE phrase inside the already existing notification, not become a paragraph.

Separately, it is worth remembering FCP's warning "You can't undo the Relink Files command": we have undo for everything, and a relink without undo would be the only exception — so it cannot be done in that form.

### Which invariants it touches

Invariant 5 (one MediaWorker owns one source) — the substitution must go through `Player::replace_media`, not around it; invariant 11 (determinism) — the placeholder frame is already in the GOLDEN paths, and its drawing cannot be changed without recomputing the references. The rules of the checklist in `src/apps/kt-app/CLAUDE.md`: 13 (do not create new strips — warnings live inside the existing windows), 18 (a menu item says WHAT and WHERE), 19 (the dialog says where the decision is returned) — broken today.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2-3 stages of the round |
| Where the main work is | kt-model (a command for changing the path + undo), kt-app (the relink dialog on the ready `media_picker`, rebuilding the guard, the texts), kt-cli (a placeholder instead of `return 1`) |
| Project format | not touched: the path is already in `target_url`, the version does not grow |
| Tests to write | `reach_tests::offline_message_offers_a_way_back` (the message names an existing repair command); a round-trip of the model on a path change; a harness test "point to one file → the neighbours from the same folder are picked up"; a kt-cli test "a render with a missing source goes on and prints the counters"; invalidation of `ktindex.db` after a substitution |
| Depends on | nothing; overlaps with F-MED-002 (a proxy is the same source substitution) |
| Breaks compatibility | no |

## Verdict

The four levels of messages and the placeholder with the name in the frame are the best there is in the project in terms of messages, and the reconnaissance confirmed this from an unexpected side: the file name IN THE FRAME is absent in all five core editors, so here we are not catching up, we are ahead. Nothing needs to change in this part.

The reconnaissance changed not the decision but its BOUNDARY. R.33 item 10 dealt with the question "can one keep working?" and answered it completely. The question "with what is it repaired?" it did not deal with at all — and it turned out that we have no answer to it, neither in the messages nor in the model: the message honestly names the file and tacitly assumes the user will put it back by hand. All five editors have a central domain command with a batch search at this place. Plus two ends of the same requirement that remain open: `kt-cli` still returns 1 when a file is missing, and a reverse substitution "live source → placeholder" when a file disappears on the fly does not exist.

TEST: reach_tests::offline_message_offers_a_way_back — fails today, because all four messages about offline (`media_missing`, `media_missing_count`, `missing_warn`, `missing_after` — lines 441-443 and 512-514 of `i18n/*.toml`) name the file and do not name a single action by which it is repaired: `kt_model::Command` has no variant that changes `MediaRef::path`, and `src/apps/kt-cli/src/main.rs:56` on the same event returns 1 instead of a placeholder.

ONE LINE: overlap — not touched | delay — not touched | capacity — the phrase "where to repair it" is added to the TEXT of the already existing notification and to the context menu of the bin card; the relink itself lives in a window to which the user comes deliberately (the ready `media_picker`); no new layer, no popup.

## Sources

Live, none of the five editors was launched. The core — from the manuals of the vendors (Resolve and FCP were read locally from PDF, by text search), Premiere — through the search index: `helpx.adobe.com` does not open from this environment either directly or through the text proxy (the page gives only navigation), therefore the wording of the Link Media dialog is taken from the indexed text of the same pages and from the official Adobe community. The access date for all of them is 2026-09-07.

**Core.**

- DaVinci Resolve 21 Reference Manual (PDF, read locally): Ch. 17 "Relinking Media Simply" pp. 414-416 (Relink Media, Relink Selected Clips, deep search, Change Source Folder), Ch. 6 p. 168 "Automatically setup when relinking", Ch. 22 p. 503 Loose/Tight Filename Match, Ch. 187 p. 4212 "Media Offline Warning in Render Queue", Edit Index "Show Offline Clips Only" —
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
- Final Cut Pro User Guide for Mac 12.3 (PDF, read locally): "Relink clips to media files" pp. 855-857 (Locate All / Locate Selected, the count of matches, the output of the naming regularity `File1_A` → `File2_A`, the requirement of matching type/fps/channels, "You can't undo the Relink Files command"), "Alert icons" — Missing File; the web version of the same page —
  https://support.apple.com/guide/final-cut-pro/relink-clips-to-media-files-ver26f5c8c9/mac
- Adobe Premiere 26, "Relink offline media automatically" and "Relink offline clips" — https://helpx.adobe.com/premiere/desktop/organize-media/file-organization/relink-offline-media-automatically.html and https://helpx.adobe.com/premiere/desktop/organize-media/ingest-proxy-workflow/relink-offline-clips.html (the pages were read through the search index, see the caveat above)
- After Effects: the mechanism of the placeholder and of the colour bars is described verbatim in "Placeholders and proxies" / "Work with placeholders and missing footage items" of the After Effects CS5 manual (read locally); the wordings "A still image of color bars" and "In most cases, you need to relink only one footage file" are from there. `[NOT VERIFIED]` for 26.x: that the mechanism has not changed is confirmed only by secondary sources and forum threads.
- VEGAS Pro: the "Resolve Offline Media" dialog with the statuses `Offline` / `Probable Match` / `Found` and the buttons Smart Search / Browse / Leave Offline is described in the VEGAS Pro 9 manual (read locally, section "Resolving offline media files", p. 85). `[NOT VERIFIED]` for 2026.0: the online help of Boris FX and MAGIX has no section on missing files (`3-projects/open.htm` and `4-media/project_media.htm` were checked in both helps); the confirmation of the modern behaviour is only from forums.

**Forums (quotes).**

- https://community.adobe.com/t5/premiere-pro-discussions/export-tells-me-quot-media-offline-quot-but-it-s-visable-in-premiere/td-p/13999862
- https://creativecow.net/forums/thread/media-offline-graphic-on-export-but-timeline-is-fi/
- https://creativecow.net/forums/thread/cant-render-video-because-of-offline-media/
- https://creativecow.net/forums/thread/can-you-toggle-off-the-missing-footage-colour-bars/

**What was searched for and NOT found** (so that the next wave does not search again): descriptions of missing material in the online help of VEGAS 2026 (`open.htm`, `project_media.htm`); confirmation from a primary source that Premiere and FCP actually WARN before an export with offline material rather than block it (for Resolve this is stated directly in the manual, for Premiere — only in forums, for FCP we found nothing); at least one editor that writes the NAME of the missing file into the placeholder frame — we did not find a single one, and this is the "trick behind the trick" of the note.

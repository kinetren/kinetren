---
id: F-HLP-002
title: Info View in Ableton Live: a permanent place for a paragraph
status: researched
priority: B
cost: S
category: help/hints
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, ableton-live-12, blender-5.2, photoshop-27]
we_have: partial
crates: [kt-app]
invariants: [12]
ui_rules: [7, 10, 13]
round: null
point: null
tests: []
depends_on: [F-HLP-001]
checked: 2026-09-07
---

# F-HLP-002 — Info View in Ableton Live: a permanent place for a paragraph

## Essence

In the lower-left corner of the Live window there is a rectangular area. It does not pop up and does not disappear: while it is open, it always shows something written about whatever the cursor is over — a bold heading with the name of the item, and under it a paragraph of several lines about what it is and why. The item's keyboard shortcut goes there too, on a separate line. The area is switched off with the `?` key or with the button at the lower left of the status bar, and the project owner can WRITE into it his own text about a specific track or clip — it is saved in the project file.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no permanent description area; the status bar serves menu items and progress, and buttons have pop-up tooltips (analysis — F-HLP-001) |
| Adobe Premiere | 26.x | none | there is no fixed area: a hint pops up next to the element |
| DaVinci Resolve | 21 | none | there is no fixed area: a hint pops up next to the element |
| Final Cut Pro | 12.3 | none | there is no fixed area: a hint pops up next to the button |
| After Effects | 26.x | the Info panel — but not about this | the panel shows pixels, coordinates and time, that is, the state of DATA; there is no description of an interface element in it |

Five out of five: nobody has a permanent place for a DESCRIPTION of an element. That is why the second table is mandatory — the mechanism does not come from video editing.

**Outside the core.** Without Ableton Live there would be nothing to write this note about: Info View is the only mass-market product in the industry that made exactly our choice (a permanent place instead of a popup) and at the same time chose a DIFFERENT capacity; Blender and Photoshop are needed to separate "a permanent place" from "one line" — they have the place, but no paragraph.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Ableton Live | 12.4.5 | Info View | an area at the lower left: a heading + a paragraph about what the cursor is over, plus a line with the keyboard shortcut; switched on with `?`; the user's notes are also written into it, and they are saved in the project |
| Blender | 5.2 LTS | Status Bar (Keymap Information) | on the LEFT of the status bar — what the mouse buttons do in the current tool; pressing Alt redraws the bar with the tools' keyboard shortcuts (`Alt Click Tool Prompt`). One line, but not about the item under the cursor — about what can be done NOW |
| Photoshop | 27.x | Info panel → Show Tool Hints | at the bottom of the Info panel — an optional hint line about the SELECTED tool (not about what is under the cursor); switched on by a checkbox in the panel's options |

## The trick behind it

**What the mechanism carries that one line cannot hold: a paragraph "what it is for" — and, as a bonus, someone else's text.** Our cargo will not fit into a line: it either does not arrive, or arrives on another surface.

Three details that are not in the feature list on the website.

**1. The payload is TWO fields, not a line.** Each element has an `Annotation Name` (heading) and an `Annotation` (body); this is not an internal detail but a public extension: the authors of third-party Max for Live devices fill both fields in the Max inspector, and their text is shown in the vendor's help area ("Info text is displayed in the Live Info View when hovering the mouse over a parameter", Ableton, m4l-production-guidelines). That is, Live's help channel is an API, not a set of lines in a catalog. Our hint is a single flat `&'static str` in which the heading, the purpose and the accelerator are glued together by the translator's hand.

**2. The price of the space is named IN the text of the hint itself.** If you hover the cursor over Info View, it describes itself, and the description contains the phrase "To save screen space, you can hide Info View by clicking the view control at the bottom left corner of the Status Bar". The vendor documents the expenditure directly in the payload. The expenditure is measurable: on the official screenshot of the guidelines (window 2000×1800) the panel takes 420×412 px — 21 % of the width and 23 % of the height, about 5 % of the window's area, and this is PERMANENT, not at the moment of hovering. Separately one sees the device of the text: the heading `Info View`, seven lines of body (308 characters), an empty line and, on a separate line, `[?] Show/Hide Info View` — the accelerator is not squeezed into a sentence, it has its own place.

**3. Two owners collide in one area.** The same area carries the user's notes (`Edit Info Text` in the context menu of a track, clip, device; the text is saved in the Live Set) — and the vendor's help displaces them: "I wish there was an option to ONLY show user text in the help view". And most importantly — someone else's text in this area is visible only under the cursor, and 13 years after the feature appeared, users ask to highlight the area when a note exists, because they forget their own entries. The largest help area in the industry reproduces inside itself exactly the defect because of which this wave was launched: **one cannot hover the mouse over what one has not found** (the thesis of R.40, analysed in F-HLP-001). Ableton's answer was not a larger area but a SECOND channel: Learn View with video lessons, which in Live 12 is shown by itself on the first launch.

**How Ableton itself closes what Info View cannot.** This is the most useful observation for us, and it concerns architecture, not hints. Info View answers "what is this" and, per the manual, **does not name the place of a setting** — there is no reverse addressing to Settings in it at all. The question "what can one even do here" the vendor solved not by growing Info View but by a **separate view**: Learn View with structured lessons (video + text, filter by topic, a "completed" mark), open by default and closable via the Help menu. That is, the only one in the sample that chose our architecture of hints, when it ran into its limit, created a SECOND SURFACE rather than enlarging the first. For the wave's thesis (pull is solved, push is not solved) this is direct confirmation from someone else's field.

## Pros

- **Capacity is a consequence of the shape, not of generosity.** The area is rectangular and the text in it WRAPS, so a paragraph fits (the sample is 308 characters, seven lines). A line cannot wrap: its capacity is limited not by a decision but by the geometry of the strip.
- **The heading is separated from the body**, so "what is it" is read in one glance, and "why" only if the reader keeps reading. A single line forces both answers into one sentence and always shows both.
- **The accelerator is a separate line**, not a tail in brackets. So it can be added, removed and translated separately from the hint text.
- **The area is an extension point.** The author of a third-party device writes into the host program's help channel without asking the host anything.
- **It is switched off with one key and remembers its state.** An experienced user takes the 5 % of the window for himself, a newcomer leaves it.
- **It also works in the settings.** Info View describes the elements of the Settings window too, so it covers dialogs and not only the main window.

## Cons

- **Permanent 5 % of the window.** Everyone pays it all the time, including those who never look at the area; the switch exists precisely because the cost is noticeable.
- **The eye travels across the whole screen.** A complaint living on the forum for 15 years: "The problem is the amount you have to move your attention and eyes takes you away from the very button or area you're interested in" — and this is an objection to the permanent place AS SUCH, not to its size. Our line pays the same price, only in a different corner.
- **All or nothing.** Switching the area off for the sake of the space, the user loses both the vendor's help and their own notes: two masters share one switch.
- **The paragraph ages and gets fixed from bug reports.** On the forum a separate genre lives on: "typo in info view for external instrument", "Info View text needs updated for Default Pad function", "Info View describes Transients in MIDI Editor" — that is, the help text needs maintenance on a par with the code, and the error in it is caught by the user, not by the build.
- **Translation lags behind.** "After selecting spanish language in Preferences, i have noticed that the Info view, as some interface sections, are partially translated" — a paragraph is five times longer than a line, and localization of paragraphs lags behind localization of labels.
- **The width of the area depends on the text.** Users recorded that when switching long paths in "Open Recent Set" the area is "increasing and decreasing in width" — the same artifact as our badge that covers the metrics on the left.
- **The area does NOT track modifiers.** The public API is two static fields per element; there is no "what if Alt is held" variant in it; the keys are listed INSIDE the text. The only dynamic reaction to a held modifier among the three surfaces of this note is given by Blender (Alt redraws the keymap in the status bar). [NOT CHECKED live for Live's own elements: the manual, the M4L guidelines and a forum search for "info view + modifier" contain no description of such a reaction.]

## What users say

> That thing is weird, like it's designed for people in their first week of
> Live use and then everyone else just closes it and forgets about it.
> — Angstrom, forum.ableton.com, 07.03.2011,
> https://forum.ableton.com/viewtopic.php?t=160133

> if you know how to use live then what's the point? closed.
> — seattletruth, same thread, 07.03.2011,
> https://forum.ableton.com/viewtopic.php?t=160133

> I was really excited when I learned that you could finally put notes into
> info view for clips, but the fact that it also shows you the help text as you
> navigate around the program is distracting and annoying.
> — Tarekith, same thread, 07.03.2011,
> https://forum.ableton.com/viewtopic.php?t=160133

> The problem is the amount you have to move your attention and eyes takes you
> away from the very button or area you're interested in.
> — wolfer, forum.ableton.com, 25.10.2019,
> https://forum.ableton.com/viewtopic.php?t=236330

> I highly doubt you will ever see this added, it's not even something that has
> any precedence in the UI currently.
> — Tarekith on tooltips, 07.01.2024,
> https://forum.ableton.com/viewtopic.php?t=236330

> It's pretty ridiculous to have to Google the locations of buttons on a UI
> because your not sure where to find them.
> — closetgeek, same thread, 07.01.2024 (and after it: buying a full license, "henge
> very strongly on issues like this"),
> https://forum.ableton.com/viewtopic.php?t=236330

> I think showing the info view under the browser while in full-height would
> save more screen space.
> — yannxou, Live 12.1, 07.11.2024,
> https://forum.ableton.com/viewtopic.php?t=251289

> The Info View is a criminally underused feature in Ableton Live. […] In fact,
> I just learned a keyboard shortcut for the MIDI editor while writing this.
> — Jon Margulies (Hobotech), hobo-tech.com,
> https://hobo-tech.com/technologies/livetips/use-the-info-view/

The outcome of the feedback, important for the verdict: the dispute is NOT about capacity but about direction and place. In 20 years not a single thread complains that a paragraph is too large; people complain that for its sake the eyes have to travel to the corner, and that an experienced user does not need it at any size. That is exactly the conclusion for which this note is written: increasing the capacity does not cure the push.

## How it fits Kinotrain

### What already exists

- The slot is the same: `KtApp::hint` / `hint_into` / `draw_status_hint` / the pure `status_line` in `src/apps/kt-app/src/app.rs`; the status bar strip is set up in `draw_statusbar` in the same file. The model is fully described in **F-HLP-001** — it is not retold here.
- There are exactly 81 hints (the keys of sections `[tb]`, `[tl.hint]`, `[hint]` plus everything ending in `_hint` in `src/apps/kt-app/i18n/*.toml`). The average length is 45 characters (en) / 46 (ru) / 49 (es). The longest is `mrph.below_note`: **145 characters in Spanish against a ceiling of 160**, that is a margin of 9 %. `tl.hint.clip` is 107 (en) / 109 (ru) / **116 (es)** — it already carries five alternatives with modifiers in one sentence: what Ableton puts in the body and in a separate line, we have glued together with commas.
- The paragraphs "what it is and why" are ALREADY written for us and ALREADY translated into five languages — `site/content/*.toml`, section `[fxdesc]` (10 values, 36–104 characters). They are not in the program: `[fxdesc]` is read only by `site/build.py`. This is exactly the useful payload for which Info View exists.
- The guard against popups and its guard test: `src/apps/kt-app/src/theme.rs` (`tooltip_delay`) and **t83** in `src/apps/kt-app/src/layout_tests.rs`, which checks that a button's hint reaches `status_line_shown` in THE SAME frame and that among the layers there is not a single `egui::Order::Tooltip`. This note does not touch t83 and does not propose to weaken it: everything proposed here is drawn in the already existing strip and in already open windows.

### What is missing

**Nobody knows the real capacity of our line.** The ceiling of 160 (`src/apps/kt-app/build.rs`, `MAX_VALUE_CHARS`) counts CHARACTERS and stands there not for the sake of the status bar but for the sake of Arabic: `src/apps/kt-app/src/i18n/bidi.rs` reorders the glyph atoms at build time, and already reordered text cannot be wrapped — each visual line would need its own order. That is, multi-line support for us is closed not by taste but by the translation pipeline.

But on screen the limit is different and is checked by nothing. `draw_status_hint` lays out the hint via `layout_no_wrap`, pins the badge to the right edge of the strip and clamps its left edge by `strip.left()`. If the galley is wider than the strip, the badge covers the metrics entirely, and the tail of the text runs past the right edge of the window — without an ellipsis and without a warning. The minimum window size is `with_min_inner_size([640, 640])` in `src/apps/kt-app/src/app.rs`; 145 characters at a proportional 12 px do not fit into 640 px for certain. No test measures this: **t128** (`layout_tests.rs`) measures menu headings, perf-panel rows and the checkbox of the economical UI — and not a single one of the 81 hints; **t255** (`reach_tests.rs`) requires truncation with an ellipsis for SOURCE NAMES, but not for hints. Rule 7 of the checklist ("the text fits") is not closed for the program's main text channel.

### What gets in the way

Not code, but the order of things: as long as the budget is not measured, any wave note that wants to "add a bit more to the line" will argue about characters, while it should argue about pixels in the narrowest window and in the longest language. Copying Ableton's area itself is blocked by rule 13 of the checklist (do not waste space) and by direct feedback from its own users: 5 % of the window permanently is the price for which they switch it off.

### Which invariants it touches

- Invariant **12** — touched, but not violated: text wrapping is computed by egui once and cached by the galley cache per finished string; no timer-driven animations or redraws appear here.
- Rule **7** (the text fits) — the subject of the proposed test.
- Rule **10** (exactly one hint, in one place) — not touched: a second area is not set up.
- Rule **13** (do not waste space) — the reason why Ableton's area is not copied.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of a round |
| Where the main work is | kt-app: `reach_tests.rs` (measurement), with findings — `i18n/*.toml` |
| Project format | not touched |
| Tests to be written | `reach_tests::hint_fits_the_status_strip` — the galley width of each of the 81 hints in all five languages against the strip width at the minimum window size |
| Depends on | F-HLP-001 |
| Breaks compatibility | no |

## Verdict

Info View proves that "a permanent place" and "one line" are two different decisions, and the second does not follow from the first. Ableton chose the same place and a different capacity: a rectangle with wrapping, the heading separately from the body, the accelerator as a separate line, someone else's text on a par with its own. We will not copy this — the price of 5 % of the window was judged unbearable by its own users — but we must stop calling our capacity "160 characters": the real limit is measured in pixels of the strip at the minimum window size, and today nobody knows it.

TEST: reach_tests::hint_fits_the_status_strip — would fail today: `draw_status_hint` lays out the hint via `layout_no_wrap` and clamps the badge by `strip.left()`, so the 145-character `mrph.below_note` (es) at the minimum window size of 640 px runs past the right edge without an ellipsis, while t128 measures the menu and the perf panel and not a single one of the 81 hints.

ONE LINE: overlap — not touched | delay — not touched | capacity — not a single character is added to the line (after measurement it will most likely DECREASE); the paragraph "why is this needed" goes to the already open FX/morpher/transition windows, where the `[fxdesc]` text from `site/content/*.toml` is already written and translated, and the surface this note touches — the status bar strip itself — is touched in it only for its budget.

## Sources

- Ableton Reference Manual v12, 2.2.2 Info View —
  https://www.ableton.com/en/manual/first-steps/ (accessed 07.09.2026)
- Ableton Reference Manual v12, 41.1 Showing and Hiding Views (`Shift ?`) —
  https://www.ableton.com/en/manual/live-keyboard-shortcuts/ (07.09.2026)
- Screenshot `InfoViewL12.png` from the manual (description of Info View by Info View itself) —
  https://ableton-production.imgix.net/live-manual/12/InfoViewL12.png
  (07.09.2026)
- Max for Live Production Guidelines, section "Info Text" + screenshot
  `images/InfoText.png` (`Info View Text` / `Info View Title` in the inspector) —
  https://github.com/Ableton/maxdevtools/blob/main/m4l-production-guidelines/m4l-production-guidelines.md
  (07.09.2026); area measurements were made from this screenshot (2000×1800)
- forum.ableton.com: t=160133, t=236330, t=251289, t=248345, t=249315,
  t=49532, t=54714 (07.09.2026)
- Blender 5.2 LTS Manual, Status Bar → Keymap Information —
  https://docs.blender.org/manual/en/latest/interface/window_system/status_bar.html
  (07.09.2026, via the text proxy `r.jina.ai`)
- Adobe: Photoshop image information (Info panel, "Show Tool Hints") —
  https://helpx.adobe.com/photoshop/using/image-information.html (07.09.2026,
  via `r.jina.ai`)

Nothing was checked live. Ableton Live, Blender and Photoshop are software outside the core, and a live check for them is not assumed (`_editors.md`). The lines of the core's five NLEs are taken from documentation and from the analysis in F-HLP-001; by forum evidence, Live's interface elements have no hints of their own, and this claim was not checked live either.

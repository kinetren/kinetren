---
id: F-HLP-018
title: A label that explains itself — "what" and "where" right in the name
status: candidate
priority: B
cost: S
category: help/discoverability
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, jetbrains-2026.2]
we_have: partial
crates: [kt-app]
invariants: []
ui_rules: [7, 13, 17, 18]
round: null
point: null
tests: []
depends_on: [F-UI-002, F-HLP-016, F-HLP-017, F-HLP-033]
trackers: [kde-invent, kde-bugzilla, pitivi, openshot]
checked: 2026-09-08
---

# F-HLP-018 — A label that explains itself — "what" and "where" right in the name

## Essence

The cheapest help is a correctly named item. The label says not only the verb but also the object and the place: not "Fade out" but "fade at the end of the event"; not "Remove crossfade" but "remove the transition with the next clip". The editor reads it before pressing, because otherwise they cannot choose — and therefore this is the only help surface that cannot be passed by. We have this recorded in rule 18 of the checklist and — a rare case — expressed as a machine check, t171.

## Who has it and how

We look not at whether the editor has a menu (all of them do), but at whether it has a RULE for wording and whether the trace of this rule is visible — renames in release notes, a documented format for labels, a ban on one label for two actions.

| Editor | Version | How it is named | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | standard Windows menus, verb+object | A rule exists de facto (`Delete Track`, `Insert/Remove Envelope ▸ Volume`); SCS/Boris FX have not published a public guideline on labels, and distinguishability is held by editing. [NOT VERIFIED: `vegascreativesoftware.com/version-history/` returns a JS shell without the text of the versions, so renames cannot be seen there either — neither confirmed nor refuted] |
| Adobe Premiere | 26 | Title Case + object in the label | Labels name the result, not the mechanism: `Ripple Delete` versus `Delete`, `Paste Insert` versus `Paste`. The pair "Delete / Ripple Delete" is the reference case where one label without the second word would mean destruction of the edit [NOT VERIFIED: `helpx.adobe.com` does not respond at all from this environment (timeout and HTTP 000), the composition of items is from secondary sources] |
| DaVinci Resolve | 21 | a label that depends on the page | The same word on Edit, Fusion and Fairlight means different things, and distinguishability is given not by the text but by the page: the context is taken from the root of the interface. The reverse side — when the habit is carried between pages, the label stops being self-sufficient. [NOT VERIFIED: the what's-new on `blackmagicdesign.com` is a marketing page, it contains no words `renam`/`now called`; the real release notes are PDFs] |
| Final Cut Pro | 12.3 | Title Case, verb+object; search by the command's description | **The only one of the five whose correction of an incorrect label made it into the release notes in plain text** (10.4.1: `Reset Point of View` → `Reset Angle`, "is now listed correctly"; 10.4.4: `Join Clips` and `Join Captions` merged into one command). Plus, the Command Editor searches for a command by the words of its DESCRIPTION, that is, the text next to the command is a search index, not decoration (F-HLP-017): the wording has a second, machine role |
| After Effects | 26 | label + hotkey; many identically named items in different panels | The worst of the five by "where": `Delete` lives on a layer, on a mask, on a keyframe and on an effect, and only the panel in which the menu is open distinguishes them. The model "the object is determined by focus" allows keeping labels short at the price that the label by itself does not answer the question "what exactly will disappear" [NOT VERIFIED, the same unavailable `helpx.adobe.com`] |

**Outside the core.** The rule for wording labels is not published by anyone in video editing: for all five it is reconstructed from behavior. It exists written down only outside NLEs — and there too exists a surface that none of the video editors has: a list where the labels of ALL commands are visible at once and identically named rows are noticeable without any test.

| Program | Version | How it is named | How it works |
|---|---|---|---|
| Visual Studio Code | 1.136 | Command Palette; UX Guidelines, Command Palette page | The norm is short and, contrary to its reputation, weak: "Use clear names for commands", "Group commands together in the same category", "Don't … Use emojis in command names". The format "Category: Command" ("Git: Commit") is NOT declared as a rule — the category is simply moved by the palette into a prefix, and "where" gets into the label as a side effect of display. It works not by norm but by surface: the palette lays out hundreds of commands in a row, and identically named ones are visible to the eye |
| IntelliJ IDEA | 2026.2 | UI Guidelines, Button page | The only source where our rule 18 is recorded as a norm almost verbatim: "The label should be short, not more than five words. If it's not obvious what element the button is related to, add more words to make it clear." And the priority is set explicitly — "Prefer clear labels to short ones", that is, the dispute "distinguishable versus short" is resolved in favor of distinguishability. Plus "Write the label as an imperative verb" and a list of exceptions where `OK` appears as tolerable rather than as a model |

**A written rule exists in three more, and all three are not NLEs.** Apple HIG on alert buttons: "Avoid using OK as the default button title unless the alert is purely informational". GNOME HIG: "Label the affirmative button with a specific imperative verb… This is clearer than a generic label like OK or Done". The most complete wording is Microsoft's (Win32 UX Guide, marked by Microsoft itself as outdated since Windows 7), and it literally describes an escalation we lack: "Use a direct object (a noun after the verb) when the object is not apparent from context" — with three steps `Add` (acceptable) → `Add Items` (better) → `Add Selected Items` (best). They are not entered as a separate row of the table: they have no keys in `_editors.md`, and the mechanism is the same as in JetBrains.

## The trick behind it

The weight of a label is not the text but **the moment and the neighborhood**. The status-bar line answers the one who has already asked with the mouse, and shows exactly one hint at a time; a label is read BEFORE the action, and it is read in a COLUMN, among eight neighbors. Hence a weight that our line will not physically lift: **the meaning of a label is half differential** — "Delete" means "not the eight others". For us this weight goes into the label itself and nowhere else: the only surface where two names stand side by side at the same time is the menu, the combo box and the toolbar, that is, exactly the places where the label lives.

Three details that are not in any list of features.

1. **Ambiguity is a property of the SET, not of a line, and therefore it cannot be seen in the source.** The label `Delete (Del)` does not lie alone: it lies next to three different objects. And the one who writes and translates labels sees lines — one at a time, in the order of the keys in the file — while the user sees a column. For us this has literally come apart: `[trn.cat] wipe = "Wipes"` and `[trn.kind] wipe = "Wipe"` lie in `i18n/en.toml` six lines apart from each other (577 and 583) and are drawn on the screen two pixels apart — the family header and the first item under it in the combo box of the transition kind selection (`transition_editor.rs:164` and `:168`). In the Chinese and Arabic catalogs it is ONE AND THE SAME line: 划像 above 划像, مسح above مسح. The translator made two flawless lines; the set became ambiguous, and nobody sees the set.

2. **A correct label saves not time but UNDO.** An incorrect hint costs a repeated glance; an incorrect label costs an executed action — by the time the person has understood, the deed is done. And the price of the error is asymmetric: we have one key `render.cancel` = "Cancel" on TWO buttons — "close the settings window without having started anything" (`render_dialog.rs:416`) and "interrupt the render in progress" (`render_dialog.rs:531`). The first costs nothing, the second costs everything that has been encoded. This is exactly the defect of item 171 ("one label for four different actions"), surviving on another screen.

3. **A label is the only help surface that has a machine-verifiable property, and the industry does not use this property.** "The text is clear" cannot be checked; "two commands are distinguishable" is a predicate over a finished catalog, checkable without running the program, in all five languages at once. Everything found in the industry works on the INPUT, not the output: `msgctxt`/`pgettext` in gettext and the disambiguation argument in Qt give the translator context, and Mozilla's rules forbid reusing a string — but none of the three checks the RESULT, that is, whether the two translated labels in the assembled interface really differ. The gettext manual sets the task in our own words and on our own example — the `File` menu (Open, New) and the `Printer` menu (Select, Open, Connect): "But in two places the string passed into the function would be Open. The translations might not be the same and therefore we are in the dilemma described above." The solution there is to give context, not to check the outcome. [NOT VERIFIED: the claim "no one has a tool for checking the final distinguishability" is negative and weak; the l10n guidelines of KDE/GNOME/Mozilla have not been fully combed through.] Our own scale should be named just as honestly: ALL the machine checking of distinguishability in the program consists of three `assert_ne!` in `i18n_tests.rs` (lines 292, 297, 304), all three inside t171, over 8 values out of 522.

## Pros

- The channel cannot be not read. Choosing an item physically requires reading its name, so the reach equals 100 % of those who got to the menu, and it does not depend on the habit of hovering with the mouse, on knowing about F1, or on whether the person has ever opened the manual. No other help surface has this property.
- Zero new pixels and zero new frames: the place for the text is already occupied, only the text changes. Rule 13 ("do not waste space") is not touched at all, and invariant 12 (repaint only on motion) — even less so: the line is static.
- The fix requires no code. One label — one value in five catalogs, zero lines of Rust; the cost is counted in F-HLP-033 and does not grow non-linearly with the number of labels corrected.
- The property is formulated as a predicate and therefore outlives its author: "these two commands are distinguishable" is checked by a machine in every language, whereas "clearly worded" is only the taste of a reviewer who will leave in a year.
- The error is caught BEFORE the action, not after. All the other surfaces of the domain (F-HLP-008 errors, F-HLP-013 log, F-HLP-010 the grip of a gesture) explain what has already happened; the label is the only one that works earlier.
- Compatible with the owner's decisions of items 1–2 (item 14 of round 8) word for word and without reservations: nothing pops up, nothing overlaps, there is no delay — the text is drawn inside the item that the user has already opened.

## Cons

- Distinguishability is bought with length, and length is finite. The longest menu label in our catalog is the fix of item 171 ("Remove the transition with the previous clip", 44 characters), the fourth-longest is its pair (40), with an average menu label length of 17.4 (89 labels in `[menu.*]`, `[tl.menu]`, `[tl.lane]`, `[bin.menu]`). The limit in `src/apps/kt-app/build.rs` is 160 characters per value, and it breaks the build; t128 measures the width budget. Rule 18 and rule 7 pull in different directions, and this is not resolved once and for all — it is a bargain over every label. The industry, however, has already resolved the bargain in one direction: JetBrains writes "Prefer clear labels to short ones", Microsoft — "Use a direct object … when the object is not apparent from context". We have no such priority recorded anywhere, and therefore every dispute starts over.
- A label says "what" and "where", but not "why" and not "what you pay with". "Remove the transition with the next clip" does not say that the bodies of the events will not change in the process (item 20 of the checklist) — this is already the weight of a paragraph, and it moves into the effect window (F-HLP-006) and onto the site (F-HLP-023).
- Rule 18 conflicts with rule 17. For a label to name "where", the same verb has to be multiplied by object: the single "Delete" → "Delete track", "Delete morpher", "Delete region", "Delete camera" — four keys instead of one, that is, twenty values instead of five. The saving on reusing a key is exactly what today's ambiguity is paid for.
- Ambiguity arises RETROACTIVELY. A new entity with the same verb makes yesterday's flawless label ambiguous, without changing anything in it. The author of the new entity has no reason to reread others' labels, and without a machine check such a regression is visible to no one.
- Translation is an independent source of ambiguity, and the author of the English label does not see it and cannot see it. A flawless English set collapses in another language: we have 13 such coincidences (10 distinct pairs) in four catalogs out of 17 pairs that are already identical in English.
- An accelerator written into a translated phrase travels with it when the key is reused — and arrives where it is wrong. This is the cost not of the label as such, but of the decision to keep the chord inside the text (F-HLP-017).
- **Rewriting a label means entering a queue, not leaving one.** The tracker showed the price on a single word: Kdenlive's `Wipe` label was asked to be renamed twice and in DIFFERENT ways — #329 (2019) into "wipe/fade", #605 into "TRANSITION", both closed; in 2026 #2219 asks to split `Image Sequence` into two names, and #2128 shows a third move, when a rename already done spoils someone else's label. The dispute over a name is not resolved once, and "tidying the class up" means agreeing to pay for it again.
- **There will be no feedback on this class.** Eight tasks out of 25,442 records of eleven rating trackers (0.03 %) — which means waiting for complaints is pointless: an incorrect label leaves no trace by which a bug is filed. The person pressed, got the wrong thing, pressed undo and moved on. Hence a consequence that is unpleasant for estimating the size: without a machine predicate we will not learn either that the class exists or that it has been swept clean — "how much is still left" is not measured by anything external.

## What users say

SIGNAL: "the label is confusing" as a registered defect — 2 tasks in kde-invent, `(menu|label|button|command|option|item|wording|caption|entry|name)[^.]{0,30}(confus|misleading|unclear|ambiguous|not obvious|misnomer|misnamed)|(confus|misleading|unclear|ambiguous|misnamed)[^.]{0,30}(menu|label|button|command|option|item|wording|caption|entry|name)`; share 0.1% against 0.0% (1 task out of 6346) in kde-bugzilla

SIGNAL: "rename X to Y" as an independent request — 4 tasks in kde-invent, `renam\w*[^.]{0,40}\bto\b`; share 0.2% against 0.0% (1 task out of 6346) in kde-bugzilla and 0.4% (3 out of 680) in otio

The first count must be read as a negative result, and it matters more than any positive one: across 25,442 records of eleven trackers of the `nle` corpus this filter found **eight** tasks (0.03 %), of which two are false positives. The class exists — and is almost never registered. The reason is visible from its very structure: an incorrect label leaves no trace by which a bug is filed — the person pressed, got the wrong thing, pressed undo and moved on. So there will be no external feedback on this class from anyone, including us, and the only available watchman is a machine.

The second count explains where such tasks are nonetheless filed. Two trackers of ONE product: in the user-facing Bugzilla the request "rename X to Y" is one task out of 6346, in the developer invent it is four out of 2217. A label rename is filed not by the one who was harmed but by a maintainer; at the top of the list is otio (0.4 %, 3 out of 680), where the type name IS the API, that is, the cost of an error in a name is at its maximum.

The most precise document on the subject is our own: the claim of the owner, Round 28 item 4, from which rule 18 was born. It is valuable because the person describes not a taste but the mechanics of the error — he did not understand WHERE, and the second label turned out to be a direct lie:

> "fade out — it is not understood at all from the name of the item where it will add this out… And the label 'Remove crossfade' is also not obvious, it turns out it removes the fadeout from the end."
> — Round 28 item 4, `docs/user-feedback.md`; analysis and fix — item 171

External confirmation was found in two forms — as a vendor's admission and as a live request in a tracker. Apple is the only one of the five who wrote "the label was incorrect" in the release notes:

> "Command Editor function of 'Reset Point of View' is now listed correctly as Reset Angle."
> — Apple, Final Cut Pro release notes, section of version 10.4.1, https://support.apple.com/en-us/102825 (accessed 2026-09-07)

And here is our defect word for word, in a foreign NLE: the label does not say WHAT it will act on, because one term went to two different entities. This is one of those eight tasks that the first count gave — and its weight is appropriate: filed 21.01.2026, closed after 24 days, 3 comments and 2 votes.

> "Do we want to implement a feature that deletes all timeline markers? `Delete All Markers` only works on (clip) markers."
> — Kdenlive, issue #2128 "Marker Menu Confusing", 21.01.2026, https://invent.kde.org/multimedia/kdenlive/-/work_items/2128 (accessed 2026-09-07)

There the CAUSE is also named, useful to us as a warning: the collision appeared after the previous rename (`Guides` → `Markers`), that is, one correction of a label made someone else's label, which nobody touched, ambiguous.

The second of those eight — also Kdenlive and also about "where", and the user himself formulates the requirement of rule 18: the label must say at WHICH point the action will act.

> "for EXAMPLE the **ADD/REMOVE GUIDE** option is creating a guide where the CURSOR is ..and not where the mouse pointer is. … or 'explain' WHERE the action is supposed to be placed ( MOUSE POINTER or TIMELINE CURSOR ? )"
> — Kdenlive, issue #1666 "right mouse button menus on timeline are 'confusional'", 10.04.2023, https://invent.kde.org/multimedia/kdenlive/-/work_items/1666 (accessed 2026-09-08)

The third — the half "what", of a third editor: the label names a quantity, not what it affects, and the person goes through four labels until he finds the fifth.
> "…I'm messing with `Duration`, `End`, `Start`, `Time`, only to eventually
> discover that Position was the property I needed to adjust to change the start
> time!"
> — OpenShot, issue #1339 "Confusing Properties Labels", 05.03.2018,
> https://github.com/OpenShot/openshot-qt/issues/1339 (accessed 2026-09-08)

What could not be found: complaints from users of the paid five. I searched for forum complaints about ambiguous labels and documented renames — `helpx.adobe.com` does not respond at all (timeout and HTTP 000), `forum.blackmagicdesign.com` returns a Cloudflare challenge for the search URL, `vegascreativesoftware.com/version-history` returns a JS shell without text, and `reddit.com` and `web.archive.org` do not open from this environment (`_editors.md`, "What was unavailable"). A side negative fact worth recording: in the last 100 Shotcut releases (GitHub Releases API, up to v26.9.6) there is not a single line with the words `renam`/`now called` — a rename of a label almost never appears in the changelog of video editors and lives in the tracker as a complaint, not in the release notes as a correction.

## How it fits Kinotrain

ADDRESS: `src/apps/kt-app/src/timeline.rs` — the class is possible, and nothing catches it: one key `tl.menu.delete` = "Delete (Del)" stands on three DIFFERENT commands — `delete_selected()`, `Command::RemoveCamera`, `Command::RemoveArea` (three calls `ui.button(tr(k::tl::menu::DELETE))`, today at lines 4761, 4985 and 5045; the numbers in the text below are from the previous revision), whereas all the machine distinguishability of labels in the program is three `assert_ne!` in `src/apps/kt-app/src/i18n_tests.rs` (t171) over 8 values out of 522. The tracker adds a mechanism of appearance to this: in Kdenlive the collision #2128 was born not from a bad label but from a RENAME (`Guides` → `Markers`), after which `Delete All Markers`, flawless yesterday, became ambiguous — that is, a regression of this kind comes from editing the neighboring line, and only a predicate over the finished catalog can catch it, not the author's review.

### What already exists

**The rule.** Item 18 of the checklist in `src/apps/kt-app/CLAUDE.md`: "The name of a menu item says WHAT and WHERE will happen. 'Fade out' does not do: a fade can be at the beginning or at the end of an event. 'Remove crossfade', which in fact removes the fade from the end, lies to the user."

**Analysis of the defect.** Item 171: one label `remove_transition` stood on FOUR different actions; it was split by the pair (fade/crossfade) × (our end/our start) into `remove_fade_start/_end` and `remove_crossfade_next/_prev`, and `fade_in/fade_out` received "at the start/at the end of the event". In the catalog this today is `tl.menu.remove_fade_start` = "Remove the fade at the clip start" and `tl.menu.remove_transition_next` = "Remove the transition with the next clip" — note, the keys themselves are not named as item 171 promises (`remove_crossfade_*`): even the record of the fix diverged from the code in its naming.

**The check.** t171 `src/apps/kt-app/src/i18n_tests.rs:278` (`t171_transition_menu_labels_say_what_and_where`) takes a group of four removal keys and requires PAIRWISE distinguishability of all six pairs, then separately `FADE_IN` ≠ `FADE_OUT` and `MARKER_HERE` ≠ `MARKER_REGION`. The key point is the outer loop `for lang in Lang::ALL`: the check runs in EVERY language, and the motive is written in the test itself — "a translation 'into one word' will bring back the same ambiguity". This is the precedent on which the whole second wave rests: **a text rule that became a machine check** — the same thing that ended Round 38 ("prose drops nothing", t252–t255).

**The house style that has already formed and is recorded nowhere.** In the catalog `src/apps/kt-app/i18n/en.toml`, 17 of the 89 menu labels carry a parenthesis, and in ten of the seventeen the parenthesis holds a CONSEQUENCE, not an accelerator: `bake_batch` = "Bake the FX batch (write effects into clips)", `reset_gain` = "Reset volume (0 dB)", `bin.menu.group_one` = "Group (lock this card)". The same device in windows: `fx.disable` = "Disable the effect (bypass)", `fx.cancel` = "Cancel (revert)". The second device is a counter in the label: `tl.lane.delete_n` = "Delete track ({0} events)", `delete_morph_n`, `delete_camera_n` — the label names the size of the consequence. The third is a label that changes with context: `marker_here` ↔ `marker_region` (item 170) and the menu title `area_title` = "Area: {0}", which gives "where" to the neighboring items (`area_from_region` = "Set range from the region" is read unambiguously only under it). The `[tl.lane]` section is a model through and through: `Delete track`, `Rename track…`, `Insert track above`.

### What is missing

Rule 18 was applied POINTWISE — to those four labels that were complained about — and the class has not been swept clean. Four groups of findings, all checked by grep over `src/apps/kt-app`.

**(1) One label for different actions — the same defect as item 171.**

- `tl.menu.delete` = "Delete (Del)" stands on THREE calls in `timeline.rs`: 4352 — delete this event (`delete_selected`), 4576 — delete the camera (`RemoveCamera`), 4636 — delete the area (`RemoveArea`). Plus the same text under another key `tb.delete` — on the toolbar button (`toolbar.rs:582`) and `menu.edit.delete` = "Delete" in the Edit menu (`toolbar.rs:297`); both call `delete_action`.
- And the accelerator in this label lies TWICE, in different ways. About the area this has already been found by F-HLP-016: `Del` does not delete the area, because in `KtApp::delete_action` (`app.rs:2582`) there is no area branch. Something of its own, new: for an EVENT, `Del` does something other than the item with the same label, and this is written in a comment right under it — the item calls `delete_selected()` with the note "exactly THIS clip (+group), not the contents of the region (item 10)", whereas `delete_action`, when a region is drawn, goes to `delete_region`. That is, the label "Delete (Del)" promises a key about which the neighboring line of code explains why it does something different (rule 17).
- `render.cancel` = "Cancel" on two buttons with incomparable cost (`render_dialog.rs:416` and `:531`, analyzed in "The trick").
- `tl.menu.rename` = "Rename…" on the area (`timeline.rs:4591`) and on the marker (`:4697`); a third object — a bin card/group — in the same line, but under its own key `bin.menu.rename` (`bin_panel.rs:528`). None of the three names what is being renamed, although for the track this is done correctly: `tl.lane.rename` = "Rename track…".

**(2) Ambiguity created by translation is not caught by anyone.** Today the catalogs contain 13 coincidences of values (10 distinct pairs) that are NOT present in English: `fx.ease` ("Ease") = `perf.smoothing` ("Smoothing") in ru/es/ar; `trn.cat.wipe` ("Wipes") = `trn.kind.wipe` ("Wipe") in zh-Hans and ar — the family header and the first item under it in one combo box; `tb.zoom_in` ("Zoom in") = `win.maximize` ("Maximize") in ar — two buttons in one top strip, whose status-bar hint becomes one word (`toolbar.rs:205` and `:601`); `mrph.on_areas_n` ("Areas: {0}") = `tl.menu.area_title` ("Area: {0}") in zh-Hans; `menu.file.quit` ("Quit") = `mrph.fade_out` ("out") in ar. This is exactly what t171 protects against — but t171 knows about eight keys.

**(3) Labels that say neither "what" nor "where".** `mrph.fade_in` = "in" and `mrph.fade_out` = "out" in the morpher window (`morpher_editor.rs:445-446`) — exactly the word from the claim of Round 28 item 4, which survived to today in another window: in the timeline menu it was fixed, in the morpher window it was not. Further: `perf.log_clear` = "Clear" (clear what?), `tl.menu.area_color` = "Colour", `fx.remove_slot` = "Delete slot", `fx.done` = "Done" next to the honest `fx.cancel` = "Cancel (revert)" — of the pair of buttons only one names the consequence, although "Done" here means "close, the edits are already applied". In all there are 20 one-word menu labels out of 89, and only three of the twenty are legitimate: these are the titles of the menus themselves (File, Edit, View).
**(4) The existing tests of this class do not see it, and it is visible why.** t253 (`reach_tests.rs:422`) asks "does the entity have an item with such a label" and itself enumerates `tr(k::tl::menu::DELETE)` for the event (`:617`), the area (`:630`) and the camera (`:644`) — that is, the collision is written into the test's table and the test is green. This is not a defect of t253: reachability and distinguishability are different properties, and the first is honestly checked. t128 (`layout_tests.rs:1899`) measures length, that is, it presses in the other direction, and measures the three menu titles and eight lines of the performance panel — not a single context menu.

### What gets in the way

Not the volume, but three constructive obstacles.

The first: **"the same label" is not a defect in itself**. Rule 17 REQUIRES the same label for the same action: `tb.region_clear` and `tl.menu.region_clear` must match, because it is one command by two paths. A naive check "all values are unique" would fail on 17 legitimate pairs in English alone. So the predicate is not "the strings differ" but "one string → one action", and it requires knowing the ACTION, not only the text.

The second: **the link "label → action" does not exist for us as data**. A label reaches the screen as a literal at the call site (`ui.button(tr(k::tl::menu::DELETE))`), and there is nowhere to ask "which other commands carry this label" — this is exactly the command registry owned by F-UI-002. **What this note adds BEYOND the registry:** the registry answers the question "which commands exist and which key invokes them", whereas here a different predicate is needed — "do two DIFFERENT commands carry one label, and did two different labels collapse into one in one of the five translations". The registry makes this possible, but does not itself ask this question.

The third: **half of the check does not wait for the registry**. Item (2) — the collapse in translation — is a predicate over finished catalogs: the set of pairs that must differ is given as a list, and the check runs over `tables::TABLES` without a harness and without UI code, exactly as t171 does today. The cheap half separates cleanly from the expensive half.

The fourth obstacle, which must be named honestly: **the fix costs length**. "Delete area 'intro'" instead of "Delete (Del)" — that is plus a dozen characters × 5 languages in places where the menu width is already set by the longest line (rule 13, t128). The ladder of lengthening is known and was not invented by us — Microsoft formulates it as `Add` → `Add Items` → `Add Selected Items`, adding a word exactly up to the moment when the object stops being non-obvious. Part of the fixes, however, is cheaper to make not by lengthening the label but by the menu title, as already done for the area (`area_title` = "Area: {0}"): "where" goes into the header and is not paid for by each item separately.

### Which invariants it touches

Invariants 1–12 of the root `CLAUDE.md` — **none**: the text is static, not a single new repaint, not a single allocation in the draw path, does not touch the project format. This is a strong argument for "cheap". Checklist rules of `src/apps/kt-app/CLAUDE.md`: 18 (directly), 17 (paired ways — an accelerator in the label must be correct for ITS entity), 7 and 13 (the label must fit, and space is not wasted) — and it is precisely 7/13 that set the ceiling for the fix.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of the round: a test plus correction of labels in five catalogs |
| Where the main work is | `src/apps/kt-app/src/i18n_tests.rs` (the check), `src/apps/kt-app/i18n/*.toml` (labels), point-wise `timeline.rs`/`render_dialog.rs`/`morpher_editor.rs` (splitting reused keys) |
| Project format | does not touch it |
| Tests that will have to be written | `i18n_tests::one_label_is_one_action` — two halves: (a) the declared groups "these labels must differ" are checked in all five languages (a generalization of t171 from 8 keys to a class), (b) a value standing on two DIFFERENT commands is an error; the second half awaits the command registry |
| Depends on | F-UI-002 (the registry — for the half "label → action"), F-HLP-016 (right click), F-HLP-017 (a chord inside the label), F-HLP-033 (the cost of five catalogs) |
| Breaks compatibility | no |

A frame, not a specification: which exact groups to declare and which labels to rewrite is a decision of the round, not of the notes.

## Verdict

We take it, and we take it first of the cheap ones: this is the only position of the domain where the surface already exists, requires no code, costs one line per language — and its verifiability is proven by our own t171. The note does not propose a new surface: it proposes to sweep up the class for which the rule and the check precedent are already written, and to stop fixing it one complaint at a time.

TEST: i18n_tests::one_label_is_one_action — today it fails three times, and all three failures are visible by one reading of the files, without running the program. (1) The key `tl.menu.delete` = "Delete (Del)" stands on three different commands (`timeline.rs:4352` — `delete_selected`, `:4576` — `RemoveCamera`, `:4636` — `RemoveArea`), and `render.cancel` = "Cancel" stands on "close the settings window" (`render_dialog.rs:416`) and "interrupt the render in progress" (`render_dialog.rs:531`). (2) In the translations there are 13 coincidences of values (10 pairs) that are absent in English, and the worst of them is `trn.cat.wipe` = `trn.kind.wipe` in zh-Hans and ar: the family header and the first item under it in one combo box for choosing the transition kind (`transition_editor.rs:164`, `:168`). (3) All of today's machine distinguishability — three `assert_ne!` in `i18n_tests.rs` (292, 297, 304), all inside t171, over 8 values out of 522.

ONE LINE: overlap — not touched | delay — not touched | capacity — zero new characters in the status-bar line, the surface is not it at all: the load falls on the label ITSELF of the menu item, button and combo-box header — on a place already occupied by text, where only the text changes.

## Sources

- `docs/user-feedback.md`, Round 28 item 4 — the primary source of the rule (accessed 2026-09-07)
- item 171 — analysis of the defect and the fix (2026-09-07)
- `src/apps/kt-app/src/i18n_tests.rs:270-311` — t171 (2026-09-07)
- `src/apps/kt-app/CLAUDE.md`, checklist items 7, 13, 17, 18 (2026-09-07)
- https://support.apple.com/en-us/102825 — Final Cut Pro release notes, the rename `Reset Point of View` → `Reset Angle` in 10.4.1 (accessed 2026-09-07)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/2128 — Kdenlive, "Marker Menu Confusing" (accessed 2026-09-07)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/2219 — Kdenlive, "Rename 'Image Sequence' to 'Image Slideshow'": one term for two different import operations (accessed 2026-09-07)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1666 — Kdenlive, "right mouse button menus on timeline are 'confusional'": the user requires that the label say "where" (accessed 2026-09-08)
- https://github.com/OpenShot/openshot-qt/issues/1339 — OpenShot, "Confusing Properties Labels": the label names a quantity, not what it affects (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/329 and https://invent.kde.org/multimedia/kdenlive/-/work_items/605 — two mutually incompatible requests to rename one and the same label `Wipe` (accessed 2026-09-08)
- Trackers (third wave, R.41): dumps from `~/.cache/kinotrain/trackers/{kde-invent,kde-bugzilla,pitivi,openshot}.jsonl`; both counts are reproduced by `python3 src/scripts/tracker-rank.py --query …` with the filters from the `SIGNAL:` lines (2026-09-08)
- https://code.visualstudio.com/api/ux-guidelines/command-palette — VS Code UX Guidelines: "Use clear names for commands", "Group commands together in the same category" (accessed 2026-09-07)
- https://plugins.jetbrains.com/docs/intellij/button.html — IntelliJ Platform UI Guidelines, the Button section: "Prefer clear labels to short ones" (accessed 2026-09-07; the former address `jetbrains.github.io/ui/` returns 404)
- https://developer.apple.com/design/human-interface-guidelines/alerts — Apple HIG on `OK` (accessed 2026-09-07)
- https://developer.gnome.org/hig/patterns/feedback/dialogs.html — GNOME HIG on an imperative verb in a button (accessed 2026-09-07)
- https://learn.microsoft.com/en-us/windows/win32/uxguide/ctrl-command-buttons — Windows UX Guide: "Use a direct object … when the object is not apparent from context", the steps `Add`/`Add Items`/`Add Selected Items` (accessed 2026-09-07; Microsoft itself marks the page as outdated since Windows 7)
- https://www.gnu.org/software/gettext/manual/html_node/Contexts.html — gettext, `msgctxt` and the example with two `Open` (accessed 2026-09-07)
- https://doc.qt.io/qt-6/i18n-source-translation.html — Qt, the disambiguation argument in `tr()` (accessed 2026-09-07)
- https://mozilla-l10n.github.io/documentation/localization/dev_best_practices.html — Mozilla l10n: do not reuse a string between a menu item and a screen title (accessed 2026-09-07)
- The strings of Premiere, After Effects, Resolve and VEGAS — from secondary sources: `helpx.adobe.com` does not respond, the Blackmagic what's-new contains no renames, the VEGAS version-history is served without text (`docs/features/_editors.md`, "What was unavailable"); not checked live
- Programs outside the core (VS Code, IntelliJ IDEA) and the quotes from Apple / GNOME / Microsoft / gettext were not checked live — only public documentation, as `_editors.md` provides

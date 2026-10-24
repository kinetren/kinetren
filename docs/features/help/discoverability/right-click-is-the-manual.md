---
id: F-HLP-016
title: Context menu as a reference: right click shows what is possible
status: candidate
priority: B
cost: S
category: help/discoverability
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app]
invariants: []
ui_rules: [9, 16, 17, 18]
round: null
point: null
tests: []
depends_on: [F-UI-002, F-HLP-017]
checked: 2026-09-07
---

# F-HLP-016 — Context menu as a reference: right click shows what is possible

## Essence

The editor right-clicks on what they are looking at and gets a list of verbs for THIS object: what can be done with it now, what is already turned on, what cannot be done at the moment. They are not obliged to know the name of the command, to remember the key, or even to suspect that the operation exists — the answer arrives where the pointer already is. The hint answers "what is this", the menu answers "what can be done here": these are different questions, and the second is not closed by any status line.

## Who has it and how

| Editor | Version | How it is named | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | the object's context menu + right-drag drop menu | its own menu for the event, the track, the scale, the window; part of the capabilities lives ONLY behind the right button — dragging from Explorer/Trimmer with the right button drops onto the timeline a MENU of insert variants ("Video Only ▸ Add Video Across Time"); such a choice cannot be obtained with the left button |
| Adobe Premiere | 26 | the context menu of the clip/track/panel | present for the clip, the track header, the panels and the project items; the composition depends on what was clicked. The structure and display of accelerators could not be checked — `helpx.adobe.com` does not open from this environment, neither directly nor through the text proxy **[NOT VERIFIED]** |
| DaVinci Resolve | 21 | Contextual Menus (a separate section of the manual) | "Nearly every panel on every page exposes additional functionality via contextual menus"; the composition is DIFFERENT for the panel background and for an object on it; there is a documented exception where the right button opens no menu at all — on a control point of the curve on the Color page it DELETES that point (checked against the 20.x manual, section "Using the DaVinci Resolve User Interface"; for 21 the section was not re-checked) |
| Final Cut Pro | 12.3 | shortcut menu (also called contextual menu) | in the manual's glossary this is a separate term, and "Control-click …, then choose …" is the main documented path: 118 mentions in the manual. A menu can also be the END OF A GESTURE: a clip is dropped onto a clip and released — and the menu "Replace / Replace from Start / Replace from End" pops up. Through the shortcut menu even an event on the timeline is renamed ("Control-click the clip and choose Rename Clip") |
| After Effects | 26 | the context menu of the layer/composition/timeline | a menu is present for the layer, the track, the project panel; the composition depends on the selection. The composition and display of accelerators could not be checked for the same reason as for Premiere **[NOT VERIFIED]** |

**Outside the core.** This feature cannot be described without Blender: only there is the context menu carried through to the end — it does not merely list verbs, but leads to the HELP on that very element and shows the key taken from the keymap, not typed by hand.

| Program | Version | How it is named | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Context menu (RMB) + "Online Manual" | a button and ANY parameter have their own menu; in it — "Online Manual" (and the same command on F1), which opens the manual page about exactly the element under the cursor. The manual honestly warns that the coverage is not 100 %, and then the program says so in the info header. About menu items it is said in plain text: "All menu entries show any relevant shortcut keys" — the chord is not written into the label, it is derived from the keymap |
| Visual Studio Code | 1.136 | context menu contribution points | menu items are extension data with visibility conditions; its own guide requires "Show actions when contextually appropriate" and forbids "Show actions for every file without context", that is, the menu must change depending on what was clicked |

## The trick behind it

The menu carries what one line cannot hold by its nature: a **LIST of verbs with their state**. A line is one sentence about one subject; a menu is N lines, and the very length of the list is the answer to "what can be done here", while the state of the items (a checkmark, a counter in the label, a grayed item, a submenu) answers "and in what condition it is now". For us this weight already lies here and should not move anywhere: in `src/apps/kt-app/src/timeline.rs` the timeline menu has 33 buttons and 2 submenus, and its composition branches on six conditions (the kind of event, the presence of a link group, neighbors flush against it, transitions that already exist, free edges, the contents of the buffer). No other surface of the wave expands capacity without taking up a single pixel at rest.

Three details that are not in the lists of features:

1. **In Blender the right click is an entry into the MANUAL, not only into verbs.** "Context menu ‣ Online Manual" opens the page about the element under the cursor, that is, the menu works as a pointer to help: the user does not need to know WHAT the thing they want to read about is called — hovering is enough. There too lies the only honest behavior for the case of a gap: if the button has no link, the program says so instead of opening a random page. Exactly this is what distinguishes the menu from the table of contents: the table of contents requires a name, the menu requires only a pointer.

2. **A menu can be the END OF A GESTURE, not an event of the right button.** In Final Cut a clip is dropped onto a clip and released — and at the point of release the menu "Replace / Replace from Start / Replace from End" appears; in VEGAS the same device is done with the right button when moving: a right-drag from the timeline or from Explorer drops a menu of insert variants. This is a push at exactly the moment when the user HAS a question (the gesture is ambiguous), and at exactly the point where they are looking anyway. Note the difference from a modal dialog: the menu does not ask for consent, it lists the outcomes.

3. **Different menus for the background and for an object is a separate promise, not a detail.** Resolve documents it word for word, and it means that "right-click on empty space" is a legitimate and meaningful branch, not a stub. The reverse side is there too: the only place where the right button, instead of a menu, immediately DELETES (the curve point) — and this is exactly the button with which the user tries an unfamiliar object. One such item devalues the promise "a right click can be used to try things safely" across the whole program.

## Pros

- It answers a question that cannot be asked by hovering: to hover, one must already know where to look. The menu lists the verbs of the object, whose existence the user did not suspect — this is the only push channel we already have in the code.
- It covers nothing and delays nothing: the surface appears on an explicit gesture and disappears on the same gesture. The dispute of R.40 about pop-up hints does not touch it by a single line.
- The capacity grows with the object, not with the screen: an event has a dozen and a half items plus a submenu of 16 transition kinds, a marker has three, and at rest both cost zero pixels.
- It teaches not only verbs but also that the SELECTION matters: a menu that changes depending on what was clicked (Resolve formulates this as a rule, VS Code as a requirement for extensions) shows the difference between "object" and "background" without a single word of explanation.
- It is cheap to localize: the labels are already translated and already counted in the catalog budget (the price of a line — F-HLP-033); no new surfaces need to be created.

## Cons

- It teaches only what was put into it: a gesture (trim, edge drag, scrub) never gets into the menu, and "how to change the length of a morph event" cannot be learned with the right click — this is the claim of item 9 of R.38, and its menu does not close it.
- It requires guessing to press the right button. For our owner this is not a problem (he pressed it first), but the channel remains conditionally push: it fires on the user's initiative, it merely does not require knowing the NAME of what one is looking for.
- An item available ONLY from the menu is a double-edged stick. In VEGAS a whole capability lives this way (the "video only" insert by dragging), and it cannot be found without a hint; below, in "How it fits", this is analyzed as rule 17 (paired ways).
- A grayed item violates the letter of rule 16 of the checklist ("impossibility of an action is an ERROR, not a faded hint"). Resolution of the dispute: rule 16 is about a GESTURE, where the user is already doing something and must receive the reason for refusal; in the menu a grayed item is the only way to show that the verb EXISTS for the object. But then it must name the condition, otherwise it turns into exactly that faded hint.
- A menu whose composition depends on six conditions cannot be checked by eye: each branch is a separate scene. Without a test this is one more piece of prose.

## What users say

The owner, R.38 item 1, `docs/user-feedback.md` — the mechanism "there are no dead ends" began with this phrase:

> "I don't understand how to delete a saved area region, I saved it and now it hangs there, I don't understand how to rename it — RMB gives nothing"
> — user-feedback.md, round R.38, 2026-09-07

An experienced VEGAS user looks for a capability that lives only behind the right button, scans the visible buttons, and comes to a forum:

> "I believe that command was right next to the black arrow against the blue rectangle. I went to the visible buttons options, but it's not there either."
> — lenslens, Boris FX forum (VEGAS Pro), 05.05.2026, https://forum.borisfx.com/t/is-there-a-command-prompt-that-has-gone-missing-paste-from-trimmer-without-audio/23190

The answer in the same thread — and it is also the answer to what happens when a capability is available only with the right button:

> "Instead of dragging with the left mouse button, right-click and drag from the Trimmer window to your timeline. When you release the mouse, a menu will pop up allowing you to select Add Video Only."
> — andyrpsmith, same thread, 05.05.2026

The same question was asked of the same editor fifteen years earlier and got the same answer ("right-drag … then select Video Only>Add Video Across Time") — Mark Krueger, Creative COW, 28.11.2011, https://creativecow.net/forums/thread/adding-video-to-time-line-from-trimmer-without-aud/
The mechanism is not broken: it simply does not find itself.

Negative feedback of exactly our class — the operation exists, but not where one clicks for it; and next to it an admission of what an unfound panel costs:

> "in the Dope Sheet you can copy & paste, you have to either use keyboard Ctrl+C or Ctrl+V, or use the options in the Edit tab dropdown at the top. A right click mouse option would be nice?"
> — Gid.Joiner, Boris FX forum, 29.01.2023, https://forum.borisfx.com/t/copy-track-keyframes/15755

> "wow I've been using Mocha for 4 years and I've literally never seen or used the Dope Sheet."
> — Triple9Nickel, same thread, 29.01.2023

## How it fits Kinotrain

### What already exists

Twelve context menus in two files — `src/apps/kt-app/src/timeline.rs` (the target is chosen on `secondary_clicked`, the menu is drawn by one `context_menu` with a `match` over `CtxTarget`) and `src/apps/kt-app/src/bin_panel.rs`. An inventory by the code, not by memory:

| Entity | RMB | Items | Where |
|---|---|---|---|
| Event (clip) | yes | Copy/Paste/Delete/Split, Effects…, Region by event, Reverse (with a checkmark), Unlink/Link A/V, Reset volume (audio only), transition with a neighbor (submenu of 16 kinds by category), fade at a free edge, transition properties, four EXACT removals ("remove the fade at the end" ≠ "remove the transition with the next one"), add a morpher layer, save the area | `CtxTarget::Clip` |
| Marker | yes | Rename…, Clear range (only for a range marker), Delete marker | `CtxTarget::Marker` |
| Named area | yes | title "Area: {name}", Rename…, Set from region (grayed without a region), Tracks from the last frame (only if a frame existed), Colour ▸ palette, Delete | `CtxTarget::Area`, item 256 |
| Morpher block | yes | Bake batch/morph (the label depends on the content), Delete morpher, Morpher window… | `CtxTarget::Morpher` |
| Camera | yes | Properties…, Delete | `CtxTarget::Camera` |
| Region edge | yes | Marker here / Marker by region (the label depends on position), Clear region | `CtxTarget::RegionEdge` |
| Track (video/audio) | yes | title "Track V1", Delete track (N events) (grayed on the last one), Rename…, Reset height (only if set), Insert above/below | `CtxTarget::Lane`, gutter |
| Morph track | yes | Add morpher here, Reset height, Rename…, Delete (N morphers), Insert below | `CtxTarget::MorphLane`, item 257 |
| Camera track | yes | Add camera at cursor, Reset height, Rename…, Delete (N cameras), Insert below | `CtxTarget::CameraLane` |
| Empty space of the timeline | yes | Paste (if the buffer is not empty), with a region: Delete in region, Cut the interior, Save area, Clear region; with an empty buffer and no region — a faded line "Select a region or an event"; always: four "Add track" — and the new one is placed BELOW the clicked row | `CtxTarget::Empty` |
| Bin card | yes | Place at cursor, Group selected (N)/Ungroup/Lock, Remove from project | `bin_panel.rs` |
| Bin group frame | yes | Rename…, Lock/Unlock, Ungroup | `bin_panel.rs`; the hint here openly advertises the menu ("right click — rename/lock/ungroup"), as do the eight zones of the timeline: the right button is named in 9 values of `en.toml` |

A guard for this set already exists — **test 253** (`reach_tests.rs`, `t253_every_entity_has_its_verbs`, item 253). It was set up in R.38 precisely because of the quote above, and it requires three things from each entity of `TlEntity::ALL`: a right click on it gives ITS target (`CtxTarget::entity()`), the mandatory verbs appear in the opened menu (Create/Rename/Delete — the INCREMENT of occurrences of the label is counted, because the same inscription lives in the toolbar), and the hint above it does not say "empty space". The absence of a verb is accepted only as `Reach::Absent(reason)`, "it is elsewhere" as `Reach::Elsewhere(reference to a test)`, and the length of the explanation is checked. The gates stand in the PRODUCT code: the array `TlEntity::ALL` of fixed length and the exhaustive `match` in `CtxTarget::entity()` break the build if an entity is added and it is not said which verbs control it.
The test has already caught something the user did not name: there was no "Rename" item either on the morph track or on the camera track, although a double click on the header renamed a row of any kind (a violation of rule 17) — and a hint that lies "empty space" above the morph and camera blocks.

Two dimensions, checked against the catalogs and the code. **The chord in a menu item**: of the 38 values of `[tl.menu]`, 7 carry it — and carry it INSIDE the translated phrase ("Copy (Ctrl+C)" in the Russian catalog, in Spanish "Eliminar (Supr)" (Delete (Supr))), whereas the main menu shows the same chord in a SECOND COLUMN via `\t` from a literal in `toolbar.rs` (`Toolbar::menu_btn`). The track labels (`[tl.lane]`, 19 values) and the bin (`[bin.menu]`, 9) have no chords at all. The dispute about the source of the chord belongs to F-HLP-017 and F-UI-002; our note adds BEYOND the command registry exactly one thing — that the chord must be correct for the entity in whose menu it is printed. **Items available only from the menu**: we have many (the area colour, "tracks from the last frame", "bake morpher", "remove the fade at the end") — and this is the norm while the object has a menu; it becomes an anti-pattern exactly when the object has no menu and there are no other paths either, as was the case with the area before R.38.

### What is missing

**A known defect of the same class has been checked: it is closed.** The hint "empty space" above a real morph/camera block (a finding of the reconnaissance of R.38) was fixed in commit `14535ee`, items 258–261: in `timeline.rs` the `None` branch of `clip_at` now parses `morph_edge_at` / `area_at` / `morpher_at` / `camera_at` and only then says `EMPTY`, and t253 guards this claim for each entity. But the class is alive, and the reconnaissance of R.40 found TWO fresh instances — both where the object has no push channel:

- **The hint promises a gesture that does not exist.** Above the volume rail of an audio event the line says "Clip volume: drag, double-click — 0 dB" (`i18n/ru.toml`, `[tl.hint] volume`), but a double click in this zone goes to the common event branch of `timeline.rs` and opens the FX window: there is NO separate branch `ClipZone::Level` in the double-click handler. The reset to 0 dB exists — but as a menu item ("Reset volume (0 dB)"), that is, the push channel tells the truth, while the pull channel lies. This is the same defect as "empty space", with the same cause: the hint was corrected separately from the gesture, and nothing links them.
- **The menu promises a key that this entity does not have.** The item for deleting an area takes the common label `tl.menu.delete` — "Delete (Del)" — but `Del` does not delete the area: `KtApp::delete_action` (`src/apps/kt-app/src/app.rs`) parses the morpher, the camera, the region and the selection, and there is no area branch there, because selection of areas does not exist in the program. The accelerator landed on a foreign object together with the reused label — a direct consequence of the chord being written INSIDE the translated phrase. Who fixes this: F-HLP-017 (a single place of truth for the chord) plus F-UI-002 (the command registry); our note adds BEYOND the registry exactly one thing — the requirement that a menu item promise only what is true for ITS entity, and check it by a test rather than by eye.

Entities that are drawn have their own hint and their own grab zones, but have no menu of their own at all (the right click gives someone else's menu or stays silent):

- **Transition and fade.** `transition_at` gives three zones (the left edge, the right edge, the joint) and its own hint, but there is no `CtxTarget` for the transition: the right click on it falls through to `clip_at` and opens the EVENT menu. The verbs are there (properties, the four exact removals), but the title is about the clip, and there is no transition in `TlEntity`, therefore t253 never asked whether its set is complete.
- **The keyframe and the automation curve** (`automation.rs`): the right click does nothing; a keyframe is deleted only by a double click, and the keyframe has no hint either (the only hint of the widget hangs on its ruler, `fx.graph_ruler_hint`). The keyframe is mute in BOTH channels at once: hover — silent, press the right button — silent. Here we are exactly in the position of Mocha in the quote above ("A right click mouse option would be nice?"), while Resolve at this same point takes another path — it deletes the curve point with the right button without a menu. One has to choose between these two consciously.
- **The effect slot in the chain strip** (`widgets.rs`): there is a hint, there is no menu; the only way to remove a slot is the cross, and to copy/paste the chain — the buttons in the window.
- **Empty space of the media bin**: an empty bin says "empty — drop files on the window", and the right click answers with nothing; "Add media…" is not in the menu.
- **Preview and meters**: the right click is silent (the double click — detach and 0 dB).

Inside one program the user reads this unambiguously: where the menu is empty or missing, the object is considered lifeless. That is exactly what happened with the area — it was drawn, but the right click was silent, and the conclusion was drawn not as "I did not find the command" but as "this thing cannot be removed".

What is missing beyond this: the menu item does not say what else does the same (rule 18 "a menu item says WHAT and WHERE" is fulfilled partially: "Region by this event (Shift+double click)" is a model, "Effects…" is silent about the fact that a double click gives the same), and a grayed item does not name the condition ("Set from region" grays out without a region and does not say why).

### What gets in the way

Not the volume, but two constructive obstacles.

The first: **the menu target and the hit test are the same code, and it is already ordered**. The chain on `secondary_clicked` in `timeline.rs` parses the label → morpher → camera → region edge → event → area, and the order in it is meaningful (service blocks are checked BEFORE the region edge, otherwise a block whose edge fell onto the region line cannot be grabbed at all — item 256). Inserting a transition into this chain is not "one more variant" but a decision about priority: the edge of a transition lies INSIDE the body of the event, and which of them wins the dispute over a pixel must be decided consciously.

The second: **automation, slots and the bin have no notion of a "menu target"** — there is no `CtxTarget` there, nor entities in `TlEntity`, and therefore t253 does not guard them. The mechanism of R.38 itself will have to be extended, not only the UI: either `TlEntity` grows and stops being "a timeline entity", or a second table of verbs for the panels appears alongside it. This is a choice of the test's architecture, not a line of code.

A third, smaller thing: `ui.close()` and a repeated right click. Test 253 has already stumbled on the fact that a right click while the menu is open CLOSES it, rather than opening its own — when the set is extended, this trap will repeat in the live scenarios (`scripts/live/deadend.py`, item 264, where `Live.right_click` appeared specifically).

### Which invariants it touches

The invariants of the root `CLAUDE.md` — none: the menu touches neither the time model, nor the audio stream, nor the cache, nor the project format; the commands all already exist (their absence is caught by t252 "there are no orphaned commands"). The single touch on invariant 12 — the labels are assembled while the menu is OPEN, that is, by an explicit gesture, without animations and timers; in the closed state this is zero work and zero pixels.

Checklist rules of `src/apps/kt-app/CLAUDE.md`: **9** (nothing superfluous — the menu must not turn into a dump: an event already has 15 items and a submenu), **16** (a grayed item versus "impossibility = error": the distinction is described in "Cons"), **17** (paired ways are identical — if a verb is in the menu, its gesture must behave the same, and vice versa: a gesture promised in the hint does not exist without an implementation), **18** (an item says WHAT and WHERE).

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of the round |
| Where the main work is | kt-app: `timeline.rs` (the menu target for the transition), `automation.rs`, `widgets.rs`, `bin_panel.rs`; catalogs `i18n/*.toml` |
| Project format | does not touch it |
| Tests that will have to be written | `reach_tests::right_click_is_never_silent`; extension of the verb table of t253 to the new entities; a check that the label of an item does not promise a key invalid for its entity |
| Depends on | F-UI-002 (the command registry — the source of the chord), F-HLP-017 (a single place of truth for showing the chord), F-HLP-033 (the price of a catalog line) |
| Breaks compatibility | no |

A frame, not a specification: which exact entities get a menu and in what order they stand in the hit test is a decision of the round.

## Verdict

We take it. This is the cheapest second surface of the whole wave: a push channel that already works for us in twelve places, covers nothing, delays nothing, and takes up not a single pixel at rest — extending it is cheaper than setting up any new surface. The work is not in "adding a menu", but in ensuring that for every object which responds to hovering, the right button responds too — and responds truthfully.

TEST: reach_tests::right_click_is_never_silent — a right click in a live panel must either open a menu whose TARGET is the object under the cursor, or stand in the table of reasons as `Reach::Absent` in t253. Today it fails at least four times in `src/apps/kt-app/src`: on the automation keyframe (`automation.rs`), on the chain slot (`widgets.rs`) and on the empty space of the bin (`bin_panel.rs`) nothing opens, and on the transition (`timeline.rs`, `transition_at`) the FOREIGN event menu opens — the transition has no variant of `CtxTarget`, therefore t253 does not even ask about its verbs. Next to it lies the second half of the promise, which the test will catch in the same pass: the label "Delete (Del)" in the area menu names a key that the area does not have (`KtApp::delete_action`, `app.rs`).

ONE LINE: overlap — not touched | delay — not touched | capacity — the status-bar line does not grow by a single character, it grows in the already existing context menu: for the objects where there is none, a list of their verbs appears, and each item must be correct precisely for its own entity.

## Sources

- Blender 5.2 LTS Manual, Help System (Context menu ‣ Online Manual, F1):
  https://docs.blender.org/manual/en/latest/getting_started/help.html
  (accessed 2026-09-07)
- Blender 5.2 LTS Manual, Menus ("All menu entries show any relevant shortcut keys"): https://docs.blender.org/manual/en/latest/interface/controls/buttons/menus.html
  (2026-09-07)
- DaVinci Resolve Reference Manual (20.x), chapter "Using the DaVinci Resolve User Interface", sections "Contextual Menus" and "Right Button":
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf
  (2026-09-07)
- Final Cut Pro User Guide for Mac (12.3), glossary entry "shortcut menu", the section on replacing a clip by dragging, "Control-click the clip and choose Rename Clip":
  https://help.apple.com/pdf/final-cut-pro/en_US/final-cut-pro-user-guide.pdf
  (2026-09-07)
- VS Code UX Guidelines, Context Menus:
  https://code.visualstudio.com/api/ux-guidelines/context-menus (2026-09-07)
- Boris FX forum (VEGAS Pro), thread "Is there a command prompt that has gone missing? Paste from trimmer without audio", 05.05.2026:
  https://forum.borisfx.com/t/is-there-a-command-prompt-that-has-gone-missing-paste-from-trimmer-without-audio/23190
  (2026-09-07)
- Creative COW, VEGAS Pro forum, the same question from 28.11.2011:
  https://creativecow.net/forums/thread/adding-video-to-time-line-from-trimmer-without-aud/
  (2026-09-07)
- Boris FX forum, thread "Copy track keyframes":
  https://forum.borisfx.com/t/copy-track-keyframes/15755 (2026-09-07)
- Not checked live in any editor: the Premiere and After Effects rows rely on the generally known structure of their menus and are marked **[NOT VERIFIED]** — `helpx.adobe.com` does not open from this environment, neither directly nor through the text proxy `r.jina.ai` (an HTTP/2 error on every attempt), and the session's `WebSearch` budget was exhausted BEFORE this note was started (200 of 200), so the search went only through direct requests via the proxy and via `search.yahoo.com`. The VEGAS/Resolve/FCP rows rely on the official manuals and forum threads listed above; for Blender and VS Code a live check was not assumed (the decision of `_editors.md`, section "Software outside the core").
- Our code and history: `src/apps/kt-app/src/{timeline.rs,bin_panel.rs,automation.rs,widgets.rs,reach_tests.rs}`, items 252–264, Rounds R.38 and R.40 of `docs/user-feedback.md`, commit `14535ee`.

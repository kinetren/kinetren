---
id: F-HLP-009
title: "Where to change it back later" — a badge naming the place to return
status: candidate
priority: B
cost: S
category: help/errors
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, jetbrains-2026.2, blender-5.2, photoshop-27, ableton-live-12]
we_have: no
crates: [kt-app, kt-model]
invariants: []
ui_rules: [13, 17, 18, 19]
round: null
point: null
tests: []
depends_on: [F-HLP-010, F-HLP-033]
trackers: [openshot, kde-bugzilla, kde-invent, losslesscut]
checked: 2026-09-08
---

# F-HLP-009 — "Where to change it back later" — a badge naming the place to return

## Essence

The program asks the user once (or itself makes the decision on his behalf) — and in the same screen says in which menu or setting this is switched back. Not "how to undo the last action" (that is undo), but "where lives the toggle that I have just clicked without looking". The strong form of the mechanism is not a phrase with a path but a BUTTON that opens the needed place; then the pointer cannot go stale when a menu item is renamed.

The owner formulated the requirement in capitals and in one breath with the option itself (R.28 item 9, `docs/user-feedback.md:477-480`):

> "I am interested in a project option that would allow several events of one track to overlap at the same time… Make such an option in the context menu and enable it by default… ALSO, IN THE BANNER, MANDATORILY SAY WHERE IT CAN BE SWITCHED BACK. And oblige the agents too to THINK about the user in ui/ux."

The requirement is recorded as rule 19 of the checklist (`src/apps/kt-app/CLAUDE.md:274`) and has not been met even once — neither in that option, for whose sake it was written, nor anywhere else.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none; suppression lives in Preferences ▸ General | Checkboxes "do not show again" in dialogs officially do not exist at all: the questions are switched on and off in advance ("Prompt to adjust project settings to match first media added to timeline", "Provide a warning if a track being deleted holds at least one event"). The reset is page by page, "Default All". The dialog does not mention Preferences |
| Adobe Premiere | 26.x | none; by a checkbox on the dialog, on DIFFERENT tabs | The "Reset All Warning Dialogs" button does NOT exist in Premiere (it is in Photoshop/InDesign/Lightroom, it did not move to the video applications). The return is by a separate checkbox: "Show Clip Mismatch Warning dialog" and the button "Reset Fit Clip dialog" lie in Timeline, "Show Project Load Error dialog" — in General. The dialog itself ("This clip does not match the sequence's settings…" + "Always ask") does not name a place |
| DaVinci Resolve | 21 | none as a class | In the Reference Manual (July 2026, 4443 pages) there are **zero** occurrences of "Do not show" / "Don't show" / "Never ask": BMD does not document the pattern at all. There are only "Reset System Preferences" / "Reset User Preferences" from the Option menu. The one-off "Change Project Frame Rate?" is irreversible altogether: "Once clips have been imported into the Media Pool, the frame rate cannot be changed again, so choose carefully" — and this is said by the manual, not by the dialog |
| Final Cut Pro | 12.3 | **Reset All** (Settings ▸ General ▸ Dialog warnings) | The only one of the five with a genuine general reset: "Click this button to reset all warning dialogs that you dismissed by selecting the "Do not remind me again" checkbox". But there is no back reference: in the full User Guide 12.3 the phrase "Do not remind me again" occurs EXACTLY ONCE — in the description of the button itself; which dialogs carry it, Apple does not list anywhere |
| After Effects | 26.x | none; nearest is Reset Coach Marks | In AE 26 the General panel has been abolished altogether (Preferences reorganised into 16 panels), there is no "Reset All Warning Dialogs". The suppressions live in Preferences one by one ("Do not warn me about missing fonts when opening projects"). The only documented return of what has already been shown: "Reset Coach Marks: Resets the list of coach marks that have been seen, showing them again" |

**Verdict on the core: zero out of five.** None of the five editors has a single dialog that itself names the place of its cancellation, and not one button-transition from the dialog to the settings. The phrases "You can change this behavior in Preferences ▸ …" in Resolve and FCP occur by the dozen — but only in the MANUAL, never in the interface. This changes the status of the owner's requirement: it is not catching up.

The mechanism in this domain lives outside video editing, and by the rule of `_editors.md` ("the 'trick behind it' is taken from this program") the second table here is mandatory.

**Outside the core.** Without these five programs the feature cannot be described: only they have the mechanism existing not as a phrase in a dialog but as an addressable object — a command, a URI, a row of a generated table — and only there does one see what it costs.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Visual Studio Code | 1.136 | Command links in notifications; `@id:` / `@modified`; Configure Trusted Domains | A notification carries a markdown link `[text](command:<id>)` — since 1.34; arguments travel as URI-encoded JSON. The setting is addressable: `vscode://settings/<settingName>`, the cog next to a setting can "copy the settings URL". The `@modified` filter is a separate surface "what did I change at all" |
| IntelliJ IDEA | 2026.2 | Notification actions; Don't show again + `Appearance \| Behavior \| Notifications` | A balloon carries actions (`NotificationAction.createSimple`); opening the settings — `ShowSettingsUtil.showSettingsDialog(project, Configurable)`. What has been muted returns through the table "Don't ask again notifications" with a Remove button. Settings are searched from Search Everywhere (`Shift Shift`, then `/`) |
| Blender | 5.2 LTS | RMB ▸ Online Manual (F1); Allow Online Access | RMB on ANY property gives an exact link to the manual: the RNA path resolves through the generated table `scripts/modules/_rna_manual_reference.py`. The extensions dialog says "You can adjust this later from "System" preferences" AND gives a button that calls `screen.userpref_show(section='SYSTEM')` |
| Adobe Photoshop | 27.x | Reset All Warning Dialogs | A button in Preferences ▸ General returns ALL suppressed warnings at once. None of the Photoshop dialogs names its place in the settings itself — users learn the path on forums |
| Ableton Live | 12.4.5 | none | The Info View describes the element UNDER THE CURSOR, not the place of the setting; no dialogs naming where to cancel a decision were found. Preferences were renamed to Settings, and the manual does not describe the search field |

Separately — platform prescriptions, because they contradict each other and this changes the formulation:

- **Microsoft (Win32 UX Guide, Dialog Boxes)** — the only found wording of the rule AS A RULE: "If users may need to restore these dialog boxes, provide a **Restore messages** command in the program's Options dialog box."
- **Apple HIG, Settings** — "consider providing a button that opens it directly from your interface"; `openSettingsURLString` is documented as "deep linking to settings". The scheme `x-apple.systempreferences:` is officially NOT supported (the answer of Apple DTS on the developer forum).
- **Material 3** — against: "Providing a third action, such as Learn more, is not recommended as it navigates the user away from the dialog."
- **Android** — a direct ban for one particular case: "Don't link to system settings in an effort to convince the user to change their decision."

No established NAME for the pattern exists in ui-patterns.com, NN/g, or Material/Carbon/Polaris. The nearest is "deep linking to settings", and that is an API term, not a UX one.

## The trick behind it

**What the mechanism carries that one status-bar line cannot hold:** a FOREIGN ADDRESS. Not text, but a link to another interface element — one that must remain correct when that element is renamed, moved or translated. In 160 characters a path fits in letters (ours is 36–54 characters); what does NOT fit there is its TRUTHFULNESS. For us the cargo goes into a button: the address is stored by type, the caption is assembled from the same catalogue keys that draw the menu itself, and no printed copy of the path is created in the catalogue at all.

Three details that are absent from every feature list.

1. **VS Code made the address of a setting a first-class object — and let it be copied.** The cog next to any setting offers "copy the setting ID, copy a JSON name-value pair, or **copy the settings URL**"; the URL has the form `vscode://settings/workbench.colorTheme`, and from a notification the same point opens through the link `[text](command:workbench.action.openSettings?["@id:…"])`. The point is not convenience: while the address is a string in prose, it cannot be copied or checked; once it became a URI, the dialog can HOLD the address without pronouncing it. Exactly for this reason VS Code also has the reverse surface `@modified` — "what did I change", with the direct justification in the docs: "if you have forgotten whether you configured a setting, or if the editor is not behaving as you expect because you accidentally configured a setting".

2. **Blender checks the consistency of text and place by a MACHINE, not by a translator.** RMB on any property ▸ Online Manual (F1) leads not to a search but to an exact anchor: the RNA path of the button is resolved through the generated table `scripts/modules/_rna_manual_reference.py` (`("bpy.types.preferencesview.show_tooltips_python*", "editors/preferences/interface.html#…")`). That is, every setting has a machine-addressable address of documentation, and a divergence "the option was renamed — the link lied" is caught by building the table, not by the reader. At the same time Blender itself has almost no dialogs by its ideology ("Blender doesn't pop up requesters that require the user to fill in data before things execute", "Modal dialogs are not great design and should be avoided") — and the pattern appears exactly where the decision cannot be rolled back through undo: the extensions dialog says "You can adjust this later from "System" preferences" AND gives the button `screen.userpref_show(section='SYSTEM')`.

3. **JetBrains keeps the address of the return page — and does not require a button there; both versions lie in one resource file.** The Javadoc of `Notification.configureDoNotAskOption(id, displayName)` describes the second parameter as "tile for UI in *Preferences | Appearance & Behavior | Notifications*": the platform KNOWS where the user will come back to undo, and stores a human name for it. But the contract of the button does not require it, and in `ApplicationBundle.properties` the pattern and the anti-pattern sit side by side: the indent detector gives a balloon with the actions `Undo` and **`Code Style Settings`** (a link to the exact page), while next to it — `notification.content.available=<b>{0} Language is available</b><br>It can be changed later in Settings`, that is, "somewhere in Settings", without a page and without a button. One product, one file, two disciplines.

And one more thing that is visible only when you search for feedback: **there are no praising quotes about the deep link.** People write to the network when they have LOST something, not when a button worked. Therefore the payoff of this mechanism cannot be measured by enthusiasm — only by the absence of threads "how to get it back".

## Pros

- The decision stops being a trap: "answered once and got stuck" is the formulation of rule 19 itself, and it describes the mechanics, not a mood. A dialog that names the place turns a choice that looks irreversible into a reversible one.
- A button instead of a path removes the coupling of the text to the location: rename the menu item — the caption moves with it, because it IS the caption of the item. A prose path would have to be corrected in five files and in five languages.
- The mechanism gives a side surface "what have I changed at all" (`@modified` in VS Code, the table "Don't ask again notifications" in JetBrains). It answers the question the user asks more often than he remembers a specific setting: "the program behaves not as it should, and I do not know what I pressed".
- The requirement is machine-checkable if the address is a type rather than a string. This is exactly what R.38 used to close the failure of the checklist: a rule that can be broken by a test stops being prose.
- Cheap where a place already exists: in Blender the extensions dialog calls `screen.userpref_show(section=…)` — one existing operation.

## Cons

- **The pointer lives longer than the place it points to.** As soon as the path is printed in prose, any rearrangement of the menu makes the text a lie — and a silent lie: no test can tell a correct path from a stale one.
- **The price multiplies by languages.** The path is translated itself: `View → Economy UI` / `视图 → 省电界面` / `عرض → واجهة موفرة`. One decision — five texts, each of which must be edited in sync with the menu.
- **A button leads away from the dialog.** The objection of Material 3 ("navigates the user away from the dialog") is real: a third action in a modal window breaks the choice for which the window was opened. Therefore a button is legitimate not everywhere, but where leaving is not a pity.
- **The deep link tempts to persuade.** The Android ban ("don't link to system settings in an effort to convince the user to change their decision") is about the fact that the mechanism "where to get it back" easily degenerates into "change your mind".
- **Discipline does not hold by itself.** The best example is inside Microsoft: the Win32 guide requires "Restore messages", while the VS Code guide for extension authors in the section "✔️ Do" prescribes "Add a **Do not show again** option for every notification" and says nothing about the way back.
- **The means of return gets lost itself.** The Photoshop button "Reset All Warning Dialogs" exists, but the current Adobe page on Preferences (July 2026) does not mention it at all — users find it on forums. FCP has the same problem head-on: a general "Reset All" exists, but the phrase "Do not remind me again" occurs across the whole manual exactly once — in the description of the button itself. A place of return without a back reference is half a mechanism, and it works only for those who already know what they are looking for.

  **The dating makes this argument decisive.** The section "Resetting all warning dialogs" has been in the Adobe Photoshop 6.0 manual (2000, p. 97) — that is, Adobe's reset button has existed for a quarter of a century, and throughout this time not one Photoshop dialog has mentioned it. So the problem is not the absence of a return mechanism: the mechanism exists and lives for decades. The problem is exactly the absence of a **pointer to it at the point of decision**, and that is the formulation of rule 19. Twenty-five years is enough time to consider this not a shortcoming of a particular vendor but a persistent failure of the genre. (The exact wording of Adobe remains unconfirmed: `helpx.adobe.com` returns 403/timeout, `web.archive.org` is blocked, ManualsLib is behind a CAPTCHA — the label and the path are confirmed by three secondary sources, there is no verbatim quote from the primary source. `[NOT VERIFIED verbatim]`)
- **Pointing in the documentation does not replace pointing in the interface.** Resolve and FCP name the address of a setting in the manual dozens of times ("You can change this behavior in Preferences ▸ User ▸ UI Settings…") — and zero times in the dialogs themselves. The user reads the dialog, not the chapter about it.
- **A second promise in the window is a second point of silent failure.** The tracker gives a case that was absent from this note: in Kdenlive the suppression checkbox itself broke (kde-invent #36, "'Do not ask again' doesn't work"), and the user noticed it, not the build. A dialog that today promises one thing ("I will not ask again") after rule 19 promises two, and the second is someone else's address. A button leading to the wrong place does not break the build or an existing test; it breaks trust in the whole window, and restoring that costs more than one search for a setting.
- **The exit and the way back are needed by different people, and the first gets in the way of the second.** The count from "What users say" separates them directly: at losslesscut the requests "remove the question altogether" are three times more numerous than "how to get it back"; at openshot it is exactly the reverse. The pointer to the place of return serves the newcomer and is paid for with a line in the window of the one who asked for the opposite — so that this window would not exist at all. Rule 19 adds text exactly where the second half of the audience already demands silence.

## What users say

SIGNAL: "turned it off myself — and ask strangers where to get it back" — 5 tasks in openshot, `(can.?t|cannot|unable|do ?n.?t know how|how (do i|to)|no way)[^.]{0,35}(get|bring|restore|re.?enable|turn|switch)[^.]{0,22}(it|them|this|that|the \w+)? ?back`; share 0.1% against 0.0% (none of the 6346) in kde-bugzilla

SIGNAL: "remove this question forever" — 3 tasks in losslesscut, `(disable|remove|get rid of|hide|suppress|stop|turn off|avoid|skip|do ?n.?t (show|ask))[^.]{0,30}(warning|confirmation|dialog|prompt|popup|pop.?up|message ?box|notification|nag)`; share 0.2% against 0.0% (3 of 6346) in kde-bugzilla

The two counts look at one mechanism from different ends, and inside their own trackers they swap places. At losslesscut there are three requests "remove the question" and one complaint "bring back what I removed"; at openshot it is exactly the reverse: one request to remove against five "how to get it back". This is not a difference between products but a difference in the people who arrive: whoever already knows the program demands a switch, while whoever is learning it looks for the way back. The mechanism of rule 19 serves the second at the expense of the first, and both sides are visible in the count.

A third count explains why the first two are small. The filter `(do ?n.?t|never|not) (show|ask|display|remind)[^.]{0,12}again` gives **three tasks across 25,442 records of eleven trackers of the `nle` corpus**, and of these three one is a request to ADD the checkbox (openshot #5489, 03.04.2024), and the second is a complaint that it does not work (kde-invent #36). That is, open-source editing tools have almost no suppressible dialog as a class, and this confirms the conclusion of the section "Who has it and how" from the other side: not "the mechanism exists, and no one talks about it", but "the mechanism does not exist at all".

All the live quotes found are about the same thing: a person pressed "do not show again" and lost a behaviour, and learned the way back from a stranger. From video editing there are three, and the first describes the mechanics more precisely than any guideline.

> «After realizing that the sequences were not matching the clips anymore I
> wanted to bring back the feature of the "Clip Mismatch Warning" dialog box
> but after unchecking "Always ask" I was unable to find a way to bring back
> ONLY that feature.»
> — Ant Gonzalez, CreativeCOW (Premiere), 20.08.2014,
> https://creativecow.net/forums/thread/how-do-you-reset-the-always-ask-checkbox-in-premie/
> The same person a year later in the same thread: «The only solution I found was to trash my
> preferences file and restart.» The right answer («In PPro Preferences ->
> General, there is a check box…») came from another user nine months later — and since then the setting has moved to the Timeline tab, so the answer from the thread is ALREADY outdated.

> «I accidentaly click on "always you this choice" when this "fit clip", "the
> source is longer than the destination" window appeared»
> — John28850210ubid, Adobe Community (Premiere), 22.03.2023,
> https://community.adobe.com/t5/premiere-pro-discussions/how-to-reset-the-warning-dialogs-to-their-default-settings/td-p/13670817

> «the window pops up that say: Change Project Frame Rate? and i want the
> project to match the video I am bring in what do I click? Don't Change or
> Change? I just want a simple answer, please.»
> — martinbrossman, Blackmagic Forum (Resolve), 15.01.2024,
> https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=195099

The same plot outside editing, where it is documented best:

> «I accidentally hit the 'Do Not Show Again' dialog box when deleting a layer
> mask. Now whenever I try to apply a layer mask, instead of asking if I want
> to delete or apply, it just automatically deletes it.»
> — Linda1234567, Adobe Community (Photoshop), 05.01.2018,
> https://community.adobe.com/t5/photoshop-ecosystem-discussions/un-checking-the-quot-do-not-show-again-quot-dialog-box/m-p/9613714
> (the answer in the same thread: «Go to Photoshop>Preferences. In the general tab,
> click 'reset all warning dialogs'.»)

> «I accidentally selected do not show this message again under Data Merge.
> Now when there is an issue with Data Merge I can't tell what it is because
> that message no longer appears.»
> — Martina_Doodles1, Adobe Community (InDesign), 16.09.2019,
> https://community.adobe.com/t5/indesign-discussions/how-do-i-undo-quot-do-not-show-this-message-again-quot/td-p/10610170

> «It use to pop up and I mistakenly clicked the "Don't show again" button. I
> want to turn it back on because I've realised how easy it is to make a
> mistake.»
> — Microsoft Q&A, 18.02.2016,
> https://learn.microsoft.com/en-us/answers/questions/2711827/how-to-turn-back-on-the-confirmation-dialogue-box

A voice from the side of a designer, formulating it as a defect rather than as an inconvenience:

> «If a user selects this option the element is removed forever so the user has
> essentially deleted a portion of the UI that they can never get back with
> just one click.»
> — GitLab UX issue #213344,
> https://gitlab.com/gitlab-org/gitlab/-/issues/213344

Trackers that were unavailable when the note was first written gave the same story, now not in proprietary software but in open-source software, and in the purest form: one click removed part of the interface, nobody named the place to return to, and the person goes off to ask outsiders.

> «So.. I pressed on one of those toolbar buttons where you see an icon of
> fullscreen and delete and i accidentaly pressed delete. now i can't get it
> back.»
> — issue #615, OpenShot, 02.05.2017,
> https://github.com/OpenShot/openshot-qt/issues/615

> «So i was on open shot and i pressed the x on the window of filters/effects.
> Anyone know how to get it back?»
> — issue #1825, OpenShot, 10.07.2018,
> https://github.com/OpenShot/openshot-qt/issues/1825

Both issues are closed with the state `completed` — that is, it was possible to restore both the panel and the window. So a place to return to existed in the program, and the price of its namelessness is measured exactly: an opened task in someone else's bug tracker instead of one line in the interface. The same story as Photoshop with its twenty-five-year-old "Reset All Warning Dialogs" button, but recorded by number, date and state, rather than by memory.

And the reverse side: a suppression mechanism about which the interface says nothing breaks silently — the first to notice is not a test but the user.

> «If you press "do not ask again" for a warning message it doesn't work.»
> — issue #36, Kdenlive (invent.kde.org), 12.02.2019,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/36

No quotes PRAISING the deep-link button turned up in the trackers either — just as they could not be found earlier in search («took me straight to the setting», «nice touch» next to the VS Code notification, «instead of hunting through menus» + permission settings). The cause appears to be substantive and is already named in "The trick behind it": nobody writes about a button that worked.

## How it fits Kinotrain

ADDRESS: src/libs/kt-model/src/project.rs:3306 — the class "pressed once and could not find where to restore it" is POSSIBLE with us, and in a worse form than with others: `overlap_layers: true` is the only one-time decision of the project, and there is nothing at all to switch it with. The only assignment outside tests is the read from a file, `src/libs/kt-otio/src/convert.rs:1026`; there is no `Command` for it in `src/libs/kt-model/src/model.rs`, no menu item, no banner, and the flag is set by hand only in the tests of five files. In OpenShot from issue #615 it was possible to restore the panel, and the question closed with one answer; with us there is nothing and nowhere to restore it, and not one test sees this.

### What already exists

- **The rule is written down and not carried out.** `src/apps/kt-app/CLAUDE.md:274-276`:
  "The dialog must say where the decision can be changed later. Every banner with a choice ends with a phrase saying in which menu/setting it is switched — otherwise the user answered once and got stuck."
- **Proof of non-compliance — by grep, not on trust.** The corpus:
  `src/apps/kt-app/i18n/{en,es,ru,zh-Hans,ar}.toml`, 522 values in
  each, **2610 lines** (counted by `^([A-Za-z_0-9]+)\s*=\s*"(.*)"$`). Three
  questions to the corpus:
  1. *Does a value name the path in a menu?* A regex over the menu names of all five
     languages (`File|Edit|View|文件|编辑|视图|Archivo|Editar|Ver|
     ملف|تحرير|عرض`) with a mandatory separator `/ → > »`. The result is **one key**,
     `msg.ktproj_import_only` ("The old .ktproj opens only
     via File / Import .ktproj…"), and it is about rule 18 (WHERE to FIND
     the action), not about 19 (WHERE to RESTORE the decision). The separator in the regex
     is mandatory: without it the Arabic `عرض` ("display") and `ملف` ("file") give three
     false hits in `cam.axis_fit_hint`, `hint.idle`,
     `msg.media_missing`.
  2. *Can the catalog say at all how to restore?* It can: **30 values**
     carry an accelerator in brackets (`play_pause`, `loop`, `undo`, `split`, …) —
     the `[tb]` section even declares this as a rule in a comment. The catalog says
     HOW; it never says WHERE.
  3. *Are there any words about restoring?* Four keys:
     `hint.preview_detached` ("close it to restore"),
     `tl.lane.row_height` ("RMB — reset"), `mrph.revert_hint`,
     `fx.cancel` ("Cancel (restore as it was)"). The first two name a GESTURE,
     the last two — a rollback inside the window. None names a place.

  Result: rule 19 has been carried out **0 times out of 2610**.
- **We have already solved exactly this task once — and solved it by name, not by path.** The language submenu (`src/apps/kt-app/src/toolbar.rs:322-338`) draws endonyms, and the comment in the code explains why: "someone who chose Chinese has nothing to look for in the Russian list". t125 keeps the self-names identical in all five catalogs. This is our own precedent against the printed path.
- **A ready second surface.** The notifications window
  (`src/apps/kt-app/src/app.rs:1103-1127`) is a place the user comes to deliberately; it neither covers nor delays anything. `struct Notice {
  at, severity, text }` (`app.rs:22`) is plain text — there is no field for an address.
- **The substitution machinery is ready and covered**: `tf!`/`interp`
  (`src/apps/kt-app/src/i18n/mod.rs:148`) with positional `{0}`; t125 checks the placeholder sets against English.
- **A precedent for machine-checking a checklist rule against the catalog**: t171
  (`src/apps/kt-app/src/i18n_tests.rs:278`) turned rule 18 into a test across all five languages. The harness can walk the menus:
  `h.get_by_label(tr(key)).click()` (`layout_tests.rs:4688`) and `right_click`
  (`reach_tests.rs:395`).

### What is missing

A list of decisions that the user makes (or receives) once, with their
real place of switching — ready material for a future text:

| Decision | Where it is made | Where it is switched back | Lives in |
|---|---|---|---|
| Clip overlap layering (`Project.overlap_layers`) | **nowhere** | **nowhere** | in the file |
| Language | `toolbar.rs:322` View ▸ Language | same place — but the menu is already in the selected language | Storage `lang` |
| Economy mode | `toolbar.rs:318` View ▸ Economy **and** `app.rs:3763` performance panel | two places, neither knows about the other | session |
| Preview resolution share | `app.rs:3676`, the "1/4" combo under the preview | same place | session |
| Preview detach | double-click on the preview (`app.rs:3355`) **and** View ▸ Detach | `hint.preview_detached` names the GESTURE, not the place | session |
| Snap | Ctrl during a drag (`timeline.rs:1996`) | there is no persistent setting at all | one frame |
| Loop | toolbar button / `L`; switched OFF by `clear_region` ITSELF (`app.rs:2305`) | the button; auto-off is not mentioned anywhere | project |
| Video FX backend | by the program: `Poisoned` → CPU (`app.rs:772`, `msg.video_fx_gpu_lost`) | nothing but a restart; `KT_VIDEO_GPU=0` only DISABLES the GPU | session |
| Autosave | not asked (`msg.autosave_restored`) | no setting | — |
| `KT_METER_DEMO` | environment variable (`meters.rs:352`) | the same | process |

**The crux — `overlap_layers`.** The only assignment outside tests is
`src/libs/kt-otio/src/convert.rs:1026` (`p.overlap_layers = !l.off;`), that
is, a READ from the file. There is no command (`grep -rn "OverlapLayers\|SetOverlap"
src/libs/kt-model/src/` — empty), no context menu item, no badge, and the
default is `true` (`src/libs/kt-model/src/project.rs:3156`). Consequently the
"no overlap layering" mode is a whole branch of behavior (`ops.rs:705` demotion
on import, `plan_move`/`restore_handles`, crossfade from an overlap,
`src/libs/kt-engine/src/audio.rs:283` overlap weight) — **unreachable from the
program at all**; nine tests in four files set the flag by hand, that is, they
check code that the user cannot reach.

From the decision of R.28 item 1 ("a project option, switched from the context
menu; on the first overlap a badge explains the consequences and says where to
switch it back") the model, the format and the mix were made — and neither the
menu, nor the badge, nor the indication of the place were made. The requirement
that gave rise to rule 19 was not fulfilled in its own round.

Separately: we have no settings AS A PLACE. In eframe, exactly two keys are
written to Storage — `lang` and `last_project` (`src/apps/kt-app/src/app.rs:5182-5185`);
everything else from the table lives for the session. There are also no
choice dialogs: ten windows (`morpher_editor.rs:273`, `fx_editor.rs:164`,
`render_dialog.rs:275` and `:439`, `camera_editor.rs:75`,
`transition_editor.rs:141`, `app.rs:1105`, `:1347`, `:2356`, `:5069`) are
editors, lists and one name dialog; there is not a single "yes/no" confirmation
in the program. The Ctrl+F search (`app.rs:4543`) searches
labels/media/tracks/clips/bin groups — it cannot see commands and settings
(searching by CONTENT is owned by F-MED-001).

### What gets in the way

**The typed path is more expensive for us than it seems, and in two places it
does not work at all.**

*Cost in characters.* The median catalog value is 14–17 characters (zh — 6),
the limit is `MAX_VALUE_CHARS = 160` (`src/apps/kt-app/build.rs:42`), the
maximum today is 145. The ready path "View → Economy mode" costs en 40 · es 47 ·
ru 36 · zh 21 · **ar 54** characters — a third of the budget before the phrase
itself has begun.

*Cost in coherence.* The path consists of two values that are ALREADY in the
catalog (`menu.view.title`, `menu.view.economy`). Writing it as prose, we
create their third and fourth copies in five files — and t125 will not notice
this: the key is in place, the length is within limits, the placeholder set
matches, but the pointer lies. This is literally the minus "the pointer lives
longer than the place" from the section above, confirmed by our own test: t125
checks completeness, usage and placeholders, but not the TRUTHFULNESS of the
address.

*Technical impossibility for Arabic.* The `ar` table is reordered at BUILD
time (`src/apps/kt-app/build.rs:135`, `bidi::visual`), therefore `{0}` stands
in a VISUAL position. A Latin file name is substituted there normally
(`msg.media_missing`) — it is an LTR atom; an Arabic path assembled at runtime
would arrive in logical order inside a visual string, that is, backwards.
So "let's substitute the path as an argument" is not a workaround, but a defect.

*Duplication of places.* Economy mode is switched from two points with two
different keys and the SAME text (`menu.view.economy` and `perf.economy`). A
prose path is obliged to choose one of them and thereby lie about the other; a
button simply leads to where the user is now.

*What must not be proposed.* A badge in our program is not a pop-up layer:
checklist rule 10 and decision item 14 of round 8 stand, and the note does not
touch them. The return address is proposed on surfaces that already exist: the
menu item through which the decision is made, and the notifications window,
where things arrive deliberately.

### What invariants it touches

- **Rule 19** — the subject of the note.
- **Rule 18** ("a menu item says WHAT and WHERE") — a neighbouring rule, already machine-checkable (t171); rule 19 is the same thing, only about the reverse route.
- **Rule 17** ("paired ways are identical") — the economy mode and the detached preview each switch in two ways; the indication of the place must be shared, otherwise the pairs will diverge in text while the behaviour is the same.
- **Rule 13** ("do not waste space") — a direct argument against the printed path: 40–54 characters out of 160 for what the button expresses by the label of an existing item.
- The engine invariants (1–12 of the root `CLAUDE.md`) are not touched: there is no new layer, no redrawing per frame, no work in the audio path. Invariant 12 (repaint only on movement) is not violated — the button is drawn in an already open window.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of the round |
| Where the main work is | kt-app: a type for the way-back address + a field on `Notice` + a button in the notifications window; kt-model — a `Command` for `overlap_layers`, which does not exist |
| Project format | not touched (`overlap_layers` is already in the `metadata.kt` envelope, `convert.rs:515`) |
| Tests that will have to be written | `reach_tests::every_choice_names_its_way_back` — an exhaustive `match` "decision → where it is switched" with an empty allow-list by the `NOT_FROM_UI` idiom (`reach_tests.rs:209`), plus a harness check that the named address is really clickable; an extension of t171 to the menu items that take part in addresses |
| Depends on | F-HLP-010 (the error channel and `Severity`), F-HLP-033 (the price of localisation — not to be recalculated here) |
| Breaks compatibility | no |

A frame, not a spec. The main design decision that the round must make consciously: the way-back address is a TYPE (an enumeration of places), not a catalog string; the label is assembled from the keys of the menu itself. As soon as the address becomes a string, the test will turn back into prose.

## Verdict

Rule 19 is not "almost fulfilled" and not "partly fulfilled" — it has been fulfilled zero times out of 2610 catalog lines, and it was not fulfilled in the very round that produced it: the overlap option lives in the model, in the format and in the mix, and it cannot be switched by anything except hand-editing the .otio. The cause is exactly the same one that closed R.38: prose drops nothing. t252 did not miss this out of oversight but by design — it enumerates the variants of `enum Command`, while what was orphaned is not a command but a DECISION that lives as a project field.

The reconnaissance has also changed the subject of the requirement itself. The owner asked for a phrase ("IN THE BANNER SAY WHERE"), but for everyone whose mechanism works, this is not a phrase but an addressable object: a settings URI in VS Code, a generated table of anchors in Blender, `Configurable` in JetBrains. A phrase is cheap to write and expensive to maintain — it binds the text tightly to the location of a menu item, and for us this is also five languages, a third copy of the existing values outside t125, and Arabic, where the substitution of a path is physically wrong. Therefore we take the requirement, but not its form: the place is named by the BUTTON that leads there, and no printed path is put into the catalog at all.

A third thing that should be said directly: here we are not catching up. Of the five core editors, the number of dialogs that name the place of their own cancellation is zero, the number of transition buttons to the settings is zero, and Resolve does not document the pattern at all. The owner's requirement puts us ahead of the market, not level with it, and this is a rare case where "doing it as others do" is certainly worse than doing it as he asked.

TEST: reach_tests::every_choice_names_its_way_back — today it fails on the very first line of the table: `Project.overlap_layers` has neither a `Command`, nor a menu item, nor a banner (the only assignment outside tests is `src/libs/kt-otio/src/convert.rs:1026`, a read from a file), therefore the exhaustive `match` "decision → where it is switched back" has nothing to fill it with, and the allow-list by the `NOT_FROM_UI` idiom must stay empty.

ONE LINE: overlap — not touched | latency — not touched | capacity — the status-bar line does not grow by a single character and receives no printed path at all; the way-back address travels as a TYPE on surfaces that already exist — on the menu item by which the decision is made, and on a button in the notifications window (`src/apps/kt-app/src/app.rs:1103`), where the user comes deliberately, and the button's label is taken from the same `menu.*` keys that draw the menu itself.

## Sources

Live checking is not assumed for programs outside the core (the rule of `_editors.md`); everything below is documentation, source code and forums.

- Microsoft, Win32 UX Guide, Dialog Boxes («Restore messages») —
  https://learn.microsoft.com/en-us/windows/win32/uxguide/win-dialog-box
  (accessed 2026-09-07)
- Microsoft, Launch the Windows Settings app (`ms-settings:` as a deep link) —
  https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings-app
- Apple HIG, Settings («a button that opens it directly») —
  https://developer.apple.com/design/human-interface-guidelines/settings
- Apple, `openSettingsURLString` —
  https://developer.apple.com/documentation/uikit/uiapplication/opensettingsurlstring ;
  the unofficial status of `x-apple.systempreferences:` — the answer of Apple DTS,
  https://developer.apple.com/forums/thread/761314
- Material 3, Dialogs (against a third action) —
  https://m3.material.io/components/dialogs/guidelines
- Android, Requesting permissions («don't link to system settings…») —
  https://developer.android.com/training/permissions/requesting
- NN/g, Confirmation Dialogs Can Prevent User Errors —
  https://www.nngroup.com/articles/confirmation-dialog/
- VS Code 1.34, Command links in notifications —
  https://code.visualstudio.com/updates/v1_34#_command-links-in-notifications
- VS Code, Settings (`@id:`, `@modified`, «copy the settings URL»,
  `vscode://settings/<settingName>`) —
  https://code.visualstudio.com/docs/configure/settings (checked 2026-09-07)
- VS Code, Command URIs (`command:` in markdown, `isTrusted`) —
  https://code.visualstudio.com/api/extension-guides/command (checked
  2026-09-07)
- VS Code, UX Guidelines ▸ Notifications («Add a Do not show again option for
  every notification») —
  https://code.visualstudio.com/api/ux-guidelines/notifications
- JetBrains, Balloon notifications («configure settings», «Don't show again»)
  — https://plugins.jetbrains.com/docs/intellij/balloon.html
- JetBrains, Notifications ▸ Restore hidden suggestions —
  https://www.jetbrains.com/help/idea/notifications.html
- JetBrains, Settings dialog (search field) —
  https://www.jetbrains.com/help/idea/settings-preferences-dialog.html
  (checked 2026-09-07)
- JetBrains, Search Everywhere (search over settings) —
  https://www.jetbrains.com/help/idea/searching-everywhere.html
- Blender HIG (Non Blocking, Operate → Settings, Modal Dialogs) —
  https://developer.blender.org/docs/features/interface/human_interface_guidelines/
- Blender Manual, Context Menu (Online Manual, Copy Full Data Path) —
  https://docs.blender.org/manual/en/latest/interface/controls/buttons/menus.html#context-menu
- Blender Manual, Searching Preferences —
  https://docs.blender.org/manual/en/latest/editors/preferences/introduction.html#searching-preferences
- Photoshop, Adjust preferences —
  https://helpx.adobe.com/photoshop/desktop/get-started/settings-and-preferences/adjust-preferences.html
  (Last updated Jul 21, 2026). The verbatim wording «Reset All Warning
  Dialogs» **[NOT VERIFIED against the live Adobe page]** — the page returns
  403/timeout; the label and the path are confirmed by a secondary source: HowToGeek,
  28.02.2021, https://www.howtogeek.com/712432/how-to-bring-back-photoshops-warning-dialogs/
- Ableton, Live 12 Manual, Live's Settings and Info View —
  https://www.ableton.com/en/live-manual/12/first-steps/
- User quotes — links in the section "What users say".

Core (all of it — official vendor documentation, accessed 2026-09-07):

- Premiere 26, General preferences and Timeline preferences (updated
  07.01.2026) —
  https://helpx.adobe.com/premiere/desktop/get-started/preferences-and-settings/general-preferences.html
  and
  https://helpx.adobe.com/premiere/desktop/get-started/preferences-and-settings/timeline-preferences.html
- After Effects 26, Preferences (updated 27.03.2026) —
  https://helpx.adobe.com/after-effects/using/preferences.html
- DaVinci Resolve 21 Reference Manual, July 2026, 4443 pages —
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
  (the claim "zero occurrences of „Do not show“" was obtained by grep over the entire PDF)
- Final Cut Pro, General settings ▸ Dialog warnings ▸ Reset All —
  https://support.apple.com/guide/final-cut-pro/general-settings-vera36110dbb/mac ;
  the count "„Do not remind me again“ exactly once" — grep over the full
  User Guide 12.3, https://help.apple.com/pdf/final-cut-pro/en_US/final-cut-pro-user-guide.pdf
- VEGAS Pro 2026, Preferences ▸ General —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/12-preferences/prefgeneraltab.htm

Trackers (R.41; the dumps live in `~/.cache/kinotrain/trackers/` and do not get into the repository — `_trackers.md`, section "Licenses"). It is precisely they that closed the gap because of which this note previously carried an honest marker about a quote not found:

- https://github.com/OpenShot/openshot-qt/issues/615 — «I removed some
  toolbars and can't get them back», 02.05.2017, closed/completed
  (accessed 2026-09-08)
- https://github.com/OpenShot/openshot-qt/issues/1825 — «i "x"'d out a window
  and i dont know how to get it back», 10.07.2018, closed/completed
  (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/36 — «"Do not ask
  again" doesn't work.», 12.02.2019 (accessed 2026-09-08)
- https://github.com/OpenShot/openshot-qt/issues/5489 — a request to ADD a
  checkbox «Don't show this again», 03.04.2024 (accessed 2026-09-08)
- https://github.com/mifi/lossless-cut/issues/847 — «I always want to "merge"
  how do I disable pointless dialog?», 01.09.2021 — the side of «remove the question»
  (accessed 2026-09-08)

Both `SIGNAL:` lines are reproduced by the command `python3
src/scripts/tracker-rank.py --query '<filter from the backticks>'` on the export of 08.09.2026: 12 dumps, 28 537 records (corpus `nle` — 25 442; ffmpeg-trac in its own corpus). `blender-vse` and
`ffmpeg-trac` were still downloading at the time of work (files empty) and do not take part in the percentage comparisons.

**What remains unverified** (so that the next wave does not search again):
the verbatim wording of the VEGAS dialog «match project settings to media» and
the checkbox of the VEGAS News window — only secondary sources: the old forum
`vegascreativesoftware.info` redirects to Discourse after the purchase by Boris FX, and the old threads were not archived; the Adobe System Compatibility Report dialog —
the help pages return 404; the verbatim Adobe line «Reset All Warning
Dialogs» for Photoshop — the live page returns 403/timeout, the label is confirmed by a
secondary source (HowToGeek). In addition, the `WebSearch` budget of the session was
exhausted (200 of 200) before work on the note began, `helpx.adobe.com` when accessed directly from the main session timed out after 60 s, and the text proxy
`r.jina.ai` returned HTTP 422; the Adobe data was collected by workarounds and is marked
above wherever it remained unconfirmed.

---
id: F-HLP-003
title: Instrument strip versus help strip — what others keep in the status bar
status: researched
priority: B
cost: S
category: help/hints
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, vscode-1.136, jetbrains-2026.2]
we_have: partial
crates: [kt-app]
invariants: [12]
ui_rules: [10, 13]
round: null
point: null
tests: []
depends_on: [F-HLP-001, F-HLP-002]
trackers: [kde-invent, kde-bugzilla, openshot]
checked: 2026-09-07
---

# F-HLP-003 — Instrument strip versus help strip — what others keep in the status bar

## Essence

At the bottom of the window — a strip one line high. In some programs it says what the object under the cursor is; in others it holds instruments: position, zoom, memory, cache, the progress of a background task, a notification bell; in third ones there is no strip at all. We have BOTH here: on the left the instruments (zoom, region, cache, underruns, V-fps), on the right — a hint, and the hint is drawn ON TOP, covering the instruments with its own background. The note answers the question of who else combines two purposes in one strip and what they pay for it.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Status Bar | YES: element No. 7 of the main window in the official 2026 help — and the only one of the seven whose "More Information" column has no link, that is, the vendor names the strip and does not describe it. It carries the help text of menu items (from the manuals of the 14–17 line, analysis — F-HLP-001; for 2026 [NOT CHECKED]), the Record Time indicator, an EDITABLE Selection Length field "in the status bar under the timeline", the progress of long operations. It can be closed, and this is remembered between launches |
| Adobe Premiere | 26.x | none | There is no full-width strip in the window. Warnings, errors and information events live in the Events panel; an "alert icon" appears in the window, a double-click on it opens the panel, and pop-up notifications are silenced by the setting Show Event Indicator |
| DaVinci Resolve | 21 | Status Bar (only the Fusion page) | At the bottom of the window — the Page bar, which is NAVIGATION (seven pages); RMB on it gives "Show Icons Only" so the bar takes less space. A real status line exists only on the Fusion page: on the left — information about the node under the cursor, on the RIGHT — the percentage of RAM cache occupancy, which "appears at all times", during playback — the achieved fps, plus a console message badge; in the viewer the same bar gives RGBA/Z and the pixel coordinates under the cursor [checked against the Resolve 20 manual; for 21 NOT CHECKED] |
| Final Cut Pro | 12.3 | none | There is no strip. Background tasks — a Background Tasks BUTTON in the toolbar plus a separate window (Cmd-9), where the running tasks and the percentage of readiness of each are listed |
| After Effects | 26.x | none (the Info panel) | There is no strip; its role is carried by the floating Info PANEL, and the help is forced to advise keeping it in front of its group, because otherwise it is not visible. Render progress gets there through a separate setting (Show Rendering Progress In Info Panel And Flowchart) |

Summary for the core: a strip at the bottom of the window exists for two of the five, and **combines the help with the instruments exactly one** — Resolve, and only on the Fusion page. Vegas has an instrument strip plus the help text of the menu; Premiere, FCP and AE have no bottom strip, and everything it usually carries is moved into panels and windows.

**Outside the core.** Without these three programs the inventory does not work out: three of the five NLEs have no strip at all, and "what happens in the status bar" would have to be described from two lines. It is precisely outside video editing that the strip is designed as a surface — with zones, with a choice of composition and with a click.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Status Bar | Three zones with fixed places. On the LEFT — Keymap Information: "The left side of the Status Bar displays mouse button shortcuts and the keymap of the active tool", and a tap on Alt redraws it into the hotkeys of the tools (Alt Click Tool Prompt). In the MIDDLE — the task progress (hovering gives a time estimate, there is a cancel button) and the Report Message, which on click opens entirely in the Info Editor. On the RIGHT — Resource Information: scene statistics, duration, memory, extension updates, version; the COMPOSITION OF THE RIGHT ZONE is chosen by the user via RMB on the strip. The whole strip is switched off via Window → Show Status Bar |
| Visual Studio Code | 1.136 | Status Bar | Purely instrumental and entirely clickable: "A status bar item is a status bar contribution that can show text and icons and run a command on click". An element has an `alignment` (left/right) and a `priority` ("Higher value means the item should be shown more to the left"), its own name like "Python Language Indicator" and ITS OWN tooltip — that is, the help about the strip pops up ABOVE the strip, and does not live in it. The notification bell is in the right corner of the strip, its position is configurable (bottom-right by default, top-right moves the icon to the title bar) |
| IntelliJ IDEA | 2026.2 | Status bar | A hybrid with a division by edges. On the LEFT — either the navigation line or the last event: "it shows the most recent event messages", a click opens it in Notifications, RMB gives Copy for pasting into a bug report; there also the progress of processes, a click — the Processes manager. On the RIGHT — button widgets: `52:11` (a click — go to the line and column), `LF` (a click — change the line separator), `UTF-8` (a click — change the encoding), indentation, the read-only mode; the composition is chosen via RMB on the strip. The VCS widget was REMOVED from the strip — it moved to the main toolbar at the top |

## The trick behind it

**What the mechanism carries that one hint line cannot hold: a STATE that is correct without a gesture — and the entry into this state.** A hint is transient by definition: it answers the question "what is under the cursor" and lives while the cursor is there. An instrument must be correct even when nobody is looking at it, and that is exactly why it cannot share pixels with text of variable length. For us this load is split in two: the cache, underruns and V-fps are duplicated in the perf box of the left column, while the zoom and the region boundaries exist nowhere else any more and disappear under the hint plate.

Three details that are not in any list of features.

**1. Resolve does BOTH of our disputed decisions at once — and separates them by time and by place.** The manual describes the timing literally: the name of an input or output of the node under the cursor "immediately appears in the Status bar. If you wait for a few more moments, a floating tooltip displays the same name right over the node". That is, the status line is a channel WITHOUT delay and WITHOUT overlap, the floating tooltip is a channel WITH delay and WITH overlap, and this is one product, not a dispute between two schools. At the same time the help in the strip does not eat the instruments: the node information goes on the left, the RAM cache percentage stands "on the far right edge" and is visible always. There is no competition not because the text is short, but because the zones have different edges.

**2. In Blender the help zone of the strip changes from the KEYBOARD, not from the mouse.** On the left it says what the mouse buttons do in the current tool, and a tap on Alt redraws this zone into the hotkeys of the tools. This is exactly what our line lacks and what F-HLP-005 owns: a hint that answers "what will happen if I hold this now", not "what is here". And note the asymmetry of rights: the composition of the INSTRUMENT zone the user chooses himself via RMB, but the help zone cannot be switched off — the vendor considers it part of control, not a decoration.

**3. In VS Code and IntelliJ a strip element is not a caption but a button.** "Can show text and icons and **run a command on click**"; a click on `52:11` leads to the line and column, on `LF` — it changes the line separator, on the message on the left — it opens Notifications, RMB on it — copies the text to the clipboard. There the strip works as a DOOR: the reading itself leads to where it is controlled, and therefore it needs no explanation. For us the strip has six readings and exactly one clickable item — the bell (`app.rs:3842`); the other five are drawn with `ui.weak()`, and the hint layer over them is declared `interactable(false)`.

Where the load goes for us: nowhere, it does not go anywhere. The hint plate fills the strip with the panel's colour from the right edge to the left, clamped by `strip.left()` (`app.rs:1316`), so the order in which instruments die is set not by importance but by the layout of R.7: first V-fps, then underruns, then the cache, then the region, and last — the zoom. Two readings, which across the whole `apps/` have exactly one drawing place, turned out to be the outermost on the left — that is, they survived by accident.

## Pros

- **The strip as an instrument surface does not require a gesture.** "The percentage of the RAM cache that's used appears at all times" — a quantity that cannot be missed, stands where the gaze runs into it; our `underruns` are of the same class: their meaning is that they are visible when nobody asked about them.
- **Marking by PLACE instead of priority makes the strip predictable.** In Blender and Resolve the help and the instruments do not compete for the same pixels, because each zone has its own edge; the length of the text does not change where to look.
- **The strip-door removes the question "where do you change this".** A click on an element opens exactly what it reports on (Notifications, Info Editor, Processes, an arbitrary command) — this is rule 18 of our checklist, expressed by geometry rather than text.
- **The composition of the instrument zone is assigned by the one who pays for the space** (RMB on the strip in Blender and JetBrains): a user who does not need memory takes the pixels for himself, and the vendor does not have to guess for everyone.
- **The strip is switched off entirely** (Window → Show Status Bar in Blender, View → Appearance → Status Bar in IDEA, closing the Status Bar in Vegas with the choice remembered) — the cost of one line of height is paid voluntarily.

## Cons

- **Combining two purposes without marking means displacement.** As soon as the help and the instruments share one rectangle and there is no marking, a longer text simply eats the neighbor. Ours is done as honestly as possible — `rect_filled` over it (`app.rs:1316`), and this is the only known implementation of "priority instead of zones" among the seven programs reviewed.
- **The bottom of the window is far from the place of work.** The main objection to a permanent help area (Info View, analysis in F-HLP-002) is "the eyes travel down". Industry answers to it do not enlarge the strip, but duplicate or move things: Resolve repeats the same text as a popup hint right above the node, FCP moves the progress into a toolbar button and a window, JetBrains removed the most needed widget (VCS) from the strip UP to the main toolbar.
- **The door costs code.** Every VS Code element is a contribution with a command, a name, a priority and its own tooltip; this is expensive exactly to the extent that our `ui.weak()` are cheap — and just as useful.
- **Instruments age silently.** People look at the strip rarely, so a defect in it lives for years: Vegas fixed "Record Time indicator … now properly displays very large values" as a separate item in the release notes. The same class for us: one and the same indicator is written in two places with two different English literals bypassing the catalogs — `"underruns: {}/{}"` (`app.rs:3869`) and `"Underruns {}/{}"` (`app.rs:3714`).
- **What can be closed — gets closed.** A Vegas user in August 2026 writes about closing the Status Bar as a routine matter. Everything that lived only in the strip disappears at that moment; for us in this position are the zoom and the region boundaries.
- **The price of "a single instance" is confirmed by a tracker in its pure form.** The author of kde-invent #1744 hid the strip to make room for an extra track and was forced to turn it back on — not because the strip is needed, but because one reading does not exist anywhere else. This is exactly the scenario that is unreachable for us by a switch (there is none) and reachable by the hint plate: the result for the zoom and the region boundaries is the same, only the way differs.
- **20 px is not "too little space", it is a separate class of defects.** Kdenlive has four appeals about the strip out of seven — about text that did not fit, on three platforms and over ten years (kde-bugzilla #353657, #437113, #471252, #501571; the last one has been open since 16.03.2025). We already have a discrepancy of the same class in the strip: the hint plate is built on a rectangle 22 px high, while the panel is set up with `exact_size(20.0)`.
- **The strip flickers while the mouse moves, and this is fixed as a bug.** Kdenlive closed it with the fix "Visual jitter in statusbar when move cursor over timeline" (kde-bugzilla #356661, 14.12.2015, https://bugs.kde.org/show_bug.cgi?id=356661). For us the source of the jitter is the same and stronger: the timeline gives a hint at each of its points (F-HLP-001), so the plate appears and disappears at every entry of the cursor into the timeline and back, and together with it the five instruments flicker.

## What users say

> When I close the Status Bar, it saves this change, but it doesn't save it for
> the toolbar; it always stays open by default.
> — hb2k, forum.borisfx.com, 25.08.2026,
> https://forum.borisfx.com/t/patch-vegas-pro-2026-0-3-build-189/25206

> Can properly edit the Selection Length box in the status bar below the
> timeline even with no selection range present on the timeline.
> — VegasDerek (publisher), release notes of VEGAS Pro 2026 build 66, 27.03.2026,
> https://forum.borisfx.com/t/21161

> The Record Time indicator in the status bar now properly displays very large
> values.
> — release notes of VEGAS Pro 20 Update 4, as given in "Vegas Pro Release History",
> NickHope, 22.06.2026, https://forum.borisfx.com/t/24284

A verbatim complaint about the distance to the place of work was found only outside video editing, and it is already quoted in F-HLP-002 ("The problem is the amount you have to move your attention and eyes takes you away from the very button or area you're interested in", forum.ableton.com, 25.10.2019).

The Kdenlive bug tracker is the only NLE with a public correspondence precisely about this strip, and there the dispute runs exactly between our two assignments. For the help zone — a request to make it PERMANENT, and the request is literally for Blender, which the table above analyzes:

> Other Applications like Blender, Inkscape and many more show "Keymap
> Infromation" permanently in the status bar. The information depends on the
> current selected tool.
> — kde-invent #916, 08.01.2021 (2 votes, 14 comments),
> https://invent.kde.org/multimedia/kdenlive/-/work_items/916

The same author names the price of combining in exactly our language: today Kdenlive writes this into the common `statusbarmessagelabel`, where the text is "visible for a few seconds after the tool is activated and they shared the label with other status messages" — that is, the help and the instruments share one label, and the help wins it.

Against — a request to move the reading OUT of the strip, and the motive is rare: the strip was hidden to gain an extra track, and a value living only in it forced the strip back on.

> a way to move this displayed value on other position? or maybe display the
> value as a textual popup if the status bar is hidden?
> — kde-invent #1744, 15.08.2023 ("i started to not use anymore the status bar
> to gain the space for another track… but this value make me forced to display
> it again"), https://invent.kde.org/multimedia/kdenlive/-/work_items/1744

And a third side — "the strip exists but is empty"; the wish list coincides with our composition almost line by line (the instrument keys, the parameter values, progress):

> OpenShot does have a status bar but it isn't used. It would be nice if we
> could make extensive use of the status bar.
> — openshot #1858, 16.07.2018,
> https://github.com/OpenShot/openshot-qt/issues/1858

A request to "remove" is also visible, and it concerns the help half: kde-bugzilla #496054 "Disable tutorial info in Statusbar" (10.11.2024, `wishlist`, confirmed by triage, https://bugs.kde.org/show_bug.cgi?id=496054).

SIGNAL: the bottom strip as a subject — 7 tasks in kde-bugzilla, `status ?bar|statusbar`; a share of 0.1% against 0.0% (1 task out of 4516) in openshot

SIGNAL: text that did not fit into the strip — 4 tasks in kde-bugzilla, `(status ?bar|statusbar)[^.]{0,40}(cut off|cropped|crop|truncat|spacing|too narrow|not be read|cannot be read)`; a share of 0.1% against 0.0% (not one of 2217) in kde-invent

The second count is the finding: more than half of the appeals to the strip in `kde-bugzilla` (4 of 7) are not about the content, but about the text not fitting into it. Three of the four — on three different platforms and with an interval of years (#437113, 14.05.2021, "wrong spacing (horizontally cut off)"; #471252, 20.06.2023, macOS, "strings are cropped"; #501571, 16.03.2025, Windows, "vertically cut off", not closed), the fourth — #353657 (07.10.2015, HiDPI 2880×1620, the strip's buttons too narrow). For a strip of one line the main defect is not "they look in the wrong place" but "it did not fit" — and this is exactly rule 7 of the checklist.

Complaints from editors in the words "I don't look there" / "far from the timeline" are still absent: three of the five core NLEs have no strip at all, and the remaining two have no public tracker (`_trackers.md`, "Whom we do NOT take"). The Discourse search of the Boris FX forum (`/search.json` for "status bar", "status bar tooltip", "status bar missing", "status bar font small", "bottom of the screen eyes") gave only release notes and a post by hb2k; the search of the Blackmagic forum (`search.php`) without a session returns the "Information" page.

## How it fits Kinotrain

ADDRESS: src/apps/kt-app/src/app.rs (`egui::Panel::bottom("status")` and `draw_statusbar`) — half of the class of kde-invent #1744 is impossible for us, because the strip cannot be hidden: the panel is set up unconditionally every frame, there is no switch in any of the three root menus, and there is nothing to remember. The second half is possible, and nothing catches it: `draw_status_hint` fills the strip with the panel's colour from the right edge to `strip.left()`, and the only measurement of the strip — t128 in `src/apps/kt-app/src/layout_tests.rs:1899` — runs `REGION_S` and `CACHE_SHORT` against the budget `PERF_COLUMN = 190.0`, that is, against the width of ANOTHER panel; no test asks what remained visible under the plate.

### What already exists

- The strip: `egui::Panel::bottom("status")` … `.exact_size(20.0)`
  (`src/apps/kt-app/src/app.rs:823`), the content is drawn by `draw_statusbar(ui)` (`app.rs:3800`). The order from left to right: the bell with a badge of unread items (the only clickable element, `:3842`), zoom `{:.0} px/s` (`:3851`), the region in seconds (`:3855`), the audio/video cache in MB (`:3863`), `underruns: a/b` (`:3869`), `{:.0} vfps` (`:3875`). The empty places of the strip drag the window (`ViewportCommand::StartDrag`, `:3811`).
- The hint is drawn ON TOP of the strip by the last operation of the frame: `draw_status_hint` (`app.rs:1274`), layer `Order::Foreground`, galley `layout_no_wrap`, the plate from `strip.right() - 6.0` to the left by the width of the text, the left edge clamped `.max(strip.left())`, fill `theme::PANEL` — that is, not a backing but an erasing. The comment in the code (`app.rs:1316`) word for word: "a long hint calmly erases the metrics on the left — it is more important". [A small note for the future: the plate is built from `statusbar_rect` 22 px high, while the panel is set up with `exact_size(20.0)` — there are no two pixels at the strip; an estimate from the code, not measured live.]
- Duplication: the cache and underruns are in the perf box of the left column (`draw_perf_box`, `app.rs:3697`: `k::perf::CACHE_MB` at `:3709`, `Underruns` at `:3714`), V-fps — a separate series of the quality chart (`app.rs:4837`). The perf box is always open, lives in a `ScrollArea` under the media bin (`app.rs:892`), and is never covered by the hint: it is drawn only within `statusbar_rect`.
- NOTHING ELSE is duplicated: `px/s` occurs across the whole `src/apps/kt-app/src` once (`app.rs:3851`), the key `k::perf::REGION_S` — once (`app.rs:3855`, plus a mention in a test). The render dialog uses the region, but shows not the boundaries but the length in the file name (`render_dialog.rs:165`).
- The instrument part of the strip is a direct decision of the owner: R.7 ("cleaning the status bar", "obvious labels are not to be made" — "zoom:", "FPS:", "(loop)" removed), R.7.1 item 1 (V-fps "here, not in the toolbar"), item 22 of R.7 (cache and underruns after the region). By the catalog rule the composition does not change without a new request; the PLACE and the second address — do change.
- An accidental mitigating detail: the timeline zone sets a hint only when there is NO gesture (`timeline.rs:2179`: `if self.drag.is_none() && let Some(pos) = hover`). So during the drag itself the hint goes out and the instruments return; they are erased while the user MOVES the mouse, and the timeline gives a hint at each of its points (F-HLP-001). The instruments are visible when the cursor is over the preview, outside the window or dragging; the rest of the time — no.
- Measured in the strip exactly the opposite of what is needed: t128 (`layout_tests.rs:1899`) runs `REGION_S` and `CACHE_SHORT` against the budget `PERF_COLUMN = 190.0` — the budget of ANOTHER panel, while their real competitor, the hint plate, is measured by nothing (this is the finding of F-HLP-002 and its test `hint_fits_the_status_strip`).
- t83 (`layout_tests.rs:234`) is not touched: nothing popping up is proposed here, `Order::Tooltip` does not appear, `theme.rs` is not touched, and rule 10 (exactly one hint in one place) remains the wording of our model.

### What is missing

**A guarantee that no reading lives ONLY under the plate.** Today there is no such guarantee in the code or in a test: two values out of five exist in a single instance and in the very place that gets erased. Plus there is not a single sign of the strip-door (only the bell is clickable), there is no choice of the composition by the user and there is no switch for the strip — that is, all three techniques by which other strips pay for combining are absent with us.

### What gets in the way

Rule 13: a new strip must not be set up, and that is right — a second line at the bottom would cost as much as Ableton's 5 % of the window (F-HLP-002), for the sake of two numbers. Zoning inside 20 px is not free either: to divide the strip honestly one would have to measure the width of the instrument part on every frame and truncate the text, while invariant 12 directly forbids extra allocations and layouts in draw paths — now there are two short ones, and this is already on the edge (F-HLP-001). The cheapest thing to do is not to divide the strip, but to give the two readings a second address in the already open perf box, where there is a ready two-column grid for them.

### Which invariants it touches

Invariant **12** — touched and constrained: any zoning of the strip adds work per frame, therefore the verdict chooses the variant that adds nothing to the draw path. Rules of the checklist: **13** (do not waste space — why neither a second strip nor its own block is set up), **10** (exactly one hint in one place — not touched), **18** (the item says WHAT and WHERE — what the foreign door-strip replaces the help with; with us it is not fulfilled by a single element of the strip except the bell), **7** (the text fits — touched, but it is owned by F-HLP-002).

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of a round |
| Where the main work is | kt-app: `reach_tests.rs` (a new test), with findings — `app.rs` (`draw_perf_box`: two lines into the existing `ui.columns(2)` grid) |
| Project format | not touched |
| Tests to be written | `reach_tests::no_reading_lives_only_under_the_hint` — every value drawn in `statusbar_rect` must have a second address outside the fill area of the plate |
| Depends on | F-HLP-001 (the model of the line), F-HLP-002 (the measurement of the hint width) |
| Breaks compatibility | no |

## Verdict

Combining the help and the instruments in one strip is not our invention: Resolve does it on the Fusion page and, in a soft form, Vegas. But everyone who combines separates the two purposes BY PLACE (the left edge against the right), while we separated them by priority and erase the instruments with a fill. The strip is not spoiled by this exactly as long as every erased reading exists somewhere else; today two of the five — the zoom and the region boundaries — exist only there, and they are lost precisely when the user moves the mouse, that is, almost always. Rule 13 forbids treating this with a place, therefore we treat it with an address, not with pixels.

TEST: reach_tests::no_reading_lives_only_under_the_hint — today fails on two readings out of five: "px/s" (`src/apps/kt-app/src/app.rs:3851`) and `perf.region_s` (`:3855`) are drawn across the whole `apps/` exactly once each, while the hint plate fills the strip from the right edge to `strip.left()` with the panel colour (`:1316`); the cache, underruns and V-fps the test will pass — they are duplicated by the always-open perf box (`:3709`, `:3714`).

ONE LINE: overlap — not touched | delay — not touched | capacity — not a single character is added to the status line and not a single pixel is taken from the hint; what changes is not the text but the guarantee — the zoom and the region boundaries get a second address in the ALREADY open perf box of the left column (`app.rs:3697`, the ready grid `ui.columns(2)`), after which the priority "the hint matters more" stops costing readings.

## Sources

- VEGAS Pro 2026 Online Help, "The User Interface" (Status Bar — element 7 of the main window; page changed 15.04.2026) —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/2-window/mainwindow.htm
  (accessed 07.09.2026)
- forum.borisfx.com (Discourse API `/search.json`, `/t/<id>.json`, `/posts/<id>.json`): topics 21161 (release 2026 build 66), 24284 (Release History), 25206 (patch 2026.0.3) — 07.09.2026
- Adobe, "View warnings, errors, and information events" (Premiere; page updated 07.01.2026) —
  https://helpx.adobe.com/premiere/desktop/edit-projects/correct-mistakes/view-warnings-errors-and-information-events.html
  (read in a text copy taken 07.09.2026: direct requests to `helpx.adobe.com` from the environment time out, `r.jina.ai` returns an HTTP/2 error)
- Adobe, "General user interface items" (After Effects, the advice to keep the Info panel in front of the group) —
  https://helpx.adobe.com/after-effects/desktop/get-started/get-familiar-with-the-interface/general-user-interface-items.html
  (text copy of 07.09.2026); the setting "Show Rendering Progress In Info Panel And Flowchart" — Adobe After Effects Help (PDF), section Display preferences
- DaVinci Resolve 20 Reference Manual (PDF, Blackmagic Design, 05.07.2025): "Introduction to DaVinci Resolve" — Status Bar (pp. 40–41), Minimizing the Resolve Page Bar (p. 21), "Exploring the Fusion Interface" — Status Bar (p. 1243) and RAM Cache (p. 1235), "Node Tooltips and the Status Bar" (p. 1365), "Status Bar Information" in the viewers (p. 1431), Spline Editor (p. 1479) —
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf
  (accessed 07.09.2026). For Resolve 21 it was not re-checked
- Apple, "View background tasks" (Final Cut Pro User Guide) —
  https://support.apple.com/guide/final-cut-pro/view-background-tasks-ver64e71609/mac
  (07.09.2026)
- Blender 5.2 LTS Manual, Status Bar (Keymap Information / Status Messages / Resource Information) —
  https://docs.blender.org/manual/en/latest/interface/window_system/status_bar.html
  (07.09.2026, via the text proxy `r.jina.ai`: direct access is blocked by Cloudflare)
- Visual Studio Code API, `StatusBarItem` / `StatusBarAlignment` (alignment, priority, "run a command on click", tooltip) —
  https://raw.githubusercontent.com/microsoft/vscode/main/src/vscode-dts/vscode.d.ts
  (07.09.2026); the position of the notification bell —
  https://code.visualstudio.com/docs/configure/custom-layout (07.09.2026)
- IntelliJ IDEA 2026.2 Help, "User interface" (Status bar: the left part, process progress, widgets on the right, RMB to choose the composition, the move of the VCS widget to the toolbar) —
  https://www.jetbrains.com/help/idea/guided-tour-around-the-user-interface.html
  (07.09.2026)
- Trackers (dumps in `~/.cache/kinotrain/trackers/`, not committed to the repository — `_trackers.md`, the licences section; both SIGNAL line counts are reproduced by `python3 src/scripts/tracker-rank.py --query '<filter>'` with the filters from the backquotes, accessed 08.09.2026):
  `kde-invent` — work_items 916 (08.01.2021, a permanent key zone in the strip) and 1744 (15.08.2023, move the reading out of the strip);
  `kde-bugzilla` — 496054 (10.11.2024, `wishlist`, remove the tutorial text), 356661 (14.12.2015, the strip flicker when the cursor moves over the timeline), 353657 / 437113 / 471252 / 501571 (2015–2025, text does not fit);
  `openshot` — 1858 (16.07.2018, "the strip exists but is not used").
  None of the five core editors has a public tracker (`_trackers.md`, "Whom we do NOT take — and why"), therefore the count is taken from open editors, and the table "Who has it and how" remains on the vendors' documentation
- Code: `src/apps/kt-app/src/app.rs` (`draw_statusbar`, `draw_status_hint`, `draw_perf_box`), `timeline.rs`, `layout_tests.rs` (t83, t128), `render_dialog.rs`; `docs/user-feedback.md` (R.7, R.7.1, R.9 item 1, R.40) — 07.09.2026

Not a single foreign program was checked live: Blender, VS Code and IntelliJ are software outside the core, and a live check is not assumed for them (`_editors.md`); the five core NLEs are described from official documentation and the vendors' release notes, and a mismatch between the manual's version and the version from `_editors.md` is marked in the table.

---
id: F-UI-004
title: System file dialog versus our own
status: candidate
priority: B
cost: S
category: ui/input
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: partial
crates: [kt-app]
invariants: [6, 12]
ui_rules: [2, 16, 17]
round: null
point: null
tests: []
depends_on: []
trackers: [kde-bugzilla, kde-invent, pitivi, shotcut, olive]
checked: 2026-09-08
---

# F-UI-004 — System file dialog versus our own

## Essence

When a program needs a file, it shows either the operating system's window or its
own. For the editor the difference is whether his bookmarks sit in the left column,
whether search works, whether network and cloud volumes are visible, and whether a
preview opens before the choice is made. For the program the difference goes deeper:
in a sandbox (Flatpak, Snap, Mac App Store) the system dialog is not a matter of
appearance, but the way to GET the right to a file. The fork appears twice: for
project files (open/save) and for media (add material), and almost all of them
resolve these two cases differently.

## Who has it and how

| Editor | Version | What it is called | How it is built |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Windows dialog + Explorer window | opening/saving a project and rendering — the system Windows window; for material inside the program there lives a permanent docked explorer panel with a preview |
| Adobe Premiere | 26.x | File → Import + Media Browser panel | importing a single file — a system dialog; the regular path for material is its own panel, because the system dialog cannot show the structure of camera cards (AVCHD/XDCAM) |
| DaVinci Resolve | 21 | Media Storage + own dialogs | the main surface for material is its own permanent Media Storage panel; the auxiliary open/save windows look identical on all three OSes, that is, they are drawn by the application itself (not documented by the vendor, not checked live) |
| Final Cut Pro | 12.3 | Import Media + system panel | "Import Media" is Apple's own window with skimming and camera archives; ordinary open/save is the system panel, and in the App Store sandbox it is exactly that which grants the application the right to the chosen file |
| After Effects | 26.x | File → Import → File + Adobe Bridge | import is a system dialog with a format filter and a "sequence" checkbox; the role of a browser with a preview is taken out into a separate program (Bridge) |

What all five have in common: **nobody chooses "our own OR the system one" — all of
them have both.** The system dialog serves a one-off choice (a project, an output
file), while the permanent own panel serves work with material. The dispute "own
versus system" arises exactly where the second surface is absent and one window
carries both roles.

**Outside the core.** Without Kdenlive this note cannot be written: for the five core
editors the fork was resolved silently by the vendor, whereas here it has lived for
six years in the public tracker by name — and the "trick behind it" rests on it.

| Program | Version | What it is called | How it is built |
|---|---|---|---|
| Kdenlive | 26.08.0 | KF-media browser + KIO picker | "File → Open" and "Save As" show the system (KDE/portal) picker, while "Project → Add Clip or Folder" opens ITS OWN browser, a fork of the KDE file widget; two surfaces with different behaviour in one program |

## The trick behind it

**The request "give us the system dialog" and the request "where are my bookmarks"
are one and the same request, and triage confirmed it.** Kdenlive bug 510202 ("the
file picker shows the generic set of Places, not the one the user configured") was
closed as a DUPLICATE of bug 500753 ("add an option for the system File Open"). The
point: the dispute is not about appearance, but about transferring user STATE which
the application does not have and has nowhere to get — bookmarks, recent places,
mounted volumes, search history live in the OS shell. An own dialog does not "look
different", it zeroes out a skill the user already has and the bookmarks already
accumulated. A file dialog is, in general, the only window of a program that the user
knows how to use BEFORE installing it.

**An own dialog is not "writing a window", it is MAINTAINING a file manager.**
Kdenlive did not write a picker from scratch — it forked the system widget, and now
the tracker of a video editor hosts requests that have nothing to do with video
editing: "the Kdenlive picker icon is of extremely low resolution" (bug 521133), "the
fork of the KDE picker stops showing new files if it was opened and closed once"
(bug 514567, 5 subscribers), "Places cannot be turned off" (bug 452141, closed as
WORKSFORME), "the GTK picker does not take the Adwaita:dark theme" (bug 497967). All
SEVEN open requests under this filter were filed after December 2024 — that is, they
accumulate faster than they are fixed, and one has to fix them in someone else's domain.

**In a sandbox the choice ceases to be a matter of taste.** Flatpak, Snap and the Mac
App Store do not give the application a file system: the right to a specific file is
granted in exchange for a choice made through a system mechanism (xdg-desktop-portal,
powerbox). There, an own dialog shows not "a different look" but EMPTINESS or a hang —
shotcut #1058: pressing "Projects Folder" is enough to make the application hang. This
is the reverse side of the same medal, and nobody writes it into the feature list
either: the portal is not a superset, but a different set. Pitivi #2073: `GTK_FILE_CHOOSER_ACTION_SELECT_FOLDER
is not supported by GtkFileChooserNativePortal` — under flatpak the application simply
lost the ability to choose a directory.

## Pros

- The system dialog brings with it state the application does not have: the user's
  bookmarks and recent places, search through the shell's index, network and cloud
  volumes (GVFS/KIO/SMB/cloud providers), a preview shared with the OS. None of this is
  a "feature of the dialog" — it is someone else's work that the dialog lends.
- In a sandbox this is the only way to get the right to a file: the portal returns not
  a path but a permission. A program that goes through the portal works in Flatpak
  without holes in the manifest (the boundary with F-RES-008 — which concerns packaging
  as a whole).
- The dialog is drawn and updated by the OS: theme changes, DPI scale, fonts, RTL,
  keyboard and gesture behaviour arrive for free and the same as across the whole system.
- An OS update fixes the dialog without our release; requests of the kind "low-resolution
  icon" and "the list did not refresh" go where they are already being fixed.

## Cons

- Type filters behave differently on different OSes: Windows shows a drop-down list of
  pairs "name + masks", the portal shows its own list, macOS filters by UTI rather than
  by extension. One and the same `add_filter` gives three different experiences, and the
  extension chosen in the dialog has to be fixed up by hand.
- Filter names cannot be translated with our own catalogs: the caption "MP4 Video" lives
  in the call argument, while all the rest of the dialog's text lives in the OS locale.
  A program with a Russian interface on an English system shows a half-English window,
  and there is nothing to fix it with.
- A view of "only the media of this project" and any custom column (duration, fps, codec,
  whether the file has already been added) does not exist in the system dialog and cannot
  exist. Multiple selection is there, but the dialog does not keep the order of selection.
- The dialog is a foreign process (the portal, zenity) or a foreign modal window: it does
  not obey our theme, our hot keys or our layout, and it cannot be checked by a harness
  test — it is outside the test process.
- The portal may be absent (a bare session, a container, a virtual screen) or may not
  support the needed action — and then what fails is not the dialog but the operation.

## What users say

SIGNAL: the file dialog as a subject of complaints — 25 tasks in pitivi, `(file|folder|directory).{0,12}(dialog|chooser|picker|browser|selector)|(dialog|chooser|picker).{0,12}(file|folder)|filedialog|xdg.desktop.portal`; share 0.9% against 0.5% in kde-bugzilla

SIGNAL: the price of the sandbox — 36 tasks in shotcut, `flatpak|snap\b|sandbox|xdg.desktop.portal|permission`; share 2.6% against 0.4% in olive

The first count reads against expectation. Pitivi shows the SYSTEM GNOME chooser, Kdenlive
has its own fork; the share of complaints about the dialog is twice HIGHER for the
"system" one. The explanation is visible in the list of tasks: almost all of Pitivi's
(534, 1084, 1085, 1413, 1606, 1910, 2546) are about a preview glued into someone else's
window, that is, about the seam between one's own and the system one. The conclusion is not
"the system one is better", but "the boundary costs more than either side": the one who
tries to get both pays.

The second count is about what the sandbox costs: shotcut, which is actually shipped as
Flatpak and Snap, carries a share more than six times higher than olive, which does not
go there. Within that share, file dialogs are the most frequent way to run into the
sandbox boundary.

The share of real fixes under the first filter can honestly be counted only in
kde-bugzilla (the GitLab trackers close without a code cause): 12 FIXED out of 23 closed,
that is 52%, and all 7 OPEN requests are younger than December 2024.

The request itself with which the reconnaissance began:

> Add an option to allow the user to use the familiar OS-native File Open dialog,
> esp. when adding files to the Project Bin.
> — kde-bugzilla #500753, 25.02.2025, status CONFIRMED/wishlist,
> https://bugs.kde.org/show_bug.cgi?id=500753

Its duplicate names the cause in plain text:

> The filepicker shown shows a generic set of Places, not the set of Places that
> the user has configured in their Plasma account (e.g. what the user sees as
> their places in Dolphin).
> — kde-bugzilla #510202, 03.10.2025, closed as DUPLICATE of 500753,
> https://bugs.kde.org/show_bug.cgi?id=510202

The same thing, but six years earlier and in the developers' tracker — with a list of
what is lost:

> the Windows native form is more accurated, can manage networks folders, and
> thumbnails of any type.
> — kde-invent #574, "[Windows] open a native windows file selection window when
> loading clips", 19.02.2020,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/574

The reverse side — the price of the sandbox, paid in a hang:

> It is enough to start the application and press the "Projects Folder" button in
> the "New Project" panel to make the application hang.
> — shotcut #1058, "Shotcut flatpak broken on kubuntu 20.04, file dialogs hang",
> 20.03.2021, https://github.com/mltframework/shotcut/issues/1058

And a third side: Olive was asked for the system dialog for exactly what Kdenlive is now
asked for — because its own window turned out worse than someone else's in small things
that do not get written into the feature list (the same issue also covers horizontal
scrolling with a touchpad in its own picker), issue #484, 12.02.2019,
https://github.com/olive-editor/olive/issues/484.

## How it fits Kinotrain

ADDRESS: src/apps/kt-app/src/app.rs:2723 — the class "our own file picker grew its own bugs" is impossible for us: all six places use `rfd::FileDialog` (`app.rs:2723,2752,5300,5325`, `render_dialog.rs:185`, `morpher_editor.rs:564`), and `rfd` 0.15.4 is built with the `xdg-portal` feature and without `gtk3`, that is, it goes through xdg-desktop-portal. But another class, which nothing catches, is possible and does exist: there is no dialog for MATERIAL at all — `add_media_clip` is called only from `handle_drops` (`src/apps/kt-app/src/app.rs:4022`), and the only way to bring a file into the project is to drag it from someone else's file manager.

### What already exists

- Six calls of the system dialog, all through `rfd`: open a project and save/save as
  (`src/apps/kt-app/src/app.rs:2723,2752`), import of a legacy `.ktproj` and saving of the
  conversion result (`app.rs:5300,5325`), choice of the render output file
  (`src/apps/kt-app/src/render_dialog.rs:185`), replacement of the morpher's source
  (`src/apps/kt-app/src/morpher_editor.rs:564`).
- The titles of three of them are translated and lie in the catalog: the `[dialog]` section
  (`open_project`, `save_project`, `import_ktproj`) in
  `src/apps/kt-app/i18n/{en,ru,es,ar,zh-Hans}.toml`, called via `tr(k::dialog::…)`.
- Drag-and-drop of files onto the POINTER is a full alternative route for getting a file:
  highlighting of the track, a ghost of the clip and a snap line; the position is read
  directly from the X server (`dnd_pointer_pos`, `src/apps/kt-app/src/app.rs:4298`),
  because winit drops the coordinates of `XdndPosition`; the lifetime rule of the position
  is the pure function `dnd_keep_hover` (test 47 and its addition, items 47/52).
- A drop understands three targets: `.otio` opens as a project, a file dropped into the
  source field of an open morpher replaces the source (t218), everything else becomes media
  plus clips on the track under the cursor.
- The empty bin names the gesture in words: `bin.empty` = "empty — drop media files on
  the window" (`src/apps/kt-app/i18n/en.toml`).
- The name of the render file is suggested sensibly: `default_render_name`
  (`src/apps/kt-app/src/render_dialog.rs:147`) takes the project name and the name of the
  range marker that matches the region (t201).

### What is missing

- **An item "Import media…" does not exist.** In `[menu.file]`
  (`src/apps/kt-app/src/toolbar.rs:246`) there are seven items, and not one about material.
  A user without a second open file-manager window cannot add anything to the project. On
  Wayland this degrades further: `dnd_pointer_pos` is X11-only, and without X11 the position
  is lost and the drop falls onto the playhead.
- Translation of filter names: `add_filter("OpenTimelineIO", …)`,
  `add_filter("MP4 Video", …)`, `add_filter("Kinotrain legacy project", …)` — English
  literals that bypass the catalog. `t131` catches only CYRILLIC in literals, `t125` works
  in the other direction (a key that nobody calls), so the English caption in the system
  window is caught by no test.
- An honest refusal when the dialog did not happen. When no portal is present, `rfd` tries
  an external `zenity`, and if that is absent it writes `log::error!` and returns `None`;
  the logger in kt-app is not initialised at all (`apps/kt-app/Cargo.toml` has neither
  `env_logger` nor `tracing`), so the record goes nowhere. The user presses "Open…" — and
  NOTHING happens, with no message. This is exactly rule 16 (impossibility = ERROR).
- No sequential layout when several files are chosen: the loop `for path in dropped` in
  `handle_drops` computes the target `target` ONCE before the loop, therefore all files land
  at the same point in layers. Multiple selection in the dialog would hit the same place.
- Neither a list of recent projects nor a start screen (F-HLP-027): everything the system
  dialog could lend in the form of "recents" is not used by us.

### What gets in the way

The `rfd` dialog is SYNCHRONOUS: `pick_file()` internally does `block_on`, that is, it holds
the UI thread until the user answers. The audio callback meanwhile keeps consuming frames
(invariant 6 — cpal is the master clock), and after the dialog closes the display catches up
at once on everything that was played blind; rule 2 ("works during playback") is formally
observed — sound plays — but the picture stands still.
The asynchronous variant of `rfd` solves this problem, but it requires a state "waiting for the
dialog's answer" in the application, not a local variable.

Second: the live scenarios will not see the dialog. `src/scripts/live-check.sh` starts Xvfb
and openbox — without a session bus and without a portal; `src/scripts/ui-probe.py` finds the
window by `WM_CLASS = "kinotrain"` and sends XTest events in its coordinates, while a portal or
zenity dialog is a FOREIGN process with a different window class. The harness (`layout_tests.rs`)
has no window at all. Therefore not one of the eleven live scenarios and not one of the tests
presses "Open…": any such click would block the run on a real portal. Only what is checked
before and after the call is verifiable — and this must be accepted as a boundary, not
an attempt to push it back.

Third: the extension. The portal does not have to append the extension from the filter, so
`save()` and `import_ktproj_dialog` already guard against it by hand
(`if p.extension().is_none() { p.with_extension("otio") }`), while
`render_dialog::launch_render` does not.

### Which invariants it touches

Invariant 6 (cpal is the master clock) and 12 (display is not quantised, the UI does not
heat the system) — both are touched not by the dialog itself, but by its synchronicity:
while the UI thread stands in `block_on`, `video_playhead()` is not computed. Rule 2
(works during playback), rule 16 (impossibility of an action is an error, not silence),
rule 17 (paired ways of one action behave the same: a drop puts the file under the mouse,
a future dialog must put it just as meaningfully, not silently on the playhead). The
project format and the engine are not touched by this topic at all.

## Estimate

| What | Answer |
|---|---|
| Volume | S — one stage of a round |
| Where the main work is | kt-app: menu item and call, i18n catalogs (filter names), one honest message on `None` |
| Project format | not touched |
| Tests that will have to be written | unit: laying out N selected files gives N different positions, not one; unit: the list of extensions of the media filter matches what `open_source_or_placeholder` can open; i18n: filter names live in the catalog (t125 extended in the other direction — a literal bypassing the catalog); layout: the menu item exists and calls the import |
| Depends on | nothing; borders on F-RES-008 (sandbox and packaging), F-HLP-027 (recent projects), F-MED-001 (organising material) |
| Breaks compatibility | no |

## Verdict

Take it, priority B, cost S — but not what is asked in the other tracker. The request
kde-bugzilla #500753 is a request to REMOVE one's own picker; we never had one and there
is no reason to create it: the three "tricks" of this note are about the price of one's own
window, and none of them pays off. Our gap is the mirror image: the system dialog exists
everywhere except for the most frequent action — adding material, where the only path is
DnD from someone else's file manager, which degrades on Wayland to "onto the playhead".
So the work is: an item "Import media…" that calls the same `rfd` with `pick_files()`, the
layout of several files not into one point, filter names in the catalog, and an honest
error instead of silence when the portal did not answer.

We do NOT create our own file browser and do not drag it into this note: a material panel
with columns and filters is F-MED-001 (smart folders and search), and there it is discussed
as a BIN, not as a replacement for the system window. Exactly this choice — "two surfaces:
the system dialog for a one-off choice, one's own panel for material" — was made by all
five core editors.

## Sources

- Registry and counting method: `docs/features/_trackers.md`; both numbers are reproduced by
  `python3 src/scripts/tracker-rank.py --query '<filter>'`. At the time of writing
  (08.09.2026) the export was COMPLETE for seven trackers — `kde-bugzilla`, `kde-invent`,
  `pitivi`, `shotcut`, `mlt`, `otio`, `olive` (by the end of the work `losslesscut` was also
  loaded); `flowblade`, `openshot`, `blender-vse` and `ffmpeg-trac` were still downloading,
  therefore their shares are not given here. All four trackers from which the numbers are
  taken were exported in full.
- https://bugs.kde.org/show_bug.cgi?id=500753 — Feature: Add option to use
  OS-native File Open dialog, CONFIRMED/wishlist (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=510202 — "Add Clip Or Folder" without configured
  Places, closed as DUPLICATE of 500753 (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=514567 — the fork of the KDE picker does not show
  new files after reopening (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=521133 — the picker icon is of low resolution
  (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=497967 — the GTK picker does not take
  Adwaita:dark (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=452141 — "Places" cannot be turned off, closed
  as WORKSFORME (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/574 — a request for the Windows
  system window when loading clips (accessed 2026-09-08)
- https://github.com/mltframework/shotcut/issues/1058 — flatpak: file dialogs hang the
  application (accessed 2026-09-08)
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2073 — the portal does not support
  choosing a directory (accessed 2026-09-08)
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2546 — broken seek in the preview of
  the import dialog (accessed 2026-09-08)
- https://github.com/olive-editor/olive/issues/484 — a request for a native dialog in
  AppImage (accessed 2026-09-08)
- Our code and dependencies checked by grep on 08.09.2026:
  `src/apps/kt-app/src/{app.rs,render_dialog.rs,morpher_editor.rs,toolbar.rs}`,
  `src/apps/kt-app/src/{i18n_tests.rs,layout_tests.rs}`,
  `src/apps/kt-app/i18n/*.toml`, `src/apps/kt-app/Cargo.toml`,
  `src/Cargo.lock` (rfd 0.15.4 → ashpd, no `gtk-sys` in the graph),
  `src/scripts/{live-check.sh,ui-probe.py}`,
  and also the sources of `rfd-0.15.4` in the cargo registry (`backend/xdg_desktop_portal.rs`,
  the fallback to `zenity`).
- The core editors' rows — versions from `docs/features/_editors.md` (checked
  2026-09-07); the behaviour of the Vegas/Premiere/Resolve/FCP/AE file dialogs was NOT
  checked live; the Resolve row relies on the observed look of the windows and is not
  confirmed by the vendor.

# Design: welcome-window-and-settings

## Context

See proposal.md — Why. The welcome window (`welcome.rs` `draw_welcome`) is a `.width(820)` window that
fits its content: one `window_body` scroll holds a left column (templates, recent) and a fixed 220 px
right column (tour, Guide link, What's new). `windows-fill-their-area` made it resizable but left the
right column fixed on purpose ("its own change comes next"). The View menu (`toolbar.rs`) holds eleven
items; six are preferences. `KtApp::ui_economy` and `KtApp::lang_choice` are fields of the app that
`replace_with` rebuilds, so File → New/Open resets both (the display language stays until restart,
then the stored `lang` is "auto").

## How others do it (the comparison the owner's "ugly" asks for)

| Program | Start window | The "show at start" switch | Preferences |
|---|---|---|---|
| DaVinci Resolve 19 | Project Manager: a grid of project posters that grows with the window, the window resizable | none (the manager is the start) | DaVinci Resolve › Preferences… (a window with sections) |
| Premiere Pro 25 | Home screen: left a column of actions (New, Open), the middle a list/grid of recent projects with thumbnails, a "What's new"/learn strip | Preferences › General › "At startup: Show Home" | Edit › Preferences (Windows) — a window with sections |
| Vegas Pro 22 | Welcome dialog: new/open on the left, recent on the right, tutorials tab | in the welcome dialog | Options › Preferences… |
| Blender 4.x | Splash: a picture, left "New file" templates, right "Recent files"; What's new is a link | Preferences › Interface › "Splash Screen" | Edit › Preferences… |

The trick they share: the start window is laid out in COLUMNS by kind of action (create · reopen ·
learn/news), each column scrolls alone, and the switch that hides the start window lives with the
other preferences, not in the window (Premiere, Blender). Two programs put the switch in the window
and in preferences (Vegas, VS Code) — exactly the duplication the owner objected to.

## Goals / Non-Goals

**Goals:** a start window in panes that use the window's area; one switch per preference in one
window; preferences that survive a project open.

**Non-Goals:** a GPU/CPU preference and an autosave interval — neither is exposed in the UI today
(`KT_AUTOSAVE_SECS` is a test seam, the GPU fallback is automatic); adding them would be new features
without a request. Project settings stay a separate window (they belong to the project).

## Decisions

### D1. Three panes, each its own filling scroll
The body is a `horizontal_top` row of three `vertical` panes; each draws a title row and then its own
`widgets::window_body` (the filling scroll that leaves the action row below; the fill probe unions the
panes). Widths: Start `clamp(0.25 w, 260, 340)`, What's new `clamp(0.34 w, 300, 560)`, Recent takes the
rest (floor 200). The window is `.size([1180, 720], [760, 440])` — not fitted: a fitted window measured
its content and once grew to 2010 points (t986's reason); a sized window cannot grow from its content.
On a screen smaller than the default the window starts at the screen's size minus a margin (one
`ALLOWED_FIXED` row: "the start window never opens larger than the screen"). Rejected: tabs (the owner
wants to SEE What's new beside the start actions; a tab hides it), a single column (the needle's eye).

### D2. What's new is the page, from the running version down
`guide::whats_new_from(version)` returns the label and the blocks from the running version's section
to the end of `whats-new.md` (the older sections keep their `##` headings), rendered by `guide::render`
— the same renderer as the Guide (tables, `<kbd>`, links into Guide pages open the Guide). Help → What's
new scrolls that pane to its top. When the update-check state already says a newer release is out, one
line "Kinetren X is out — Releases page" heads the pane (the spec promised it; no request is made).

### D3. Start pane: primary, templates by shape, lessons
The primary tile is the first element of the Start pane, drawn larger with the accent stroke (one
primary, `primary()` unchanged). Templates are rows with a small frame drawn in the template's aspect
(16:9, 9:16, 1:1) — the shape is what distinguishes them, not the number. The five lessons are rows
"N · title" with their progress (done/steps, a check mark when done); a click opens that lesson. The tour
tile stays ("Start the tour" / "Continue the tour — lesson N, step M") above them.

### D4. Recent: a grid that grows
Cards per row = how many 180-point cards fit; the card width is then stretched to fill the row, the
poster keeps 16:9. Name and folder truncate to the card; a missing project is flagged with Locate…;
right-click → Remove from the list. Clear the list sits in the pane's title row.

### D5. Preferences window `kt_prefs` — the owner's "Settings", the industry's "Preferences"
File › Preferences… (en), "Настройки программы…" (ru, beside "Настройки проекта…"). Rule 21 (name by
industry): Resolve, Premiere, Vegas, Blender and AE all say "Preferences"; in English "Settings…" next
to "Project settings…" reads as the same window twice. Sections: Start (welcome window at start),
Interface (Language as a combo, Economy UI), Privacy and updates (statistics, the daily check — moved
with their disabled-with-a-reason form), FFmpeg (the state line and "FFmpeg…" opening `kt_ffmpeg`).
Kind `Tool`, `.width(560, [420, 240])`, body in `window_body`, Close in the action row. Registered in
`WINDOWS`: "the program's preferences: per machine, outside any project, reachable with no project
open — the View menu held them mixed with what is viewed (kkt-26.11.01 item 13)".

### D6. Revision: the start switch leaves the welcome window and View (welcome-window-and-tour D1/D11)
welcome-window-and-tour put "Show at start" in the window AND in View ("said where to get it back").
Revision — reason: the owner's new request (item 13) names exactly that duplication. The switch lives
only in Preferences; the welcome window's action row has a "Preferences…" button (a door, not a copy),
whose hint says the window's opening is switched there; Help → Welcome window… still reopens it.

### D7. Revision: telemetry and the update check leave View (telemetry, distribution)
Those requirements put their checkbox in View ("a per-machine preference lives next to the other
ones", owner 2026-10-27). Revision — reason: the owner's request of 2026-11-01 moves the program's
preferences out of View into one window; the rule behind the old place ("next to the other per-machine
preferences") is kept: they are next to Language in Preferences, the update check right after
statistics. Their stored values stay in eframe storage (their process-global objects own them).

### D8. Language and Economy UI live in the user store
`StartChoice` gains `economy_ui: bool` and `lang: Option<String>` (the choice code; `None` — never
chosen through this record). `run` reads the start store BEFORE resolving the language: `KT_LANG` →
the store's `lang` → the former eframe `lang` (one-time fallback) → the OS locale. The `KtApp` fields go;
the code reads `self.start.store.start`, which `replace_with` carries — so File → New keeps both (the
found defect, t1295). Writes happen on change only (tmp + rename, as the store does).

### D9. One switch, one place — a guard over the writers
A closed list of preference writers (`set_show_at_start`, `set_economy_ui`, `set_lang`, telemetry and
update `set_choice`) must each be called from exactly one non-test site, and that site is
`prefs_window.rs`; the View menu and the welcome window are probed in the harness and hold none of the
preference labels. Rejected: comparing checkbox labels — the original defect had two DIFFERENT labels
("Show at start", "Welcome window at start") for one value.

## Risks / Trade-offs

- [Old paths in guards and live scenarios] → t840/t850/t878/t986/t1146 and the `view.*` probes are
  rewritten to the Preferences path in this change, each with the reason in its doc comment.
- [Three panes on a narrow screen] → minimum 760 wide: Start 260 + Recent 200 + What's new 300 fits;
  below it the window cannot shrink (the shell's min).
- [Catalog churn in five languages] → keys move with their translations; the arrows rule (no `→`) holds;
  new CJK glyphs → `subset-fonts.py` if t100 asks.

## Migration Plan

The store's `start.json` gains two keys; an older build ignores them. The eframe `lang` is still read
when the store has no `lang`, so the first start of this build keeps the person's language.

# Design

## Context

See proposal.md — Why. What is in the code today (read 2026-10-28, branch
`kkt-26-10-27-proposals`):

- **There is no window builder.** 21 sites call `egui::Window::new` and 2 call `egui::Modal::new`
  directly; each sets its own `.open`, `.collapsible`, `.resizable`, `.vscroll`, sizes and
  anchors. What they share is only `widgets::window_body` (one body `ScrollArea`, height
  `min(0.62·screen, available − ACTIONS_H)`, `src/apps/kt-app/src/widgets.rs:454-467`) and
  `widgets::window_actions` (`widgets.rs:477`) — the R.66 form contract.
- **The Esc cascade is a hand-kept list.** `KtApp::escape_action` (`src/apps/kt-app/src/app.rs:2703`)
  checks, in a fixed order: speed field, search, preset name, paste prefix, published list,
  render settings, track window, FX windows, morpher windows, notices, FFmpeg, diagnostics; then
  drag, region+selection, collapse, leave the reel. The transition, camera, project settings
  and envelope-fit windows are not in the list, so Esc never reaches them (observed by reading:
  no `Key::Escape` in `camera_editor.rs`, `transition_editor.rs`, `settings_window.rs`,
  `envclip.rs`). The R.52 lesson in memory says the same: "editor windows MUST join the Esc
  cascade, otherwise a live scenario cannot remove the window" — and the list still lost four.
  The order is fixed, not "the top window": with the camera window over an FX window, Esc
  closes the FX window first.
- **Four dialogs swallow Esc themselves** while their text field has focus
  (`pubparam.rs:332`, `app.rs:3554`, `app.rs:4683`, `app.rs:4751`) — four copies of the same
  five lines. The two modals (`app.rs:1890` recovery, `app.rs:1946` ask) do not react to Esc at
  all (no `should_close`).
- **Window-level scroll.** egui 0.36.1 builds every `Window` with
  `ScrollArea::neither().auto_shrink(false)` (`egui-0.36.1/src/containers/window.rs:115`);
  `.vscroll(true)` turns it on. R.67 removed it from the FX window (`fx_editor.rs:475-478`,
  the comment) and the transition window, because it scrolled the action buttons away. It is
  still on in the morpher (`morpher_editor.rs:330`), camera (`camera_editor.rs:84`), notices
  (`app.rs:2074`) and diagnostics (`app.rs:8866`) windows.
- **egui's `Resize` only grows while not dragged**: `desired_size = desired_size.max(last_content_size)`
  (`egui-0.36.1/src/containers/resize.rs:269`, and again per axis at `:356` for resizable
  axes). Any frame whose content is a fraction wider than the window widens it for good.
- **t779** (`src/apps/kt-app/src/reach_tests.rs:3056`) checks the form contract for the
  openable windows; the morpher, transition and camera windows are `OpenWin::Cant` there
  (`reach_tests.rs:2985-2996`), so the three windows with the most content are not checked at all.
  **t757** (`reach_tests.rs:2675`) finds windows by the text `egui::Window::new`/`Modal::new` and
  an `egui::Id::new("…")` within four lines.

### Window inventory

| id | site | cross | collapse | resize | window scroll | body scroll | Esc today |
|---|---|---|---|---|---|---|---|
| `fx` | `fx_editor.rs:479` | yes | **yes** (default) | yes | no | `window_body` | cascade: close, keep edits |
| `morph` | `morpher_editor.rs:324` | yes | **yes** | yes | **`vscroll(true)`** | `window_body` | cascade |
| `transition` | `transition_editor.rs:183` | yes | **yes** | yes | no | `window_body` | **none** |
| `camera` | `camera_editor.rs:78` | yes | **yes** | yes | **`vscroll(true)`** | `window_body` | **none** (owner's case) |
| `track_window` | `track_window.rs:128` | yes | no | no | no | **own copy** of `window_body` (`track_window.rs:139-142`) | cascade |
| `env_fit` | `envclip.rs:171` | yes | no | no | no | `window_body` | **none** |
| `project_settings` | `settings_window.rs:134` | yes (= Cancel) | no | no | no | `window_body` | **none** |
| `kt_render_settings` | `render_dialog.rs:276` | yes | no | no, anchored | no | `window_body` | cascade |
| `kt_render_progress` | `render_dialog.rs:501` | no | no | **yes** | no | none | none |
| `kt_ffmpeg` | `ffmpeg_setup.rs:449` | yes | no | no | no | `window_body` | cascade |
| `pub_name` | `pubparam.rs:302` | yes | no | no | no | none | own handler `pubparam.rs:332` |
| `published_win` | `pubparam.rs:390` | yes | **yes** | yes | no | `window_body` | cascade |
| `pub_unpublish` | `pubparam.rs:543` | **no** | no | no | no | none | **none** |
| `kt_analysis` | `app.rs:2043` | no | no | no, anchored | no | none | none |
| `kt_notices` | `app.rs:2068` | yes | **yes** | yes | **`vscroll(true)`** | none | cascade |
| `kt_search` | `app.rs:2333` | yes | no | no, anchored | no | none | cascade + own `app.rs:2355` |
| `seq_name` | `app.rs:3521` | yes | no | no | no | none | own `app.rs:3554` |
| `paste_names` | `app.rs:4615` | yes | no | no | no | none | own `app.rs:4683` + cascade |
| `preset_save` | `app.rs:4708` | yes | no | no | no | none | own `app.rs:4751` + cascade |
| `kt_diag` | `app.rs:8860` | yes | **yes** | yes | **`vscroll(true)`** | none | cascade |
| `kt_recovery` | `app.rs:1890` (Modal) | — | — | — | no | `window_body` | **none** |
| `kt_ask` | `app.rs:1946` (Modal) | — | — | — | no | none | **none** |

Bold = deviates from what the rest of its kind does. Seven windows keep egui's collapse arrow
by default, which nobody chose; four keep the window scroll; six do not react to Esc.

### What the minimal reproduction showed (assumption NOT confirmed)

The prior research blamed `vscroll(true)` plus full-width rows (the chain strip's
`horizontal_wrapped`, `refs_row` `morpher_editor.rs:867`, the curve graph
`allocate_exact_size(available_width, 150)` `automation.rs:677`) under a fractional scale. A
scratch program outside the tree (egui 0.36.1 + egui_kittest 0.36.1, the same `window_body`
code, a resizable window 420×260 with `vscroll(true)`, a wrapped strip of 30 buttons, a
full-width 150 px graph, a multiline `TextEdit` with `desired_width(INFINITY)`, a collapsing
group with a wrapped weak note and a wrapped selectable-label row; 300 frames with the pointer
moving) gave: scale 1.0 — 420.00 stable; 1.25 with `vscroll` — 420.38 from frame 2, then
stable; 1.75 without `vscroll` — 420.31, stable; 1.5 and 2.0 — 420.00. **No ratchet.** So the
window scroll gives a one-off sub-pixel step at 1.25 but is not by itself the endless growth;
something in the real morpher window feeds back. Candidates, none checked: content whose width
depends on the previous frame's width (the thumbnail picker, a field sized from
`ui.available_width()` minus a measured label), content that changes every frame (a time
readout under the pointer, the curve graph's hover readout), or the owner's other system not
being a fractional scale at all (Windows/wine, a different font). Hence stage 1 below is an
investigation in the REAL window, not a fix.

## Goals / Non-Goals

**Goals:**
- One shell through which every registry window is built; the window sites keep only their
  content and their kind.
- Esc closes every window, the top one first; dialogs cancel safely; declared exceptions.
- The morpher growth found and fixed with a test that is red on today's code; no window grows
  by itself at 1.0/1.25/1.5.
- Mechanical guards so the next window cannot opt out.

**Non-Goals:**
- No re-layout of window contents (R.66: only the six dense forms were laid out, the rest obey
  the contract — unchanged).
- No new window and no window removed (the registry entries and reasons stay).
- The detached preview viewport (`app.rs:1740-1765`) is not an `egui::Window` and keeps its own
  local Esc (kt-app CLAUDE.md: "Esc of the preview window is scoped locally").
- Popups, menus and combo boxes are not windows and are not touched.

## Decisions

### D1. A builder with a declared kind, not a trait over the windows

`window_shell::show(ctx, Spec { id, title, kind, size }, body, actions) -> ShellResponse`.
`kind` is a closed enum:

| kind | cross | collapse | resize | Esc |
|---|---|---|---|---|
| `Tool` (fx, morph, transition, camera, track_window, published_win, notices, diag, search, ffmpeg) | yes | never | resizable when the site gives `default_size`/`min_size`, fixed otherwise (search) | close, keep edits |
| `Dialog` (env_fit, project_settings, render_settings, pub_name, pub_unpublish, seq_name, paste_names, preset_save) | yes | never | no | the declared "loses nothing" action (Cancel / No) |
| `Progress` (render_progress, analysis) | no | never | no | not touched — reason string mandatory |
| `Modal` (recovery, ask) | — | — | — | the declared safe answer (Later / Cancel) |

The shell draws the body inside the one body scroll (`window_body`, moved into the shell),
then the actions row (`window_actions`), and returns whether the window is still open, whether it
was closed by the cross or Esc, and its `LayerId`. Alternative considered: a trait each window
type implements — rejected, the windows are not types but closures over `&mut KtApp`, and a trait
would force a second state object per window; the builder keeps the call sites as they are minus
the options.

Collapsing is removed from the seven windows that had it by egui default: no decision ever
asked for it, a collapsed window still takes Esc and covers the timeline with its title bar, and
the guard needs one rule. This is not a look set by the owner (`docs/user-feedback.md` has no
request about collapsing windows).

### D2. Esc closes the top window by layer order

Each frame the shell records `(LayerId, id, kind, on_esc)` of every window it showed into a
per-frame list on `KtApp`. `escape_action` step 1 becomes: take the shown windows whose kind lets
Esc act, pick the one that is highest in `ctx.memory(|m| m.areas().order())`, run its declared Esc
action, return. Step 0 (the speed field) stays first — it is not a window. The hand-kept list is
deleted, so a new window cannot be forgotten. Alternative: keep the list and add the four
missing windows — rejected, it is exactly the mechanism that lost them, and it closes by a fixed
priority instead of what the person sees on top.

The four per-dialog Esc handlers move into the shell: when the egui focus is inside a window (the
focused widget's layer is the window's layer), the shell itself consumes Esc for that window in
the same frame. The track window's comment (`track_window.rs:457-461`) documents that egui
releases focus on Esc at the start of the frame, so the cascade sees the key on the same press —
the shell keeps that behaviour and asserts it in t919.

### D3. One scroll, and the width comes only from the person or the content

The shell never sets a window-level scroll (`.vscroll`/`.scroll`). The width rule is decided in
stage 1 from what the investigation finds; the candidates, in order of preference:
(a) remove the feedback source in the content (if one widget sizes itself from the previous
frame); (b) the shell pins the body to the window's inner width rounded DOWN to the physical
pixel grid (`ui.set_max_width(floor(w·ppp)/ppp)`), so a sub-pixel excess cannot be reported as
content size; (c) `max_width` of the window at the screen's content width as the last ceiling.
(a) is the fix, (b) is the shell's guarantee for every window, (c) only bounds the damage.
Revision of nothing: R.67's "one scroll per window" (ui-timeline design, "One scroll per window")
is the same rule, now applied to all windows instead of two.

### D4. Guards

- t921 scans kt-app (test code cut at `\n#[cfg(test)]`, like t779/t131) for `egui::Window::new`,
  `Modal::new`, window `.vscroll(`/`.scroll(`/`.hscroll(`, `.collapsible(` outside the shell
  module. `ScrollArea::…` calls stay legal (panels, lists).
- t757's scanner follows the shell call (`window_shell::show(` + the `id` field) instead of
  `egui::Window::new`; the registry and reasons stay byte for byte.
- t779's openers get real scenes for `morph`, `transition`, `camera` — borrowed from the scenes of
  t242, t227 and t231b in `layout_tests.rs` — and three `OpenWin::Cant` excuses disappear.
- t918 runs every openable window at scales 1.0/1.25/1.5 (`Harness::builder().with_pixels_per_point`),
  t923 is the owner's morpher scene; t919/t920/t922 are the Esc and cross tests.

## Risks / Trade-offs

- [Esc by layer order picks a window the person did not look at last when two windows overlap
  without focus] → the order is egui's own "who is drawn on top"; it is what the person sees,
  and t920 fixes it as the contract.
- [Moving 23 sites in one change breaks a live scenario that clicks a window by offset] →
  stage 3 runs the full `./scripts/live-check.sh`; the shell does not change content layout, only
  the frame options; removing the collapse arrow shifts nothing inside the window.
- [Rounding the body width down hides a real content overflow by under one pixel] → harmless;
  overflow of more than a pixel still widens the window as content.
- [The cause on the owner's system is not the fractional scale] → stage 1 records what reproduces;
  if nothing reproduces under Xvfb at 1.25/1.5, the owner is asked for the system (Open Question 1)
  before the width fix is claimed.

## Migration Plan

No data migration. The window positions and sizes live in egui memory by window id; the ids do
not change, so stored positions survive. A window that had already grown is shrunk to the new
rule on the first frame only if its stored size came from growth — the stored size is a size,
egui does not know where it came from, so a grown window keeps its size once and stops growing;
the person can make it smaller by the edge. Rollback is a revert of the change.

## Open Questions

1. What is the owner's "other system" — OS, display scale factor, Wayland or X11, and does the
   growth happen in the FX window too? Recommended answer: assume Linux with a fractional scale
   (1.25/1.5) and reproduce under Xvfb with `WINIT_X11_SCALE_FACTOR`; ask only if stage 1 cannot
   reproduce it.
2. Should Esc in the project settings window cancel (as its cross does today) or apply?
   Recommended answer: cancel — the cross already cancels, and "Esc loses nothing" is the rule
   for every dialog.
3. Export progress: should Esc stay inert, or ask "Cancel the export?" Recommended answer: inert,
   with the reason written; the window has its own Cancel button, and a job of minutes must not die
   from an Esc meant for the timeline.

# Proposal

## Why

The owner (2026-10-27, item 8): "We have no guideline inside the program explaining what can be
done and where." Asked why the guided tour was rejected (F-HLP-028), he answered (2026-10-28):
"I would not mind if at program start we had an intro window (as in programming IDEs) offering
either to create a project from a template or a list of recent projects (with thumbnails), and
right there a tile 'start the tour'… a main tile where the most likely action lands: for the first
opening it is the tour, for later openings the tile of the previous project. And in the same window
we can show what's new (and hint there that an update is needed)." Today the start silently opens
`last_project` (`src/apps/kt-app/src/app.rs:9130`), the program keeps exactly one invisible recent
path, the first-run instruction dies in the status line after 6 s (F-HLP-027), there is no Help menu
at all (`src/apps/kt-app/src/toolbar.rs`: File, Edit, View), and the written user guide
(`docs/guide/*.md`) cannot be reached from the program.

## What Changes

- **The welcome window** at start (unless a project was given on the command line, a recovery is
  offered, or the person switched it off): "new from template" tiles (built-in 1080p25, 1080p30, 4K30,
  9:16 vertical, 1:1 square; plus the person's own templates saved from Project settings), recent
  projects with poster thumbnails (a frame rendered by the offline path into the user data directory,
  never into the project tree; a placeholder until it exists; a missing file stays in the list and
  says where it was), a "Start the tour" tile, a link to the Guide, and a What's new panel for the
  running version with a calm "a newer version is out" line read from the update-check state. One
  tile is the PRIMARY one — the most likely action, chosen by a pure function: the tour on the first
  start, the previous project later.
- **"Don't show at start"** — a checkbox in the window and in the View menu; off means the former
  start (open the last project silently).
- **The tour — lessons in their own view** (the Ableton Learn View form that F-HLP-028 deferred): a
  docked side panel with lessons of short steps; a step closes on a predicate over the project's
  state (not on a command registry), "Show" opens or focuses the step's target window, panel or menu;
  the lessons run on a copy of our built-in demo project made of generators (F-HLP-029); progress
  lives in the user store, with a reset. This **revises the F-HLP-028 verdict**: a teaching layer over
  the interface stays rejected, the lesson form is taken.
- **Help menu** (new): Guide…, Start the tour, Welcome window…, What's new.
- **The Guide window**: a searchable index "I want to … → where / which gesture", and pages rendered
  from `docs/guide/*.md` and `docs/guide/shortcuts.md` compiled into the binary (one source with
  the repository's guide).
- The user store gains: the recent list with posters, user project templates, tour progress, the
  start choice.
- Feature notes updated: F-HLP-027 (welcome screen), F-HLP-028 (tour, dated revision), F-HLP-023
  (manual), F-HLP-029 (sample project).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `ui-reach`: adds the welcome window and its primary tile, the lessons view and its step
  predicates and "Show" targets, the Guide window and the Help menu; adds the guard that no layer
  above the window layer explains the interface.
- `durability`: adds the user-store records that live between projects — the bounded recent list and
  its posters (outside the project tree), user project templates, tour progress and the start choice —
  with the same broken-file and future-version rules as the existing store.

## Impact

- Code (kt-app): new `welcome.rs` (window, primary-tile function, poster service on the offline
  reel-preview path), `lessons.rs` (lesson data, step predicates, the side panel), `guide.rs` (index,
  markdown-subset renderer, search); `app.rs` (start order in `run`, the side panel, the poster job on
  save/close), `toolbar.rs` (Help menu, View checkbox), `user_store.rs` (four records),
  `reach_tests.rs` (`WINDOWS` gains `kt_welcome` and `kt_guide`), `i18n/*.toml` (new sections ×5).
- A shared demo-project builder moves from `src/apps/kt-cli/src/main.rs` (`demo_project`) to a crate
  both programs can call, in its generator-only form.
- Reads, does not add: the update-check state (`openspec/changes/update-check`); no network code.
- Docs: `docs/guide/whats-new.md` (new, English, one section per `ktYY.MM`); notes F-HLP-023/027/028/029.
- No project-format change; no engine change; invariant 12 (no repaint loop, nothing animates) and
  invariant 5 (the poster render owns its own short-lived sources) are kept.
- Tests reserved: t984–t989.

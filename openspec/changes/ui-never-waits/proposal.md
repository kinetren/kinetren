# Proposal: ui-never-waits

## Why

The owner, 2026-11-07 (`docs/ktprompts/kkt-26.11.07.txt`, row E8): "стоит мне случайно ввести какой-то
аномальный параметр эффекта, ну скажем сейчас очень паршивая скорость работы datamosh … у меня весь ui
начинает лагать как хуй знает что, я просто БУКВАЛЬНО НЕ МОГУ ОТМЕНИТЬ СВОЙ ЖЕ ЭФФЕКТ! … тупых решений
не предлагай, нужны правильные." On 2026-11-08 (`kkt-26.11.08`, row H9) his release run
kt26.11.08-360b50e6: his journal (`fails/26.11.08/new2/load-report.txt`, 06:38–06:39) notes the UI
thread blocked 0.3–4.5 s inside single frames during playback (4509, 3704, 3343, 3232 ms …) while the
whole process takes a quarter to half a core: the UI thread waited, it did not compute. Found-defects
121 ("the composite stays on the UI thread during playback") is the same.

CLAUDE.md: "No interface action waits for a picture: a slow effect costs frames, never the Undo."

## The mechanism

The UI thread does or waits for work whose duration it does not control. The composite ran in the
UI frame (`draw_preview` → `present_preview`), and a Datamosh stream stage runs INSIDE the plan: its
kernel decodes the event's history, one compute job per frame, each waited for with
`poll(wait_indefinitely)`. On his project one frame took seconds, ~45 device submits a second, and
the UI thread stood in it. The same mechanism acts wherever the UI thread takes a lock another thread
holds through long work, waits for a GPU, opens a file or builds an engine. The walk of every such
place is design D2.

## What Changes

- The preview composes on a thread of its own (`compose_queue`, from the started work
  `claude/ui-never-waits-wip`): the UI asks and goes on, the newest ask replaces a waiting one, a
  frame of a state the model has left is abandoned, the picture on screen stays until the next one
  comes. A chain whose executor time passes 150 ms moves to the offline device, so the display's queue
  (and egui's swapchain behind it) never stands behind it; only the executor's own time counts.
- The bars' check of kept moments against an edit (cached-range-plays) builds plans, so it runs on the
  preview's thread between frames; the UI reads the map. The executor's memo is asked with the map
  free.
- The display target and the glow are published apart from the engine's lock, which the preview's
  thread holds for a whole composite.
- Locks: the project lock no longer lives through the audio render thread's engine build, nor the
  source-facts lock through the prefetch thread's; a decoder is joined after the workers' write
  guard is gone; the thumbnail worker opens a file with the media map free.
- The UI's own audio renderer of a new project version (the meters on a pause, the loop's pinned
  head) is built on a thread of its own while the one before answers; the first one is still built
  where it is asked.
- The render window finds the largest source on a thread, keyed by the files instead of the revision
  (every edit opened all the files again on the UI thread while the window was open).
- The effects window asks whether the compute device works without opening it on the UI thread.
- `kt-cli probe` prints the video renderer's build cost; the geometry dump carries each event's chain
  length.
- Tests t1840–t1848; live scenario `ui_wait`.

## Impact

- `kt-engine` (player, compose_queue), `kt-gpu` (engine: the published target, a test hold),
  `kt-plugin-api` (`compute::unavailable_now`), `kt-app` (preview, render window, thumbnails, effects
  window, geometry dump), `kt-cli` (probe), `scripts/live/ui_wait.py`.
- No file format change, no visible control changes. Visible: a slow effect shows its picture late;
  the interface, Undo and every window answer meanwhile.

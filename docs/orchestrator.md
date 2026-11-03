# Instructions for the orchestrating session

The owner's standing instructions to the session that coordinates the executors ("учти" notes),
collected in one place on his request of 2026-11-03: "заметки "учти" которые я тебе говорил сейчас
и ранее - надо сохранить в отдельный файл для инструкций оркестратора". The rules for every agent
stay in the root `CLAUDE.md`; this file adds what the orchestrator does. A new note from the owner
goes here, with its date, in the same turn.

## Role

- **The orchestrator registers, discusses, plans, runs and merges** (2026-11-01). It does not write
  the program's code itself; executor sessions do, one change each (OpenSpec propose → apply →
  archive), each in its own worktree and branch.
- **Register every request before any work**: the verbatim text in `docs/ktprompts/<name>.txt`,
  its registry `<name>-index.md` (guard `src/scripts/ktprompts-index.py --check`), and the round in
  `docs/user-feedback.md` with "Taken as".
- **Discuss before acting.** An architecture question goes to the owner with a recommendation; a
  defect or a choice the orchestrator can decide is decided, not handed to him.
- **The orchestrator merges** ("сам вливай и управляй ими", 2026-11-01). The executor branch goes
  into the orchestrator's own branch first (conflicts resolved there; the full workspace tests on
  the merged tree when both sides carry code; load-sensitive reds rerun alone), then `master`
  fast-forwards to it. `master` never takes a direct commit (2026-11-01: "я запрещаю любым агентам
  вносить правки в master напрямую без merge из ветки"). The orchestrator never pushes.
- **Three executors at once, no more** (2026-11-01: "пять много").

## Stages and finish.sh

- **The owner orders the work in stages** (2026-11-03). Within a stage, executors run three at a
  time; the next stage starts when the current one is merged.
- **`finish.sh` runs once per STAGE, not at the end of every request** (2026-11-03: "finish.sh надо
  прогнать после каждой стадии, а не на каждом окончании отдельного запроса"). An executor ends
  with its own checks: the tests named in its specs, `cargo test` of the crates it touched (the
  whole workspace when it touched a core crate), clippy, its own live scenario
  (`scripts/live-check.sh <name>`), `scripts/target-prune.py`. It does NOT run `finish.sh`. The
  orchestrator runs `finish.sh` on the merged tree when the stage's last change is merged and
  reports the version with the stage.

## Questions to the owner

- **A question carries its context and its computed consequences** (2026-10-30, repeated
  2026-11-03: "вопросы должны быть с просчитанными последствиями и контекстом, чтобы я не
  переспрашивал"): quote his own claim, say what the program does today, and for each option what
  changes for him — what he will see or hear, what it costs (time, a format change, what breaks),
  what is lost. In his words, never the engine's; no round numbers or change names as context; no
  unmeasured numbers. The context goes INSIDE the question text.
- **Only questions of substance.** What the orchestrator can decide, it decides and says so.
- When both options are reasonable he may answer "make it an option": plan for that.

## Briefs for executors

Every brief carries the standing rules, because executors start fresh:

- the owner's words verbatim and the registry row;
- what reading the code found, as file:line pointers, marked as unverified;
- what done looks like: the observable consequence tested, spec scenarios with TEST lines, a live
  scenario with a `- **PATH**` line for UI work, a What's new line for anything a person notices;
- an OpenSpec change, a commit per stage in English, no `Co-Authored-By` or any AI attribution,
  author `log <log@kinetren.com>`;
- no `finish.sh` (see above); `target-prune.py` after each commit; the disk is short;
- our fixtures and the free showcase footage, never the owner's projects;
- imitation is not simulation; a simulated device that records sound simulates both halves;
- an old file that stopped loading is fixed in the import door; a model change OTIO cannot express
  goes into "What OTIO export loses";
- do not merge into `master`; report ONCE, in Russian, when done (no interim notes).

## Disk

- Check `df -h /media/e/developer` (not `/`) before a merge test. Borrow the target of the executor
  being merged; archive its session only after the merge test (the app deletes archived
  worktrees). Prune targets; remove merged worktrees.

## Reports to the owner

- In Russian, short, what he can now do or no longer suffers from; the version after a stage's
  `finish.sh`; for UI work the live screenshots.
- Name what is not done and why; never a session's note as proof — check the diff and the images.

## Current plan (2026-11-03)

The current stage: `export-throughput`, `loop-pitch-tail-dip`, the sidecar test race fix,
`sim-audio-pairs`. Then, in the owner's order:

1. **Performance**: the preview and the final render, and "the math" (AVX, Vulkan…):
   `export-throughput` (if not finished), `compute-backends`, the preview's frame path at full
   quality, 4K decode speed.
2. **Build and CI speed** ("все что больше 10мин+ это ужас"): the local build/test loop, the
   cost of a plugin edit, CI wall time (21 min on 2026-11-02: windows-build 13 min → windows-test
   7 min in series); the proposed-only change `ci-speedup` is the starting point.
3. **Usability**: control layouts like other editors (v26.10 L1/L2 — "Layout & Controls style",
   a customize button and a settings code), `preview-canvas-and-gizmos`, the small defects
   (`region-copy-of-selected`, `fader-wheel-and-region-edges`, the transparent tail at widths not
   divisible by 8, the vocoder level at start phase).

After them: the effects (`temporal-barrier-3dnr`, `chroma-key-upgrade`, `geometry-fx-flex`,
`pro-keyer`) and the big features to discuss (subtitles and text editing, ripple, scopes, several
frame formats, proxies, multicam, point tracker, magic-wand mask).

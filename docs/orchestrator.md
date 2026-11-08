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

- **A hint is not a reference** (2026-11-07): when he names an older build or another program in
  passing, that is not the bar to reach. Do not turn it into a target or a change.
- **One message is often several requests** (2026-11-07). Each request gets its own change, session
  and report. Never fold them into one executor because they look related.
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
- do not merge into `master`; report ONCE, in Russian, when done (no interim notes); a question
  for the owner goes into that report, addressed to the orchestrator, which carries it; the report
  ends with "Найдено, не исправлено" (see "What executors find and leave").

## Never let a stopped executor stand

2026-11-04: "Ты врешь. Трое не работали двое тупо меня ждали пока я не отвечу и мы потеряли из-за
тебя время. Запомни на будущее для оркестраторов." One executor stood 69 minutes on its questions
for the owner and another 50 minutes unmerged, while the last word to him was "three are working".

- **Silence proves nothing.** The app wakes the orchestrator only when NONE of its sessions still
  works. While one works, the note of a finished one waits for the owner's next message, and the
  orchestrator cannot see it.
- **Subscribe right after every start**: `SendMessage` with `notify_when_idle: true` and no message,
  addressed by the name and ref `ListAgents` prints (the short handle does not resolve). It is
  one-shot: subscribe again after sending it a follow-up, or once it is busy again; a subscription
  on a session already idle fires at once. A session id does not take a subscription, and a session
  still starting is not listed yet. The tool's reply must say "Subscribed"; a failure is fixed
  before the turn ends. Between notices, `list_sessions` is checked on every turn.
- **A watcher runs while any executor runs**: `python3 src/scripts/executor-watch.py <worktree
  names>` in the background; its exit wakes the orchestrator, which reads the report, handles it and
  starts the watcher again. An idle notice alone is not enough: it also fires on a pause with a
  background test run, and the real end then passes unseen (2026-11-04: `sim-audio-pairs` stood
  2 h 18 min, hours after this very section was written).
- **"Working" is a measured claim.** Any statement of who works comes from
  `mcp__ccd_session_mgmt__list_sessions` in the same turn and carries its time. An `isRunning: false`
  session is said to have stopped, with what it waits for. An earlier statement is never repeated.
- **An executor that ends with a question has stopped.** The orchestrator decides it, or carries it
  to the owner at once with its context and consequences. The owner is not expected to open
  executor sessions; a brief says that questions end in the report to the orchestrator.
- **A finished executor is handled in that turn**: its report read, its branch merged, the merge
  test started, the next slot filled.
- **A free slot is a defect.** The app refuses to start a session on a turn an event started, so on
  such a turn the next executor goes up as a card and the message says in one line what it waits
  for. On every turn the owner writes, free slots are filled first, from the stage's list; when
  the current stage's work no longer fills three slots, the next stage's work that touches other
  files starts early.

## What executors find and leave

2026-11-04: "а ты вообще читаешь что там агенты находят какие-то баги которые не трогают? Как бы нам
это надо собирать и обдумывать тоже.."

- **`docs/found-defects.md` is the one list** of what any agent saw and did not fix: what a person
  suffers, where, the evidence, the state, the verdict. A remark in a report, in a session's memory
  or on a proposal card does not count as recorded.
- **A run never reaches his desktop** (owner, 2026-11-07: a folder under `target/live-out/idle/xdg`
  opened in his file manager again and again). `util::open_in_file_manager` spawns nothing under
  `cfg!(test)` or `KT_NO_EXTERNAL_OPEN` (the live bench sets it and logs the path to `opened.txt`).
  Anything new that starts an outside program (a browser, a player, a file manager) goes through
  the same guard.
  2026-11-08: during `finish.sh`'s workshop tests his desktop session manager (`cinnamon-session`)
  aborted with SIGABRT and took the whole desktop, the Claude app and every executor with it; no
  memory shortage. Cause not proved, but the runs inherited his `DISPLAY` and session bus. Now
  `finish.sh` unsets `DISPLAY`/`WAYLAND_DISPLAY`/`XAUTHORITY` and sets
  `DBUS_SESSION_BUS_ADDRESS=disabled:`, and `kt_live.py` gives the program no session bus either.
  Every brief says: run `cargo test`/`nextest` as
  `env -u DISPLAY -u WAYLAND_DISPLAY -u XAUTHORITY DBUS_SESSION_BUS_ADDRESS=disabled: <cmd>`.
- **After a merge, build the binary too** (2026-11-08): a conflict resolved by keeping both lines
  dropped a `#[cfg(test)]`, the test build passed and `cargo build -p kt-app` failed; two executors
  found it. Every merge check runs `cargo build -p kt-app -p kt-cli` before the tests.
- **A runaway test kills only itself** (2026-11-07, 20:55: a new test of `cached-range-plays`,
  `mix_player`, took 55 GB of 61 with no swap; the OOM killer took the Claude app and all three
  executors with it). Every brief says: heavy or new test binaries and the program on a big project
  run under a memory cap, `systemd-run --user --scope -p MemoryMax=12G -p MemorySwapMax=0 <cmd>`.
- **Discussion is not a go** (owner, 2026-11-07, stopped a start: "я не говорил ИДТИ ДЕЛАТЬ. МЫ НЕ
  ОБСУДИЛИ ЕЩЕ ДО КОНЦА!"). An executor starts only on his explicit word to start; an answer, a file
  or a folder he sends is part of the discussion, not that word. Every brief carries the class rule
  of CLAUDE.md: the complaint's mechanism, then every subsystem where it can act; never a scope
  named after the one place he gave as an example.
- **Every executor's report ends with "Найдено, не исправлено"**, or the word "ничего". A report
  without the section goes back to the executor before the merge.
- **The merge is not finished until the list is updated**: each item gets a row in the turn its
  branch is merged, or the row of an item the merge fixed moves to `fixed`.
- **Thought over, not only collected.** At the end of each stage the orchestrator gives every open
  row a verdict (fix in the coming stage, fix later, not a defect) and reports to the owner what it
  decided; a person-visible defect goes into the next stage's plan unless he says otherwise.

## Disk

- Check `df -h /media/e/developer` (not `/`) before a merge test. Borrow the target of the executor
  being merged; archive its session only after the merge test (the app deletes archived
  worktrees). Prune targets; remove merged worktrees.

## Reports to the owner

- In Russian, short, what he can now do or no longer suffers from; the version after a stage's
  `finish.sh`; for UI work the live screenshots.
- Name what is not done and why; never a session's note as proof — check the diff and the images.

## Current plan (2026-11-06)

Stage closed: `finish.sh` at kt26.11.07-677c2f5b, master there: 1588 of 1588, 79 of 79 live, 578 s. It
adds `idle-cpu` to the stage of kt26.11.06-b42cbff4, which held `gpu-render-hardening`, `render-speed-no-card` (a render with no card beats
the deleted CPU build), `control-layouts`, `test-stability`. Next, in the owner's order:

0a. **Now (owner, 2026-11-07 evening): maximum performance of everything**, "невозможно пользоваться
   программой". Running: `ui-frame-cost`, `cached-range-plays` (merging), `telemetry-journal`.
   Queue by his words: `edit-without-rebuild` (found-defects 110, 111, 117: a real edit, a loop
   region or a meter read rebuilds the whole audio engine, 412-447 ms on his project), then
   `audio-fx-cost` (every audio effect against real time). Deferred: `window-content-reach`, the
   VHS audio dropout as a level dip (his choice b), any change to an effect's substance.
0. **Out of turn, the owner, 2026-11-07** (`kkt-26.11.07`), each its own change, each on his word:
   `idle-cpu` (E1, merged; the code of E2, E3, E9, E11 came with it), `hover-static-load` (E2), `scroll-position-cost` (E9),
   `load-proof-tool` (E3), `hover-composition-load` (E4), `fx-window-load` (E10), `idle-load-guard`
   (E11), `ui-never-waits` (E8), `fx-preset-change` (E6), `fx-form-width`
   (E7). `ctrl-s-writes-nothing` (row 47) waits for his word too.
2. **Performance**: rows 93 (preview on the VHS span), 94, 96, and the older 10, 37, 42, 52, 53, 55, 72.
3. **Build and CI speed**: logs of a real master run and the first `windows-picture` and FXC result,
   both need the owner's push; the plugin test-build cost (card); an ARM build (owner, 2026-11-04).
4. **Usability**: `preview-canvas-and-gizmos` (card, B9),
   the small defects (`region-copy-of-selected`, `fader-wheel-and-region-edges` cards, the transparent
   tail at widths not divisible by 8, the vocoder level at start phase).

5. **The owner's answers of 2026-11-07** (`docs/user-feedback.md`): the VEGAS tile apart from ours,
   the fuller settings code, other interface-scale keys, the marker split, the warning before a
   render, the Noise default, `.ktren`, the tests' temp files in tmpfs. He said to start nothing yet.

After them: the effects (`temporal-barrier-3dnr`, `chroma-key-upgrade`, `geometry-fx-flex`,
`pro-keyer`) and the big features to discuss (subtitles and text editing, ripple, scopes, several
frame formats, proxies, multicam, point tracker, magic-wand mask).

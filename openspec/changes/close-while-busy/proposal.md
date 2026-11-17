## Why

The owner's claim T1 of `kkt-26.11.18`: "закрытие программы при catching up не срабатывает приходится
убивать процесс" — on release kt26.11.17-2abc3af9, closing the window while a loop catches up does not
end the program; he kills it.

His trace (`fails/26.11.18/new`) shows why it could not be answered from the trace, and what the close
waited for:
- the trace holds NO word of the close: nothing recorded it, and the trace writer was never flushed at a
  clean exit (the last 25 ms batch was lost). All three of his runs end without "the program closes";
- the prefetch thread's passes ran up to 37 s (a Datamosh `stream_advance` up to 18 s, a compute job of
  2.2 s, admissions waiting up to 4.9 s), and `Player::drop` joined it with no bound; the decoder's join
  waited for a whole span (up to 64 pictures); the sound device's thread, the effect-cell lane and
  kt-gpu's exit hook waited with no bound either. A close blocked there leaves the window frozen.

## What Changes

- Every wait of a close is BOUNDED and RECORDED (`kt_time::shutdown`): a thread is joined if it ends by
  its deadline (2 s), else left behind; each wait writes a `Shutdown` trace record (who, how long, ended
  or left). `Player::drop` (engine-build, audio-render, prefetch, every media worker), `AudioOutput`,
  `FxThumbService`, kt-gpu's exit hook (10 s), `on_exit`'s autosave, save, poster and offline-engine
  waits go through it.
- A decoder stops between two chunks of its span once its worker closes; the render-ahead producer's
  stream history walk stops when its lane closes (the player's drop closes it).
- The close is named: the journal's `close asked` and a `Shutdown` record when the close goes on, the
  durability point, eframe's return; the trace is flushed after `on_exit` and after eframe returns.
- **The exit is bounded once the work is safe**: after `on_exit` wrote the recovery file (durability is
  untouched and comes first), a watcher gives the rest of the exit `EXIT_BOUND` (8 s); past it, it names
  what was still waited for, flushes the trace and ends the process with `_exit(0)` (not `exit`: its C
  hooks wait for device work and run LLVM's destructors under drawing threads).
- `kt-cli trace … shutdown`: the close, every wait, a forced exit, what each thread did after the close
  was asked; a run with no close says so.
- `File → Quit` publishes its rectangle (`file.quit`) for the live scenario.

## Capabilities

### Modified Capabilities
- `durability`: the close of a busy program is bounded after its durability; open/new release the old
  project the same bounded way.
- `telemetry`: the close is in the trace (`Shutdown`, `kt-cli trace shutdown`), flushed at exit.
- `transport`: a player's drop is bounded; its threads and decoders stop at their next check.

## Impact

kt-time (`shutdown`, `Ev::Shutdown = 120`, `Ctx::cancel_check`), kt-engine (player drop, worker, render
ahead), kt-gpu (exit hook), kt-app (`exit_watch`, `on_exit`, `ui` close record, audio output, effect
cells, `file.quit`), kt-cli (`trace shutdown`). Live scenario `close_busy`. Tests t2950–t2954.

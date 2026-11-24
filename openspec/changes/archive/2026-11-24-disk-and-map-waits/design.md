## Decisions

**D1. The journal's line is stamped at the call and written by a thread of its own, in order.** It is
opened, rotated and appended on the thread `journal`; the caller only pushes. Bounded at 4096 lines, a
drop is said in the file. Evidence of the defect, `track` of the stage-24 finish: the trace has
`MoveClip` at +84.150 and one UI frame of 5213 ms, 0 ms of it waiting for a processor; the telemetry
journal's record of the same line, taken right after the write, stands at +89.363. The stall is the write.
Rejected: a line dropped when the disk is busy (the journal exists for exactly that run).

**D2. A small file of the store is pushed with its text; the newest of a path wins, and a read sees what is
on its way.** `File → Open` builds a new `KtApp` that reads the workspace again: without the read-through it
would read the former text. The workspace keeps its fsync, the records keep tmp + rename without one.

**D3. Who flushes:** `on_exit` (1 s each), the panic hook (300 ms, the journal), the forced exit (500 ms),
the return of `run_native` (a window that never opened says why). A native crash flushes nothing: a line
said in the last milliseconds before it may be missing from `kt-app.log`, as from the telemetry journal.

**D4. The class, walked (every `fs::` write kt-app makes):**

| where | on the UI thread | verdict |
|---|---|---|
| `journal::log` (refusals, warnings) | yes, per line | moved (D1) |
| `user_store` workspace | yes, after each save and at exit, two fsyncs | moved (D2) |
| `user_store` recent, templates, tour, start | yes, per open, lesson step, preference | moved (D2) |
| `user_store` effect and render presets | yes, on Save/Delete preset and at a render's start, two fsyncs; their error is shown | found-defects 1002: needs an answer the windows wait for |
| `ffmpeg_setup` the chosen folder | yes, once, after Locate… | found-defects 1002 |
| `save_flight_log` | yes, on its own menu item | found-defects 1002 |
| `project_lock` | on the media lane's open | not the UI thread |
| project, autosave, recovery | save and background lanes | done before (one-scheduler) |
| posters, thumbnails, `telemetry`, `update_check`, `tele_journal`, trace | threads of their own | not the UI thread |
| `timeline::write_dump` | a thread of its own | done before (ui-never-waits) |
| `load_watch` dump | its own thread, rig only | not the UI thread |

**D5. The lost map request.** Reproduced on an idle machine: a map asked 16–32 ms into openbox's start is
lost for good. The evidence of 830 matches: the window exists, unmapped, not reparented, and the program
drew 10 frames. A desktop's session can start programs beside its window manager, so the program asks
again itself: a look a second, at most 120, at its own window (`_NET_WM_PID` and the class) among the
root's children; unmapped and without `WM_STATE` is lost. Wayland and Windows have no such request.
The rig's 0.5 s sleep became a probe window it waits to see taken.

**D6. t1913 (class of 825/827/828):** the bound was ×1.5 of 1.8 ms, and a loaded machine charges a thread
for the caches and the sibling core it takes: ×1.9 seen at load ~300. The scenes are ten times larger, so
the effect stands past the noise; per event the new bounds are the stricter ones.

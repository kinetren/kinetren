# The interface waits for no small file; a window the window manager lost is asked for again

## Why
Found-defects 833 (`track`): a locked track's refusal held the interface for 5.2 s — the line of
`kt-app.log` was written on the UI thread, and the disk was busy. Found-defects 830 and 954: three
scenarios waited ten minutes for a window that was asked for and never managed: openbox drops a map
request that comes while it starts. Found-defects 176: t1913 red under load.

## What Changes
- The journal's lines and the store's small files (workspace, recent list, templates, tour, start
  choice) are written by threads of their own; the exit, a panic and a forced exit flush them within a bound.
- On X11 the program looks for its own window until a window manager has taken it and asks again.
- The rig waits until openbox takes a window before it starts a program; t1913 and t1914 measure scenes
  whose effect stands past the noise.

## Impact
Specs `durability`, `ui-timeline`. Code: kt-app `journal.rs`, `later.rs`, `user_store.rs`, `map_watch.rs`,
the live rig, `frame_cost_tests.rs`.

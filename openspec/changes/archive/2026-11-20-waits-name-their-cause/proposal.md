# Proposal

## Why

- **found-defects 824**: a 4-worker live run waited the whole 600 s guard on four stands with an empty
  log; nothing said whether the program had died, never reached its display or was still starting.
- **found-defects 760, 781, 782, 783**: kt-app tests red under load and green alone — t1644, t2380,
  t1621. One mechanism: the test waits a fixed time, or reads a flag the code sets BEFORE its last
  designed step, and then asserts a state the code reaches only after that step:
  - t2380: after `open = false` the output thread makes one more designed pass (a pause's check
    `PAUSE_CHECK` later, or the pass after a close); the test's 300 ms quiet window started at the
    flag, so that pass landed in it; and "50 ms after start" was taken for "the thread started";
  - t1644: the lane's thread lets its copy of the service's state go after it reports idle; the test
    took the count of that `Arc` for the drop's outcome;
  - t1621: a held Space plays once the hold is PROVEN — a frame that began the threshold after the
    press, the threshold widened by how busy the frames are; a fixed 330 ms and two frames was a
    claim about the machine's speed.

## What Changes

- **The live rig names why no window came**: `scripts/live/_why.py` — the display, the pid and the
  program's first log line at the start; while waiting, every 30 s, and on giving up: the program
  ended (its code), the editor child ended under a standing supervisor, not connected to its
  display (XRes client pids), or connected and still starting. A dead program ends the wait at once.
  The stand (`worker.py`) and a scenario's own process (`kt_live._wait_window`, `the_window`) use it.
- **The three tests wait for the consequence**: t2380 waits until the thread started and, after the
  close, until it settled (no pass over 100 ms) before measuring the quiet; t1644 reads the drop's own
  record (`shutdown::left_behind`) for the branch and waits for the worker's state to go; t1621 steps
  until playing and asserts it did not play before the threshold — every wait bounded by the test's
  guard.

## Capabilities

### Modified Capabilities

- `ui-reach`: the live rig names why no window came.
- `distribution`: a test waits for the code's last designed step, not for a flag before it.

## Impact

`scripts/live/{_why,worker,kt_live}.py`; kt-app `audio_out.rs`, `fx_thumbs.rs`, `layout_tests.rs`
(tests only); kt-cli `dist_guard.rs` (t3239).

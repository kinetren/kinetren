# Tasks: ui-frame-cost

- [x] 1.1 Profile a UI frame on his relinked project, release build: pointer over the preview, over the timeline at two zooms, playback, the effects window open (gdb sampling, `frame_cost`)
- [x] 1.2 The borrowed document `TimelineModel::doc`/`doc_arc`, one per state (t1911); every drawing path that copied it reads it
- [x] 1.3 `ClipWarp` lays its speed integral out once, bit for bit (t1910); the composite reads it too
- [x] 1.4 The event index per state: event pass, coverage strip, tails, hit test, handles, transitions (t1912, t1913)
- [x] 1.5 The virtual bin grid, the bin's list and placement per state, group members by id
- [x] 1.6 Windows: effects, camera, composition, transition, render, settings, published (t1914)
- [x] 1.7 `frame_cost`: stages in Diagnostics, the load report and the bench dump
- [x] 1.8 Live scenario `ui_frame` on the heavy showcase
- [x] 1.9 The class walked and written down (design.md); measured before/after on his project

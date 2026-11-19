## Decisions

### D1. The poster reads what the preview holds

At `queue_poster` (Ctrl+S, a close) the app takes `Player::kept_picture_reader(at)`: a closure over the
player's `PreviewParts` and the version asked now. On the `posters` lane it builds the probe plan of
the latest build (`probe_plan`, no stage run), asks the display's executor for the key's composite
(`read_kept`, a readback that waits for the device — never on the UI thread) and, failing that, the disk
tier (`disk_key`). A build that is not the version asked gives nothing: an edit in flight would show the
moment before it. The picture is the one the preview showed (the same plan key), at the preview's size,
fitted to the poster's 320×180 by `fit_into`; nothing found — `render_poster` as before.

### D2. The class walked

The mechanism: "a background job composes a moment the preview already composed". Checked:
- posters — changed (D1);
- the render-cache prerender — it already asks the store before composing (`one`: `store.contains`);
- thumbnails and filmstrips — decoded source frames, not composites; no overlap;
- effect cells (`fx-thumbs`) — a chain with a candidate effect over a sample or the event's frame: a
  picture the preview never shows;
- reel strips and posters of reels (`reel_preview`) — a reel alone at its own time and size, not the
  composite on screen;
- the export — the project's size, every moment; the disk tier is excluded by design (no export reads it).

### D3. A test process's native death leaves a report

`plugins::install` (every `KtApp` constructor, every harness test) installs `kt_crash` once under
`cfg(test)`, the raw report in the temp directory (never the person's data). The exit crashes of rows
695/783/784 did not reproduce here: 60 plain runs of t578 at 15 in parallel, 100 runs of t553/t572/t573/
t578 under gdb at 20 in parallel, and a probe that exits while four threads draw on llvmpipe (12 runs) —
none died. The next one names its thread and stack.

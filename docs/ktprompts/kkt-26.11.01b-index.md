# Registry of the owner's list `kkt-26.11.01b`

Source — [`kkt-26.11.01b.txt`](kkt-26.11.01b.txt) (verbatim, the owner's second message of
01.11.2026: ten requests). Columns and states — as in [`v26.10-index.md`](v26.10-index.md). The
round is in `docs/user-feedback.md` ("2026-11-01 — the second list").

A change name in the Address is the planned name; the orchestrator runs three executors at once
(the owner, 2026-11-01), so the Stage gives the queue position until the change is proposed.

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| B1 | 2 | rendering the region `rend` of `fails/26.11.01/render_crash` (or any 0.2 s piece) reaches 100 % and the program dies; the file may even be written. Found while registering: the tests write into the owner's own journal `~/.local/share/kinetren/kt-app.log` (t959 lines), which rotates his real records away | done render-finish-crash, t1312 + live `renderregion` + t1310, t1311: the crash is the panic of 2026-10-31 (the window's remaining time went negative on an off-grid region), fixed by render-crash-and-recovery after his build a04b201d; held end to end by a real render through the window; tests write into their own data folder, never the person's | `render-finish-crash` | render-finish-crash |
| B2 | 3 | the chroma key has no eyedropper: pick the key colour from the preview frame or anywhere on the screen | no: feature, no note → video-fx | `chroma-key-upgrade` | queue 4 |
| B3 | 4 | the chroma key is poor and of bad quality; an auto-analysis button instead of picking every value by hand | no: feature, no note → video-fx | `chroma-key-upgrade` | queue 4 |
| B4 | 5 | a professional keyer as a new FX (AI or another proper algorithm): shadows, hair, an uneven screen, green spill and bounce light on the subject | no: feature, no note → video-fx | `pro-keyer` | queue 7 |
| B5 | 6 | speech to text, and not a window that dumps the event's text: walk the text, navigate it, search it | no: feature, no note → joins the taken ★14 | `subtitles-and-text-editing` | with ★14 |
| B6 | 7 | Mirror has no reflection angle (Vegas has one) — only Kaleidoscope gets near; EventGrid and the other legacy effects have no flexibility | no: feature, no note → video-fx | `geometry-fx-flex` | queue 5 |
| B7 | 8 | clone into a grid, mirror vertically, mirror that reflection horizontally — not possible with the current effects; we reach neither AE nor Vegas | no: feature, no note → video-fx | `geometry-fx-flex` | queue 5 |
| B8 | 9 | a repeat-texture effect to slide the texture back and forth (a "saw" from the top half after a mirror) | no: feature, no note → video-fx | `geometry-fx-flex` | queue 5 |
| B9 | 10 | editing a pan's zoom in the preview: no zooming the preview out to reach a large zoom, no rotation; no proper on-screen control of an effect | no: feature, no note → ui-timeline, video-plan | `preview-canvas-and-gizmos` | queue 6 |
| B10 | 11 | copying the part of an event under the region: with the event highlighted the whole event is copied (the rule of v26.10.18 item 4 — "one selected event wins over the region") | no: defect of an earlier rule → ui-timeline | `region-copy-of-selected` | queue 3 |

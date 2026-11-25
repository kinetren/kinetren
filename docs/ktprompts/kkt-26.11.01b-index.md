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
| B2 | 3 | the chroma key has no eyedropper: pick the key colour from the preview frame or anywhere on the screen | done `chroma-key-eyedropper` (F-MSK-007), t3120, t3121, t3122, live `eyedropper`: a pipette beside every colour of the program reads the screen under the next click (the preview's frame, a thumbnail); outside the program's window and on Wayland not — noted | `chroma-key-eyedropper` | features-slow |
| B3 | 4 | the chroma key is poor and of bad quality; an auto-analysis button instead of picking every value by hand | no: feature → F-MSK-004, change `chroma-key-upgrade`; the eyedropper is in (`chroma-key-eyedropper`) | `chroma-key-upgrade` | designed, implementation-ready (build queue) |
| B4 | 5 | a professional keyer as a new FX (AI or another proper algorithm): shadows, hair, an uneven screen, green spill and bounce light on the subject | no: feature → F-MSK-009, change `pro-keyer` | `pro-keyer` | designed, implementation-ready (build queue) |
| B5 | 6 | speech to text, and not a window that dumps the event's text: walk the text, navigate it, search it | no: feature → F-TXT-004, change `subtitles-and-text-editing` | `subtitles-and-text-editing` | designed, implementation-ready (build queue) |
| B6 | 7 | Mirror has no reflection angle (Vegas has one) — only Kaleidoscope gets near; EventGrid and the other legacy effects have no flexibility | no: feature → F-FX-019, change `geometry-fx-flex` | `geometry-fx-flex` | designed, implementation-ready (build queue) |
| B7 | 8 | clone into a grid, mirror vertically, mirror that reflection horizontally — not possible with the current effects; we reach neither AE nor Vegas | no: feature → F-FX-019, change `geometry-fx-flex` | `geometry-fx-flex` | designed, implementation-ready (build queue) |
| B8 | 9 | a repeat-texture effect to slide the texture back and forth (a "saw" from the top half after a mirror) | no: feature → F-FX-019, change `geometry-fx-flex` | `geometry-fx-flex` | designed, implementation-ready (build queue) |
| B9 | 10 | editing a pan's zoom in the preview: no zooming the preview out to reach a large zoom, no rotation; no proper on-screen control of an effect | no: feature → F-UI-011, change `preview-canvas-and-gizmos` | `preview-canvas-and-gizmos` | designed, implementation-ready (build queue) |
| B10 | 11 | copying the part of an event under the region: with the event highlighted the whole event is copied (the rule of v26.10.18 item 4 — "one selected event wins over the region") | done `region-crops-copy`, t3105 (t926 changed by this decision), live `region_crops_copy`: a region that crops the one selected event copies its part under the region, nothing of the other events; a covering or distant region keeps the whole event | `region-crops-copy` | features-slow |

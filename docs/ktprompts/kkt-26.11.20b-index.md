# Registry of the owner's list `kkt-26.11.20b`

Source — [`kkt-26.11.20b.txt`](kkt-26.11.20b.txt) (verbatim, his answers of 20–22.11.2026 to the second round of design
questions after stage 22). Columns and states — as in [`v26.10-index.md`](v26.10-index.md).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| B1 | 6–7 | the former proxies are named "Persistent cache" ("Постоянный кэш") and copy heavy files — and above all they keep the results of HEAVY EFFECTS on disk, not only decoded sources: "У НАС СУТЬ ПО БОЛЬШЕЙ ЧАСТИ В КЭШИРОВАНИИ ЧТОБЫ ЭФФЕКТЫ ТЯЖЕЛЫЕ НЕ СЧИТАТЬ" | no: feature, change `disk-copies` redesigned as one persistent cache of effect results and heavy sources, joined with the disk prerender (`render-cache-disk`) | spec-features with spec-engine | 23 |
| B2 | 8–9 | one cache folder per project folder; a folder that cannot take more writes cyclically, dropping the oldest, with an alert in the notices | done: `project-cache-folder` (Q1 a; the cyclic rule; t3824–t3843, t3808, live `cache_folder`) | spec-engine | 23 |
| B3 | 10–12 | the download list comes from our signed server for everything; the library now, plugins with the wasm host; a missing font pack draws a marked placeholder with Download | no: feature, change `download-library` (Q1 b, Q2 a, Q3 a) | spec-engine | 23 |
| B4 | 13–15 | 16-bit path: 1.0 = SDR white 203 cd/m²; an 8-bit effect runs marked "8 bit"; HDR goes to an HDR monitor at once where the system can, export SDR + HDR10 + HLG | no: feature, change `hdr-sources-to-sdr` (Q1 a, Q2 a, Q3 b, Q4 a) | spec-engine | 23 |
| B5 | 16 | auto-ducking keyed by SEVERAL tracks, hand edits useful too; his original item was cut short in the question — the design restores it whole (v26.10 lines 83–87: sections intro/title/content, the background up or muted and the title's sound unmuted during a title) | no: feature, change `auto-ducking-by-role` revised from his full item | spec-features | 23 |
| B6 | 17 | loudness normalization at the PROJECT level, not in render presets, by the professional path for normalizing tracks | done `loudness-norms`, t3919, t3920, t3922, t3923, t3966, t4200, t4201, t4202, t4203, live `render_norms`: the target in Project settings, "Match loudness…" for events and a track, the program stage under the meters with "Match the program" | spec-features | 23 |
| B7 | 18–19 | the fisheye-on-speed item means the EDIT's speed (a speed curve on a composition, morpher or event), not the playhead's motion; preview and render agree | no: feature, change `motion-driven-params` redesigned around the outer time map's rate | spec-features | 23 |
| B8 | 20 | effects on a word: select it in the text field, the range follows edits; twenty words selected one by one must be designed for from the start | no: feature, change `title-word-effects` (Q1 a + many ranges) | spec-features | 24 |
| B9 | 21 | dragging a window by the preview is dropped | done `window-chrome`, t3937: built without the preview drag (Q1 withdrawn) | spec-features | 23 |
| B10 | 22 | real 3D: intersecting planes and extruded shapes now, models and lights later; a track switch makes the group | no: feature, change `real-3d-groups` (Q1 a, Q2 a) | executor | 25 |
| B11 | 23–24 | wasm plugins on wasmtime, video plugins give WGSL; layered-scene exchange a design of its own later | no: feature, change `wasm-plugin-host` (Q1 a, Q2 a, Q3 a) | executor | 25 |
| B12 | 25–26 | the bottom counter: resident memory plus VRAM, nothing else, in GB, the hover expands the rest; the project's disk = only the project's stores | no: feature, change `memory-breakdown` (Q1 revised, Q2 a) | executor | 24 |

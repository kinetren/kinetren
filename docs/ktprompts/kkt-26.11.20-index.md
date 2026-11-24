# Registry of the owner's list `kkt-26.11.20`

Source — [`kkt-26.11.20.txt`](kkt-26.11.20.txt) (verbatim, his answers of 20.11.2026 to the questions Q1–Q54 of
`docs/owner-checks.md`, after stage 22, release kt26.11.20-ea0aff6f). Columns and states — as in
[`v26.10-index.md`](v26.10-index.md).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| A1 | 4 | Edit menu and bin: one "Create a generator…" item, the kind by the selected track (video → solid colour, audio → tone), none selected → solid colour | done `generator-menu-one-item`: t3712, t1164, t3713 (a kind change keeps the sound pair), live `genaudio`, `lessons` | `generator-menu-one-item` | 23 |
| A2 | 5 | "Render to MP4…" becomes "Render…" | done `render-settings` follow-up (branch render-menu-name): t3711 (every catalog; zh and ar said "Export…" beside the Export submenu), live `lessons` clicks File › Render… | `render-menu-name` | 23 |
| A3 | 6 | export default: BT.709 at HD and above, BT.601 below, as the decode | no: defect, capability `export` (found-defects 1054) | executor mosh-sharp | 23 |
| A4 | 7–8 | an effect's preset replaces the whole effect (curves too, Undo returns); a chain preset is joined to the existing chain on the graph, the extra nodes selected and deleted with the mouse | no: feature, change `fx-preset-change` (F-UI-010) revised by the decision | spec-features, then an executor | 23 |
| A5 | 9–10 | graph v2: picture branches first, no inputs from other layers in v2 | no: feature, change `fx-graph-dag` (Q3 a, Q4 a) | executor fx-graph | 23 |
| A6 | 11 | Slowed, Datamosh and pitch allowed inside a branch at the cost of a second read | no: feature, change `fx-graph-dag` (Q5 b) | executor fx-graph | 23 |
| A7 | 12 | the Datamosh frame shares one device with the window now, no measurement first | no: defect, capability `video-plan`, change `stream-picture-stays` (Q51 b) | executor mosh-sharp | 23 |
| A8 | 13 | scopes in their own window, the preview frame while playing, a full one on a pause | no: feature, change `scopes` (Q6 a, Q7 a) | executor | 24 |
| A9 | 14 | ripple across an edit point refuses and highlights the obstacle; he does not understand the case yet and asks to be shown it later | question: does the ripple refusal read right when he tries it? (owner-checks C9) | change `ripple-and-trim-modes` | 24 |
| A10 | 15–18 | the "proxies" become an adaptive durable store at the preview's quality, a better quality rendered when chosen, a switch "as in the preview" beside; never named "proxy", nor bare "cache" (it must read as not in memory) | no: feature, change `proxies` redesigned and renamed `persistent-cache` (was `disk-copies`) (name pending the owner, Q1 there) | spec-features | 23 |
| A11 | 19 | subtitles: whisper.cpp pinned by sha256, the Text tab beside the bin, fonts by click; everything downloaded becomes one library, like plugins, which can be downloaded too — a plugin library | no: feature, change `subtitles-and-text-editing` (Q14–Q16 a) and a new design `download-library` (FFmpeg, whisper, fonts, plugins) | spec-engine | 23 |
| A12 | 20 | decoding on the card: on by default, with a checkbox in Settings | no: feature, change `device-decode-sources` (Q17 a, Q18 b) | executor | 24 |
| A13 | 21 | export through dma-buf into VAAPI built now | no: feature, change `export-host-copy` (Q19 b) | executor mosh-sharp | 24 |
| A14 | 22 | the Datamosh history on disk in LZ4 (he asks why it is not compressed when Datamosh itself is compression), and the caches leave the one sqlite base for a subfolder of caches in the project folder, the preview prerender among them | no: feature, change `durable-history-and-mix` revised (Q23 b, Q24 b) and a new design `project-cache-folder`; the history half replaced by kkt-26.11.22 D6 (no history on disk) | spec-engine | 23 |
| A15 | 23 | seek into a reverb tail: always compute the full tail | no: feature, change `track-stage-preroll` (Q25 a) | executor | 24 |
| A16 | 24–25 | a "draft render" option takes the preview's prerender; the cached render reuses earlier renders with its index inside the video (SEI) | no: feature, change `cached-final-render` (Q20 b, Q21 a, Q22 b) | spec-engine revises, an executor builds | 24 |
| A17 | 26–27 | frame formats: each event by its own geometry; platform safe-area presets | no: feature, change `frame-format-variants` (Q26 a, Q27 a) | executor | 24 |
| A18 | 28–29 | multicam: a composition of angles, one sound by default plus an "audio follows the angle" option, the grid in the preview | no: feature, change `multicam` (Q28 a, Q29 a + option, Q30 a) | executor | 24 |
| A19 | 30 | shared events built now | no: feature, change `shared-events` (Q31 b) | executor | 24 |
| A20 | 31 | point AND planar tracker now, the defaults the most convenient and exact; the wand algorithmic now, a neural one later | no: feature, changes `point-tracker` (planar joins) and `magic-wand-mask` (Q34 a, Q35 a) | spec-features revises | 24 |
| A21 | 32 | 3DNR trail capped at 64 frames, rolling shutter and flicker in the same change | no: feature, change `temporal-barrier-3dnr` (Q36 a, Q37 a) | executor | 24 |
| A22 | 33 | every chroma key moves to the new model (old projects look different); Analyze over five frames, a clean plate per frame, light wrap later | no: feature, changes `chroma-key-upgrade` (Q38 b) and `pro-keyer` (Q46 a, Q47 a) | executor | 24 |
| A23 | 34 | Datamosh on a track: the cut at the incoming event, a gap ends the smear, an event's own Datamosh allowed | no: feature, change `track-stream-stage` (Q40–Q42 a) | executor | 24 |
| A24 | 35 | Mirror: old ones keep flipping, a new one reflects; the rest of the geometry audit later | no: feature, change `geometry-fx-flex` (Q44 a, Q45 a) | executor | 24 |
| A25 | 36 | HDR: the project's (composition's) setting decides — on, an f16 path that shows HDR right; off, the fast 8-bit path | no: feature, change `hdr-sources-to-sdr` redesigned around a per-project working depth | spec-engine | 23 |
| A26 | 37 | preview zoomed out: outlines only, the zoom not remembered | no: feature, change `preview-canvas-and-gizmos` (Q53 a, Q54 a) | executor | 24 |

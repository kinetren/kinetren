# Registry of the owner's list `kkt-26.10.27`

Source — [`kkt-26.10.27.txt`](kkt-26.10.27.txt) (verbatim, the owner's list `kkt.txt` of 27.10.2026).
Cross-check — `openspec/changes/r71-ktprompts-index/design.md` §3c (state as of 28.10.2026). The file
numbers two items "3"; the registry numbers the program items K1–K9 in file order and the site items
W1–W4. Columns and states — as in [`v26.10-index.md`](v26.10-index.md).

Overlaps with `v26.10` are named, not duplicated: K5 takes U1 (ask before overwriting) and T12 (stats
after finish); K6 takes T16 (the h264_vaapi crash); K7 takes F1/F2; K8 takes P1–P6; K9 sits next to the
help notes F-HLP-023/027/028/029.

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| K1 | 1–2 | wine: Play on a loop region freezes the picture until a click | no: defect → transport | `loop-play-latency-hold` | loop-play-latency-hold |
| K2 | 4 | the morpher window grows wider forever on another system | no: defect → ui-reach | `shared-window-shell` | shared-window-shell |
| K3 | 6 | `kinetren-mime.xml` is not fully translated | no: defect → distribution | `i18n-gaps` | i18n-gaps |
| K4 | 8 | the FFmpeg download window says nothing (speed, where) | no: feature, no note → media-index | `ffmpeg-download-window-info` | ffmpeg-download-window-info |
| K5 | 10 | the render window is poorer than the C++ one | no: feature, no note → export | `render-window-v1-parity` (takes U1, T12 of v26.10) | render-window-v1-parity |
| K6 | 12 | a Flatpak release with hardware encoding in render | no: feature, no note → distribution, export | `hw-encode-context` → `flatpak-flathub` (takes T16 of v26.10) | hw-encode-context |
| K7 | 14 | effects and transitions lag far behind CapCut | no: feature, no note → video-fx, transitions | `effects-transitions-catchup` (after K8; takes F1/F2 of v26.10) | effects-transitions-catchup |
| K8 | 16 | the plugin system first: build time, one interface | no: feature → F-EXT-003 | `plugin-api-stage1` (takes P1–P6 of v26.10) | plugin-api-stage1 |
| K9 | 18 | no guide inside the program | no: feature → F-HLP-028 | `welcome-window-and-tour` | welcome-window-and-tour |
| W1 | 29 | the site: download buttons next to the big heading | no: site → other repository | `kt.9.site` — not indexed by this guard beyond the row | other repository |
| W2 | 30 | the site: links for the visitor's OS, older releases, what's new | no: site → other repository | `kt.9.site` | other repository |
| W3 | 31 | the site: several rotating slogans | no: site → other repository | `kt.9.site` | other repository |
| W4 | 32 | the site: the "video effects" tab slides under the next heading | no: site → other repository | `kt.9.site` | other repository |

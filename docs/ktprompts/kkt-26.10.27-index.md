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
| K1 | 1–2 | wine: Play on a loop region freezes the picture until a click | partial: the fix is in the tree (`loop-play-latency-hold`: the release follows the display lap, t900, t901, t904); its probe and his repeat on wine with a sound device are left (tasks 1.3, 3.5) | `loop-play-latency-hold` | awaits his wine (owner-checks C15) |
| K2 | 4 | the morpher window grows wider forever on another system | done `shared-window-shell`, t918, t923, live `winshell`: no window grows by itself, the owner's morpher scene at zoom 1.1/1.2 | `shared-window-shell` | shared-window-shell |
| K3 | 6 | `kinetren-mime.xml` is not fully translated | done `i18n-gaps`, t930, t931: every catalog language is in the desktop and MIME files | `i18n-gaps` | i18n-gaps |
| K4 | 8 | the FFmpeg download window says nothing (speed, where) | done `ffmpeg-download-window-info`, t950, t951, t952: the window says source, speed, time left and where it went | `ffmpeg-download-window-info` | ffmpeg-download-window-info |
| K5 | 10 | the render window is poorer than the C++ one | done `render-window-v1-parity`, t954, t956, t957, t958: the encoder named, rows from the run, counters stop, Replace asked | `render-window-v1-parity` (takes U1, T12 of v26.10) | render-window-v1-parity |
| K6 | 12 | a Flatpak release with hardware encoding in render | partial: `hw-encode-context` done (t961, t964, t25); the Flatpak release is `flatpak-flathub`, stopped at his Q75 | `hw-encode-context` → `flatpak-flathub` (takes T16 of v26.10) | hw-encode-context |
| K7 | 14 | effects and transitions lag far behind CapCut | partial: the catalogue and its guards are in (`effects-transitions-catchup`, t978, t979, t980). Its seven waves are all designed now: W1 `fx-lens-and-camera` (revision 2, may start), W2 `fx-looks-w2`, W3 `title-motion-slots` (words: `title-word-effects`), W4 `speed-ramp-presets` (beat and zoom by sound: `edit-analysis`), W5 `transitions-w5`, W6 `fx-buffer-and-relief`, W7 `voice-presets` (2026-11-25) | `fx-lens-and-camera`, `fx-looks-w2`, `title-motion-slots`, `speed-ramp-presets`, `transitions-w5`, `fx-buffer-and-relief`, `voice-presets` | designed, implementation-ready (build queue) |
| K8 | 16 | the plugin system first: build time, one interface | done plugin-api-stage1, plugin-api-barriers, t975, t1018, t1019: every picked effect, transition and generator is a plugin crate behind one contract; a family edit rebuilds no core crate (the crate map). The wasm host is `wasm-plugin-host` (v26.10 P1–P5) | `plugin-api-stage1` (takes P1–P6 of v26.10) | done |
| K9 | 18 | no guide inside the program | done welcome-window-and-tour, t984, t986, t989: the Guide inside the program (a searchable index and its pages), the welcome window and the tour's lessons | `welcome-window-and-tour` | done |
| W1 | 29 | the site: download buttons next to the big heading | no: site → other repository | `kt.9.site` — not indexed by this guard beyond the row | other repository |
| W2 | 30 | the site: links for the visitor's OS, older releases, what's new | no: site → other repository | `kt.9.site` | other repository |
| W3 | 31 | the site: several rotating slogans | no: site → other repository | `kt.9.site` | other repository |
| W4 | 32 | the site: the "video effects" tab slides under the next heading | no: site → other repository | `kt.9.site` | other repository |

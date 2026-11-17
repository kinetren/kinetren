# Tasks: source-colour-matrix

- [x] 1 The rule: `kt_ffmpeg::colour` resolves tags to a matrix and range, HD → BT.709 / SD → BT.601 (D1, t2890, t2891)
- [x] 2 The decode converts with it; the default leaves swscale untouched; the key carries it; `Probe` reports it (D2, D3)
- [x] 3 Fixtures: a BT.709 patch tagged HD, tagged SD, untagged HD (gen.sh); t2892, t2893
- [x] 4 The class walk: every YUV↔RGB path and every disk cache of decoded pixels (D4); sidecar `thumb_pixels`, `STRIP_ROW_VERSION` 3 (t2894)
- [x] 5 found-defects rows 697 and the HDR row; What's new; crate CLAUDE.md; tests, clippy; commit, target-prune; archive

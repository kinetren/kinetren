# Tasks: per-sample-costs

## 1. Audio (rows 128, 350–360)

- [x] 1.1 Before-digests printed on the base tree: t2680 (engine paths), t2681 (noise)
- [x] 1.2 `math::exp10f` and its exhaustive proof (t2682); Phaser, compressor, distortion (128)
- [x] 1.3 Warp buffers (350), strength/twin scratch (351), the mixdown pool, reel maps, sub-mix bursts, the seam memo (352); t2684
- [x] 1.4 Camera mic polyphase (353), rows in place (354), gusts (358); Tape deck in place (354), dropout per frame (357)
- [x] 1.5 One tune per channel pair (355), ring wraps (356), vocoder (359), noise rows (360)
- [x] 1.6 t2680, t2171, t1981, t2315, t1980, t2681 green; `kt-cli fx-cost` before/after (release)

## 2. GPU (rows 361–363)

- [x] 2.1 Composite side sizes once a fragment (361); A/B hashes and timing on llvmpipe
- [x] 2.2 Camera ISP transfer table (362); t2683, t2310, t1217; `kt-cli pass-cost` before/after
- [x] 2.3 Row 363 measured (constants in place of the trig), not taken

## 3. Close

- [x] 3.1 Class walk into design.md; found-defects rows removed; crate CLAUDE.md notes; What's new
- [x] 3.2 Workspace tests, clippy, live idle/heavy load check; commit; target-prune
- [x] 3.3 Archive (specs merged); commit; target-prune

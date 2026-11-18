# Registry of the owner's list `kkt-26.11.18b`

Source — [`kkt-26.11.18b.txt`](kkt-26.11.18b.txt) (verbatim, his answer of 18.11.2026 to the question on
found-defects 734 after stage 20, release kt26.11.18-78f67a11). Columns and states — as in
[`v26.10-index.md`](v26.10-index.md).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| U1 | 1 | Datamosh at a low codec size (144p) breaks beautifully but the picture is soft: a mode where the break is computed at the codec's size and the result comes out sharp at the project's resolution — not pixelation | no note → `mosh-sharp` writes it first (beside `datamosh-codec-path.md`) | `video-fx`, `fx-video-codec` | 21 |
| U2 | 2 | yes to found-defects 734: the Datamosh history and the effects in front of it follow the project's preview settings, so the preview does less work | no → `mosh-sharp` | `video-plan`, `video-fx` | 21 |
| U3 | 5 | a morpher adding Datamosh / VHS Tape / Camera ISP gives no way to turn off the morph of the effect's audio half; the morpher's and the FX chain's control is clumsy — a compact, convenient graph is needed | partial: F-UI-008 written and the proposal `fx-graph` designed (cause: `MorphPatch::derived_pairs` derives the half, so it has no slot and no switch); waits for his go and decisions O1–O5 | `morpher`, `ui-controls`, `video-fx`, `audio-fx` | 21 |
| U4 | 6 | question: is the disk prerender encoded, or kept raw? | partial: answered in the round's report (intra-frame lossless at the preview's size, not raw) — `render-cache-disk` | `video-plan` | 21 |

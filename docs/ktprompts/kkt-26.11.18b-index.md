# Registry of the owner's list `kkt-26.11.18b`

Source — [`kkt-26.11.18b.txt`](kkt-26.11.18b.txt) (verbatim, his answer of 18.11.2026 to the question on
found-defects 734 after stage 20, release kt26.11.18-78f67a11). Columns and states — as in
[`v26.10-index.md`](v26.10-index.md).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| U1 | 1 | Datamosh at a low codec size (144p) breaks beautifully but the picture is soft: a mode where the break is computed at the codec's size and the result comes out sharp at the project's resolution — not pixelation | done mosh-sharp: the row `sharp` — the codec decides at `lines`, a carrier executes its decisions at the layer's size (F-FX-015); t2970–t2973, t2975, live `datamosh_sharp` | `video-fx`, `fx-video-codec` | 21 |
| U2 | 2 | yes to found-defects 734: the Datamosh history and the effects in front of it follow the project's preview settings, so the preview does less work | done mosh-sharp: the stream's input follows the layer's size (the preview's in the preview), the codec keeps `lines`; found-defects 734 closed; t2974, t1248 | `video-plan`, `video-fx` | 21 |

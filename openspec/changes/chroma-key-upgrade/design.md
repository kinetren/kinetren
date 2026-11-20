# Design: chroma-key-upgrade

> DESIGN ONLY, revision 2 (2026-11-20), base `e56a543c`: the owner's A22 — every key moves to the new model (D8).
> Revision 1 (base `069046f5`) kept old slots on the distance model.

## Context

Read on `069046f5`:

- `greenscreen` (`src/plugins/fx-video-color/src/lib.rs`, `greenscreen.wgsl`, `tap_greenscreen.wgsl`):
  rows `key_r/g/b` (u8), `tolerance` 1..200, `softness`, `spill` 0..1; a point-wise tap and a barrier form;
  alpha by Euclidean distance; despill `v − (v − max(others))·spill` on the dominant channel.
- The eyedropper (F-MSK-007) sets the key colour from the screen.
- The node editor's Viewer (fx-graph v1, `openspec/changes/fx-graph/design.md`): Ctrl+Shift+click a node →
  the preview shows the picture after that node, the chain cut by the plan's own code; Esc returns.
- Slot masks (shape-masks): a mask on the key slot mixes the slot's result with its dry side — a garbage
  matte (subtract) and a holdout (add, inverted) are expressible today.

## Goals / Non-Goals

**Goals:** a key that works equally on green and blue; a matte one can see and fix; hair that keeps its
brightness; a first result from one click.

**Non-Goals:** shadows kept as a ratio to a clean plate, uneven screens via a clean plate, light wrap,
bounce-light colour restore — the professional keyer (change `pro-keyer`); temporal stabilisation of the
matte.

## D1. The difference model

Linearised from 8-bit (the sRGB-ish code values as they are — the key works on code values, like Keylight's
non-linear mode; a linear mode is the pro keyer's). With the screen colour S (the key rows) and its
dominant channel k (G or B), the other two a, b:

```text
e(P)  = P_k − (w·P_a + (1−w)·P_b)            // w: balance (0..1)
e_S   = S_k − (w·S_a + (1−w)·S_b)            // the screen's own excess, > 0
m     = clamp(e(P) / e_S, 0, 1)              // 1 = pure screen
alpha = 1 − smooth_clip(m, clip_black, clip_white)   // clip: m ≤ cb → 0 screen-ness … ≥ cw → 1
```

`clip_black`/`clip_white` remap m into 0..1 (below cb the pixel is fully subject, above cw fully screen).

## D2. Matte operations (a barrier)

After the alpha: choke (± px: a min/max filter of the alpha, separable, integer), soften (a box blur of the
alpha, separable, radius px). Both need neighbours, so the difference model runs as a barrier kind with a
pass graph: pass 1 point-wise alpha + despill (rgb) into an RGBA target; pass 2–3 choke H/V; pass 4–5 soften
H/V on alpha only. Integer arithmetic on code values; the reference frames are stored (`FrameTol` exact for
integer passes).

## D3. Despill with luminance restore

`spill_amount = max(0, e(P))·bias` removed from the key channel; then by `despill_mode`:
- `restore` (default): the pixel's Rec.709 luma is restored to the original's by adding the lost luma
  equally to all channels (clamped) — light hair stays light;
- `replace`: the removed amount is replaced by `bias × replacement colour` (a warm grey by default);
- `none`: subtraction only (today's behaviour on the difference model).
There is no distance model after this change (D8).

## D4. Views (preview only)

The Viewer (fx-graph v1) gains a view mode for a key node: Composite (default), Matte (alpha as grey),
Status (alpha < 2/255 → black, > 253 → white, in between tinted: near-0 dark red, near-1 dark green — the
exaggeration that shows dirt), Screen (the input with the screen-ness m as grey). The mode is UI state of the
Viewer (not a row, not in the file); the preview plan appends a view op after the cut; the export plan never
has a Viewer. Invariant 7 holds: the cut and the view are the preview's request of the one plan, like the
Viewer itself.

## D5. Analyze

On a click: five material frames at 10 %, 30 %, 50 %, 70 %, 90 % of the event (Q2 a) at the key slot's INPUT
are composed once through the plan (the Viewer's cut before the key) and read back (a one-shot UI action,
not a render path; offline job class `Visible`, cancelled by an edit). Then, integer:
1. Cb/Cr histogram (64×64) of pixels with chroma ≥ 24; the peak cluster in the green or blue sector = the
   screen; S = the mean RGB of pixels in that cluster;
2. k = argmax(S), w = 0.5 for green, 0.95 for blue (Keylight's defaults);
3. compute m for all pixels; clip_black = the 1st percentile of m over pixels outside the screen cluster's
   chroma neighbourhood ×0.9 (the subject must stay solid), clip_white = the 99th percentile of m over the
   screen cluster ×1.02 (the screen must vanish), clamped to a sane band;
4. despill restore, bias 1.
One `SetFx` (one undo step); the status line says "Screen: green (31, 178, 64), balance 0.5, clip 0.12…0.87".
No screen found → an error naming why ("no saturated green or blue area").

## D6. Format, OTIO, idle

New rows (balance, clip black/white, choke, soften, despill mode, bias) are omitted at their defaults; there is no
`model` row. A file with any new row at a non-default value: the next free version by content. A file written
before this change reads as it is: its rows are mapped by D8 at evaluation, the text is unchanged on write-back. OTIO: rows in metadata. Idle: the analysis is a click; views compose once per change.

## D7. Class walk — "a key's quality from a formula that ignores the screen"

| Effect | Same mechanism? | Verdict |
|---|---|---|
| greenscreen | distance in RGB | this change |
| luma key (if any / `Posterize`-like thresholds) | luminance threshold — no screen balance needed | unchanged |
| Duotone / ColorGrade | not keys | — |
| magic-wand mask (change) | colour distance per seed | its own space choice (RGB/luma/chroma) |
| pro keyer (change) | clean plate, linear | builds on D1 in linear light |

## D8. Every key on the new model (the owner's A22)

The distance kernel (`greenscreen.wgsl`'s distance branch, the tap form's) is deleted. An existing slot's rows
are read by the difference model:
- key colour → the screen colour S (as before);
- `tolerance` (1..200, an RGB distance) → `clip_black = clamp(tolerance / 441, 0.02, 0.6)` (441 = the RGB cube's
  diagonal): a larger tolerance took more of the screen's neighbourhood, a higher clip black does the same;
- `softness` → `clip_white = min(1, clip_black + max(softness / 441, 0.05))`;
- `spill` → `bias`, despill mode `restore`;
- balance from S's dominant channel (green 0.5, blue 0.95); choke and soften 0.
A slot whose new rows are set reads them instead. The mapping lives in one function of the plugin
(`legacy_rows_to_difference`) and is covered by a test on the golden projects.

The stored reference frames with a greenscreen slot change: they are re-stored in the same commit that deletes the
distance kernel, the commit naming the owner's decision (A22). This is the owner's explicit decision about a
regression test's references, not a weakening: the scenario that held "old keys keep their bytes" becomes "old
keys open with the new model".

## Rejected

- **Keeping old slots on the distance model** (revision 1, Q1 a) — the owner chose every key on the new model;
  two models would keep the poor one alive in old projects.
- **A neural matte** — the owner's base for keys is an algorithm (2026-11-01).

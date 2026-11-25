# Design

## Context

A transition is `trans_<id>(uv, k)` over `samp(0u, uv)` (outgoing) and `samp(1u, uv)` (incoming), eight numbers packed by
`TransKernel::pack`. `k` is 0–4096 (the host clamps). Iris shape is `i32(p0.x)`: 0 circle, 1 diamond, 2 box; its
`front_mask(r / rmax, k, soft)` needs a distance `r` normalised to the far corner. The t459 closed-form test and the
"inversion by plan" rule (R.51) hold for every kind.

## Decisions

### D1. Shapes 3 heart, 4 star, 5 hexagon
Each is an SDF evaluated at `c` (aspect-corrected), divided by the SDF's own value at the far corner found by the
same formula over the four corners (`rmax`), so `front_mask` sees 0 at the centre and 1 past the last corner and the
frame is full at k=4096. Heart: the polar formula with its notch; star: five points by folding the angle; hexagon:
`max(|x|·0.866 + |y|/2, |y|)`. Choice values are appended: a file naming `circle` still reads index 0. The picker shows
the new shapes as variants.

### D2. Warps are one shader each
`lens_warp`: the incoming arrives through a magnifying lens growing from the centre (`cx, cy`); `ripple_warp`: both sides
displaced by a ring of radius k with amplitude falling to 0 at both ends; `twirl_warp`: rotation by `turns · (1−|2k−1|)`
of the angle by distance. At k=0 and k=4096 each is exactly the pure side (an amplitude factor that is 0 there), so
the ends are the sides byte for byte. All three symmetric (`symmetric() = true`: the same formula from either side under
`mix(y, x, 1−t) == mix(x, y, t)`), verified by the inversion test. Rows: `lens_warp` strength, size, cx, cy; `ripple_warp`
amount, freq; `twirl_warp` turns, radius.

### D3. Motion blur on push and slide
Row `blur` 0–1 (default 0, omitted). With it >0: eight taps along the direction of travel over `blur·0.12` of the frame,
weighted by the velocity's envelope `4t(1−t)`, so it is 0 at both ends. With 0 the former two branches run (a `>` test,
not a mix by 0): the same bytes. The four dirs use the same code.

### D4. Class walk
Slide, push, whip and zoom are the motion family: whip and zoom_blur already blur. The three volumes (cube, door, swap)
do not take the row: their faces' own motion is their geometry. The preview strip, the gallery and the picker get the
kinds by registry; nothing names kinds by a list but `BUILTIN_IDS` and the catalogs.

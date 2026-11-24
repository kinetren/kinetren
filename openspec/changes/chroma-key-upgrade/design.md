# Design: chroma-key-upgrade

> DESIGN ONLY, revision 3 (2026-11-24), base `0c2a4065`.

## D0. What revision 2 said and the code contradicts

1. **The mapping of old rows was the wrong way round.** Revision 2 wrote `clip_black = tolerance / 441`, "a larger
   tolerance … a higher clip black does the same". With `m = 1` on the pure screen and everything below clip black
   solid, a HIGHER clip black keys LESS. Corrected in D2, where no mapping is needed at all.
2. **A barrier only.** A greenscreen is a dual-position kind (plugin-api-barriers D4): between two geometry steps it is
   a TAP of the fused pass (t305 holds the fused run at one op), at the edge it is the barrier kernel. Revision 2 made
   it a barrier, which would break every geometry series that has a key inside and turn t305 red. D3.
3. **More than eight rows.** `PluginTap::of_at` (`kt-fx/src/geom.rs:602`) copies the first EIGHT values and a tap is
   handed seven f32s; the key has six rows today and gets thirteen. D3.

## D1. Goals / Non-goals

A key that works alike on green and blue; a matte one can see; hair that keeps its brightness; a first result from one
click. Not here (`pro-keyer`): linear light, a clean plate, shadows, light wrap. Not here: temporal smoothing.

## D2. The model

On 8-bit code values (the pro keyer is the linear one). `S` — the key colour; `k` — its largest channel; `a`, `b` — the
other two, `a` the larger of them in `S`; `w` — the balance.

```text
e(P) = P_k − (w·P_a + (1−w)·P_b)
eS   = e(S)
m    = clamp(e(P) / eS, 0, 1)                     // 1 = the screen
tol  = tolerance, hard = max(tolerance − softness, 0)     // code values, as today
cb   = clamp(1 − tol / eS, 0, 0.98)               // m ≤ cb: subject
cw   = clamp(1 − hard / eS, cb + 1/255, 1)        // m ≥ cw: gone
t    = clamp((cw − m) / (cw − cb), 0, 1)
alpha_out = alpha_in · t                          // linear, as the ramp is today
```

- **Balance.** `balance_auto` (flag, default on): `w = 0.5` when `k` is green, `0.95` when blue, `0.5` when red
  (Keylight's values). Off: the row `balance` 0..1, default 0.5, smooth.
- **Why no clip rows.** Along the key axis (`P = S` but for channel `k` lowered by `d`) `m = 1 − d/eS`, so `t` is 0 at
  `d ≤ hard` and 1 at `d ≥ tol`: today's formula exactly. The rows a person, a curve, a link or a preset already holds
  keep their meaning on the axis that matters; off the axis the model is the new one. `tolerance` grows to 1..255
  and `softness` to 0..255 (a value in a file never leaves its range by this).
- **A neutral key colour** (`eS < 8`: grey, white, black, or a colour with two equal leaders): the difference has no
  axis. `m = 1 − max(|ΔY|, |ΔCb|, |ΔCr|)/255` with Rec.709 integer weights and the same `tol`/`hard` over 255. Dropping
  such keys would un-key projects on opening; keeping the RGB distance would keep the model the owner asked to
  leave. Despill does nothing there.

## D3. Two positions, one formula

- `Greenscreen` stays `VideoClass::PointWise` with a barrier kernel. The tap (`tap_greenscreen.wgsl`) and the barrier's
  first pass (`greenscreen.wgsl`) hold the same function text (`key_px`), one file included by both with `concat!`.
- **The seven numbers of the tap** (`gpu_cooked`): `[code, w, eS, cb, cw, bias, rgb24]`, `code = perm + 6·despill`
  (`perm` 0..5 names `(k, a, b)`; `eS = 0` says the neutral branch), `rgb24` — the replacement colour, or `S` itself
  in the neutral branch (exact in an f32). `Cooked` (eight f64) holds the same seven.
- **`PluginTap::of_at`** gives `cook_at` the slot's WHOLE value slice (`slot.values`); the `[f64; 8]` copy stays for
  `gpu_params`'s default only. Posterize, color grade, duotone, vignette and noise have at most eight rows: their bytes
  do not move (t329).
- **`VideoKernel::neighbours(&self, p: &[f64]) -> bool { false }`**, new. The key answers `choke != 0 || soften != 0`.
  `geom::is_pointwise` (`geom.rs:565`) becomes "class is point-wise AND not `neighbours(values)`": the one answer
  `run_window_end` (`plan.rs:257`) and both filters (`:446`, `:718`) ask. With it true the key ends a geometry series
  and runs as the barrier it already is. A curve on `choke` through zero changes the plan between frames, as a curve on
  noise's amount does today. `fuses_in_runs` is unchanged (a barrier kernel: never in a run), so no run shape moves.

## D4. Matte operations (the barrier graph, `gpu_values`: it reads all rows)

1. `key` pass: alpha and despill, on the whole frame: **despill runs on every pixel**, not only where `a > 0` as
   today, so a grown edge shows no green.
2. **Choke** (px, −20..20, `× res_scale`): min (positive) or max (negative) of the alpha over a line, H then V, by
   three-tap passes with a growing offset: reach `R`, next offset `d ≤ 2R+1` → reach `R+d` (1, 4, 13, 40, 80): at most
   five passes a direction at K 4, none when 0. Taps are written out, offsets in `P.u0` (kt-gpu: a bounded loop is dear
   on llvmpipe).
3. **Soften** (px, 0..20, `× res_scale`): `Save(0)`, `Resample` down and up as Blur does for the same radius
   (`fx-video-blur/src/lib.rs:116`), then `join`: rgb from `Saved(0)`, alpha from `Prev`.
A graph at the defaults is one pass, as today. `pass_names`: `key`, `choke`, `join`. No write through a dynamic vector
index (FXC): `select`.

## D5. Despill

`x = max(0, e(P))·bias` (`spill` row) is taken from channel `k`; then by `despill` (choice row):
- `restore` (default): `0.7152/0.2126/0.0722·x` — the luma lost — is added to all three channels (clamped);
- `replace`: the replacement colour × `x / 255` is added;
- `subtract`: nothing.
`spill` keeps its default 0 and range 0..1: an old slot without despill has none.

## D6. The views

The Viewer's project (`node_editor::Viewer::cut_event` after fx-graph v2d, `::cut` before it) is a copy the player holds
and the model never sees (`push_project_to_player`, `app.rs:4677`). `Viewer` gains `view: ViewMode`; the cut appends one
slot to the copy's chain:
- **`matte_view`**, an internal point-wise plugin of `fx-video-color` (`tap_matte_view`, one choice row `mode`), opaque
  output: `matte` — alpha as grey; `status` — alpha < 2 black, > 253 white, low halves tinted dark red, high halves
  dark green; `screen` — `255 − alpha`.
- **Screen** needs `m` before the clips: the copy's key slot gets `tolerance = eS`, `softness = eS`, choke and soften 0
  (then `alpha = 1 − m`) and `matte_view` mode `screen`.
- `UiDecl.internal: bool` (default false): an internal kind is not in the picker, the library or the guides. It is in
  `FxInstance::all()` and t329 like any kind. A file naming it is refused by `check` (`InternalKind`): a project
  never holds it.
- The chooser stands in the badge (`app.rs:7903`) as small chips after "Viewer: Greenscreen"; Screen is drawn only when
  the node is a key. Every video node has Matte and Status: alpha comes from geometry, masks and 3D too. The mode goes
  back to Composite when the Viewer goes to another node; Esc, Play and closing the window end it as today.
- Not through the node editor's canvas, so nothing here waits for v2d: only `cut` → `cut_event` is a text merge.

## D7. Analyze

- **Where**: the key colour row of the slot's form (`fx_editor.rs:1884`), right after the eyedropper: "Analyze". Three
  owners draw that form: an event's chain, a morpher's patch and a track's chain. One function `key_input(project,
  owner, slot) -> Project` cuts the chain before the slot (the Viewer's own cut with the slot itself left out; a patch
  and a track's chain truncated the same way) and solos the track (`fx_thumbs::solo_video`), so the five moments hold
  what the key receives, over nothing.
- **Moments**: 10, 30, 50, 70, 90 % of the event; of the morpher block; on a track, of the event under the cursor on it
  (none: the button says so and does nothing).
- **The job**: `AnalysisKind::Key { owner, slot }` through `Analysis::request_with` (the "one analysis" gate, its
  window with Cancel, its lane `Background`, the revision of the request: a result over another revision is not
  applied and the status says so). The body composes each moment at 640 wide on `kt_gpu::side_engine()` through a
  function factored out of `fx_thumbs::render_scene` (`compose_scene(scene, quality) -> Frame`; `render_scene` becomes
  it plus `fit_into`): own sources, invariant 5's offline path, stopped by Cancel.
- **The arithmetic** (integers, one pure function `kt_fx::key_analyze` over the frames, t3562):
  1. Cb/Cr histogram 64×64 of pixels with chroma ≥ 24; the fullest 3×3 neighbourhood whose hue lies in the green or the
     blue sector; under 3 % of the pixels → `NoScreen`;
  2. `S` = the mean RGB of that neighbourhood's pixels; `k`, `a`, `b`, balance auto;
  3. `m` of every pixel (D2). `screen` = the neighbourhood's pixels; `subject` = `m < 0.3`; between — mixed edges, left out;
  4. `cw = P1(m over screen) × 0.98`; `cb = min(P99(m over subject) + 0.02, cw − 0.05)`;
  5. `tolerance = round(eS·(1 − cb))`, `softness = round(eS·(cw − cb))`, `spill = 1`, `despill = restore`,
     `balance_auto = on`.
- **Applied** as one `SetFx` (`SetMorpher`, `SetTrackFx`): the base rows. A row with a live curve keeps its curve and
  the status line names it ("Tolerance is animated and stays"). The status: "Screen: green (31, 178, 64); tolerance 58,
  softness 21". `NoScreen`: "No saturated green or blue area in this picture".

## D8. What the stored frames, the tests and the files do

- Deleted with the distance kernel: nothing a file holds. t973's two forms read and write byte for byte (the rows
  are the same, the new ones omitted).
- **Stored again, in one commit naming A22**: the greenscreen cases of t181 (`parity.rs:191, 232, 297, 373, 434, 520`),
  t329 (`:862`), the shadow after a key (`:1040`), t1052's; `kt_fx` thumbnails (t544's kinds). Tolerances stay
  (`FrameTol::THRESHOLD`; `choke` alone is exact).
- Read again, not weakened: t26 (the green corner gone, the plain corner whole), t228 (spill lowers green, alpha does not
  move), t305, t519–t521 (`greenscreen` as an alpha maker with tolerance 8), `gen_demo` and the `demo-concat` fixtures
  04 and 07. One that turns red is a cause to find; only a picture the owner's A22 changes by its words may move.
- The variants (`GREENSCREEN_VARIANTS`): Tight (40/10), Wide (120/45), Blue stay; "Blue" also leaves balance auto.
- Format: `balance_auto`, `balance`, `choke`, `soften`, `despill`, `repl_r/g/b` omitted at defaults; a file with any of
  them takes the next free version by content (`project_file::version_for`). The older build refuses it naming it.

## D9. Class walk — "a key's quality from a formula that ignores the screen", and its neighbours

| Place | Same mechanism? | Here |
|---|---|---|
| greenscreen, tap and barrier | distance in RGB | D2, D3 |
| duotone, posterize, color grade | not keys | — |
| halftone/edges/glow thresholds | a luma threshold, no screen | — |
| `matte.wgsl` (a Merge's luma matte, fx-graph) | luma by design | — |
| the wand mask (`magic-wand-mask`) | a distance per seed on purpose: the seed is any colour | its own space choice |
| pro keyer | linear, clean plate | builds on D2 |
| the form shows no result of a number (a matte) | crop, 3D, masks, chroma | Matte and Status for EVERY video node (D6) |
| the picker's and library's cells | alpha over the sample | unchanged (they show the composite) |
| effects whose rows > 8 read as a tap | none today | `of_at` fixed for all |

## D10. Cost

Defaults: one pass, no more arithmetic than today's (two divisions, no square root). Choke and soften: 3–13 passes.
Analyze: five composites once per click. The Viewer's view: one tap in the chain's last pass. At rest nothing; the idle
check runs with the Viewer on a key on a paused frame.

## Rejected

- **clip_black/clip_white rows** (revisions 1, 2) — a second pair of numbers for what `tolerance`/`softness` already
  say, a sentinel for "unset", and every old curve pointing at a dead row.
- **A view row in the key's passport** — it would be a value of the file, of presets and of links.
- **A barrier only** — breaks fused geometry series (D0).
- **Keeping old slots on the old model** — the owner chose otherwise.
- **A neural matte** — the owner's rule for keys (2026-11-01).

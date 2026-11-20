# Design: scopes

> DESIGN ONLY, revision 1 (2026-11-20), base `ec30cb02`. Nothing is built before the owner's answers
> to Q1–Q2 (`proposal.md`).

## Context

Read on `ec30cb02`:

- **The preview no longer reads back.** Since preview-throughput D2 the composite stays on the
  device: `GpuFxEngine::present` (`src/libs/kt-gpu/src/engine.rs`) draws into the display target that
  `src/apps/kt-app/src/preview_display.rs` registers with egui; F-CLR-005's "the readback is
  already paid for" (written 2026-09-07) is no longer true. A covered range replays from
  `PresentMemo` (cached-range-plays) and runs only the display pass.
- **A derived target already exists**: `PresentOptions { glow }` (`src/libs/kt-engine/src/video_plan.rs`)
  refreshes a 32×18 ambient glow from the composite in the same submit. The scope tap is a second such
  target, bigger and read back.
- **The device constraints** (`src/libs/kt-gpu/CLAUDE.md`): the display device may be egui's GL device
  with downlevel_webgl2 limits — fragment passes only, no compute, no atomics (t183). A histogram on
  the device without atomics is either 256 passes or a sort; on a ≤512×288 frame on the CPU it is
  150 k pixels, < 1 ms.
- **The export's readback ring** (export-throughput D5) maps buffers without waiting; the tap uses the
  same pattern at a depth of two.
- **The rest rules** (`src/apps/kt-app/CLAUDE.md`): at rest no frame is asked; a background thread
  with a result calls `wake::now()`; new work gets a `Work` row.

## Goals / Non-Goals

**Goals:** five instruments that read the presented picture with no second composition while
playing; honest 8-bit scales; clipping always visible; zero cost on a pause.

**Non-Goals:** scopes of a single event's input ("по событию" is served by the Viewer of the node
editor — `fx-graph` v1 cuts a chain to a node; the scope measures what the preview shows, so
"show this node" + scope = the event's scope, no second mode); CIE chromaticity; HDR nits; a
"before grade" composite (that would be a second composition path).

## D1. The tap: a downscale pass and a non-waiting readback

`PresentOptions.scope: Option<ScopeTap { max_w: u32 }>`. When set, after the composite (or after
the display pass of a replayed kept composite) the engine runs one fragment pass `scope_down.wgsl`:
an integer box filter into an `Rgba8Uint` texture of `w = min(max_w, frame_w)`, `h` by aspect
(rounded), each output texel the integer mean of its source block (exact, no sampler — the rule of
the crate). It copies it into one of two mapped buffers and `map_async`s it; the next `present`
polls (`Maintain::Poll`) and hands a finished buffer to `ScopeSink` (a channel to kt-app). A tap still
in flight when a newer frame asks is overwritten (the older frame is not worth measuring). Nothing in
`present` waits.

Why a box mean and not a decimation: a decimation drops single clipped pixels (the very thing a
scope is for). The mean keeps the level of areas; the **extents** (D3) keep the excursions: the same
pass writes, per output texel, the min and max of its block into a second `Rgba8Uint` target
(two outputs of one fragment pass, both integer), so a lone 255 pixel lights the top row.

## D2. The paused export frame (Q2 option a)

When the transport is paused and the window is open, kt-app asks `Player::measure_paused(t)` once per
(plan key, project size): it composes the moment at `Quality::Full` through `execute_into` on the
offline route — the same plan builder (invariant 7) — downscales on the CPU with the same box rule
(the shared function `scope_reduce` in kt-engine, also the oracle of t3401) and sends the result to the
same sink. The answer is memoized by the plan key; a second pause on the same moment costs nothing.
It is a job of class `Visible` in the one scheduler (one-scheduler), cancelled by play.

## D3. Instruments (kt-app `scopes.rs`, its own thread)

Input: `ScopeFrame { w, h, mean: Vec<u8>, lo: Vec<u8>, hi: Vec<u8>, quality, key }`.

| Instrument | Counts | Image |
|---|---|---|
| Waveform (luma) | per column x, 256 bins of Y = (54·R + 183·G + 19·B + 128) >> 8 (Rec.709 integer weights) | W×256, intensity = log-scaled count; rows 0 and 255 lit where any `lo`/`hi` reached them |
| Parade | the same per R, G, B, three panels | 3W×256 |
| Vectorscope | Cb, Cr (Rec.709, integer) → 256×256 bins | 256×256, graticule: 75 % targets of R, Mg, B, Cy, G, Yl, the skin line at 123° |
| Histogram | 256 bins of Y and of R, G, B | 256×128, the four overlaid |
| False colour | per pixel of the analysis frame: zones by Y (0–5 % blue, 5–15 % teal, 40–50 % grey "18 %", 50–60 % pink "skin", 95–100 % red, others the luma) | drawn over the preview, same rectangle |

All integer; the image is built into one reused buffer and uploaded as one texture per new
`ScopeFrame` (`ctx.load_texture` update, no per-frame allocation — UI rule of t40). The thread calls
`wake::now()` after a new image; nothing polls.

The scale labels: "0 · 64 · 128 · 192 · 255" or "0 · 25 · 50 · 75 · 100 %" (one switch in the window's
head); no 10-bit, IRE or mV — the decode path is RGBA8 (`sws_getContext` without
`sws_setColorspaceDetails`, F-CLR-005) and a finer scale would print digits we do not have.

## D4. The window (Q1 option a)

`window_shell::Shell::new("scopes", …, Kind::Tool)`, a `WINDOWS` row with its reason ("a vectorscope
is square and needs room no existing panel has; it can sit on a second screen"). The head: up to four
instrument toggles (a gallery, fixed width — UI rule 3), the scale switch, the quality label. The
body splits evenly. False colour is a toggle that paints the preview, not a pane. `UiState.scopes:
Option<ScopesUi { shown: Vec<ScopeKind>, scale: ScopeScale }>` keeps the choice (serde default,
`skip_serializing_if = None`).

## D5. Idle cost

The tap is set in `PresentOptions` only when (window open ∨ false colour on) ∧ the presented key
differs from the last measured. On a pause: no `present` runs at all (no frame is asked), so no tap;
D2 runs once and memoizes. Mouse over the scope: egui hover frames redraw the window texture only —
no pass, no readback, no scope thread work. Counters: `Work::ScopeTap`, `Work::ScopeCount`; the live
scenario `idle` adds "scopes open, mouse moving" and requires both zero (t3410).

## D6. Class walk — "a measurement that must not cost when nothing changes"

| Place | Same mechanism? | Found |
|---|---|---|
| the ambient glow (`PresentOptions.glow`) | yes, a derived target per present | refreshed every 4th frame already, no change |
| meters (`meters.rs`) | decaying state | `is_active()` rule kept; scopes have no decay |
| reel previews (`reel_preview.rs`) | offline composite on demand | memo by `content_hash` — D2 uses the plan key the same way |
| Diagnostics window | samples on a clock twice a second | the scope does not; it is driven by frames |
| bin card scrub | its own decode | not measured by the scope (it measures the project preview only) |

## D7. Format and OTIO

`UiState.scopes` only. No project file version change (UI state, `version_for` untouched). OTIO
carries `UiState` in `metadata["kt"].ui` already; no row in "What OTIO export loses" (a foreign
editor has nothing to show for it, and nothing of the edit is lost).

## Risks

- A box-filtered analysis frame at 512 px of a 4K project averages 7×7 blocks: the waveform's body is
  smoother than Resolve's at "High"; the extents keep the excursions, which is what is read.
- The GL device's `map_async` latency: one or two frames late — the scope trails the picture by up to
  two frames while playing; on a pause it is exact.

## Rejected

- **A compute histogram with atomics** — not on the GL display device, and atomics break determinism
  on the device (t183).
- **Reading back the full composite** — 3.7 MB per 720p frame at 60 fps for an instrument that needs
  ≤ 0.6 MB; the tap is the reduced form.
- **A trace smoothing knob** — it understates clipping (F-CLR-005 trick 2).

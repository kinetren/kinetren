# Design: scopes

> DESIGN ONLY, revision 2 (2026-11-24), base `229c694b`. Q1–Q2 decided (A8, `proposal.md`). Implementation-ready.

## D0. What revision 1 said and the code contradicts

1. **"The next `present` polls the map."** On a pause there is no next `present`: the last measurement would never come.
2. **One path, `present`.** There are three: the display target (`present`, `present_kept`), the bytes of a slow chain or
   of a GL display (`Composed.rgba`, `gl-context-holder`), and the producer's kept composites while playing
   (`present_ahead` → `show_kept`). A tap in `present` alone measures nothing on GL and on a slow chain.
3. **A pass that makes the box mean, minimum and maximum.** The composite can be copied as it is: at the preview's default
   640×360 it is 0.9 MB. Counting every pixel needs no reduction, no new WGSL and no extents; the "lone clipped pixel"
   is then counted, not hinted.
4. **False colour drawn from the analysis frame** trails the picture by one or two frames while playing. It belongs to
   the display pass (`display.wgsl`), in step with the frame it paints.
5. **`Player::measure_paused` in kt-engine.** The offline frame of a project exists: `fx_thumbs::SceneCtx` (its sources,
   cache, stream memo, build memo; a lane of class `Visible`; `kt_gpu::side_engine`). A second one is not written.
6. **`UiState.scopes`.** What a tool shows is the person's, not the project's (UI rules 23, 27): `user_store::Workspace`, beside
   `geom_controls`. The project file is not touched at all.
7. Opening the window on a pause: nothing is presented, so nothing is measured. The UI asks the shown moment once more.

## Context

Read on `229c694b`: `VideoGpuExec::{present, present_kept, wait_presented, poll_done}` and `PresentOptions { glow }`
(`video_plan.rs:523–660`), `GpuFxEngine` (`kt-gpu/src/engine.rs:2004–2110, 2560–2830`, the display pass `pass_display`
and `display.wgsl`), the compose thread (`compose_queue.rs:193–290`: after a composite it waits in `wait_presented`;
the producer's path does not), `draw_preview` (`app.rs:7484`: the ask key `(tick, quality, content, project)`),
`preview_display.rs`, `push_project_to_player` (`app.rs:4673`: the reel or the Viewer's cut project), the preview's menu
`preview_menu_ui` (`app.rs:12242`), the window shell and `WINDOWS`. `Composed` carries the bytes when `!on_display`.

## Goals / Non-Goals

**Goals:** five instruments that read the picture the preview shows, on every path it takes; every pixel counted; a
full-size measurement once per paused moment; false colour in the frame it paints; nothing at rest.

**Non-Goals:** CIE, HDR nits and IRE (the composite is RGBA8; `hdr-sources-to-sdr` stage 5 gives the tap a 16-bit
twin there); a scope of one event's input as a mode of its own: the Viewer (fx-graph) already shows the picture after
any node, and the scope measures what is shown ("по кадру превью или по событию"); smoothing (it hides clipping).

## D1. The tap: the presented composite copied, never waited for

```text
PresentOptions { glow, tap: bool, false_colour: bool }          // Copy; the literals of tests take `..Default::default()`
VideoGpuExec::take_tap(&self, out: &mut TapFrame) -> bool       // default false
TapFrame { w, h, rgba: Vec<u8>, t: Tick (set by the compose thread) }
```

- **kt-gpu**: `Inner.taps: [Option<TapBuf>; 2]` (`COPY_DST | MAP_READ`, sized by the composite, in the memory ledger as
  `Staging`). With `o.tap`, `present` copies `result` (and `present_kept` the kept texture) into a free buffer in the
  SAME encoder, after the display pass; with none free the tap is skipped. After the submit `map_async` sets a flag.
  `take_tap` copies the rows of the newest finished buffer, unmaps it and frees the older one; it never waits. It runs
  through `guarded` and `gl_gate` as the other entries do. An empty plan taps nothing (the composite is zeros).
- **The compose thread** (`compose_queue::run`): after it hands out an answer whose ask had `tap`:
  - `on_display`: if no ask waits, `wait_presented()` (one composite in the queue, as today), then `take_tap`; with an
    ask waiting it takes on its next round. So the last tap of a play arrives on the pause, and the first one while
    paused needs no second frame;
  - bytes: the same `rgba` the answer carries is copied into the `TapFrame` (no readback of its own);
  - an incomplete frame taps nothing (it has no picture).
  The frame goes to `Composer::tap_sink: Mutex<Option<TapFrame>>` + a wake of the scope thread; the newest replaces.
- **How often**: the UI sets `tap` only when a window wants it AND the sink was emptied by the scope thread; a machine
  that counts slowly is measured less often, never queued for. While playing the copy of a 640×360 composite is 0.9 MB,
  of a full 1080p 8.3 MB.
- **The trace**: `Ev::Job` `SCOPE_TAP` with the frame's id, so `kt-cli trace frame <id>` shows what a tap cost.

## D2. The paused full-size frame

`SceneCtx::frame(scene, project, t, q, stop) -> SceneFrame { Done(Vec<u8>) | Incomplete | Stopped }` is extracted from
`render_scene` (the picker keeps its size and its fit), with the cache limit a parameter (64 MiB for a cell; for a full
frame: what one moment of the plan needs, `4 × Σ layers` at their decode sizes, at least 64 MiB). The `scopes` lane
(`Visible`, so playing and an export pause it) holds one `SceneCtx` for `Scene { project: the Arc the player holds }`:
the same reel or Viewer-cut project the preview composes.

- Asked when: the window is open, the transport stands, the shown moment is complete, no pointer button is down and the
  measured full key differs from `(tick, project_version, content_version)`. One job per key; a newer key cancels.
- The raster is the project's (`player.raster()`). Export and preview share `frame_plan`, so the bytes are the
  export's frame of that moment (t3401 compares with `compose_export`'s).
- Until it lands the instruments show the preview's frame under its own label ("640×360"), then "1920×1080". A moment that
  comes out incomplete offline keeps the preview's reading with the label "preview only", never a partial one.
- Cost: one composite with its decodes per paused moment and per edit's end; a drag of a slider re-measures the preview's
  tap per frame and the full frame once, at the release (the button rule above).

## D3. The instruments (`apps/kt-app/src/scopes.rs`)

A lane of class `Visible` stands while playing, and the scope must move then: the counting is a thread of its own, class
`Near`, parked on the sink's condvar with no clock, joined through `kt_time::shutdown` (2 s) at the window's close. Input `TapFrame`; alpha is applied first (`c = (c·a + 127)/255`, black
below, as the export writes it).

| Instrument | Counts | Image |
|---|---|---|
| Waveform | per image column (≤ 512, `x·W/w`), 256 bins of Y = (54·R + 183·G + 19·B + 128) >> 8 | 512×256, intensity by `log2(1 + count)` scaled to the column's pixels |
| Parade | R, G, B | 3 × 170×256 |
| Vectorscope | Cb, Cr (Rec.709, integer) → 256×256 | 256×256; graticule: the six 75 % targets and the 123° skin line |
| Histogram | 256 bins of Y, R, G, B | 256×128, four curves |
| False colour | none (D4) | a toggle |

A row at 0 and at 255 is lit in the error colour where its bin is not empty in the waveform and parade; the histogram's
first and last bins carry the same mark and the share of clipped pixels in the status line on hover. All counts are
`u32`, integer, independent of threads (one thread). One reused buffer per instrument; a texture `set` per new image on
the UI thread; `wake::now()` after a new set. Scale labels 0–255 or 0–100 %, the switch in the head.

## D4. False colour is the display pass's

`display.wgsl` gets `u0.…`'s free word as a mode: 0 — today's bytes (t1525 holds), 1 — false colour: Y by the same integers,
zones 0–5 % blue, 5–15 % teal, 40–50 % grey-green ("18 %"), 50–60 % pink ("skin"), 95–100 % red, else the grey of
Y. The glow keeps mode 0. The bytes path runs the same table on the CPU (`scopes::false_colour`, also the test's oracle).
The kept composites are untouched: a toggle re-presents the shown moment once (`present_kept`). It is a state of the
session, not stored, and the Viewer-like badge "False colour" stands on the picture while it is on (rule 10's cascade:
Esc turns it off after the Viewer).

## D5. The window and its places

`window_shell::Shell::new(egui::Id::new("kt_scopes"), …, Kind::Tool)`, size 640×360, min 360×220; the row in `WINDOWS`:
"instruments of the picture: a vectorscope is square and needs room no panel has; it can stand on a second screen".
The head: five toggles (icons with a hint; fixed widths), the scale switch, at the right the measured size. The body:
the shown instruments in equal cells, 1–4; a fifth is False colour, which paints the preview and takes no cell.
Opened from View › Scopes and from the preview's right-click menu (one body). `Workspace.scopes: Option<ScopesPrefs>` (shown
set, scale, open) with the useful default Waveform + Vectorscope.

## D6. At rest

- The ask: `draw_preview` adds `scope_due = tap wanted && measured.key != (preview key)`; it asks the shown moment ONCE
  (`preview_asked` guards), counted as any composite (`show_kept` replays it).
- With the key measured: no ask, no tap, no job, no wake. Mouse motion over the window draws textures.
- Closing the window ends the thread and drops both buffers.
- Counters `Work::ScopeTap`, `Work::ScopeCount` (`ALL` 17 → 19); `idle.py` opens the window on the heavy showcase, pauses,
  moves the mouse ten seconds and requires both zero after the first two (t3410).

## D7. Class walk: "a measurement that must not cost when nothing changes"

| Place | Found |
|---|---|
| ambient glow | a target in the same submit, every fourth frame; unchanged |
| meters | a decay with a limit (`is_active`); unchanged |
| Viewer's cut | a project of its own in the player; the scope follows it through the same `Arc` |
| picker cells, reel strip, poster | offline frames by key; D2 shares their context |
| render-check scene (`picture-check`, designed) | measures every exported frame; `scopes::count` (Y bins, clipped share) is the function it will call, one rule for "clipped" |
| colour match / Analyze of the key (`chroma-key-upgrade`) | samples frames of its own; not a scope |
| the detached preview | the same display texture; the tap and false colour act there too |
| two players (a reel's preview window) | there is one player; none |

## D8. Order of merging

`engine.rs` and `video_plan.rs` are also `export-into-surface`'s (its section 2): one at a time, the second rebases the
literals of `PresentOptions`. Nothing of v2d's open part is touched.

## Rejected

- **A reduction pass with extents** (revision 1) — an approximation nobody needs at these sizes.
- **`read_kept` from a scope thread** — it waits for the device under the engine's lock (`engine.rs:2092`), in the way of the
  display's `present`.
- **A histogram in a compute shader** — not on a GL display device, and a second rule of counting.
- **`UiState`** — a tool's setting in a project file.

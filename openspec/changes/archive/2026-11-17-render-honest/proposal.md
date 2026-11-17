# Proposal: render-honest

## Why

Two rows of `docs/found-defects.md` and item R8 of the architecture review of 2026-11-17:

- **Row 710.** A software adapter's export opens 2–4 devices by a fixed rule (an eighth of the cores,
  as many as the memory holds) and uses all of them on every frame. Measured by render-speed: on a light
  1080p stretch four devices took 31.8 s against 12.5 s on one; over the whole project four won (129–132 s
  against 144–172 s). No fixed count suits both; the count was never chosen by what the time says.
- **Row 711 / R8.** The owner: "финальный рендер у нас очень медленный во всех версиях, медленее чем
  превью" (kkt-26.11.15 item 3). Not reproduced here at equal size. His preview composes at 640×360 (his
  `rendereg_2` load report), his render at 1920×1080: nine times the pixels a frame, and nothing on screen
  says so. The render window predicts nothing, and no render records how it compared with what was
  expected.

## What Changes

- The export of a software adapter still OPENS its devices by the memory and core ceiling, but USES as many
  of them as the measured time says: it measures the frames written per second of wall over windows of
  frames, climbs while one more device makes it faster by a clear margin, keeps the fewer devices on a
  tie, and re-tries one more and one fewer as the render goes, so a light stretch and a heavy one each get
  their own count. One rule on every machine, no constant tuned for ours. The frames written do not depend
  on the count (t1573), and the export's line names the counts it used.
- The preview keeps what its composites cost: the device time of each composed moment at its size. The
  render window shows the preview's size against the render's ("9× the pixels of a frame"), and a predicted
  render time from the measured device time per pixel over the range when it is known, saying how many of
  the range's frames were measured; with nothing measured it says to play the range once.
- Every render's journal line records the predicted time against the actual one (or that there was no
  prediction).

## Capabilities

### Modified Capabilities
- `export`: the device count of a software export follows measured time; the render window states the
  render's cost against the preview; the journal records predicted against actual.

## Impact

kt-engine (`export_pipeline.rs`: device rule, writer order; `export.rs`: `ExportBackends::devices`;
`player.rs`, `video_plan.rs`: the preview cost ledger, `VideoGpuExec::last_device_ns`), kt-gpu (`engine.rs`),
kt-app (`render_dialog.rs`, i18n catalogs, `layout_tests.rs`, a live scenario). No format change; OTIO
unaffected.

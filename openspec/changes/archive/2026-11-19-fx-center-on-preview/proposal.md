# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 52, row T10): "Radial Blur имеет параметры центра X Y, и
эти параметр находятся на разных строках. Мало того - я еще не могу как в том же Vegas указать центр
по самой превью. А Vegas - это базовый минимум". The centre was two numbers on two rows.

## What Changes

- The selected slot of an open effects window that has a centre pair (`center_x`/`center_y`) shows a
  crosshair on the paused preview at that centre, in the frame the slot works on (the source placed
  by the Geometry node before it, the canvas after it).
- Dragging the crosshair writes the pair in one history step: the base, or the key at the playhead
  when the pair has a curve. The status line says so while hovering.
- The parameters grid keeps every `<stem>_x`/`<stem>_y` pair on one row.

## Capabilities

- `video-fx`: ADDED requirement "An effect's centre is dragged on the preview".

## Impact

`kt-app`: new `centerctl.rs`; `maskctl.rs` (`slot_frame_view`, `View` shared), `widgets.rs`
(`params_grid_bound` pairs, `field_probe::forget` for tests), `app.rs` (the one call site in
`draw_preview`), five catalogs. Tests t3123–t3126; live `fx_center`. No model or format change.

# Tasks: shape-masks

## 1. Model (kt-model)

- [x] 1.1 `mask.rs`: `Mask`, `MaskShape`, `Vertex`, `MaskMode`, `Falloff`, serde with defaults skipped; `specs()`, `get/set`, `at(tick)`, `to_path()`, vertex insertion with keys at every path key, delete with curves — t1261, t1275
- [x] 1.2 `ClipEvent.masks/fx_masks`, `Track.masks`, `active_fx_masks`, `needs_video_stage`; validation errors; `SetMasks/SetFxMasks/SetTrackMasks`, `SetFx` trims, exhaustive matches; clipboard blocks — t1260, t1262
- [x] 1.3 Links: `ParamOwner::Mask`, link path text, `check_links/resolve_derived/read_at` over mask curves — t1263
- [x] 1.4 `.kt` format 2 by content — t1264; kt-otio envelope v14 by content — t1265

## 2. Raster (kt-fx)

- [x] 2.1 `mask.rs`: flattening, scanline inside, band SDF, feather/falloff/expansion, modes, invert, opacity; frozen math only; bounded memo — t1266, t1267, t1268, t1269
- [x] 2.2 Apply kernels (alpha, mix) and the planner: `MaskMix` as a strength stop, `MaskAlpha` as a barrier; the mapping "reference → raster" through the leading node's placement

## 3. Engine and GPU

- [x] 3.1 kt-engine: materialize picture/slot masks into the chain, evaluate at the material tick, the post-FX key, the track stage — t1270, t1271, t1272, t1273
- [x] 3.2 kt-gpu: `mask.wgsl`, `FxOp::MaskAlpha`, coverage texture in the mix — t1274

## 4. Interface (kt-app)

- [x] 4.1 `masks.rs` list widget; Geometry node group, effect slot group, track window section — t1279, t1280, t1281
- [x] 4.2 `maskctl.rs`: overlay, tools, handles, keyboard, the preview menu; one call site — t1276, t1277, t1278, t1282. Found on the way: with the R.64 frame on (the default) a right click on the picture opened NO menu — the frame took it. The menu body is one function now (`preview_menu_ui`), attached to the picture, the frame and the mask editor (ui-reach requirement added)
- [x] 4.3 Texts in five catalogs — t1283

## 5. Live, docs, finish

- [ ] 5.1 Live scenario `src/scripts/live/masks.py` — t1284
- [ ] 5.2 F-MSK-001/002 verdicts, What's new, registry S1, "What OTIO export loses" row, crate CLAUDE.md lines
- [ ] 5.3 `cargo test --workspace`, clippy, `git merge master`, `finish.sh` — the version here

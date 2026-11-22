# Tasks: persistent-cache

> DESIGN ONLY until the owner answers Q1 and the orchestrator's go. Revision 3 (B1). Tests reserved: t3440–t3459
> (used t3440–t3453).

## 0. Notes first

- [x] 0.1 F-MED-002 renamed "Persistent cache" and refreshed with A10 and B1
- [x] 0.2 proposal, design (D1–D9, class walk), deltas `video-plan`, `media-index`, `export`, `ui-reach`; `openspec validate persistent-cache --strict`
- [x] 0.3 coordination with `project-cache-folder` (spec-engine, agreed in its `301d5a49`): the results/ layout, the value order, the key without the build
- [ ] 0.4 the owner's answer to Q1 folded in

## 1. The store (kt-engine)

- [ ] 1.1 `render_cache.rs` → the store of three kinds in the cache folder, one budget, value order; t3453
- [ ] 1.2 the layer key (durable hash + decoder major + kernel versions + COMPOSITE_PATH_VERSION); t3452
- [ ] 1.3 cut points from measured pass times; the producer writes layer results; `frame_plan` starts from a held cut; t3451
- [ ] 1.4 export guard; t3445

## 2. Source copies (kt-ffmpeg, kt-media)

- [ ] 2.1 `MuxConfig` per-frame times; MKV; `keyint=1`; audio stream copy; t3440
- [ ] 2.2 the copy job and the adaptive size for heavy sources; camera copies; t3441, t3442, t3449
- [ ] 2.3 the copy source with the original's natives; t3443, t3444, t3446

## 3. Interface (kt-app)

- [ ] 3.1 the switch, badges, the Diagnostics section; i18n ×5; t3447, t3450; live `persistent_cache` (t3448)
- [ ] 3.2 idle on a heavy project with the store full

## 4. Close

- [ ] 4.1 What's new; F-MED-002 verdict; registry B1 moved
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report

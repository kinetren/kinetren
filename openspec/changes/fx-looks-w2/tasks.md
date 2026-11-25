# Tasks

Every stage is committed with target-prune. The kinds are independent.

- [ ] 1.1 `light_leak` (D4); t4267 (`src/libs/kt-gpu/tests/light_leak.rs`): identity at 0, distinct at two moments, the same twice; t328, t329 rows
- [ ] 1.2 `glitch_blocks` (D1); t4268 (`src/libs/kt-gpu/tests/glitch_blocks.rs`): amount 0 and a calm frame give no pass, seeds differ, hold holds
- [ ] 1.3 `film_damage` (D2); t4269 (`src/libs/kt-gpu/tests/film_damage.rs`)
- [ ] 1.4 `lens_blur` (D3); t4270 (`src/libs/kt-gpu/tests/lens_blur.rs`) measured first, stored frames after
- [ ] 2.1 `radial_blur`'s directional mode (D5); t4271 (`src/plugins/fx-video-blur/tests/radial_mode.rs`): old bytes stand, the mode streaks along its angle
- [ ] 2.2 The four variants; t4272 (`src/apps/kt-app/src/layout_tests.rs`): each distinguishable in the gallery (the registry's variant guard covers it: extend its list)
- [ ] 3.1 `BUILTIN_IDS`, catalogs ×5, keywords; t978's list; t4273 (`src/apps/kt-app/src/reach_tests.rs`, PATH): the picker finds "light leak" and one click adds it
- [ ] 3.2 A live scenario step per kind (screenshots); t4274 (`src/apps/kt-app/src/idle_tests.rs`): a paused project with the four kinds wakes nothing
- [ ] 3.3 `cargo test` of kt-gpu, kt-app, kt-cli, clippy; F-FX-007 rows `have`; What's new; commit

# Tasks: render-honest

- [ ] 1 Device count by measured time: `DeviceRule` (D1), the dispatcher's active count and the writer's
      order channel, `ExportBackends::devices` (scripted for tests), the counts in the export line (t2900, t2901)
- [ ] 2 The preview cost ledger: `VideoGpuExec::last_device_ns` (kt-gpu), `PreviewCosts` in the player,
      `Player::preview_cost` (D2, t2902)
- [ ] 3 The render window: size against the preview, the predicted time; the journal line (D3, D4, t2903,
      t2904); catalogs in five languages; font/i18n tests
- [ ] 4 Live scenario `render_honest` with a PATH; found-defects rows 710, 711; What's new; crate CLAUDE.md;
      tests, clippy, commit, target-prune; archive

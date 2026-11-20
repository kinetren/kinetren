# Tasks: disk-bytes-once

> Small; no owner question. Tests reserved: t3356–t3360. Base: orch `f1f063c4`.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D5, class walk D4), delta `video-plan`; `openspec validate disk-bytes-once --strict`

## 1. The fix

- [ ] 1.1 `show_kept` takes the target: `present_kept` for the display, `read_kept` for bytes; the bytes
      path's executor; `restore_from_disk` for both; `disk_bytes` removed
- [ ] 1.2 t3356, t3357 (kt-engine), t3358 (kt-cli and the live `render_cache` under GL)
- [ ] 1.3 found-defects 774 closed; `cargo test -p kt-engine`, clippy, target-prune; commit

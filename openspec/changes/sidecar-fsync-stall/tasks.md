# Tasks

## 1. No sidecar lock across an fsync (media-index)

- [x] 1.1 Reproduce: `/tmp` on a rotational disk; under fsync write load a sidecar opening round
      took 137 s; the stalled-disk VFS shim gives the finish.sh `DatabaseBusy` on the old code — t1460
- [x] 1.2 `MediaIndexDb::try_open`: `synchronous=NORMAL` and `SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE` on
      every opening; t1460 red on the old code and with NORMAL alone, green with both
- [ ] 1.3 `kt-media/CLAUDE.md` rule, `docs/media-index.md`; the kt-media and kt-app tests green

## 2. Close

- [ ] 2.1 `cargo clippy --all-targets`; `src/scripts/finish.sh` twice with no red among t1171, t640,
      t1413 and t411 — the versions here and in the report
- [ ] 2.2 Archive the change into `media-index`

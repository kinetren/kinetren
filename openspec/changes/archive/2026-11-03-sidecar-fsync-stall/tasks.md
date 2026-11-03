# Tasks

## 1. No sidecar lock across an fsync (media-index)

- [x] 1.1 Reproduce: `/tmp` on a rotational disk; under fsync write load a sidecar opening round
      took 137 s; the stalled-disk VFS shim gives the finish.sh `DatabaseBusy` on the old code — t1460
- [x] 1.2 `MediaIndexDb::try_open`: `synchronous=NORMAL` and `SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE` on
      every opening; t1460 red on the old code and with NORMAL alone, green with both
- [x] 1.3 `kt-media/CLAUDE.md` rule, `docs/media-index.md`; the kt-media and kt-app tests green

## 2. Close

- [x] 2.1 `cargo clippy -p kt-media --all-targets` clean; `cargo nextest run --workspace` twice
      (the finish.sh workspace command; finish.sh itself is the orchestrator's per stage since
      2026-11-03), then `cargo test --workspace`. t1171, t640, t1413, t411 and t1460 passed in every
      run. Run 1 (load ~23): t1171 took 320 s but passed, because the openers now wait at the gate
      behind a stalled fsync instead of failing. One unrelated red: t815, where writing the recovery
      file took 4.6 s against its 2 s threshold (a disk stall outside the sidecar). Run 2 and
      `cargo test --workspace`: all green.
- [x] 2.2 Archive the change into `media-index`

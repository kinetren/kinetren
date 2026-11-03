# Proposal

## Why

Four sidecar tests went red only inside the whole-workspace `finish.sh` run (`cargo nextest`, every
crate's tests in parallel). They were green alone in 0.1–2.6 s. Seen on 2026-11-03, `kt26.11.03-6e21b83b`:

- t1171: 9 openings `DatabaseBusy`;
- t640: "second open rendered the reel again", and once "first open: no preview";
- t411: "the round trip through the DB changed the number";
- t1413: "the reel preview was not computed".

Together they made finish.sh red at random and could hide real regressions.

Cause, measured: both disks of the build machine are spinning, and `/tmp` sits on one of them.
Under the write load of a parallel run, one `fsync` took **137 s** (a 12-thread opening round of a
new sidecar). The sidecar held SQLite locks across such fsyncs:

- a commit synced the WAL under the write lock (`synchronous=FULL`, the default);
- the LAST close of a connection checkpointed the WAL into the database and synced it, holding the
  database's EXCLUSIVE lock, which every opening needs.

A connection waiting behind either lock gave up after its 5 s busy timeout, and the error became a
cache miss:

- t1171's openings failed behind the previous opener's closing checkpoint;
- t640's reel job lost its disk tier and rendered again;
- t411's second index service could not open the sidecar, so it measured the file afresh and got the
  unrounded loudness instead of the stored one. The DB keeps 1/100 dB.

The tests' isolation is not at fault: each test has its own directory. Under nextest each test is
also its own process.

## What Changes

- `kt-media` `MediaIndexDb::open`: every connection runs with `PRAGMA synchronous=NORMAL` and
  `SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE`.
  - In WAL mode a commit no longer syncs. A close no longer takes the EXCLUSIVE lock or
    checkpoints.
  - The passive auto-checkpoint, which blocks no reader and no writer, folds the WAL into the file.
  - The `-wal` and `-shm` files stay next to the sidecar after the program exits.
  - On power loss NORMAL keeps the database consistent, and only the last commits roll back. The
    sidecar is a cache, so it can lose them.
- A regression check that reproduces the contention deterministically. A SQLite VFS shim stalls
  every fsync the way the loaded disk did, and two threads open, write, read and close a ready
  sidecar at once. On the old code it gives the exact `DatabaseBusy` "database is locked" of the
  finish.sh runs.

## Capabilities

- `media-index` — ADDED: no lock of the sidecar is held across an fsync (t1460).

## Impact

- `src/libs/kt-media/src/index.rs`, `src/libs/kt-media/tests/sidecar_stall.rs` (new test binary),
  `src/libs/kt-media/CLAUDE.md`, `docs/media-index.md`.
- No schema change and no format change. A sidecar written by an older build opens as before.

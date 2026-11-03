# Spec Delta

## ADDED Requirements

### Requirement: No lock of the sidecar is held across an fsync
A connection to the sidecar SHALL NOT hold a SQLite lock that another opening, reader or writer
waits on while the disk syncs:
- a commit SHALL NOT sync the WAL (`synchronous=NORMAL` in WAL mode);
- closing a connection SHALL NOT checkpoint, sync or take the database's EXCLUSIVE lock (no
  checkpoint on close; the passive auto-checkpoint folds the WAL in).

Both SHALL be set on every opening and SHALL NOT write the file. The `-wal` and `-shm` files MAY
stay next to the sidecar after the last connection closes.

Why: on the build machine's spinning disks one fsync under a parallel test run took 137 s. A commit
synced under the write lock, and the last close checkpointed and synced under the EXCLUSIVE lock
every opening needs. Connections waiting behind them gave up after the 5 s busy timeout, and the
error became a cache miss: t1171 `DatabaseBusy`, t640 a re-rendered reel, t411 a loudness measured
again instead of read back. These were red only in the whole-workspace finish.sh run. The same
stall happens next to a person's project on a spinning disk.

#### Scenario: Two threads use a ready sidecar while every fsync is stalled
- **WHEN** a ready sidecar was written while the disk answered; then every fsync blocks until
  released, and two threads released by one barrier each open the sidecar, write a media record and
  a loudness, read it back and close, five times
- **THEN** both threads finish within 3 s (under the 5 s busy timeout), with no error and no fsync
  attempted. Before: one thread's commit hung in the fsync, and the other failed with `DatabaseBusy`
  ("database is locked") after 5 s; with `synchronous=NORMAL` alone, a close hung in its checkpoint; without the close checkpoint
  alone, a commit hung and the other thread's write failed with `DatabaseBusy`
- **TEST** t1460 (`src/libs/kt-media/tests/sidecar_stall.rs`); its consequences in the
  whole-workspace run: t1171 (`src/libs/kt-media/tests/index_db.rs`), t640 and t1413
  (`src/apps/kt-app/src/layout_tests.rs`), t411 (`src/apps/kt-app/src/util.rs`)

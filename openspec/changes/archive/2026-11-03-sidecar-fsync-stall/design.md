# Design

## D1. The red was a lock held across an fsync, not the tests' isolation

Suspects ruled out:

- **Shared paths.** Each test has its own directory: `tempfile`, or `kt-tNNN-<pid>` with a
  distinct name per test. Under nextest each test is its own process, so the in-process `open_gate`
  never meets another test.
- **A crowd of openers (t1171's own fix).** The gate serialises the setup, and a quiet machine
  never fails it.

What does fail it, measured:

- `/tmp` is on a rotational disk (`/sys/block/sda/queue/rotational` = 1).
- Under sustained `dd … conv=fsync` writes, one 12-thread opening round of a NEW sidecar took
  137 s. All 12 openers waited at the gate behind the first one's fsync, and none failed: nobody
  else held a SQLite lock.
- A failure needs a lock HELD across the fsync while another connection waits on SQLite's busy
  handler.

The sidecar had two such holders:

1. **The commit.** With `synchronous=FULL` (SQLite's default) every WAL commit syncs the WAL while
   holding the WAL write lock. Another writer waits, then fails after 5 s.
2. **The last close.** SQLite's default on the last close of a WAL database:
   - take the database's EXCLUSIVE lock;
   - checkpoint the WAL into the file and sync it;
   - delete the WAL.

   Every opening and every reader waits on that lock. In t1171 the setup is serialised, but the
   connection returned is dropped OUTSIDE the gate, so opener k+1 met opener k's close. In t411
   and t640 the second service or opening met the close of the first one's threads.

## D2. Rule: no sidecar lock across an fsync

- `PRAGMA synchronous=NORMAL`. A WAL commit only writes into the page cache. Syncs remain:
  - in checkpoints;
  - once on the header of a new or restarted WAL.
- `SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE`. A close takes no lock and checkpoints nothing. The WAL is
  folded in by the auto-checkpoint after a commit that crosses 1000 pages. That checkpoint is
  PASSIVE: it holds only the checkpoint lock, so readers and writers go on. Its syncs delay only
  the committing thread's return.
- Both are per connection, so `try_open` sets them on every opening, before the journal-mode
  read. Neither writes the file, so the "a ready sidecar's opening writes nothing" rule (t1171)
  holds.
- Measured with the stalled-disk shim:
  - neither setting: red, `DatabaseBusy` on a write after 5 s;
  - NORMAL alone: red, a close stalled in its checkpoint;
  - no checkpoint on close alone: red, a commit stalled and the other thread's write failed
    with `DatabaseBusy`;
  - both: green with zero stalled syncs.

What stays, and why it is acceptable:

- **The first WAL header sync of a NEW sidecar** happens in the setup, inside the gate. In-process
  openers wait on the gate's mutex, not on a busy timeout, so they are delayed and do not fail.
- **The header sync of a WAL restarted after a full checkpoint** (once per ~4 MB of writes) is
  taken under the write lock. Only another WRITER can wait on it. Under a stall past 5 s that
  write is lost, which is a cache miss, as every DB error already is.
- **`-wal`/`-shm` stay next to the sidecar after exit.** The next process's first opening recovers
  the WAL index from the file, a read without a sync. The deletion of a broken sidecar (t872)
  removes both files as before.

## D3. The regression check reproduces the stall, not the load

The test `sidecar_stall.rs` is its own test binary. It installs a SQLite VFS as the process
default:

- The VFS is the default VFS with `xOpen` wrapped. The base opens the file in place, then the
  file's io-methods table is swapped for a copy whose `xSync` blocks while the test's "disk" is
  stalled.
- Every other method is the base's own and gets the same file, so the shim forwards nothing else.

The scenario:

1. The ready sidecar is written while the disk answers.
2. The disk stalls.
3. Two threads, released by one barrier, each open, write, read and close the sidecar five times.

Asserted: both finish within 3 s (under the 5 s busy timeout), with no error and no stalled sync.

On the old code the test is deterministic: one thread's first commit always stalls in the fsync,
and the other always fails with `DatabaseBusy` after 5 s. The test then clears the stall so the
blocked thread can be joined.

**Rejected:**

- **A longer busy timeout.** A 137 s fsync outlasts any sane timeout, and the UI threads would
  wait that long.
- **`synchronous=OFF`.** It also removes the checkpoint syncs, and an OS crash can then corrupt
  the file. Corruption found later in a query, not on opening, would turn into lasting cache
  misses: `open` recreates only a sidecar that fails to open.
- **One shared connection.** Rejected before (t872): it serialises every thread's DB work.
- **Moving the tests' temp dirs to tmpfs.** It would hide the defect. The same disk stall happens
  next to a person's project on a spinning disk.

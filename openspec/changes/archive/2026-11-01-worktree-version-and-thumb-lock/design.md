# Design

## D1. The version watches what git names, not a guessed path

`build.rs` runs in `src/libs/kt-version`; `../../../.git` was the main checkout's git dir. In a worktree
`.git` is a file and the paths did not exist, so cargo was told nothing about git and never reran the
script after a commit (it reruns only on a change of the package's own files).

- git gives the two directories: `--git-dir` (HEAD of THIS checkout — `…/.git/worktrees/<name>` in a
  worktree) and `--git-common-dir` (refs, `packed-refs`). In a plain checkout both are `.git`.
- Watched: HEAD; the loose branch file `refs/heads/<branch>` HEAD points to; `packed-refs`. A detached
  HEAD holds the hash itself — HEAD is enough. A packed-only branch has no loose file: its parent
  directory is watched (never above `refs` — cargo scans a watched directory whole), and the next
  commit creates the file there.
- **Only existing paths.** For a missing path cargo reruns the script on every build; a rerun rebuilds
  `kt-version` and relinks every binary.
- **Rejected:** watching `refs/heads` whole (the former rule) — in a worktree it is shared with every
  parallel session, and each of their commits would relink this tree; `cargo:rerun-if-changed` on the
  `.git` file — it never changes; parsing the `.git` file by hand — git is already required for the
  version string itself, and `rev-parse` also handles `GIT_DIR`, submodules and relative `gitdir:`.
- The function lives in `git_watch.rs`, `#[path]`-included by `build.rs` and by `tests/git_watch.rs`
  (t1170): a build script cannot be unit-tested in place, and a copy would drift.

## D2. The sidecar opening is serialised per path; it writes only when the file needs it

Cause, measured with a debug line in `MediaIndexDb::open` (full kt-app suite and a 12-thread test):
`PRAGMA journal_mode=WAL` returned `SQLITE_BUSY` (extended code 5) after 204 µs — no busy wait — on
the `index-service` thread of a lesson sandbox; on a NEW file, 12 simultaneous openings had one or
three fail after 5.7–6.2 s (the busy timeout outwaited: each opening switches the journal and creates
the schema, and the writers starve each other). t640 is the same race: its openings start the index
worker, the thumbnail worker and the reel-preview job together, and the reel job's failed opening is a
silent re-render (the assertion "second open rendered the reel again").

- **A per-path lock for the setup only** (`open_gate`): journal mode, schema check, creation. After it
  the connection is the thread's own — the independent connections of t872's design stay (a decoder
  and a connection per thread, invariant 5), only the few milliseconds of setup are taken in turn. The
  lock map is bounded by the sidecars a process opens (one per project, the tests' temporary ones).
- **WAL is read before it is set**: it is persistent in the file, so only the creator writes it; every
  later opening only reads (a read never conflicts with a writer under WAL).
- **`BEGIN IMMEDIATE` for the schema**: the write lock is taken up front under the busy handler; a
  deferred `BEGIN` reads first and then may not upgrade without the deadlock check refusing it.
- **Without the lock** (read-before-set and IMMEDIATE only) the 12-thread test is still red 4 of 4: two
  first openers of a new file both switch it to WAL. The lock is necessary, the other two are hygiene
  that keep an opening from writing at all on an existing sidecar (another process — kt-cli — is not
  covered by the lock and meets only reads).
- **Rejected:** a longer busy timeout — the journal-mode BUSY does not wait at all; one shared
  connection per process — rejected in t872's design (it serialises every thread's DB work); retrying
  on BUSY in a loop — hides the collision instead of removing it, and still loses on a slow disk.
- Invariant 8 is untouched: no new decode requests; the reel's disk tier simply hits again.

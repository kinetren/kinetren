# Proposal

## Why

Two defects of master that wave 0–1 executors of round kkt-26.11.01 found on the side
(`side-session-notes`: render-crash-and-recovery, tour-lessons-fixes):

- **A stale version in every worktree.** `libs/kt-version/build.rs` watched `../../../.git/HEAD` and
  `.git/refs/heads`. In a git worktree `.git` is a FILE (`gitdir: …/.git/worktrees/<name>`), neither
  path exists, nothing was watched, and a commit never rebuilt the version: the crash window said
  `kt26.11.01-a498ecfe` while `finish.sh` printed `b6c8f121`. This is the "the owner ran a stale
  program" class `finish.sh` exists against — every executor of the round works in a worktree.
- **t640 red on a quiet machine** with `reel preview: sidecar open failed: sqlite: database is locked`:
  a project opening starts several threads that open `<project>.ktindex.db` at the same moment (index
  worker, thumbnails, reel preview, sources). Each opening WROTE (`PRAGMA journal_mode=WAL`, the
  schema); SQLite returns `SQLITE_BUSY` from the journal-mode switch in 0.2 ms without calling the busy
  handler (measured), and on a new file a dozen openers outwait the 5 s busy timeout. The reel preview
  then loses its disk tier and renders again — the owner's R.67 complaint ("EVERY TIME it recalculates
  the previews") can come back by chance.

## What Changes

- `kt-version`: the watched paths come from git itself (`rev-parse --git-dir` / `--git-common-dir`):
  the checkout's HEAD, the loose file of its branch (or, for a packed-only branch, the directory the
  next commit writes it into), `packed-refs`. Only existing paths (a missing one reruns the script and
  relinks everything on every build); not the whole `refs/heads` (shared with every parallel session).
  No git → nothing watched, the environment/`nogit` rule unchanged. The logic is one file
  (`git_watch.rs`) included by `build.rs` and by the test.
- `kt-media` `MediaIndexDb::open`: the SETUP of an opening is serialised per sidecar path inside the
  process; WAL is set only when the file is not WAL yet (it is a property of the file); the schema is
  created under `BEGIN IMMEDIATE`. The returned connection stays the thread's own.

## Capabilities

- `distribution` — ADDED: the embedded version follows the checked-out commit, in a worktree too (t1170).
- `media-index` — ADDED: simultaneous openings of one sidecar all succeed (t1171; t640 is its UI
  consequence).

## Impact

`src/libs/kt-version/{build.rs,git_watch.rs,tests/git_watch.rs}`, `src/libs/kt-media/src/index.rs`,
`src/libs/kt-media/tests/index_db.rs`; specs `distribution`, `media-index` (+ their design.md);
`kt-media/CLAUDE.md`. No format change, no UI change. What's new: one line for the reel previews
(a person could see a composition preview rebuilt on reopening).

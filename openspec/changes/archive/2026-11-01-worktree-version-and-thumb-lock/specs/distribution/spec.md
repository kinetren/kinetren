# Spec Delta

## ADDED Requirements

### Requirement: The embedded version follows the checked-out commit, in a worktree too
The version baked into `kinetren` and `kt-cli` (`kt_version::FULL`) SHALL name the commit checked out
where they are built, after any commit, checkout or merge, in a plain checkout and in a git worktree
alike (where `.git` is a file). The build script SHALL watch the files git names for that checkout —
its HEAD, the loose file of the branch HEAD points to (or, for a branch that is only packed, the
directory the next commit writes it into) and `packed-refs` — and SHALL watch only paths that exist.
A commit on another branch of the same repository SHALL NOT change any watched file. Without git the
build watches nothing and the version comes from the environment or is `kt00.00.00-nogit`.
Why: the former rule watched `../../../.git/HEAD`; in a worktree it does not exist, the version was
never rebuilt, and the crash window said `kt26.11.01-a498ecfe` while `finish.sh` printed `b6c8f121` —
the "stale program" class `finish.sh` exists against. A missing watched path reruns the script on
every build (a relink of everything), and the whole shared `refs/heads` would relink a worktree on
every parallel session's commit.

#### Scenario: A worktree commit rebuilds the version
- **WHEN** a temporary repository has a worktree on its own branch; a commit is made on the main
  branch, then in the worktree, then the refs are packed and another commit is made in the worktree;
  and a directory outside any repository is asked
- **THEN** the plain checkout watches its `.git/HEAD` and its branch file; the worktree watches its
  HEAD under `.git/worktrees/<name>` and its branch file; the main-branch commit changes none of the
  worktree's watched files; each worktree commit changes one (the packed case through the watched
  directory); outside a repository nothing is watched
- **TEST** t1170 (`src/libs/kt-version/tests/git_watch.rs`)

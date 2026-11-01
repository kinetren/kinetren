# Tasks

## 1. The version in a worktree (distribution)

- [x] 1.1 `kt-version/git_watch.rs`: the watched paths from `rev-parse --git-dir`/`--git-common-dir`
      (HEAD, the branch file or its directory when packed, `packed-refs`; existing only); `build.rs`
      prints them — t1170
- [x] 1.2 Check on this worktree: the build script's output names `.git/worktrees/<name>/HEAD` and
      the branch file; a commit rebuilds the version of `target/debug/kinetren`

## 2. Simultaneous sidecar openings (media-index)

- [x] 2.1 `MediaIndexDb::open`: the per-path setup lock, WAL read before set, `BEGIN IMMEDIATE` for the
      schema — t1171 (red 4 of 4 without the lock)
- [x] 2.2 t640 alone and inside the whole kt-app suite; `kt-media/CLAUDE.md` rule; What's new line

## 3. Close

- [ ] 3.1 `cargo test --workspace`, `cargo clippy --all-targets`
- [ ] 3.2 `git merge master`, `src/scripts/finish.sh` — the version here and in the report; archive

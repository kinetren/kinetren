//! Which files decide the commit the version names (worktree-version-and-thumb-lock).
//!
//! Shared by `build.rs` (it prints them as `cargo:rerun-if-changed`) and the crate's test
//! (`tests/git_watch.rs`, t1170): the test builds the layouts and asserts what is watched.
//!
//! The former rule watched `../../../.git/HEAD` and `.git/refs/heads`. In a git WORKTREE `.git`
//! is a FILE (`gitdir: <main>/.git/worktrees/<name>`), so neither path existed, nothing was
//! watched, and a commit never rebuilt the version: the program said `kt26.11.01-a498ecfe`
//! while `finish.sh` printed `b6c8f121` — the "stale program" class `finish.sh` exists against.
//!
//! git itself names the two directories (`rev-parse --git-dir`, `--git-common-dir`): HEAD lives
//! in the first, the refs and `packed-refs` in the second (in a plain checkout both are `.git`).
//! Watched: that HEAD, the loose file of the branch it points to, and `packed-refs`. A branch
//! that is only packed has no loose file yet; then its parent directory is watched, where
//! the next commit creates it. Only paths that exist are returned: cargo reruns a build
//! script on EVERY build for a missing path, and a rerun here relinks every binary. Not the
//! whole `refs/heads`: in a worktree it is shared with every other session, and each of
//! their commits would relink this tree.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git_in(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").current_dir(dir).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// The files whose change means another commit is checked out in `dir`; empty without git
/// (a tarball build, the release container — the version then comes from the environment).
pub fn watch_paths(dir: &Path) -> Vec<PathBuf> {
    let abs = |p: String| {
        let p = PathBuf::from(p);
        if p.is_absolute() { p } else { dir.join(p) }
    };
    let Some(git_dir) = git_in(dir, &["rev-parse", "--git-dir"]).map(abs) else {
        return Vec::new();
    };
    let common = git_in(dir, &["rev-parse", "--git-common-dir"]).map(abs).unwrap_or(git_dir.clone());
    let head = git_dir.join("HEAD");
    let mut out = vec![head.clone()];
    // `ref: refs/heads/<branch>` — a detached HEAD holds the hash itself, HEAD is enough
    if let Some(r) = std::fs::read_to_string(&head)
        .ok()
        .and_then(|s| s.trim().strip_prefix("ref:").map(|r| r.trim().to_string()))
    {
        let loose = common.join(&r);
        if loose.is_file() {
            out.push(loose);
        } else if let Some(parent) = loose.parent() {
            // packed only: the next commit writes the loose file here (never above `refs`:
            // cargo scans a watched directory whole, and the git dir holds the objects)
            let refs = common.join("refs");
            let mut d = parent.to_path_buf();
            while !d.is_dir() && d.starts_with(&refs) && d.pop() {}
            if d.starts_with(&refs) {
                out.push(d);
            }
        }
    }
    out.push(common.join("packed-refs"));
    out.retain(|p| p.exists());
    out
}

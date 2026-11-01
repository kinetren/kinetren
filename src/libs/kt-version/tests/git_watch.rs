//! Test 1170 (worktree-version-and-thumb-lock): a commit in a git WORKTREE changes a file the
//! version's build script watches.
//!
//! The defect: `build.rs` watched `../../../.git/HEAD` and `.git/refs/heads`; in a worktree
//! `.git` is a file, nothing was watched, and the binary kept the hash of an older commit.
//! What is asserted is the consequence cargo acts on: among the watched paths there is one
//! whose content changes with the commit (the old rule returned nothing here). And the
//! reverse: a commit on ANOTHER branch of the same repository changes none of them — in a
//! worktree the refs are shared with every parallel session, and each of their commits must
//! not relink this tree.

#[path = "../git_watch.rs"]
mod git_watch;

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", if cfg!(windows) { "NUL" } else { "/dev/null" })
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn commit(dir: &Path, n: &str) {
    std::fs::write(dir.join("f.txt"), n).unwrap();
    git(dir, &["add", "f.txt"]);
    git(dir, &["commit", "-q", "-m", n]);
}

/// What cargo compares, made comparable across fast commits: a file by its content, a
/// directory by its entries (an mtime may not tick within one second).
fn snapshot(paths: &[PathBuf]) -> Vec<(PathBuf, Vec<String>)> {
    paths
        .iter()
        .map(|p| {
            let v = if p.is_dir() {
                let mut e: Vec<String> = std::fs::read_dir(p)
                    .unwrap()
                    .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect();
                e.sort();
                e
            } else {
                vec![std::fs::read_to_string(p).unwrap_or_default()]
            };
            (p.clone(), v)
        })
        .collect()
}

fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap()
}

#[test]
fn t1170_worktree_commit_changes_a_watched_file() {
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("t1170: no git on this machine — nothing to watch, nothing to check");
        return;
    }
    let root = std::env::temp_dir().join(format!("kt-t1170-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let main = root.join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q", "-b", "master"]);
    commit(&main, "one");
    git(&main, &["worktree", "add", "-q", "-b", "feature", "../wt"]);
    let wt = root.join("wt");
    // the scene: `.git` of the worktree is a FILE, the layout the old rule could not see
    assert!(wt.join(".git").is_file(), "the scene degenerated: .git of a worktree is not a file");
    assert!(!wt.join(".git/HEAD").exists());

    // a plain checkout: its HEAD and the branch file
    let plain: Vec<PathBuf> = git_watch::watch_paths(&main).iter().map(|p| canon(p)).collect();
    assert!(plain.contains(&canon(&main.join(".git/HEAD"))), "plain checkout: {plain:?}");
    assert!(plain.contains(&canon(&main.join(".git/refs/heads/master"))), "plain: {plain:?}");

    // the worktree: ITS HEAD (in the main git dir) and ITS branch
    let watched = git_watch::watch_paths(&wt);
    let names: Vec<PathBuf> = watched.iter().map(|p| canon(p)).collect();
    assert!(names.contains(&canon(&main.join(".git/worktrees/wt/HEAD"))), "worktree: {names:?}");
    assert!(names.contains(&canon(&main.join(".git/refs/heads/feature"))), "worktree: {names:?}");

    // a commit on ANOTHER branch (another session's) touches nothing watched here
    let before = snapshot(&watched);
    commit(&main, "two");
    assert_eq!(snapshot(&watched), before, "a commit on master would relink the worktree");

    // a commit in the worktree changes a watched file — cargo reruns build.rs
    commit(&wt, "three");
    assert_ne!(snapshot(&watched), before, "a worktree commit changed no watched file");

    // a branch that is only PACKED: its parent directory is watched, and the next commit
    // (which writes the loose file there) changes it
    git(&wt, &["pack-refs", "--all"]);
    assert!(!main.join(".git/refs/heads/feature").exists(), "pack-refs left the loose ref");
    let packed = git_watch::watch_paths(&wt);
    let before = snapshot(&packed);
    commit(&wt, "four");
    assert_ne!(snapshot(&packed), before, "a commit on a packed branch changed nothing watched");

    // no repository (the temp dir is not inside one): nothing — a missing path would rerun
    // the script on every build
    let bare = root.join("plain-dir");
    std::fs::create_dir_all(&bare).unwrap();
    let inside_repo = Command::new("git")
        .current_dir(&bare)
        .args(["rev-parse", "--git-dir"])
        .output()
        .is_ok_and(|o| o.status.success());
    if !inside_repo {
        assert_eq!(git_watch::watch_paths(&bare), Vec::<PathBuf>::new());
    }

    let _ = std::fs::remove_dir_all(&root);
}

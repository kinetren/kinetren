//! Build version (item 6 of round 12, prefix — item 3 of round 13): `ktYY.MM.DD-<hash8>` by the
//! date of the COMMIT. The prefix is the same everywhere: the short ktYY.MM is simply a
//! truncated full one, and the user need not keep two formats in mind.
//!
//! The version in the manifest stays 0.1.0 — semver does not accept “v26.08.27-…”.
//! The displayed version is computed here, in three ways by priority:
//!   1. the environment variables KT_VERSION_FULL/KT_VERSION_SHORT — this is how release-build.sh
//!      passes them in: inside the build container there is no
//!      git and no .git directory (only src/ is mounted);
//!   2. git in the working tree;
//!   3. the fallback “v00.00.00-nogit” — so the build does not fail anywhere.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn main() {
    // rebuild on a change of commit and on an external substitution of the version
    println!("cargo:rerun-if-env-changed=KT_VERSION_FULL");
    println!("cargo:rerun-if-env-changed=KT_VERSION_SHORT");
    for p in ["../../../.git/HEAD", "../../../.git/refs/heads"] {
        if std::path::Path::new(p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }

    let full = std::env::var("KT_VERSION_FULL").ok().or_else(|| {
        let date = git(&["log", "-1", "--format=%cd", "--date=format:%y.%m.%d"])?;
        let hash = git(&["rev-parse", "--short=8", "HEAD"])?;
        Some(format!("kt{date}-{hash}"))
    });
    let full = full.unwrap_or_else(|| "kt00.00.00-nogit".to_string());
    // short: ktYY.MM from the same digits
    let short = std::env::var("KT_VERSION_SHORT").ok().unwrap_or_else(|| {
        let digits: String = full.trim_start_matches("kt").chars().take(5).collect();
        format!("kt{digits}")
    });

    println!("cargo:rustc-env=KT_VERSION_FULL={full}");
    println!("cargo:rustc-env=KT_VERSION_SHORT={short}");
}

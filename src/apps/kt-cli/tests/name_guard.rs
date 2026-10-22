//! t833 (kinetren-rename): the program is called Kinetren. The former name
//! may appear only where the rename itself is handled — the former project
//! marker and its import repair, the one-time move of the data directory,
//! the cleanup in install.sh, the refusal message and their tests — or in
//! deliberately frozen data. A new "Kinotrain" anywhere else is a missed
//! rename and turns this test red.

use std::path::{Path, PathBuf};

/// Files whose former-name text is frozen data: title golden hashes (the
/// sample text is part of the pinned render) and the section heading of
/// the feature notes, which `features-index.py` checks in ~100 documents.
const FROZEN: &[&str] = &[
    "apps/kt-app/src/layout_tests.rs",
    "libs/kt-engine/tests/title_golden.rs",
    "libs/kt-text/src/lib.rs",
    "scripts/features-index.py",
];

/// A line that handles the rename says so.
fn handles_the_rename(line: &str) -> bool {
    let l = line.to_lowercase();
    l.contains("former") || l.contains("2026-10-21") || l.contains("previous")
        || l.contains("\"kinotrain\"") || l.contains("\\\"kinotrain\\\"")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.is_dir() {
            if name != "target" && name != "out" && name != "vendor" && !name.starts_with('.') {
                walk(&p, out);
            }
        } else if [".rs", ".toml", ".py", ".sh", ".xml", ".desktop", ".svg", ".md"]
            .iter()
            .any(|x| name.ends_with(x))
        {
            out.push(p);
        }
    }
}

#[test]
fn t833_the_former_name_appears_only_where_the_rename_is_handled() {
    let rust = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let mut files = Vec::new();
    for sub in ["apps", "libs", "packaging", "scripts"] {
        walk(&rust.join(sub), &mut files);
    }
    let mut stray = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&rust).unwrap().to_string_lossy().replace('\\', "/");
        if FROZEN.contains(&rel.as_str()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        for (n, line) in text.lines().enumerate() {
            if line.to_lowercase().contains("kinotrain") && !handles_the_rename(line) {
                stray.push(format!("src/{rel}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    assert!(stray.is_empty(), "the former name outside the rename handling:\n{}", stray.join("\n"));
}

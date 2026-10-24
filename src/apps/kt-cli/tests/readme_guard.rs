//! t826/t827 (readme-showcase): the guard of the repository's public face —
//! `src/scripts/readme-check.py`. README, CONTRIBUTING and docs/guide are
//! English, their links and images resolve, every screenshot is produced by
//! the pipeline (`SHOTS` of `src/scripts/live/readme_shots.py`) and none is
//! left unreferenced or over budget.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    // CARGO_MANIFEST_DIR = src/apps/kt-cli
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn guard(root: &Path) -> (bool, String) {
    let out = Command::new("python3")
        .arg(repo().join("src/scripts/readme-check.py"))
        .arg("--check")
        .arg("--root")
        .arg(root)
        .output()
        .expect("python3 for the README guard");
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

#[test]
fn t826_readme_and_guide_pass_the_guard() {
    if cfg!(windows) {
        eprintln!("t826 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let (ok, text) = guard(&repo());
    assert!(ok, "README/docs guard is red:\n{text}");
}

/// A minimal tree: the declared shots, a README showing each, one guide page.
fn clean_tree(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kt-readme-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("docs/images")).unwrap();
    std::fs::create_dir_all(dir.join("docs/guide")).unwrap();
    std::fs::create_dir_all(dir.join("src/scripts/live")).unwrap();
    std::fs::write(
        dir.join("src/scripts/live/readme_shots.py"),
        "SHOTS = [\n    (\"hero\", \"the window\"),\n    (\"timeline\", \"the timeline\"),\n]\n",
    )
    .unwrap();
    for name in ["hero", "timeline"] {
        std::fs::write(dir.join(format!("docs/images/{name}.webp")), [0u8; 64]).unwrap();
    }
    std::fs::write(
        dir.join("README.md"),
        "# Kinetren\n\n![hero](docs/images/hero.webp)\n\nSee [the guide](docs/guide/start.md#first-steps).\n\n\
         ```bash\n# a code block is not checked for links: [x](missing.md)\n```\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("docs/guide/start.md"),
        "# Start\n\n## First steps\n\n<img src=\"../images/timeline.webp\" alt=\"timeline\">\n\n[Back](../../README.md)\n",
    )
    .unwrap();
    dir
}

#[test]
fn t827_guard_catches_each_class_of_defect() {
    if cfg!(windows) {
        eprintln!("t827 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let dir = clean_tree("clean");
    let (ok, text) = guard(&dir);
    assert!(ok, "the clean tree must pass:\n{text}");
    let _ = std::fs::remove_dir_all(&dir);

    type Break = fn(&Path);
    let cases: [(&str, Break, &str); 7] = [
        ("cyrillic", |d| append(d, "README.md", "\nКиноплёнка\n"), "README.md:11: cyrillic"),
        ("link", |d| append(d, "README.md", "\n[gone](docs/nope.md)\n"), "README.md:11: link"),
        ("anchor", |d| append(d, "README.md", "\n[x](docs/guide/start.md#nowhere)\n"), "link"),
        ("image", |d| append(d, "README.md", "\n![x](docs/images/missing.webp)\n"), "README.md:11: image"),
        (
            "orphan",
            |d| std::fs::write(d.join("docs/images/extra.webp"), [0u8; 8]).unwrap(),
            "docs/images/extra.webp:0: orphan",
        ),
        (
            "size",
            |d| std::fs::write(d.join("docs/images/hero.webp"), vec![0u8; 901 * 1024]).unwrap(),
            "docs/images/hero.webp:0: size",
        ),
        (
            "shots",
            |d| {
                let p = d.join("src/scripts/live/readme_shots.py");
                let s = std::fs::read_to_string(&p).unwrap();
                std::fs::write(p, s.replace("]\n", "    (\"morpher\", \"m\"),\n]\n")).unwrap();
            },
            "shots: declared shot “morpher”",
        ),
    ];
    for (tag, break_it, want) in cases {
        let dir = clean_tree(tag);
        break_it(&dir);
        let (ok, text) = guard(&dir);
        assert!(!ok, "{tag}: the guard must be red:\n{text}");
        assert!(text.contains(want), "{tag}: expected “{want}” in:\n{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn append(dir: &Path, file: &str, text: &str) {
    let p = dir.join(file);
    let s = std::fs::read_to_string(&p).unwrap();
    std::fs::write(p, s + text).unwrap();
}

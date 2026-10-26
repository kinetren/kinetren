//! t826/t827/t869 (showcase): the guard of the repository's public face —
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
         <img src=\"docs/images/timeline.webp\" alt=\"timeline\">\n\n\
         ```bash\n# a code block is not checked for links: [x](missing.md)\n```\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("docs/guide/start.md"),
        // the guide holds only the Markdown the site renders (the `markup` class,
        // t869): an HTML <img> lives in the README, an image here is `![]()`
        "# Start\n\n## First steps\n\n![timeline](../images/timeline.webp)\n\n[Back](../../README.md)\n",
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
        ("cyrillic", |d| append(d, "README.md", "\nКиноплёнка\n"), "README.md:13: cyrillic"),
        ("link", |d| append(d, "README.md", "\n[gone](docs/nope.md)\n"), "README.md:13: link"),
        ("anchor", |d| append(d, "README.md", "\n[x](docs/guide/start.md#nowhere)\n"), "link"),
        ("image", |d| append(d, "README.md", "\n![x](docs/images/missing.webp)\n"), "README.md:13: image"),
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

/// The clean tree plus a shortcuts page written the way the site renders it:
/// `<kbd>` caps, `\|` in a cell, and the forbidden constructs only inside code.
fn clean_guide_tree(tag: &str) -> PathBuf {
    let dir = clean_tree(tag);
    std::fs::write(
        dir.join("docs/guide/shortcuts.md"),
        "# Keyboard and mouse\n\n## Transport\n\n| Keys | Action |\n|---|---|\n\
         | <kbd>Space</kbd> | play / pause |\n\
         | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd> / <kbd>Ctrl</kbd>+<kbd>Y</kbd> | redo |\n\
         | `a \\| b` and a \\| pipe | a pipe in a cell |\n\n\
         The user_store keeps it; **careful** with *this* one.\n\n\
         - one\n  - nested once\n- two\n\n\
         Code is not checked: `<br>`, `_x_`, `**Ctrl+S**`.\n\n\
         ```text\n> quoted\n<br>\n[x][y]\n_italic_ \\ back\n```\n",
    )
    .unwrap();
    dir
}

/// t869 (guide-shortcuts-reference): the `markup` class — the guide holds only the
/// Markdown the site (kinetren.github.io) renders, and shortcuts.md writes keys as
/// `<kbd>` caps in `| Keys | Action |` tables. Each defect alone must turn the guard
/// red, naming the file, the line and the class.
#[test]
fn t869_guard_keeps_the_guide_in_the_sites_markdown() {
    if cfg!(windows) {
        eprintln!("t869 skipped on Windows: a repository guard run by python3; it runs in the Linux job");
        return;
    }
    let dir = clean_guide_tree("markup-clean");
    let (ok, text) = guard(&dir);
    assert!(ok, "the clean guide must pass:\n{text}");
    let _ = std::fs::remove_dir_all(&dir);

    // start.md ends with an empty line 8: an appended "\nX\n" puts X on line 9
    let start = |d: &Path, s: &str| append(d, "docs/guide/start.md", s);
    let keys = |d: &Path, from: &str, to: &str| {
        let p = d.join("docs/guide/shortcuts.md");
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(from), "the fixture lost “{from}”");
        std::fs::write(p, s.replacen(from, to, 1)).unwrap();
    };
    type Break = Box<dyn Fn(&Path)>;
    let cases: Vec<(&str, Break, &str)> = vec![
        ("html", Box::new(move |d| start(d, "\nline<br>break\n")), "docs/guide/start.md:9: markup"),
        ("kbd-unpaired", Box::new(move |d| start(d, "\n<kbd>Ctrl\n")), "docs/guide/start.md:9: markup"),
        ("quote", Box::new(move |d| start(d, "\n> a quote\n")), "docs/guide/start.md:9: markup"),
        ("setext", Box::new(move |d| start(d, "\nTitle\n=====\n")), "docs/guide/start.md:10: markup"),
        ("ref-link", Box::new(move |d| start(d, "\nsee [x][y]\n")), "docs/guide/start.md:9: markup"),
        ("ref-def", Box::new(move |d| start(d, "\n[y]: start.md\n")), "docs/guide/start.md:9: markup"),
        ("footnote", Box::new(move |d| start(d, "\na note[^1]\n")), "docs/guide/start.md:9: markup"),
        ("italic", Box::new(move |d| start(d, "\nan _italic_ word\n")), "docs/guide/start.md:9: markup"),
        ("backslash", Box::new(move |d| start(d, "\na \\ slash\n")), "docs/guide/start.md:9: markup"),
        (
            "nesting",
            Box::new(move |d| start(d, "\n- a\n  - b\n    - c\n")),
            "docs/guide/start.md:11: markup",
        ),
        (
            "header",
            Box::new(move |d| keys(d, "| Keys | Action |", "| Key | Action |")),
            "docs/guide/shortcuts.md:5: markup",
        ),
        (
            "bold-chord",
            Box::new(move |d| keys(d, "| <kbd>Space</kbd> |", "| **Ctrl+S** |")),
            "docs/guide/shortcuts.md:7: markup",
        ),
        (
            "bold-key",
            Box::new(move |d| keys(d, "| <kbd>Space</kbd> |", "| **Space** |")),
            "docs/guide/shortcuts.md:7: markup",
        ),
    ];
    for (tag, break_it, want) in cases {
        let dir = clean_guide_tree(&format!("markup-{tag}"));
        break_it(&dir);
        let (ok, text) = guard(&dir);
        assert!(!ok, "{tag}: the guard must be red:\n{text}");
        assert!(text.contains(want), "{tag}: expected “{want}” in:\n{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

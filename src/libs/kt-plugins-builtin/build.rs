//! The source hash of every family crate: the version of its plugins in the keys of the frames kept on
//! disk (project-cache-folder D1b). Nobody has to move it by hand, so nobody can forget.

use std::path::{Path, PathBuf};

/// FNV-1a over a file's lines, comments and blank lines left out: an edit that can change no pixel
/// makes nothing stale.
fn hash_file(h: &mut u64, path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
    let text = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for line in String::from_utf8_lossy(&text).lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with("//") {
            continue;
        }
        for b in l.bytes().chain([b'\n']) {
            *h = (*h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

const FNV: u64 = 0xcbf2_9ce4_8422_2325;

/// The files under `dir`, by name.
fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut all: Vec<PathBuf> =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).map(|e| e.unwrap().path()).collect();
    all.sort();
    for p in all {
        if p.is_dir() {
            files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs" || x == "wgsl") {
            out.push(p);
        }
    }
}

fn main() {
    let root = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../plugins");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut crates: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("src/plugins")
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("src").is_dir())
        .collect();
    crates.sort();
    let mut text = String::from("const SOURCES: &[(&str, u64)] = &[\n");
    for c in crates {
        let mut h = FNV;
        let mut all = Vec::new();
        files(&c.join("src"), &mut all);
        for f in all {
            hash_file(&mut h, &f);
        }
        text.push_str(&format!("    ({:?}, 0x{h:016x}),\n", c.file_name().unwrap().to_string_lossy()));
    }
    text.push_str("];\n");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("sources.rs"), text).unwrap();
}

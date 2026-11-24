//! `COMPOSITE_PATH_VERSION`: what the composite code outside the plugins' kernels is
//! (project-cache-folder D1b) — the fusion, `plan_chain`, the core passes and shaders, the conversions,
//! the titles and the CPU generators. A frame kept on disk under another is a miss.

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
        for b in l.bytes().chain(*b"\n") {
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

/// The files, from `src/libs` (a folder: everything in it). t3808 reads the same list.
const PATHS: &str = include_str!("composite-path.txt");

fn main() {
    let libs = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("..");
    println!("cargo:rerun-if-changed=composite-path.txt");
    let mut all = Vec::new();
    for p in PATHS.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let p = libs.join(p);
        if p.is_dir() {
            files(&p, &mut all);
        } else {
            all.push(p);
        }
    }
    let mut h = FNV;
    for f in &all {
        hash_file(&mut h, f);
    }
    let text = format!("/// The composite code outside the kernels, as it is in this build.\npub const COMPOSITE_PATH_VERSION: u64 = 0x{h:016x};\n");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("composite_path.rs"), text).unwrap();
}

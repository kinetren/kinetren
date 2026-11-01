//! t1066 (ci-green-again, invariant 11): the crates of the frozen paths call no transcendental
//! function of the platform — only `kt_plugin_api::math`.
//!
//! `f32::sin`, `f64::powf` and their kin call the platform's libm, and glibc and the Windows one
//! round differently in the last bit: the references printed on Linux failed on windows-latest
//! (t973c, t1019, t1048 on 94e36e9e). Exact functions (`sqrt`, `floor`, `rem_euclid`, `powi`,
//! `to_radians`…) are not named here. Third-party code is not scanned: rubato's sinc table was
//! replaced by our own (`kt-fx/src/audio/sinc.rs`), rustfft's twiddles are `f64` rounded to `f32`.

use std::path::{Path, PathBuf};

/// The crates whose output is frozen by a reference (audio, CPU pixels, generators, curves).
const CRATES: &[&str] = &[
    "libs/kt-time",
    "libs/kt-model",
    "libs/kt-plugin-api",
    "libs/kt-fx",
    "libs/kt-text",
    "libs/kt-media",
    "libs/kt-engine",
];

const FUNCS: &str = "sin|cos|tan|exp|exp2|exp_m1|ln|ln_1p|log|log2|log10|powf|atan|atan2|asin|acos|\
                     sinh|cosh|tanh|asinh|acosh|atanh|cbrt|hypot|sin_cos|mul_add";

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// The platform calls in one source text: `x.sin(`, `f32::powf(` — not in a `//` comment.
fn platform_calls(text: &str) -> Vec<(usize, String)> {
    let funcs: Vec<&str> = FUNCS.split('|').collect();
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for f in &funcs {
            for pre in [".", "f32::", "f64::"] {
                let pat = format!("{pre}{f}(");
                let mut from = 0;
                // the dot (or path) right before the name keeps `.asin(` from matching `.sin(`
                // and our `.fsin(` from matching either
                while let Some(i) = code[from..].find(&pat) {
                    out.push((n + 1, line.trim().to_string()));
                    from += i + pat.len();
                }
            }
        }
    }
    out
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn t1066_frozen_paths_call_only_our_math() {
    // the scanner itself, on lines it must and must not name
    let probe = "let a = x.sin();\nlet b = 10f32.powf(g);\nlet c = f64::exp(t);\n// x.cos() in a comment\n\
                 let d = x.fsin() + y.sqrt() + z.rem_euclid(1.0) + w.powi(2);\nlet e = v.asin();";
    let hits: Vec<usize> = platform_calls(probe).iter().map(|h| h.0).collect();
    assert_eq!(hits, vec![1, 2, 3, 6], "the scanner itself is wrong");

    let root = src();
    let mut dirs: Vec<PathBuf> = CRATES.iter().map(|c| root.join(c).join("src")).collect();
    for e in std::fs::read_dir(root.join("plugins")).unwrap().flatten() {
        dirs.push(e.path().join("src"));
    }
    let mut bad = Vec::new();
    for d in dirs {
        let mut files = Vec::new();
        sources(&d, &mut files);
        for f in files {
            if f.ends_with("kt-plugin-api/src/math.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            for (n, line) in platform_calls(&text) {
                bad.push(format!("{}:{n}: {line}", f.strip_prefix(&root).unwrap().display()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "the platform's libm on a frozen path — call kt_plugin_api::math (FrozenMath) instead:\n{}",
        bad.join("\n")
    );
}

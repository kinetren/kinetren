//! Code generation for translation catalogs (R.19).
//!
//! Reads `i18n/*.toml`, validates them and writes two inserts to `$OUT_DIR`:
//! `i18n_keys.rs` (key constants as a tree of sections) and `i18n_tables.rs`
//! (one `&'static str` array per language). This approach was chosen for three things
//! at once: `t()` returns `&'static str` without allocations, the translator edits a plain
//! TOML file, and a missing key fails the BUILD instead of showing an empty label.
//!
//! The Arabic table is reordered HERE (`bidi::visual`): at runtime this costs
//! neither time nor cache — egui’s galley cache is keyed by the already
//! finished string.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

#[path = "src/i18n/bidi.rs"]
mod bidi;
#[path = "src/i18n/catalog.rs"]
mod catalog;

/// Order = order of the `Lang` variants and the tables in `tables.rs`.
const LANG_FILES: [(&str, &str); 5] = [
    ("EN", "en.toml"),
    ("RU", "ru.toml"),
    ("ES", "es.toml"),
    ("ZH", "zh-Hans.toml"),
    ("AR", "ar.toml"),
];

/// Endonym keys for languages in the order of `Lang::ALL`.
const ENDONYM_KEYS: [&str; 5] = ["lang.en", "lang.ru", "lang.es", "lang.zh", "lang.ar"];

/// Sections that are NOT reordered for Arabic: their values
/// are compared with what the user types (FX search), or they are
/// endonyms identical in all catalogs.
const NO_BIDI_SECTIONS: [&str; 2] = ["fxreg.kw.", "lang."];

/// Length ceiling for a value. A reordered Arabic paragraph cannot be wrapped
/// again (each visual line would need its own order), so the
/// catalog must not contain text that someone would want to wrap.
const MAX_VALUE_CHARS: usize = 160;

fn fail(msg: String) -> ! {
    println!("cargo:warning={msg}");
    panic!("{msg}");
}

fn main() {
    windows_resource();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("i18n");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut cats: Vec<Vec<catalog::Entry>> = Vec::new();
    for (_, file) in LANG_FILES {
        let p = dir.join(file);
        println!("cargo:rerun-if-changed={}", p.display());
        let src = std::fs::read_to_string(&p).unwrap_or_else(|e| fail(format!("i18n/{file}: {e}")));
        let entries = catalog::parse(&src).unwrap_or_else(|e| fail(format!("i18n/{file}: {e}")));
        cats.push(entries);
    }

    // en.toml defines the set and the ORDER of keys; the index in it is the Key.
    let reference: Vec<catalog::Entry> = cats[0].clone();
    let index: BTreeMap<&str, usize> =
        reference.iter().enumerate().map(|(i, e)| (e.key.as_str(), i)).collect();

    if reference.len() > u16::MAX as usize {
        fail(format!("i18n: too many keys ({})", reference.len()));
    }

    let mut values: Vec<Vec<String>> = Vec::new();
    for (li, (name, file)) in LANG_FILES.iter().enumerate() {
        let entries = &cats[li];
        let have: BTreeSet<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        for e in &reference {
            if !have.contains(e.key.as_str()) {
                fail(format!("i18n/{file}: missing key {:?}", e.key));
            }
        }
        let mut row = vec![String::new(); reference.len()];
        for e in entries {
            let Some(&i) = index.get(e.key.as_str()) else {
                fail(format!("i18n/{file}:{}: key {:?} is not in en.toml", e.line, e.key));
            };
            check_value(file, name, e);
            if catalog::placeholders(&e.value) != catalog::placeholders(&reference[i].value) {
                fail(format!(
                    "i18n/{file}:{}: key {:?} has placeholders {:?}, en.toml has {:?}",
                    e.line,
                    e.key,
                    catalog::placeholders(&e.value),
                    catalog::placeholders(&reference[i].value)
                ));
            }
            if e.key.starts_with("lang.") && e.value != reference[i].value {
                fail(format!(
                    "i18n/{file}:{}: {:?} is an endonym and must be identical in every catalog",
                    e.line, e.key
                ));
            }
            row[i] = e.value.clone();
        }
        values.push(row);
    }

    // What is not yet translated. A match can be legitimate (“Mbit/s”, “Vertical”,
    // effect names), so this is a warning, not an error — but the KEYS
    // are named one by one: a bare counter forces guessing what stands behind it.
    for (li, (name, _)) in LANG_FILES.iter().enumerate().skip(1) {
        let same: Vec<&str> = (0..reference.len())
            .filter(|&i| !reference[i].key.starts_with("lang.") && values[li][i] == values[0][i])
            .map(|i| reference[i].key.as_str())
            .collect();
        if !same.is_empty() {
            println!(
                "cargo:warning=i18n {name}: {} value(s) identical to English: {}",
                same.len(),
                same.join(" ")
            );
        }
    }

    let ar = LANG_FILES.iter().position(|(n, _)| *n == "AR").unwrap();
    let ar_logical = values[ar].clone();
    for (i, e) in reference.iter().enumerate() {
        if !NO_BIDI_SECTIONS.iter().any(|s| e.key.starts_with(s)) {
            values[ar][i] = bidi::visual(&ar_logical[i]);
        }
    }

    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(Path::new(&out).join("i18n_keys.rs"), gen_keys(&reference))
        .expect("write i18n_keys.rs");
    std::fs::write(
        Path::new(&out).join("i18n_tables.rs"),
        gen_tables(&reference, &values, &ar_logical),
    )
    .expect("write i18n_tables.rs");
}

fn check_value(file: &str, name: &str, e: &catalog::Entry) {
    if e.value.is_empty() {
        fail(format!("i18n/{file}:{}: {:?} is empty", e.line, e.key));
    }
    if e.value.chars().count() > MAX_VALUE_CHARS {
        fail(format!(
            "i18n/{file}:{}: {:?} is {} chars, limit is {MAX_VALUE_CHARS} (split it into two keys)",
            e.line,
            e.key,
            e.value.chars().count()
        ));
    }
    if let Some(c) = e.value.chars().find(|c| matches!(*c as u32, 0xE000..=0xF8FF)) {
        fail(format!(
            "i18n/{file}:{}: {:?} contains private-use U+{:04X}; icons are composed in Rust, not in catalogs",
            e.line, e.key, c as u32
        ));
    }
    // ASCII punctuation in Arabic splits the word into atoms and breaks the cursive script:
    // Arabic signs are needed; they are inside the Arabic block.
    if name == "AR"
        && let Some(c) = e.value.chars().find(|c| matches!(c, ',' | ';' | '?'))
        && e.value.chars().any(bidi::is_arabic)
    {
        fail(format!(
            "i18n/{file}:{}: {:?} uses ASCII {c:?}; use Arabic punctuation (U+060C, U+061B, U+061F)",
            e.line, e.key
        ));
    }
}

/// Rust keywords that cannot be used as a module name even as `r#`.
const HARD_RESERVED: [&str; 4] = ["crate", "self", "super", "Self"];

fn seg_ident(seg: &str) -> String {
    if HARD_RESERVED.contains(&seg) {
        fail(format!("i18n: section segment {seg:?} cannot be a Rust module"));
    }
    // `r#` is safe for any other keyword (loop, type, move, …)
    let kw = [
        "as", "break", "const", "continue", "dyn", "else", "enum", "extern", "false", "fn", "for",
        "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
        "static", "struct", "trait", "true", "type", "unsafe", "use", "where", "while", "async",
        "await", "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof",
        "unsized", "virtual", "yield", "try", "gen",
    ];
    if kw.contains(&seg) { format!("r#{seg}") } else { seg.to_string() }
}

#[derive(Default)]
struct Node {
    consts: Vec<(String, usize)>,
    kids: BTreeMap<String, Node>,
}

fn gen_keys(reference: &[catalog::Entry]) -> String {
    let mut root = Node::default();
    for (i, e) in reference.iter().enumerate() {
        let parts: Vec<&str> = e.key.split('.').collect();
        let (last, path) = parts.split_last().expect("non-empty key");
        let mut node = &mut root;
        for seg in path {
            node = node.kids.entry((*seg).to_string()).or_default();
        }
        node.consts.push((last.to_uppercase(), i));
    }

    let mut s = String::new();
    s.push_str("// Generated by build.rs from i18n/en.toml - do not edit by hand.\n");
    let _ = writeln!(s, "#[allow(dead_code)]\npub const N_KEYS: usize = {};", reference.len());
    // some keys are used not via the `k::…` path (endonyms go into ENDONYM),
    // and the tables are read only by the i18n accessors — dead_code here is a false positive
    s.push_str("#[allow(dead_code)]\npub mod k {\n");
    emit_node(&mut s, &root, 1);
    s.push_str("}\n");
    s
}

fn emit_node(s: &mut String, node: &Node, depth: usize) {
    let pad = "    ".repeat(depth);
    for (name, i) in &node.consts {
        let _ = writeln!(s, "{pad}pub const {name}: crate::i18n::Key = crate::i18n::Key({i});");
    }
    for (seg, kid) in &node.kids {
        let _ = writeln!(s, "{pad}pub mod {} {{", seg_ident(seg));
        emit_node(s, kid, depth + 1);
        let _ = writeln!(s, "{pad}}}");
    }
}

fn lit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn gen_tables(
    reference: &[catalog::Entry],
    values: &[Vec<String>],
    ar_logical: &[String],
) -> String {
    let n = reference.len();
    let mut s = String::new();
    s.push_str("// Generated by build.rs from i18n/*.toml - do not edit by hand.\n");

    let _ = writeln!(s, "pub static KEY_NAMES: [&str; {n}] = [");
    for e in reference {
        let _ = writeln!(s, "    {},", lit(&e.key));
    }
    s.push_str("];\n");

    for (li, (name, _)) in LANG_FILES.iter().enumerate() {
        let _ = writeln!(s, "static {name}: [&str; {n}] = [");
        for v in &values[li] {
            let _ = writeln!(s, "    {},", lit(v));
        }
        s.push_str("];\n");
    }

    let names: Vec<&str> = LANG_FILES.iter().map(|(n, _)| *n).collect();
    let _ = writeln!(
        s,
        "pub static TABLES: [&[&str; {n}]; {}] = [{}];",
        names.len(),
        names.iter().map(|n| format!("&{n}")).collect::<Vec<_>>().join(", ")
    );

    let _ = writeln!(s, "#[allow(dead_code)]\npub static AR_LOGICAL: [&str; {n}] = [");
    for v in ar_logical {
        let _ = writeln!(s, "    {},", lit(v));
    }
    s.push_str("];\n");

    let idx: BTreeMap<&str, usize> =
        reference.iter().enumerate().map(|(i, e)| (e.key.as_str(), i)).collect();
    let endonyms: Vec<String> = ENDONYM_KEYS
        .iter()
        .map(|k| {
            let i = idx
                .get(k)
                .unwrap_or_else(|| fail(format!("i18n/en.toml: missing endonym key {k:?}")));
            lit(&values[0][*i])
        })
        .collect();
    let _ = writeln!(s, "pub static ENDONYM: [&str; 5] = [{}];", endonyms.join(", "));
    s
}

/// kinetren.exe carries the Kinetren icon and its version (windows-build): Explorer shows them
/// in the file list and in Properties. Only for a Windows TARGET (a cross-build runs this script
/// on Linux, so `cfg!(windows)` would describe the wrong machine); mingw's windres compiles it.
fn windows_resource() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // the run-time variable: a build-script binary reused from another checkout path would
    // carry a stale compile-time `env!` path
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let ico = Path::new(&manifest).join("../../packaging/kinetren.ico");
    println!("cargo:rerun-if-changed={}", ico.display());
    println!("cargo:rerun-if-env-changed=KT_VERSION_FULL");
    let version = std::env::var("KT_VERSION_FULL").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    let ico = ico.canonicalize().expect("packaging/kinetren.ico").to_string_lossy().replace('\\', "/");
    let rc = format!(
        r#"1 ICON "{ico}"
1 VERSIONINFO
FILEVERSION 0,1,0,0
PRODUCTVERSION 0,1,0,0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "Kinetren"
      VALUE "FileDescription", "Kinetren video editor"
      VALUE "FileVersion", "{version}"
      VALUE "ProductName", "Kinetren"
      VALUE "ProductVersion", "{version}"
      VALUE "OriginalFilename", "kinetren.exe"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("kinetren.rc");
    std::fs::write(&out, rc).expect("write kinetren.rc");
    embed_resource::compile(&out, embed_resource::NONE).manifest_optional().expect("compile kinetren.rc");
}

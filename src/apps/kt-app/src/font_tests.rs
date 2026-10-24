//! Test 100 (item 12, round 11): EVERY non-printing or non-ASCII character that the program
//! may show must exist in the fonts it loads.
//!
//! The user twice caught a placeholder box instead of a character (`●` U+25CF in
//! the change indicator, wrong code points of Phosphor). The rule “check the
//! glyphs” does not work without a test — so the check is automatic: the scanner
//! parses the sources of kt-app, extracts the contents of string and char
//! literals (including the `\u{...}` escapes in which the icons are written),
//! and checks every non-ASCII character against the cmap of ALL fonts of the
//! Proportional family (Ubuntu-Light, NotoEmoji, emoji-icon-font) plus the
//! vendored ones — exactly the set that `install_fonts` installs.
//!
//! Since R.19 the translation catalogs `i18n/*.toml` are scanned AS WELL: the interface
//! text moved there, and without this t100 would have stopped checking anything meaningful
//! (the sources would have been left with ASCII keys only).

use std::collections::BTreeMap;

/// All fonts the program actually loads: the egui default plus the vendored ones.
/// The list of vendored fonts is taken from `icons::VENDORED`, not duplicated
/// here: a mismatch between the test and `install_fonts` is exactly the hole this
/// test was written for.
fn loaded_fonts() -> Vec<(String, Vec<u8>)> {
    let defs = eframe::egui::FontDefinitions::default();
    let family = defs
        .families
        .get(&eframe::egui::FontFamily::Proportional)
        .expect("no Proportional family")
        .clone();
    let mut out = Vec::new();
    for name in family {
        let data = defs.font_data.get(&name).expect("the family font is not loaded");
        out.push((name, data.font.to_vec()));
    }
    for (name, bytes) in crate::icons::VENDORED {
        out.push((name.to_string(), bytes.to_vec()));
    }
    out
}

/// Translation catalogs: the values themselves, without keys and comments.
fn catalog_values() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("i18n");
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).expect("no i18n/") {
        let p = e.expect("bad directory entry").path();
        if p.extension().map(|x| x == "toml").unwrap_or(false) {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let src = std::fs::read_to_string(&p).expect("unreadable catalog");
            let entries =
                crate::i18n::catalog::parse(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
            for entry in entries {
                out.push((format!("{name}:{}", entry.line), entry.value));
            }
        }
    }
    out.sort();
    out
}

/// Whether the glyph of the character exists in at least one font of the family.
fn covered(fonts: &[(String, ab_glyph::FontVec)], ch: char) -> bool {
    use ab_glyph::Font as _;
    fonts.iter().any(|(_, f)| f.glyph_id(ch).0 != 0)
}

/// Characters that the program does NOT draw, although they sit in literals:
/// whitespace and control ones (`\n`, NBSP, etc.) do not require glyphs.
fn needs_glyph(ch: char) -> bool {
    !ch.is_ascii() && !ch.is_whitespace() && !ch.is_control()
}

/// Parsing of the source: the contents of string/char literals, outside
/// comments. `\u{...}` is expanded — the icons are written exactly that way.
pub fn literal_chars(src: &str) -> Vec<(usize, char)> {
    let b: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;
    // the line counter is kept as i advances
    let bump = |c: char, line: &mut usize| {
        if c == '\n' {
            *line += 1;
        }
    };
    while i < b.len() {
        let c = b[i];
        // comments
        if c == '/' && i + 1 < b.len() && b[i + 1] == '/' {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            let mut depth = 1;
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == '/' && i + 1 < b.len() && b[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && i + 1 < b.len() && b[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    bump(b[i], &mut line);
                    i += 1;
                }
            }
            continue;
        }
        // raw string r"..." / r#"..."#
        if c == 'r' && i + 1 < b.len() && (b[i + 1] == '"' || b[i + 1] == '#') {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < b.len() && b[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < b.len() && b[j] == '"' {
                j += 1;
                loop {
                    if j >= b.len() {
                        break;
                    }
                    if b[j] == '"' {
                        let mut k = j + 1;
                        let mut h = 0;
                        while k < b.len() && b[k] == '#' && h < hashes {
                            h += 1;
                            k += 1;
                        }
                        if h == hashes {
                            j = k;
                            break;
                        }
                    }
                    if needs_glyph(b[j]) {
                        out.push((line, b[j]));
                    }
                    bump(b[j], &mut line);
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        // ordinary string
        if c == '"' {
            i += 1;
            while i < b.len() && b[i] != '"' {
                if b[i] == '\\' {
                    i += 1;
                    if i >= b.len() {
                        break;
                    }
                    if b[i] == 'u' && i + 1 < b.len() && b[i + 1] == '{' {
                        let mut hex = String::new();
                        i += 2;
                        while i < b.len() && b[i] != '}' {
                            hex.push(b[i]);
                            i += 1;
                        }
                        if let Some(ch) = u32::from_str_radix(&hex, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .filter(|c| needs_glyph(*c))
                        {
                            out.push((line, ch));
                        }
                    }
                    i += 1;
                    continue;
                }
                if needs_glyph(b[i]) {
                    out.push((line, b[i]));
                }
                bump(b[i], &mut line);
                i += 1;
            }
            i += 1;
            continue;
        }
        // char literal against lifetime: 'x' / '\n' / 'a
        if c == '\'' {
            let is_char = (i + 2 < b.len() && b[i + 1] != '\\' && b[i + 2] == '\'')
                || (i + 1 < b.len() && b[i + 1] == '\\');
            if is_char {
                let mut j = i + 1;
                if b[j] == '\\' {
                    j += 1;
                    if j < b.len() && b[j] == 'u' && j + 1 < b.len() && b[j + 1] == '{' {
                        let mut hex = String::new();
                        j += 2;
                        while j < b.len() && b[j] != '}' {
                            hex.push(b[j]);
                            j += 1;
                        }
                        if let Some(ch) = u32::from_str_radix(&hex, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .filter(|c| needs_glyph(*c))
                        {
                            out.push((line, ch));
                        }
                    }
                } else if needs_glyph(b[j]) {
                    out.push((line, b[j]));
                }
                while j < b.len() && b[j] != '\'' {
                    j += 1;
                }
                i = j + 1;
                continue;
            }
        }
        bump(c, &mut line);
        i += 1;
    }
    out
}

/// UI sources without test code: the text of asserts and the panic message are not seen by
/// the user, and arrows/symbols there are legitimate. A test module in a file starts with a
/// line `#[cfg(test)]` at zero indentation (attributes of fields are indented).
///
/// The walk is RECURSIVE: since R.19 part of the code lives in `src/i18n/`,
/// and a flat `read_dir` would silently drop it from t100 and t131.
pub fn sources() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out.sort();
    out
}

fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    for e in std::fs::read_dir(dir).expect("no src/") {
        let p = e.expect("bad directory entry").path();
        if p.is_dir() {
            walk(root, &p, out);
            continue;
        }
        if p.extension().map(|x| x == "rs").unwrap_or(false) {
            let name = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            if name.ends_with("_tests.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&p).expect("unreadable source");
            let ui_part = match src.find("\n#[cfg(test)]") {
                Some(at) => src[..at].to_string(),
                None => src,
            };
            out.push((name, ui_part));
        }
    }
}

/// t100: not a single character without a glyph in all UI strings.
#[test]
fn t100_ui_chars_have_glyphs() {
    let fonts: Vec<(String, ab_glyph::FontVec)> = loaded_fonts()
        .into_iter()
        .map(|(n, d)| (n, ab_glyph::FontVec::try_from_vec(d).expect("the font does not parse")))
        .collect();

    // characters that MUST be / MUST NOT be present — insurance for the test itself:
    // if the check breaks and starts letting everything through, these two fail.
    for (ch, what) in [
        ('я', "Cyrillic"),
        ('ñ', "Latin with diacritics"),
        ('日', "an ideograph from the CJK subset"),
        ('ب', "an Arabic letter from the subset"),
    ] {
        assert!(covered(&fonts, ch), "{what} must be covered");
    }
    // Insurance against degeneration: if the check breaks and starts skipping
    // everything, this one fails. U+25CF no longer fits here — it is present in both Hack and
    // Noto CJK; take a Thai letter that is in none of the sets and that
    // structurally cannot end up in the subset (the cmap is cut by catalogs).
    assert!(
        !covered(&fonts, char::from_u32(0x0E01).unwrap()),
        "U+0E01 is still missing — if it appeared, the self-check must change"
    );

    let mut missing: BTreeMap<char, Vec<String>> = BTreeMap::new();
    for (file, src) in sources() {
        for (line, ch) in literal_chars(&src) {
            if !covered(&fonts, ch) {
                missing.entry(ch).or_default().push(format!("{file}:{line}"));
            }
        }
    }
    for (where_, value) in catalog_values() {
        for ch in value.chars().filter(|c| needs_glyph(*c)) {
            if !covered(&fonts, ch) {
                missing.entry(ch).or_default().push(where_.clone());
            }
        }
    }
    assert!(
        missing.is_empty(),
        "characters without a glyph in the program fonts (the user will see a box): {}",
        missing
            .iter()
            .map(|(ch, where_)| format!("U+{:04X} {:?} — {}", *ch as u32, ch, where_.join(", ")))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

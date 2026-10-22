//! Order of Arabic text for egui 0.36.
//!
//! WHY THIS IS NEEDED AT ALL, and why `unicode_bidi::reorder_line` cannot be used:
//!
//! * `epaint::text::text_layout::segment_into_runs` cuts the string into runs BY
//!   FONT GLYPH SET, with no stickiness for neutrals: a space resolves in
//!   Ubuntu-Light, an Arabic letter in the Arabic fallback, so every space
//!   breaks an Arabic phrase into separate runs.
//! * `shape_text` calls `buffer.guess_segment_properties()`, and for an Arabic
//!   run harfrust itself sets direction=RTL: it already reverses the glyphs
//!   inside the run, and the cursive joining is correct.
//! * The runs are nevertheless laid out left to right in LOGICAL order.
//!
//! Result: a single Arabic word is drawn correctly even today, but a phrase has
//! correct letters and the wrong word order. Reversing by CHARACTERS would make
//! it worse: harfrust would reverse each run a second time, and the letter forms
//! would be computed on the reversed sequence.
//!
//! So here the GLYPH ATOMS are reordered, while the characters inside an atom
//! stay in logical order. An atom is a maximal piece of the Arabic block
//! (it will be drawn by the Arabic font) or a single other character. The predicate is exact,
//! not heuristic: in the loaded set the Arabic codepoints are present in exactly one
//! font (`icons.rs::install_fonts`; the order of families must not be changed).
//!
//! Pinned by Test 126, including a check of the failure with `reorder_line`.

use unicode_bidi::{BidiInfo, Level};

/// A codepoint is drawn with the Arabic typeface.
pub fn is_arabic(c: char) -> bool {
    matches!(c as u32,
        0x0600..=0x06FF   // Arabic
        | 0x0750..=0x077F // Arabic Supplement
        | 0x08A0..=0x08FF // Arabic Extended-A
        | 0xFB50..=0xFDFF // Arabic Presentation Forms-A
        | 0xFE70..=0xFEFF // Arabic Presentation Forms-B
    )
}

/// Whether the string contains anything at all that must be reordered. A cheap scan:
/// for LTR languages it is the only cost of the whole mechanism.
///
/// Deliberately limited to Arabic: we do not ship Hebrew, and touching a Hebrew
/// file name (it would pass as “other characters”, one per atom, that is,
/// backwards) is worse than leaving it as it is.
pub fn has_rtl(s: &str) -> bool {
    s.chars().any(is_arabic)
}

/// Mirrored pairs for characters at the RTL level (UAX #9, Bidi_Mirrored).
/// The list is deliberately short: exactly those brackets and quotes that
/// can occur in the catalog.
fn mirror(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '\u{00AB}' => '\u{00BB}', // “ ”
        '\u{00BB}' => '\u{00AB}',
        '\u{2039}' => '\u{203A}', // ‹ ›
        '\u{203A}' => '\u{2039}',
        other => other,
    }
}

/// Logical order → visual order. The result is handed to egui as is.
pub fn visual(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    visual_into(s, &mut out);
    out
}

/// The same, but into a foreign buffer: after warm-up it does not allocate at all (see
/// `Memo` — a stub in case the profile someday shows its cost).
pub fn visual_into(s: &str, out: &mut String) {
    out.clear();
    if !has_rtl(s) {
        out.push_str(s);
        return;
    }
    // `\n` and `\t` are hard separators: a line break must not drift
    // inside the phrase, and a tab separates a menu item from its shortcut.
    let mut rest = s;
    loop {
        match rest.find(['\n', '\t']) {
            Some(i) => {
                reorder_segment(&rest[..i], out);
                out.push_str(&rest[i..i + 1]);
                rest = &rest[i + 1..];
            }
            None => {
                reorder_segment(rest, out);
                return;
            }
        }
    }
}

/// One paragraph without breaks: runs in visual order, RTL runs — by
/// atoms.
fn reorder_segment(s: &str, out: &mut String) {
    if s.is_empty() {
        return;
    }
    let info = BidiInfo::new(s, Some(Level::rtl()));
    let Some(para) = info.paragraphs.first() else {
        out.push_str(s);
        return;
    };
    let (levels, runs) = info.visual_runs(para, para.range.clone());
    for run in runs {
        let text = &info.text[run.clone()];
        if levels[run.start].is_rtl() {
            push_rtl_run(text, out);
        } else {
            out.push_str(text);
        }
    }
}

/// The atoms of a run in reverse order; inside an Arabic atom the characters stay
/// logical — harfrust will reverse and join them.
fn push_rtl_run(text: &str, out: &mut String) {
    let mut atoms: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let mut in_arabic = false;
    for (i, c) in text.char_indices() {
        let arabic = is_arabic(c);
        if i == 0 {
            in_arabic = arabic;
            continue;
        }
        if arabic != in_arabic || !arabic {
            atoms.push(&text[start..i]);
            start = i;
            in_arabic = arabic;
        }
    }
    if start < text.len() {
        atoms.push(&text[start..]);
    }
    for atom in atoms.iter().rev() {
        if atom.chars().next().is_some_and(is_arabic) {
            out.push_str(atom);
        } else {
            out.extend(atom.chars().map(mirror));
        }
    }
}

/// A single-slot memo for strings assembled on every frame. Not needed yet
/// (there are about ten such places and they already call `format!`), but it lives here
/// so that the fix is a substitution, not a rewrite.
#[derive(Default)]
#[allow(dead_code)]
pub struct Memo {
    src: String,
    vis: String,
}

#[allow(dead_code)]
impl Memo {
    pub fn get(&mut self, s: &str) -> &str {
        if self.src != s {
            self.src.clear();
            self.src.push_str(s);
            visual_into(s, &mut self.vis);
        }
        &self.vis
    }
}

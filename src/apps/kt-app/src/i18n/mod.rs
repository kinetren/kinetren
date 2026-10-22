//! Multilingual interface (R.19): en / ru / es / zh-Hans / ar.
//!
//! The text lives in `apps/kt-app/i18n/*.toml`; `build.rs` checks the catalogs and
//! generates static tables from them, so `t()` returns `&'static str`
//! without allocations and without locks — the invariant “no allocations in draw paths”
//! is not violated.
//!
//! English is the source language of the codebase: any new string literal
//! is written in English; the Russian UI text lives only in `ru.toml`
//! (rule R.19, pinned by test 131).

use std::cell::Cell;
use std::fmt;

pub mod bidi;
#[allow(dead_code)] // the runtime user — build.rs via #[path]; here the tests call it
pub mod catalog;
pub mod detect;
mod keys;
pub(crate) mod tables;

// `N_KEYS` is needed only by the catalog integrity tests (125)
#[allow(unused_imports)]
pub use keys::{N_KEYS, k};

/// Index of the string in the tables. Not an enum: there are hundreds of variants, while the call site reads
/// as a path of the catalog section (`k::menu::file::SAVE`), and a typo is a build error.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key(pub u16);

/// The order of the variants = the order of the tables in `tables.rs`; change only together with
/// `LANG_FILES` in `build.rs`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    En,
    Ru,
    Es,
    Zh,
    Ar,
}

impl Lang {
    pub const ALL: [Lang; 5] = [Lang::En, Lang::Ru, Lang::Es, Lang::Zh, Lang::Ar];

    /// Code for storage in eframe Storage and for `KT_LANG`.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
            Lang::Es => "es",
            Lang::Zh => "zh-Hans",
            Lang::Ar => "ar",
        }
    }

    pub fn from_code(s: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code() == s)
    }

    /// Self-name: in the language list each item is written in its own language,
    /// otherwise someone who picked Chinese would have nothing to find in a Russian list.
    pub fn endonym(self) -> &'static str {
        tables::ENDONYM[self as usize]
    }

    pub fn is_rtl(self) -> bool {
        matches!(self, Lang::Ar)
    }
}

// A thread, not a global atomic: `cargo test` runs tests in parallel, and
// the five-language harness tests of the layout would otherwise interfere with each other. The UI is single-threaded
// (eframe); background threads do not call `t()` and would get English.
thread_local! {
    static LANG: Cell<Lang> = const { Cell::new(Lang::En) };
}

pub fn set_lang(l: Lang) {
    LANG.with(|c| c.set(l));
}

/// Switching from the UI: besides the language itself, it fixes the style and requests a redraw.
///
/// `selectable_labels` is turned off for Arabic on purpose: the labels lie in the
/// VISUAL order (see `bidi.rs`), and text copied from them would be
/// garbage. Input fields are not affected by this — their content is not reordered.
pub fn set_lang_ctx(ctx: &eframe::egui::Context, l: Lang) {
    set_lang(l);
    ctx.all_styles_mut(|st| st.interaction.selectable_labels = !l.is_rtl());
    ctx.request_repaint();
}

/// The user's choice (`None` = “auto”) → the actual language.
pub fn resolve(choice: Option<Lang>) -> Lang {
    choice.or_else(detect::from_env).unwrap_or(Lang::En)
}

/// How the choice is stored in eframe Storage and in `KT_LANG`.
pub fn choice_code(choice: Option<Lang>) -> &'static str {
    match choice {
        Some(l) => l.code(),
        None => "auto",
    }
}

/// Conversely: everything unrecognized is “auto”, not an error (Storage survives
/// a change of the language set).
pub fn choice_from_code(s: &str) -> Option<Lang> {
    Lang::from_code(s)
}

pub fn current() -> Lang {
    LANG.with(|c| c.get())
}

/// A string in the current language. The name is short on purpose: this call stands at
/// every label. `t` is taken by time ticks in timeline.rs.
#[inline]
pub fn tr(k: Key) -> &'static str {
    tables::TABLES[current() as usize][k.0 as usize]
}

/// The English variant — needed by FX search, so that “blur” finds the effect in
/// any locale.
#[inline]
pub fn tr_en(k: Key) -> &'static str {
    tables::TABLES[Lang::En as usize][k.0 as usize]
}

/// Arabic BEFORE reordering: only the logical order can be compared with user input.
/// Used by test 126.
#[allow(dead_code)]
#[inline]
pub fn ar_logical(k: Key) -> &'static str {
    tables::AR_LOGICAL[k.0 as usize]
}

/// Key name (`"menu.file.save"`) — for diagnostics and tests.
#[allow(dead_code)]
pub fn key_name(k: Key) -> &'static str {
    tables::KEY_NAMES[k.0 as usize]
}

/// Substitution of positional arguments `{0}`, `{1}`, … Positional, not `{}`:
/// Chinese and Arabic regularly need a different order of arguments.
/// `{{` and `}}` are literal braces.
pub fn interp(tmpl: &str, args: &[&dyn fmt::Display]) -> String {
    use fmt::Write as _;
    let mut out = String::with_capacity(tmpl.len() + 16 * args.len());
    let mut it = tmpl.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                out.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                out.push('}');
            }
            '{' => {
                let mut idx = 0usize;
                let mut any = false;
                while let Some(d) = it.peek().and_then(|c| c.to_digit(10)) {
                    idx = idx * 10 + d as usize;
                    any = true;
                    it.next();
                }
                if it.peek() == Some(&'}') {
                    it.next();
                }
                if any && let Some(a) = args.get(idx) {
                    let _ = write!(out, "{a}");
                }
            }
            _ => out.push(c),
        }
    }
    // Level 2 of RTL order: static strings are reordered by the build, and
    // those assembled from arguments — here. For the other languages this is one scan.
    if current().is_rtl() { bidi::visual(&out) } else { out }
}

/// `tf!(k::msg::CLIPS_COPIED, n)` — a catalog string with arguments.
#[macro_export]
macro_rules! tf {
    ($k:expr $(, $a:expr)* $(,)?) => {
        $crate::i18n::interp($crate::i18n::tr($k), &[$(&$a as &dyn ::std::fmt::Display),*])
    };
}

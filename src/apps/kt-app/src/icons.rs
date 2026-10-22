//! Phosphor icons (vendored TTF in assets/, PUA codepoints).
//! The default egui fonts have no glyphs for 🗐🚩🎬💾⏮ and the like — the buttons
//! were drawn as squares (item 23). The font is installed in install_fonts().

use eframe::egui;

pub const PLAY: &str = "\u{E3D0}";
pub const PAUSE: &str = "\u{E39E}";
pub const SKIP_BACK: &str = "\u{E5A4}";
pub const FLAG: &str = "\u{E244}";
pub const SCISSORS: &str = "\u{EAE0}";
pub const COPY: &str = "\u{E1CA}";
pub const CLIPBOARD: &str = "\u{E198}";
pub const TRASH: &str = "\u{E4A6}";
pub const TRIM_START: &str = "\u{E062}"; // arrow-line-left
pub const TRIM_END: &str = "\u{E064}"; // arrow-line-right
pub const LANE_UP: &str = "\u{E522}"; // arrow-fat-line-up
pub const LANE_DOWN: &str = "\u{E51C}"; // arrow-fat-line-down
pub const MINUS: &str = "\u{E32A}";
pub const PLUS: &str = "\u{E3D4}";
/// “Key BEFORE cursor” / “key AFTER cursor” (R.40 item 18): place a key on the
/// neighboring frame, pinning the previous or next segment of the curve.
pub const KEY_BEFORE: &str = "\u{E062}"; // arrow-line-left
pub const KEY_AFTER: &str = "\u{E064}"; // arrow-line-right
pub const REGION_IN: &str = "\u{E85E}"; // brackets-square
pub const REGION_OUT: &str = "\u{E862}"; // brackets-angle
pub const CLEAR: &str = "\u{E4F6}"; // x
pub const LOOP: &str = "\u{E3F6}"; // repeat
pub const SELECTION: &str = "\u{E69A}"; // selection (event selection outline)
pub const AREA_SAVE: &str = "\u{E69C}"; // selection-plus (keep the region, R.35)
pub const UNDO: &str = "\u{E08A}"; // arrow-u-up-left
pub const REDO: &str = "\u{E08C}"; // arrow-u-up-right
pub const RENDER: &str = "\u{E8C2}"; // film-slate
pub const SAVE: &str = "\u{E248}"; // floppy-disk
pub const FX: &str = "\u{E2DE}"; // lightning (clip with effects)
pub const WARN: &str = "\u{E4E0}"; // warning (E4DE was a voicemail — item 1 of round 8)
pub const SEARCH: &str = "\u{E30C}"; // magnifying-glass (item 3 of round 10)
pub const GAUGE: &str = "\u{E000}"; // pulse (the gauge E628 was read as a Reddit robot — item 5 of round 10)
pub const SQUARES_FOUR: &str = "\u{E464}"; // grid (bin mode)
pub const LIST: &str = "\u{E2F0}"; // list; it is also the “hamburger” of the narrow toolbar
pub const CHECK: &str = "\u{E182}"; // check mark in menu items (≡/✓ from Unicode — tofu, test t100)
#[allow(dead_code)] // used by the bin groups (item 13 of round 11)
pub const LOCK: &str = "\u{E2FA}"; // lock (bin groups)
pub const ARROWS_OUT: &str = "\u{E0A2}"; // free layout
pub const SKIP_FWD: &str = "\u{E5A6}"; // to the next point
/// Pin of a public parameter of a composition (R.59, stage 3): stands before the
/// label of a property declared outward, and on an instance event that
/// has overrides.
pub const PIN: &str = "\u{E3E2}"; // push-pin

// Glyphs of the CATEGORIES of the effect picker (R.54): an effect row starts with
// the icon of its category — it is read faster than a caption heading, and
// does not take a separate band (rule 13).
pub const CAT_MOTION: &str = "\u{E0A4}"; // arrows-out-cardinal
pub const CAT_GEOMETRY: &str = "\u{ED6A}"; // flip-horizontal
pub const CAT_BLUR: &str = "\u{E566}"; // drop-half
pub const CAT_KEY: &str = "\u{E568}"; // eyedropper
pub const CAT_STYLE: &str = "\u{E6B6}"; // magic-wand
pub const CAT_GLOW: &str = "\u{E472}"; // sun
pub const CAT_THREE_D: &str = "\u{E1DA}"; // cube
pub const CAT_VIDEO: &str = "\u{E792}"; // film-strip
pub const CAT_COLOR: &str = "\u{E6C8}"; // palette
pub const CAT_DISTORT: &str = "\u{EA9A}"; // wave-sine
pub const CAT_VOLUME: &str = "\u{E44A}"; // speaker-high
pub const CAT_FILTER: &str = "\u{E192}"; // circles-three
pub const CAT_STEREO: &str = "\u{E69A}"; // selection
pub const CAT_MODULATION: &str = "\u{E6A2}"; // sparkle
pub const CAT_DELAY: &str = "\u{E1EC}"; // diamond
pub const CAT_DYNAMICS: &str = "\u{E000}"; // pulse
pub const CAT_TIMESTRETCH: &str = "\u{E1A0}"; // clock-counter-clockwise

/// Vendored fonts: the name in egui and the bytes. The order here is the fallback order,
/// and it must NOT be changed.
///
/// The subsets come AFTER the default families and after Phosphor: Latin and Cyrillic
/// must stay with Ubuntu-Light (Noto Sans CJK contains them too), and the
/// PUA icons with Phosphor. The precision of the predicate
/// “Arabic glyph atom” in `i18n::bidi` rests on this same order — there it is derived from the fact that
/// the Arabic codepoints are present in exactly one loaded font.
///
/// The subsets are built by `scripts/subset-fonts.py` from the characters of `i18n/*.toml`
/// (R.19): the full Noto Sans CJK weighs ~19 MB, trimmed — 75 KB.
pub const VENDORED: [(&str, &[u8]); 3] = [
    ("phosphor", include_bytes!("../assets/Phosphor.ttf")),
    ("noto_sc", include_bytes!("../assets/NotoSansSC-kt.otf")),
    ("noto_ar", include_bytes!("../assets/NotoSansArabic-kt.ttf")),
];

/// Attach the vendored fonts to egui (as a fallback to both families).
///
/// All languages are loaded ALWAYS, not just the current one: 136 KB for both subsets is cheaper
/// than `set_fonts` on the language-switch path and a frame with squares after it.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in VENDORED {
        fonts
            .font_data
            .insert(name.into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
        // both families: bin_panel and the toolbar also build monospace FontIds
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            if let Some(list) = fonts.families.get_mut(&family) {
                list.push(name.into());
            }
        }
    }
    ctx.set_fonts(fonts);
}
#[allow(dead_code)] // slot reordering now by mouse (R.33)
pub const CARET_LEFT: &str = "\u{E138}";
pub const CARET_RIGHT: &str = "\u{E13A}";
#[allow(dead_code)] // slots are switched off by a check mark (R.33)
pub const POWER: &str = "\u{E3DA}";
pub const BELL: &str = "\u{E0CE}";

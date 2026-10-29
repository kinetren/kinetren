//! Tests of the multilingual system (R.19): 125 — catalog integrity,
//! 126 — order of Arabic text.

use crate::i18n::{self, Lang, catalog, tables};

/// Sections that are consumed not through `k::…` but otherwise: self-names end up
/// in the generated `ENDONYM` and are read through `Lang::endonym()`.
// `desktop.` — the packaging texts (i18n-gaps, D1): read by the .desktop/MIME files, not the UI;
// t930 is their user
const NOT_REFERENCED_BY_PATH: [&str; 2] = ["lang.", "desktop."];

/// The full key `menu.file.save` → the tail of the path of the constant `menu::file::SAVE`,
/// by which the key is looked up in the sources.
fn const_path(key: &str) -> String {
    let mut parts: Vec<String> = key.split('.').map(str::to_string).collect();
    if let Some(last) = parts.last_mut() {
        *last = last.to_uppercase();
    }
    parts.join("::")
}

/// t125 (R.19): the translation catalogs are complete and consistent.
///
/// The GENERATED result is checked, not the files themselves: a missing key fails
/// the build in `build.rs`, and without this test a regression of `build.rs` itself would pass
/// unnoticed. Plus the reverse side — a key that nobody calls any more:
/// without such a check the catalog rots and the translator works in vain.
#[test]
fn t125_catalogs_are_complete_and_used() {
    assert_eq!(tables::KEY_NAMES.len(), i18n::N_KEYS);
    assert_eq!(tables::AR_LOGICAL.len(), i18n::N_KEYS);
    assert_eq!(tables::TABLES.len(), Lang::ALL.len());

    for (li, lang) in Lang::ALL.into_iter().enumerate() {
        let table = tables::TABLES[li];
        assert_eq!(table.len(), i18n::N_KEYS, "{}: wrong table length", lang.code());
        for (i, v) in table.iter().enumerate() {
            assert!(!v.is_empty(), "{}: key {:?} is empty", lang.code(), tables::KEY_NAMES[i]);
            assert_eq!(
                catalog::placeholders(v),
                catalog::placeholders(tables::TABLES[Lang::En as usize][i]),
                "{}: key {:?} has a different placeholder set than English",
                lang.code(),
                tables::KEY_NAMES[i]
            );
        }
        assert!(!lang.endonym().is_empty(), "{}: endonym is empty", lang.code());
    }

    // self-names are the same in all catalogs — otherwise someone who picked Chinese
    // would have nothing to find in the Russian list
    for li in 1..Lang::ALL.len() {
        for (i, name) in tables::KEY_NAMES.iter().enumerate() {
            if name.starts_with("lang.") {
                assert_eq!(
                    tables::TABLES[li][i],
                    tables::TABLES[0][i],
                    "endonym {name:?} differs between catalogs"
                );
            }
        }
    }

    let sources = crate::font_tests::sources();
    let mut unused: Vec<&str> = Vec::new();
    for name in tables::KEY_NAMES.iter() {
        if NOT_REFERENCED_BY_PATH.iter().any(|p| name.starts_with(p)) {
            continue;
        }
        let path = const_path(name);
        if !sources.iter().any(|(_, src)| src.contains(&path)) {
            unused.push(name);
        }
    }
    assert!(
        unused.is_empty(),
        "catalog keys nobody uses (delete them or wire them up): {unused:?}"
    );
}

/// t126 (R.19), part “a”: reordering is by WORDS, not by letters.
///
/// egui 0.36 cuts the string into runs by font glyph set and hands each one to
/// harfrust with `guess_segment_properties()` — it reverses the glyphs itself
/// inside the run. Therefore the characters inside an Arabic atom must stay in
/// LOGICAL order, and it is the atoms that must swap places. Analysis — in `bidi.rs`.
#[test]
fn t126_bidi_reorders_words_not_letters() {
    use crate::i18n::bidi;

    // without Arabic the string is not touched at all
    for s in ["", "Save", "1280x720", "00:01:30", "Ctrl+S"] {
        assert_eq!(bidi::visual(s), s, "LTR text must pass through untouched");
    }

    let one = "\u{0645}\u{0644}\u{0641}"; // “file”
    let two = "\u{062D}\u{0641}\u{0638}"; // “save”

    // one word: the order of characters inside the word is NOT changed
    assert_eq!(bidi::visual(one), one, "letters inside a word must stay logical");

    // two words are swapped, the letters inside each are not
    let phrase = format!("{two} {one}");
    assert_eq!(bidi::visual(&phrase), format!("{one} {two}"), "words must swap, letters must not");

    // Latin/digits inside an Arabic phrase move to their visual position
    let mixed = format!("{one}: 42");
    let vis = bidi::visual(&mixed);
    assert!(
        vis.starts_with("42"),
        "an LTR number must move to the left of an RTL phrase, got {vis:?}"
    );
    assert!(vis.contains(one), "the word itself must survive: {vis:?}");

    // brackets at the RTL level are mirrored
    let paren = format!("({one})");
    let vis = bidi::visual(&paren);
    assert!(
        vis.starts_with('(') && vis.ends_with(')'),
        "mirrored brackets must still read as a pair, got {vis:?}"
    );

    // tab and line break are hard separators, they do not drift inside the phrase
    let tabbed = format!("{one}\tCtrl+S");
    assert_eq!(
        bidi::visual(&tabbed),
        format!("{one}\tCtrl+S"),
        "a tab separates the label from its accelerator and must not move"
    );
}

/// t126, part “b”: sections compared with user input lie in the
/// Arabic table in LOGICAL order.
#[test]
fn t126_search_sections_are_not_reordered() {
    for (i, name) in tables::KEY_NAMES.iter().enumerate() {
        if name.starts_with("fxreg.kw.") || name.starts_with("lang.") {
            assert_eq!(
                tables::TABLES[Lang::Ar as usize][i],
                tables::AR_LOGICAL[i],
                "{name:?} must stay logical: it is matched against what the user types"
            );
        }
    }
}

/// Parsing of a catalog: the format is strict on purpose — a silently swallowed line is worse
/// than a failed build.
#[test]
fn t125_catalog_parser_is_strict() {
    let ok = catalog::parse("[a.b]\nkey = \"value\" # note\n").expect("valid catalog");
    assert_eq!(ok.len(), 1);
    assert_eq!(ok[0].key, "a.b.key");
    assert_eq!(ok[0].value, "value");

    let esc = catalog::parse(r#"k = "a\tb\nc\"d\\e\u00ab""#).expect("escapes");
    assert_eq!(esc[0].value, "a\tb\nc\"d\\e\u{00ab}");

    for bad in [
        "key = value",          // without quotes
        "Key = \"v\"",          // key not in lower case
        "[a b]\nk = \"v\"",     // space in a section
        "k = \"unterminated",   // string not closed
        "k = \"v\" junk",       // garbage after a value
        "k = \"v\"\nk = \"w\"", // duplicate
        "k = \"\\q\"",          // unknown escape
    ] {
        assert!(catalog::parse(bad).is_err(), "must be rejected: {bad:?}");
    }

    assert_eq!(catalog::placeholders("{0} and {2} and {0}"), vec![0, 2]);
    assert_eq!(catalog::placeholders("{{0}} literal"), Vec::<usize>::new());
}

/// t130 (R.19): the language on first launch is taken from the OS locale.
#[test]
fn t130_locale_detection() {
    use crate::i18n::detect::from_locale;
    assert_eq!(from_locale("zh_CN.UTF-8"), Some(Lang::Zh));
    assert_eq!(from_locale("zh_TW"), Some(Lang::Zh));
    assert_eq!(from_locale("ar_EG.UTF-8"), Some(Lang::Ar));
    assert_eq!(from_locale("es_MX"), Some(Lang::Es));
    assert_eq!(from_locale("ru_RU.UTF-8"), Some(Lang::Ru));
    assert_eq!(from_locale("en-GB@euro"), Some(Lang::En));
    assert_eq!(from_locale("pt_BR"), None);
    assert_eq!(from_locale("C"), None);
    assert_eq!(from_locale("POSIX"), None);
    assert_eq!(from_locale(""), None);

    for l in Lang::ALL {
        assert_eq!(Lang::from_code(l.code()), Some(l));
    }
}

/// t131 (R.19): the source language of the codebase is English.
///
/// The owner demanded: “if anything remains in Russian, it must be
/// translated into English, and from then on all agents by default write all strings in
/// English”. Without a test this is a memo, not a rule: the next agent will write
/// `note("not opened")`, it will pass review and go past the catalog — that is, it will never
/// be translated.
///
/// The Russian UI text lives ONLY in `i18n/ru.toml`. Comments and
/// `docs/*.md` remain in Russian — this is the repository convention; the rule is about
/// string LITERALS. Test code is excluded by the same logic as in t100:
/// the user does not see the text of the assertions.
#[test]
fn t131_source_language_is_english() {
    fn is_cyrillic(c: char) -> bool {
        // Cyrillic + Cyrillic supplement
        matches!(c as u32, 0x0400..=0x052F)
    }

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("src/ workspace root");

    let mut stack = vec![root.to_path_buf()];
    let mut bad: Vec<String> = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("unreadable directory") {
            let p = e.expect("bad directory entry").path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            if p.is_dir() {
                // target/ — build output, vendor/ — third-party FFmpeg headers,
                // tests/ — integration tests (the text of assertions is not visible)
                if !matches!(name.as_str(), "target" | "vendor" | "tests" | "fixtures") {
                    stack.push(p);
                }
                continue;
            }
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&p).expect("unreadable source");
            let ui_part = match src.find("\n#[cfg(test)]") {
                Some(at) => &src[..at],
                None => &src[..],
            };
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            for (line, ch) in crate::font_tests::literal_chars(ui_part) {
                if is_cyrillic(ch) {
                    bad.push(format!("{rel}:{line}"));
                    break;
                }
            }
        }
    }
    bad.sort();
    bad.dedup();
    assert!(
        bad.is_empty(),
        "Cyrillic in string literals — UI text belongs in i18n/ru.toml, \
         everything else is written in English: {bad:?}"
    );
}

fn has_cyrillic(s: &str) -> bool {
    s.chars().any(|c| matches!(c as u32, 0x0400..=0x052F))
}

/// Cyrillic values of a catalog other than `ru.toml`: `(section.key, value)`. Only the endonym
/// `[lang] ru` may be Russian there.
fn cyrillic_in_catalog(text: &str) -> Vec<(String, String)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(s) = l.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = s.to_string();
            continue;
        }
        let Some((key, value)) = l.split_once('=') else { continue };
        let key = key.trim();
        if has_cyrillic(value) && !(section == "lang" && key == "ru") {
            out.push((format!("{section}.{key}"), value.trim().to_string()));
        }
    }
    out
}

/// Committed test data that carries Cyrillic ON PURPOSE, each with its reason (D4). Anything else
/// Cyrillic in the scanned files is a word that crept in.
const CYRILLIC_TEST_DATA: [(&str, &str); 3] = [
    ("libs/kt-engine/tests/title_golden.rs", "the frozen title golden renders a Cyrillic line (R.52)"),
    ("libs/kt-otio/tests/format.rs", "a non-ASCII media path must survive the exchange round trip"),
    ("apps/kt-cli/tests/readme_guard.rs", "the README guard injects a Cyrillic line to prove it is red"),
];

/// The first Cyrillic line of a committed text file, `None` if it has none.
fn cyrillic_line(text: &str) -> Option<usize> {
    text.lines().position(has_cyrillic).map(|i| i + 1)
}

/// Test 933 (i18n-gaps, D4): a Cyrillic word cannot hide outside `ru.toml` and the declared data.
///
/// The owner (kkt 2026-10-27 item 3): "check that ru words did not get anywhere else in other
/// languages". t131 holds the Rust literals; this holds the rest: the four other catalogs value by
/// value (only the endonym «Русский» of `[lang]`), and the committed fixtures, the live scenarios and
/// the workspace's integration tests as text, against the declared list with reasons. The scanners
/// are proven on broken copies here (a Russian word in a Spanish value, in a fixture line), not by
/// writing into the tree.
#[test]
fn t933_cyrillic_stays_in_ru_and_in_declared_data() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("src/ workspace root")
        .to_path_buf();
    // the catalogs
    let i18n = root.join("apps/kt-app/i18n");
    for lang in ["en", "es", "zh-Hans", "ar"] {
        let text = std::fs::read_to_string(i18n.join(format!("{lang}.toml"))).expect("catalog");
        let bad = cyrillic_in_catalog(&text);
        assert!(bad.is_empty(), "{lang}.toml carries Russian: {bad:?}");
    }
    // the committed fixtures, the live scenarios, the integration tests
    let mut files = Vec::new();
    let mut stack = vec![root.join("fixtures"), root.join("scripts/live")];
    let mut tests = vec![root.join("libs"), root.join("apps")];
    while let Some(dir) = tests.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() && p.file_name().is_some_and(|n| n != "target" && n != "src") {
                if p.file_name().is_some_and(|n| n == "tests") {
                    stack.push(p);
                } else {
                    tests.push(p);
                }
            }
        }
    }
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            if p.is_dir() {
                if name != "out" {
                    stack.push(p);
                }
            } else if [".kt", ".otio", ".ktproj", ".json", ".py", ".rs"].iter().any(|x| name.ends_with(x)) {
                files.push(p);
            }
        }
    }
    let mut bad = Vec::new();
    for p in &files {
        let rel = p.strip_prefix(&root).unwrap_or(p).to_string_lossy().replace('\\', "/");
        let Ok(text) = std::fs::read_to_string(p) else { continue };
        if let Some(line) = cyrillic_line(&text)
            && !CYRILLIC_TEST_DATA.iter().any(|(f, _)| *f == rel)
        {
            bad.push(format!("{rel}:{line}"));
        }
    }
    assert!(bad.is_empty(), "Cyrillic outside the declared test data: {bad:?}");
    for (f, why) in CYRILLIC_TEST_DATA {
        // the walk itself reached the declared files: a walk that misses `tests/` would pass empty
        assert!(files.iter().any(|p| p.ends_with(f)), "the scan did not reach {f}");
        let text = std::fs::read_to_string(root.join(f)).unwrap_or_default();
        assert!(cyrillic_line(&text).is_some(), "{f} is declared ({why}) but has no Cyrillic any more");
    }
    // the scanners on broken copies: a Russian word in a Spanish value, in a fixture line
    let es = std::fs::read_to_string(i18n.join("es.toml")).unwrap();
    let broken = es.replacen("nest_default_name = \"Bobina\"", "nest_default_name = \"Лента\"", 1);
    assert_eq!(
        cyrillic_in_catalog(&broken),
        [("msg.nest_default_name".to_string(), "\"Лента\"".to_string())],
        "the catalog scanner missed a Russian value"
    );
    assert_eq!(cyrillic_line("{\n  \"name\": \"Лента\"\n}"), Some(2), "the fixture scanner missed it");
}

/// Test 171 (item 4 of R.28): the menu item says WHAT and WHERE it will happen.
///
/// The complaint verbatim: “fade out — it is not clear at all from the name of the item where it
/// will add this out… And the label “Remove crossfade” is not obvious either:
/// it turns out it removes the fadeout from the end.” One label for four different
/// actions is a defect; the test keeps them DISTINCT in every language, otherwise a
/// translation “into one word” would bring back the same ambiguity.
#[test]
fn t171_transition_menu_labels_say_what_and_where() {
    use crate::i18n::k;
    let groups: [[i18n::Key; 4]; 1] = [[
        k::tl::menu::REMOVE_FADE_START,
        k::tl::menu::REMOVE_FADE_END,
        k::tl::menu::REMOVE_TRANSITION_NEXT,
        k::tl::menu::REMOVE_TRANSITION_PREV,
    ]];
    for lang in Lang::ALL {
        for keys in groups {
            let vals: Vec<&str> =
                keys.iter().map(|k| tables::TABLES[lang as usize][k.0 as usize]).collect();
            for (i, a) in vals.iter().enumerate() {
                for b in vals.iter().skip(i + 1) {
                    assert_ne!(a, b, "{}: the transition removal items are indistinguishable", lang.code());
                }
            }
        }
        // the start and the end of an event are distinguishable also in the ADD items
        assert_ne!(
            tables::TABLES[lang as usize][k::tl::menu::FADE_IN.0 as usize],
            tables::TABLES[lang as usize][k::tl::menu::FADE_OUT.0 as usize],
            "{}: fade in and fade out are indistinguishable",
            lang.code()
        );
        // “Marker here” and “Marker over the region” are different actions
        assert_ne!(
            tables::TABLES[lang as usize][k::tl::menu::MARKER_HERE.0 as usize],
            tables::TABLES[lang as usize][k::tl::menu::MARKER_REGION.0 as usize],
            "{}: a point marker and a range marker are indistinguishable",
            lang.code()
        );
    }
}

/// Test 756c (R.63 item 8.1): ducking is NAMED AFTER THE INDUSTRY.
///
/// The owner: “I do not understand what duck under a track “mute under
/// the track”??? what??? what is under a track that needs to be muted??
/// is this Audio-Ducking? why did you not name it normally? a note to future
/// agents”. The rule from this: a function that has an industry name
/// has THAT name in the interface — otherwise the person cannot link our button to
/// what they know from Vegas/Premiere/Resolve.
#[test]
fn t756c_ducking_is_named_the_way_the_industry_names_it() {
    for lang in Lang::ALL {
        i18n::set_lang(lang);
        let menu = i18n::tr(i18n::k::tl::menu::DUCK).to_lowercase();
        let title = i18n::tr(i18n::k::msg::DUCK_TITLE).to_lowercase();
        for (what, s) in [("menu item", &menu), ("title", &title)] {
            // the former name was a DESCRIPTION OF THE PLACE (“under a track”), not the
            // name of the function — and there was nothing to explain it with
            assert!(
                !s.contains("под дорожкой") && !s.contains("under a track"),
                "{}: {what} still names a place, not a function: {s:?}",
                lang.code()
            );
        }
        // industry name: “ducking” / “ducking” / 闪避 / خفض
        let known = ["дакинг", "ducking", "闪避", "خفض"];
        assert!(
            known.iter().any(|k| menu.contains(k)) && known.iter().any(|k| title.contains(k)),
            "{}: the industry name is neither in the menu item ({menu:?}) nor in the title ({title:?})",
            lang.code()
        );
    }
    i18n::set_lang(Lang::Ru);
}

/// Test 759b (R.64): the labels of the “Transform” form are translated, and the units in
/// them are NOT written.
///
/// The unit comes from the parameter's descriptor (`ParamSpec::unit`) and is drawn by the
/// field; written also into the label, it would split in two — rule 9 of the checklist
/// (“nothing superfluous”) and the rule of one source of truth. In addition, a guard of
/// the usual kind: the new keys have five translations, not five
/// English copies.
#[test]
fn t759b_transform_form_labels_are_translated_and_carry_no_units() {
    let keys = [
        i18n::k::geomctl::GROUP_FRAME,
        i18n::k::geomctl::GROUP_POS,
        i18n::k::geomctl::GROUP_CROP,
        i18n::k::geomctl::RESET,
        i18n::k::geomctl::RESET_HINT,
        i18n::k::geomctl::SCHEMA_HINT,
    ];
    // the labels of the PARAMETERS of the leading node — from the shared `fxp` area
    let params = [
        i18n::k::fxp::POS_X,
        i18n::k::fxp::POS_Y,
        i18n::k::fxp::ANCHOR_X,
        i18n::k::fxp::ANCHOR_Y,
        i18n::k::fxp::CROP_L,
        i18n::k::fxp::CROP_T,
        i18n::k::fxp::CROP_R,
        i18n::k::fxp::CROP_B,
        i18n::k::fxp::ZOOM,
        i18n::k::fxp::ROT,
    ];
    let mut en: Vec<String> = Vec::new();
    i18n::set_lang(Lang::En);
    for k in keys {
        en.push(i18n::tr(k).to_string());
    }
    for lang in Lang::ALL {
        i18n::set_lang(lang);
        for k in keys {
            let s = i18n::tr(k);
            assert!(!s.trim().is_empty(), "{}: an empty form label", lang.code());
        }
        for k in params {
            let s = i18n::tr(k);
            for unit in ['%', '°', '×'] {
                assert!(
                    !s.contains(unit),
                    "{}: the label “{s}” spells out the unit “{unit}” — it comes from the \
                     parameter passport and is drawn by the field, it must not be written twice",
                    lang.code()
                );
            }
        }
        // “Reset” must say WHAT exactly will come back (rule 19).
        // Measuring by absolute length is not allowed — CJK is twice as dense as
        // Latin; we measure RELATIVE to the button itself.
        let hint = i18n::tr(i18n::k::geomctl::RESET_HINT);
        let btn = i18n::tr(i18n::k::geomctl::RESET);
        assert!(
            hint.chars().count() > btn.chars().count() * 3,
            "{}: the reset hint is no longer than its own button — it must \
             list what comes back and what stays: {hint:?}",
            lang.code()
        );
        if !matches!(lang, Lang::En) {
            let now: Vec<String> = keys.iter().map(|k| i18n::tr(*k).to_string()).collect();
            assert!(
                now.iter().zip(&en).any(|(a, b)| a != b),
                "{}: the form labels match the English ones — no translation",
                lang.code()
            );
        }
    }
    i18n::set_lang(Lang::Ru);
}

/// t861 (windows-build, ui-reach): without LC_ALL/LC_MESSAGES/LANG the OS locale name
/// decides (Windows); a set variable still wins; an unknown language falls back to English.
#[test]
fn t861_language_from_the_os_locale_when_no_variable() {
    use crate::i18n::detect::detect;
    let none = |_: &str| None::<String>;
    assert_eq!(detect(&none, Some("ru-RU".into())), Some(Lang::Ru));
    assert_eq!(detect(&none, Some("zh-Hans-CN".into())), Some(Lang::Zh));
    let lang_es = |v: &str| (v == "LANG").then(|| "es_ES.UTF-8".to_string());
    assert_eq!(detect(&lang_es, Some("ru-RU".into())), Some(Lang::Es), "the variable wins");
    assert_eq!(detect(&none, Some("fr-FR".into())), None, "unknown → English by the caller");
    assert_eq!(detect(&none, None), None);
}


/// The `[desktop]` section of a catalog: key → value.
fn desktop_section(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut on = false;
    let mut out = std::collections::BTreeMap::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            on = l == "[desktop]";
            continue;
        }
        if let (true, Some((k, v))) = (on, l.split_once('=')) {
            out.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
        }
    }
    out
}

/// What the packaging files must say for each catalog language (D1): the language's freedesktop
/// locale (`None` — the untranslated base, English) and its `[desktop]` section; returns every
/// mismatch as "file: key[locale]: wanted …".
fn packaging_mismatches(
    langs: &[(Option<&str>, std::collections::BTreeMap<String, String>)],
    desktop: &str,
    mime: &str,
) -> Vec<String> {
    let mut bad = Vec::new();
    let de = |key: &str, loc: Option<&str>| -> Option<String> {
        let name = match loc {
            Some(l) => format!("{key}[{l}]="),
            None => format!("{key}="),
        };
        desktop.lines().find_map(|l| l.strip_prefix(name.as_str())).map(str::to_string)
    };
    for (loc, sec) in langs {
        let v = |k: &str| sec.get(k).cloned().unwrap_or_default();
        let kw = format!("{};", v("keywords").split_whitespace().collect::<Vec<_>>().join(";"));
        for (key, want) in
            [("GenericName", v("generic_name")), ("Comment", v("comment")), ("Keywords", kw)]
        {
            if de(key, *loc).as_deref() != Some(want.as_str()) {
                bad.push(format!("kinetren.desktop: {key}[{}]: wanted “{want}”", loc.unwrap_or("")));
            }
        }
        for (ty, key) in [
            ("application/x-kinetren-project", "mime_project"),
            ("application/x-opentimelineio", "mime_otio"),
            ("application/x-kinotrain-legacy-project", "mime_legacy"),
        ] {
            let want = match loc {
                Some(l) => format!("<comment xml:lang=\"{l}\">{}</comment>", v(key)),
                None => format!("<comment>{}</comment>", v(key)),
            };
            let block = mime
                .split("<mime-type ")
                .find(|b| b.starts_with(&format!("type=\"{ty}\"")))
                .unwrap_or("");
            if !block.contains(&want) {
                bad.push(format!(
                    "kinetren-mime.xml: {ty} comment[{}]: wanted {want}",
                    loc.unwrap_or("")
                ));
            }
        }
    }
    bad
}

/// Test 930 (i18n-gaps, D1): every catalog language is in the desktop and MIME files, equal to the
/// catalog's `[desktop]` section.
///
/// The owner (kkt 2026-10-27 item 3): "the mime, desktop files hold only ru, translate the
/// remaining languages there too". The languages come from the catalogs on disk — a sixth catalog
/// language with no packaging lines is red; so is a removed line. `Keywords=` is the untranslated
/// base plus one line per language, the catalog's space-separated words joined with `;`.
#[test]
fn t930_packaging_speaks_every_catalog_language() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let locale = |lang: &str| -> Option<&'static str> {
        match lang {
            "en" => None,
            "ru" => Some("ru"),
            "es" => Some("es"),
            "zh-Hans" => Some("zh_CN"),
            "ar" => Some("ar"),
            other => panic!("a catalog language with no freedesktop locale mapped: {other}"),
        }
    };
    let mut langs = Vec::new();
    for e in std::fs::read_dir(src.join("apps/kt-app/i18n")).expect("catalogs").flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(lang) = name.strip_suffix(".toml") else { continue };
        let sec = desktop_section(&std::fs::read_to_string(e.path()).unwrap());
        assert_eq!(sec.len(), 6, "{name}: the [desktop] section is incomplete: {sec:?}");
        langs.push((locale(lang), sec));
    }
    assert_eq!(langs.len(), crate::i18n::Lang::ALL.len(), "the catalogs on disk are the languages");
    let desktop = std::fs::read_to_string(src.join("packaging/kinetren.desktop")).unwrap();
    let mime = std::fs::read_to_string(src.join("packaging/kinetren-mime.xml")).unwrap();
    let bad = packaging_mismatches(&langs, &desktop, &mime);
    assert!(bad.is_empty(), "the packaging files differ from the catalogs:\n{}", bad.join("\n"));

    // broken copies: a sixth language with no lines; Comment[es] removed
    let mut six = langs.clone();
    let mut fr = langs[0].1.clone();
    fr.insert("comment".into(), "Éditeur vidéo non linéaire".into());
    six.push((Some("fr"), fr));
    let bad = packaging_mismatches(&six, &desktop, &mime);
    assert!(bad.iter().any(|b| b.contains("Comment[fr]")), "a sixth language went unnoticed: {bad:?}");
    let no_es: String = desktop
        .lines()
        .filter(|l| !l.starts_with("Comment[es]="))
        .map(|l| format!("{l}\n"))
        .collect();
    let bad = packaging_mismatches(&langs, &no_es, &mime);
    assert!(bad.iter().any(|b| b.contains("Comment[es]")), "a removed Comment[es] went unnoticed: {bad:?}");
}

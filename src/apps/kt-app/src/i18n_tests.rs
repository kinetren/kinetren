//! Тесты системы мультиязычности (Р.19): 125 — целостность каталогов,
//! 126 — порядок арабского текста.

use crate::i18n::{self, Lang, catalog, tables};

/// Секции, которые расходуются не через `k::…`, а иначе: самоназвания попадают
/// в сгенерированный `ENDONYM` и читаются через `Lang::endonym()`.
const NOT_REFERENCED_BY_PATH: [&str; 1] = ["lang."];

/// Полный ключ `menu.file.save` → хвост пути константы `menu::file::SAVE`,
/// по которому ключ ищется в исходниках.
fn const_path(key: &str) -> String {
    let mut parts: Vec<String> = key.split('.').map(str::to_string).collect();
    if let Some(last) = parts.last_mut() {
        *last = last.to_uppercase();
    }
    parts.join("::")
}

/// t125 (Р.19): каталоги перевода полны и согласованы.
///
/// Проверяется СГЕНЕРИРОВАННЫЙ результат, а не сами файлы: пропуск ключа валит
/// сборку в `build.rs`, и без этого теста регресс самого `build.rs` прошёл бы
/// незамеченным. Плюс обратная сторона — ключ, который больше никто не зовёт:
/// без такой проверки каталог гниёт и переводчик работает вхолостую.
#[test]
fn t125_catalogs_are_complete_and_used() {
    assert_eq!(tables::KEY_NAMES.len(), i18n::N_KEYS);
    assert_eq!(tables::AR_LOGICAL.len(), i18n::N_KEYS);
    assert_eq!(tables::TABLES.len(), Lang::ALL.len());

    for (li, lang) in Lang::ALL.into_iter().enumerate() {
        let table = tables::TABLES[li];
        assert_eq!(table.len(), i18n::N_KEYS, "{}: wrong table length", lang.code());
        for (i, v) in table.iter().enumerate() {
            assert!(
                !v.is_empty(),
                "{}: key {:?} is empty",
                lang.code(),
                tables::KEY_NAMES[i]
            );
            assert_eq!(
                catalog::placeholders(v),
                catalog::placeholders(tables::TABLES[Lang::En as usize][i]),
                "{}: key {:?} has a different placeholder set than English",
                lang.code(),
                tables::KEY_NAMES[i]
            );
        }
        assert!(
            !lang.endonym().is_empty(),
            "{}: endonym is empty",
            lang.code()
        );
    }

    // самоназвания одинаковы во всех каталогах — иначе выбравшему китайский
    // нечего искать в русском списке
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

/// t126 (Р.19), часть «а»: переупорядочивание — по СЛОВАМ, а не по буквам.
///
/// egui 0.36 режет строку на прогоны по шрифтовому начертанию и отдаёт каждый
/// harfrust'у с `guess_segment_properties()` — тот сам разворачивает глифы
/// внутри прогона. Поэтому символы внутри арабского атома обязаны остаться в
/// ЛОГИЧЕСКОМ порядке, а меняться местами должны атомы. Разбор — в `bidi.rs`.
#[test]
fn t126_bidi_reorders_words_not_letters() {
    use crate::i18n::bidi;

    // без арабского строка не трогается вовсе
    for s in ["", "Save", "1280x720", "00:01:30", "Ctrl+S"] {
        assert_eq!(bidi::visual(s), s, "LTR text must pass through untouched");
    }

    let one = "\u{0645}\u{0644}\u{0641}"; // «файл»
    let two = "\u{062D}\u{0641}\u{0638}"; // «сохранить»

    // одно слово: порядок символов внутри слова НЕ меняется
    assert_eq!(bidi::visual(one), one, "letters inside a word must stay logical");

    // два слова меняются местами, буквы внутри каждого — нет
    let phrase = format!("{two} {one}");
    assert_eq!(
        bidi::visual(&phrase),
        format!("{one} {two}"),
        "words must swap, letters must not"
    );

    // латиница/цифры внутри арабской фразы уезжают на свою визуальную позицию
    let mixed = format!("{one}: 42");
    let vis = bidi::visual(&mixed);
    assert!(
        vis.starts_with("42"),
        "an LTR number must move to the left of an RTL phrase, got {vis:?}"
    );
    assert!(vis.contains(one), "the word itself must survive: {vis:?}");

    // скобки на RTL-уровне зеркалятся
    let paren = format!("({one})");
    let vis = bidi::visual(&paren);
    assert!(
        vis.starts_with('(') && vis.ends_with(')'),
        "mirrored brackets must still read as a pair, got {vis:?}"
    );

    // табуляция и перенос — жёсткие разделители, они не уезжают внутрь фразы
    let tabbed = format!("{one}\tCtrl+S");
    assert_eq!(
        bidi::visual(&tabbed),
        format!("{one}\tCtrl+S"),
        "a tab separates the label from its accelerator and must not move"
    );
}

/// t126, часть «в»: секции, которые сравниваются с вводом пользователя, в
/// арабской таблице лежат в ЛОГИЧЕСКОМ порядке.
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

/// Разбор каталога: формат строгий намеренно — молча съеденная строка хуже,
/// чем упавшая сборка.
#[test]
fn t125_catalog_parser_is_strict() {
    let ok = catalog::parse("[a.b]\nkey = \"value\" # note\n").expect("valid catalog");
    assert_eq!(ok.len(), 1);
    assert_eq!(ok[0].key, "a.b.key");
    assert_eq!(ok[0].value, "value");

    let esc = catalog::parse(r#"k = "a\tb\nc\"d\\e\u00ab""#).expect("escapes");
    assert_eq!(esc[0].value, "a\tb\nc\"d\\e\u{00ab}");

    for bad in [
        "key = value",            // без кавычек
        "Key = \"v\"",            // ключ не в нижнем регистре
        "[a b]\nk = \"v\"",       // пробел в секции
        "k = \"unterminated",     // не закрыта строка
        "k = \"v\" junk",         // мусор после значения
        "k = \"v\"\nk = \"w\"",   // дубликат
        "k = \"\\q\"",            // неизвестный эскейп
    ] {
        assert!(catalog::parse(bad).is_err(), "must be rejected: {bad:?}");
    }

    assert_eq!(catalog::placeholders("{0} and {2} and {0}"), vec![0, 2]);
    assert_eq!(catalog::placeholders("{{0}} literal"), Vec::<usize>::new());
}

/// t130 (Р.19): язык при первом запуске берётся из локали ОС.
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

/// t131 (Р.19): исходный язык кодовой базы — английский.
///
/// Пользователь потребовал: «если что-то остается на русском — то надо
/// перевести на английский и далее все агенты по дефолту все строки пишут на
/// английском». Без теста это памятка, а не правило: следующий агент напишет
/// `note("не открылось")`, оно пройдёт ревью и уедет мимо каталога — то есть
/// не переведётся никогда.
///
/// Русский текст интерфейса живёт ТОЛЬКО в `i18n/ru.toml`. Комментарии и
/// `docs/*.md` остаются русскими — это конвенция репозитория, правило про
/// строковые ЛИТЕРАЛЫ. Тестовый код исключается по той же логике, что в t100:
/// текст ассертов пользователь не видит.
#[test]
fn t131_source_language_is_english() {
    fn is_cyrillic(c: char) -> bool {
        // кириллица + кириллица-дополнение
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
                // target/ — сборка, vendor/ — чужие заголовки FFmpeg,
                // tests/ — интеграционные тесты (текст ассертов не виден)
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
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().into_owned();
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

/// Тест 171 (п.4 Р.28): пункт меню говорит, ЧТО и ГДЕ произойдёт.
///
/// Жалоба дословно: «fade out — вообще не понятно по названию пункта куда он
/// добавит этот out… И надпись "Remove crossfade" тоже не очевидно,
/// оказывается она fadeout с конца убирает». Одна надпись на четыре разных
/// действия — дефект; тест держит их РАЗЛИЧИМЫМИ в каждом языке, иначе
/// перевод «в одно слово» вернёт ту же неоднозначность.
#[test]
fn t171_transition_menu_labels_say_what_and_where() {
    use crate::i18n::k;
    let groups: [[i18n::Key; 4]; 1] = [[
        k::tl::menu::REMOVE_FADE_START,
        k::tl::menu::REMOVE_FADE_END,
        k::tl::menu::REMOVE_CROSSFADE_NEXT,
        k::tl::menu::REMOVE_CROSSFADE_PREV,
    ]];
    for lang in Lang::ALL {
        for keys in groups {
            let vals: Vec<&str> =
                keys.iter().map(|k| tables::TABLES[lang as usize][k.0 as usize]).collect();
            for (i, a) in vals.iter().enumerate() {
                for b in vals.iter().skip(i + 1) {
                    assert_ne!(a, b, "{}: пункты снятия перехода неразличимы", lang.code());
                }
            }
        }
        // начало и конец события различимы и у пунктов ДОБАВЛЕНИЯ
        assert_ne!(
            tables::TABLES[lang as usize][k::tl::menu::FADE_IN.0 as usize],
            tables::TABLES[lang as usize][k::tl::menu::FADE_OUT.0 as usize],
            "{}: появление и затухание неразличимы",
            lang.code()
        );
        // «Marker here» и «Marker over the region» — разные действия
        assert_ne!(
            tables::TABLES[lang as usize][k::tl::menu::MARKER_HERE.0 as usize],
            tables::TABLES[lang as usize][k::tl::menu::MARKER_REGION.0 as usize],
            "{}: точечная метка и метка-диапазон неразличимы",
            lang.code()
        );
    }
}

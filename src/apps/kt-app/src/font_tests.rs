//! Тест 100 (п.12 р.11): КАЖДЫЙ непечатный/неASCII символ, который программа
//! может показать, обязан существовать в шрифтах, которые она грузит.
//!
//! Пользователь дважды ловил квадрат-заглушку вместо символа (`●` U+25CF в
//! индикаторе изменений, неверные кодпоинты Phosphor). Правило «проверяй
//! глифы» без теста не работает — поэтому проверка автоматическая: сканер
//! разбирает исходники kt-app, достаёт содержимое строковых и символьных
//! литералов (включая `\u{...}`-эскейпы, которыми записаны иконки), и
//! проверяет каждый неASCII-символ по cmap ВСЕХ шрифтов семейства
//! Proportional (Ubuntu-Light, NotoEmoji, emoji-icon-font) плюс
//! вендоренных — ровно того набора, что ставит `install_fonts`.
//!
//! С Р.19 сканируются ЕЩЁ И каталоги перевода `i18n/*.toml`: текст интерфейса
//! переехал туда, и без этого t100 перестал бы проверять что-либо осмысленное
//! (в исходниках остались бы одни ключи из ASCII).

use std::collections::BTreeMap;

/// Все шрифты, которые реально грузит программа: дефолт egui + вендоренные.
/// Список вендоренных берётся из `icons::VENDORED`, а не дублируется здесь:
/// расхождение между тестом и `install_fonts` — ровно та дыра, ради которой
/// этот тест и написан.
fn loaded_fonts() -> Vec<(String, Vec<u8>)> {
    let defs = eframe::egui::FontDefinitions::default();
    let family = defs
        .families
        .get(&eframe::egui::FontFamily::Proportional)
        .expect("нет семейства Proportional")
        .clone();
    let mut out = Vec::new();
    for name in family {
        let data = defs.font_data.get(&name).expect("шрифт семейства не загружен");
        out.push((name, data.font.to_vec()));
    }
    for (name, bytes) in crate::icons::VENDORED {
        out.push((name.to_string(), bytes.to_vec()));
    }
    out
}

/// Каталоги перевода: сами значения, без ключей и комментариев.
fn catalog_values() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("i18n");
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).expect("no i18n/") {
        let p = e.expect("bad directory entry").path();
        if p.extension().map(|x| x == "toml").unwrap_or(false) {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let src = std::fs::read_to_string(&p).expect("unreadable catalog");
            let entries = crate::i18n::catalog::parse(&src)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            for entry in entries {
                out.push((format!("{name}:{}", entry.line), entry.value));
            }
        }
    }
    out.sort();
    out
}

/// Есть ли глиф символа хотя бы в одном шрифте семейства.
fn covered(fonts: &[(String, ab_glyph::FontVec)], ch: char) -> bool {
    use ab_glyph::Font as _;
    fonts.iter().any(|(_, f)| f.glyph_id(ch).0 != 0)
}

/// Символы, которые программа НЕ рисует, хотя они лежат в литералах:
/// пробельные и управляющие (`\n`, NBSP и т.п.) глифов не требуют.
fn needs_glyph(ch: char) -> bool {
    !ch.is_ascii() && !ch.is_whitespace() && !ch.is_control()
}

/// Разбор исходника: содержимое строковых/символьных литералов, вне
/// комментариев. `\u{...}` раскрывается — иконки записаны именно так.
pub fn literal_chars(src: &str) -> Vec<(usize, char)> {
    let b: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;
    // счётчик строк ведём по мере продвижения i
    let bump = |c: char, line: &mut usize| {
        if c == '\n' {
            *line += 1;
        }
    };
    while i < b.len() {
        let c = b[i];
        // комментарии
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
        // сырая строка r"..." / r#"..."#
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
        // обычная строка
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
        // символьный литерал против лайфтайма: 'x' / '\n' / 'a
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

/// Исходники UI без тестового кода: текст ассертов и панику пользователь не
/// видит, а стрелки/символы там законны. Тестовый модуль в файле начинается
/// строкой `#[cfg(test)]` с нулевым отступом (атрибуты полей идут с отступом).
///
/// Обход РЕКУРСИВНЫЙ: с Р.19 часть кода живёт в `src/i18n/`, и плоский
/// `read_dir` молча выронил бы его из t100 и t131.
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
            let name = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .into_owned();
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

/// t100: ни одного символа без глифа во всех строках UI.
#[test]
fn t100_ui_chars_have_glyphs() {
    let fonts: Vec<(String, ab_glyph::FontVec)> = loaded_fonts()
        .into_iter()
        .map(|(n, d)| {
            (
                n,
                ab_glyph::FontVec::try_from_vec(d).expect("шрифт не парсится"),
            )
        })
        .collect();

    // символы, которые ТОЧНО должны быть/не быть — страховка самого теста:
    // если проверка сломается и станет всё пропускать, эти две упадут.
    for (ch, what) in [
        ('я', "кириллица"),
        ('ñ', "латиница с диакритикой"),
        ('日', "иероглиф из сабсета CJK"),
        ('ب', "арабская буква из сабсета"),
    ] {
        assert!(covered(&fonts, ch), "{what} обязана быть покрыта");
    }
    // Страховка от вырождения: если проверка сломается и станет пропускать
    // всё, эта упадёт. U+25CF сюда больше не годится — он есть и в Hack, и в
    // Noto CJK; берём тайскую букву, которой нет ни в одном из наборов и
    // которая структурно не может попасть в сабсет (cmap режется по каталогам).
    assert!(
        !covered(&fonts, char::from_u32(0x0E01).unwrap()),
        "U+0E01 всё ещё отсутствует — если появился, самопроверку надо менять"
    );

    let mut missing: BTreeMap<char, Vec<String>> = BTreeMap::new();
    for (file, src) in sources() {
        for (line, ch) in literal_chars(&src) {
            if !covered(&fonts, ch) {
                missing
                    .entry(ch)
                    .or_default()
                    .push(format!("{file}:{line}"));
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
        "символы без глифа в шрифтах программы (пользователь увидит квадрат): {}",
        missing
            .iter()
            .map(|(ch, where_)| format!("U+{:04X} {:?} — {}", *ch as u32, ch, where_.join(", ")))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

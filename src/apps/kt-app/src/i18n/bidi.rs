//! Порядок арабского текста для egui 0.36.
//!
//! ЗАЧЕМ ЭТО ВООБЩЕ НУЖНО и почему нельзя взять `unicode_bidi::reorder_line`:
//!
//! * `epaint::text::text_layout::segment_into_runs` режет строку на прогоны ПО
//!   ШРИФТОВОМУ НАЧЕРТАНИЮ, без липкости для нейтралов: пробел резолвится в
//!   Ubuntu-Light, арабская буква — в арабский фолбэк, значит каждый пробел
//!   рвёт арабскую фразу на отдельные прогоны.
//! * `shape_text` зовёт `buffer.guess_segment_properties()`, и для арабского
//!   прогона harfrust сам ставит direction=RTL: глифы внутри прогона он уже
//!   разворачивает, вязь соединяет верно.
//! * Прогоны при этом выкладываются слева направо в ЛОГИЧЕСКОМ порядке.
//!
//! Итого: одно арабское слово рисуется правильно уже сегодня, а фраза — с
//! верными буквами и неверным порядком слов. Разворот по СИМВОЛАМ сделал бы
//! хуже: harfrust развернул бы каждый прогон второй раз, а формы букв
//! посчитались бы на развёрнутой последовательности.
//!
//! Поэтому здесь переупорядочиваются АТОМЫ НАЧЕРТАНИЯ, а символы внутри атома
//! остаются в логическом порядке. Атом — максимальный кусок арабского блока
//! (его нарисует арабский шрифт) либо одиночный прочий символ. Предикат точен,
//! а не эвристичен: в загружаемом наборе арабские кодпоинты есть ровно в одном
//! шрифте (`icons.rs::install_fonts`, порядок семей менять нельзя).
//!
//! Закреплено тестом 126, включая проверку на падение с `reorder_line`.

use unicode_bidi::{BidiInfo, Level};

/// Кодпоинт рисуется арабским начертанием.
pub fn is_arabic(c: char) -> bool {
    matches!(c as u32,
        0x0600..=0x06FF   // Arabic
        | 0x0750..=0x077F // Arabic Supplement
        | 0x08A0..=0x08FF // Arabic Extended-A
        | 0xFB50..=0xFDFF // Arabic Presentation Forms-A
        | 0xFE70..=0xFEFF // Arabic Presentation Forms-B
    )
}

/// Есть ли в строке хоть что-то, что нужно переупорядочивать. Дешёвый скан:
/// для LTR-языков это единственная цена всего механизма.
///
/// Намеренно ограничен арабским: иврит мы не поставляем, и трогать ивритское
/// имя файла (оно прошло бы как «прочие символы», по одному в атоме, то есть
/// задом наперёд) хуже, чем оставить как есть.
pub fn has_rtl(s: &str) -> bool {
    s.chars().any(is_arabic)
}

/// Зеркальные пары для символов на RTL-уровне (UAX #9, Bidi_Mirrored).
/// Список короткий намеренно: это ровно те скобки и кавычки, которые могут
/// встретиться в каталоге.
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
        '\u{00AB}' => '\u{00BB}', // « »
        '\u{00BB}' => '\u{00AB}',
        '\u{2039}' => '\u{203A}', // ‹ ›
        '\u{203A}' => '\u{2039}',
        other => other,
    }
}

/// Логический порядок → визуальный. Результат отдаётся egui как есть.
pub fn visual(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    visual_into(s, &mut out);
    out
}

/// То же, но в чужой буфер: после прогрева не аллоцирует вовсе (см.
/// `Memo` — заготовка на случай, если профиль когда-нибудь покажет цену).
pub fn visual_into(s: &str, out: &mut String) {
    out.clear();
    if !has_rtl(s) {
        out.push_str(s);
        return;
    }
    // `\n` и `\t` — жёсткие разделители: перенос строки не должен уезжать
    // внутрь фразы, а таб разделяет пункт меню и его ярлык.
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

/// Один абзац без переносов: прогоны в визуальном порядке, RTL-прогоны — по
/// атомам.
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

/// Атомы прогона в обратном порядке; внутри арабского атома символы остаются
/// логическими — их развернёт и соединит harfrust.
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

/// Одноместный мемо для строк, собираемых на каждом кадре. Пока не нужен
/// (таких мест около десятка и они и так зовут `format!`), но лежит здесь,
/// чтобы фикс был подстановкой, а не переписыванием.
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

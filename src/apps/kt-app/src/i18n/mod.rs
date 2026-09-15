//! Мультиязычность интерфейса (Р.19): en / ru / es / zh-Hans / ar.
//!
//! Текст живёт в `apps/kt-app/i18n/*.toml`; `build.rs` проверяет каталоги и
//! генерирует из них статические таблицы, поэтому `t()` отдаёт `&'static str`
//! без аллокаций и без блокировок — инвариант «никаких аллокаций в draw-путях»
//! не задет.
//!
//! Английский — исходный язык кодовой базы: любой новый строковый литерал
//! пишется по-английски, русский текст интерфейса живёт только в `ru.toml`
//! (правило Р.19, закреплено тестом 131).

use std::cell::Cell;
use std::fmt;

pub mod bidi;
#[allow(dead_code)] // рабочий пользователь — build.rs через #[path]; тут его дёргают тесты
pub mod catalog;
pub mod detect;
mod keys;
pub(crate) mod tables;

// `N_KEYS` нужен только тестам целостности каталога (125)
#[allow(unused_imports)]
pub use keys::{N_KEYS, k};

/// Индекс строки в таблицах. Не enum: вариантов сотни, а так call-site читается
/// как путь секции каталога (`k::menu::file::SAVE`) и опечатка — ошибка сборки.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key(pub u16);

/// Порядок вариантов = порядок таблиц в `tables.rs`; менять только вместе с
/// `LANG_FILES` в `build.rs`.
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

    /// Код для хранения в eframe Storage и для `KT_LANG`.
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

    /// Самоназвание: в списке языков каждый пункт написан на своём языке,
    /// иначе выбравшему китайский нечего искать в русском списке.
    pub fn endonym(self) -> &'static str {
        tables::ENDONYM[self as usize]
    }

    pub fn is_rtl(self) -> bool {
        matches!(self, Lang::Ar)
    }
}

// Поток, а не глобальный атомик: `cargo test` гоняет тесты параллельно, и
// пятиязычные харнесс-тесты вёрстки иначе мешали бы друг другу. UI однопоточен
// (eframe), фоновые потоки `t()` не зовут и получили бы английский.
thread_local! {
    static LANG: Cell<Lang> = const { Cell::new(Lang::En) };
}

pub fn set_lang(l: Lang) {
    LANG.with(|c| c.set(l));
}

/// Переключение из UI: помимо самого языка правит стиль и просит перерисовку.
///
/// `selectable_labels` для арабского выключается намеренно: подписи лежат в
/// ВИЗУАЛЬНОМ порядке (см. `bidi.rs`), и скопированный из них текст был бы
/// мусором. Поля ввода это не затрагивает — их содержимое не переупорядочивается.
pub fn set_lang_ctx(ctx: &eframe::egui::Context, l: Lang) {
    set_lang(l);
    ctx.all_styles_mut(|st| st.interaction.selectable_labels = !l.is_rtl());
    ctx.request_repaint();
}

/// Выбор пользователя (`None` = «авто») → фактический язык.
pub fn resolve(choice: Option<Lang>) -> Lang {
    choice.or_else(detect::from_env).unwrap_or(Lang::En)
}

/// Как выбор хранится в eframe Storage и в `KT_LANG`.
pub fn choice_code(choice: Option<Lang>) -> &'static str {
    match choice {
        Some(l) => l.code(),
        None => "auto",
    }
}

/// Обратно: всё нераспознанное — «авто», а не ошибка (Storage переживает
/// смену набора языков).
pub fn choice_from_code(s: &str) -> Option<Lang> {
    Lang::from_code(s)
}

pub fn current() -> Lang {
    LANG.with(|c| c.get())
}

/// Строка на текущем языке. Имя короткое намеренно: этот вызов стоит на
/// каждой надписи. `t` занято под тики времени в timeline.rs.
#[inline]
pub fn tr(k: Key) -> &'static str {
    tables::TABLES[current() as usize][k.0 as usize]
}

/// Английский вариант — нужен поиску по FX, чтобы «blur» находил эффект в
/// любой локали.
#[inline]
pub fn tr_en(k: Key) -> &'static str {
    tables::TABLES[Lang::En as usize][k.0 as usize]
}

/// Арабский ДО переупорядочивания: сравнивать с пользовательским вводом можно
/// только логический порядок. Расходуется тестом 126.
#[allow(dead_code)]
#[inline]
pub fn ar_logical(k: Key) -> &'static str {
    tables::AR_LOGICAL[k.0 as usize]
}

/// Имя ключа (`"menu.file.save"`) — для диагностики и тестов.
#[allow(dead_code)]
pub fn key_name(k: Key) -> &'static str {
    tables::KEY_NAMES[k.0 as usize]
}

/// Подстановка позиционных аргументов `{0}`, `{1}`, … Позиционные, а не `{}`:
/// китайскому и арабскому регулярно нужен другой порядок аргументов.
/// `{{` и `}}` — литеральные скобки.
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
    // Уровень 2 порядка RTL: статические строки переупорядочены сборкой, а
    // собранные из аргументов — здесь. Для остальных языков это один скан.
    if current().is_rtl() { bidi::visual(&out) } else { out }
}

/// `tf!(k::msg::CLIPS_COPIED, n)` — строка каталога с аргументами.
#[macro_export]
macro_rules! tf {
    ($k:expr $(, $a:expr)* $(,)?) => {
        $crate::i18n::interp($crate::i18n::tr($k), &[$(&$a as &dyn ::std::fmt::Display),*])
    };
}

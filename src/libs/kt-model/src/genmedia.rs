//! Генераторы медиа (Р.52, F-TXT-003 + F-TXT-001): медиа, у которого нет
//! файла — картинку считает источник. Параметры генератора — функция
//! МЕДИА, а не клипа (кэш движка ключуется MediaId), поэтому они живут в
//! `MediaRef.path` строкой `gen:<вид>[:<ключ>=<знач>]*` — как тестовая
//! схема `synth:`, ноль новых полей в модели и в kt-секции. Здесь —
//! ЕДИНСТВЕННЫЙ словарь видов и ЕДИНСТВЕННЫЙ разбор/сборка пути: разбор
//! `synth:` был продублирован в kt-app и kt-cli и уже расходился по
//! регистру (сноска п.211), поэтому разбор регистронезависимый, а вторых
//! парсеров не бывает. Рендерит кадры kt-media (`GeneratorSource`),
//! формат обмена — kt-otio (`GeneratorReference.1`).

use serde::{Deserialize, Serialize};

use crate::project::{fx_cast, params_for, HasParams, ParamKind, ParamSpec, ParamUnit};

/// Вид генератора с параметрами. Serde-представление (internally tagged
/// `kind`) нужно kt-otio: поля уезжают словарём `parameters` в
/// `GeneratorReference.1`, сам `kind` — строкой `generator_kind`
/// (`kt.<tag>`). Незнакомые поля игнорируются, пропущенные берут
/// умолчания — файл от будущей сборки читается без отказа.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GeneratorKind {
    /// Сплошной цвет — единственный генератор, которым пользуются
    /// ежедневно (разведка F-TXT-003). Умолчание — чёрный (правило UI 4).
    Solid {
        #[serde(default)]
        color_r: u8,
        #[serde(default)]
        color_g: u8,
        #[serde(default)]
        color_b: u8,
    },
}

impl GeneratorKind {
    /// Все виды с УМОЛЧАНИЯМИ параметров — единственный реестр (как
    /// `TransitionKind::ALL`): по нему меню создания в UI и полнота спек
    /// в тестах. Новый вид добавляется В КОНЕЦ.
    pub const ALL: &'static [GeneratorKind] =
        &[GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }];

    /// Тег вида — он же имя в `gen:`-пути и суффикс `generator_kind`
    /// (`kt.<tag>`) в файле обмена.
    pub fn tag(&self) -> &'static str {
        match self {
            GeneratorKind::Solid { .. } => "solid",
        }
    }

    /// Вид с умолчаниями по тегу (регистр тега — забота вызывающего:
    /// `parse_gen_path` приводит сам).
    pub fn default_for_tag(tag: &str) -> Option<GeneratorKind> {
        GeneratorKind::ALL.iter().copied().find(|k| k.tag() == tag)
    }
}

params_for! { GeneratorKind, gen_specs, gen_set_param, gen_get_param, {
    Solid {
        color_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
    },
}}

impl HasParams for GeneratorKind {
    fn param_specs(&self) -> &'static [ParamSpec] {
        gen_specs(self)
    }
    fn get_param(&self, name: &str) -> Option<f64> {
        gen_get_param(self, name)
    }
    fn set_param(&mut self, name: &str, v: f64) -> bool {
        gen_set_param(self, name, v)
    }
}

/// Путь-псевдосхема, за которым не стоит файла: `synth:` (тестовые
/// источники) или `gen:` (генераторы). Единый предикат для всех мест,
/// которые сегодня писали `starts_with("synth:")` руками: резолв путей,
/// watchdog офлайна, служба индекса, sidecar-БД.
pub fn is_pseudo_path(path: &str) -> bool {
    starts_ci(path, "synth:") || starts_ci(path, "gen:")
}

/// Разбор `gen:`-пути. Регистронезависимый по схеме, тегу и ключам
/// (значения-строки не трогаются). Любой незнакомый тег, ключ или битое
/// значение — `None`: выше это честная ошибка «файл отсутствует», а не
/// молчаливое умолчание (урок сноски п.211).
pub fn parse_gen_path(path: &str) -> Option<GeneratorKind> {
    let rest = strip_ci(path, "gen:")?;
    let mut toks = rest.split(':');
    let tag = toks.next()?.to_ascii_lowercase();
    let mut kind = GeneratorKind::default_for_tag(&tag)?;
    for t in toks {
        let (key, val) = t.split_once('=')?;
        apply_token(&mut kind, &key.to_ascii_lowercase(), val)?;
    }
    Some(kind)
}

/// Канонический `gen:`-путь: нижний регистр, умолчания не пишутся,
/// цвет — одним токеном `c=rrggbb`. `format(parse(s))` канонизирует;
/// модель всегда хранит канон (его производят окно и ридер kt-otio).
pub fn format_gen_path(k: &GeneratorKind) -> String {
    let mut out = format!("gen:{}", k.tag());
    match *k {
        GeneratorKind::Solid { color_r, color_g, color_b } => {
            if (color_r, color_g, color_b) != (0, 0, 0) {
                out.push_str(&format!(":c={color_r:02x}{color_g:02x}{color_b:02x}"));
            }
        }
    }
    out
}

/// Один `ключ=значение`-токен пути. Ключ уже в нижнем регистре.
fn apply_token(k: &mut GeneratorKind, key: &str, val: &str) -> Option<()> {
    match k {
        GeneratorKind::Solid { color_r, color_g, color_b } => match key {
            "c" => {
                (*color_r, *color_g, *color_b) = parse_rgb(val)?;
                Some(())
            }
            _ => None,
        },
    }
}

/// `rrggbb` (6 hex-цифр, регистр любой) → тройка компонент.
fn parse_rgb(s: &str) -> Option<(u8, u8, u8)> {
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some((c(0)?, c(2)?, c(4)?))
}

/// Экранирование строкового значения для токена `gen:`-пути (текст
/// титра): служебные `%`, `:`, `=` и управляющие — в `%XX`. Путь остаётся
/// одной строкой без переводов строк, а `unescape(escape(s)) == s` для
/// любой строки (тест t469b).
pub fn escape_gen_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '%' | ':' | '=' => out.push_str(&format!("%{:02X}", ch as u32)),
            c if (c as u32) < 0x20 => out.push_str(&format!("%{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Обратное к [`escape_gen_value`]. Битая последовательность — `None`
/// (правило то же, что у всего разбора: ошибка, а не догадка).
pub fn unescape_gen_value(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(ch) = it.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        let hi = it.next()?.to_digit(16)?;
        let lo = it.next()?.to_digit(16)?;
        out.push(char::from_u32(hi * 16 + lo)?);
    }
    Some(out)
}

fn starts_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

fn strip_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    starts_ci(s, prefix).then(|| &s[prefix.len()..])
}

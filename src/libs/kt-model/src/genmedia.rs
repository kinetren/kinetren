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

use crate::project::{
    flags_for, fx_cast, params_for, FlagSpec, HasParams, ParamKind, ParamScope, ParamSpec,
    ParamUnit,
};

/// Вид генератора с параметрами. Serde-представление (internally tagged
/// `kind`) нужно kt-otio: поля уезжают словарём `parameters` в
/// `GeneratorReference.1`, сам `kind` — строкой `generator_kind`
/// (`kt.<tag>`). Незнакомые поля игнорируются, пропущенные берут
/// умолчания — файл от будущей сборки читается без отказа.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Титр (F-TXT-001): текст рисует kt-text (вендоренный шрифт,
    /// правило детерминизма — kt-text/CLAUDE.md). Всё относительное:
    /// кегль — доля высоты кадра, трекинг/обводка/тень/паддинги — доли
    /// em. Поля оформления (F-TXT-005) и `order` — В ФОРМАТЕ с первого
    /// дня (урок Kdenlive: смена порядка константой перерисовала чужие
    /// проекты), даже пока их рендер (С3) и окно не готовы.
    Text {
        #[serde(default)]
        text: String,
        #[serde(default = "default_family")]
        family: String,
        #[serde(default)]
        bold: bool,
        #[serde(default = "default_text_size")]
        size: f64,
        #[serde(default = "default_one")]
        line_spacing: f64,
        #[serde(default)]
        tracking: f64,
        #[serde(default)]
        align: TextAlign,
        #[serde(default)]
        anchor: TextAnchor,
        #[serde(default = "default_half")]
        pos_x: f64,
        #[serde(default = "default_half")]
        pos_y: f64,
        #[serde(default = "default_255")]
        fill_r: u8,
        #[serde(default = "default_255")]
        fill_g: u8,
        #[serde(default = "default_255")]
        fill_b: u8,
        #[serde(default = "default_255")]
        fill_a: u8,
        #[serde(default)]
        stroke_w: f64,
        #[serde(default)]
        stroke_r: u8,
        #[serde(default)]
        stroke_g: u8,
        #[serde(default)]
        stroke_b: u8,
        #[serde(default = "default_255")]
        stroke_a: u8,
        #[serde(default = "default_shadow_off")]
        shadow_dx: f64,
        #[serde(default = "default_shadow_off")]
        shadow_dy: f64,
        #[serde(default = "default_shadow_blur")]
        shadow_blur: f64,
        #[serde(default)]
        shadow_r: u8,
        #[serde(default)]
        shadow_g: u8,
        #[serde(default)]
        shadow_b: u8,
        #[serde(default)]
        shadow_a: u8,
        #[serde(default = "default_plate_px")]
        plate_pad_x: f64,
        #[serde(default = "default_plate_py")]
        plate_pad_y: f64,
        #[serde(default)]
        plate_r: u8,
        #[serde(default)]
        plate_g: u8,
        #[serde(default)]
        plate_b: u8,
        #[serde(default)]
        plate_a: u8,
        #[serde(default = "default_order")]
        order: [TextLayer; 4],
    },
    /// Линейный градиент между двумя цветами (угол в градусах).
    Gradient {
        #[serde(default = "default_angle")]
        angle_deg: f64,
        #[serde(default)]
        a_r: u8,
        #[serde(default)]
        a_g: u8,
        #[serde(default)]
        a_b: u8,
        #[serde(default = "default_255")]
        b_r: u8,
        #[serde(default = "default_255")]
        b_g: u8,
        #[serde(default = "default_255")]
        b_b: u8,
    },
    /// Четырёхцветный градиент по углам (билинейный).
    Corners {
        #[serde(default = "default_255")]
        tl_r: u8,
        #[serde(default)]
        tl_g: u8,
        #[serde(default)]
        tl_b: u8,
        #[serde(default)]
        tr_r: u8,
        #[serde(default = "default_255")]
        tr_g: u8,
        #[serde(default)]
        tr_b: u8,
        #[serde(default)]
        bl_r: u8,
        #[serde(default)]
        bl_g: u8,
        #[serde(default = "default_255")]
        bl_b: u8,
        #[serde(default = "default_255")]
        br_r: u8,
        #[serde(default = "default_255")]
        br_g: u8,
        #[serde(default = "default_255")]
        br_b: u8,
    },
    /// Шахматка: клетка задаётся СЧЁТОМ колонок (resolution-independent).
    Checker {
        #[serde(default = "default_cols")]
        cols: u32,
        #[serde(default)]
        a_r: u8,
        #[serde(default)]
        a_g: u8,
        #[serde(default)]
        a_b: u8,
        #[serde(default = "default_255")]
        b_r: u8,
        #[serde(default = "default_255")]
        b_g: u8,
        #[serde(default = "default_255")]
        b_b: u8,
    },
    /// Шумовое поле: counter-based зерно от (x, y, pts МАТЕРИАЛА) —
    /// сдвиг клипа зерно не меняет; полный диапазон яркости («серое
    /// поле», которого Noise-ЭФФЕКТ дать не может — F-TXT-003).
    #[serde(rename = "noise")]
    NoiseField {
        #[serde(default = "default_one")]
        amount: f64,
        #[serde(default = "default_true_flag")]
        mono: bool,
    },
    /// SMPTE-полосы 75 % (+ опоры в нижней трети); тон 1 кГц — параметр:
    /// включение делает медиа звучащим (уровень в dBFS).
    #[serde(rename = "smpte")]
    SmpteBars {
        #[serde(default)]
        tone: bool,
        #[serde(default = "default_tone_db")]
        tone_db: f64,
    },
    /// Тайм-код БЕГУЩИМИ цифрами. Показывает время МАТЕРИАЛА (источник
    /// «часов таймлайна» не видит по построению): клип от нуля с
    /// source_in = 0 совпадает с часами таймлайна, трим/сдвиг показывают
    /// материал. Burn-in выхода — другой будущий объект (F-TXT-003).
    Timecode {
        #[serde(default = "default_tc_fps")]
        fps: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// Якорь 3×3: какой точкой блок текста сидит в (pos_x, pos_y).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAnchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    #[default]
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Слой оформления титра, порядок отрисовки снизу вверх.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextLayer {
    Plate,
    Shadow,
    Stroke,
    Fill,
}

fn default_family() -> String {
    "Noto Sans".into()
}
fn default_text_size() -> f64 {
    0.07
}
fn default_one() -> f64 {
    1.0
}
fn default_half() -> f64 {
    0.5
}
fn default_255() -> u8 {
    255
}
fn default_shadow_off() -> f64 {
    0.04
}
fn default_shadow_blur() -> f64 {
    0.06
}
fn default_plate_px() -> f64 {
    0.25
}
fn default_plate_py() -> f64 {
    0.12
}
fn default_angle() -> f64 {
    90.0
}
fn default_cols() -> u32 {
    8
}
fn default_true_flag() -> bool {
    true
}
fn default_tone_db() -> f64 {
    -20.0
}
fn default_tc_fps() -> u32 {
    30
}
fn default_order() -> [TextLayer; 4] {
    [TextLayer::Plate, TextLayer::Shadow, TextLayer::Stroke, TextLayer::Fill]
}

impl GeneratorKind {
    /// Все виды с УМОЛЧАНИЯМИ параметров — единственный реестр (роль
    /// `TransitionKind::ALL`; функция, а не const: у титра есть String):
    /// по нему меню создания в UI и полнота спек в тестах. Новый вид
    /// добавляется В КОНЕЦ.
    pub fn all() -> Vec<GeneratorKind> {
        vec![
            GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 },
            GeneratorKind::default_text(),
            GeneratorKind::Gradient {
                angle_deg: 90.0,
                a_r: 0,
                a_g: 0,
                a_b: 0,
                b_r: 255,
                b_g: 255,
                b_b: 255,
            },
            GeneratorKind::Corners {
                tl_r: 255,
                tl_g: 0,
                tl_b: 0,
                tr_r: 0,
                tr_g: 255,
                tr_b: 0,
                bl_r: 0,
                bl_g: 0,
                bl_b: 255,
                br_r: 255,
                br_g: 255,
                br_b: 255,
            },
            GeneratorKind::Checker { cols: 8, a_r: 0, a_g: 0, a_b: 0, b_r: 255, b_g: 255, b_b: 255 },
            GeneratorKind::NoiseField { amount: 1.0, mono: true },
            GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 },
            GeneratorKind::Timecode { fps: 30 },
        ]
    }

    /// Титр с умолчаниями (текст пуст — его набирают в окне).
    pub fn default_text() -> GeneratorKind {
        GeneratorKind::Text {
            text: String::new(),
            family: default_family(),
            bold: false,
            size: default_text_size(),
            line_spacing: 1.0,
            tracking: 0.0,
            align: TextAlign::Center,
            anchor: TextAnchor::Center,
            pos_x: 0.5,
            pos_y: 0.5,
            fill_r: 255,
            fill_g: 255,
            fill_b: 255,
            fill_a: 255,
            stroke_w: 0.0,
            stroke_r: 0,
            stroke_g: 0,
            stroke_b: 0,
            stroke_a: 255,
            shadow_dx: default_shadow_off(),
            shadow_dy: default_shadow_off(),
            shadow_blur: default_shadow_blur(),
            shadow_r: 0,
            shadow_g: 0,
            shadow_b: 0,
            shadow_a: 0,
            plate_pad_x: default_plate_px(),
            plate_pad_y: default_plate_py(),
            plate_r: 0,
            plate_g: 0,
            plate_b: 0,
            plate_a: 0,
            order: default_order(),
        }
    }

    /// Тег вида — он же имя в `gen:`-пути и суффикс `generator_kind`
    /// (`kt.<tag>`) в файле обмена.
    pub fn tag(&self) -> &'static str {
        match self {
            GeneratorKind::Solid { .. } => "solid",
            GeneratorKind::Text { .. } => "text",
            GeneratorKind::Gradient { .. } => "gradient",
            GeneratorKind::Corners { .. } => "corners",
            GeneratorKind::Checker { .. } => "checker",
            GeneratorKind::NoiseField { .. } => "noise",
            GeneratorKind::SmpteBars { .. } => "smpte",
            GeneratorKind::Timecode { .. } => "timecode",
        }
    }

    /// Вид с умолчаниями по тегу (регистр тега — забота вызывающего:
    /// `parse_gen_path` приводит сам).
    pub fn default_for_tag(tag: &str) -> Option<GeneratorKind> {
        GeneratorKind::all().into_iter().find(|k| k.tag() == tag)
    }
}

params_for! { GeneratorKind, gen_specs, gen_set_param, gen_get_param, {
    Solid {
        color_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
    },
    // числовые параметры титра; текст/семья/выравнивание/цвета — не
    // спеки (свои контролы окна, «законное исключение» F-TXT-001)
    Text {
        size: f64, 0.005, 0.5, 0.07, 0.001, Norm, Smooth;
        line_spacing: f64, 0.5, 3.0, 1.0, 0.05, Ratio, Smooth;
        tracking: f64, -0.05, 0.5, 0.0, 0.005, Norm, Smooth;
        pos_x: f64, 0.0, 1.0, 0.5, 0.001, Norm, Smooth;
        pos_y: f64, 0.0, 1.0, 0.5, 0.001, Norm, Smooth;
        stroke_w: f64, 0.0, 0.25, 0.0, 0.005, Norm, Smooth;
        shadow_dx: f64, -0.5, 0.5, 0.04, 0.005, Norm, Smooth;
        shadow_dy: f64, -0.5, 0.5, 0.04, 0.005, Norm, Smooth;
        shadow_blur: f64, 0.0, 0.5, 0.06, 0.005, Norm, Smooth;
        plate_pad_x: f64, 0.0, 2.0, 0.25, 0.01, Norm, Smooth;
        plate_pad_y: f64, 0.0, 2.0, 0.12, 0.01, Norm, Smooth;
    },
    Gradient {
        angle_deg: f64, 0.0, 360.0, 90.0, 1.0, Deg, Smooth;
        a_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        a_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        a_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        b_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        b_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        b_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Corners {
        tl_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        tl_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        tl_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        tr_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        tr_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        tr_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        bl_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        bl_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        bl_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        br_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        br_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        br_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Checker {
        cols: u32, 1.0, 256.0, 8.0, 1.0, Count, Structural;
        a_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        a_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        a_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        b_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        b_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        b_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    NoiseField {
        amount: f64, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
    },
    SmpteBars {
        tone_db: f64, -60.0, 0.0, -20.0, 0.5, Db, Smooth;
    },
    Timecode {
        fps: u32, 1.0, 120.0, 30.0, 1.0, Count, Structural;
    },
}}

flags_for! { GeneratorKind, gen_flag_specs, gen_set_flag, gen_get_flag, {
    // «да/нет» ползунком не тянется и кривой не автоматизируется, но и
    // забытым быть не должно: тон 1 кГц и цветной шум существовали в
    // формате с Р.52, а включить их было нечем (Р.53, п.414).
    NoiseField { mono: true },
    SmpteBars { tone: false },
    Text { bold: false },
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
    fn params_scope(&self) -> ParamScope {
        ParamScope::Generator
    }
    fn flag_specs(&self) -> &'static [FlagSpec] {
        gen_flag_specs(self)
    }
    fn get_flag(&self, name: &str) -> Option<bool> {
        gen_get_flag(self, name)
    }
    fn set_flag(&mut self, name: &str, v: bool) -> bool {
        gen_set_flag(self, name, v)
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
    let mut push = |key: &str, val: String| {
        out.push(':');
        out.push_str(key);
        out.push('=');
        out.push_str(&val);
    };
    match k {
        GeneratorKind::Solid { color_r, color_g, color_b } => {
            if (*color_r, *color_g, *color_b) != (0, 0, 0) {
                push("c", format!("{color_r:02x}{color_g:02x}{color_b:02x}"));
            }
        }
        GeneratorKind::Text {
            text,
            family,
            bold,
            size,
            line_spacing,
            tracking,
            align,
            anchor,
            pos_x,
            pos_y,
            fill_r,
            fill_g,
            fill_b,
            fill_a,
            stroke_w,
            stroke_r,
            stroke_g,
            stroke_b,
            stroke_a,
            shadow_dx,
            shadow_dy,
            shadow_blur,
            shadow_r,
            shadow_g,
            shadow_b,
            shadow_a,
            plate_pad_x,
            plate_pad_y,
            plate_r,
            plate_g,
            plate_b,
            plate_a,
            order,
        } => {
            // умолчания не пишутся; f64 — кратчайшая точная запись Display
            if !text.is_empty() {
                push("t", escape_gen_value(text));
            }
            if family != &default_family() {
                push("f", escape_gen_value(family));
            }
            if *bold {
                push("b", "1".into());
            }
            let f = |v: &f64| format!("{v}");
            if *size != default_text_size() {
                push("s", f(size));
            }
            if *line_spacing != 1.0 {
                push("ls", f(line_spacing));
            }
            if *tracking != 0.0 {
                push("tr", f(tracking));
            }
            if *align != TextAlign::default() {
                push("al", align_token(*align).into());
            }
            if *anchor != TextAnchor::default() {
                push("an", anchor_token(*anchor).into());
            }
            if *pos_x != 0.5 {
                push("px", f(pos_x));
            }
            if *pos_y != 0.5 {
                push("py", f(pos_y));
            }
            let rgba =
                |r: &u8, g: &u8, b: &u8, a: &u8| format!("{r:02x}{g:02x}{b:02x}{a:02x}");
            if (*fill_r, *fill_g, *fill_b, *fill_a) != (255, 255, 255, 255) {
                push("c", rgba(fill_r, fill_g, fill_b, fill_a));
            }
            if *stroke_w != 0.0 {
                push("sw", f(stroke_w));
            }
            if (*stroke_r, *stroke_g, *stroke_b, *stroke_a) != (0, 0, 0, 255) {
                push("sc", rgba(stroke_r, stroke_g, stroke_b, stroke_a));
            }
            if *shadow_dx != default_shadow_off() {
                push("sdx", f(shadow_dx));
            }
            if *shadow_dy != default_shadow_off() {
                push("sdy", f(shadow_dy));
            }
            if *shadow_blur != default_shadow_blur() {
                push("sb", f(shadow_blur));
            }
            if (*shadow_r, *shadow_g, *shadow_b, *shadow_a) != (0, 0, 0, 0) {
                push("shc", rgba(shadow_r, shadow_g, shadow_b, shadow_a));
            }
            if *plate_pad_x != default_plate_px() {
                push("ppx", f(plate_pad_x));
            }
            if *plate_pad_y != default_plate_py() {
                push("ppy", f(plate_pad_y));
            }
            if (*plate_r, *plate_g, *plate_b, *plate_a) != (0, 0, 0, 0) {
                push("plc", rgba(plate_r, plate_g, plate_b, plate_a));
            }
            if *order != default_order() {
                push("or", order.iter().map(|l| layer_letter(*l)).collect());
            }
        }
        GeneratorKind::Gradient { angle_deg, a_r, a_g, a_b, b_r, b_g, b_b } => {
            if *angle_deg != default_angle() {
                push("ang", format!("{angle_deg}"));
            }
            if (*a_r, *a_g, *a_b) != (0, 0, 0) {
                push("a", format!("{a_r:02x}{a_g:02x}{a_b:02x}"));
            }
            if (*b_r, *b_g, *b_b) != (255, 255, 255) {
                push("b", format!("{b_r:02x}{b_g:02x}{b_b:02x}"));
            }
        }
        GeneratorKind::Corners {
            tl_r,
            tl_g,
            tl_b,
            tr_r,
            tr_g,
            tr_b,
            bl_r,
            bl_g,
            bl_b,
            br_r,
            br_g,
            br_b,
        } => {
            let hex = |r: &u8, g: &u8, b: &u8| format!("{r:02x}{g:02x}{b:02x}");
            if (*tl_r, *tl_g, *tl_b) != (255, 0, 0) {
                push("tl", hex(tl_r, tl_g, tl_b));
            }
            if (*tr_r, *tr_g, *tr_b) != (0, 255, 0) {
                push("tr", hex(tr_r, tr_g, tr_b));
            }
            if (*bl_r, *bl_g, *bl_b) != (0, 0, 255) {
                push("bl", hex(bl_r, bl_g, bl_b));
            }
            if (*br_r, *br_g, *br_b) != (255, 255, 255) {
                push("br", hex(br_r, br_g, br_b));
            }
        }
        GeneratorKind::Checker { cols, a_r, a_g, a_b, b_r, b_g, b_b } => {
            if *cols != default_cols() {
                push("n", format!("{cols}"));
            }
            if (*a_r, *a_g, *a_b) != (0, 0, 0) {
                push("a", format!("{a_r:02x}{a_g:02x}{a_b:02x}"));
            }
            if (*b_r, *b_g, *b_b) != (255, 255, 255) {
                push("b", format!("{b_r:02x}{b_g:02x}{b_b:02x}"));
            }
        }
        GeneratorKind::NoiseField { amount, mono } => {
            if *amount != 1.0 {
                push("amt", format!("{amount}"));
            }
            if !*mono {
                push("mono", "0".into());
            }
        }
        GeneratorKind::SmpteBars { tone, tone_db } => {
            if *tone {
                push("tone", "1".into());
            }
            if *tone_db != default_tone_db() {
                push("db", format!("{tone_db}"));
            }
        }
        GeneratorKind::Timecode { fps } => {
            if *fps != default_tc_fps() {
                push("fps", format!("{fps}"));
            }
        }
    }
    out
}

fn align_token(a: TextAlign) -> &'static str {
    match a {
        TextAlign::Left => "l",
        TextAlign::Center => "c",
        TextAlign::Right => "r",
    }
}

fn align_from(s: &str) -> Option<TextAlign> {
    Some(match s {
        "l" => TextAlign::Left,
        "c" => TextAlign::Center,
        "r" => TextAlign::Right,
        _ => return None,
    })
}

fn anchor_token(a: TextAnchor) -> &'static str {
    match a {
        TextAnchor::TopLeft => "tl",
        TextAnchor::TopCenter => "tc",
        TextAnchor::TopRight => "tr",
        TextAnchor::CenterLeft => "cl",
        TextAnchor::Center => "cc",
        TextAnchor::CenterRight => "cr",
        TextAnchor::BottomLeft => "bl",
        TextAnchor::BottomCenter => "bc",
        TextAnchor::BottomRight => "br",
    }
}

fn anchor_from(s: &str) -> Option<TextAnchor> {
    Some(match s {
        "tl" => TextAnchor::TopLeft,
        "tc" => TextAnchor::TopCenter,
        "tr" => TextAnchor::TopRight,
        "cl" => TextAnchor::CenterLeft,
        "cc" => TextAnchor::Center,
        "cr" => TextAnchor::CenterRight,
        "bl" => TextAnchor::BottomLeft,
        "bc" => TextAnchor::BottomCenter,
        "br" => TextAnchor::BottomRight,
        _ => return None,
    })
}

fn layer_letter(l: TextLayer) -> char {
    match l {
        TextLayer::Plate => 'p',
        TextLayer::Shadow => 'h',
        TextLayer::Stroke => 's',
        TextLayer::Fill => 'f',
    }
}

fn order_from(s: &str) -> Option<[TextLayer; 4]> {
    let mut out = [TextLayer::Plate; 4];
    let chars: Vec<char> = s.chars().collect();
    if chars.len() != 4 {
        return None;
    }
    for (i, c) in chars.iter().enumerate() {
        out[i] = match c {
            'p' => TextLayer::Plate,
            'h' => TextLayer::Shadow,
            's' => TextLayer::Stroke,
            'f' => TextLayer::Fill,
            _ => return None,
        };
    }
    // каждый слой ровно один раз
    for l in [TextLayer::Plate, TextLayer::Shadow, TextLayer::Stroke, TextLayer::Fill] {
        if !out.contains(&l) {
            return None;
        }
    }
    Some(out)
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
        GeneratorKind::Text {
            text,
            family,
            bold,
            size,
            line_spacing,
            tracking,
            align,
            anchor,
            pos_x,
            pos_y,
            fill_r,
            fill_g,
            fill_b,
            fill_a,
            stroke_w,
            stroke_r,
            stroke_g,
            stroke_b,
            stroke_a,
            shadow_dx,
            shadow_dy,
            shadow_blur,
            shadow_r,
            shadow_g,
            shadow_b,
            shadow_a,
            plate_pad_x,
            plate_pad_y,
            plate_r,
            plate_g,
            plate_b,
            plate_a,
            order,
        } => {
            let n = || val.parse::<f64>().ok().filter(|v| v.is_finite());
            match key {
                "t" => *text = unescape_gen_value(val)?,
                "f" => *family = unescape_gen_value(val)?,
                "b" => *bold = val == "1",
                "s" => *size = n()?,
                "ls" => *line_spacing = n()?,
                "tr" => *tracking = n()?,
                "al" => *align = align_from(val)?,
                "an" => *anchor = anchor_from(val)?,
                "px" => *pos_x = n()?,
                "py" => *pos_y = n()?,
                "c" => (*fill_r, *fill_g, *fill_b, *fill_a) = parse_rgba(val)?,
                "sw" => *stroke_w = n()?,
                "sc" => (*stroke_r, *stroke_g, *stroke_b, *stroke_a) = parse_rgba(val)?,
                "sdx" => *shadow_dx = n()?,
                "sdy" => *shadow_dy = n()?,
                "sb" => *shadow_blur = n()?,
                "shc" => (*shadow_r, *shadow_g, *shadow_b, *shadow_a) = parse_rgba(val)?,
                "ppx" => *plate_pad_x = n()?,
                "ppy" => *plate_pad_y = n()?,
                "plc" => (*plate_r, *plate_g, *plate_b, *plate_a) = parse_rgba(val)?,
                "or" => *order = order_from(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Gradient { angle_deg, a_r, a_g, a_b, b_r, b_g, b_b } => {
            match key {
                "ang" => *angle_deg = val.parse::<f64>().ok().filter(|v| v.is_finite())?,
                "a" => (*a_r, *a_g, *a_b) = parse_rgb(val)?,
                "b" => (*b_r, *b_g, *b_b) = parse_rgb(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Corners {
            tl_r,
            tl_g,
            tl_b,
            tr_r,
            tr_g,
            tr_b,
            bl_r,
            bl_g,
            bl_b,
            br_r,
            br_g,
            br_b,
        } => {
            match key {
                "tl" => (*tl_r, *tl_g, *tl_b) = parse_rgb(val)?,
                "tr" => (*tr_r, *tr_g, *tr_b) = parse_rgb(val)?,
                "bl" => (*bl_r, *bl_g, *bl_b) = parse_rgb(val)?,
                "br" => (*br_r, *br_g, *br_b) = parse_rgb(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Checker { cols, a_r, a_g, a_b, b_r, b_g, b_b } => {
            match key {
                "n" => *cols = val.parse::<u32>().ok().filter(|v| (1..=256).contains(v))?,
                "a" => (*a_r, *a_g, *a_b) = parse_rgb(val)?,
                "b" => (*b_r, *b_g, *b_b) = parse_rgb(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::NoiseField { amount, mono } => {
            match key {
                "amt" => *amount = val.parse::<f64>().ok().filter(|v| v.is_finite())?,
                "mono" => *mono = val != "0",
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::SmpteBars { tone, tone_db } => {
            match key {
                "tone" => *tone = val == "1",
                "db" => *tone_db = val.parse::<f64>().ok().filter(|v| v.is_finite())?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Timecode { fps } => {
            match key {
                "fps" => *fps = val.parse::<u32>().ok().filter(|v| (1..=120).contains(v))?,
                _ => return None,
            }
            Some(())
        }
    }
}

/// `rrggbbaa` (8 hex-цифр) → четвёрка компонент.
fn parse_rgba(s: &str) -> Option<(u8, u8, u8, u8)> {
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some((c(0)?, c(2)?, c(4)?, c(6)?))
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

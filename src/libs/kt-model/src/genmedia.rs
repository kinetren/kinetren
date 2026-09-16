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
    FlagSpec, HasParams, ParamKind, ParamScope, ParamSpec, ParamUnit, flags_for, fx_cast,
    params_for,
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
        /// Анимация титра (Р.53, пп.423-424). Времени в kt-text нет:
        /// kt-media считает из pts материала ДВА числа — долю раскрытия
        /// и смещение блока, — и передаёт их растеризатору.
        #[serde(default)]
        anim: TextAnim,
        /// Длительность раскрытия или полного прохода, СЕКУНДЫ.
        #[serde(default = "default_anim_secs")]
        anim_secs: f64,
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
    // --- Р.53, пп.419-422: фигуры, подложка, отсчёт, таблицы ----------
    /// ПРОЗРАЧНАЯ подложка (п.420): кадр из нулей. Держатель места на
    /// дорожке и заготовка матта. Отдельный вид, а не «Solid с альфой»:
    /// полупрозрачная заливка уже выражается уровнем события, и второй
    /// выключатель того же смысла запрещён (правило UI 9).
    Transparent,
    /// Прямоугольник со скруглением (п.419). Размеры — в долях СВОЕЙ
    /// оси, поэтому подложка переживает смену формата проекта;
    /// изотропные длины (скругление, растушёвка) — в долях ВЫСОТЫ.
    Rect {
        #[serde(default = "default_half")]
        cx: f64,
        #[serde(default = "default_half")]
        cy: f64,
        #[serde(default = "default_shape_w")]
        w: f64,
        #[serde(default = "default_shape_h")]
        h: f64,
        #[serde(default)]
        radius: f64,
        #[serde(default = "default_feather")]
        feather: f64,
        #[serde(default = "default_255")]
        color_r: u8,
        #[serde(default = "default_255")]
        color_g: u8,
        #[serde(default = "default_255")]
        color_b: u8,
        #[serde(default = "default_255")]
        color_a: u8,
    },
    /// Эллипс (п.419). Отдельный вид от прямоугольника: у него нет
    /// скругления, и объединение дало бы мёртвый контрол (правило UI 9).
    Ellipse {
        #[serde(default = "default_half")]
        cx: f64,
        #[serde(default = "default_half")]
        cy: f64,
        #[serde(default = "default_shape_w")]
        w: f64,
        #[serde(default = "default_shape_h")]
        h: f64,
        #[serde(default = "default_feather")]
        feather: f64,
        #[serde(default = "default_255")]
        color_r: u8,
        #[serde(default = "default_255")]
        color_g: u8,
        #[serde(default = "default_255")]
        color_b: u8,
        #[serde(default = "default_255")]
        color_a: u8,
    },
    /// Отрезок (п.419): концы, а не центр с размерами — своя пара
    /// параметров, значит свой вид.
    Line {
        #[serde(default = "default_line_x1")]
        x1: f64,
        #[serde(default = "default_half")]
        y1: f64,
        #[serde(default = "default_line_x2")]
        x2: f64,
        #[serde(default = "default_half")]
        y2: f64,
        #[serde(default = "default_thickness")]
        thickness: f64,
        #[serde(default = "default_feather")]
        feather: f64,
        #[serde(default = "default_255")]
        color_r: u8,
        #[serde(default = "default_255")]
        color_g: u8,
        #[serde(default = "default_255")]
        color_b: u8,
        #[serde(default = "default_255")]
        color_a: u8,
    },
    /// Обратный отсчёт (п.421): крест, окружность, бегущий сектор и
    /// цифра. Время — pts МАТЕРИАЛА, как у тайм-кода. «2-pop» длиной
    /// ровно `1/fps` и биип 1 кГц тем же кодом, что тон SMPTE.
    Leader {
        #[serde(default = "default_leader_secs")]
        seconds: u32,
        #[serde(default = "default_tc_fps")]
        fps: u32,
        #[serde(default)]
        beep: bool,
        #[serde(default = "default_tone_db")]
        beep_db: f64,
    },
    /// Инженерная таблица (п.422). Своих ЧИСЕЛ у таблиц нет, поэтому вид
    /// ОДИН, а разновидность — поле: пять вариантов enum с пустым
    /// набором параметров были бы пятью копиями одного кода.
    #[serde(rename = "table")]
    TestPattern {
        #[serde(default)]
        table: TestTable,
    },
    /// Окно уровня (п.422): белое окно заданной ДОЛИ ПЛОЩАДИ кадра на
    /// чёрном — измерение APL и разогрева панели. Числа свои, значит вид
    /// свой.
    #[serde(rename = "window")]
    WindowPattern {
        #[serde(default = "default_window_area")]
        area: f64,
        #[serde(default = "default_one")]
        level: f64,
    },
}

/// Анимация титра (Р.53, пп.423-424).
///
/// Две ветви и они РАЗНЫЕ по существу: раскрытие меняет ВИДИМОСТЬ
/// кластеров, движение — СМЕЩЕНИЕ всего блока. Смешивать их в одно поле
/// «скорость» нельзя: у раскрытия конец есть, у движения он за кадром.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAnim {
    #[default]
    None,
    /// Печатная машинка: кластеры появляются по одному, жёстким срезом.
    Typewriter,
    /// Мягкое проявление: у фронта ширина в один кластер.
    Fade,
    /// Бегущая строка: блок едет справа налево, один полный проход.
    Crawl,
    /// Ползок финальных титров: снизу вверх.
    Roll,
}

impl TextAnim {
    pub const ALL: [TextAnim; 5] =
        [TextAnim::None, TextAnim::Typewriter, TextAnim::Fade, TextAnim::Crawl, TextAnim::Roll];

    pub fn token(self) -> &'static str {
        match self {
            TextAnim::None => "no",
            TextAnim::Typewriter => "tw",
            TextAnim::Fade => "fd",
            TextAnim::Crawl => "cr",
            TextAnim::Roll => "rl",
        }
    }

    pub fn from_token(s: &str) -> Option<TextAnim> {
        TextAnim::ALL.into_iter().find(|a| a.token() == s)
    }

    /// Зависит ли КАДР от времени. Ровно по этому признаку у титра
    /// отключается кэш по Quality (`GeneratorSource`): статические титры
    /// от появления анимации не страдают.
    pub fn animated(self) -> bool {
        self != TextAnim::None
    }
}

/// Разновидность инженерной таблицы (Р.53, п.422).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestTable {
    /// Цветные полосы 75 % (восемь, включая чёрную).
    #[default]
    Ebu75,
    /// Они же на полной амплитуде.
    Ebu100,
    /// Непрерывная горизонтальная шкала серого 0..255.
    Grayscale,
    /// Десять ступеней серого.
    Steps10,
    /// Плоскость цветности BT.709: Cb по X, Cr по Y при постоянной Y.
    RampYcbcr,
}

impl TestTable {
    pub const ALL: [TestTable; 5] = [
        TestTable::Ebu75,
        TestTable::Ebu100,
        TestTable::Grayscale,
        TestTable::Steps10,
        TestTable::RampYcbcr,
    ];

    pub fn token(self) -> &'static str {
        match self {
            TestTable::Ebu75 => "ebu75",
            TestTable::Ebu100 => "ebu100",
            TestTable::Grayscale => "grey",
            TestTable::Steps10 => "steps",
            TestTable::RampYcbcr => "ycbcr",
        }
    }

    pub fn from_token(s: &str) -> Option<TestTable> {
        TestTable::ALL.into_iter().find(|t| t.token() == s)
    }
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
fn default_anim_secs() -> f64 {
    1.0
}
fn default_shape_w() -> f64 {
    0.5
}
fn default_shape_h() -> f64 {
    0.25
}
/// Растушёвка края по умолчанию — полпроцента высоты: без неё
/// аналитический край выглядит «лесенкой» на крупном кадре.
fn default_feather() -> f64 {
    0.005
}
fn default_thickness() -> f64 {
    0.01
}
fn default_line_x1() -> f64 {
    0.2
}
fn default_line_x2() -> f64 {
    0.8
}
fn default_leader_secs() -> u32 {
    8
}
/// Доля площади кадра у окна уровня: классические 10 % APL.
fn default_window_area() -> f64 {
    0.1
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
            GeneratorKind::Checker {
                cols: 8,
                a_r: 0,
                a_g: 0,
                a_b: 0,
                b_r: 255,
                b_g: 255,
                b_b: 255,
            },
            GeneratorKind::NoiseField { amount: 1.0, mono: true },
            GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 },
            GeneratorKind::Timecode { fps: 30 },
            // Р.53: новые виды — В КОНЕЦ реестра. Инженерные таблицы
            // стоят ПЯТЬЮ строками одного вида: меню показывает каждую
            // разновидность, кода у них один.
            GeneratorKind::Transparent,
            GeneratorKind::Rect {
                cx: 0.5,
                cy: 0.5,
                w: default_shape_w(),
                h: default_shape_h(),
                radius: 0.0,
                feather: default_feather(),
                color_r: 255,
                color_g: 255,
                color_b: 255,
                color_a: 255,
            },
            GeneratorKind::Ellipse {
                cx: 0.5,
                cy: 0.5,
                w: default_shape_w(),
                h: default_shape_h(),
                feather: default_feather(),
                color_r: 255,
                color_g: 255,
                color_b: 255,
                color_a: 255,
            },
            GeneratorKind::Line {
                x1: default_line_x1(),
                y1: 0.5,
                x2: default_line_x2(),
                y2: 0.5,
                thickness: default_thickness(),
                feather: default_feather(),
                color_r: 255,
                color_g: 255,
                color_b: 255,
                color_a: 255,
            },
            GeneratorKind::Leader {
                seconds: default_leader_secs(),
                fps: default_tc_fps(),
                beep: false,
                beep_db: default_tone_db(),
            },
            GeneratorKind::TestPattern { table: TestTable::Ebu75 },
            GeneratorKind::TestPattern { table: TestTable::Ebu100 },
            GeneratorKind::TestPattern { table: TestTable::Grayscale },
            GeneratorKind::TestPattern { table: TestTable::Steps10 },
            GeneratorKind::TestPattern { table: TestTable::RampYcbcr },
            GeneratorKind::WindowPattern { area: default_window_area(), level: 1.0 },
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
            anim: TextAnim::None,
            anim_secs: default_anim_secs(),
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
            GeneratorKind::Transparent => "transparent",
            GeneratorKind::Rect { .. } => "rect",
            GeneratorKind::Ellipse { .. } => "ellipse",
            GeneratorKind::Line { .. } => "line",
            GeneratorKind::Leader { .. } => "leader",
            GeneratorKind::TestPattern { .. } => "table",
            GeneratorKind::WindowPattern { .. } => "window",
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
    // всё, что в ДОЛЯХ КАДРА, показывается процентами (Р.53, п.415):
    // хранение не меняется, читаемость меняется целиком. `line_spacing`
    // и `plate_pad_*` — доли СВОЕЙ высоты текста, это множители.
    Text {
        size: f64, 0.005, 0.5, 0.07, 0.001, Percent, Smooth;
        line_spacing: f64, 0.5, 3.0, 1.0, 0.05, Ratio, Smooth;
        tracking: f64, -0.05, 0.5, 0.0, 0.005, Percent, Smooth;
        pos_x: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        pos_y: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        stroke_w: f64, 0.0, 0.25, 0.0, 0.005, Percent, Smooth;
        shadow_dx: f64, -0.5, 0.5, 0.04, 0.005, Percent, Smooth;
        shadow_dy: f64, -0.5, 0.5, 0.04, 0.005, Percent, Smooth;
        shadow_blur: f64, 0.0, 0.5, 0.06, 0.005, Percent, Smooth;
        plate_pad_x: f64, 0.0, 2.0, 0.25, 0.01, Ratio, Smooth;
        plate_pad_y: f64, 0.0, 2.0, 0.12, 0.01, Ratio, Smooth;
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
    // Р.53 (п.419): доли СВОЕЙ оси у размеров и позиций, доли ВЫСОТЫ у
    // изотропных длин — подложка переживает смену формата проекта
    Rect {
        cx: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        cy: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        w: f64, 0.0, 2.0, 0.5, 0.001, Percent, Smooth;
        h: f64, 0.0, 2.0, 0.25, 0.001, Percent, Smooth;
        radius: f64, 0.0, 1.0, 0.0, 0.001, Percent, Smooth;
        feather: f64, 0.0, 0.5, 0.005, 0.001, Percent, Smooth;
        color_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_a: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Ellipse {
        cx: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        cy: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        w: f64, 0.0, 2.0, 0.5, 0.001, Percent, Smooth;
        h: f64, 0.0, 2.0, 0.25, 0.001, Percent, Smooth;
        feather: f64, 0.0, 0.5, 0.005, 0.001, Percent, Smooth;
        color_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_a: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Line {
        x1: f64, 0.0, 1.0, 0.2, 0.001, Percent, Smooth;
        y1: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        x2: f64, 0.0, 1.0, 0.8, 0.001, Percent, Smooth;
        y2: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        thickness: f64, 0.0, 0.5, 0.01, 0.001, Percent, Smooth;
        feather: f64, 0.0, 0.5, 0.005, 0.001, Percent, Smooth;
        color_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_a: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Leader {
        seconds: u32, 1.0, 60.0, 8.0, 1.0, Count, Structural;
        fps: u32, 1.0, 120.0, 30.0, 1.0, Count, Structural;
        beep_db: f64, -60.0, 0.0, -20.0, 0.5, Db, Smooth;
    },
    WindowPattern {
        area: f64, 0.0, 1.0, 0.1, 0.005, Percent, Smooth;
        level: f64, 0.0, 1.0, 1.0, 0.005, Percent, Smooth;
    },
}}

flags_for! { GeneratorKind, gen_flag_specs, gen_set_flag, gen_get_flag, {
    // «да/нет» ползунком не тянется и кривой не автоматизируется, но и
    // забытым быть не должно: тон 1 кГц и цветной шум существовали в
    // формате с Р.52, а включить их было нечем (Р.53, п.414).
    NoiseField { mono: true },
    SmpteBars { tone: false },
    Text { bold: false },
    Leader { beep: false },
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
            anim,
            anim_secs,
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
            let rgba = |r: &u8, g: &u8, b: &u8, a: &u8| format!("{r:02x}{g:02x}{b:02x}{a:02x}");
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
            if *anim != TextAnim::default() {
                push("am", anim.token().into());
            }
            if *anim_secs != default_anim_secs() {
                push("ams", f(anim_secs));
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
        // --- Р.53 (пп.419-422) -------------------------------------------
        GeneratorKind::Transparent => {}
        GeneratorKind::Rect {
            cx,
            cy,
            w,
            h,
            radius,
            feather,
            color_r,
            color_g,
            color_b,
            color_a,
        } => {
            let f = |v: &f64| format!("{v}");
            if *cx != 0.5 {
                push("cx", f(cx));
            }
            if *cy != 0.5 {
                push("cy", f(cy));
            }
            if *w != default_shape_w() {
                push("w", f(w));
            }
            if *h != default_shape_h() {
                push("h", f(h));
            }
            if *radius != 0.0 {
                push("rad", f(radius));
            }
            if *feather != default_feather() {
                push("fe", f(feather));
            }
            if (*color_r, *color_g, *color_b, *color_a) != (255, 255, 255, 255) {
                push("c", format!("{color_r:02x}{color_g:02x}{color_b:02x}{color_a:02x}"));
            }
        }
        GeneratorKind::Ellipse { cx, cy, w, h, feather, color_r, color_g, color_b, color_a } => {
            let f = |v: &f64| format!("{v}");
            if *cx != 0.5 {
                push("cx", f(cx));
            }
            if *cy != 0.5 {
                push("cy", f(cy));
            }
            if *w != default_shape_w() {
                push("w", f(w));
            }
            if *h != default_shape_h() {
                push("h", f(h));
            }
            if *feather != default_feather() {
                push("fe", f(feather));
            }
            if (*color_r, *color_g, *color_b, *color_a) != (255, 255, 255, 255) {
                push("c", format!("{color_r:02x}{color_g:02x}{color_b:02x}{color_a:02x}"));
            }
        }
        GeneratorKind::Line {
            x1,
            y1,
            x2,
            y2,
            thickness,
            feather,
            color_r,
            color_g,
            color_b,
            color_a,
        } => {
            let f = |v: &f64| format!("{v}");
            if *x1 != default_line_x1() {
                push("x1", f(x1));
            }
            if *y1 != 0.5 {
                push("y1", f(y1));
            }
            if *x2 != default_line_x2() {
                push("x2", f(x2));
            }
            if *y2 != 0.5 {
                push("y2", f(y2));
            }
            if *thickness != default_thickness() {
                push("th", f(thickness));
            }
            if *feather != default_feather() {
                push("fe", f(feather));
            }
            if (*color_r, *color_g, *color_b, *color_a) != (255, 255, 255, 255) {
                push("c", format!("{color_r:02x}{color_g:02x}{color_b:02x}{color_a:02x}"));
            }
        }
        GeneratorKind::Leader { seconds, fps, beep, beep_db } => {
            if *seconds != default_leader_secs() {
                push("n", format!("{seconds}"));
            }
            if *fps != default_tc_fps() {
                push("fps", format!("{fps}"));
            }
            if *beep {
                push("beep", "1".into());
            }
            if *beep_db != default_tone_db() {
                push("db", format!("{beep_db}"));
            }
        }
        GeneratorKind::TestPattern { table } => {
            if *table != TestTable::default() {
                push("tb", table.token().into());
            }
        }
        GeneratorKind::WindowPattern { area, level } => {
            if *area != default_window_area() {
                push("ar", format!("{area}"));
            }
            if *level != 1.0 {
                push("lv", format!("{level}"));
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
            anim,
            anim_secs,
        } => {
            let n = || val.parse::<f64>().ok().filter(|v| v.is_finite());
            match key {
                "am" => *anim = TextAnim::from_token(val)?,
                "ams" => {
                    *anim_secs = val.parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0)?
                }
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
        // --- Р.53 (пп.419-422) -------------------------------------------
        // у подложки параметров нет: любой токен — битый путь
        GeneratorKind::Transparent => None,
        GeneratorKind::Rect {
            cx,
            cy,
            w,
            h,
            radius,
            feather,
            color_r,
            color_g,
            color_b,
            color_a,
        } => {
            match key {
                "cx" => *cx = unit(val)?,
                "cy" => *cy = unit(val)?,
                "w" => *w = span(val)?,
                "h" => *h = span(val)?,
                "rad" => *radius = unit(val)?,
                "fe" => *feather = unit(val)?,
                "c" => (*color_r, *color_g, *color_b, *color_a) = parse_rgba(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Ellipse { cx, cy, w, h, feather, color_r, color_g, color_b, color_a } => {
            match key {
                "cx" => *cx = unit(val)?,
                "cy" => *cy = unit(val)?,
                "w" => *w = span(val)?,
                "h" => *h = span(val)?,
                "fe" => *feather = unit(val)?,
                "c" => (*color_r, *color_g, *color_b, *color_a) = parse_rgba(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Line {
            x1,
            y1,
            x2,
            y2,
            thickness,
            feather,
            color_r,
            color_g,
            color_b,
            color_a,
        } => {
            match key {
                "x1" => *x1 = unit(val)?,
                "y1" => *y1 = unit(val)?,
                "x2" => *x2 = unit(val)?,
                "y2" => *y2 = unit(val)?,
                "th" => *thickness = unit(val)?,
                "fe" => *feather = unit(val)?,
                "c" => (*color_r, *color_g, *color_b, *color_a) = parse_rgba(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Leader { seconds, fps, beep, beep_db } => {
            match key {
                "n" => *seconds = val.parse::<u32>().ok().filter(|v| (1..=60).contains(v))?,
                "fps" => *fps = val.parse::<u32>().ok().filter(|v| (1..=120).contains(v))?,
                "beep" => *beep = val == "1",
                "db" => *beep_db = val.parse::<f64>().ok().filter(|v| v.is_finite())?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::TestPattern { table } => {
            match key {
                "tb" => *table = TestTable::from_token(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::WindowPattern { area, level } => {
            match key {
                "ar" => *area = unit(val)?,
                "lv" => *level = unit(val)?,
                _ => return None,
            }
            Some(())
        }
    }
}

/// Доля 0..1 из токена. Вне диапазона или не число — битый путь, а не
/// молчаливый кламп (правило разбора: ошибка, а не догадка).
fn unit(val: &str) -> Option<f64> {
    val.parse::<f64>().ok().filter(|v| v.is_finite() && (0.0..=1.0).contains(v))
}

/// Размер фигуры: до ДВУХ долей оси — фигура законно вылезает за кадр
/// (подложка под титр шире кадра — обычное дело).
fn span(val: &str) -> Option<f64> {
    val.parse::<f64>().ok().filter(|v| v.is_finite() && (0.0..=2.0).contains(v))
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

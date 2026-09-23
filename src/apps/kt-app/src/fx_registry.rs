//! Единый реестр FX + popup-пикер с поиском (п.3 v1.4): одно место выбора
//! эффектов для всех четырёх точек добавления (FX-редактор клипа ×2 вида,
//! morpher FX-батч ×2 цепочки), масштабируемое на сотни эффектов —
//! строка поиска (имя + keywords, без регистра) и группировка по
//! категориям. Шаблоны — те же, что были в кнопках/комбо (тест 48).

use crate::fx_editor::video_fx_templates;
use crate::i18n::{Key, k, tr, tr_en};
use eframe::egui;
use kt_model::FxInstance;

pub struct FxEntry {
    /// Имя эффекта НЕ переводится: это отраслевой термин, по нему эффект ищут
    /// в любой локали (и он же сверяется с `fx_display_name`, тест 48).
    pub name: &'static str,
    pub category: Key,
    /// В каком пикере предлагать. Обычно ровно одно из двух; двусторонним
    /// бывает ТОЛЬКО варп времени (Р.37): он не пиксельный фильтр, а
    /// правило выбора исходного кадра, и осмыслен на обоих видах.
    pub audio: bool,
    pub video: bool,
    /// Ключевые слова поиска. Ключ каталога, а не текст: реестр от языка не
    /// зависит и `OnceLock` не пересобирается при переключении.
    pub keywords: Option<Key>,
    pub template: FxInstance,
    /// Встроенные ВАРИАНТЫ (Р.54). У аудио-записей пусты: миниатюру звука
    /// не нарисовать, а полоска без картинок — это второй список имён.
    pub variants: Vec<FxVariant>,
}

/// Встроенный вариант эффекта (Р.54): тот же вид, приведённый в
/// осмысленное состояние, с именем по ВИДУ, а не по параметру.
///
/// Вариант — это ровно `FxInstance`, а не отдельная сущность: клик по его
/// миниатюре идёт тем же путём `Picked::Fx`, что и клик по имени эффекта,
/// поэтому ни новой команды, ни новой сущности таймлайна не появляется
/// (t252/t253). Библиотека пользовательских пресетов (Р.48) — другое:
/// там блок ЦЕПОЧКИ, он не принадлежит виду эффекта и живёт в файле.
///
/// Имя варианта ПЕРЕВОДИТСЯ, в отличие от имени эффекта: «Тушь», «Чертёж»,
/// «Крупно» отраслевыми терминами не являются. Двойная политика в одном
/// списке — осознанная цена, разобранная в F-HLP-031 («Минусы»).
pub struct FxVariant {
    pub name: Key,
    pub fx: FxInstance,
}

/// Категория и keywords видео-эффекта по имени из video_fx_templates.
fn video_meta(name: &str) -> (Key, Option<Key>) {
    // Полными путями намеренно: тест 125 ищет ключ каталога по пути `k::…`
    // и сокращённый `use` спрятал бы использование.
    match name {
        "Geometry" => (k::fxreg::cat::MOTION, Some(k::fxreg::kw::GEOMETRY)),
        "Mirror" => (k::fxreg::cat::GEOMETRY, Some(k::fxreg::kw::MIRROR)),
        "Blur" => (k::fxreg::cat::BLUR, Some(k::fxreg::kw::BLUR)),
        "Greenscreen" => (k::fxreg::cat::KEY, Some(k::fxreg::kw::GREENSCREEN)),
        "Edges" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::EDGES)),
        "Noise" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::NOISE)),
        "VHS" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::VHS)),
        "Glow" => (k::fxreg::cat::GLOW, Some(k::fxreg::kw::GLOW)),
        "EventGrid" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::EVENTGRID)),
        "Pan 3D" => (k::fxreg::cat::THREE_D, Some(k::fxreg::kw::PAN3D)),
        // Р.47 Кв2: цвет/стиль/перспектива
        "Color Grade" => (k::fxreg::cat::COLOR, Some(k::fxreg::kw::COLOR_GRADE)),
        "Duotone" => (k::fxreg::cat::COLOR, Some(k::fxreg::kw::DUOTONE)),
        "Posterize" => (k::fxreg::cat::COLOR, Some(k::fxreg::kw::POSTERIZE)),
        "Vignette" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::VIGNETTE)),
        "Corner Pin" => (k::fxreg::cat::GEOMETRY, Some(k::fxreg::kw::CORNER_PIN)),
        // Р.47 Кв3: барьеры
        "Sharpen" => (k::fxreg::cat::BLUR, Some(k::fxreg::kw::SHARPEN)),
        "Pixelate" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::PIXELATE)),
        "Chromatic Aberration" => {
            (k::fxreg::cat::DISTORT, Some(k::fxreg::kw::CHROMATIC_ABERRATION))
        }
        "Radial Blur" => (k::fxreg::cat::BLUR, Some(k::fxreg::kw::RADIAL_BLUR)),
        "Drop Shadow" => (k::fxreg::cat::KEY, Some(k::fxreg::kw::DROP_SHADOW)),
        "Halftone" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::HALFTONE)),
        // Р.47 Кв4: Warp-семейство и цилиндр
        "Lens Distort" => (k::fxreg::cat::DISTORT, Some(k::fxreg::kw::LENS_DISTORT)),
        "Twirl" => (k::fxreg::cat::DISTORT, Some(k::fxreg::kw::TWIRL)),
        "Ripple" => (k::fxreg::cat::DISTORT, Some(k::fxreg::kw::RIPPLE)),
        "Kaleidoscope" => (k::fxreg::cat::DISTORT, Some(k::fxreg::kw::KALEIDOSCOPE)),
        "Cylinder 3D" => (k::fxreg::cat::THREE_D, Some(k::fxreg::kw::CYLINDER3D)),
        _ => (k::fxreg::cat::VIDEO, None),
    }
}

/// Вариант из шаблона: значения ставятся ПО ИМЕНИ через `set_param`, а не
/// литералом структуры. Так новый параметр эффекта не трогает полсотни
/// мест (правило kt-fx), а значение вне спеки невозможно по построению —
/// `set_param` клампит диапазоном (t543 сверяет это отдельно).
fn var(tmpl: &FxInstance, name: Key, sets: &[(&str, f64)]) -> FxVariant {
    let mut fx = tmpl.clone();
    for (p, v) in sets {
        // опечатка в имени параметра иначе прошла бы молча: set_param
        // вернул бы false, и вариант оказался бы копией шаблона
        debug_assert!(fx.param_spec(p).is_some(), "{}: no such param {p}", fx.tag());
        fx.set_param(p, *v);
    }
    FxVariant { name, fx }
}

/// Встроенные варианты видео-эффекта (Р.54).
///
/// Порядок в полоске — от слабого к сильному: первая ячейка обязана быть
/// НЕ тождеством, потому что у пяти видов (Pan, Pan 3D, Color Grade,
/// Corner Pin, Drop Shadow) сам шаблон — тождество, и его миниатюра
/// показывает исходный кадр. Именно первый вариант и служит им иконкой
/// (t543 держит это условие).
///
/// Полными путями к ключам намеренно — та же причина, что у `video_meta`:
/// t125 ищет ключ каталога по пути, сокращённый `use` спрятал бы его.
fn video_variants(name: &str, tmpl: &FxInstance) -> Vec<FxVariant> {
    match name {
        // Р.62: дополнительный узел «Геометрия» — три варианта (тождественный
        // дефолт иконкой служить не может, t543)
        "Geometry" => vec![
            var(tmpl, k::fxreg::var::CLOSEUP, &[("zoom", 1.25)]),
            var(tmpl, k::fxreg::var::PUNCH, &[("zoom", 2.0)]),
            var(tmpl, k::fxreg::var::OFFCENTER, &[("zoom", 1.4), ("pos_x", -0.12)]),
        ],
        // у зеркала числовых параметров нет вовсе — только две галки,
        // поэтому вариантов ровно два: третье состояние и есть шаблон
        "Mirror" => vec![
            FxVariant {
                name: k::fxreg::var::VERTICAL,
                fx: FxInstance::Mirror { horizontal: false, vertical: true },
            },
            FxVariant {
                name: k::fxreg::var::BOTH,
                fx: FxInstance::Mirror { horizontal: true, vertical: true },
            },
        ],
        "Blur" => vec![
            var(tmpl, k::fxreg::var::SOFT, &[("radius", 3.0)]),
            var(tmpl, k::fxreg::var::HEAVY, &[("radius", 24.0)]),
            // смаз по одной оси: radius_y = 1 (не 0 — ноль значит «как X»)
            var(tmpl, k::fxreg::var::HORIZONTAL, &[("radius", 28.0), ("radius_y", 1.0)]),
        ],
        "Greenscreen" => vec![
            var(tmpl, k::fxreg::var::TIGHT, &[("tolerance", 40.0), ("softness", 10.0)]),
            var(tmpl, k::fxreg::var::WIDE, &[("tolerance", 120.0), ("softness", 45.0)]),
            var(tmpl, k::fxreg::var::BLUE, &[("key_g", 0.0), ("key_b", 255.0)]),
        ],
        "Edges" => vec![
            var(
                tmpl,
                k::fxreg::var::INK,
                &[("threshold", 90.0), ("color_r", 0.0), ("color_g", 0.0), ("color_b", 0.0)],
            ),
            var(
                tmpl,
                k::fxreg::var::NEON,
                &[
                    ("threshold", 36.0),
                    ("thickness", 2.0),
                    ("color_r", 0.0),
                    ("color_g", 255.0),
                    ("color_b", 160.0),
                ],
            ),
            var(
                tmpl,
                k::fxreg::var::BLUEPRINT,
                &[("threshold", 60.0), ("color_r", 80.0), ("color_g", 160.0), ("color_b", 255.0)],
            ),
        ],
        "Noise" => vec![
            var(tmpl, k::fxreg::var::FILM, &[("amount", 0.05), ("size", 2.0)]),
            var(tmpl, k::fxreg::var::HEAVY, &[("amount", 0.25)]),
            var(tmpl, k::fxreg::var::COARSE, &[("amount", 0.18), ("size", 8.0)]),
        ],
        "VHS" => vec![
            var(
                tmpl,
                k::fxreg::var::SUBTLE,
                &[("rgb_shift", 2.0), ("bleed", 0.2), ("scan_step", 3.0), ("jitter", 0.03)],
            ),
            var(tmpl, k::fxreg::var::CLASSIC, &[("bleed", 0.5), ("jitter", 0.12)]),
            var(
                tmpl,
                k::fxreg::var::TRASH,
                &[("rgb_shift", 10.0), ("bleed", 0.85), ("scan_step", 6.0), ("jitter", 0.45)],
            ),
        ],
        "Glow" => vec![
            var(
                tmpl,
                k::fxreg::var::SOFT,
                &[("strength", 0.4), ("radius", 8.0), ("threshold", 128.0)],
            ),
            var(
                tmpl,
                k::fxreg::var::DREAM,
                &[("strength", 1.1), ("radius", 16.0), ("threshold", 96.0)],
            ),
            var(
                tmpl,
                k::fxreg::var::GOLDEN,
                &[
                    ("strength", 0.9),
                    ("radius", 12.0),
                    ("threshold", 110.0),
                    ("tint_g", 200.0),
                    ("tint_b", 120.0),
                ],
            ),
        ],
        "Pan 3D" => vec![
            var(tmpl, k::fxreg::var::TILT, &[("rot_x", 15.0)]),
            var(tmpl, k::fxreg::var::TURN, &[("rot_y", -20.0)]),
            var(tmpl, k::fxreg::var::DEPTH, &[("pos_z", -200.0), ("rot_y", 10.0), ("scale", 0.85)]),
        ],
        "EventGrid" => vec![
            var(tmpl, k::fxreg::var::GRID9, &[("cols", 3.0), ("rows", 3.0)]),
            var(tmpl, k::fxreg::var::GRID16, &[("cols", 4.0), ("rows", 4.0)]),
            var(tmpl, k::fxreg::var::STRIP, &[("cols", 4.0), ("rows", 1.0)]),
        ],
        "Color Grade" => vec![
            var(
                tmpl,
                k::fxreg::var::WARM,
                &[("hue", 8.0), ("saturation", 1.15), ("brightness", 0.04)],
            ),
            var(
                tmpl,
                k::fxreg::var::COOL,
                &[("hue", -12.0), ("saturation", 0.9), ("contrast", 0.1)],
            ),
            var(tmpl, k::fxreg::var::BW, &[("saturation", 0.0), ("contrast", 0.15)]),
        ],
        "Duotone" => vec![
            var(
                tmpl,
                k::fxreg::var::SEPIA,
                &[
                    ("shadow_r", 48.0),
                    ("shadow_g", 28.0),
                    ("shadow_b", 12.0),
                    ("high_r", 255.0),
                    ("high_g", 226.0),
                    ("high_b", 180.0),
                ],
            ),
            var(
                tmpl,
                k::fxreg::var::BLUE,
                &[
                    ("shadow_r", 8.0),
                    ("shadow_g", 16.0),
                    ("shadow_b", 64.0),
                    ("high_r", 180.0),
                    ("high_g", 220.0),
                    ("high_b", 255.0),
                ],
            ),
            var(
                tmpl,
                k::fxreg::var::NEON,
                &[
                    ("shadow_r", 32.0),
                    ("shadow_g", 0.0),
                    ("shadow_b", 64.0),
                    ("high_r", 255.0),
                    ("high_g", 64.0),
                    ("high_b", 192.0),
                ],
            ),
        ],
        "Posterize" => vec![
            var(tmpl, k::fxreg::var::HARD, &[("levels", 3.0)]),
            var(tmpl, k::fxreg::var::COMIC, &[("levels", 4.0)]),
            var(tmpl, k::fxreg::var::SOFT, &[("levels", 12.0)]),
        ],
        "Vignette" => vec![
            var(
                tmpl,
                k::fxreg::var::SOFT,
                &[("strength", 0.3), ("radius", 0.9), ("softness", 0.6)],
            ),
            var(
                tmpl,
                k::fxreg::var::STRONG,
                &[("strength", 0.9), ("radius", 0.55), ("softness", 0.35)],
            ),
            var(
                tmpl,
                k::fxreg::var::TIGHT,
                &[("strength", 0.7), ("radius", 0.4), ("softness", 0.2)],
            ),
        ],
        "Corner Pin" => vec![
            // трапеция: верхняя кромка сужена и опущена
            var(
                tmpl,
                k::fxreg::var::PERSPECTIVE,
                &[("x0", 0.12), ("y0", 0.06), ("x1", 0.88), ("y1", 0.06)],
            ),
            // правый край короче левого
            var(tmpl, k::fxreg::var::TILT, &[("y1", 0.14), ("y3", 0.86)]),
            var(tmpl, k::fxreg::var::SKEW, &[("x0", 0.18), ("x3", 0.82)]),
        ],
        "Sharpen" => vec![
            var(tmpl, k::fxreg::var::SOFT, &[("amount", 0.5), ("radius", 1.0)]),
            var(tmpl, k::fxreg::var::STRONG, &[("amount", 2.0)]),
            var(tmpl, k::fxreg::var::HEAVY, &[("amount", 3.2), ("radius", 4.0)]),
        ],
        "Pixelate" => vec![
            var(tmpl, k::fxreg::var::FINE, &[("size", 5.0)]),
            var(tmpl, k::fxreg::var::COARSE, &[("size", 28.0)]),
            var(tmpl, k::fxreg::var::CENSOR, &[("size", 64.0)]),
        ],
        "Chromatic Aberration" => vec![
            var(tmpl, k::fxreg::var::SUBTLE, &[("amount", 0.004)]),
            var(tmpl, k::fxreg::var::STRONG, &[("amount", 0.028)]),
            // НЕ 0.05 (потолок спеки): поле f32, а кламп в `set_param`
            // считается в f64 — ровный потолок при записи в f32
            // округляется ВВЕРХ и значение оказывается ВНЕ спеки (нашёл
            // t543). Вариант — это вид, а не проверка предела
            var(tmpl, k::fxreg::var::GLITCH, &[("amount", 0.045)]),
        ],
        "Radial Blur" => {
            let mut v = vec![
                var(tmpl, k::fxreg::var::ZOOM, &[("amount", 0.5)]),
                var(tmpl, k::fxreg::var::HEAVY, &[("amount", 0.85)]),
            ];
            // вращение вместо наезда — единственный нечисловой параметр
            // среди всех вариантов: у `spin` спеки нет, он bool
            let mut spin = var(tmpl, k::fxreg::var::SPIN, &[("amount", 0.4)]);
            if let FxInstance::RadialBlur { spin: s, .. } = &mut spin.fx {
                *s = true;
            }
            v.insert(1, spin);
            v
        }
        "Drop Shadow" => vec![
            var(
                tmpl,
                k::fxreg::var::SOFT,
                &[("offset_x", 6.0), ("offset_y", 6.0), ("radius", 12.0), ("opacity", 0.45)],
            ),
            var(
                tmpl,
                k::fxreg::var::HARD,
                &[("offset_x", 4.0), ("offset_y", 4.0), ("radius", 1.0), ("opacity", 0.8)],
            ),
            var(
                tmpl,
                k::fxreg::var::FAR,
                &[("offset_x", 24.0), ("offset_y", 24.0), ("radius", 16.0), ("opacity", 0.5)],
            ),
        ],
        "Halftone" => vec![
            var(tmpl, k::fxreg::var::FINE, &[("size", 3.0), ("angle", 0.0)]),
            var(tmpl, k::fxreg::var::PRINT, &[("size", 5.0), ("angle", 15.0)]),
            var(tmpl, k::fxreg::var::COMIC, &[("size", 12.0)]),
        ],
        "Lens Distort" => vec![
            var(tmpl, k::fxreg::var::BARREL, &[("k1", 0.6)]),
            var(tmpl, k::fxreg::var::PINCUSHION, &[("k1", -0.45)]),
            var(tmpl, k::fxreg::var::FISHEYE, &[("k1", 0.9), ("k2", 0.3)]),
        ],
        "Twirl" => vec![
            var(tmpl, k::fxreg::var::SUBTLE, &[("angle", 45.0)]),
            var(tmpl, k::fxreg::var::STRONG, &[("angle", 360.0)]),
            var(tmpl, k::fxreg::var::REVERSE, &[("angle", -240.0)]),
        ],
        "Ripple" => vec![
            var(tmpl, k::fxreg::var::WATER, &[("amount", 0.05), ("freq", 18.0)]),
            var(tmpl, k::fxreg::var::RINGS, &[("amount", 0.15), ("freq", 6.0)]),
            var(tmpl, k::fxreg::var::STRONG, &[("amount", 0.3), ("freq", 12.0)]),
        ],
        "Kaleidoscope" => vec![
            var(tmpl, k::fxreg::var::MIRROR, &[("segments", 2.0)]),
            var(tmpl, k::fxreg::var::STAR, &[("segments", 10.0)]),
            var(tmpl, k::fxreg::var::MANDALA, &[("segments", 20.0), ("angle", 15.0)]),
        ],
        "Cylinder 3D" => vec![
            var(tmpl, k::fxreg::var::SUBTLE, &[("curvature", 0.2)]),
            var(tmpl, k::fxreg::var::STRONG, &[("curvature", 0.85)]),
            var(tmpl, k::fxreg::var::TUBE, &[("curvature", 1.0)]),
        ],
        // пустой список = забытая запись; t543 краснеет на нём так же,
        // как t48 на категории-фолбэке
        _ => Vec::new(),
    }
}

pub fn registry() -> &'static [FxEntry] {
    static REG: std::sync::OnceLock<Vec<FxEntry>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        let mut v = vec![
            FxEntry {
                name: "Gain",
                category: k::fxreg::cat::VOLUME,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::GAIN),
                template: FxInstance::gain(0.0),
            },
            FxEntry {
                name: "Slowed+Reverb",
                category: k::fxreg::cat::TIMESTRETCH,
                audio: true,
                // ДВУСТОРОННИЙ (Р.37 п.2): на видео-клипе он растягивает
                // время, то есть меняет выбор исходного кадра
                video: true,
                keywords: Some(k::fxreg::kw::SLOWEDREVERB),
                template: FxInstance::slowed_reverb(0.82, 0.1, 85.0),
                // вариантов нет и у него: на статичном кадре растяжение
                // времени невидимо, а полоска из одинаковых картинок —
                // ровно то «впустую занятое место», про которое правило 13
                variants: Vec::new(),
            },
            FxEntry {
                name: "Filter",
                category: k::fxreg::cat::FILTER,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::FILTER),
                template: FxInstance::filter(),
            },
            FxEntry {
                name: "EQ",
                category: k::fxreg::cat::FILTER,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::EQ),
                template: FxInstance::eq(),
            },
            FxEntry {
                name: "Distortion",
                category: k::fxreg::cat::DISTORT,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::DISTORTION),
                template: FxInstance::distortion(),
            },
            FxEntry {
                name: "Bitcrusher",
                category: k::fxreg::cat::DISTORT,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::BITCRUSHER),
                template: FxInstance::bitcrusher(),
            },
            FxEntry {
                name: "Tremolo",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::TREMOLO),
                template: FxInstance::tremolo(),
            },
            FxEntry {
                name: "AutoPan",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::AUTOPAN),
                template: FxInstance::auto_pan(),
            },
            FxEntry {
                name: "Vibrato",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::VIBRATO),
                template: FxInstance::vibrato(),
            },
            FxEntry {
                name: "Chorus",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::CHORUS),
                template: FxInstance::chorus(),
            },
            FxEntry {
                name: "Flanger",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::FLANGER),
                template: FxInstance::flanger(),
            },
            FxEntry {
                name: "Phaser",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::PHASER),
                template: FxInstance::phaser(),
            },
            FxEntry {
                name: "Delay",
                category: k::fxreg::cat::DELAY,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::DELAY),
                template: FxInstance::delay(),
            },
            FxEntry {
                name: "Compressor",
                category: k::fxreg::cat::DYNAMICS,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::COMPRESSOR),
                template: FxInstance::compressor(),
            },
            FxEntry {
                name: "Gate",
                category: k::fxreg::cat::DYNAMICS,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::GATE),
                template: FxInstance::gate(),
            },
            FxEntry {
                name: "Limiter",
                category: k::fxreg::cat::DYNAMICS,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::LIMITER),
                template: FxInstance::limiter(),
            },
            FxEntry {
                name: "Pitch Shift",
                category: k::fxreg::cat::TIMESTRETCH,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::PITCH_SHIFT),
                template: FxInstance::pitch_shift(),
            },
            FxEntry {
                name: "Stereo Width",
                category: k::fxreg::cat::STEREO,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::STEREOWIDTH),
                template: FxInstance::stereo_width(),
            },
        ];
        for (name, tmpl) in video_fx_templates() {
            let (category, keywords) = video_meta(name);
            let variants = video_variants(name, &tmpl);
            v.push(FxEntry {
                name,
                category,
                audio: false,
                video: true,
                keywords,
                template: tmpl,
                variants,
            });
        }
        v
    })
}

/// Фильтр реестра: вид (аудио/видео) + подстрока запроса в имени или
/// keywords, без регистра. Чистая функция — тест 48.
///
/// Ищет по ключевым словам ТЕКУЩЕГО языка И по английским: иначе выбравший
/// китайский не смог бы найти «blur», хотя имя эффекта именно такое.
/// Значения keywords лежат в каталоге уже в нижнем регистре.
///
/// С Р.54 совпадение по имени ВАРИАНТА тоже показывает строку («чертёж»
/// находит Edges). Матч — на уровне СТРОКИ, а не ячейки: сколько бы
/// вариантов ни совпало, строка остаётся одна, и счётчики фильтра не
/// зависят от числа вариантов (t48).
pub fn filter_entries<'a>(entries: &'a [FxEntry], audio: bool, query: &str) -> Vec<&'a FxEntry> {
    let q = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|e| if audio { e.audio } else { e.video })
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || e.keywords.is_some_and(|kw| tr(kw).contains(&q) || tr_en(kw).contains(&q))
                || e.variants.iter().any(|v| {
                    tr(v.name).to_lowercase().contains(&q)
                        || tr_en(v.name).to_lowercase().contains(&q)
                })
        })
        .collect()
}

/// Кнопка «+ эффект…» с popup: поиск + список по категориям.
/// Что выбрали в пикере.
pub enum Picked {
    /// Пресет из библиотеки пользователя (индекс в переданном срезе).
    Preset(usize),
    /// Управление пользовательским пресетом из его контекст-меню.
    PresetAction(crate::widgets::PresetAction),
    /// Приколотая голова полосы (у морфера — подмена источника).
    Head,
    Fx(FxInstance),
}

/// Ширина галереи (Р.54). Фиксирована галлеей самого длинного варианта
/// содержимого: правило 3 чек-листа — ничего не скачет при наборе в
/// строке поиска и при смене числа ячеек.
pub(crate) const GALLERY_W: f32 = 520.0;
/// Высота прокрутки списка эффектов и секции пресетов.
const GALLERY_H: f32 = 450.0;
const PRESETS_H: f32 = 160.0;
/// Рисуемый размер ячейки (хранится вдвое крупнее — hidpi).
pub(crate) const CELL_DRAW_W: f32 = 80.0;
const CELL_DRAW_H: f32 = 45.0;

/// Значок категории для строки эффекта. Исчерпывающе по категориям
/// реестра: новая категория без значка обязана краснеть в t543b, а не
/// молча рисоваться пустотой.
fn cat_glyph(cat: Key) -> &'static str {
    // Полными путями к ключам — та же причина, что у `video_meta`.
    match cat {
        c if c == k::fxreg::cat::MOTION => crate::icons::CAT_MOTION,
        c if c == k::fxreg::cat::GEOMETRY => crate::icons::CAT_GEOMETRY,
        c if c == k::fxreg::cat::BLUR => crate::icons::CAT_BLUR,
        c if c == k::fxreg::cat::KEY => crate::icons::CAT_KEY,
        c if c == k::fxreg::cat::STYLE => crate::icons::CAT_STYLE,
        c if c == k::fxreg::cat::GLOW => crate::icons::CAT_GLOW,
        c if c == k::fxreg::cat::THREE_D => crate::icons::CAT_THREE_D,
        c if c == k::fxreg::cat::COLOR => crate::icons::CAT_COLOR,
        c if c == k::fxreg::cat::DISTORT => crate::icons::CAT_DISTORT,
        c if c == k::fxreg::cat::VOLUME => crate::icons::CAT_VOLUME,
        c if c == k::fxreg::cat::FILTER => crate::icons::CAT_FILTER,
        c if c == k::fxreg::cat::STEREO => crate::icons::CAT_STEREO,
        c if c == k::fxreg::cat::MODULATION => crate::icons::CAT_MODULATION,
        c if c == k::fxreg::cat::DELAY => crate::icons::CAT_DELAY,
        c if c == k::fxreg::cat::DYNAMICS => crate::icons::CAT_DYNAMICS,
        c if c == k::fxreg::cat::TIMESTRETCH => crate::icons::CAT_TIMESTRETCH,
        _ => crate::icons::CAT_VIDEO,
    }
}

/// Сколько промежуточных ступеней между соседними точками оси силы
/// (Р.54, п.433). Ось КВАНТУЕТСЯ, потому что каждая позиция — свой
/// рецепт и свой рендер: без квантования проводка мышью порождала бы
/// новый кадр на каждый пиксель.
const AXIS_STEPS: i32 = 8;

/// Точка оси силы по сдвигу курсора. Ось — это ряд `[шаблон, вариант₀,
/// вариант₁, …]`, ячейка варианта `cell` стоит в позиции `cell + 1`, и
/// один размер ячейки сдвига = один шаг по оси.
///
/// Возвращает `(j, t)`: смешивать `axis[j]` с `axis[j + 1]` в доле `t`.
/// Чистая функция — её проверяет t551 без всякого egui.
pub(crate) fn axis_at(cell: usize, dx_px: f32, points: usize) -> (usize, f64) {
    let last = points.saturating_sub(1).max(1);
    let raw = (cell + 1) as f32 + dx_px / CELL_DRAW_W;
    // квантуем и зажимаем в границы ряда: за краями ось упирается, а не
    // «тянет» несуществующие значения
    let q = ((raw * AXIS_STEPS as f32).round() as i32).clamp(0, last as i32 * AXIS_STEPS);
    let j = (q / AXIS_STEPS) as usize;
    if j >= last {
        // правый край ряда выражается как «предыдущая точка, доля 1»
        return (last - 1, 1.0);
    }
    (j, (q % AXIS_STEPS) as f64 / AXIS_STEPS as f64)
}

/// Точка ряда: нулевая — ШАБЛОН эффекта (сила «ничего»), дальше варианты.
/// Ряд не материализуется: аллокация на каждую строку в draw-пути — это
/// инвариант №12.
fn axis_point(e: &FxEntry, k: usize) -> &FxInstance {
    if k == 0 { &e.template } else { &e.variants[k - 1].fx }
}

fn axis_name(e: &FxEntry, k: usize) -> &str {
    if k == 0 { e.name } else { tr(e.variants[k - 1].name) }
}

/// Ячейка полоски: картинка (или плашка, пока считается) и подпись под
/// ней, ОДНА интеракция на всё. Клик в центре без движения обязан дать
/// действие (философия t254), а по подписи ячейка находится харнессом.
/// `prefix` — имя эффекта для ДОСТУПНОГО имени ячейки. Видимая подпись
/// остаётся короткой («Мягко»), а доступное имя обязано быть уникальным:
/// словарь вариантов общий на реестр, и «Мягко» есть у шести видов —
/// без префикса ни скринридер, ни харнесс не отличат ячейку Blur от
/// ячейки Glow (нашлось при написании t547). Строка склеивается ВНУТРИ
/// `widget_info`, а его замыкание egui зовёт только при включённой
/// доступности — в обычном запуске это ноль аллокаций.
#[allow(clippy::too_many_arguments)]
fn preview_cell(
    ui: &mut egui::Ui,
    preview: &mut Option<&mut crate::fx_thumbs::PickerCtx<'_>>,
    extra: &[FxInstance],
    prefix: &str,
    label: &str,
    frame: egui::Color32,
    // ось силы: запись реестра и номер ячейки в её полоске. `None` —
    // ячейка без оси (пресет библиотеки: смешивать ЦЕПОЧКИ нельзя).
    axis: Option<(&FxEntry, usize)>,
) -> (egui::Response, Option<(usize, f64, FxInstance)>) {
    // подпись рисуется ПОД картинкой; пустая подпись = имя стоит рядом
    // (строка пресета), и место под текст не резервируется (правило 13)
    let cap_h =
        if label.is_empty() { 0.0 } else { ui.text_style_height(&egui::TextStyle::Small) + 2.0 };
    let size = egui::vec2(CELL_DRAW_W, CELL_DRAW_H + cap_h);
    // click_and_drag: клик без движения = именно этот вариант (t254),
    // а проводка по горизонтали ведёт ОСЬ СИЛЫ между соседями (п.433).
    // Идиома репозитория — одна интеракция с разбором clicked()/dragged(),
    // а не два вложенных виджета (запрет dnd_drag_source, t254b).
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let img_rect = egui::Rect::from_min_size(rect.min, egui::vec2(CELL_DRAW_W, CELL_DRAW_H));
    // НИЖЕ СГИБА прокрутки ячейка не считается и не рисуется: список
    // раскладывается целиком (прокрутке нужен полный размер), и без этого
    // гарда одно открытие галереи заказывало полторы сотни миниатюр —
    // видимых из них полтора десятка. Нашёл живой сценарий: кликать по
    // невидимой ячейке он уходил МИМО попапа, в таймлайн (её
    // прямоугольник честно лежал там, где её никто не видит).
    if !ui.is_rect_visible(rect) {
        return (resp, None);
    }
    // ОСЬ СИЛЫ (п.433): пока ведут по горизонтали, ячейка показывает
    // смесь соседних точек ряда — «сколько тут ставят» видно кадрами, а
    // не объявленным «главным параметром» (у VHS и Color Grade таких
    // параметра четыре, и выбор одного был бы спорным).
    //
    // Отсчёт — от ЦЕНТРА своей ячейки по абсолютной позиции указателя, а
    // не накоплением `drag_delta`: накопление уезжает от руки на каждом
    // пропущенном кадре. `drag_stopped` тоже читается — на кадре
    // отпускания `dragged()` уже false, а применять надо показанное.
    let pick = axis.and_then(|(e, cell)| {
        if !(resp.dragged() || resp.drag_stopped()) {
            return None;
        }
        let pos = resp.interact_pointer_pos()?;
        let (j, t) = axis_at(cell, pos.x - rect.center().x, e.variants.len() + 1);
        Some((j, t, axis_point(e, j).lerp(axis_point(e, j + 1), t)))
    });
    let shown: &[FxInstance] =
        pick.as_ref().map(|(_, _, fx)| std::slice::from_ref(fx)).unwrap_or(extra);
    let p = ui.painter();
    // подложка ОДНИМ прямоугольником, не шахматкой: прозрачные места
    // (Greenscreen, Pan3d, CornerPin) видны на ней как есть, а шахматка
    // из плиток дала бы тысячи примитивов на открытие (инвариант №12)
    p.rect_filled(img_rect, 2.0, crate::theme::EXTREME);
    if let Some(ctx) = preview.as_deref_mut()
        && let Some(tex) = ctx.texture(ui.ctx(), shown)
    {
        p.image(
            tex,
            img_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    let stroke = if resp.hovered() || pick.is_some() { crate::theme::ACCENT } else { frame };
    p.rect_stroke(img_rect, 2.0, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    resp.widget_info(|| {
        let name = match (prefix.is_empty(), label.is_empty()) {
            (_, true) => prefix.to_string(),
            (true, _) => label.to_string(),
            _ => format!("{prefix} {label}"),
        };
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name)
    });
    if label.is_empty() {
        return (resp, pick);
    }
    // подпись усекается, а не разносит ячейку (правило 7 чек-листа)
    let galley = ui.fonts_mut(|f| {
        let mut job = egui::text::LayoutJob::single_section(
            label.to_string(),
            egui::TextFormat {
                font_id: egui::TextStyle::Small.resolve(ui.style()),
                color: ui.visuals().weak_text_color(),
                ..Default::default()
            },
        );
        job.wrap = egui::text::TextWrapping {
            max_width: CELL_DRAW_W,
            max_rows: 1,
            overflow_character: Some('…'),
            ..Default::default()
        };
        f.layout_job(job)
    });
    let cap_x = img_rect.center().x - galley.size().x / 2.0;
    ui.painter().galley(egui::pos2(cap_x, img_rect.bottom() + 2.0), galley, egui::Color32::WHITE);
    (resp, pick)
}

/// Пикер, у которого ПЕРВОЙ строкой стоит голова полосы. В `registry()`
/// её нет намеренно: t48 сверяет реестр с шаблонами эффектов, а у
/// псевдо-слота шаблона не бывает — он не эффект, а часть морфера.
///
/// С Р.54 это галерея: у каждого видео-эффекта под именем стоит полоска
/// миниатюр (встроенные варианты и подходящие одноэффектные пресеты), а у
/// пресетов библиотеки — своя миниатюра целого блока. `preview` — контекст
/// ячеек; `None` означает «картинок не рисуем» (аудио-пикер, харнесс-тесты
/// без сервиса), и тогда пикер вырождается в прежний список имён.
#[allow(clippy::too_many_arguments)]
pub fn fx_picker_with_head(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    audio: bool,
    query: &mut String,
    head: Option<&str>,
    presets: &[crate::user_store::PresetRow],
    hover: &mut Option<String>,
    mut preview: Option<&mut crate::fx_thumbs::PickerCtx<'_>>,
) -> Option<Picked> {
    let mut picked = None;
    // CloseOnClickOutside обязателен (Р.54): у меню egui дефолт
    // CloseOnClick — попап закрывается ЛЮБЫМ кликом, в том числе по
    // миниатюре, и галерея была бы мертворождённой. Наши клики закрывают
    // попап явным `ui.close()`, Esc работает как раньше.
    let cfg = egui::containers::menu::MenuConfig::new()
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    let (btn, _) =
        egui::containers::menu::MenuButton::new(tr(k::fxreg::ADD)).config(cfg).ui(ui, |ui| {
            ui.set_min_width(if preview.is_some() { GALLERY_W } else { 230.0 });
            let edit = egui::TextEdit::singleline(query)
                .hint_text(tr(k::fxreg::SEARCH))
                .desired_width(210.0);
            let resp = ui.add(edit);
            // фокус в строку поиска при открытии меню
            if ui.input(|i| i.events.is_empty()) && !resp.has_focus() {
                resp.request_focus();
            }
            ui.separator();
            // голова — своим цветом и ПЕРВОЙ («поднять в списке fx»);
            // ищется той же строкой поиска, что и эффекты
            if let Some(h) = head
                && (query.is_empty() || h.to_lowercase().contains(&query.to_lowercase()))
            {
                let text = egui::RichText::new(h).color(crate::theme::MORPH_SLOT);
                if ui.button(text).clicked() {
                    picked = Some(Picked::Head);
                    query.clear();
                    ui.close();
                }
                ui.separator();
            }
            // Пресеты (Р.48) — СВОЕЙ секцией сразу после головы: это не
            // виды эффектов, и в статический реестр они не попадают (t48
            // сверяет его с шаблонами). Ищутся той же строкой — и по имени
            // пресета, и по именам эффектов внутри него.
            let q = query.to_lowercase();
            let shown: Vec<(usize, &crate::user_store::PresetRow)> = presets
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    q.is_empty()
                        || r.name.to_lowercase().contains(&q)
                        || r.block.slots.iter().any(|sl| {
                            crate::fx_editor::fx_display_name(&sl.fx).to_lowercase().contains(&q)
                        })
                })
                .collect();
            if !shown.is_empty() {
                ui.label(egui::RichText::new(tr(k::fx::PRESETS)).small().weak());
                // Пресеты живут НАД прокруткой эффектов (правило Р.53: за
                // ними тянутся первыми), но своя прокрутка им нужна — иначе
                // тридцать сохранённых цепочек выдавят список эффектов за
                // край экрана.
                egui::ScrollArea::vertical().id_salt("fxpreset-rows").max_height(PRESETS_H).show(
                    ui,
                    |ui| {
                        for (i, r) in shown {
                            let resp = if preview.is_some() && r.video_ok {
                                // миниатюра ЦЕЛОГО блока: кривые берутся в точке входа
                                // пресета (ключи в блоке ребейзнуты к нулю)
                                let chain: Vec<FxInstance> = r
                                    .block
                                    .slots
                                    .iter()
                                    .filter(|sl| !sl.bypass)
                                    .map(|sl| {
                                        let mut fx = sl.fx.clone();
                                        kt_model::apply_fx_auto(
                                            &mut fx,
                                            &sl.curves,
                                            kt_time::Tick(0),
                                        );
                                        fx
                                    })
                                    .collect();
                                ui.horizontal(|ui| {
                                    // подпись НЕ дублируем: имя стоит
                                    // кнопкой сразу справа от миниатюры
                                    // (правило 13). Оси у пресета нет —
                                    // смешивать ЦЕПОЧКИ бессмысленно.
                                    let (c, _) = preview_cell(
                                        ui,
                                        &mut preview,
                                        &chain,
                                        &r.name,
                                        "",
                                        crate::theme::MORPH_SLOT,
                                        None,
                                    );
                                    let lbl = crate::widgets::elided_name(
                                        ui,
                                        &r.name,
                                        ui.visuals().text_color(),
                                        false,
                                    );
                                    let b = ui.button(lbl);
                                    if c.clicked() { c } else { b }
                                })
                                .inner
                            } else {
                                let label = crate::widgets::elided_name(
                                    ui,
                                    &r.name,
                                    ui.visuals().text_color(),
                                    false,
                                );
                                ui.button(label)
                            };
                            if resp.clicked() {
                                picked = Some(Picked::Preset(i));
                                query.clear();
                                ui.close();
                            }
                            // заводские не переименовываются и не удаляются — и это
                            // видно: у них нет меню, а подсказка говорит почему
                            if r.factory {
                                resp.on_hover_text(tr(k::msg::PRESET_FACTORY));
                            } else {
                                let name = r.name.clone();
                                resp.context_menu(|ui| {
                                    if ui.button(tr(k::fx::RENAME_PRESET)).clicked() {
                                        picked = Some(Picked::PresetAction(
                                            crate::widgets::PresetAction::Rename(name.clone()),
                                        ));
                                        ui.close();
                                    }
                                    if ui
                                        .button(tr(k::fx::DELETE_PRESET))
                                        .on_hover_text(tr(k::fx::DELETE_PRESET_HINT))
                                        .clicked()
                                    {
                                        picked = Some(Picked::PresetAction(
                                            crate::widgets::PresetAction::Delete(name.clone()),
                                        ));
                                        ui.close();
                                    }
                                });
                            }
                        }
                    },
                );
                ui.separator();
            }
            let list = filter_entries(registry(), audio, query);
            if list.is_empty() && head.is_none() {
                ui.weak(tr(k::fxreg::NOTHING));
            }
            // КАТЕГОРИИ ГРУППИРУЮТСЯ НА ОТРИСОВКЕ (Р.54). Реестр отсортирован
            // по смыслу («кварталы» Р.47), и категории в нём повторяются:
            // заголовок «Стилизация» печатался пять раз, «Размытие» — три.
            // Порядок самих категорий — по первому появлению, сортировка
            // стабильная, поэтому внутри категории порядок реестра цел.
            let mut cats: Vec<Key> = Vec::new();
            for e in &list {
                if !cats.contains(&e.category) {
                    cats.push(e.category);
                }
            }
            let mut rows = list;
            rows.sort_by_key(|e| cats.iter().position(|c| *c == e.category).unwrap_or(usize::MAX));
            let mut last_cat: Option<Key> = None;
            egui::ScrollArea::vertical().id_salt("fxreg-rows").max_height(GALLERY_H).show(
                ui,
                |ui| {
                    for e in rows {
                        if last_cat != Some(e.category) {
                            last_cat = Some(e.category);
                            ui.label(egui::RichText::new(tr(e.category)).small().weak());
                        }
                        // строка имени: глиф категории + имя (клик = ШАБЛОН, как
                        // было до Р.54)
                        let mut hit_name = None;
                        ui.horizontal(|ui| {
                            ui.label(cat_glyph(e.category));
                            let r = crate::app::hint_into(
                                hover,
                                ui.button(e.name),
                                tr(k::fxreg::TEMPLATE_HINT),
                            );
                            if r.clicked() {
                                hit_name = Some(e.template.clone());
                            }
                        });
                        if let Some(t) = hit_name {
                            picked = Some(Picked::Fx(t));
                            query.clear();
                            ui.close();
                        }
                        // полоска: встроенные варианты, затем ОДНОэффектные
                        // пресеты пользователя того же вида — они про то же
                        // («вот как это настраивают»), и место у них там же
                        if preview.is_some() && !e.variants.is_empty() {
                            let mut hit = None;
                            ui.horizontal_wrapped(|ui| {
                                for (vi, v) in e.variants.iter().enumerate() {
                                    let (c, pick) = preview_cell(
                                        ui,
                                        &mut preview,
                                        std::slice::from_ref(&v.fx),
                                        e.name,
                                        tr(v.name),
                                        crate::theme::OUTLINE,
                                        Some((e, vi)),
                                    );
                                    crate::widgets::field_probe::note(
                                        &format!("fxreg.var.{}.{vi}", e.template.tag()),
                                        c.rect,
                                    );
                                    // подсказка — в статусбар, как всё в
                                    // программе (правило 10): пока ведут
                                    // ось, она говорит МЕЖДУ ЧЕМ ведут и
                                    // на сколько
                                    let axis_hint = pick.as_ref().map(|(j, t, _)| {
                                        crate::tf!(
                                            k::fxreg::AXIS_HINT,
                                            axis_name(e, *j),
                                            axis_name(e, j + 1),
                                            (t * 100.0).round() as i64
                                        )
                                    });
                                    let hint =
                                        axis_hint.as_deref().unwrap_or(tr(k::fxreg::VAR_HINT));
                                    let c = crate::app::hint_into(hover, c, hint);
                                    // клик без движения — ИМЕННО этот
                                    // вариант (t254); проводка — то, что
                                    // показано на кадре отпускания
                                    if c.clicked() {
                                        hit = Some(Picked::Fx(v.fx.clone()));
                                    } else if c.drag_stopped()
                                        && let Some((_, _, fx)) = pick
                                    {
                                        hit = Some(Picked::Fx(fx));
                                    }
                                }
                                for (i, r) in presets.iter().enumerate() {
                                    let one = match r.block.slots.as_slice() {
                                        [only] => only.fx.tag() == e.template.tag(),
                                        _ => false,
                                    };
                                    if !one || !r.video_ok {
                                        continue;
                                    }
                                    let mut fx = r.block.slots[0].fx.clone();
                                    kt_model::apply_fx_auto(
                                        &mut fx,
                                        &r.block.slots[0].curves,
                                        kt_time::Tick(0),
                                    );
                                    let (c, _) = preview_cell(
                                        ui,
                                        &mut preview,
                                        std::slice::from_ref(&fx),
                                        "",
                                        &r.name,
                                        crate::theme::MORPH_SLOT,
                                        None,
                                    );
                                    if crate::app::hint_into(hover, c, tr(k::fxreg::VAR_HINT))
                                        .clicked()
                                    {
                                        hit = Some(Picked::Preset(i));
                                    }
                                }
                            });
                            if let Some(h) = hit {
                                picked = Some(h);
                                query.clear();
                                ui.close();
                            }
                        }
                    }
                },
            );
            let _ = id_salt;
        });
    // прямоугольники для живых сценариев: модальные попапы egui в дереве
    // доступности не ищутся, а угадывать смещения внутри окна сценарий не
    // должен (Р.27). Ветка стоит один OnceLock-чек без KT_GEOM_DUMP.
    crate::widgets::field_probe::note("fxreg.add", btn.rect);
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Тест 48 (п.3 v1.4): полнота реестра и идентичность шаблонов прежним
    /// кнопкам/комбо; фильтр находит по подстроке имени и по keyword и
    /// отделяет аудио от видео.
    #[test]
    fn t48_registry_complete_and_filter_works() {
        let reg = registry();
        // все 10 видео-шаблонов на месте и байт-в-байт равны прежним
        let old = video_fx_templates();
        assert_eq!(reg.iter().filter(|e| !e.audio).count(), old.len());
        // ДВУСТОРОННЯЯ запись ровно одна — варп времени (Р.37). Он не
        // пиксельный фильтр, а правило выбора исходного кадра, поэтому
        // осмыслен на обоих видах; всё остальное обязано остаться в своей
        // корзине, иначе деление вырождается
        assert_eq!(
            reg.iter().filter(|e| e.audio && e.video).map(|e| e.name).collect::<Vec<_>>(),
            ["Slowed+Reverb"],
            "двусторонним бывает только растяжение времени"
        );
        for (name, tmpl) in &old {
            let e = reg.iter().find(|e| e.name == *name).expect("видео-эффект в реестре");
            assert!(!e.audio);
            assert_eq!(
                serde_json::to_string(&e.template).unwrap(),
                serde_json::to_string(tmpl).unwrap(),
                "шаблон {name} идентичен прежнему"
            );
        }
        // аудио-шаблоны прежних кнопок
        let gain = reg.iter().find(|e| e.name == "Gain").unwrap();
        assert!(gain.audio);
        assert_eq!(
            serde_json::to_string(&gain.template).unwrap(),
            serde_json::to_string(&FxInstance::gain(0.0)).unwrap()
        );
        let sr = reg.iter().find(|e| e.name == "Slowed+Reverb").unwrap();
        assert!(sr.audio);
        assert_eq!(
            serde_json::to_string(&sr.template).unwrap(),
            serde_json::to_string(&FxInstance::slowed_reverb(0.82, 0.1, 85.0)).unwrap()
        );
        // морф-слот в реестре НЕ живёт и жить не может (Р.35): у
        // псевдо-слота нет шаблона, а этот тест сверяет реестр именно с
        // шаблонами — «просто дописать его в реестр» обязано краснеть
        assert!(
            !reg.iter().any(|e| e.name.to_lowercase().contains("source")),
            "подмена источника — не эффект: её место в голове полосы, не в реестре"
        );
        // Р.47 (п.331): категория-фолбэк VIDEO — признак забытой записи в
        // video_meta; каждый видео-эффект обязан жить в своей категории
        for e in reg.iter().filter(|e| e.video && e.name != "Slowed+Reverb") {
            assert_ne!(
                e.category,
                k::fxreg::cat::VIDEO,
                "{}: fell into the fallback category (add it to video_meta)",
                e.name
            );
        }
        // имена согласованы с fx_display_name
        for e in reg {
            assert_eq!(crate::fx_editor::fx_display_name(&e.template), e.name);
        }
        // Р.54: у аудио-записей вариантов не бывает — миниатюру звука не
        // нарисовать, а полоска из одних подписей была бы вторым списком
        // имён на том же месте (правило 13)
        for e in reg.iter().filter(|e| e.audio) {
            assert!(e.variants.is_empty(), "{}: audio entries carry no variants", e.name);
        }
        // Р.47 (п.326): счёт — СВОЙСТВОМ реестра FxInstance::all(), а не
        // константой. Каждый аудио-вид из all() (кроме LayerMix — слот
        // силы, у него нет места в пикере) обязан быть в аудио-списке;
        // каждый видео-вид, у которого есть шаблон, — в видео-списке.
        crate::i18n::set_lang(crate::i18n::Lang::Ru);
        let audio_kinds: std::collections::BTreeSet<&str> =
            FxInstance::all().iter().filter(|f| f.is_audio()).map(|f| f.tag()).collect();
        let audio_in_picker: std::collections::BTreeSet<&str> =
            filter_entries(reg, true, "").iter().map(|e| e.template.tag()).collect();
        assert_eq!(audio_in_picker, audio_kinds, "каждый аудио-вид реестра — в пикере");
        // видео-список = все видео-шаблоны + двусторонний Slowed+Reverb
        assert_eq!(filter_entries(reg, false, "").len(), old.len() + 1);
        assert_eq!(filter_entries(reg, false, "reverb").len(), 1, "варп ищется и в видео-пикере");
        assert_eq!(filter_entries(reg, true, "РевЕрб").len(), 1, "кириллический keyword");
        assert_eq!(filter_entries(reg, false, "виш").len(), 0);
        assert_eq!(filter_entries(reg, false, "vhs").len(), 1);
        // Р.47 Кв3: "blur" находит Blur И Radial Blur
        assert_eq!(filter_entries(reg, false, "blur").len(), 2);
        assert_eq!(filter_entries(reg, true, "gain").len(), 1);
        assert!(filter_entries(reg, true, "vhs").is_empty(), "видео-эффект не в аудио-списке");
        // Р.19: английские ключевые слова обязаны находить эффект В ЛЮБОМ
        // языке — имя эффекта не переводится, и искать его будут по нему
        for lang in crate::i18n::Lang::ALL {
            crate::i18n::set_lang(lang);
            assert_eq!(filter_entries(reg, false, "blur").len(), 2, "blur в {}", lang.code());
            assert_eq!(filter_entries(reg, false, "vhs").len(), 1, "vhs в {}", lang.code());
            assert_eq!(filter_entries(reg, true, "reverb").len(), 1, "reverb в {}", lang.code());
        }
        crate::i18n::set_lang(crate::i18n::Lang::Ru);
    }

    /// Тест 543 (Р.54, п.427): встроенные варианты эффекта.
    ///
    /// Вариант — это НАСТРОЕННОЕ состояние того же вида, и у него ровно три
    /// обязанности: быть тем же видом, отличаться от шаблона и лежать в
    /// границах спек. Первая ловушка, которую тест ловит на старом коде, —
    /// опечатка в имени параметра: `set_param` возвращает false, значение
    /// не ставится, и вариант молча становится копией шаблона (в релизе
    /// `debug_assert!` в `var()` не стреляет, а ячейка полоски показывает
    /// исходный кадр).
    #[test]
    fn t543_builtin_variants_are_distinct_and_in_range() {
        let reg = registry();
        // порядок языков не важен, но имена читаем в известном
        crate::i18n::set_lang(crate::i18n::Lang::Ru);
        let mut with_variants = 0;
        for e in reg.iter().filter(|e| e.video && e.name != "Slowed+Reverb") {
            // забытая запись в video_variants даёт пустой список — та же
            // ловушка, что категория-фолбэк в t48
            assert!(
                e.variants.len() >= 2,
                "{}: fewer than two variants (add it to video_variants)",
                e.name
            );
            with_variants += 1;
            let tmpl_repr = format!("{:?}", e.template);
            let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for v in &e.variants {
                // тот же ВИД: вариант не подменяет эффект другим
                assert_eq!(
                    std::mem::discriminant(&v.fx),
                    std::mem::discriminant(&e.template),
                    "{}: variant changes the effect kind",
                    e.name
                );
                let repr = format!("{:?}", v.fx);
                // отличается от шаблона. Для Pan/Pan 3D/Color Grade/Corner
                // Pin/Drop Shadow это ещё и условие ИКОНКИ: их шаблон —
                // тождество, иконку рисует ПЕРВЫЙ вариант, и копия шаблона
                // дала бы пустую картинку
                assert_ne!(repr, tmpl_repr, "{}: variant equals the template", e.name);
                // и от остальных вариантов того же эффекта
                assert!(seen.insert(repr), "{}: two identical variants", e.name);
                // значения в границах спек (`set_param` клампит, но Mirror
                // собран литералом, и будущая правка может обойти var()).
                //
                // Проверка СТРОГАЯ намеренно, и вот почему: у f32-поля
                // ровный потолок спеки после `set_param` оказывается ВНЕ
                // спеки — кламп считается в f64, а запись в f32 округляет
                // вверх (`amount` = 0.05 у Chromatic Aberration давал
                // 0.05000000074505806). Вариант — это ВИД, а не проверка
                // предела: упёрся в границу — отойди от неё, а не ослабляй
                // тест допуском.
                for s in v.fx.param_specs() {
                    let x = v.fx.get_param(&s.name).unwrap_or(f64::NAN);
                    assert!(
                        x.is_finite() && x >= s.min && x <= s.max,
                        "{} / {}: {} = {x} out of [{}, {}]",
                        e.name,
                        tr(v.name),
                        s.name,
                        s.min,
                        s.max
                    );
                }
            }
            // имена вариантов ОДНОГО эффекта различимы во всех языках:
            // словарь общий на реестр, и «Мягко» дважды в одной полоске —
            // это два неразличимых кликабельных места
            for lang in crate::i18n::Lang::ALL {
                crate::i18n::set_lang(lang);
                let mut names: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
                for v in &e.variants {
                    let n = tr(v.name);
                    assert!(!n.is_empty(), "{}: empty variant name in {}", e.name, lang.code());
                    assert!(
                        names.insert(n),
                        "{}: duplicate variant name {n:?} in {}",
                        e.name,
                        lang.code()
                    );
                }
            }
            crate::i18n::set_lang(crate::i18n::Lang::Ru);
        }
        // счёт — свойством реестра, не константой (правило t48)
        assert_eq!(with_variants, video_fx_templates().len());
        // голова полосы вариантов не имеет: она не эффект (Р.35)
        // и в реестре её нет — проверено в t48.

        // поиск по имени варианта показывает СТРОКУ эффекта, и в любом
        // языке — по английскому имени тоже (правило Р.19)
        for lang in crate::i18n::Lang::ALL {
            crate::i18n::set_lang(lang);
            let by_var = filter_entries(reg, false, "blueprint");
            assert_eq!(
                by_var.iter().map(|e| e.name).collect::<Vec<_>>(),
                ["Edges"],
                "blueprint → Edges в {}",
                lang.code()
            );
        }
        crate::i18n::set_lang(crate::i18n::Lang::Ru);
        assert_eq!(
            filter_entries(reg, false, "чертёж").iter().map(|e| e.name).collect::<Vec<_>>(),
            ["Edges"],
            "русское имя варианта тоже ищется"
        );
        // ...и не ломает прежние счётчики: матч на уровне СТРОКИ
        assert_eq!(filter_entries(reg, false, "blur").len(), 2);
    }
}

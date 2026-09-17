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
        "Pan" => (k::fxreg::cat::MOTION, Some(k::fxreg::kw::PAN)),
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
        "Pan" => vec![
            var(tmpl, k::fxreg::var::CLOSEUP, &[("zoom", 1.25)]),
            var(tmpl, k::fxreg::var::PUNCH, &[("zoom", 2.0)]),
            var(tmpl, k::fxreg::var::OFFCENTER, &[("zoom", 1.4), ("pan_x", 0.33)]),
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

/// Пикер, у которого ПЕРВОЙ строкой стоит голова полосы. В `registry()`
/// её нет намеренно: t48 сверяет реестр с шаблонами эффектов, а у
/// псевдо-слота шаблона не бывает — он не эффект, а часть морфера.
pub fn fx_picker_with_head(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    audio: bool,
    query: &mut String,
    head: Option<&str>,
    presets: &[crate::user_store::PresetRow],
) -> Option<Picked> {
    let mut picked = None;
    ui.menu_button(tr(k::fxreg::ADD), |ui| {
        ui.set_min_width(230.0);
        let edit =
            egui::TextEdit::singleline(query).hint_text(tr(k::fxreg::SEARCH)).desired_width(210.0);
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
            for (i, r) in shown {
                let label =
                    crate::widgets::elided_name(ui, &r.name, ui.visuals().text_color(), false);
                let resp = ui.button(label);
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
            ui.separator();
        }
        let list = filter_entries(registry(), audio, query);
        if list.is_empty() && head.is_none() {
            ui.weak(tr(k::fxreg::NOTHING));
        }
        let mut last_cat: Option<Key> = None;
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for e in list {
                if last_cat != Some(e.category) {
                    last_cat = Some(e.category);
                    ui.label(egui::RichText::new(tr(e.category)).small().weak());
                }
                if ui.button(e.name).clicked() {
                    picked = Some(Picked::Fx(e.template.clone()));
                    query.clear();
                    ui.close();
                }
            }
        });
        let _ = id_salt;
    });
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
                    let x = v.fx.get_param(s.name).unwrap_or(f64::NAN);
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

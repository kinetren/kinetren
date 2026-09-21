//! Форма ГЕНЕРАТОРА события (Р.59, стадия 2).
//!
//! До этого раунда у генератора и у титра были СВОИ окна, а параметры
//! лежали в пути медиа. Владелец назвал это дословно: генератор «запрятан
//! в жопу», а его форма обязана быть первой крошкой цепочки эффектов
//! ТОГО ЖЕ события — рядом со всем остальным, что у события правится.
//! Поэтому окон больше нет: здесь живёт одна форма, которую рисует
//! `fx_editor` в приколотой голове полосы (`widgets::HeadSlot`), и
//! переиспользуемые куски прежних двух окон — меню создания, имена видов,
//! цветовые группы, предупреждения о шрифте и глифах, пресеты титра.
//!
//! Значения едут на СОБЫТИЕ (`Command::SetClipGen`), кривые —
//! `SetClipGenAuto`, «править во всех местах разом» — отдельная команда
//! `ApplyGenToAll` (решение владельца: правка одного события соседей
//! больше не трогает). Вид генератора остаётся свойством МЕДИА: это его
//! адрес и его кэш-полоска (решение Р.53 в силе), поэтому смена вида —
//! `SetMediaPath`, и модель сама переодевает события (`fit_gen_to_media`).

use crate::i18n::{k, tr};
use crate::widgets;
use eframe::egui;
use kt_model::{GeneratorKind, HasParams, TextAlign, TextAnchor};

/// Локализованное имя вида. Полные пути — тест 125 ищет ключ по ним.
/// Подменю «Создать генератор…» — ОДНО ТЕЛО на все входы (правило 17).
///
/// Р.53: видов девятнадцать, плоский список из них — свалка. Группа —
/// ФУНКЦИЯ вида (`GenGroup::of`), второго списка нет: новый вид попадает
/// в меню сам. Прямоугольники пунктов и групп — наружу для живых
/// сценариев (Р.49): подменю в дереве доступности не ищется.
///
/// Р.55 (С4 претензии 8): вход перестал быть единственным. Раньше
/// генератор создавался ТОЛЬКО правым кликом по пустому фону бина — «я не
/// знаю как создать генератор медиа, у нас код есть а как создавать в ui
/// не понятно». Теперь тот же самый список висит и в главном меню, и
/// второго списка видов при этом не завелось.
pub fn generator_menu(ui: &mut egui::Ui) -> Option<GeneratorKind> {
    generator_kind_menu(ui, tr(k::bin::menu::NEW_GENERATOR))
}

/// То же меню под другой подписью: «создать» и «сменить вид» — ОДИН
/// список видов (правило 17), и новый вид попадает в оба входа сам.
pub fn generator_kind_menu(ui: &mut egui::Ui, label: &str) -> Option<GeneratorKind> {
    let mut picked = None;
    let mr = ui.menu_button(label, |ui| {
        for grp in GenGroup::ALL {
            let kinds: Vec<_> =
                GeneratorKind::all().into_iter().filter(|k| GenGroup::of(k) == grp).collect();
            if kinds.is_empty() {
                continue;
            }
            let gr = ui.menu_button(grp.name(), |ui| {
                for kd in kinds {
                    let r = ui.button(kind_name(&kd));
                    let probe = match &kd {
                        // у таблиц тег один на пять видов — различаем
                        // разновидностью (живые сценарии)
                        GeneratorKind::TestPattern { table } => {
                            format!("gen.new.table.{}", table.token())
                        }
                        other => format!("gen.new.{}", other.tag()),
                    };
                    crate::widgets::field_probe::note(&probe, r.rect);
                    if r.clicked() {
                        picked = Some(kd);
                        ui.close();
                    }
                }
            });
            crate::widgets::field_probe::note(
                &format!("gen.new.group.{}", grp.probe()),
                gr.response.rect,
            );
        }
    });
    crate::widgets::field_probe::note("gen.new", mr.response.rect);
    picked
}

pub fn kind_name(kind: &GeneratorKind) -> &'static str {
    match kind {
        GeneratorKind::Solid { .. } => tr(k::genkind::SOLID),
        GeneratorKind::Text { .. } => tr(k::genkind::TEXT),
        GeneratorKind::Gradient { .. } => tr(k::genkind::GRADIENT),
        GeneratorKind::Corners { .. } => tr(k::genkind::CORNERS),
        GeneratorKind::Checker { .. } => tr(k::genkind::CHECKER),
        GeneratorKind::NoiseField { .. } => tr(k::genkind::NOISE),
        GeneratorKind::SmpteBars { .. } => tr(k::genkind::SMPTE),
        GeneratorKind::Timecode { .. } => tr(k::genkind::TIMECODE),
        GeneratorKind::Transparent => tr(k::genkind::TRANSPARENT),
        GeneratorKind::Rect { .. } => tr(k::genkind::RECT),
        GeneratorKind::Ellipse { .. } => tr(k::genkind::ELLIPSE),
        GeneratorKind::Line { .. } => tr(k::genkind::LINE),
        GeneratorKind::Leader { .. } => tr(k::genkind::LEADER),
        // разновидность таблицы — часть ИМЕНИ: в меню их пять, и «просто
        // таблица» не сказала бы, какая именно (правило UI 18)
        GeneratorKind::TestPattern { table } => match table {
            kt_model::TestTable::Ebu75 => tr(k::genkind::EBU75),
            kt_model::TestTable::Ebu100 => tr(k::genkind::EBU100),
            kt_model::TestTable::Grayscale => tr(k::genkind::GRAYSCALE),
            kt_model::TestTable::Steps10 => tr(k::genkind::STEPS10),
            kt_model::TestTable::RampYcbcr => tr(k::genkind::YCBCR),
        },
        GeneratorKind::WindowPattern { .. } => tr(k::genkind::WINDOW),
    }
}

/// Даёт ли вид С УМОЛЧАНИЯМИ пустой кадр (Р.59, стадия 2).
///
/// Значения переехали на событие, и карточка бина показывает теперь вид с
/// умолчаниями. У титра умолчание — пустая строка, у прозрачной подложки
/// пусто по смыслу: вместо чёрного прямоугольника карточка пишет ИМЯ
/// ВИДА (правило 16 — невозможность показать картинку видна, а не молчит).
pub fn blank_default(kind: &GeneratorKind) -> bool {
    match kind {
        GeneratorKind::Text { text, .. } => text.trim().is_empty(),
        GeneratorKind::Transparent => true,
        _ => false,
    }
}

/// Группа меню создания (Р.53, п.422): видов стало девятнадцать, и
/// плоский список из них — свалка. Группы — функция ВИДА, второго
/// списка не заводим.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenGroup {
    Background,
    Shapes,
    Titles,
    Check,
}

impl GenGroup {
    pub(crate) const ALL: [GenGroup; 4] =
        [GenGroup::Background, GenGroup::Shapes, GenGroup::Titles, GenGroup::Check];

    pub(crate) fn name(self) -> &'static str {
        match self {
            GenGroup::Background => tr(k::genkind::GROUP_BG),
            GenGroup::Shapes => tr(k::genkind::GROUP_SHAPES),
            GenGroup::Titles => tr(k::genkind::GROUP_TITLES),
            GenGroup::Check => tr(k::genkind::GROUP_CHECK),
        }
    }

    /// Стабильный ключ группы для живых сценариев (имя локализовано и
    /// щупать по нему нельзя).
    pub(crate) fn probe(self) -> &'static str {
        match self {
            GenGroup::Background => "bg",
            GenGroup::Shapes => "shapes",
            GenGroup::Titles => "titles",
            GenGroup::Check => "check",
        }
    }

    pub(crate) fn of(kind: &GeneratorKind) -> GenGroup {
        match kind {
            GeneratorKind::Solid { .. }
            | GeneratorKind::Gradient { .. }
            | GeneratorKind::Corners { .. }
            | GeneratorKind::Transparent
            | GeneratorKind::NoiseField { .. } => GenGroup::Background,
            GeneratorKind::Rect { .. }
            | GeneratorKind::Ellipse { .. }
            | GeneratorKind::Line { .. } => GenGroup::Shapes,
            GeneratorKind::Text { .. }
            | GeneratorKind::Timecode { .. }
            | GeneratorKind::Leader { .. } => GenGroup::Titles,
            GeneratorKind::Checker { .. }
            | GeneratorKind::SmpteBars { .. }
            | GeneratorKind::TestPattern { .. }
            | GeneratorKind::WindowPattern { .. } => GenGroup::Check,
        }
    }
}

/// Цветовые группы вида для пипеток формы (подпись, [имена R,G,B]).
/// Публична для t508: параметр либо подписан, либо закрыт пипеткой —
/// третьего не бывает (Р.53, п.414).
pub(crate) fn color_groups(
    kind: &GeneratorKind,
) -> &'static [(crate::i18n::Key, [&'static str; 3])] {
    match kind {
        GeneratorKind::Solid { .. } => &[(k::trn::COLOR, ["color_r", "color_g", "color_b"])],
        GeneratorKind::Gradient { .. } => {
            &[(k::genw::COLOR_A, ["a_r", "a_g", "a_b"]), (k::genw::COLOR_B, ["b_r", "b_g", "b_b"])]
        }
        GeneratorKind::Corners { .. } => &[
            (k::genw::CORNER_TL, ["tl_r", "tl_g", "tl_b"]),
            (k::genw::CORNER_TR, ["tr_r", "tr_g", "tr_b"]),
            (k::genw::CORNER_BL, ["bl_r", "bl_g", "bl_b"]),
            (k::genw::CORNER_BR, ["br_r", "br_g", "br_b"]),
        ],
        GeneratorKind::Checker { .. } => {
            &[(k::genw::COLOR_A, ["a_r", "a_g", "a_b"]), (k::genw::COLOR_B, ["b_r", "b_g", "b_b"])]
        }
        // у фигур цвет с АЛЬФОЙ (Р.53, п.419)
        GeneratorKind::Rect { .. } | GeneratorKind::Ellipse { .. } | GeneratorKind::Line { .. } => {
            &[(k::trn::COLOR, ["color_r", "color_g", "color_b"])]
        }
        GeneratorKind::Text { .. }
        | GeneratorKind::NoiseField { .. }
        | GeneratorKind::SmpteBars { .. }
        | GeneratorKind::Timecode { .. }
        | GeneratorKind::Transparent
        | GeneratorKind::Leader { .. }
        | GeneratorKind::TestPattern { .. }
        | GeneratorKind::WindowPattern { .. } => &[],
    }
}

/// Имя альфа-компоненты цветовой группы вида, если она есть (Р.53,
/// п.419). У фигур цвет четырёхкомпонентный, у остальных видов — нет.
pub(crate) fn color_alpha(kind: &GeneratorKind) -> Option<&'static str> {
    matches!(
        kind,
        GeneratorKind::Rect { .. } | GeneratorKind::Ellipse { .. } | GeneratorKind::Line { .. }
    )
    .then_some("color_a")
}

/// Предупреждение о ПОДМЕНЕ шрифта (Р.52 С4): выбранной семьи на этой
/// машине нет — кадр рисуется вендоренной (решение владельца: «промах =
/// подмена вендоренным + видимое уведомление»).
pub fn font_warning(kind: &GeneratorKind) -> Option<String> {
    let GeneratorKind::Text { family, bold, .. } = kind else { return None };
    match kt_text::resolve_family(family, *bold) {
        kt_text::FontResolve::MissingFallback => {
            Some(crate::tf!(k::titlew::FONT_MISSING, family.clone()))
        }
        _ => None,
    }
}

/// Предупреждение «глифа нет» для текущего текста (правило UI 11: t100
/// проверяет каталоги, а текст титра приходит от пользователя — ответ
/// обязан быть видимым, тофу в кадре — допустимым, молчание — нет).
pub fn title_warning(kind: &GeneratorKind) -> Option<String> {
    let GeneratorKind::Text { text, family, bold, .. } = kind else { return None };
    let style = if *bold { kt_text::FaceStyle::Bold } else { kt_text::FaceStyle::Regular };
    let missing = kt_text::uncovered(text, family, style);
    let first = *missing.first()?;
    Some(crate::tf!(
        k::titlew::MISSING,
        format!("«{first}» U+{:04X}", first as u32),
        missing.len().saturating_sub(1)
    ))
}

/// Кэш предупреждений формы на кадр (Р.53, п.413): «глифа нет»
/// перебирает символы через резолв начертания, «шрифта нет» — карту
/// семей, и оба считались НА КАЖДЫЙ КАДР окна. Ключ — канонический путь
/// вида со значениями: другого входа у них нет.
#[derive(Default)]
pub struct GenWarn {
    key: Option<String>,
    missing: Option<String>,
    font: Option<String>,
}

impl GenWarn {
    fn refresh(&mut self, kind: &GeneratorKind) {
        let key = kt_model::format_gen_path(kind);
        if self.key.as_deref() == Some(key.as_str()) {
            return;
        }
        self.missing = title_warning(kind);
        self.font = font_warning(kind);
        self.key = Some(key);
    }
}

/// Пресеты титра (Р.53, п.424). Решение владельца: **пресет ставит ВСЁ**
/// — кегль, позицию, якорь, выравнивание, цвета и анимацию, — и это один
/// шаг истории: нажал, не понравилось, один Ctrl+Z вернул как было.
///
/// Активный пресет определяется ОБРАТНОЙ функцией — сравнением с тем,
/// что пресет произвёл бы из текущего текста. Поэтому ручная подкрутка
/// честно показывает «не совпадает ни с одним», а не подсвечивает
/// кнопку, к которой титр уже не имеет отношения.
pub struct TitlePreset {
    pub key: crate::i18n::Key,
    apply: fn(&mut GeneratorKind),
    /// Анимация пресета (Р.59, стадия 2): ставится КРИВЫМИ на событие, а
    /// не полем вида. Формулы — `kt_model::text_anim_curves`, и это
    /// единственное их место: теми же читаются старые файлы.
    pub anim: kt_model::TextAnim,
    pub anim_secs: f64,
}

pub fn title_presets() -> &'static [TitlePreset] {
    use kt_model::TextAnim;
    &[
        TitlePreset {
            key: k::titlew::PRESET_LOWER_THIRD,
            anim: TextAnim::None,
            anim_secs: 1.0,
            apply: |g| {
                let GeneratorKind::Text {
                    size,
                    pos_x,
                    pos_y,
                    anchor,
                    align,
                    plate_a,
                    plate_pad_x,
                    plate_pad_y,
                    ..
                } = g
                else {
                    return;
                };
                *size = 0.055;
                *pos_x = 0.08;
                *pos_y = 0.82;
                *anchor = TextAnchor::BottomLeft;
                *align = TextAlign::Left;
                *plate_a = 170;
                *plate_pad_x = 0.35;
                *plate_pad_y = 0.2;
            },
        },
        TitlePreset {
            key: k::titlew::PRESET_HEADLINE,
            anim: TextAnim::None,
            anim_secs: 1.0,
            apply: |g| {
                let GeneratorKind::Text {
                    size,
                    pos_x,
                    pos_y,
                    anchor,
                    align,
                    plate_a,
                    stroke_w,
                    ..
                } = g
                else {
                    return;
                };
                *size = 0.14;
                *pos_x = 0.5;
                *pos_y = 0.5;
                *anchor = TextAnchor::Center;
                *align = TextAlign::Center;
                *plate_a = 0;
                *stroke_w = 0.02;
            },
        },
        TitlePreset {
            key: k::titlew::PRESET_TYPEWRITER,
            anim: TextAnim::Typewriter,
            anim_secs: 2.0,
            apply: |g| {
                let GeneratorKind::Text { size, pos_x, pos_y, anchor, align, plate_a, .. } = g
                else {
                    return;
                };
                *size = 0.07;
                *pos_x = 0.5;
                *pos_y = 0.5;
                *anchor = TextAnchor::Center;
                *align = TextAlign::Center;
                *plate_a = 0;
            },
        },
        TitlePreset {
            key: k::titlew::PRESET_CRAWL,
            anim: TextAnim::Crawl,
            anim_secs: 12.0,
            apply: |g| {
                let GeneratorKind::Text { size, pos_x, pos_y, anchor, align, plate_a, .. } = g
                else {
                    return;
                };
                *size = 0.06;
                *pos_x = 0.5;
                *pos_y = 0.9;
                *anchor = TextAnchor::Center;
                *align = TextAlign::Left;
                *plate_a = 0;
            },
        },
        TitlePreset {
            key: k::titlew::PRESET_ROLL,
            anim: TextAnim::Roll,
            anim_secs: 20.0,
            apply: |g| {
                let GeneratorKind::Text { size, pos_x, pos_y, anchor, align, plate_a, .. } = g
                else {
                    return;
                };
                *size = 0.06;
                *pos_x = 0.5;
                *pos_y = 0.5;
                *anchor = TextAnchor::Center;
                *align = TextAlign::Center;
                *plate_a = 0;
            },
        },
    ]
}

impl TitlePreset {
    /// Что пресет сделал бы с ЭТИМ титром (текст и семья не трогаются).
    ///
    /// Значения, которые ставит анимация (`reveal`, смещения), входят
    /// сюда же: иначе «активный пресет» переставал бы узнавать свой
    /// собственный результат, как только кривые лягут на событие.
    pub fn applied(&self, base: &GeneratorKind) -> GeneratorKind {
        let mut g = base.clone();
        (self.apply)(&mut g);
        kt_model::text_anim_values(&mut g, self.anim);
        g
    }

    /// Кривые пресета в домене МАТЕРИАЛА события: формулы живут в
    /// `kt_model::text_anim_curves` (ими же читаются старые файлы —
    /// разойдись миграция с пресетом, проект прошлой сборки поехал бы не
    /// так), а якорь `source_in` дописывает вызывающий.
    pub fn curves(
        &self,
        applied: &GeneratorKind,
        frame: (u32, u32),
        source_in: kt_time::Tick,
    ) -> Vec<kt_model::ParamCurve> {
        let mut curves = kt_model::text_anim_curves(
            self.anim,
            self.anim_secs,
            anim_span(applied, self.anim, frame),
        );
        for c in &mut curves {
            for key in &mut c.keys {
                key.t = kt_time::Tick(key.t.0 + source_in.0);
            }
        }
        curves
    }
}

/// Полный путь блока в долях кадра: `1 + размер_блока/размер_кадра`.
/// Раскладку считает ТА ЖЕ функция, что рисует кадр и рамку драга —
/// второго измерителя текста в программе нет (kt-text/CLAUDE.md).
fn anim_span(kind: &GeneratorKind, anim: kt_model::TextAnim, (w, h): (u32, u32)) -> f64 {
    let (w, h) = (w.max(1), h.max(1));
    let Some(info) = kt_media::generator::title_layout(kind, w, h) else { return 2.0 };
    match anim {
        kt_model::TextAnim::Crawl => 1.0 + info.block.w / w as f64,
        kt_model::TextAnim::Roll => 1.0 + info.block.h / h as f64,
        _ => 2.0,
    }
}

/// Номер пресета, которому титр СОВПАДАЕТ целиком, или `None` («не
/// совпадает ни с одним»). Обратная функция — сравнение результата, а не
/// отдельный флаг «какой пресет нажимали»: флаг врал бы после первой же
/// ручной правки.
///
/// С Р.59 пресет ставит ещё и КРИВЫЕ, поэтому сравниваются оба:
/// умолчание титра по значениям совпадает с «печатной машинкой» (тот же
/// кегль и та же середина кадра), и без второй половины сравнения новый
/// пустой титр объявлялся бы напечатанным.
pub fn active_preset(kind: &GeneratorKind, curves: &[kt_model::ParamCurve]) -> Option<usize> {
    title_presets().iter().position(|p| p.applied(kind) == *kind && anim_matches(p.anim, curves))
}

/// Тот ли НАБОР рядов анимирован, что ставит пресет. Значения ключей не
/// сравниваются намеренно: путь бегущей строки считается по раскладке
/// текста, а текст человек меняет — иначе пресет «отваливался» бы от
/// собственного результата на первой же букве.
fn anim_matches(anim: kt_model::TextAnim, curves: &[kt_model::ParamCurve]) -> bool {
    use kt_model::TextAnim as A;
    let want: &[&str] = match anim {
        A::None => &[],
        A::Typewriter | A::Fade => &["reveal"],
        A::Crawl => &["offset_x"],
        A::Roll => &["offset_y"],
    };
    let mut have: Vec<&str> = curves.iter().map(|c| c.param.as_str()).collect();
    have.sort_unstable();
    have == want
}

/// Порядок слоёв титра (Р.53, п.415). Поле `order` живёт в формате с
/// Р.52 — урок Kdenlive, где порядок был константой и перерисовал чужие
/// проекты, — но редактора у него не было: поменять местами заливку и
/// обводку было нечем. Четыре кнопки-слота, клик меняет местами с
/// соседом; подпись говорит, что верх списка рисуется ПОСЛЕДНИМ.
fn order_row(ui: &mut egui::Ui, g: &mut GeneratorKind) -> bool {
    let GeneratorKind::Text { order, .. } = g else { return false };
    let name = |l: kt_model::TextLayer| match l {
        kt_model::TextLayer::Plate => tr(k::titlew::PLATE),
        kt_model::TextLayer::Shadow => tr(k::titlew::SHADOW),
        kt_model::TextLayer::Stroke => tr(k::titlew::STROKE),
        kt_model::TextLayer::Fill => tr(k::titlew::FILL),
    };
    let mut changed = false;
    ui.horizontal(|ui| {
        crate::widgets::hint(&ui.label(tr(k::titlew::ORDER)), tr(k::titlew::HINT_ORDER));
        for i in 0..order.len() {
            let r = ui.small_button(name(order[i]));
            crate::widgets::field_probe::note(&format!("titlew.order.{i}"), r.rect);
            // клик — «поднять выше соседа»: перестановка без драга, как у
            // слотов цепочки (правило 1: попасть мышью с первого раза)
            if r.clicked() && i + 1 < order.len() {
                order.swap(i, i + 1);
                changed = true;
            }
        }
    });
    changed
}

/// Что форма генератора наредактировала за кадр.
///
/// Разделение то же, что у `widgets::ParamsChanged`: база едет
/// `SetClipGen`, кривая — `SetClipGenAuto`. Остальное — жесты, у которых
/// СВОЙ шаг истории (правило 20): смена вида и пресет.
#[derive(Default)]
pub struct GenEdit {
    pub changed: widgets::ParamsChanged,
    /// Сменить ВИД генератора (свойство медиа, свой шаг истории).
    pub switch_kind: Option<GeneratorKind>,
    /// Нажат пресет: и значения, и кривые, одним своим шагом истории.
    pub preset: bool,
    /// «Применить ко всем (×N)».
    pub apply_all: bool,
    /// Куда двинуть главный курсор (жест по мини-таймлайну графика).
    pub seek: Option<kt_time::Tick>,
    pub axis: crate::automation::AxisReq,
}

/// Что форме нужно знать о месте, где она нарисована.
pub struct GenCtx<'a> {
    /// Событие-владелец: домен кривых — его материал.
    pub clip: &'a kt_model::ClipEvent,
    /// Контекст ПУБЛИКАЦИИ (Р.59, стадия 3): подписи полей генератора —
    /// та же дверь наружу, что у параметров эффекта. Именно здесь живёт
    /// «заголовок новости» из претензии владельца.
    pub pubc: Option<&'a crate::widgets::PublishCtx>,
    /// Сколько мест на монтаже у этого генератора («применить ко всем»).
    pub uses: usize,
    /// Размер кадра проекта — по нему считается путь бегущей строки.
    pub frame: (u32, u32),
    pub frame_step: i64,
    pub playhead: kt_time::Tick,
    /// Окно ГЛАВНОГО таймлайна (замок оси графика).
    pub tl_view: (kt_time::Tick, kt_time::Tick),
    pub axis_linked: bool,
}

/// Форма генератора: вид, значения, кривые — ОДНИМ телом на все входы.
///
/// Рисуется в окне эффектов события, когда выбрана приколотая голова
/// полосы. Своего окна у генератора и титра больше нет: владелец просил
/// видеть источник крошкой ПЕРЕД эффектами, в том же окне.
#[allow(clippy::too_many_lines)]
pub fn draw_gen_section(
    ui: &mut egui::Ui,
    ctx: &GenCtx<'_>,
    kind: &mut GeneratorKind,
    auto: &mut Vec<kt_model::ParamCurve>,
    view: &mut crate::automation::AutoView,
    warn: &mut GenWarn,
    hover: &mut Option<String>,
) -> GenEdit {
    let mut out = GenEdit::default();
    let clip = ctx.clip;

    // ---- вид: тот же пикер, что был в обоих прежних окнах -------------
    ui.horizontal(|ui| {
        widgets::hint(&ui.label(tr(k::trn::KIND)), tr(k::genw::HINT_KIND));
        egui::ComboBox::from_id_salt(("genkind", clip.id.0))
            .selected_text(kind_name(kind))
            .show_ui(ui, |ui| {
                for kd in GeneratorKind::all() {
                    let same = kd.tag() == kind.tag();
                    let r = ui.selectable_label(same, kind_name(&kd));
                    crate::widgets::field_probe::note(&format!("genw.kind.{}", kd.tag()), r.rect);
                    if r.clicked() && !same {
                        // смена вида = его умолчания: параметры у видов
                        // разные, переносить нечего
                        out.switch_kind = Some(kd);
                    }
                }
            });
    });
    if out.switch_kind.is_some() {
        // вид уезжает целиком — рисовать старую форму дальше незачем
        return out;
    }

    // ---- титр: строка, предупреждения, пресеты -------------------------
    if matches!(kind, GeneratorKind::Text { .. }) {
        {
            let GeneratorKind::Text { text, .. } = kind else { unreachable!() };
            let te = egui::TextEdit::multiline(text)
                .id(egui::Id::new(("title-text", clip.id.0)))
                .desired_rows(3)
                .desired_width(f32::INFINITY);
            let r = ui.add(te);
            crate::widgets::field_probe::note("titlew.text", r.rect);
            widgets::hint(&r, tr(k::titlew::HINT_TEXT));
            // ПУБЛИКАЦИЯ ЗАГОЛОВКА — то, ради чего затеян весь раунд
            // («не будут же люди каждый раз лезть в композицию и менять
            // там текст»). У многострочного поля титра подписи нет и
            // заводить её ради одной надписи нельзя (правило 13),
            // поэтому дверь висит на самом поле — там же, где жест.
            widgets::publish_on_field(
                &r,
                ctx.pubc,
                "text",
                // имя ручки ПО УМОЛЧАНИЮ — короткая подпись свойства, а не
                // фраза подсказки: дальше его правит автор, но начинать
                // ему с предложения не следует
                tr(k::genp::TEXT),
                kt_model::ParamType::Text { multiline: true },
            );
            if r.changed() {
                out.changed.base = true;
            }
        }
        warn.refresh(kind);
        for w in [&warn.missing, &warn.font].into_iter().flatten() {
            ui.colored_label(egui::Color32::from_rgb(255, 105, 97), w);
        }
        // пресеты живут НАД остальной формой: за ними тянутся первыми
        let active = active_preset(kind, auto);
        let mut pick: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(tr(k::titlew::PRESETS));
            for (i, pr) in title_presets().iter().enumerate() {
                let r = ui.selectable_label(active == Some(i), tr(pr.key));
                crate::widgets::field_probe::note(&format!("titlew.preset.{i}"), r.rect);
                widgets::hint(&r, tr(k::titlew::HINT_PRESET));
                if r.clicked() {
                    pick = Some(i);
                }
            }
            if active.is_none() {
                // честный ответ вместо подсветки кнопки, к которой титр
                // уже не имеет отношения (решение владельца)
                ui.weak(tr(k::titlew::PRESET_NONE));
            }
        });
        if let Some(i) = pick {
            let pr = &title_presets()[i];
            let made = pr.applied(kind);
            *auto = pr.curves(&made, ctx.frame, clip.source_in);
            *kind = made;
            out.preset = true;
            out.changed.base = true;
            out.changed.curve = true;
            // выбор ключа на графике стал бессмысленным: кривые другие
            view.sel = None;
            view.active = 0;
        }
    }

    // ---- цвета: Rgb8-тройки — пипетками (правило 13) -------------------
    let groups = color_groups(kind);
    let alpha = color_alpha(kind);
    let mut skip: Vec<&str> = Vec::new();
    for (_, names) in groups {
        skip.extend_from_slice(names);
    }
    for (i, (label, names)) in groups.iter().enumerate() {
        if widgets::color_row(
            ui,
            tr(*label),
            kind,
            *names,
            alpha,
            (i == 0).then_some("genw.color"),
            ctx.pubc,
        ) {
            out.changed.base = true;
        }
    }

    // ---- титр: семья, начертание, якорь, цвета слоёв, порядок ----------
    if matches!(kind, GeneratorKind::Text { .. }) {
        out.changed.base |= draw_title_extras(ui, clip.id, kind);
    }

    // тайм-код показывает время МАТЕРИАЛА — сказать прямо
    if matches!(kind, GeneratorKind::Timecode { .. }) {
        ui.weak(tr(k::genw::TC_NOTE));
    }
    // ФЛАГИ вида — один виджет на все формы (Р.53, п.414)
    if widgets::flags_row(ui, ("genf", clip.id.0), kind) {
        out.changed.base = true;
    }

    // ---- числа: ТА ЖЕ сетка, что у эффектов, и со связью с кривыми -----
    // (Р.59: каждый числовой ряд генератора получает и поле, и кривую)
    let at = kt_model::clip_material_at(
        clip.source_in,
        clip.start,
        clip.duration,
        clip.reversed,
        ctx.frame_step,
        ctx.playhead,
    );
    {
        let mut bind = widgets::CurveBind { curves: auto, sel: view.sel, at };
        out.changed |= widgets::params_grid_bound(
            ui,
            ("genp", clip.id.0),
            kind,
            &skip,
            Some(&mut bind),
            ctx.pubc,
        );
    }

    // ---- «применить ко всем»: отдельная команда, один шаг истории ------
    // Счётчик мест — В САМОЙ КНОПКЕ, отдельной строки под него не
    // заводим (правило 13): «мест: N» рядом с «применить ко всем (×N)»
    // было бы одним и тем же числом дважды.
    ui.horizontal(|ui| {
        let r = crate::app::hint_into(
            hover,
            ui.add_enabled(
                ctx.uses > 1,
                egui::Button::new(crate::tf!(k::genw::APPLY_ALL, ctx.uses)),
            ),
            tr(k::genw::APPLY_ALL_HINT),
        );
        crate::widgets::field_probe::note("genw.apply_all", r.rect);
        if r.clicked() {
            out.apply_all = true;
        }
    });

    // ---- кривые: ТОТ ЖЕ виджет, что у слотов эффектов ------------------
    let r = draw_gen_auto(ui, ctx, kind, auto, view, hover);
    out.changed.curve |= r.changed;
    out.seek = r.seek;
    out.axis = out.axis.merge(&r);
    out
}

/// Части формы титра, которые числом не выражаются («первое законное
/// исключение» F-TXT-001): семья, начертание, выравнивание, якорь, цвета
/// слоёв и порядок отрисовки.
fn draw_title_extras(ui: &mut egui::Ui, id: kt_model::ClipId, g: &mut GeneratorKind) -> bool {
    let mut changed = false;
    {
        let GeneratorKind::Text { family, .. } = g else { return false };
        ui.horizontal(|ui| {
            widgets::hint(&ui.label(tr(k::titlew::FAMILY)), tr(k::titlew::HINT_FAMILY));
            egui::ComboBox::from_id_salt(("title-family", id.0))
                .width(190.0)
                .selected_text(crate::widgets::elided_name(
                    ui,
                    family,
                    ui.visuals().text_color(),
                    false,
                ))
                .show_ui(ui, |ui| {
                    let vend = kt_text::VENDORED_FAMILY;
                    if ui.selectable_label(family == vend, vend).clicked() && family != vend {
                        *family = vend.to_string();
                        changed = true;
                    }
                    ui.separator();
                    for f in kt_text::system_families() {
                        if ui.selectable_label(family == f, f).clicked() && family != f {
                            family.clone_from(f);
                            changed = true;
                        }
                    }
                });
        });
    }

    let mut center = false;
    {
        let GeneratorKind::Text { bold, align, anchor, .. } = g else { unreachable!() };
        ui.horizontal(|ui| {
            // «жирный» — флаг паспорта (Р.53, п.414), но рисуется тут же,
            // рядом с выравниванием
            changed |= ui.checkbox(bold, tr(k::titlew::BOLD)).changed();
            ui.separator();
            for (a, label) in [
                (TextAlign::Left, tr(k::titlew::ALIGN_L)),
                (TextAlign::Center, tr(k::titlew::ALIGN_C)),
                (TextAlign::Right, tr(k::titlew::ALIGN_R)),
            ] {
                if ui.selectable_label(*align == a, label).clicked() && *align != a {
                    *align = a;
                    changed = true;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr(k::titlew::ANCHOR));
            egui::Grid::new(("title-anchor", id.0)).spacing([2.0, 2.0]).show(ui, |ui| {
                use TextAnchor as A;
                for anchors in [
                    [A::TopLeft, A::TopCenter, A::TopRight],
                    [A::CenterLeft, A::Center, A::CenterRight],
                    [A::BottomLeft, A::BottomCenter, A::BottomRight],
                ] {
                    for a in anchors {
                        let on = *anchor == a;
                        if ui.add_sized([18.0, 14.0], egui::Button::selectable(on, "•")).clicked()
                            && !on
                        {
                            *anchor = a;
                            changed = true;
                        }
                    }
                    ui.end_row();
                }
            });
            // «по центру» — один жест вместо трёх полей (п.415)
            let r = ui.button(tr(k::titlew::CENTER));
            widgets::hint(&r, tr(k::titlew::HINT_CENTER));
            crate::widgets::field_probe::note("titlew.center", r.rect);
            if r.clicked() {
                *anchor = TextAnchor::Center;
                center = true;
                changed = true;
            }
        });
    }
    if center {
        g.set_param("pos_x", 0.5);
        g.set_param("pos_y", 0.5);
    }

    // цвета слоёв: RGBA-пипетка + HEX. Цвета титра — ПРЯМЫЕ поля, а не
    // `ParamSpec`, поэтому и виджет свой: путь через `get_param` молча
    // вернул бы ноль, и заливка стала бы прозрачно-чёрной (п.415).
    {
        let GeneratorKind::Text {
            fill_r,
            fill_g,
            fill_b,
            fill_a,
            stroke_r,
            stroke_g,
            stroke_b,
            stroke_a,
            shadow_r,
            shadow_g,
            shadow_b,
            shadow_a,
            plate_r,
            plate_g,
            plate_b,
            plate_a,
            ..
        } = g
        else {
            unreachable!()
        };
        changed |= widgets::color_row_rgba(
            ui,
            tr(k::titlew::FILL),
            (fill_r, fill_g, fill_b, fill_a),
            Some("titlew.fill"),
        );
        changed |= widgets::color_row_rgba(
            ui,
            tr(k::titlew::STROKE),
            (stroke_r, stroke_g, stroke_b, stroke_a),
            Some("titlew.stroke"),
        );
        changed |= widgets::color_row_rgba(
            ui,
            tr(k::titlew::SHADOW),
            (shadow_r, shadow_g, shadow_b, shadow_a),
            Some("titlew.shadow"),
        );
        changed |= widgets::color_row_rgba(
            ui,
            tr(k::titlew::PLATE),
            (plate_r, plate_g, plate_b, plate_a),
            Some("titlew.plate"),
        );
    }
    changed |= order_row(ui, g);
    changed
}

/// Секция автоматизации ГЕНЕРАТОРА — зеркало `fx_editor::draw_auto_for`
/// для второго носителя кривых.
///
/// Сам виджет один (`automation::draw`, правило «своего редактора
/// кейфреймов ни у кого быть не должно»), здесь только то, что про
/// кривую как свойство источника: список свободных параметров, ease и
/// снятие кривой. Домен — материал события, ровно тот `pts`, по которому
/// кривые разрешает ИСТОЧНИК (`GenState::kind_at`): разойдись они, ключ
/// кэша перестал бы определять кадр.
fn draw_gen_auto(
    ui: &mut egui::Ui,
    ctx: &GenCtx<'_>,
    kind: &GeneratorKind,
    curves: &mut Vec<kt_model::ParamCurve>,
    view: &mut crate::automation::AutoView,
    hover: &mut Option<String>,
) -> crate::automation::AutoResult {
    let specs = kind.param_specs();
    if specs.is_empty() {
        return crate::automation::AutoResult::default();
    }
    let clip = ctx.clip;
    let src_in = clip.source_in;
    let mat_end = src_in.0 + clip.duration.0.max(1);
    let mut changed = false;
    ui.separator();
    ui.horizontal(|ui| {
        let free: Vec<&kt_model::ParamSpec> =
            specs.iter().filter(|s| !curves.iter().any(|c| c.param == s.name)).collect();
        if !free.is_empty() {
            ui.menu_button(tr(k::fx::AUTO_ADD), |ui| {
                for spec in &free {
                    let n: &str = &spec.name;
                    let label = crate::widgets::param_label_scoped(kind.params_scope(), n);
                    if ui.button(label).clicked() {
                        let v0 = kind.get_param(n).unwrap_or(spec.default);
                        curves.push(kt_model::ParamCurve {
                            param: n.to_string(),
                            ease: true,
                            keys: vec![
                                kt_model::AutoKey { t: src_in, v: v0 },
                                kt_model::AutoKey { t: kt_time::Tick(mat_end), v: v0 },
                            ],
                            source: Default::default(),
                        });
                        view.active = curves.len() - 1;
                        view.sel = None;
                        changed = true;
                    }
                }
            });
        }
        if let Some(c) = curves.get_mut(view.active) {
            changed |= ui.checkbox(&mut c.ease, tr(k::fx::EASE)).changed();
            if ui.button(tr(k::fx::AUTO_REMOVE)).clicked() {
                let i = view.active;
                curves.remove(i);
                view.active = 0;
                view.sel = None;
                changed = true;
            }
        }
    });
    let frame_step = ctx.frame_step;
    let to_tl = |t: kt_time::Tick| {
        kt_model::clip_timeline_at(
            clip.source_in,
            clip.start,
            clip.duration,
            clip.reversed,
            frame_step,
            t,
        )
    };
    let to_mat = |t: kt_time::Tick| {
        kt_model::clip_material_at(
            clip.source_in,
            clip.start,
            clip.duration,
            clip.reversed,
            frame_step,
            t,
        )
    };
    let actx = crate::automation::AutoCtx {
        specs,
        range: (src_in, kt_time::Tick(mat_end)),
        view: crate::automation::view_in_curve(&to_mat, ctx.tl_view),
        to_timeline: &to_tl,
        to_curve: &to_mat,
        frame_step,
        linked: ctx.axis_linked,
    };
    let mut res = crate::automation::draw(
        ui,
        ("genauto", clip.id.0),
        &actx,
        curves,
        view,
        ctx.playhead,
        hover,
    );
    res.changed |= changed;
    res
}

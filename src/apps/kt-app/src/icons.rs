//! Иконки Phosphor (вендоренный TTF в assets/, кодпоинты PUA).
//! Дефолтные шрифты egui не имеют глифов 🗐🚩🎬💾⏮ и т.п. — кнопки
//! рисовались квадратами (п.23). Шрифт ставится в install_fonts().

use eframe::egui;

pub const PLAY: &str = "\u{E3D0}";
pub const PAUSE: &str = "\u{E39E}";
pub const SKIP_BACK: &str = "\u{E5A4}";
pub const FLAG: &str = "\u{E244}";
pub const SCISSORS: &str = "\u{EAE0}";
pub const COPY: &str = "\u{E1CA}";
pub const CLIPBOARD: &str = "\u{E198}";
pub const TRASH: &str = "\u{E4A6}";
pub const TRIM_START: &str = "\u{E062}"; // arrow-line-left
pub const TRIM_END: &str = "\u{E064}"; // arrow-line-right
pub const LANE_UP: &str = "\u{E522}"; // arrow-fat-line-up
pub const LANE_DOWN: &str = "\u{E51C}"; // arrow-fat-line-down
pub const MINUS: &str = "\u{E32A}";
pub const PLUS: &str = "\u{E3D4}";
/// «Ключ ДО курсора» / «ключ ПОСЛЕ курсора» (Р.40 п.18): ставят ключ на
/// соседний кадр, закрепляя предыдущий или следующий участок кривой.
pub const KEY_BEFORE: &str = "\u{E062}"; // arrow-line-left
pub const KEY_AFTER: &str = "\u{E064}"; // arrow-line-right
pub const REGION_IN: &str = "\u{E85E}"; // brackets-square
pub const REGION_OUT: &str = "\u{E862}"; // brackets-angle
pub const CLEAR: &str = "\u{E4F6}"; // x
pub const LOOP: &str = "\u{E3F6}"; // repeat
pub const SELECTION: &str = "\u{E69A}"; // selection (рамка выделения событий)
pub const AREA_SAVE: &str = "\u{E69C}"; // selection-plus (сохранить область, Р.35)
pub const UNDO: &str = "\u{E08A}"; // arrow-u-up-left
pub const REDO: &str = "\u{E08C}"; // arrow-u-up-right
pub const RENDER: &str = "\u{E8C2}"; // film-slate
pub const SAVE: &str = "\u{E248}"; // floppy-disk
pub const FX: &str = "\u{E2DE}"; // lightning (клип с эффектами)
pub const WARN: &str = "\u{E4E0}"; // warning (E4DE был voicemail — п.1 р.8)
pub const SEARCH: &str = "\u{E30C}"; // magnifying-glass (п.3 р.10)
pub const GAUGE: &str = "\u{E000}"; // pulse (gauge E628 читался как робот Reddit — п.5 р.10)
pub const SQUARES_FOUR: &str = "\u{E464}"; // сетка (режим бина)
pub const LIST: &str = "\u{E2F0}"; // список; он же «гамбургер» узкого тулбара
pub const CHECK: &str = "\u{E182}"; // галка в пунктах меню (≡/✓ из юникода — тофу, т.100)
#[allow(dead_code)] // задействуется группами бина (п.13 р.11)
pub const LOCK: &str = "\u{E2FA}"; // замок (группы бина)
pub const ARROWS_OUT: &str = "\u{E0A2}"; // свободная раскладка
pub const SKIP_FWD: &str = "\u{E5A6}"; // к следующей точке

/// Вендоренные шрифты: имя в egui и байты. Порядок здесь = порядок фолбэка,
/// и менять его НЕЛЬЗЯ.
///
/// Сабсеты идут ПОСЛЕ дефолтных семей и после Phosphor: латиница и кириллица
/// обязаны остаться за Ubuntu-Light (Noto Sans CJK содержит и их тоже), а
/// PUA-иконки — за Phosphor. На этом же порядке держится точность предиката
/// «атом арабского начертания» в `i18n::bidi` — там он выведен из того, что
/// арабские кодпоинты есть ровно в одном загруженном шрифте.
///
/// Сабсеты собирает `scripts/subset-fonts.py` по символам `i18n/*.toml`
/// (Р.19): целиком Noto Sans CJK весит ~19 МБ, обрезанный — 75 КБ.
pub const VENDORED: [(&str, &[u8]); 3] = [
    ("phosphor", include_bytes!("../assets/Phosphor.ttf")),
    ("noto_sc", include_bytes!("../assets/NotoSansSC-kt.otf")),
    ("noto_ar", include_bytes!("../assets/NotoSansArabic-kt.ttf")),
];

/// Подключить вендоренные шрифты к egui (фолбэком к обеим семьям).
///
/// Все языки грузятся ВСЕГДА, а не по текущему: 136 КБ на оба сабсета дешевле,
/// чем `set_fonts` в пути переключения языка и кадр с квадратами после него.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in VENDORED {
        fonts
            .font_data
            .insert(name.into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
        // обе семьи: bin_panel и тулбар строят и моноширинные FontId
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            if let Some(list) = fonts.families.get_mut(&family) {
                list.push(name.into());
            }
        }
    }
    ctx.set_fonts(fonts);
}
#[allow(dead_code)] // перестановка слотов теперь мышью (Р.33)
pub const CARET_LEFT: &str = "\u{E138}";
pub const CARET_RIGHT: &str = "\u{E13A}";
#[allow(dead_code)] // слоты выключаются галкой (Р.33)
pub const POWER: &str = "\u{E3DA}";
pub const BELL: &str = "\u{E0CE}";

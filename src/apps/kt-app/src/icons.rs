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
pub const REGION_IN: &str = "\u{E85E}"; // brackets-square
pub const REGION_OUT: &str = "\u{E862}"; // brackets-angle
pub const CLEAR: &str = "\u{E4F6}"; // x
pub const LOOP: &str = "\u{E3F6}"; // repeat
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

/// Подключить Phosphor к egui-шрифтам (fallback к proportional).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "phosphor".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/Phosphor.ttf"))),
    );
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.push("phosphor".into());
    }
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        list.push("phosphor".into());
    }
    ctx.set_fonts(fonts);
}
pub const CARET_LEFT: &str = "\u{E138}";
pub const CARET_RIGHT: &str = "\u{E13A}";
pub const POWER: &str = "\u{E3DA}";
pub const BELL: &str = "\u{E0CE}";

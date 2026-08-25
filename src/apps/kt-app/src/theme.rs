//! Палитра и тема (п.23 р.6, решение пользователя: темнее + моноширинные
//! цифры + минимум скруглений/градиентов, но НЕ плоско — глубина, акценты
//! и glow-эффекты остаются). Ключевые цвета собраны здесь; исторические
//! литералы в timeline/meters вытягиваются сюда по мере надобности.

use eframe::egui::{self, Color32};

/// Фон панелей — почти чёрный.
pub const PANEL: Color32 = Color32::from_rgb(14, 15, 17);
/// Фон окон/попапов.
pub const WINDOW: Color32 = Color32::from_rgb(19, 20, 23);
/// Самый тёмный (поля ввода, «колодцы»).
pub const EXTREME: Color32 = Color32::from_rgb(8, 9, 11);
/// 1px швы между панелями.
pub const SEAM: Color32 = Color32::from_rgb(38, 41, 46);
/// Акцент (выделение, активные элементы).
pub const ACCENT: Color32 = Color32::from_rgb(62, 110, 200);
/// Ховер-заливка.
pub const HOVER: Color32 = Color32::from_rgb(40, 44, 51);
/// Рамка превью-кадров филмстрипа.
pub const THUMB_FRAME: Color32 = Color32::from_rgb(0, 0, 0);

/// Общая настройка визуала egui (вызов из run()).
pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = WINDOW;
    v.extreme_bg_color = EXTREME;
    // минимум скруглений — «брутально», но не нулевые (не картон)
    v.widgets.noninteractive.corner_radius = 1.into();
    v.widgets.inactive.corner_radius = 1.into();
    v.widgets.hovered.corner_radius = 1.into();
    v.widgets.active.corner_radius = 1.into();
    v.window_corner_radius = 2.into();
    v.selection.bg_fill = ACCENT;
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.inactive.bg_fill = Color32::from_rgb(26, 28, 32);
    // жёсткие 1px границы виджетов вместо мягких
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, SEAM);
    ctx.set_visuals(v);
    ctx.all_styles_mut(|st| {
        st.spacing.item_spacing = egui::vec2(6.0, 3.0);
        st.spacing.button_padding = egui::vec2(5.0, 2.0);
        st.spacing.window_margin = 4.into();
        st.spacing.menu_margin = 4.into();
    });
}

/// Фрейм панелей без воздуха по краям (п.22 р.6) + 1px шов.
pub fn panel_frame() -> egui::Frame {
    egui::Frame::new().fill(PANEL).inner_margin(2)
}

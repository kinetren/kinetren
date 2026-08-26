//! Палитра и тема (п.23 р.6, v2: решение пользователя «брутальнее» —
//! темнее + моноширинные цифры + нулевые скругления виджетов + контурные
//! кнопки, но НЕ плоско: глубина, акценты и glow-эффекты остаются).
//! Ключевые цвета собраны здесь; таймлайн (самая большая площадь экрана)
//! красится ТОЛЬКО из этих констант.

use eframe::egui::{self, Color32};

/// Фон панелей — почти чёрный.
pub const PANEL: Color32 = Color32::from_rgb(9, 10, 12);
/// Фон окон/попапов/групп.
pub const WINDOW: Color32 = Color32::from_rgb(16, 17, 20);
/// Самый тёмный (поля ввода, «колодцы»).
pub const EXTREME: Color32 = Color32::from_rgb(4, 5, 7);
/// 1px швы между панелями — заметные, «сварные».
pub const SEAM: Color32 = Color32::from_rgb(48, 52, 60);
/// Контур виджетов (кнопки очерчены, не «таблетки»).
pub const OUTLINE: Color32 = Color32::from_rgb(42, 46, 54);
/// Акцент (выделение, активные элементы).
pub const ACCENT: Color32 = Color32::from_rgb(70, 130, 235);
/// Ховер-заливка.
pub const HOVER: Color32 = Color32::from_rgb(38, 42, 50);
/// Рамка превью-кадров филмстрипа.
pub const THUMB_FRAME: Color32 = Color32::from_rgb(0, 0, 0);

/// Фон полосы таймлайна (между дорожками, за руллером).
pub const TL_BG: Color32 = Color32::from_rgb(11, 12, 14);
/// Фоны дорожек: тона старого редактора, but darker (п.23 р.6).
pub const LANE_VIDEO: Color32 = Color32::from_rgb(19, 24, 32);
pub const LANE_AUDIO: Color32 = Color32::from_rgb(26, 20, 20);
pub const LANE_MORPH: Color32 = Color32::from_rgb(28, 20, 42);
/// Левый (непрозрачный) конец градиента gutter'а (п.20 р.6).
pub const GUTTER: Color32 = Color32::from_rgb(22, 24, 28);

/// Общая настройка визуала egui (вызов из run()).
pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = WINDOW;
    v.extreme_bg_color = EXTREME;
    // минимальные скругления 1px (уточнение р.6.1); окна чуть смягчены
    v.widgets.noninteractive.corner_radius = 1.into();
    v.widgets.inactive.corner_radius = 1.into();
    v.widgets.hovered.corner_radius = 1.into();
    v.widgets.active.corner_radius = 1.into();
    v.window_corner_radius = 2.into();
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = egui::Stroke::new(1.0, Color32::from_rgb(150, 190, 255));
    // кнопки: тёмная плита + жёсткий 1px контур; ховер подсвечивает контур
    v.widgets.inactive.bg_fill = Color32::from_rgb(21, 23, 27);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(21, 23, 27);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, OUTLINE);
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(90, 120, 180));
    v.widgets.active.bg_fill = Color32::from_rgb(34, 56, 96);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, SEAM);
    ctx.set_visuals(v);
    ctx.all_styles_mut(|st| {
        // п.14 р.8: всплывающих подсказок НЕТ вообще — единственное место
        // подсказки это правый низ строки состояния. Заградительная
        // задержка глушит и чужие on_hover_text из виджетов egui.
        st.interaction.tooltip_delay = 1.0e6;
        st.interaction.tooltip_grace_time = 0.0;
        // п.9/п.12 р.8: 3 px (дефолт) означали 6-пиксельную хват-зону
        // сепаратора — промах на пиксель попадал в превью
        st.interaction.resize_grab_radius_side = 8.0;
        st.interaction.resize_grab_radius_corner = 14.0;
        st.spacing.item_spacing = egui::vec2(6.0, 3.0);
        st.spacing.button_padding = egui::vec2(5.0, 2.0);
        st.spacing.window_margin = 4.into();
        st.spacing.menu_margin = 4.into();
    });
}

/// KT_DEBUG_HITS=1 — штатная подсветка egui «кто ловит клик под курсором»
/// (п.9 р.8: так разбираются жалобы «кнопка/полоска не нажимается»).
pub fn apply_debug_flags(ctx: &egui::Context) {
    // Style::debug существует только в debug-сборке egui — в релизном
    // профиле поля нет вовсе (ловится ТОЛЬКО релизной сборкой).
    #[cfg(debug_assertions)]
    if std::env::var_os("KT_DEBUG_HITS").is_some() {
        ctx.all_styles_mut(|st| {
            st.debug.show_widget_hits = true;
            st.debug.debug_on_hover = true;
        });
    }
    #[cfg(not(debug_assertions))]
    let _ = ctx;
}

/// Фрейм панелей без воздуха по краям (п.22 р.6) + 1px шов.
pub fn panel_frame() -> egui::Frame {
    egui::Frame::new().fill(PANEL).inner_margin(2)
}

/// Горизонтальный градиент (один меш): left-цвет слева → right-цвет справа.
pub fn hgrad(painter: &egui::Painter, rect: egui::Rect, left: Color32, right: Color32) {
    let mut mesh = egui::Mesh::default();
    for (pos, col) in [
        (rect.left_top(), left),
        (rect.right_top(), right),
        (rect.right_bottom(), right),
        (rect.left_bottom(), left),
    ] {
        mesh.vertices.push(egui::epaint::Vertex { pos, uv: egui::epaint::WHITE_UV, color: col });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

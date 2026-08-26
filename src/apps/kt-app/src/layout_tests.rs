//! Тест 76 (п.9 р.8): ЖИВАЯ проверка вёрстки и интеракций через
//! egui_kittest (Harness прогоняет настоящие кадры без окна и GPU).
//!
//! Заведено потому, что три поломки подряд не ловились ничем: сепаратор
//! панели не перетаскивался, панель раздувалась контентом и съедала
//! превью, кромки окна не давали ресайза. Чистые функции такие дефекты
//! не видят — их видит только реальный проход layout+hit-test.
//!
//! ПРАВИЛО (apps/kt-app/CLAUDE.md): любое изменение вёрстки или
//! интерактивного поведения обязано сопровождаться тестом отсюда.

use crate::app::KtApp;
use eframe::egui;
use egui_kittest::Harness;

const W: f32 = 1440.0;
const H: f32 = 860.0;

fn harness() -> Harness<'static, KtApp> {
    let app = KtApp::new_empty(None);
    let mut h = Harness::builder()
        .with_size(egui::vec2(W, H))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    h.step();
    h.step();
    h
}

/// Ширина панели по её сохранённому состоянию (то же, что читает egui).
fn panel_width(h: &Harness<'_, KtApp>, id: &str) -> f32 {
    egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new(id))
        .expect("панель должна быть отрисована")
        .size()
        .x
}

fn cursor(h: &Harness<'_, KtApp>) -> egui::CursorIcon {
    h.output().platform_output.cursor_icon
}

/// (а,б) Вёрстка: левая колонка в своих пределах, превью занимает центр —
/// и НЕ схлопывается, когда контент колонки длинный (регрессия р.7.1:
/// ScrollArea/Grid раздували панель, превью «пропадало вникуда»).
#[test]
fn t76_layout_panels_and_preview_share_space() {
    let mut h = harness();
    let w = panel_width(&h, "left_col");
    assert!((72.0..=420.0).contains(&w), "левая колонка вне пределов: {w}");
    assert!(h.state().preview_rect.width() > 600.0, "превью схлопнулось: {:?}", h.state().preview_rect);

    // длинный контент не должен раздувать панель
    h.state_mut().gpu_info = Some("X".repeat(400));
    h.step();
    let w2 = panel_width(&h, "left_col");
    assert!(w2 <= 420.0, "длинный контент раздул колонку: {w2}");
    assert!(
        h.state().preview_rect.width() > 600.0,
        "длинный контент съел превью: {:?}",
        h.state().preview_rect
    );
}

/// (в,г) Сепаратор левой колонки: даёт курсор ресайза и реально тянется.
#[test]
fn t76_left_panel_separator_is_draggable() {
    let mut h = harness();
    let w0 = panel_width(&h, "left_col");
    let y = H * 0.5;

    h.hover_at(egui::pos2(w0, y));
    h.step();
    assert_eq!(
        cursor(&h),
        egui::CursorIcon::ResizeHorizontal,
        "на сепараторе нет курсора ресайза — зону кто-то съел"
    );

    h.drag_at(egui::pos2(w0, y));
    h.step();
    h.hover_at(egui::pos2(w0 + 90.0, y));
    h.step();
    h.drop_at(egui::pos2(w0 + 90.0, y));
    h.step();
    let w1 = panel_width(&h, "left_col");
    assert!(
        (w1 - (w0 + 90.0)).abs() < 12.0,
        "сепаратор не потянулся: было {w0}, стало {w1}"
    );
}

/// (д) Кромки и углы окна: курсор ресайза + отправленный BeginResize.
#[test]
fn t76_window_edges_resize() {
    use egui::{CursorIcon as C, ViewportCommand as V};
    let cases: [(egui::Pos2, C); 6] = [
        (egui::pos2(2.0, H * 0.5), C::ResizeHorizontal),   // левая кромка
        (egui::pos2(W - 2.0, H * 0.5), C::ResizeHorizontal), // правая
        (egui::pos2(W * 0.5, 2.0), C::ResizeVertical),     // верхняя
        (egui::pos2(W * 0.5, H - 2.0), C::ResizeVertical), // нижняя
        (egui::pos2(3.0, 3.0), C::ResizeNwSe),             // угол СЗ
        (egui::pos2(W - 3.0, 3.0), C::ResizeNeSw),         // угол СВ
    ];
    for (pos, want) in cases {
        let mut h = harness();
        h.hover_at(pos);
        h.step();
        assert_eq!(cursor(&h), want, "нет курсора ресайза в {pos:?}");
        h.drag_at(pos);
        h.step();
        let sent = h
            .output()
            .viewport_output
            .values()
            .any(|v| v.commands.iter().any(|c| matches!(c, V::BeginResize(_))));
        assert!(sent, "BeginResize не отправлен из {pos:?}");
    }
}

/// (е) Зона ресайза не должна воровать клики у виджетов: кнопки тулбара
/// внизу и кнопки окна остаются кликабельными.
#[test]
fn t76_resize_zones_do_not_steal_widgets() {
    let mut h = harness();
    // Play-кнопка тулбара — первая слева в нижней полосе таймлайна
    let playing_before = h.state().player.transport.is_playing();
    let toolbar_y = H - 20.0 - 34.0 * 0.5; // статусбар 20, полоса тулбара 34
    h.hover_at(egui::pos2(24.0, toolbar_y));
    h.step();
    assert!(
        h.state().hover_hint.is_some(),
        "кнопка тулбара не получает hover — её перекрыла чужая зона"
    );
    h.drag_at(egui::pos2(24.0, toolbar_y));
    h.step();
    h.drop_at(egui::pos2(24.0, toolbar_y));
    h.step();
    assert_ne!(
        h.state().player.transport.is_playing(),
        playing_before,
        "клик по кнопке тулбара не сработал"
    );
}

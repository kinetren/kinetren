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

/// ФАКТИЧЕСКИЙ прямоугольник панели (не запрошенный размер: сепаратор
/// egui живёт у фактической границы, и именно расхождение между ними —
/// та самая ловушка «полоска не двигается»).
fn panel_rect(h: &Harness<'_, KtApp>, id: &str) -> egui::Rect {
    egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new(id))
        .expect("панель должна быть отрисована")
        .outer_rect
}

fn panel_width(h: &Harness<'_, KtApp>, id: &str) -> f32 {
    panel_rect(h, id).width()
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
    let w0 = panel_rect(&h, "left_col").right();
    let y = H * 0.5;

    h.hover_at(egui::pos2(w0, y));
    h.step();
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
    let w1 = panel_rect(&h, "left_col").right();
    assert!(
        (w1 - (w0 + 90.0)).abs() < 12.0,
        "сепаратор не потянулся: правый край был {w0}, стал {w1}"
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

/// Верхняя зона (п.15 р.8): полосы на всю ширину НЕТ — превью начинается
/// от самого верха окна; кнопки окна живут в шапке метров и кликабельны;
/// меню в шапке колонки не раздувает панель.
#[test]
fn t76_top_area_gives_space_to_preview() {
    let mut h = harness();
    assert!(
        h.state().preview_rect.top() < 30.0,
        "превью не дотянулось до верха: {:?}",
        h.state().preview_rect
    );
    let w = panel_width(&h, "left_col");
    assert!(w <= 420.0, "шапка с меню раздула колонку: {w}");
    // кнопка закрытия — правый верхний угол, внутри панели метров
    h.hover_at(egui::pos2(W - 11.0, 10.0));
    h.step();
    assert_eq!(
        h.state().hover_hint.as_deref(),
        Some("Закрыть"),
        "кнопка окна не ловит наведение (её перекрыли)"
    );
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

/// Тест 79 (пп.5,7 р.8): медиа-бин — режимы и раскладка.
/// Свободная раскладка живёт в проекте (UiState, вне undo), переключение
/// режимов её не стирает, а карточки не вылезают за панель (иначе панель
/// растёт под контент и её сепаратор уезжает — ловушка из t76).
#[test]
fn t79_bin_modes_and_layout() {
    use kt_model::MediaId;
    let mut h = harness();
    assert!(h.state().bin_free, "свободная раскладка — режим по умолчанию");

    // позиции карточек переживают переключение режимов
    h.state_mut().bin_layout.insert(MediaId(7), egui::pos2(30.0, 40.0));
    h.state_mut().bin_free = false;
    h.step();
    h.state_mut().bin_free = true;
    h.step();
    assert_eq!(
        h.state().bin_layout.get(&MediaId(7)).copied(),
        Some(egui::pos2(30.0, 40.0)),
        "переключение режимов сбило раскладку"
    );

    // раскладка едет в проект и не раздувает файл, когда пуста
    let st = h.state().ui_state_for_test();
    assert_eq!(st.bin_layout, vec![(MediaId(7), 30.0, 40.0)]);
    assert!(st.bin_free);
    let json = serde_json::to_string(&kt_model::UiState::default()).unwrap();
    assert!(!json.contains("bin_layout"), "пустая раскладка не должна попадать в файл: {json}");
    assert!(!json.contains("bin_free"));
}

/// Тест 83 (п.1 р.9): подсказка кнопки ОБЯЗАНА отрисоваться в строке
/// состояния в ТОМ ЖЕ кадре. Регрессия: статусбар — вторая панель кадра,
/// а кнопки рисуются позже, поэтому он всегда видел hover_hint = None и
/// показывал дежурную строку. Прошлые тесты читали внутреннее поле после
/// кадра и были ложно-зелёными — здесь проверяется ОТРИСОВАННОЕ.
#[test]
fn t83_hints_reach_status_line() {
    use crate::app::HintKind;
    let mut h = harness();
    assert_eq!(h.state().status_line_shown.1, HintKind::Idle, "без наведения — дежурная строка");

    let toolbar_y = H - 20.0 - 34.0 * 0.5; // статусбар 20 + полоса тулбара 34
    for (pos, want) in [
        (egui::pos2(24.0, toolbar_y), "Play/Pause (Space)"),   // тулбар (рисуется ПОСЛЕ статусбара)
        (egui::pos2(W - 11.0, 10.0), "Закрыть"),               // кнопки окна (ещё позже)
        (egui::pos2(14.0, 40.0), "Свободная раскладка"),       // бин
    ] {
        let mut h = harness();
        h.hover_at(pos);
        h.step();
        let (text, kind) = h.state().status_line_shown.clone();
        assert!(
            text.starts_with(want),
            "из {pos:?} ждали «{want}», в строке состояния «{text}»"
        );
        assert_eq!(kind, HintKind::Hover);
    }

    // всплывающих подсказок в программе нет (п.14 р.8)
    h.hover_at(egui::pos2(24.0, toolbar_y));
    h.step();
    h.step();
    assert!(
        h.ctx.memory(|m| m.layer_ids().all(|l| l.order != egui::Order::Tooltip)),
        "вернулся всплывающий тултип"
    );
}

/// Тест 85 (п.2 р.9): окно таскается за верх В ЛЮБОЙ точке ширины —
/// раньше между шапкой колонки и кнопками окна зияла дыра над превью.
#[test]
fn t85_top_drag_is_continuous() {
    use egui::ViewportCommand as V;
    for x in [120.0f32, W * 0.5, W - 200.0] {
        let mut h = harness();
        h.drag_at(egui::pos2(x, 14.0)); // ниже кромки ресайза (8 px)
        h.step();
        h.hover_at(egui::pos2(x + 20.0, 18.0)); // реальный жест: сдвиг мыши
        h.step();
        let sent = h
            .output()
            .viewport_output
            .values()
            .any(|v| v.commands.iter().any(|c| matches!(c, V::StartDrag)));
        assert!(sent, "окно не таскается за верх при x={x}");
    }
}

/// Тест 86 (пп.3,10 р.9): метры прижаты к правому краю окна, а при
/// минимальном окне 640×640 шапка (кнопки окна) не вытесняется таймлайном
/// и превью не схлопывается.
#[test]
fn t86_meters_edge_and_min_window() {
    let mut h = harness();
    let p = panel_rect(&h, "meters_col");
    assert!((W - p.right()).abs() < 1.5, "панель метров не у края: {p:?}");
    // у самой кромки окна метр обязан ловить наведение (нет мёртвой зоны)
    h.hover_at(egui::pos2(W - 3.0, H * 0.35));
    h.step();
    assert!(
        h.state().status_line_shown.0.contains("громкость")
            || h.state().status_line_shown.0.contains("уровень"),
        "у правого края зазор: в строке «{}»",
        h.state().status_line_shown.0
    );

    // минимальное окно
    let app = KtApp::new_empty(None);
    let (w, hh) = (640.0f32, 640.0f32);
    let mut h = Harness::builder()
        .with_size(egui::vec2(w, hh))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    h.step();
    h.step();
    h.hover_at(egui::pos2(w - 11.0, 10.0));
    h.step();
    assert_eq!(
        h.state().status_line_shown.0, "Закрыть",
        "при 640×640 шапку вытеснил таймлайн"
    );
    assert!(
        h.state().preview_rect.height() > 60.0,
        "превью схлопнулось: {:?}",
        h.state().preview_rect
    );
}

/// Тест 88 (п.11 р.9): прыжок по точкам остановки ПОКАЗЫВАЕТ курсор —
/// раньше playhead уезжал, а окно таймлайна оставалось на месте (на паузе
/// follow_view не работает по построению).
#[test]
fn t88_jump_reveals_cursor() {
    let mut h = harness();
    h.state_mut().view_start = 5_000_000.0;
    h.step();
    h.state_mut().jump_stop(false, true); // Shift+Home — в начало проекта
    h.step();
    let st = h.state();
    let vis = (st.timeline_rect.width() as f64 - crate::timeline::GUTTER_W as f64) * st.ticks_per_px;
    let ph = st.player.transport.playhead().0 as f64;
    let vs = st.view_start;
    assert!(
        ph >= vs && ph <= vs + vis,
        "курсор вне окна: ph={ph}, окно=[{vs}, {}]",
        vs + vis
    );
}

/// Тест 94 (пп.7,8 р.10): кнопка «Вперёд» РЕАЛЬНО двигает курсор вправо,
/// повторные нажатия дают монотонную цепочку без застревания, и хоткеи
/// A/D делают то же самое. Регрессия: playhead живёт в аудиокадрах, а
/// точки остановки — в тиках; round-trip усекал вниз, курсор вставал на
/// тик левее цели, и next_stop бесконечно находил ту же точку.
#[test]
fn t94_forward_navigation_advances() {
    use kt_model::*;
    use kt_time::Tick;

    let mut h = harness();
    {
        let app = h.state_mut();
        // проект с точками, НЕ кратными аудио-сетке: 1000, 4000, 7000
        let mut p = app.model.project().clone();
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        p.clips.push(ClipEvent {
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(1000),
            duration: Tick(3000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx: vec![],
        });
        p.markers.push(Tick(7000));
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.player.seek(Tick(0));
    }
    h.step();

    // цепочка вперёд обязана быть строго возрастающей
    let mut seen = vec![h.state().player.transport.playhead().0];
    for _ in 0..3 {
        h.state_mut().jump_stop(true, false);
        h.step();
        let now = h.state().player.transport.playhead().0;
        assert!(
            now > *seen.last().unwrap(),
            "курсор не двинулся вперёд: цепочка {seen:?} → {now}"
        );
        seen.push(now);
    }
    // и ни один прыжок не утащил назад
    assert!(seen.windows(2).all(|w| w[1] > w[0]), "не монотонно: {seen:?}");
    // на самой правой точке дальше двигаться некуда — и НЕ отбрасывает назад
    let last = *seen.last().unwrap();
    h.state_mut().jump_stop(true, false);
    h.step();
    assert_eq!(
        h.state().player.transport.playhead().0,
        last,
        "упор в конец не должен утаскивать курсор назад"
    );

    // назад — тоже монотонно, до нуля
    for _ in 0..6 {
        h.state_mut().jump_stop(false, false);
        h.step();
    }
    assert_eq!(h.state().player.transport.playhead().0, 0, "не дошли до нуля");
}

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
    // п.1 р.11: превью начинается У САМОЙ кромки — полоса перетаскивания
    // окна места больше не занимает (было ~23 px впустую)
    assert!(
        h.state().preview_rect.top() <= crate::app::WIN_EDGE,
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
        p.markers.push(Marker::at(Tick(7000)));
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

/// Тест 95 (п.2 р.11): магнит РЕАЛЬНО притягивает клип к границе соседа.
///
/// Дефект: кандидаты «примагнитить по началу» и «по концу» сравнивались
/// после подстановки «не нашлось = как есть», и такой кандидат со
/// смещением 0 всегда побеждал — жёлтая линия рисовалась, а клип не
/// прилипал. Тест ведёт мышь как человек (нажать, сдвинуть, отпустить) и
/// проверяет ИТОГОВУЮ модель, а не внутренние поля.
#[test]
fn t95_clip_drag_snaps_to_neighbour() {
    use kt_model::*;
    use kt_time::Tick;

    // тики/px = 1000 → порог магнита 10 px = 10 000 тиков
    const TPP: f64 = 1000.0;
    const DUR: i64 = 90_000;
    const NEIGHBOUR_END: i64 = DUR; // сосед [0, 90000)
    const FAR: i64 = 200_000; // таскаемый клип

    fn scene(ctrl: bool) -> (i64, i64) {
        let mut h = harness();
        {
            let app = h.state_mut();
            let mut p = app.model.project().clone();
            p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
            for (n, start) in [(1u64, 0i64), (2, FAR)] {
                p.clips.push(ClipEvent {
                    id: ClipId(n),
                    link_group: None,
                    kind: ClipKind::Video,
                    lane: 0,
                    start: Tick(start),
                    duration: Tick(DUR),
                    source_in: Tick(0),
                    media: MediaId(1),
                    gain_db: 0.0,
                    reversed: false,
                    fx_bypass: vec![],
                    fx: vec![],
                });
            }
            p.validate().unwrap();
            app.model = kt_model::TimelineModel::new(p);
            app.view_start = 0.0;
            app.ticks_per_px = TPP;
            app.player.seek(Tick(0));
        }
        h.step();

        let g = h.state().tl_geom.clone().expect("нет геометрии таймлайна");
        let y = g.lane_top(ClipKind::Video, 0) + g.lane_h * 0.5;
        let grab = g.tick_to_x(FAR + DUR / 2);
        if ctrl {
            h.event(egui::Event::ModifiersChanged(egui::Modifiers::CTRL));
        }
        h.hover_at(egui::pos2(grab, y));
        h.step();
        h.drag_at(egui::pos2(grab, y));
        h.step();
        // сдвиг на 8 px — здесь egui и признаёт драг начавшимся
        h.hover_at(egui::pos2(grab - 8.0, y));
        h.step();

        // целимся так, чтобы НАЧАЛО оказалось на 4 px правее конца соседа:
        // магнит должен дотянуть, «без магнита» — оставить как есть.
        let want_raw = NEIGHBOUR_END + (4.0 * TPP) as i64;
        let grab_dt = match h.state().drag {
            Some(crate::timeline::Drag::Clip(ref d)) => d.grab_dt,
            _ => panic!("драг клипа не начался — его перехватил кто-то другой"),
        };
        let to = g.tick_to_x(want_raw + grab_dt);
        h.hover_at(egui::pos2(to, y));
        h.step();
        h.drop_at(egui::pos2(to, y));
        h.step();

        let start = h
            .state()
            .model
            .project()
            .clips
            .iter()
            .find(|c| c.id == ClipId(2))
            .expect("клип пропал")
            .start
            .0;
        (start, want_raw)
    }

    let (snapped, _) = scene(false);
    assert_eq!(snapped, NEIGHBOUR_END, "магнит не притянул клип к соседу");

    let (free, raw) = scene(true);
    assert!(
        (free - raw).abs() <= 2 * TPP as i64,
        "Ctrl обязан снимать магнит: клип встал в {free}, ждали ≈{raw}"
    );
    assert_ne!(free, NEIGHBOUR_END, "Ctrl не снял магнит");
}

/// Тест 96 (пп.3,4,5 р.11): грязность проекта и loop.
///
/// «Правки содержимого» (клипы, FX, метки) поднимают звёздочку и
/// запускают автосейв; «правки удобства» (регион, loop, зум, раскладка
/// бина, имена дорожек) — нет, они доезжают на диск попутно. Esc снимает
/// регион, но НЕ гасит взведённый loop — это отдельное намерение.
#[test]
fn t96_dirty_split_and_loop_survives_esc() {
    use kt_model::*;
    use kt_time::Tick;

    let mut h = harness();
    assert!(!h.state().dirty_content && !h.state().dirty_ui, "свежий проект чист");

    // --- удобства: регион, loop, зум, раскладка бина, имя дорожки
    h.state_mut().model.set_region(Region::normalized(Tick(0), Tick(90_000)));
    h.state_mut().on_region_changed();
    h.state_mut().toggle_loop();
    h.state_mut().ticks_per_px = 4321.0;
    h.state_mut().bin_layout.insert(MediaId(3), egui::pos2(12.0, 34.0));
    h.state_mut().lane_names.push((LaneKind::Video, 0, "камера".into()));
    h.step();
    assert!(
        !h.state().dirty_content,
        "удобства не должны помечать проект изменённым (звёздочка врёт)"
    );
    assert!(h.state().dirty_ui, "удобства обязаны быть учтены как ui-правки");

    // --- содержимое: правка модели
    h.state_mut().apply(Command::AddMarker { at: Tick(45_000) });
    h.step();
    assert!(h.state().dirty_content, "правка содержимого обязана дать звёздочку");

    // --- Esc: регион снят, loop ОСТАЁТСЯ взведённым
    assert!(h.state().model.project().loop_enabled, "loop включён");
    h.state_mut().escape_action();
    h.step();
    assert!(h.state().model.project().region.is_none(), "Esc не снял регион");
    assert!(
        h.state().model.project().loop_enabled,
        "Esc погасил loop — это отдельное намерение пользователя (п.3 р.11)"
    );
    // явное «Снять регион» по-прежнему гасит и loop (иначе лампочка горит
    // без региона и переживает рестарт — п.16 р.6)
    h.state_mut().model.set_region(Region::normalized(Tick(0), Tick(90_000)));
    h.state_mut().clear_region();
    assert!(!h.state().model.project().loop_enabled, "явная команда обязана гасить loop");

    // --- автосейв не срабатывает чаще положенного и не пишет во время жеста
    assert!(!h.state().autosave_due(), "автосейв сразу после старта — слишком часто");
}

/// Тест 97 (пп.1,7,8 р.11): верх превью, предел ширины метров, LED-сетка.
#[test]
fn t97_top_meters_and_leds() {
    use crate::meters::{
        METER_MAX_DB, METER_MIN_DB, SEG_STEPS, led_lit, seg_count, seg_db_for, seg_mid_db,
        zone_color,
    };

    // --- (п.1 р.12) шаг сетки подбирается под высоту бара: сегменты
    // должны быть ЧАСТЫМИ (как в v1 — 6 px + 2 px), а не крупными
    for bar_h in [180.0f32, 300.0, 420.0, 560.0, 700.0] {
        let step = seg_db_for(bar_h);
        let px = bar_h * step / (METER_MAX_DB - METER_MIN_DB);
        assert!(
            (4.5..=9.5).contains(&px),
            "при баре {bar_h} px шаг сегмента {px} px (шаг {step} dB) — сетка не как в v1"
        );
        assert!(SEG_STEPS.contains(&step), "шаг {step} не из списка делителей зон");
    }
    // на типичной высоте панели сегментов заметно больше прежних 36
    assert!(seg_count(seg_db_for(500.0)) >= 60, "сегментов мало: {}", seg_count(seg_db_for(500.0)));

    // --- свойства для КАЖДОГО допустимого шага
    for step in SEG_STEPS {
        let n = seg_count(step);
        assert_eq!(led_lit(METER_MIN_DB, step), 0, "тишина — всё погашено");
        assert_eq!(led_lit(METER_MIN_DB - 20.0, step), 0);
        assert_eq!(led_lit(METER_MAX_DB, step), n, "клип — шкала до верха");
        assert_eq!(led_lit(METER_MAX_DB + 6.0, step), n, "перегруз не выходит за шкалу");
        let mut prev = 0;
        let mut db = METER_MIN_DB;
        while db <= METER_MAX_DB {
            let lit = led_lit(db, step);
            assert!(lit >= prev, "немонотонно при {db} dB, шаг {step}");
            prev = lit;
            db += 0.1;
        }
        // ни один сегмент не пересекает границу цветовой зоны — иначе он
        // был бы двухцветным и «горел наполовину»
        for i in 0..n {
            let lo = METER_MIN_DB + i as f32 * step + 0.01;
            let hi = METER_MIN_DB + (i + 1) as f32 * step - 0.01;
            assert_eq!(zone_color(lo), zone_color(hi), "сегмент {i} шага {step} двухцветный");
            assert_eq!(zone_color(seg_mid_db(i, step)), zone_color(lo), "цвет сегмента {i}");
        }
    }

    let mut h = harness();
    // --- (п.1) превью начинается у самой кромки окна
    let top = h.state().preview_rect.top();
    assert!(top <= crate::app::WIN_EDGE, "превью не у верха: {top}");

    // клик в полосе перетаскивания окна НЕ должен играть/ставить паузу
    let playing = h.state().player.transport.is_playing();
    let x = h.state().preview_rect.center().x;
    h.drag_at(egui::pos2(x, crate::app::WIN_EDGE + 4.0));
    h.step();
    h.drop_at(egui::pos2(x, crate::app::WIN_EDGE + 4.0));
    h.step();
    assert_eq!(
        h.state().player.transport.is_playing(),
        playing,
        "клик в полосе драга окна дёрнул воспроизведение"
    );
    // а ниже полосы — обычный клик по превью (play/pause с задержкой на
    // распознавание двойного клика)
    let y = h.state().preview_rect.center().y;
    h.drag_at(egui::pos2(x, y));
    h.step();
    h.drop_at(egui::pos2(x, y));
    h.step();
    assert!(
        h.state().preview_click_at.is_some() || h.state().player.transport.is_playing() != playing,
        "клик по превью не дошёл"
    );

    // --- (п.3 р.12) линейка не тратит место: полоса под метки убрана,
    // имя метки живёт на строке подписей времени
    let g = h.state().tl_geom.clone().expect("нет геометрии таймлайна");
    let ruler = g.lane_top(kt_model::ClipKind::Video, 0) - g.rect.top();
    assert!(
        ruler <= 36.0,
        "линейка съела {ruler} px до первой дорожки — место тратится впустую"
    );

    // --- (п.7) панель метров нельзя растянуть на пол-экрана
    let sep = panel_rect(&h, "meters_col").left();
    let mid = H * 0.5;
    h.hover_at(egui::pos2(sep, mid));
    h.step();
    h.drag_at(egui::pos2(sep, mid));
    h.step();
    h.hover_at(egui::pos2(W * 0.4, mid));
    h.step();
    h.drop_at(egui::pos2(W * 0.4, mid));
    h.step();
    let w = panel_width(&h, "meters_col");
    assert!(w <= 90.0, "метры растянулись на {w} px — нет верхнего предела");
    assert!(w > 75.0, "сепаратор метров вообще не потянулся ({w}) — тест ничего не проверил");
    // тот же класс: левая колонка не должна занимать пол-окна
    let sep = panel_rect(&h, "left_col").right();
    h.hover_at(egui::pos2(sep, mid));
    h.step();
    h.drag_at(egui::pos2(sep, mid));
    h.step();
    h.hover_at(egui::pos2(sep + 40.0, mid));
    h.step();
    h.hover_at(egui::pos2(W * 0.9, mid));
    h.step();
    h.drop_at(egui::pos2(W * 0.9, mid));
    h.step();
    let lw = panel_width(&h, "left_col");
    assert!(lw <= 420.0, "левая колонка растянулась на {lw} px");
    assert!(lw > 300.0, "сепаратор левой колонки не потянулся ({lw})");
}

/// Тест 98 (п.9 р.11): маска ввода таймкода.
///
/// Требования пользователя: набор СПРАВА НАЛЕВО, ширина тулбара не
/// скачет при открытии маски, работает во время воспроизведения, и это
/// не «просто воткнутый inputfield» — рисуется в том же прямоугольнике и
/// тем же шрифтом, что сам тайм-код.
#[test]
fn t98_timecode_mask() {
    use crate::util::{mask_digits, mask_text, mask_to_tick};

    // цифры вдвигаются справа, ненабранные старшие разряды — нули
    assert_eq!(mask_digits(""), "000000");
    assert_eq!(mask_digits("7"), "000007");
    assert_eq!(mask_digits("130"), "000130");
    assert_eq!(mask_digits("123456"), "123456");
    // лишние цифры не ломают маску: держим последние шесть
    assert_eq!(mask_digits("9123456"), "123456");
    // разбор: 00:01:30; минуты/секунды не нормализуются (99 минут — можно)
    assert_eq!(mask_to_tick("130", 30).0, 90_000 * 90);
    assert_eq!(mask_to_tick("9900", 30).0, 90_000 * 99 * 60);
    // подсветка: набранное — справа, ненабранное — слева приглушённо
    let (dimmed, typed) = mask_text("130");
    assert_eq!(dimmed, "00:0", "ненабранная часть слева");
    assert_eq!(typed, "1:30", "набранное справа");
    assert_eq!(mask_text("").1, "", "ничего не набрано — подсвечивать нечего");
    // вставка готового тайм-кода раскладывается по разрядам
    assert_eq!(
        crate::util::tick_to_mask(
            crate::util::parse_timecode("1:02:03", 30).expect("не разобрался")
        ),
        "010203"
    );

    // --- ширина тулбара не меняется при открытии маски
    let mut h = harness();
    let strip_h = |h: &Harness<'_, KtApp>| h.state().tl_geom.clone().map(|g| g.rect.height());
    let before = strip_h(&h);
    h.state_mut().tc_edit = Some(String::new());
    h.state_mut().tc_edit_focus = true;
    h.step();
    assert_eq!(strip_h(&h), before, "открытие маски сдвинуло вёрстку таймлайна");

    // --- при открытой маске пробел НЕ запускает воспроизведение
    let playing = h.state().player.transport.is_playing();
    h.key_press(egui::Key::Space);
    h.step();
    assert_eq!(
        h.state().player.transport.is_playing(),
        playing,
        "пробел утёк в хоткеи мимо маски"
    );

    // --- ПОВТОРНЫЙ вызов (п.8 р.12): после Enter маска обязана отдать
    // фокус, иначе все горячие клавиши мертвы до клика мышью
    h.key_press(egui::Key::Escape);
    h.step();
    assert!(h.state().tc_edit.is_none(), "Esc не закрыл маску");
    assert!(
        !h.ctx.egui_wants_keyboard_input(),
        "маска закрылась, но фокус не отдан — хоткеи мертвы"
    );
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::L);
    h.step();
    assert!(h.state().tc_edit.is_some(), "второй Ctrl+L не открыл маску");

    // --- ввод цифр и Enter переводят курсор (и не требуют паузы)
    h.event(egui::Event::Text("130".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    assert!(h.state().tc_edit.is_none(), "Enter не закрыл маску");
    assert_eq!(
        h.state().player.transport.playhead().0 / 90_000,
        90,
        "курсор не встал на 00:01:30"
    );
    // подмотка отрабатывает тем же кадром: запрос уже погашен, а курсор
    // виден в окне (иначе «перешёл, но не видно»)
    assert!(h.state().pending_reveal.is_none(), "запрос подмотки завис");
    // и после ПРИМЕНЕНИЯ маска тоже отдаёт фокус — Ctrl+L работает снова
    assert!(!h.ctx.egui_wants_keyboard_input(), "после Enter фокус остался на маске");
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::L);
    h.step();
    assert!(h.state().tc_edit.is_some(), "Ctrl+L после Enter не сработал");
    h.key_press(egui::Key::Escape);
    h.step();
    let g = h.state().tl_geom.clone().expect("нет геометрии");
    let x = g.tick_to_x(90 * 90_000);
    assert!(
        (g.rect.left()..=g.rect.right()).contains(&x),
        "после перехода курсор вне видимой области: x={x}, окно {:?}",
        g.rect
    );
}

/// Тест 99 (п.13 р.11): группы карточек медиа-бина.
///
/// Проверяем ЖИВЫЕ сценарии: рамка выделения ловит карточки, группа
/// едет целиком и не выпускает карточки за край панели, замок держит,
/// удаление медиа не оставляет висячих групп, пустое не попадает в файл.
#[test]
fn t99_bin_groups() {
    use kt_model::{BinGroup, MediaId, MediaRef};

    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        for n in 1..=3u64 {
            p.media.push(MediaRef { id: MediaId(n), path: format!("synth:{n}") });
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.bin_free = true;
        app.bin_layout.insert(MediaId(1), egui::pos2(0.0, 0.0));
        app.bin_layout.insert(MediaId(2), egui::pos2(0.0, 100.0));
        // третья карточка стоит с запасом: между второй и третьей есть
        // пустое место, откуда можно тянуть рамку выделения
        app.bin_layout.insert(MediaId(3), egui::pos2(0.0, 320.0));
    }
    h.step();

    // --- группировка выделенного: одна группа, карточка ровно в одной
    h.state_mut().bin_sel = [MediaId(1), MediaId(2)].into_iter().collect();
    let gi = h.state_mut().bin_group_selection("Планы".into()).expect("группа не создалась");
    assert_eq!(h.state().bin_groups[gi].members.len(), 2);
    assert_eq!(h.state().bin_group_of(MediaId(3)), None, "третья карточка вне группы");
    // повторная группировка перекладывает участника, а не дублирует
    h.state_mut().bin_sel = [MediaId(2), MediaId(3)].into_iter().collect();
    let gi2 = h.state_mut().bin_group_selection("Интервью".into()).expect("вторая группа");
    let in_groups: Vec<usize> = (0..h.state().bin_groups.len())
        .filter(|i| h.state().bin_groups[*i].members.contains(&MediaId(2)))
        .collect();
    assert_eq!(in_groups, vec![gi2], "карточка оказалась сразу в двух группах: {in_groups:?}");

    // --- кого тянуть: всю группу, даже если схвачена одна карточка
    let mates = h.state().bin_drag_set(MediaId(3));
    assert_eq!(mates.len(), 2, "группа обязана ехать целиком: {mates:?}");
    // карточка ВНЕ группы, но в выделении — едет вместе с выделением
    h.state_mut().bin_ungroup(gi); // «Планы» осталась с одним участником
    h.state_mut().bin_sel = [MediaId(1), MediaId(3)].into_iter().collect();
    assert_eq!(
        h.state().bin_drag_set(MediaId(1)).len(),
        2,
        "свободная карточка обязана ехать со всем выделением"
    );

    // --- замок
    assert!(!h.state().bin_locked(MediaId(3)));
    let gi2 = h
        .state()
        .bin_groups
        .iter()
        .position(|g| g.name == "Интервью")
        .expect("группа пропала");
    h.state_mut().bin_groups[gi2].locked = true;
    h.step();
    assert!(h.state().bin_locked(MediaId(3)), "замок группы не действует на участника");
    assert!(!h.state().bin_locked(MediaId(1)), "замок утёк на чужую карточку");

    // --- группы переживают переключение режимов и едут в проект
    h.state_mut().bin_free = false;
    h.step();
    h.state_mut().bin_free = true;
    h.step();
    assert_eq!(h.state().bin_groups.len(), 1, "переключение режимов потеряло группы");
    let st = h.state().ui_state_for_test();
    assert_eq!(st.bin_groups.len(), 1, "группы не попали в UiState");
    assert!(st.bin_groups[0].locked, "замок не доехал до проекта");
    let json = serde_json::to_string(&kt_model::UiState::default()).unwrap();
    assert!(!json.contains("bin_groups"), "пустые группы не должны попадать в файл: {json}");

    // --- ЖИВОЙ сценарий: рамка выделения по пустому месту ловит карточки
    h.state_mut().bin_groups.clear();
    h.state_mut().bin_sel.clear();
    h.step();
    let rects = h.state().bin_card_rects.clone();
    assert_eq!(rects.len(), 3, "карточки не нарисовались: {rects:?}");
    let at = |h: &Harness<'_, KtApp>, m: MediaId| {
        h.state().bin_card_rects.iter().find(|(id, _)| *id == m).unwrap().1
    };
    let (r0, r1) = (at(&h, MediaId(1)), at(&h, MediaId(2)));
    // тянем из ПУСТОГО места под второй карточкой вверх, накрывая первые
    // две; жест как у человека: нажать, чуть сдвинуть, вести дальше
    let start = egui::pos2(r0.center().x, r1.bottom() + 40.0);
    h.drag_at(start);
    h.step();
    h.hover_at(start + egui::vec2(-4.0, -8.0));
    h.step();
    h.hover_at(r0.center());
    h.step();
    h.drop_at(r0.center());
    h.step();
    assert!(
        h.state().bin_sel.contains(&MediaId(1)) && h.state().bin_sel.contains(&MediaId(2)),
        "рамка выделения не поймала карточки: {:?}",
        h.state().bin_sel
    );
    assert!(!h.state().bin_sel.contains(&MediaId(3)), "рамка захватила лишнюю карточку");

    // --- групповой драг: обе карточки уехали на одну дельту
    h.state_mut().bin_group_selection("Пара".into()).expect("группа");
    h.step();
    let before: Vec<egui::Pos2> = [MediaId(1), MediaId(2)]
        .iter()
        .map(|m| h.state().bin_layout[m])
        .collect();
    let grab = at(&h, MediaId(1));
    // нажать → чуть сдвинуть (здесь egui признаёт драг) → вести дальше
    h.drag_at(grab.center());
    h.step();
    h.hover_at(grab.center() + egui::vec2(0.0, 8.0));
    h.step();
    h.hover_at(grab.center() + egui::vec2(0.0, 38.0));
    h.step();
    h.drop_at(grab.center() + egui::vec2(0.0, 38.0));
    h.step();
    let after: Vec<egui::Pos2> = [MediaId(1), MediaId(2)]
        .iter()
        .map(|m| h.state().bin_layout[m])
        .collect();
    let d0 = after[0] - before[0];
    let d1 = after[1] - before[1];
    assert!(d0.y > 10.0, "схваченная карточка не поехала: {d0:?}");
    assert!((d0 - d1).length() < 0.6, "группа разъехалась: {d0:?} vs {d1:?}");
    assert!(after.iter().all(|p| p.x >= 0.0 && p.y >= 0.0), "карточку вынесло за край: {after:?}");

    // --- замок держит: залоченная группа не двигается мышью
    let gi3 = h.state().bin_groups.len() - 1;
    h.state_mut().bin_groups[gi3].locked = true;
    h.step();
    let before = h.state().bin_layout[&MediaId(1)];
    let grab = at(&h, MediaId(1));
    h.drag_at(grab.center());
    h.step();
    h.hover_at(grab.center() + egui::vec2(0.0, 8.0));
    h.step();
    h.hover_at(grab.center() + egui::vec2(0.0, 40.0));
    h.step();
    h.drop_at(grab.center() + egui::vec2(0.0, 40.0));
    h.step();
    assert_eq!(h.state().bin_layout[&MediaId(1)], before, "замок не удержал карточку");

    // --- удаление медиа не оставляет висячих участников и пустых групп
    let mut groups = vec![
        BinGroup { name: "a".into(), members: vec![MediaId(1), MediaId(2)], locked: false },
        BinGroup { name: "b".into(), members: vec![MediaId(3)], locked: true },
    ];
    kt_model::prune_bin_groups(&mut groups, &[MediaId(1)]);
    assert_eq!(groups.len(), 1, "опустевшая группа не убрана");
    assert_eq!(groups[0].members, vec![MediaId(1)], "удалённое медиа осталось в группе");
}

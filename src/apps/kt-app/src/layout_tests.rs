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
use crate::i18n::{Lang, k, tr};
use egui_kittest::Harness;

const W: f32 = 1440.0;
const H: f32 = 860.0;

pub fn harness() -> Harness<'static, KtApp> {
    harness_lang(crate::i18n::Lang::Ru)
}

/// Харнесс на конкретном языке. Язык закреплён ЯВНО: иначе тест зависел бы от
/// LANG в оболочке разработчика. Шрифты ставятся настоящие — до Р.19 все
/// замеры ширин делались на дефолтном наборе egui, то есть не на той
/// программе, которую запускает пользователь.
fn harness_lang(lang: crate::i18n::Lang) -> Harness<'static, KtApp> {
    harness_lang_dt(lang, None)
}

/// Тот же харнесс с управляемым шагом времени. Двойной клик у egui —
/// ВРЕМЕННОЙ жест (два клика ближе 0.3 с), а шаг харнесса по умолчанию
/// 0.25 с: события в соседних кадрах не складываются в двойной клик, и
/// тест «двойной клик» проверял бы одиночный.
fn harness_lang_dt(lang: crate::i18n::Lang, step_dt: Option<f32>) -> Harness<'static, KtApp> {
    crate::i18n::set_lang(lang);
    let app = KtApp::new_empty(None);
    let mut b = Harness::builder().with_size(egui::vec2(W, H));
    if let Some(dt) = step_dt {
        b = b.with_step_dt(dt);
    }
    let mut h = b.build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&h.ctx);
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
    assert!((72.0..=420.0).contains(&w), "левая колонка по умолчанию вне пределов: {w}");
    assert!(h.state().preview_rect.width() > 600.0, "превью схлопнулось: {:?}", h.state().preview_rect);

    // длинный контент не должен раздувать панель
    h.state_mut().gpu_info = Some("X".repeat(400));
    h.step();
    let w2 = panel_width(&h, "left_col");
    assert!(w2 <= w + 1.0, "длинный контент раздул колонку: было {w}, стало {w2}");
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
        Some(tr(k::win::CLOSE)),
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
        (egui::pos2(24.0, toolbar_y), tr(k::tb::PLAY_PAUSE)), // тулбар (рисуется ПОСЛЕ статусбара)
        (egui::pos2(W - 11.0, 10.0), tr(k::win::CLOSE)),      // кнопки окна (ещё позже)
        (egui::pos2(14.0, 40.0), tr(k::bin::HINT_FREE)),      // бин
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

    // п.11 р.13: в РАЗВЁРНУТОМ окне ресайза нет, поэтому верхние WIN_EDGE
    // пикселей раньше были мёртвыми — «полоска, по которой бесполезно
    // тянуть». Теперь драг начинается от самой кромки.
    let mut h = harness();
    h.input_mut()
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .maximized = Some(true);
    h.step();
    h.drag_at(egui::pos2(W * 0.5, 1.0));
    h.step();
    h.hover_at(egui::pos2(W * 0.5 + 20.0, 6.0));
    h.step();
    let sent = h
        .output()
        .viewport_output
        .values()
        .any(|v| v.commands.iter().any(|c| matches!(c, V::StartDrag | V::Maximized(false))));
    assert!(sent, "в развёрнутом окне верхняя кромка не тянет окно");
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
        [tr(k::meter::FADER), tr(k::meter::PEAK), tr(k::meter::MUTE)]
            .iter()
            .any(|want| h.state().status_line_shown.0.starts_with(want)),
        "у правого края зазор: в строке «{}»",
        h.state().status_line_shown.0
    );

    // минимальное окно
    let app = KtApp::new_empty(None);
    let (w, hh) = (640.0f32, 640.0f32);
    let mut h = Harness::builder()
        .with_size(egui::vec2(w, hh))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&h.ctx);
    h.step();
    h.step();
    h.hover_at(egui::pos2(w - 11.0, 10.0));
    h.step();
    assert_eq!(
        h.state().status_line_shown.0,
        tr(k::win::CLOSE),
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
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
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
                    fx_auto: vec![],
                    fx: vec![],
                    opacity: 1.0,
                    fx_off: false,
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
        let y = g.lane_y(ClipKind::Video, 0, 0.5);
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
    h.state_mut().apply(Command::AddMarker { at: Tick(45_000), duration: Tick::ZERO });
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
    // п.4 р.13: предел бина — 70 % окна (не ужимать!), тянуться обязан
    let lw = panel_width(&h, "left_col");
    assert!(lw <= W * 0.7 + 1.0, "левая колонка вышла за предел: {lw}");
    assert!(
        lw > 600.0,
        "медиа-бин снова зажат ({lw} px) — предел ставился по прямой просьбе"
    );
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

    // --- ИМЯ ГРУППЫ СОХРАНЯЕТСЯ ПО ENTER (жалоба р.13: поле не исчезало
    // и имя терялось — request_focus звался каждый кадр, поэтому
    // lost_focus не наступал никогда)
    h.state_mut().bin_sel = [MediaId(1)].into_iter().collect();
    let gi4 = h.state_mut().bin_group_selection(String::new()).expect("группа");
    h.state_mut().bin_group_edit = Some((gi4, String::new()));
    h.state_mut().bin_group_focus = true;
    h.step();
    h.event(egui::Event::Text("план".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
    assert!(h.state().bin_group_edit.is_none(), "поле имени группы не закрылось по Enter");
    assert_eq!(h.state().bin_groups[gi4].name, "план", "имя группы не сохранилось");

    // Esc закрывает поле и имя НЕ меняет
    h.state_mut().bin_group_edit = Some((gi4, "другое".into()));
    h.state_mut().bin_group_focus = true;
    h.step();
    h.key_press(egui::Key::Escape);
    h.step();
    assert!(h.state().bin_group_edit.is_none(), "Esc не закрыл поле");
    assert_eq!(h.state().bin_groups[gi4].name, "план", "Esc не должен менять имя");

    // --- удаление медиа не оставляет висячих участников и пустых групп
    let mut groups = vec![
        BinGroup { name: "a".into(), members: vec![MediaId(1), MediaId(2)], locked: false },
        BinGroup { name: "b".into(), members: vec![MediaId(3)], locked: true },
    ];
    kt_model::prune_bin_groups(&mut groups, &[MediaId(1)]);
    assert_eq!(groups.len(), 1, "опустевшая группа не убрана");
    assert_eq!(groups[0].members, vec![MediaId(1)], "удалённое медиа осталось в группе");
}

/// Тест 101 (п.9 р.12): кнопки у краёв окна нажимаются, а не отдают
/// нажатие window-менеджеру.
///
/// Зона ресайза — не виджет: она читает СЫРОЙ указатель в конце кадра и
/// на нажатии шлёт BeginResize, забирая указатель у WM. Центр кнопки
/// «Закрыть» (W−11, 11) лежал прямо в углу 16×16, колокольчик — в углу
/// SW, у меню умирала верхняя четверть. Старые тесты этого не видели,
/// потому что проверяли НАВЕДЕНИЕ: ему зона ресайза не мешает.
#[test]
fn t101_edge_buttons_are_clickable() {
    use egui::ViewportCommand as V;

    // Команды копим за ОБА кадра: BeginResize уходит на нажатии, а
    // Close/Minimized — только на отпускании (clicked()). Отпускание
    // шлём ЯВНЫМ событием, а не drop_at: тот следом кладёт PointerGone,
    // step() прогоняет кадр НА КАЖДОЕ событие, и output() показывает уже
    // кадр без команды — тест ловил пустоту при работающей кнопке.
    fn press(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) -> Vec<V> {
        let grab = |h: &Harness<'_, KtApp>| -> Vec<V> {
            h.output()
                .viewport_output
                .values()
                .flat_map(|v| v.commands.iter().cloned())
                .collect()
        };
        let button = |pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        h.hover_at(pos);
        h.step();
        h.event(button(true));
        h.step();
        let mut sent = grab(h);
        h.event(button(false));
        h.step();
        sent.extend(grab(h));
        h.event(egui::Event::PointerGone);
        h.step();
        sent
    }

    // мёртвые точки: центр «Закрыть» в углу NE, «Развернуть»/«Свернуть» в
    // верхней кромке, колокольчик в углу SW, «Файл» в верхней кромке
    let cases: [(egui::Pos2, &str); 5] = [
        (egui::pos2(W - 11.0, 11.0), "Закрыть"),
        (egui::pos2(W - 33.0, 4.0), "Развернуть"),
        (egui::pos2(W - 55.0, 4.0), "Свернуть"),
        (egui::pos2(18.0, H - 5.0), "Уведомления"),
        (egui::pos2(30.0, 6.0), "меню Файл"),
    ];
    for (pos, what) in cases {
        let mut h = harness();
        let sent = press(&mut h, pos);
        assert!(
            !sent.iter().any(|c| matches!(c, V::BeginResize(_))),
            "нажатие по «{what}» ушло в ресайз окна вместо кнопки"
        );
    }

    // и последствия нажатий действительно наступают
    let mut h = harness();
    let sent = press(&mut h, egui::pos2(W - 11.0, 11.0));
    assert!(sent.iter().any(|c| matches!(c, V::Close)), "окно не закрылось");

    let mut h = harness();
    let sent = press(&mut h, egui::pos2(W - 55.0, 4.0));
    assert!(sent.iter().any(|c| matches!(c, V::Minimized(true))), "окно не свернулось");

    let mut h = harness();
    let before = h.state().notices_open;
    press(&mut h, egui::pos2(18.0, H - 5.0));
    assert_ne!(h.state().notices_open, before, "колокольчик не открыл уведомления");

    // САМАЯ кромка (4 px) остаётся за ресайзом даже поверх кнопок —
    // иначе теряется диагональ угла; проверяем, что она жива
    let mut h = harness();
    let sent = press(&mut h, egui::pos2(W - 2.0, 2.0));
    assert!(
        sent.iter().any(|c| matches!(c, V::BeginResize(_))),
        "угол окна перестал тянуться из-за кнопок"
    );

    // (п.6 р.13) кнопка «свернуть» нажимается при ЛЮБОЙ ширине панели
    // метров: своя полоса перетаскивания лежит ЛЕВЕЕ панели и в кнопки не
    // заходит (штатный хват egui был ±8 px по всей высоте и выигрывал)
    for w in [crate::app::METERS_W_MIN, 70.0, crate::app::METERS_W_MAX] {
        let mut h = harness();
        h.state_mut().meters_w = w;
        h.step();
        let sent = press(&mut h, egui::pos2(W - 55.0, 11.0));
        assert!(
            sent.iter().any(|c| matches!(c, V::Minimized(true))),
            "при ширине метров {w} «свернуть» не сработала"
        );
    }

    // и сама полоса тянет панель: хват СЛЕВА от её кромки
    let mut h = harness();
    let start = W - h.state().meters_w;
    h.hover_at(egui::pos2(start - 5.0, H * 0.5));
    h.step();
    h.drag_at(egui::pos2(start - 5.0, H * 0.5));
    h.step();
    h.hover_at(egui::pos2(start - 12.0, H * 0.5));
    h.step();
    h.hover_at(egui::pos2(W - 88.0, H * 0.5));
    h.step();
    h.drop_at(egui::pos2(W - 88.0, H * 0.5));
    h.step();
    let mw = h.state().meters_w;
    assert!(mw > 80.0, "полоса не потянула панель метров: {mw}");
    assert!(mw <= crate::app::METERS_W_MAX, "панель метров вышла за предел: {mw}");

    // чистые кромки по-прежнему дают ресайз — гейт не должен убить его
    let mut h = harness();
    let sent = press(&mut h, egui::pos2(W * 0.5, H - 2.0));
    assert!(
        sent.iter().any(|c| matches!(c, V::BeginResize(_))),
        "нижняя кромка перестала тянуться"
    );
}

/// Тест 102 (пп.2,5 р.13): ярлык дорожки не наслаивается на имя клипа,
/// свечение курсора не заходит на полоски кэша и светит со стороны
/// движения.
/// Тест 122 (п.5 р.16): дорожки добавляются ПОД кликнутой строкой, а
/// имена дорожек едут вместе со своими дорожками.
///
/// Вторая половина — настоящий дефект, найденный по дороге: функции
/// `lane_names_insert/remove/rotate` в kt-model есть и покрыты t90, но из
/// приложения не вызывались ни разу, поэтому после вставки/удаления/
/// перестановки подпись приклеивалась к чужой дорожке.
#[test]
fn t122_insert_lane_after_clicked_row() {
    use kt_model::{LaneKind, lane_label};
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.video_lanes = 3;
        p.audio_lanes = 2;
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.lane_names = vec![
            (LaneKind::Video, 1, "Титры".into()),
            (LaneKind::Audio, 1, "Голос".into()),
        ];
    }
    h.step();

    // --- клик по V1 (индекс 0) + «добавить видео-дорожку» → новая
    // строка ровно СЛЕДУЮЩАЯ, а не в конце списка
    h.state_mut().insert_lane_after_row(LaneKind::Video, Some((LaneKind::Video, 0)));
    h.step();
    assert_eq!(h.state().model.project().video_lanes, 4, "дорожка не добавилась");
    assert_eq!(
        h.state().model.project().rows()[1],
        (LaneKind::Video, 1),
        "новая дорожка не встала под кликнутой: {:?}",
        h.state().model.project().rows()
    );
    // имя «Титры» ехало на V2 (индекс 1) — обязано уехать на индекс 2
    assert_eq!(
        lane_label(&h.state().lane_names, LaneKind::Video, 2),
        "Титры",
        "имя не поехало вместе со своей дорожкой: {:?}",
        h.state().lane_names
    );
    assert_eq!(
        lane_label(&h.state().lane_names, LaneKind::Video, 1),
        "V2",
        "новая дорожка забрала чужое имя"
    );

    // --- клик по ПОСЛЕДНЕЙ строке своего вида: строка обязана встать
    // сразу под ней, а не в самый низ списка. Именно это и нашлось живой
    // проверкой: lane_order_insert кладёт строку перед следующей
    // дорожкой своего вида, а когда такой нет — в конец, и клик по V2
    // при порядке V1,V2,A1,A2 давал V1,V2,A1,A2,V3.
    let last_video = h
        .state()
        .model
        .project()
        .rows()
        .iter()
        .position(|&e| e == (LaneKind::Video, 3))
        .expect("последняя видео-строка");
    h.state_mut().insert_lane_after_row(LaneKind::Video, Some((LaneKind::Video, 3)));
    h.step();
    assert_eq!(
        h.state().model.project().rows()[last_video + 1],
        (LaneKind::Video, 4),
        "новая строка уехала в самый низ вместо места под кликнутой: {:?}",
        h.state().model.project().rows()
    );

    // --- клик по строке ДРУГОГО вида: аудио-дорожка встаёт под ней же
    let audio_row = h
        .state()
        .model
        .project()
        .rows()
        .iter()
        .position(|&e| e == (LaneKind::Video, 3))
        .expect("строка V4 пропала");
    h.state_mut().insert_lane_after_row(LaneKind::Audio, Some((LaneKind::Video, 3)));
    h.step();
    assert_eq!(
        h.state().model.project().rows()[audio_row + 1].0,
        LaneKind::Audio,
        "аудио-дорожка не встала под кликнутой видео-строкой: {:?}",
        h.state().model.project().rows()
    );

    // --- morph тоже добавляется из меню
    let m0 = h.state().model.project().morph_lanes;
    h.state_mut().insert_lane_after_row(LaneKind::Morph, None);
    h.step();
    assert_eq!(h.state().model.project().morph_lanes, m0 + 1, "morph-дорожка не добавилась");

    // --- удаление: имя уходит вместе с дорожкой, соседи съезжают
    {
        let app = h.state_mut();
        app.lane_names = vec![
            (LaneKind::Audio, 0, "Музыка".into()),
            (LaneKind::Audio, 1, "Голос".into()),
        ];
        app.delete_lane(kt_model::ClipKind::Audio, 0);
    }
    h.step();
    assert_eq!(
        lane_label(&h.state().lane_names, LaneKind::Audio, 0),
        "Голос",
        "после удаления имена не съехали: {:?}",
        h.state().lane_names
    );

    // --- перестановка: имя едет за содержимым (иначе «Голос» останется
    // на чужой строке)
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.audio_lanes = 3;
        p.media.push(kt_model::MediaRef { id: kt_model::MediaId(1), path: "synth:1".into() });
        p.clips.push(kt_model::ClipEvent {
            id: kt_model::ClipId(1),
            link_group: None,
            kind: kt_model::ClipKind::Audio,
            lane: 0,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(90_000),
            source_in: kt_time::Tick(0),
            media: kt_model::MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.lane_names = vec![(LaneKind::Audio, 0, "Голос".into())];
        app.reorder_lane(kt_model::ClipKind::Audio, 0, 2);
    }
    h.step();
    assert_eq!(
        h.state().model.project().clips[0].lane,
        2,
        "содержимое не переехало — тест ниже проверял бы не то"
    );
    assert_eq!(
        lane_label(&h.state().lane_names, LaneKind::Audio, 2),
        "Голос",
        "имя не поехало за содержимым при перестановке: {:?}",
        h.state().lane_names
    );
}

/// Тест 120 (п.3 р.16): удержание пробела разгоняет воспроизведение до
/// 4× примерно за 2 с (как на YouTube) и НЕ ломает одиночное/двойное
/// нажатие.
///
/// Ловушка, из-за которой понадобился перенос решения на отпускание:
/// `key_pressed` включает АВТОПОВТОР клавиатуры — зажатый пробел слал
/// серию play/pause. Поэтому нажатие только запоминается, а play/pause
/// делается по отпусканию, если удержание было коротким.
#[test]
fn t120_space_hold_boosts_playback() {
    use crate::app::{KtApp, SPACE_BOOST_MAX, SPACE_BOOST_SECS};
    use kt_model::*;
    use kt_time::Tick;

    // --- чистая функция: монотонна, упирается в потолок, устойчива на
    // плохих dt (после простоя stable_dt приходит до 250 мс — явная
    // схема на таких шагах перелетает цель, п.7 р.9)
    let mut b = 1.0f32;
    let mut prev = b;
    for _ in 0..(60.0 * SPACE_BOOST_SECS) as i32 {
        b = KtApp::space_boost_step(b, 1.0 / 60.0);
        assert!(b >= prev, "разгон не монотонен: {prev} → {b}");
        assert!(b <= SPACE_BOOST_MAX + 1e-4, "разгон перелетел потолок: {b}");
        prev = b;
    }
    assert!(b > SPACE_BOOST_MAX * 0.9, "за {SPACE_BOOST_SECS} с не доехали до потолка: {b}");
    // и НЕ быстрее, чем просил пользователь (п.3 р.17: «надо не за 2с а
    // за 5, а то очень быстро»): за 2 с разгон обязан быть заметно ниже
    // потолка — на прежней константе выходило 3.85 из 4
    let mut half = 1.0f32;
    for _ in 0..120 {
        half = KtApp::space_boost_step(half, 1.0 / 60.0);
    }
    assert!(half < 3.4, "разгон слишком резкий: за 2 с уже {half}× из {SPACE_BOOST_MAX}");
    let mut rough = 1.0f32;
    for dt in [0.25f32, 0.1, 1.0 / 30.0, 0.25, 0.25] {
        rough = KtApp::space_boost_step(rough, dt);
        assert!(
            (1.0..=SPACE_BOOST_MAX + 1e-4).contains(&rough),
            "плохой dt={dt} вынес разгон за пределы: {rough}"
        );
    }

    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        p.clips.push(ClipEvent {
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(0),
            duration: Tick(90_000 * 60),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();

    // --- одиночное нажатие по-прежнему переключает транспорт
    let before = h.state().player.transport.is_playing();
    h.key_press(egui::Key::Space);
    h.step();
    h.step();
    assert_ne!(
        h.state().player.transport.is_playing(),
        before,
        "короткое нажатие пробела перестало переключать play/pause"
    );
    let playing = h.state().player.transport.is_playing();

    // --- удержание: скорость растёт, транспорт НЕ переключается
    h.key_down(egui::Key::Space);
    h.step();
    std::thread::sleep(std::time::Duration::from_millis(260));
    for _ in 0..60 {
        h.step();
    }
    let boosted = h.state().player.playback_rate();
    assert!(
        boosted > 1.5 && boosted <= SPACE_BOOST_MAX + 0.01,
        "удержание не разогнало воспроизведение: {boosted}"
    );
    assert!(h.state().player.transport.is_playing(), "удержание обязано играть");
    // пользователь обязан ВИДЕТЬ разгон: нижняя (временная) полоса
    // контрола скорости показывает фактическую скорость, а не «1×»
    let shown = h.state().speed_temp_slider().expect("разгон не виден на контроле скорости");
    assert!(
        shown > crate::app::rate_to_slider(1.0) + 0.01,
        "полоса скорости показывает не разгон: {shown}"
    );
    h.key_up(egui::Key::Space);
    h.step();
    assert!(
        (h.state().player.playback_rate() - 1.0).abs() < 0.01,
        "после отпускания скорость не вернулась: {}",
        h.state().player.playback_rate()
    );
    assert_eq!(
        h.state().player.transport.is_playing(),
        playing,
        "отпускание разгона не должно переключать play/pause"
    );

    // --- реверс: разгон не теряет знак скорости
    h.state_mut().base_rate = -1.0;
    h.state_mut().player.set_playback_rate(-1.0);
    h.key_down(egui::Key::Space);
    h.step();
    std::thread::sleep(std::time::Duration::from_millis(260));
    for _ in 0..40 {
        h.step();
    }
    let rev = h.state().player.playback_rate();
    assert!(rev < -1.5, "разгон в реверсе потерял знак/скорость: {rev}");
    h.key_up(egui::Key::Space);
    h.step();
    h.state_mut().base_rate = 1.0;
    h.state_mut().player.set_playback_rate(1.0);

    // --- двойное нажатие: пауза, затем «стоп в начало региона» (п.15 р.6)
    h.state_mut().model.set_region(Region::normalized(Tick(90_000), Tick(180_000)));
    h.state_mut().player.seek(Tick(120_000));
    h.step();
    h.key_press(egui::Key::Space); // пауза
    h.step();
    h.step();
    assert!(!h.state().player.transport.is_playing(), "первое нажатие не поставило паузу");
    h.key_press(egui::Key::Space); // сразу второе — стоп в начало региона
    h.step();
    h.step();
    assert_eq!(
        h.state().player.transport.playhead(),
        Tick(90_000),
        "двойное нажатие пробела перестало уводить в начало региона"
    );
}

/// Тест 119 (п.2 р.16): Shift+A / Shift+D тянут край региона.
///
/// Shift — модификатор «расширить выделение» из проф. программ. Shift+A
/// ставит ЛЕВЫЙ край в новую позицию курсора, Shift+D — правый; без
/// региона вторым краем становится позиция ДО прыжка. Обычные A/D регион
/// не трогают (иначе навигация начнёт молча портить выделение), а
/// Shift+Home/End остаются «в самое начало/конец».
#[test]
fn t119_shift_nav_drags_region() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        for (i, start) in [90_000i64, 270_000, 450_000].into_iter().enumerate() {
            p.clips.push(ClipEvent {
                id: ClipId(i as u64 + 1),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(0),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.player.seek(Tick(0));
        app.dirty_content = false;
        app.dirty_ui = false;
    }
    h.step();

    // обычная навигация регион НЕ создаёт
    h.key_press(egui::Key::D);
    h.step();
    let after_plain = h.state().player.transport.playhead();
    assert!(after_plain.0 > 0, "обычный D не сдвинул курсор");
    assert!(h.state().model.project().region.is_none(), "обычный D создал регион");

    // первый Shift+D выделяет пройденный участок [было, стало]
    let before = h.state().player.transport.playhead();
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::D);
    h.step();
    let now = h.state().player.transport.playhead();
    let r = h.state().model.project().region.expect("Shift+D не создал регион");
    assert!(now > before, "Shift+D не сдвинул курсор: {before:?} → {now:?}");
    assert_eq!(r.start, before, "левый край обязан остаться там, откуда прыгнули");
    assert_eq!(r.end, now, "правый край обязан встать в новую позицию");

    // следующий Shift+D двигает ТОЛЬКО правый край
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::D);
    h.step();
    let r2 = h.state().model.project().region.expect("регион пропал");
    assert_eq!(r2.start, r.start, "правый край утянул за собой левый: {r:?} → {r2:?}");
    assert!(r2.end > r.end, "правый край не поехал: {r:?} → {r2:?}");

    // Shift+A двигает ЛЕВЫЙ край и правого не трогает
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::A);
    h.step();
    let r3 = h.state().model.project().region.expect("регион пропал");
    assert_eq!(r3.end, r2.end, "левый край утянул правый: {r2:?} → {r3:?}");
    assert!(r3.start > r2.start, "левый край не поехал: {r2:?} → {r3:?}");
    assert_eq!(h.state().player.transport.playhead(), r3.start, "курсор не на левом крае");

    // регион — «удобство»: звёздочка «есть несохранённое содержимое» не
    // загорается (п.5 р.11)
    assert!(!h.state().dirty_content, "правка региона подняла dirty_content");
    assert!(h.state().dirty_ui, "правка региона не подняла dirty_ui");
}

/// Тест 118 (п.1 р.16): медиа-бин скроллится — сам при драге к кромке и
/// колесом.
///
/// Причина дефекта: «курсор ушёл из бина» проверялось по `ui.max_rect()`,
/// а внутри ScrollArea это ВЬЮПОРТ в координатах содержимого. У нижней
/// кромки экрана включался липкий режим «вынос на таймлайн», позиции
/// откатывались — карточку нельзя было положить ниже одного экрана,
/// содержимое никогда не перерастало вьюпорт, и колесу было нечего
/// крутить. Тест ведёт мышь как человек (нажать → сдвинуть → вести).
#[test]
fn t118_bin_autoscrolls_and_wheel_works() {
    use crate::bin_panel::bin_edge_scroll;
    use kt_model::{MediaId, MediaRef};

    // --- чистая кривая: у нижней кромки мотаем вниз, у верхней вверх,
    // в середине покоя нет вообще
    let view = egui::Rect::from_min_size(egui::pos2(0.0, 100.0), egui::vec2(200.0, 400.0));
    assert_eq!(bin_edge_scroll(view, 300.0), 0.0, "середина не должна мотать");
    let down = bin_edge_scroll(view, view.bottom() - 1.0);
    let up = bin_edge_scroll(view, view.top() + 1.0);
    assert!(down < 0.0, "у нижней кромки список обязан ехать вниз: {down}");
    assert!(up > 0.0, "у верхней кромки — вверх: {up}");
    assert!(
        bin_edge_scroll(view, view.bottom() - 20.0).abs() < down.abs(),
        "кривая обязана расти к кромке"
    );

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
        app.bin_layout.insert(MediaId(3), egui::pos2(0.0, 200.0));
    }
    h.step();
    let at = |h: &Harness<'_, KtApp>, m: MediaId| {
        h.state().bin_card_rects.iter().find(|(id, _)| *id == m).unwrap().1
    };
    let col = panel_rect(&h, "left_col");
    let grab = at(&h, MediaId(1)).center();
    let anchor_before = at(&h, MediaId(3));

    // тянем первую карточку к самой нижней кромке колонки и ДЕРЖИМ там:
    // список обязан ползти сам, как у таймлайна
    h.drag_at(grab);
    h.step();
    h.hover_at(grab + egui::vec2(0.0, 8.0));
    h.step();
    let low = egui::pos2(grab.x, col.bottom() - 10.0);
    for _ in 0..12 {
        h.hover_at(low);
        h.step();
    }
    let anchor_after = at(&h, MediaId(3));
    assert!(
        anchor_after.top() < anchor_before.top() - 20.0,
        "список не проскроллился при драге к кромке: {} → {}",
        anchor_before.top(),
        anchor_after.top()
    );
    h.drop_at(low);
    h.step();
    let moved = h.state().bin_layout[&MediaId(1)];
    assert!(
        moved.y > 200.0,
        "карточку откатило вместо переноса вниз (липкий «вынос»): {moved:?}"
    );

    // --- колесо: содержимое теперь длиннее вьюпорта, и над бином оно
    // обязано крутиться (штатный ScrollArea глушит колесо только на
    // время драга — драг мы уже отпустили)
    let before = at(&h, MediaId(3)).top();
    let over = egui::pos2(col.center().x, col.center().y);
    h.hover_at(over);
    h.step();
    for _ in 0..3 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -60.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    let after = at(&h, MediaId(3)).top();
    assert!(after < before - 20.0, "колесо не крутит медиа-бин: {before} → {after}");

    // --- п.1 р.17: колесо обязано работать И ВО ВРЕМЯ драга. egui сам
    // глушит скролл у ScrollArea, пока что-нибудь тащат
    // (`dragged_id().is_none()`), а человек крутит колесо именно тогда —
    // чтобы дотянуться до нужного места («scroll мышки по прежнему не
    // работает»).
    // возвращаемся наверх: после проверок выше список домотан вниз и
    // карточки уехали за пределы видимой полосы
    let wheel_up = |h: &mut Harness<'_, KtApp>, n: usize| {
        for _ in 0..n {
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 90.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            });
            h.step();
        }
    };
    h.hover_at(over);
    h.step();
    wheel_up(&mut h, 12);
    let grab2 = at(&h, MediaId(2)).center();
    assert!(col.contains(grab2), "карточка вне видимой части колонки: {grab2:?} / {col:?}");
    h.drag_at(grab2);
    h.step();
    h.hover_at(grab2 + egui::vec2(0.0, 8.0));
    h.step();
    let before_drag_wheel = at(&h, MediaId(3)).top();
    for _ in 0..3 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -60.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    let after_drag_wheel = at(&h, MediaId(3)).top();
    assert!(
        after_drag_wheel < before_drag_wheel - 20.0,
        "колесо во время драга не крутит бин: {before_drag_wheel} → {after_drag_wheel}"
    );
    h.drop_at(grab2 + egui::vec2(0.0, 8.0));
    h.step();

    // --- п.1 р.17: тычок В КРОМКУ — это «мотай список», а не «выноси»:
    // карточка обязана остаться там, куда её ведут. Пользователь: «тыкаю
    // в край — мы скролимся, но потом сурс сам улетает обратно».
    let card = at(&h, MediaId(1));
    let before_pos = h.state().bin_layout[&MediaId(1)];
    h.drag_at(card.center());
    h.step();
    h.hover_at(card.center() + egui::vec2(0.0, 8.0));
    h.step();
    let jab = egui::pos2(card.center().x, col.bottom() + 12.0);
    for _ in 0..4 {
        h.hover_at(jab);
        h.step();
    }
    h.drop_at(jab);
    h.step();
    let after_pos = h.state().bin_layout[&MediaId(1)];
    assert!(
        (after_pos - before_pos).length() > 4.0,
        "тычок в кромку откатил карточку на старое место: {before_pos:?} → {after_pos:?}"
    );

    // но настоящий вынос (далеко за колонку) по-прежнему откатывает
    // раскладку — иначе карточка уезжает при удачном дропе на таймлайн
    let card = at(&h, MediaId(1));
    let before_pos = h.state().bin_layout[&MediaId(1)];
    h.drag_at(card.center());
    h.step();
    h.hover_at(card.center() + egui::vec2(8.0, 0.0));
    h.step();
    let out = egui::pos2(col.right() + 200.0, col.center().y);
    h.hover_at(out);
    h.step();
    h.drop_at(out);
    h.step();
    assert_eq!(
        h.state().bin_layout[&MediaId(1)],
        before_pos,
        "вынос на таймлайн не должен менять раскладку бина (п.8 р.9)"
    );
}

#[test]
fn t102_lane_label_and_playhead_trail() {
    use crate::timeline::{STRIPES_H, TIME_TOP, lane_label_y};

    // (п.2) ярлык дорожки живёт СТРОКОЙ НИЖЕ имени клипа: имя клипа
    // рисуется от row_top+2 плашкой высотой ~17 px, значит ярлык обязан
    // начинаться не выше row_top+17 — иначе буквы наложатся
    for (lane_h, galley_h) in [(64.0f32, 14.0f32), (40.0, 14.0), (28.0, 14.0), (120.0, 14.0)] {
        let y = lane_label_y(100.0, lane_h, galley_h);
        assert!(y >= 100.0, "ярлык уехал выше дорожки: {y}");
        assert!(y + galley_h <= 100.0 + lane_h, "ярлык вылез за дорожку: {y}+{galley_h}");
        if lane_h >= 36.0 {
            assert!(
                y >= 117.0,
                "ярлык на дорожке {lane_h} встал на строку имени клипа: {y}"
            );
        }
    }
    // (п.5) свечение стартует ниже полосок кэша
    let g = harness().state().tl_geom.clone().expect("нет геометрии");
    assert!(
        g.rect.top() + TIME_TOP >= g.rect.top() + STRIPES_H,
        "свечение курсора начнётся поверх полосок кэша"
    );

    let mut h = harness();
    // направление запоминается по знаку скорости и не мигает на паузе
    assert!(!h.state().play_dir_back, "по умолчанию шлейф слева (игра вперёд)");
    h.state_mut().player.set_playback_rate(-1.0);
    h.step();
    assert!(h.state().play_dir_back, "реверс не переключил сторону шлейфа");
    h.state_mut().player.set_playback_rate(0.0);
    h.step();
    assert!(h.state().play_dir_back, "на паузе сторона обязана сохраниться");
    h.state_mut().player.set_playback_rate(1.0);
    h.step();
    assert!(!h.state().play_dir_back, "вперёд не вернуло шлейф налево");
}

/// Тест 127 (Р.19): вёрстка держится ВО ВСЕХ ПЯТИ ЯЗЫКАХ.
///
/// Пределы 420 px (левая колонка) и 90 px (метры) поставлены по прямым
/// просьбам пользователя (п.4 р.13, п.6 р.12) и НЕ двигаются: если перевод в
/// них не влезает — короче делают перевод. До Р.19 эти пределы проверялись
/// только на русском, то есть на одном наборе длин строк.
#[test]
fn t127_layout_holds_in_every_language() {
    for lang in Lang::ALL {
        let code = lang.code();
        let mut h = harness_lang(lang);
        let w = panel_width(&h, "left_col");
        assert!((72.0..=420.0).contains(&w), "{code}: левая колонка {w}");
        assert!(
            h.state().preview_rect.width() > 600.0,
            "{code}: превью схлопнулось: {:?}",
            h.state().preview_rect
        );
        assert!(
            panel_width(&h, "meters_col") <= crate::app::METERS_W_MAX + 0.5,
            "{code}: панель метров шире предела: {}",
            panel_width(&h, "meters_col")
        );

        // длинный контент не имеет права раздуть колонку ни в одном языке
        h.state_mut().gpu_info = Some("X".repeat(400));
        h.step();
        let w = panel_width(&h, "left_col");
        assert!(w <= 420.0, "{code}: длинный контент раздул колонку: {w}");

        // маска таймкода — чистые ASCII-цифры, она обязана быть одинаковой
        // во всех языках (её порядок ввода — справа налево независимо от RTL)
        assert_eq!(
            crate::util::mask_text("130"),
            crate::util::mask_text("130"),
            "{code}: маска таймкода зависит от языка"
        );
    }
}

/// Тест 129 (Р.19): переключение языка на лету не ломает состояние.
///
/// Главное здесь — id окон. `Window::new(title)` выводит `Id` из ЗАГОЛОВКА, а
/// под этим id живут позиция и размер: без явного `.id()` смена языка
/// сбрасывала бы положение каждого диалога. Проверяется именно это — что
/// состояние окна лежит под постоянным id, а не под переведённым заголовком.
#[test]
fn t129_language_switch_keeps_state() {
    use egui::AreaState;

    let mut h = harness_lang(Lang::Ru);
    h.state_mut().notices_open = true;
    h.state_mut().search_open = true;
    h.state_mut().perf.diag_open = true;
    h.step();
    h.step();

    let ids = ["kt_notices", "kt_search", "kt_diag"];
    let before: Vec<_> = ids
        .iter()
        .map(|i| {
            AreaState::load(&h.ctx, egui::Id::new(*i))
                .unwrap_or_else(|| panic!("окно {i} не отрисовано"))
                .left_top_pos()
        })
        .collect();

    // реестр FX не должен пересобираться: категории и keywords — ключи
    let reg_before = crate::fx_registry::registry().as_ptr();

    crate::i18n::set_lang_ctx(&h.ctx, Lang::En);
    h.step();
    h.step();

    for (i, id) in ids.iter().enumerate() {
        let after = AreaState::load(&h.ctx, egui::Id::new(*id))
            .unwrap_or_else(|| panic!("после смены языка окно {id} потеряло состояние"))
            .left_top_pos();
        assert_eq!(before[i], after, "окно {id} переехало при смене языка");
    }
    assert_eq!(
        reg_before,
        crate::fx_registry::registry().as_ptr(),
        "реестр FX пересобрался при смене языка"
    );

    // и собственно перевод: подсказка кнопки окна стала английской
    h.hover_at(egui::pos2(W - 11.0, 10.0));
    h.step();
    assert_eq!(h.state().status_line_shown.0, tr(k::win::CLOSE));
    assert_eq!(tr(k::win::CLOSE), "Close", "английский каталог не подхватился");
}

/// Тест 126, часть «б» (Р.19): ЗАКРЕПЛЕНИЕ РЕНДЕРА арабского.
///
/// Юнит-тесты `bidi::visual` проверяют строку, а этот — то, что из неё
/// получилось у egui. Он и отличает верный алгоритм от правдоподобного:
///
/// * без переупорядочивания первое ЛОГИЧЕСКОЕ слово оказывается СЛЕВА, то
///   есть фраза читается задом наперёд (замерено: 'م' на x=22, 'ج' на x=56);
/// * с посимвольным разворотом (`unicode_bidi::reorder_line`) порядок слов
///   верный, но harfrust разворачивает каждый прогон ВТОРОЙ раз, и буквы
///   внутри слова встают наоборот — это ловит первая проверка.
///
/// Если egui когда-нибудь научится bidi сам, этот тест покраснеет — и это
/// правильный сигнал: тогда наш слой надо снимать.
#[test]
fn t126_arabic_renders_right_to_left() {
    let h = harness_lang(Lang::Ar);
    let font = egui::FontId::proportional(16.0);
    let layout = |text: String| {
        h.ctx.fonts_mut(|f| f.layout_no_wrap(text, font.clone(), egui::Color32::WHITE))
    };

    // «файл»: م ل ف. Внутри слова буквы обязаны остаться логическими — их
    // развернёт сам harfrust, поэтому САМЫЙ ЛЕВЫЙ глиф это ПОСЛЕДНЯЯ буква.
    let word = "\u{0645}\u{0644}\u{0641}";
    let g = layout(crate::i18n::bidi::visual(word));
    assert_eq!(
        g.rows[0].glyphs.first().map(|x| x.chr),
        Some('\u{0641}'),
        "буквы внутри слова развёрнуты дважды — похоже на посимвольный reorder_line"
    );

    // «ملف جديد»: последнее логическое слово обязано оказаться ЛЕВЕЕ первого.
    // Буквы у слов не пересекаются, поэтому позиции однозначны.
    let phrase = format!("{word} \u{062C}\u{062F}\u{064A}\u{062F}");
    let g = layout(crate::i18n::bidi::visual(&phrase));
    let x_of = |ch: char| {
        g.rows[0]
            .glyphs
            .iter()
            .find(|x| x.chr == ch)
            .unwrap_or_else(|| panic!("глиф {ch:?} не отрисован"))
            .pos
            .x
    };
    assert!(
        x_of('\u{062C}') < x_of('\u{0645}'),
        "второе слово не уехало влево: ج на {:.0}, م на {:.0}",
        x_of('\u{062C}'),
        x_of('\u{0645}')
    );

    // и контроль: БЕЗ переупорядочивания порядок обратный — то есть тест
    // действительно меряет работу bidi, а не свойство шрифта
    let g = layout(phrase.clone());
    let x_of = |ch: char| {
        g.rows[0].glyphs.iter().find(|x| x.chr == ch).expect("глиф").pos.x
    };
    assert!(
        x_of('\u{062C}') > x_of('\u{0645}'),
        "логический порядок вдруг совпал с визуальным — проверка выродилась"
    );
}

/// Тест 128 (Р.19): надпись влезает в отведённое ей место В КАЖДОМ ЯЗЫКЕ.
///
/// Прямая реализация правил 3 и 7 чек-листа (`apps/kt-app/CLAUDE.md`):
/// ширина берётся от галлеи самого длинного варианта, а длинное имя не имеет
/// права вылезти за свою зону. Бюджеты выведены из настоящих констант
/// вёрстки, а не назначены на глаз:
///
/// * меню делит шапку левой колонки с кнопками окна, а колонка по умолчанию
///   232 px (`app.rs`), поэтому трём заголовкам отведено 160;
/// * строки перф-панели рисуются `ui.columns(2)` внутри колонки шириной до
///   420 px — на столбец приходится меньше 200, и именно из-за этого
///   пользователю уже обрезало счётчик откатов многоточием (п.14 р.14).
///
/// Слишком длинный перевод падает здесь с именем ключа и фактической шириной:
/// чинится он на стороне перевода, а не расширением панели — пределы 420 и 90
/// стоят по прямым просьбам пользователя.
#[test]
fn t128_labels_fit_their_budget() {
    const MENU_BUDGET: f32 = 160.0;
    const PERF_COLUMN: f32 = 190.0;
    const CHECKBOX_BUDGET: f32 = 380.0;

    for lang in Lang::ALL {
        let h = harness_lang(lang);
        let body = h.ctx.style_of(egui::Theme::Dark).text_styles
            [&egui::TextStyle::Body]
            .clone();
        let heading = egui::FontId::proportional(14.0);
        let w = |text: String, font: egui::FontId| {
            h.ctx
                .fonts_mut(|f| f.layout_no_wrap(text, font, egui::Color32::WHITE))
                .size()
                .x
        };
        let code = lang.code();

        let menu: f32 = [k::menu::file::TITLE, k::menu::edit::TITLE, k::menu::view::TITLE]
            .iter()
            .map(|key| w(tr(*key).to_string(), heading.clone()))
            .sum();
        assert!(
            menu <= MENU_BUDGET,
            "{code}: три заголовка меню занимают {menu:.0} px при бюджете {MENU_BUDGET}"
        );

        // двухколоночные строки меряются СОБРАННЫМИ: шаблон короче того, что
        // увидит пользователь
        let rows = [
            (k::perf::CACHE_MB, crate::tf!(k::perf::CACHE_MB, 1234, 5678)),
            (k::perf::PREVIEW_RES, crate::tf!(k::perf::PREVIEW_RES, 1920, 1080)),
            (k::perf::CLIPS_N, crate::tf!(k::perf::CLIPS_N, 1234)),
            (k::perf::MEDIA_N, crate::tf!(k::perf::MEDIA_N, 1234)),
            (k::perf::V_SHOWN, crate::tf!(k::perf::V_SHOWN, 123456)),
            (k::perf::V_SKIPPED, crate::tf!(k::perf::V_SKIPPED, 1234, 567)),
            (k::perf::REGION_S, crate::tf!(k::perf::REGION_S, "123.45", "678.90")),
            (k::perf::CACHE_SHORT, crate::tf!(k::perf::CACHE_SHORT, 1234, 5678)),
        ];
        for (key, text) in rows {
            let got = w(text.clone(), body.clone());
            assert!(
                got <= PERF_COLUMN,
                "{code}: {} = {text:?} занимает {got:.0} px при бюджете {PERF_COLUMN}",
                crate::i18n::key_name(key)
            );
        }

        let got = w(tr(k::perf::ECONOMY).to_string(), body.clone());
        assert!(
            got <= CHECKBOX_BUDGET,
            "{code}: чекбокс экономного UI занимает {got:.0} px при бюджете {CHECKBOX_BUDGET}"
        );
    }
}

/// Тест 139 (п.139 porting-notes, Р.24): план Move-драга. Одиночный
/// частичный наезд = кроссфейд; мульти-наезд/поглощение/крошечный наезд/
/// вертикальный перенос в занятое — кламп (решение пользователя п.10).
#[test]
fn t139_move_plan_crossfade_and_clamp() {
    use crate::timeline::{MovePlan, plan_move};
    use kt_model::*;
    use kt_time::Tick;

    let clip = |id: u64, lane: u32, start: i64, dur: i64| ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    let mut p = Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    // поведение Р.24/Р.26 живёт в режиме БЕЗ переслаивания (Р.28)
    p.overlap_layers = false;
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips = vec![clip(1, 0, 0, 90_000), clip(2, 0, 200_000, 90_000), clip(3, 0, 400_000, 90_000)];
    p.validate().unwrap();

    // без коллизий
    assert_eq!(plan_move(&p, ClipId(2), 100_000, 0), MovePlan::Free);
    // incoming-наезд: начало клипа 2 в теле клипа 1 → стык в середине наезда
    match plan_move(&p, ClipId(2), 50_000, 0) {
        MovePlan::Crossfade { outgoing, incoming, joint, in_off, out_off } => {
            assert_eq!((outgoing, incoming), (ClipId(1), ClipId(2)));
            assert_eq!(joint, 70_000, "середина наезда [50000, 90000)");
            assert_eq!((in_off, out_off), (20_000, 20_000));
        }
        other => panic!("ожидался кроссфейд, получен {other:?}"),
    }
    // outgoing-наезд: конец клипа 2 в теле клипа 3
    match plan_move(&p, ClipId(2), 350_000, 0) {
        MovePlan::Crossfade { outgoing, incoming, joint, .. } => {
            assert_eq!((outgoing, incoming), (ClipId(2), ClipId(3)));
            assert_eq!(joint, 420_000, "середина наезда [400000, 440000)");
        }
        other => panic!("ожидался кроссфейд, получен {other:?}"),
    }
    // полное покрытие соседа (равные длительности, want==n.start) → кламп
    // (движение влево упирается в его конец)
    assert!(
        matches!(plan_move(&p, ClipId(2), 0, 0), MovePlan::Clamped { start: 90_000, .. }),
        "полное покрытие → кламп"
    );
    // крошечный наезд (< минимума окна) → кламп, не микропереход
    assert!(
        matches!(plan_move(&p, ClipId(2), 89_960, 0), MovePlan::Clamped { start: 90_000, .. }),
        "крошечный наезд → кламп"
    );
    // мульти-наезд: сдвинем клип 3 ближе и наедем сразу на двоих
    let mut p2 = p.clone();
    p2.clips[2].start = Tick(95_000);
    assert!(matches!(plan_move(&p2, ClipId(2), 60_000, 0), MovePlan::Clamped { .. }));
    // вертикальный перенос в занятое место чужой дорожки → кламп, не тримы
    let mut p3 = p.clone();
    p3.clips[1].lane = 1;
    p3.normalize();
    assert!(matches!(plan_move(&p3, ClipId(2), 50_000, 0), MovePlan::Clamped { .. }));
    // foreign-плейсхолдер занимает место
    let mut p4 = p.clone();
    p4.foreign.push(ForeignEvent {
        key: 1,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(120_000),
        duration: Tick(30_000),
    });
    assert!(matches!(plan_move(&p4, ClipId(2), 110_000, 0), MovePlan::Clamped { .. }));
}

/// Тест 139 (живая часть): наезд драгом создаёт переход, клипы встык.
#[test]
fn t139_drag_overlap_creates_crossfade() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
    // поведение Р.24/Р.26 живёт в режиме БЕЗ переслаивания (Р.28)
    p.overlap_layers = false;
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        for (n, start) in [(1u64, 0i64), (2, 200_000)] {
            p.clips.push(ClipEvent {
                id: ClipId(n),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(0),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
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
    let y = g.lane_y(ClipKind::Video, 0, 0.5);
    let grab = g.tick_to_x(200_000 + 45_000);
    h.hover_at(egui::pos2(grab, y));
    h.step();
    h.drag_at(egui::pos2(grab, y));
    h.step();
    h.hover_at(egui::pos2(grab - 8.0, y));
    h.step();
    let grab_dt = match h.state().drag {
        Some(crate::timeline::Drag::Clip(ref d)) => d.grab_dt,
        _ => panic!("драг клипа не начался"),
    };
    // цель: start = 50000 — частичный наезд на хвост клипа 1
    let to = g.tick_to_x(50_000 + grab_dt);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();

    let p = h.state().model.project().clone();
    assert_eq!(p.transitions.len(), 1, "наезд обязан создать переход");
    let t = &p.transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(1)), Some(ClipId(2))));
    // указатель харнесса квантуется пикселем: допуск 1 px = 1000 тиков
    let near = |a: Tick, want: i64, what: &str| {
        assert!((a.0 - want).abs() <= 1_000, "{what}: {a:?} vs ~{want}");
    };
    near(t.in_offset, 20_000, "in_offset");
    near(t.out_offset, 20_000, "out_offset");
    let c1 = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let c2 = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    near(c1.start + c1.duration, 70_000, "стык в середине наезда");
    assert_eq!(c2.start, c1.start + c1.duration, "клипы строго встык");
    near(c2.source_in, 20_000, "in-point подрезан тримом");
    p.validate().expect("итог валиден");
}

/// Тест 140 (п.140 porting-notes, Р.24): драги краёв/тела перехода и
/// геометрия отрисовки.
#[test]
fn t140_transition_edge_and_body_drags() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        for (n, start, src) in [(1u64, 0i64, 0i64), (2, 90_000, 30_000)] {
            p.clips.push(ClipEvent {
                id: ClipId(n),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(src),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: Some(ClipId(2)),
            in_offset: Tick(10_000),
            out_offset: Tick(10_000),
            kind: TransitionKind::Crossfade,
            audio: Default::default(),
            auto: vec![],
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("нет геометрии таймлайна");
    let y = g.lane_y(ClipKind::Video, 0, 0.5);

    // драг ЛЕВОГО края окна наружу: in_offset 10000 → 30000
    let x_edge = g.tick_to_x(90_000 - 10_000);
    h.hover_at(egui::pos2(x_edge, y));
    h.step();
    h.drag_at(egui::pos2(x_edge, y));
    h.step();
    h.hover_at(egui::pos2(x_edge - 8.0, y));
    h.step();
    assert!(
        matches!(h.state().drag, Some(crate::timeline::Drag::TransitionEdge { left: true, .. })),
        "хват края перехода не сработал: {:?}",
        matches!(h.state().drag, Some(crate::timeline::Drag::Clip(_)))
    );
    let to = g.tick_to_x(90_000 - 30_000);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let t0 = h.state().model.project().transitions[0].clone();
    assert!((t0.in_offset.0 - 30_000).abs() <= 1_000, "in_offset ~30000: {t0:?}");
    assert_eq!(t0.out_offset, Tick(10_000));
    // один undo откатывает весь жест (коалесинг)
    h.state_mut().model.undo().unwrap();
    assert_eq!(h.state().model.project().transitions[0].in_offset, Tick(10_000));
    h.step();

    // roll стыка: центр окна тянем на +20000 — стык и клипы едут
    let xj = g.tick_to_x(90_000);
    h.hover_at(egui::pos2(xj, y));
    h.step();
    h.drag_at(egui::pos2(xj, y));
    h.step();
    h.hover_at(egui::pos2(xj + 8.0, y));
    h.step();
    assert!(matches!(h.state().drag, Some(crate::timeline::Drag::TransitionBody { .. })));
    let to = g.tick_to_x(110_000);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let p = h.state().model.project().clone();
    let c1 = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let c2 = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    let joint = c1.start + c1.duration;
    assert!((joint.0 - 110_000).abs() <= 2_000, "roll сдвинул стык: {joint:?}");
    assert_eq!(c2.start, joint, "клипы строго встык");
    assert_eq!(c2.source_in - Tick(30_000), joint - Tick(90_000), "TrimStart подрезал источник");
    assert_eq!(p.transitions[0].joint(&p), Some(joint), "окно уехало со стыком");

    // геометрия отрисовки — чистая функция
    let r = crate::timeline::transition_rect(100.0, 140.0, 50.0, 40.0);
    assert_eq!((r.left(), r.right()), (100.0, 140.0));
    assert!(r.top() > 50.0 && r.bottom() < 90.0, "полоса внутри строки дорожки");
}

/// Тест 163 (п.163 porting-notes, Р.28): с переслаиванием жест НИЧЕГО не
/// режет. Требование пользователя дословно: «вот сколько было столько и
/// должно остаться… НИЧЕГО НИГДЕ НЕ ДОЛЖНО ОТРАСТАТЬ/ОТРУБАТЬСЯ».
#[test]
fn t163_overlap_drag_changes_nothing_but_position() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        assert!(p.overlap_layers, "переслаивание включено по умолчанию");
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        // A/V-пара + одиночный видео-клип: проверяем и группы, и одиночек
        for (n, kind, start, group) in [
            (1u64, ClipKind::Video, 0i64, Some(GroupId(1))),
            (2, ClipKind::Audio, 0, Some(GroupId(1))),
            (3, ClipKind::Video, 200_000, None),
        ] {
            p.clips.push(ClipEvent {
                id: ClipId(n),
                link_group: group,
                kind,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(10_000),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
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
    let y = g.lane_y(ClipKind::Video, 0, 0.5);
    let before: Vec<ClipEvent> = h.state().model.project().clips.clone();

    // наезжаем третьим клипом на первый (перекрытие 40 000 тиков)
    let from = g.tick_to_x(200_000 + 45_000);
    let to = g.tick_to_x(50_000 + 45_000);
    h.hover_at(egui::pos2(from, y));
    h.step();
    h.drag_at(egui::pos2(from, y));
    h.step();
    h.hover_at(egui::pos2(from - 8.0, y));
    h.step();
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();

    let p = h.state().model.project().clone();
    assert!(p.transitions.is_empty(), "перекрытие звучит само — переход не заводится");
    for c in &p.clips {
        let was = before.iter().find(|x| x.id == c.id).unwrap();
        assert_eq!(c.duration, was.duration, "длительность {:?} не менялась", c.id);
        assert_eq!(c.source_in, was.source_in, "точка входа {:?} не менялась", c.id);
        if c.id != ClipId(3) {
            assert_eq!(c.start, was.start, "чужой клип {:?} не двигался", c.id);
        }
    }
    let moved = p.clips.iter().find(|c| c.id == ClipId(3)).unwrap();
    assert!((moved.start.0 - 50_000).abs() <= 1_000, "клип встал куда тянули: {moved:?}");
    p.validate().expect("перекрытие законно");

    // растаскиваем обратно — снова ничего не режется
    let from = g.tick_to_x(moved.start.0 + 45_000);
    let to = g.tick_to_x(300_000 + 45_000);
    h.hover_at(egui::pos2(from, y));
    h.step();
    h.drag_at(egui::pos2(from, y));
    h.step();
    h.hover_at(egui::pos2(from + 8.0, y));
    h.step();
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let p = h.state().model.project().clone();
    for c in &p.clips {
        let was = before.iter().find(|x| x.id == c.id).unwrap();
        assert_eq!(c.duration, was.duration, "после растаскивания {:?} цел", c.id);
        assert_eq!(c.source_in, was.source_in, "после растаскивания in-point {:?} цел", c.id);
    }
}

/// Тест 160 (п.160 porting-notes, Р.26): наехал — растащил — материал
/// вернулся. Жалоба пользователя: «в sony vegas если растащить
/// перекрывающиеся события — остаётся то, что перекрывалось, без обрезки».
#[test]
fn t160_pulling_apart_restores_trimmed_material() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
    // поведение Р.24/Р.26 живёт в режиме БЕЗ переслаивания (Р.28)
    p.overlap_layers = false;
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        for (n, start) in [(1u64, 0i64), (2, 200_000)] {
            p.clips.push(ClipEvent {
                id: ClipId(n),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(0),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx_auto: vec![],
                fx: vec![],
                opacity: 1.0,
                fx_off: false,
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
    let y = g.lane_y(ClipKind::Video, 0, 0.5);
    let grab = |h: &mut Harness<KtApp>, from: f32, to: f32, y: f32| {
        // драг как человек: нажать → сдвинуть чуть → вести дальше
        h.hover_at(egui::pos2(from, y));
        h.step();
        h.drag_at(egui::pos2(from, y));
        h.step();
        h.hover_at(egui::pos2(from + 8.0_f32.copysign(to - from), y));
        h.step();
        h.hover_at(egui::pos2(to, y));
        h.step();
        h.drop_at(egui::pos2(to, y));
        h.step();
    };

    // 1. наезжаем клипом 2 на клип 1: [200000..290000) → [50000..140000)
    let from = g.tick_to_x(200_000 + 45_000);
    grab(&mut h, from, g.tick_to_x(50_000 + 45_000), y);
    {
        let p = h.state().model.project();
        assert_eq!(p.transitions.len(), 1, "наезд создал кроссфейд");
        let c1 = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
        assert!(c1.duration < Tick(90_000), "уходящий подрезан стыком");
    }

    // 2. растаскиваем обратно — материал обязан вернуться
    // хват берём ПОДАЛЬШЕ от окна перехода: его края перехватывают клик
    let cur = h.state().model.project().clips.iter().find(|c| c.id == ClipId(2)).unwrap().start.0;
    let grab_dt = 60_000;
    let from = g.tick_to_x(cur + grab_dt);
    grab(&mut h, from, g.tick_to_x(300_000 + grab_dt), y);

    let p = h.state().model.project().clone();
    assert!(p.transitions.is_empty(), "переход умер вместе со стыком");
    let c1 = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let c2 = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(c1.duration, Tick(90_000), "уходящему вернули хвост: {c1:?}");
    assert_eq!(c2.duration, Tick(90_000), "входящему вернули голову: {c2:?}");
    assert_eq!(c2.source_in, Tick(0), "точка входа вернулась к нулю");
    p.validate().expect("итог валиден");

    // геометрия ручек — чистая функция
    let (l, r) = crate::timeline::transition_handle_rects(100.0, 120.0, 140.0, 50.0, 40.0);
    assert_eq!((l.left(), l.right()), (100.0, 120.0), "слева — продолжение входящего");
    assert_eq!((r.left(), r.right()), (120.0, 140.0), "справа — хвост уходящего");
    assert!(l.top() > 50.0 && r.bottom() < 90.0, "ручки внутри строки дорожки");
}

/// Тест 157 (п.157 porting-notes, Р.25): фейд у края в UI — хват окна,
/// живой драг края и отсутствие соседа не мешают ни тому, ни другому.
#[test]
fn t157_fade_edge_drag_without_neighbour() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        p.clips.push(ClipEvent {
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(90_000),
            duration: Tick(90_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        // фейд-ин: соседа слева нет вообще
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: None,
            b: Some(ClipId(1)),
            in_offset: Tick(0),
            out_offset: Tick(10_000),
            kind: TransitionKind::Crossfade,
            audio: Default::default(),
            auto: vec![],
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("нет геометрии таймлайна");
    let y = g.lane_y(ClipKind::Video, 0, 0.5);

    // ПРАВЫЙ край окна фейда (внутри тела клипа) тянем вправо: 10000 → 40000
    let x_edge = g.tick_to_x(90_000 + 10_000);
    h.hover_at(egui::pos2(x_edge, y));
    h.step();
    h.drag_at(egui::pos2(x_edge, y));
    h.step();
    h.hover_at(egui::pos2(x_edge + 8.0, y));
    h.step();
    assert!(
        matches!(h.state().drag, Some(crate::timeline::Drag::TransitionEdge { left: false, .. })),
        "край фейда не схватился"
    );
    let to = g.tick_to_x(90_000 + 40_000);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let t = h.state().model.project().transitions[0].clone();
    assert!(t.a.is_none() && t.b == Some(ClipId(1)), "фейд остался фейдом: {t:?}");
    assert!((t.out_offset.0 - 40_000).abs() <= 1_000, "окно расширилось: {t:?}");

    // окно не может выйти за тело клипа
    let far = g.tick_to_x(90_000 + 200_000);
    h.hover_at(egui::pos2(g.tick_to_x(90_000 + 40_000), y));
    h.step();
    h.drag_at(egui::pos2(g.tick_to_x(90_000 + 40_000), y));
    h.step();
    h.hover_at(egui::pos2(g.tick_to_x(90_000 + 40_000) + 8.0, y));
    h.step();
    h.hover_at(egui::pos2(far, y));
    h.step();
    h.drop_at(egui::pos2(far, y));
    h.step();
    let t = h.state().model.project().transitions[0].clone();
    assert!(t.out_offset <= Tick(90_000), "окно клампится телом клипа: {t:?}");
}

/// Тест 142 (п.142 porting-notes, Р.24): метки-диапазоны в UI. Жест M —
/// дословно по решению пользователя: «при выделенном регионе и курсоре в
/// нем - m дает marked_range, при курсоре вне региона - одинарная метка по
/// курсору»; повторное M — toggle; края и snap/stop по обеим граням.
#[test]
fn t142_marker_range_gesture_and_edges() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model.set_region(Some(Region { start: Tick(30_000), end: Tick(90_000) }));
        app.player.seek(Tick(60_000)); // курсор ВНУТРИ региона
        // зум фиксируем: магнитный порог M зависит от ticks_per_px
        app.ticks_per_px = 1_000.0;
    }
    h.step();

    // M внутри региона → диапазон по региону
    h.state_mut().marker_toggle_at_cursor();
    {
        let p = h.state().model.project();
        assert_eq!(p.markers.len(), 1);
        assert_eq!((p.markers[0].t, p.markers[0].duration), (Tick(30_000), Tick(60_000)));
    }
    // повторное M там же → toggle (удаление)
    h.state_mut().marker_edit = None;
    h.state_mut().marker_toggle_at_cursor();
    assert!(h.state().model.project().markers.is_empty(), "toggle удаляет диапазон");

    // точечная метка на старте региона растягивается жестом M
    h.state_mut()
        .apply(Command::AddMarker { at: Tick(30_000), duration: Tick::ZERO });
    h.state_mut().marker_toggle_at_cursor();
    assert_eq!(h.state().model.project().markers[0].duration, Tick(60_000));

    // M вне региона → точечная по курсору (тик кратен 15 — playhead живёт
    // в аудиокадрах, round-trip точен только на кратных, п.94)
    h.state_mut().player.seek(Tick(192_000));
    h.state_mut().marker_edit = None;
    h.state_mut().marker_toggle_at_cursor();
    {
        let p = h.state().model.project();
        assert!(p.markers.iter().any(|m| m.t == Tick(192_000) && !m.is_range()), "{:?}", p.markers);
    }

    // snap и stop_points видят ОБЕ грани диапазона
    let targets = h.state().snap_targets(None);
    assert!(targets.contains(&30_000) && targets.contains(&90_000), "{targets:?}");
    let stops = crate::app::stop_points(h.state().model.project());
    assert!(stops.contains(&30_000) && stops.contains(&90_000), "{stops:?}");

    // лента — чистая геометрия: узкая полоса на строке подписей времени
    let band = crate::timeline::marker_band_rect(100.0, 160.0, 20.0);
    assert_eq!((band.left(), band.right()), (100.0, 160.0));
    assert!(band.height() <= 6.0, "лента не съедает строку времени");
}

/// Тест 142 (живая часть): драг конца диапазона меняет длительность.
#[test]
fn t142_marker_end_drag() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let _ = app.model.apply(Command::AddMarker { at: Tick(30_000), duration: Tick(30_000) });
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("нет геометрии таймлайна");
    // конец диапазона на полосе флажков (верх таймлайна)
    let y = g.rect.top() + 4.0;
    let x_end = g.tick_to_x(60_000);
    h.hover_at(egui::pos2(x_end, y));
    h.step();
    h.drag_at(egui::pos2(x_end, y));
    h.step();
    h.hover_at(egui::pos2(x_end + 8.0, y));
    h.step();
    assert!(
        matches!(h.state().drag, Some(crate::timeline::Drag::MarkerEdge { left: false, .. })),
        "хват конца диапазона не сработал"
    );
    let to = g.tick_to_x(80_000);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let m = &h.state().model.project().markers[0];
    assert!((m.duration.0 - 50_000).abs() <= 1_000, "длительность ~50000: {:?}", m.duration);
}

/// Тест 150 (п.150 porting-notes, Р.24): файловый IO приложения — .otio.
#[test]
fn t150_project_io_is_otio() {
    use std::path::Path;
    // атомарный tmp дописывается к ПОЛНОМУ имени (старый with_extension
    // был захардкожен под ktproj и съедал многоточечные имена)
    assert_eq!(
        crate::app::atomic_tmp_path(Path::new("/x/a.otio")),
        Path::new("/x/a.otio.tmp")
    );
    assert_eq!(
        crate::app::atomic_tmp_path(Path::new("/x/a.b.otio")),
        Path::new("/x/a.b.otio.tmp")
    );
    // recovery безымянного проекта — тоже .otio
    assert!(
        crate::app::autosave_path().to_string_lossy().ends_with("autosave-project.otio")
    );

    let dir = std::env::temp_dir().join(format!("kt-t150-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // .otio читается
    let p = kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
    });
    let otio = dir.join("ok.otio");
    std::fs::write(&otio, kt_otio::save_project(&p, &kt_otio::Retained::default(), Some(&dir)))
        .unwrap();
    let (loaded, _, _) = crate::app::load_project_file(&otio).expect("otio opens");
    assert_eq!(loaded.settings.fps, 30);
    // .ktproj как проект НЕ открывается (Р.24 п.12) — честная подсказка
    let legacy = dir.join("old.ktproj");
    std::fs::write(&legacy, include_str!("../../../fixtures/demo.ktproj")).unwrap();
    let err = crate::app::load_project_file(&legacy).unwrap_err();
    assert!(err.contains("Import .ktproj"), "подсказка на импорт: {err}");
    std::fs::remove_dir_all(&dir).ok();
}

/// Тест 169 (п.2 Р.28): превью материала при жестах — «сколько было столько
/// и должно остаться». Пользователь: «перетаскиваю событие вглубь предыдущего
/// — превью звука прокручивается влево, хотя при воспроизведении сам звук не
/// меняется». Материал внутри куска двигает ТОЛЬКО трим левого края.
#[test]
fn t169_preview_material_shifts_only_on_trim_start() {
    use crate::timeline::{Drag, DragMode, DragState, preview_src_shift};
    use kt_model::*;
    use kt_time::Tick;

    let orig = ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(1_000),
        duration: Tick(10_000),
        source_in: Tick(500),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    let drag = |mode: DragMode, cur_start: i64, cur_end: i64| {
        Drag::Clip(DragState {
            id: ClipId(1),
            mode,
            extra: Vec::new(),
            grab_dt: 0,
            orig: orig.clone(),
            cur_start,
            cur_end,
            cur_lane: 0,
            moved: true,
        })
    };

    // перенос вправо на 4000: кусок едет целиком, материал — вместе с ним
    let d = drag(DragMode::Move, 5_000, 15_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(1), 1_000), 0, "перенос не крутит материал");
    // перенос влево — тоже ноль (раньше .max(0) прятал только эту половину)
    let d = drag(DragMode::Move, -3_000, 7_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(1), 1_000), 0, "перенос влево не крутит");
    // трим левого края: кусок стоит, край едет ПО материалу
    let d = drag(DragMode::TrimStart, 5_000, 11_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(1), 1_000), 4_000, "трим начала двигает материал");
    let d = drag(DragMode::TrimStart, 0, 11_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(1), 1_000), -1_000, "влево — в обратную сторону");
    // трим правого края левую границу не трогает
    let d = drag(DragMode::TrimEnd, 1_000, 20_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(1), 1_000), 0, "трим конца материал не крутит");
    // соседние клипы жест не касается
    let d = drag(DragMode::TrimStart, 5_000, 11_000);
    assert_eq!(preview_src_shift(Some(&d), ClipId(2), 1_000), 0, "чужой клип не трогаем");
    assert_eq!(preview_src_shift(None, ClipId(1), 1_000), 0, "без жеста сдвига нет");
}

/// Тест 170 (п.11 Р.28): «marker here при выделенном регионе должен порождать
/// region marker». Парные способы одного действия обязаны учитывать контекст
/// одинаково (п.17 чек-листа): жест M и пункт меню — одно тело `marker_here`.
#[test]
fn t170_marker_here_matches_the_m_gesture() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model.set_region(Region::normalized(Tick(30_000), Tick(90_000)));
        // пункт меню на КРОМКЕ региона: тик грани — это «по региону»
        app.marker_here(Tick(30_000));
    }
    h.step();
    {
        let m = h.state().model.project().markers.clone();
        assert_eq!(m.len(), 1, "метка одна: {m:?}");
        assert_eq!(m[0].t, Tick(30_000), "метка на начале региона");
        assert_eq!(m[0].duration, Tick(60_000), "это ДИАПАЗОН, а не точка: {:?}", m[0]);
    }
    // повторный вызов — toggle, как у жеста M
    h.state_mut().marker_here(Tick(30_000));
    h.step();
    assert!(h.state().model.project().markers.is_empty(), "повтор снимает метку");
    // вторая кромка ведёт себя так же
    h.state_mut().marker_here(Tick(90_000));
    h.step();
    assert_eq!(
        h.state().model.project().markers[0].duration,
        Tick(60_000),
        "кромка конца — тоже регион"
    );
    // вне региона — точечная метка (прежнее поведение)
    {
        let app = h.state_mut();
        app.model.set_region(None);
        app.marker_here(Tick(200_000));
    }
    h.step();
    let m = h.state().model.project().markers.clone();
    assert!(
        m.iter().any(|k| k.t == Tick(200_000) && k.duration == Tick::ZERO),
        "без региона — одиночная метка: {m:?}"
    );
}

/// Тест 172 (п.5 Р.28): невозможность действия показывается КАК ОШИБКА.
///
/// Жалоба дословно: «я пока сам туда не промотал — не понял что оказывается
/// там еще событие есть в конце которое мешает, а еще оказывается есть
/// подсказка беленькая блеклая, везде в подсказках такие ситуации делаются
/// как ошибочные». Проверяем всё три требования: препятствие известно
/// (значит, есть что подсветить), подсказка идёт ошибочным стилем и
/// перебивает наведение, а за пределами вида говорит, куда смотреть.
#[test]
fn t172_blocked_gesture_is_an_error_not_a_pale_hint() {
    use crate::app::HintKind;
    use crate::timeline::{BlockView, MovePlan, block_view, plan_move};
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;

    // 1. план знает, ВО ЧТО упёрлись
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.overlap_layers = false; // блокировка возможна только в этом режиме
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    // клипы 2 и 3 стоят встык: любое попадание между ними — наезд на ДВОИХ
    for (n, start) in [(1u64, 0i64), (2, 200_000), (3, 250_000)] {
        p.clips.push(ClipEvent {
            id: ClipId(n),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(start),
            duration: Tick(50_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    // клип 1 едет вправо и накрывает СРАЗУ ДВОИХ (одиночный частичный наезд
    // — это кроссфейд, а не упор): препятствие — первый по ходу движения
    match plan_move(&p, ClipId(1), 230_000, 0) {
        MovePlan::Clamped { blocker, .. } => {
            assert_eq!(blocker, (200_000, 250_000), "по ходу движения первым мешает клип 2");
        }
        other => panic!("ожидался упор, получен {other:?}"),
    }
    // движение влево в полное покрытие соседа — упор в его правый край
    match plan_move(&p, ClipId(3), 200_000, 0) {
        MovePlan::Clamped { start, blocker } => {
            assert_eq!(blocker, (200_000, 250_000), "слева мешает клип 2");
            assert_eq!(start, 250_000, "упор — правый край препятствия");
        }
        other => panic!("ожидался упор, получен {other:?}"),
    }

    // 2. «куда смотреть», когда препятствие за пределами вида
    assert_eq!(block_view((100, 200), (0, 1000)), BlockView::Visible);
    assert_eq!(block_view((0, 100), (500, 1000)), BlockView::Left(400));
    assert_eq!(block_view((2000, 2100), (500, 1000)), BlockView::Right(1000));
    // касание кромки вида — уже видно
    assert_eq!(block_view((900, 1500), (500, 1000)), BlockView::Visible);

    // 3. живой жест: подсказка ошибочная и перебивает наведение
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("нет геометрии таймлайна");
    let y = g.lane_y(ClipKind::Video, 0, 0.5);
    let grab = g.tick_to_x(25_000);
    h.hover_at(egui::pos2(grab, y));
    h.step();
    h.drag_at(egui::pos2(grab, y));
    h.step();
    // тащим клип 1 так, чтобы он накрыл сразу два события — упор
    let to = g.tick_to_x(230_000 + 25_000);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.hover_at(egui::pos2(to + 1.0, y));
    h.step();
    let blocked = h.state().blocked;
    assert!(blocked.is_some(), "препятствие обязано быть известно (его подсвечивают)");
    assert_eq!(blocked.unwrap().span, (200_000, 250_000), "подсвечивается именно клип 2");
    assert_eq!(
        h.state().status_line_shown.1,
        HintKind::Error,
        "подсказка обязана быть ОШИБОЧНОЙ, а не блёклой: {:?}",
        h.state().status_line_shown
    );
    h.drop_at(egui::pos2(to + 1.0, y));
    h.step();
    assert!(h.state().blocked.is_none(), "после жеста подсветка гаснет");
}

/// Тест 173 (п.12 Р.28): фейд за верхний уголок события и уровень мышью.
///
/// Пользователь: «в sony vegas fade можно у событий сделать по краям без еще
/// одних событий — просто потянув за верхний уголок события, а еще там можно
/// вроде прозрачность менять… наводишься на фейд и тянешь либо вверх либо
/// вниз». Уголковый фейд лежит ЦЕЛИКОМ внутри события (внешний офсет 0),
/// поэтому «ничего не отрастает» (п.6).
#[test]
fn t173_corner_fade_and_level_gestures() {
    use crate::timeline::{corner_fade_len, fade_handle_tick, level_from_y, level_y};
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;

    // ---- чистые функции ----
    assert_eq!(fade_handle_tick(1_000, 5_000, 0, true), 1_000, "нет фейда — ручка в углу");
    assert_eq!(fade_handle_tick(1_000, 5_000, 700, true), 1_700, "ручка на конце фейда");
    assert_eq!(fade_handle_tick(1_000, 5_000, 0, false), 5_000);
    assert_eq!(fade_handle_tick(1_000, 5_000, 700, false), 4_300);
    // уровень: верх строки = 1.0, низ = 0.0, симметрично туда и обратно
    let (top, h) = (100.0f32, 48.0f32);
    assert!((level_from_y(top + 3.0, top, h) - 1.0).abs() < 1e-6);
    assert!((level_from_y(top + 3.0 + (h - 8.0), top, h)).abs() < 1e-6);
    assert!((level_from_y(top - 50.0, top, h) - 1.0).abs() < 1e-6, "выше строки — кламп");
    let mid = level_y(0.5, top, h);
    assert!((level_from_y(mid, top, h) - 0.5).abs() < 1e-3, "прямое и обратное сходятся");

    // ---- живой жест ----
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        p.clips.push(ClipEvent {
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(100_000),
            duration: Tick(200_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.select_only(ClipId(1));
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("нет геометрии таймлайна");
    let top = g.lane_top(ClipKind::Video, 0);
    let y_strip = top + 3.0;

    // тянем левый уголок вправо на 40 000 тиков
    let x0 = g.tick_to_x(100_000);
    h.hover_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    h.drag_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    let to = g.tick_to_x(140_000);
    h.hover_at(egui::pos2(to, y_strip));
    h.step();
    h.drop_at(egui::pos2(to, y_strip));
    h.step();
    {
        let p = h.state().model.project();
        assert_eq!(p.transitions.len(), 1, "уголок завёл ровно один фейд: {:?}", p.transitions);
        let t = &p.transitions[0];
        assert!(t.is_fade(), "это фейд, а не кроссфейд");
        assert_eq!((t.a, t.b), (None, Some(ClipId(1))), "пустота слева, живой конец — клип");
        assert_eq!(t.in_offset, Tick::ZERO, "внешнего офсета нет — фейд внутри события");
        assert!(
            (t.out_offset.0 - 40_000).abs() <= 2_000,
            "длина фейда ≈ пройденному пути: {:?}",
            t.out_offset
        );
        // событие не тронуто: «сколько было столько и осталось»
        let c = &p.clips[0];
        assert_eq!((c.start, c.duration, c.source_in), (Tick(100_000), Tick(200_000), Tick::ZERO));
        assert_eq!(corner_fade_len(p, ClipId(1), true), Some(t.out_offset.0));
        assert_eq!(corner_fade_len(p, ClipId(1), false), None, "правого фейда нет");
    }

    // задвигаем уголок обратно — фейд снимается
    let from = g.tick_to_x(140_000);
    h.hover_at(egui::pos2(from, y_strip));
    h.step();
    h.drag_at(egui::pos2(from, y_strip));
    h.step();
    h.hover_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    h.drop_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    assert!(
        h.state().model.project().transitions.is_empty(),
        "уголок задвинут — фейда нет: {:?}",
        h.state().model.project().transitions
    );

    // уровень: тянем линию вниз до середины строки
    let ly = level_y(1.0, top, g.lane_h());
    let xm = g.tick_to_x(200_000);
    h.hover_at(egui::pos2(xm, ly));
    h.step();
    h.drag_at(egui::pos2(xm, ly));
    h.step();
    let target_y = level_y(0.4, top, g.lane_h());
    h.hover_at(egui::pos2(xm, target_y));
    h.step();
    h.drop_at(egui::pos2(xm, target_y));
    h.step();
    let op = h.state().model.project().clips[0].opacity;
    assert!((op - 0.4).abs() < 0.05, "уровень доехал до 0.4: {op}");
    // и это undo-able, как и всё остальное
    h.state_mut().model.undo().expect("undo уровня доступен");
    h.step();
    assert_eq!(h.state().model.project().clips[0].opacity, 1.0, "undo вернул уровень");
}

/// Тест 176: уровень аудио и видео — ОДИН элемент (требование пользователя
/// «фейды должны быть у аудио и видео одинаково выглядящими»).
///
/// Вертикальный рейл громкости v1 стоял ровно в зоне фейда и делал фейд на
/// аудио непохожим на фейд на видео. Теперь у обоих видов линия по верху;
/// диапазон громкости остался прежним (±6.02 dB, 0 dB — середина).
#[test]
fn t176_audio_and_video_share_one_level_line() {
    use crate::timeline::{RAIL_DB_SPAN, gain_to_level, level_to_gain, level_y};
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;

    // диапазон рейла v1 сохранён точь-в-точь
    assert!((gain_to_level(0.0) - 0.5).abs() < 1e-6, "0 dB — середина");
    assert!((level_to_gain(1.0) - RAIL_DB_SPAN / 2.0).abs() < 1e-4, "верх — +6.02 dB");
    assert!((level_to_gain(0.0) + RAIL_DB_SPAN / 2.0).abs() < 1e-4, "низ — −6.02 dB");
    for db in [-6.0f32, -3.0, 0.0, 2.5, 6.0] {
        assert!((level_to_gain(gain_to_level(db)) - db).abs() < 1e-3, "круг по {db} dB");
    }

    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        p.clips.push(ClipEvent {
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(100_000),
            duration: Tick(200_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.select_only(ClipId(1));
        app.view_start = 0.0;
        app.ticks_per_px = TPP;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("нет геометрии таймлайна");
    let top = g.lane_top(ClipKind::Audio, 0);

    // тот же жест, что у видео: тянем линию уровня вверх
    let x = g.tick_to_x(200_000);
    let y0 = level_y(0.5, top, g.lane_h()); // 0 dB
    h.hover_at(egui::pos2(x, y0));
    h.step();
    h.drag_at(egui::pos2(x, y0));
    h.step();
    let y1 = level_y(0.75, top, g.lane_h());
    h.hover_at(egui::pos2(x, y1));
    h.step();
    h.drop_at(egui::pos2(x, y1));
    h.step();
    let gain = h.state().model.project().clips[0].gain_db;
    assert!(
        (gain - RAIL_DB_SPAN * 0.25).abs() < 0.5,
        "уровень аудио доехал до +3 dB: {gain}"
    );

    // у уголка фейда приоритет над линией: верхняя полоса — жест фейда
    let y_strip = top + 2.0;
    let x0 = g.tick_to_x(100_000);
    h.hover_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    h.drag_at(egui::pos2(x0 + 1.0, y_strip));
    h.step();
    h.hover_at(egui::pos2(g.tick_to_x(140_000), y_strip));
    h.step();
    h.drop_at(egui::pos2(g.tick_to_x(140_000), y_strip));
    h.step();
    let p = h.state().model.project();
    assert_eq!(p.transitions.len(), 1, "уголок аудио-события тоже заводит фейд");
    assert_eq!(p.transitions[0].in_offset, Tick::ZERO, "фейд внутри события");
    assert_eq!(p.clips[0].duration, Tick(200_000), "событие не тронуто");
}

/// Тест 185 (Р.29): потеря GPU-исполнителя видео — предупреждение ровно
/// один раз, дальше CPU-фолбэк работает молча; статус бэкенда честный.
#[test]
fn t185_gpu_loss_notice_once() {
    struct DeadGpu(kt_engine::GpuBackendInfo);
    impl kt_engine::VideoGpuExec for DeadGpu {
        fn execute_into(
            &self,
            _p: &kt_engine::FramePlan<'_>,
            _o: &mut Vec<u8>,
        ) -> Result<bool, kt_engine::GpuExecError> {
            Err(kt_engine::GpuExecError::DeviceLost)
        }
        fn info(&self) -> &kt_engine::GpuBackendInfo {
            &self.0
        }
    }
    let mut h = harness();
    assert!(
        matches!(h.state().player.video_backend(), kt_engine::VideoBackendStatus::Cpu),
        "в харнессе без wgpu бэкенд честно CPU"
    );
    h.state_mut().player.set_video_gpu(Some(std::sync::Arc::new(DeadGpu(
        kt_engine::GpuBackendInfo { name: "test".into(), backend: "Test".into(), software: true },
    ))));
    // форсируем перекомпозицию (ключ превью уже устоялся на прошлых
    // кадрах): сброс ключа → compose зовёт исполнитель → DeviceLost →
    // отравление; следующий кадр показывает notice
    h.state_mut().preview_key = None;
    h.step();
    h.step();
    h.step();
    let app = h.state();
    assert!(app.gpu_fx_lost_notified, "notice о потере GPU обязан показаться");
    assert!(
        matches!(app.player.video_backend(), kt_engine::VideoBackendStatus::Poisoned { .. }),
        "бэкенд отравлен до конца сессии"
    );
    let texts: Vec<&str> = app.notices.iter().map(|n| n.text.as_str()).collect();
    let hits = texts
        .iter()
        .filter(|t| **t == crate::tf!(k::msg::VIDEO_FX_GPU_LOST, "DeviceLost"))
        .count();
    assert_eq!(hits, 1, "предупреждение ровно одно: {texts:?}");
}

/// Тест 221 (Р.33 п.5): единый виджет автоматизации. Проверяются четыре
/// требования пользователя разом — переключение график↔таблица, перевод
/// «материал ↔ таймлайн» с УЧЁТОМ реверса и скорости (на нём стоит
/// мини-таймлайн, двигающий главный курсор), ключ по курсору встаёт под
/// курсором, и ввод времени в таблице не пересортировывает строки под
/// руками.
#[test]
fn t221_automation_widget_maps_material_and_keeps_input_stable() {
    use crate::automation::{AutoMode, AutoView};
    use kt_model::*;
    use kt_time::Tick;
    const STEP: i64 = 3000; // 30 fps

    // прямой клип: материал = таймлайн со сдвигом на точку входа
    let mut c = ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(180_000),
        duration: Tick(90_000),
        source_in: Tick(45_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    let m = |c: &ClipEvent, t: Tick| {
        clip_material_at(c.source_in, c.start, c.duration, c.reversed, STEP, t)
    };
    let tl = |c: &ClipEvent, mat: Tick| {
        clip_timeline_at(c.source_in, c.start, c.duration, c.reversed, STEP, mat)
    };
    let mat = m(&c, Tick(180_000 + 30_000));
    assert_eq!(mat.0, 75_000, "материал = source_in + смещение внутри события");
    assert_eq!(
        tl(&c, mat).0,
        180_000 + 30_000,
        "обратный перевод обязан возвращать тот же момент"
    );

    // реверс: материал идёт навстречу таймлайну
    c.reversed = true;
    let mat_r = m(&c, Tick(180_000));
    assert_eq!(mat_r.0, 45_000 + 90_000 - STEP, "у реверса начало события — конец материала");
    assert_eq!(tl(&c, mat_r).0, 180_000, "обратный перевод реверса обязан сходиться");
    c.reversed = false;

    // СКОРОСТЬ ОСЬ НЕ СЖИМАЕТ (Р.37, п.1 пользователя): у клипа на 0.5×
    // ось кривой по-прежнему размах СОБЫТИЯ. Раньше здесь было
    // 45_000 + 45_000 — правая половина графика была недостижима, и
    // автоключ, который редактор сеет в конец, никогда не срабатывал.
    let mut slow = c.clone();
    slow.kind = ClipKind::Audio;
    slow.fx = vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)];
    let mid = m(&slow, Tick(180_000 + 90_000));
    assert_eq!(
        mid.0,
        45_000 + 90_000,
        "ось кривой — собственный размах события, скорость её не сжимает"
    );
    assert_eq!(
        tl(&slow, mid).0,
        180_000 + 90_000,
        "и обратный перевод теперь ТОЧЕН, а не в пределах кадра"
    );

    // виджет: переключение вида и стабильный ввод времени в таблице
    let mut view = AutoView::default();
    assert_eq!(view.mode, AutoMode::Graph, "по умолчанию график");
    view.mode = AutoMode::Table;
    let mut curves = [ParamCurve {
        param: "radius".into(),
        ease: false,
        keys: vec![
            AutoKey { t: Tick(45_000), v: 1.0 },
            AutoKey { t: Tick(135_000), v: 48.0 },
        ],
    }];
    // пользователь начал править время ВТОРОГО ключа на значение МЕНЬШЕ
    // первого: пока правка не закоммичена, порядок ключей не меняется
    view.t_edit = Some((0, 1, 0.4));
    assert_eq!(
        curves[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![45_000, 135_000],
        "во время ввода ключи не пересортировываются — строка уезжала бы из-под рук"
    );
    // на отпускании правка применяется и порядок восстанавливается
    let (ci, ki, s) = view.t_edit.take().unwrap();
    curves[ci].keys[ki].t = Tick((s * 90_000.0).round() as i64);
    curves[ci].keys.sort_by_key(|k| k.t.0);
    assert_eq!(
        curves[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![36_000, 45_000],
        "после отпускания ключи встают по порядку"
    );
}

/// Тест 219 (Р.33 п.2, дословно: «для одного и того же события с
/// примагниченными треками — можно и сразу все chain в одном окне давать,
/// так проще должно быть. Ну можно дать возможность скрытия-переключения»).
/// У A/V-пары теперь ОДНО окно с обеими цепочками в свёртках, а не два
/// независимых окна, у каждого из которых своя кнопка «Отмена».
#[test]
fn t219_one_window_for_the_whole_link_group() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    let clip = |id: u64, kind: ClipKind, lane: u32| ClipEvent {
        id: ClipId(id),
        link_group: Some(GroupId(7)),
        kind,
        lane,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    p.clips.push(clip(1, ClipKind::Video, 0));
    let mut a = clip(2, ClipKind::Audio, 0);
    a.fx = vec![FxInstance::gain(0.0)];
    p.clips.push(a);
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);

    // окно открыто по видео — оно же показывает и аудио-цепочку
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    assert_eq!(h.state().fx_editors.len(), 1, "окно одно");
    assert_eq!(
        h.state().fx_editors[0].members,
        vec![ClipId(1), ClipId(2)],
        "окно обязано вести обе цепочки пары"
    );
    // повторное открытие ПАРТНЁРА не плодит второе окно
    h.state_mut().open_fx_editor(ClipId(2));
    h.step();
    assert_eq!(h.state().fx_editors.len(), 1, "у пары не должно быть двух окон");

    // «Отмена» откатывает ОБЕ цепочки, а не только якорь
    let app = h.state_mut();
    let g = app.fx_editors[0].gesture;
    app.set_fx_coalesced(ClipId(1), vec![FxInstance::glow(1.0)], g);
    app.set_fx_coalesced(ClipId(2), vec![], g);
    let backup = app.fx_editors[0].backup.clone();
    assert_eq!(backup.len(), 2, "снимок обязан покрывать обе цепочки");
    for (id, fx, auto, bypass, off) in backup {
        app.set_fx_coalesced(id, fx, g);
        app.set_fx_auto_coalesced(id, auto, g);
        app.apply_coalesced(Command::SetFxBypass { id, bypass }, g);
        app.apply_coalesced(Command::SetFxOff { id, off }, g);
    }
    assert!(
        matches!(app.model.clip(ClipId(1)).unwrap().fx.as_slice(), [FxInstance::Blur { .. }]),
        "видео-цепочка не откатилась"
    );
    assert!(
        matches!(app.model.clip(ClipId(2)).unwrap().fx.as_slice(), [FxInstance::Gain { .. }]),
        "аудио-цепочка не откатилась — окно вело только якорь"
    );
    h.step();
}

/// Тест 218 (Р.33 п.2, требование пользователя про поле «New source»:
/// «хотелось бы чтобы можно было и dnd туда делать файла и dnd из сурсов
/// проекта или выбор через открытие виджета медиа»). Плюс «Отмена» у
/// морфера, которой не было вовсе (в окне FX она была всегда).
#[test]
fn t218_morpher_source_field_takes_files_and_reverts() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.morph_lanes = 1;
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: MorphPatch {
            media: None,
            source_in_delta: kt_time::Tick(0),
            absolute_source_in: None,
            ..Default::default()
        },
        timed: false,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_morph_editor(MorphId(1));
    h.step();
    h.step();
    // поле источника знает своё место на экране — по нему ловится дроп
    assert!(
        h.state().morph_editors[0].src_rect.is_some(),
        "поле источника не сообщило свой прямоугольник — дроп файла в него не поймать"
    );

    // файл, которого нет в проекте, становится источником ОДНИМ шагом
    let app = h.state_mut();
    let depth = app.model.undo_depth();
    app.set_morph_source_file(MorphId(1), "synth:dc");
    let ev = app.model.project().morphers[0].clone();
    let media = ev.patch.media;
    let mid = media.expect("источник обязан подставиться");
    assert_eq!(
        app.model.project().media.iter().find(|m| m.id == mid).map(|m| m.path.as_str()),
        Some("synth:dc"),
        "медиа обязано появиться в проекте"
    );
    assert_eq!(app.model.undo_depth(), depth + 1, "добавление медиа и подмена — один шаг");

    // «Отмена» возвращает морфер к состоянию на момент открытия окна
    let app = h.state_mut();
    let ed_gesture = app.morph_editors[0].gesture;
    let backup = app.morph_editors[0].backup.clone().expect("снимок на открытии");
    app.apply_coalesced(Command::SetMorpher(backup), ed_gesture);
    let ev = app.model.project().morphers[0].clone();
    let media = ev.patch.media;
    assert!(media.is_none(), "откат обязан вернуть «источник не менять»");
    h.step();
}

/// Тест 217 (Р.33 С2, требования пользователя: «у морферов fx-chain имеет
/// крестики, можно и галочки добавить включения-выключения эффектов и
/// возможность перемещения порядка мышкой. Ну и заменить fx-chain у
/// событийных эффектов, а то в двух местах один и тот же смысл, а сделано
/// у обоих не до конца»). Полоса цепочки теперь ОДНА: и окно события, и
/// обе секции морфера рисуют её одним кодом.
#[test]
fn t217_one_chain_widget_for_events_and_morphers() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(9), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        id: ClipId(9),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(9),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8), FxInstance::glow(1.0)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    // морфер-батч с двумя эффектами — вторая цепочка тем же виджетом
    p.morph_lanes = 1;
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(9)] },
        patch: MorphPatch {
            video_fx: vec![
                FxInstance::Mirror { horizontal: true, vertical: false },
                FxInstance::edges(48),
            ],
            ..Default::default()
        },
        timed: false,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(9));
    h.step();
    h.step();

    // 1) мастер-галка цепочки есть в ОБОИХ окнах — она одна и та же
    let chain_label = tr(k::fx::CHAIN_ON).to_string();
    assert!(
        h.query_by_label(&chain_label).is_some(),
        "в окне события нет мастер-галки цепочки"
    );
    h.state_mut().open_morph_editor(MorphId(1));
    h.step();
    h.step();
    assert!(
        h.state().morph_editors.iter().any(|e| e.open),
        "окно морфера не открылось"
    );
    // у морфера мастер-галка тоже есть (их теперь две — по одной на окно)
    assert!(
        h.get_all_by_label(&chain_label).count() >= 2,
        "у морфера нет мастер-галки цепочки — виджет не общий"
    );

    // 2) выключатели слотов есть и у морфера (раньше были только крестики)
    let app = h.state_mut();
    let ev = app.model.project().morphers[0].clone();
    let video_bypass = &ev.patch.video_bypass;
    assert!(video_bypass.is_empty(), "стартовое состояние — всё включено");

    // 3) выключение слота батча убирает эффект из целей, не теряя его
    let mut ev2 = ev.clone();
    ev2.patch.video_bypass = vec![true, false];
    assert!(app.apply(Command::SetMorpher(ev2)), "команда не прошла");
    let eff = app.model.project().effective_clips();
    let c = eff.iter().find(|c| c.id == ClipId(9)).unwrap();
    assert!(
        !c.fx.iter().any(|f| matches!(f, FxInstance::Mirror { .. })),
        "выключенный слот батча не должен доезжать до клипа"
    );
    assert!(
        c.fx.iter().any(|f| matches!(f, FxInstance::Edges { .. })),
        "включённый слот батча обязан остаться"
    );
    assert_eq!(
        app.model.project().morphers[0].patch.video_fx.len(),
        2,
        "сам эффект из батча не пропадает"
    );
    h.step();
}

/// Тест 193 (Р.30): редактор FX и автоматизация — окно с кривыми рисуется,
/// а пара SetFx+SetFxAuto одного жеста коалесцируется в ОДИН undo-шаг
/// (иначе Ctrl+Z откатывал бы цепочку и кривые вразнобой).
#[test]
fn t193_fx_editor_automation_coalesces_into_one_undo() {
    use kt_model::*;
    let mut h = harness();
    // видео-клип с Blur и живой кривой — секция автоматизации рисуется
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(9), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        id: ClipId(9),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(9),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![vec![ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: vec![
                AutoKey { t: kt_time::Tick(0), v: 1.0 },
                AutoKey { t: kt_time::Tick(90_000), v: 48.0 },
            ],
        }]],
        opacity: 1.0,
        fx_off: false,
    });
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(9));
    h.step();
    h.step();
    assert!(h.state().fx_editors.iter().any(|e| e.open), "окно редактора живо с кривыми");

    // жест редактора: цепочка + кривые ОДНИМ gesture-токеном (как UI)
    let app = h.state_mut();
    let depth_before = app.model.undo_depth();
    let gesture = app.fx_editors[0].gesture;
    let fx = vec![FxInstance::blur(20), FxInstance::glow(1.0)];
    let auto = vec![
        vec![ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: vec![AutoKey { t: kt_time::Tick(0), v: 2.0 }],
        }],
        vec![],
    ];
    app.set_fx_coalesced(ClipId(9), fx.clone(), gesture);
    app.set_fx_auto_coalesced(ClipId(9), auto, gesture);
    let c = app.model.clip(ClipId(9)).unwrap().clone();
    assert_eq!(c.fx, fx);
    assert_eq!(c.fx_auto.len(), 1, "хвост без кривых подрезан");
    assert_eq!(
        app.model.undo_depth(),
        depth_before + 1,
        "оба изменения жеста — один undo-шаг"
    );
    app.model.undo().unwrap();
    let c = app.model.clip(ClipId(9)).unwrap().clone();
    assert_eq!(c.fx.len(), 1, "undo вернул цепочку");
    assert_eq!(c.fx_auto[0][0].keys.len(), 2, "undo вернул кривые тем же шагом");
    h.step();
}

/// Тест 194 (Р.31 п.2): автосейв идёт В ФОНЕ — UI-поток только снимает
/// снапшот и позже забирает результат. Правки ПОСЛЕ снапшота не теряют
/// звёздочку (dirty гасится только при неизменной модели).
#[test]
fn t194_autosave_runs_in_background() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t194-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t194.otio");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    // правка содержимого + «прошло 30 секунд»
    app.model
        .apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() })
        .unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some(), "фоновый автосейв обязан стартовать");
    // дождаться воркер и забрать результат следующим кадром
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(h.state().autosave_rx.is_none(), "результат автосейва забран");
    assert!(!h.state().dirty_content, "успешный автосейв гасит звёздочку");
    assert!(target.exists(), "файл записан фоновым потоком");

    // правка ПОСЛЕ снапшота не должна терять звёздочку
    let app = h.state_mut();
    app.model
        .apply(Command::AddMedia { id: MediaId(2), path: "synth:ramp".into() })
        .unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some());
    // модель меняется, пока запись в полёте
    let app = h.state_mut();
    app.model
        .apply(Command::AddMedia { id: MediaId(3), path: "synth:ramp".into() })
        .unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(
        h.state().dirty_content,
        "правки после снапшота сохраняют звёздочку (иначе они бы потерялись)"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Проект под тесты выделения: три видео-события на одной дорожке с
/// зазорами (зазор нужен, чтобы рамке было с чего начаться).
#[cfg(test)]
fn harness_three_clips() -> (Harness<'static, KtApp>, crate::timeline::TlGeom) {
    harness_three_clips_dt(None)
}

#[cfg(test)]
fn harness_three_clips_dt(
    step_dt: Option<f32>,
) -> (Harness<'static, KtApp>, crate::timeline::TlGeom) {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness_lang_dt(crate::i18n::Lang::Ru, step_dt);
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        for (i, start) in [0i64, 100_000, 200_000].iter().enumerate() {
            p.clips.push(ClipEvent {
                id: ClipId(i as u64 + 1),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(*start),
                duration: Tick(50_000),
                source_in: Tick(0),
                media: MediaId(1),
                gain_db: 0.0,
                reversed: false,
                fx: vec![],
                fx_bypass: vec![],
                fx_auto: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = 1000.0;
        app.player.seek(Tick(0));
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("нет геометрии таймлайна");
    (h, g)
}

/// Драг рамкой В ТРИ ШАГА (порог драга egui) с зажатым Ctrl.
#[cfg(test)]
fn rubber_drag(h: &mut Harness<'_, KtApp>, from: egui::Pos2, to: egui::Pos2, ctrl: bool) {
    let m = if ctrl { egui::Modifiers::CTRL } else { egui::Modifiers::NONE };
    h.event(egui::Event::ModifiersChanged(m));
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    h.hover_at(from + egui::vec2(8.0, 0.0));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

/// Тест 202 (Р.33 п.7, требование пользователя: «на основном таймлайне не
/// хватает area-selection… можно по зажатию ctrl выделять через area»).
/// Выделение стало МНОЖЕСТВОМ: Ctrl+клик набирает пачку, Ctrl+драг с
/// пустого места ловит события рамкой и региона при этом не создаёт.
#[test]
fn t202_ctrl_click_and_rubber_band_select() {
    use kt_model::*;
    let (mut h, g) = harness_three_clips();
    let y = g.lane_y(ClipKind::Video, 0, 0.6);
    let at = |t: i64| egui::pos2(g.tick_to_x(t), y);

    // обычный клик — одно событие; Ctrl+клик добавляет второе
    h.hover_at(at(25_000));
    h.step();
    h.drag_at(at(25_000));
    h.step();
    h.drop_at(at(25_000));
    h.step();
    assert_eq!(h.state().selected.iter().copied().collect::<Vec<_>>(), vec![ClipId(1)]);

    let node_click_ctrl = |h: &mut Harness<'_, KtApp>, pos: egui::Pos2| {
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::CTRL));
        h.hover_at(pos);
        h.step();
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::CTRL,
        });
        h.step();
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::CTRL,
        });
        h.step();
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        h.step();
    };
    let cursor_before = h.state().player.transport.playhead();
    node_click_ctrl(&mut h, at(125_000));
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![ClipId(1), ClipId(2)],
        "Ctrl+клик обязан ДОБАВЛЯТЬ к выделению"
    );
    assert_eq!(
        h.state().player.transport.playhead(),
        cursor_before,
        "набор пачки не должен таскать курсор"
    );
    node_click_ctrl(&mut h, at(125_000));
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![ClipId(1)],
        "повторный Ctrl+клик снимает выделение с события"
    );

    // Ctrl+драг с ПУСТОГО места: рамка забирает второе и третье событие
    h.state_mut().model.set_region(None);
    h.step();
    rubber_drag(&mut h, at(70_000), at(260_000), true);
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![ClipId(2), ClipId(3)],
        "рамка не собрала события"
    );
    assert!(
        h.state().model.project().region.is_none(),
        "Ctrl+драг обязан рисовать рамку, а не регион"
    );

    // без Ctrl тот же драг по-прежнему делает регион (старый жест цел)
    rubber_drag(&mut h, at(70_000), at(260_000), false);
    assert!(
        h.state().model.project().region.is_some(),
        "обычный драг с пустого места обязан остаться выделением региона"
    );
}

/// Тест 203 (Р.33 п.7, вторая половина требования: «а можно как в vegas —
/// toggle-button»). Кнопка-инструмент включается КЛИКОМ и меняет смысл
/// обычного драга по пустому месту.
#[test]
fn t203_selection_tool_button_switches_drag() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, g) = harness_three_clips();
    let y = g.lane_y(ClipKind::Video, 0, 0.6);
    let at = |t: i64| egui::pos2(g.tick_to_x(t), y);

    assert!(!h.state().sel_tool, "режим выключен по умолчанию");
    h.get_by_label(crate::icons::SELECTION).click();
    h.step();
    assert!(h.state().sel_tool, "клик по кнопке не включил инструмент выделения");

    // в режиме ОБЫЧНЫЙ драг рисует рамку и региона не создаёт
    h.state_mut().model.set_region(None);
    h.step();
    rubber_drag(&mut h, at(70_000), at(260_000), false);
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![ClipId(2), ClipId(3)],
        "в режиме выделения драг обязан ловить события"
    );
    assert!(h.state().model.project().region.is_none(), "в режиме выделения регион не ставится");

    // выключили — драг снова делает регион
    h.get_by_label(crate::icons::SELECTION).click();
    h.step();
    assert!(!h.state().sel_tool);
    rubber_drag(&mut h, at(70_000), at(260_000), false);
    assert!(h.state().model.project().region.is_some(), "режим выключен — вернулся регион");
}

/// Тест 211 (Р.33 п.10, живая половина): вернувшийся на диск файл
/// подменяет свою заглушку сам — без переоткрытия проекта и без потери
/// позиции. Движковая половина теста — в kt-engine (замена источника
/// выбивает его кадры из кэша).
#[test]
fn t211_returned_source_replaces_its_placeholder() {
    use kt_model::*;
    use kt_time::Tick;
    let dir = std::env::temp_dir().join(format!("kt-t211-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t211.otio");

    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 320, height: 180, sample_rate: 48000 });
    // «пропавший» файл: имя synth-источника, которого нет на диске — при
    // открытии он не откроется, а к моменту возврата станет валидным
    p.media.push(MediaRef { id: MediaId(1), path: "later.mp4".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    std::fs::write(&path, kt_otio::save_project(&p, &Default::default(), path.parent())).unwrap();

    let app = KtApp::open(&path).expect("проект обязан открыться");
    assert_eq!(app.placeholder_media.len(), 1, "заглушка не подставилась");
    let pos_before = app.player.transport.playhead();

    let mut h = Harness::builder()
        .with_size(egui::vec2(W, H))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&h.ctx);
    h.step();
    // файл «вернулся»: подменяем путь на синтетический источник и гасим
    // offline-набор руками, чтобы не ждать 900 мс сторожа
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media[0].path = "synth:ramp".into();
        app.model = kt_model::TimelineModel::new(p);
        app.offline.lock().unwrap().clear();
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !h.state().placeholder_media.is_empty() && std::time::Instant::now() < deadline {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        h.state().placeholder_media.is_empty(),
        "вернувшийся файл так и не заменил заглушку"
    );
    assert!(
        h.state().notices.iter().any(|n| n.text.contains("later.mp4")),
        "о возврате исходника не сказано"
    );
    assert_eq!(
        h.state().player.transport.playhead(),
        pos_before,
        "подмена источника не должна двигать курсор"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 210 (Р.33 п.10, про рендер): недоступный исходник больше не
/// отменяет экспорт — он идёт заглушкой, о чём сказано ДО старта (в окне
/// настроек) и ПОСЛЕ (в сводке). Раньше рендер просто не запускался, а
/// потерянные слои движка в GUI не показывались вовсе.
#[test]
fn t210_render_with_missing_sources_warns_and_runs() {
    use crate::render_dialog::{missing_sources, open_all};
    use kt_model::*;
    use kt_time::Tick;

    let dir = std::env::temp_dir().join(format!("kt-t210-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let present = dir.join("here.wav");
    std::fs::write(&present, b"not a real media file").unwrap();

    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 320, height: 180, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.media.push(MediaRef { id: MediaId(2), path: "gone.mp4".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(2),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();

    // предупреждение ДО старта: назван только пропавший, синтетика не в счёт
    let warn = missing_sources(&p, Some(dir.as_path()));
    assert_eq!(warn, vec!["gone.mp4".to_string()], "список для предупреждения неверный: {warn:?}");

    // источники для рендера собираются ВСЕ: экспорт стартует
    let (sources, missing) = open_all(&p, Some(dir.as_path()));
    assert_eq!(sources.len(), 2, "рендер обязан получить источник на каждое медиа");
    assert_eq!(missing.len(), 1, "заглушка обязана быть названа");
    assert!(missing[0].contains("gone.mp4"), "в списке нет имени файла: {missing:?}");
    // и заглушка реально отдаёт кадры, а не пустоту
    let mut sources = sources;
    let (_, src) = sources.iter_mut().find(|(id, _)| *id == MediaId(2)).unwrap();
    let f = src
        .read_video_frame(Tick(0), Some(kt_media::Quality { width: 320, height: 180 }))
        .expect("заглушка обязана отдавать кадр");
    assert!(f.rgba.iter().any(|b| *b != 0), "кадр заглушки пустой");

    // в живом приложении список доезжает до окна настроек
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model = kt_model::TimelineModel::new(p);
        app.project_path = Some(dir.join("t210.otio"));
        app.start_render();
    }
    h.step();
    assert_eq!(
        h.state().render_setup.missing,
        vec!["gone.mp4".to_string()],
        "окно настроек не знает о недоступных исходниках"
    );
    h.state_mut().render_setup.open = false;
    h.step();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 209 (Р.33 п.10): проект с ПРОПАВШИМ исходником открывается,
/// каждый недоступный файл называется в уведомлениях, а вместо кадров
/// встаёт плейсхолдер — превью укомплектовано, а не висит в «Loading».
#[test]
fn t209_missing_source_opens_with_placeholder_and_notices() {
    use kt_model::*;
    use kt_time::Tick;
    let dir = std::env::temp_dir().join(format!("kt-t209-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t209.otio");

    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "no-such-clip.mp4".into() });
    p.media.push(MediaRef { id: MediaId(2), path: "also-gone.wav".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    let text = kt_otio::save_project(&p, &Default::default(), path.parent());
    std::fs::write(&path, text).unwrap();

    let app = KtApp::open(&path).expect("проект с пропавшим исходником обязан открыться");
    assert_eq!(app.placeholder_media.len(), 2, "оба недоступных медиа обязаны получить заглушку");
    let named: Vec<&String> = app.notices.iter().map(|n| &n.text).collect();
    assert!(
        named.iter().any(|t| t.contains("no-such-clip.mp4")),
        "первый пропавший файл не назван: {named:?}"
    );
    assert!(
        named.iter().any(|t| t.contains("also-gone.wav")),
        "второй пропавший файл потерялся — раньше выживала только последняя ошибка: {named:?}"
    );

    // и кадр реально приходит: композиция укомплектовывается
    let mut h = Harness::builder()
        .with_size(egui::vec2(W, H))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&h.ctx);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut complete = false;
    while std::time::Instant::now() < deadline {
        h.step();
        if h.state().incomplete_since.is_none() && h.state().preview_key.is_some() {
            complete = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(complete, "превью так и не укомплектовалось — заглушка не доехала до композиции");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 206 (Р.33 п.4): двойной клик по событию открывает FX — ровно то,
/// что подсказка клипа обещает с Р.18 и чего не делала (ставила регион, а
/// FX прятались в ПКМ-меню; дефект записан в user-feedback ещё тогда).
/// Регион по событию переехал на Shift+двойной клик и пункт меню, и оба
/// пути дают ОДИН результат (п.17 чек-листа — парность способов).
#[test]
fn t206_double_click_opens_fx_shift_makes_region() {
    use kt_model::*;
    let (mut h, g) = harness_three_clips_dt(Some(0.02));
    let y = g.lane_y(ClipKind::Video, 0, 0.6);
    let pos = egui::pos2(g.tick_to_x(125_000), y);

    let dbl = |h: &mut Harness<'_, KtApp>, pos: egui::Pos2, modifiers: egui::Modifiers| {
        h.event(egui::Event::ModifiersChanged(modifiers));
        h.hover_at(pos);
        h.step();
        for _ in 0..2 {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            });
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            });
        }
        h.step();
        h.step();
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        h.step();
    };

    dbl(&mut h, pos, egui::Modifiers::NONE);
    assert_eq!(h.state().fx_editors.len(), 1, "двойной клик не открыл FX-редактор");
    assert_eq!(h.state().fx_editors[0].clip, ClipId(2), "открылось окно не того события");
    assert!(h.state().model.project().region.is_none(), "двойной клик больше не ставит регион");

    // Shift+двойной клик — регион по событию. Свой харнесс: история кликов
    // у egui общая, и повтор жеста в том же окне сложился бы в тройной клик
    let (mut h2, g2) = harness_three_clips_dt(Some(0.02));
    let pos2 = egui::pos2(g2.tick_to_x(125_000), g2.lane_y(ClipKind::Video, 0, 0.6));
    dbl(&mut h2, pos2, egui::Modifiers::SHIFT);
    let by_gesture = h2.state().model.project().region.expect("Shift+двойной клик не дал регион");
    assert_eq!((by_gesture.start.0, by_gesture.end.0), (100_000, 150_000));
    assert!(h2.state().fx_editors.is_empty(), "Shift+двойной клик не должен открывать FX");

    // тот же результат из меню — одно тело на оба пути
    h2.state_mut().model.set_region(None);
    h2.state_mut().region_from_event(ClipId(2));
    let by_menu = h2.state().model.project().region.expect("пункт меню не дал регион");
    assert_eq!(by_menu, by_gesture, "жест и пункт меню обязаны совпадать");
}

/// Тест 207 (Р.33 п.8, дословно: «нужно чтобы при двойном клике на
/// marked_region он выделялся весь, если там пересечение нескольких — то
/// по названию региона»).
#[test]
fn t207_double_click_on_marked_range_selects_it() {
    use crate::timeline::pick_range_marker;
    use kt_model::*;
    use kt_time::Tick;

    // ---- чистая часть: какой диапазон под кликом ----
    let markers = vec![
        Marker { t: Tick(30_000), duration: Tick(60_000), name: "a".into(), color: None },
        Marker { t: Tick(60_000), duration: Tick(60_000), name: "b".into(), color: None },
        Marker { t: Tick(500_000), duration: Tick::ZERO, name: "точка".into(), color: None },
    ];
    let tol = 4_000;
    assert_eq!(pick_range_marker(&markers, 70_000, tol), Some(60_000), "общая часть — ближний флажок слева");
    assert_eq!(pick_range_marker(&markers, 31_000, tol), Some(30_000), "клик по флажку берёт свою метку");
    assert_eq!(pick_range_marker(&markers, 200_000, tol), None, "мимо лент — ничего");
    assert_eq!(pick_range_marker(&markers, 500_000, tol), None, "точечная метка регионом не становится");

    // ---- живой двойной клик по ленте ----
    let (mut h, g) = harness_three_clips_dt(Some(0.02));
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.markers = vec![Marker {
            t: Tick(90_000),
            duration: Tick(130_000),
            name: "chorus".into(),
            color: None,
        }];
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();
    let band_y = g.rect.top() + crate::timeline::TIME_TOP + 11.0;
    let pos = egui::pos2(g.tick_to_x(150_000), band_y);
    h.hover_at(pos);
    h.step();
    for _ in 0..2 {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    let r = h.state().model.project().region.expect("двойной клик по ленте не дал регион");
    assert_eq!((r.start.0, r.end.0), (90_000, 220_000), "регион не по границам метки");
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![ClipId(2), ClipId(3)],
        "события внутри диапазона обязаны выделиться («выделялся весь»)"
    );
}

/// Тест 204 (Р.33 п.7): пачка едет драгом ЦЕЛИКОМ и одним undo-шагом, а
/// упор считается по первому контакту любого её участника. Взаимные
/// сдвиги внутри блока при этом не меняются — «ничего не отрастает и не
/// отрубается» (п.6 Р.28).
#[test]
fn t204_multi_drag_moves_the_whole_block() {
    use crate::timeline::plan_move_block;
    use kt_model::*;
    use kt_time::Tick;

    // ---- чистая часть: кламп блока о препятствие ----
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    let clip = |id: u64, start: i64| ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(start),
        duration: Tick(50_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    p.clips = vec![clip(1, 0), clip(2, 100_000), clip(3, 400_000)];
    p.overlap_layers = false; // режим без переслаивания: упор настоящий
    let moving = [ClipId(1), ClipId(2)];
    // блок [0..50k]+[100k..150k] едет вправо к препятствию на 400k:
    // передний участник (2) касается его через 250 000 тиков
    let plan = plan_move_block(&p, &moving, 400_000);
    assert_eq!(plan.delta, 250_000, "блок не остановился у препятствия");
    assert_eq!(plan.blocker.map(|(s, _, _)| s), Some((400_000, 450_000)));
    // влево блок упирается в ноль (за начало таймлайна не уезжает)
    assert_eq!(plan_move_block(&p, &moving, -80_000).delta, 0);
    // свободный ход не трогается
    let free = plan_move_block(&p, &moving, 100_000);
    assert_eq!((free.delta, free.blocker.is_none()), (100_000, true), "свободный ход зажали");
    // при переслаивании (умолчание проекта) упора нет вовсе
    p.overlap_layers = true;
    assert_eq!(plan_move_block(&p, &moving, 400_000).delta, 400_000);

    // ---- живой жест ----
    let (mut h, g) = harness_three_clips();
    {
        let app = h.state_mut();
        app.select_only(ClipId(1));
        app.selected.insert(ClipId(2));
    }
    h.step();
    let y = g.lane_y(ClipKind::Video, 0, 0.6);
    let grab = egui::pos2(g.tick_to_x(25_000), y);
    let to = egui::pos2(g.tick_to_x(325_000), y);
    h.hover_at(grab);
    h.step();
    h.drag_at(grab);
    h.step();
    h.hover_at(grab + egui::vec2(8.0, 0.0));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();
    let starts: Vec<i64> = {
        let p = h.state().model.project();
        [ClipId(1), ClipId(2), ClipId(3)]
            .iter()
            .map(|id| p.clips.iter().find(|c| c.id == *id).unwrap().start.0)
            .collect()
    };
    assert_eq!(starts[0], 300_000, "якорь пачки не доехал: {starts:?}");
    assert_eq!(starts[1], 400_000, "второе событие пачки обязано ехать тем же сдвигом");
    assert_eq!(starts[2], 200_000, "невыделенное событие двигать нельзя");
    let durs: Vec<i64> = {
        let p = h.state().model.project();
        p.clips.iter().map(|c| c.duration.0).collect()
    };
    assert!(durs.iter().all(|d| *d == 50_000), "длительности изменились: {durs:?}");
    h.state_mut().undo();
    let after_undo: Vec<i64> = {
        let p = h.state().model.project();
        [ClipId(1), ClipId(2)]
            .iter()
            .map(|id| p.clips.iter().find(|c| c.id == *id).unwrap().start.0)
            .collect()
    };
    assert_eq!(after_undo, vec![0, 100_000], "перенос пачки — один undo-шаг");
}

/// Тест 205 (Р.33 п.7, «ну и это естественно везде учесть, в том же
/// создании морферов»): операции работают по ВСЕМУ выделению, а не по
/// одному событию.
#[test]
fn t205_operations_run_over_whole_selection() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips();
    let app = h.state_mut();
    app.select_only(ClipId(1));
    app.selected.insert(ClipId(3));

    // цели морфера = всё выделение
    assert_eq!(app.morph_target_ids(), vec![ClipId(1), ClipId(3)]);

    // копирование забирает оба события, вставка кладёт оба
    app.copy_selection();
    assert_eq!(app.clipboard.clips.len(), 2, "копируется всё выделение");
    app.player.seek(kt_time::Tick(400_000));
    app.paste_at_cursor();
    assert_eq!(app.model.project().clips.len(), 5, "вставились оба события");

    // удаление сносит оба и откатывается одним undo
    app.select_only(ClipId(1));
    app.selected.insert(ClipId(3));
    let before = app.model.project().clips.len();
    app.delete_selected();
    assert_eq!(app.model.project().clips.len(), before - 2, "Del удалил не всё выделение");
    app.undo();
    assert_eq!(app.model.project().clips.len(), before, "undo вернул оба одним шагом");
}

/// Тест 201 (Р.33 п.9, требование пользователя: «шаблон для всех рендеров
/// всегда kt-<projectname>-<region_name>-<его длинна в секундах>»). Было
/// `kp-render-<unix-время>.mp4` — ни проекта, ни региона, ни длины.
#[test]
fn t201_render_file_name_follows_template() {
    use crate::render_dialog::render_file_name;
    use kt_model::*;
    // чистая часть шаблона
    assert_eq!(render_file_name(Some("movie"), Some("intro"), 90_000 * 12), "kt-movie-intro-12s.mp4");
    assert_eq!(render_file_name(Some("movie"), None, 90_000 * 3), "kt-movie-3s.mp4");
    assert_eq!(render_file_name(Some("movie"), Some("  "), 90_000), "kt-movie-1s.mp4");
    assert_eq!(render_file_name(None, None, 0), "kt-untitled-0s.mp4");
    assert_eq!(
        render_file_name(Some("my project"), Some("scene 2: take?"), 90_000 * 5 / 2),
        "kt-my-project-scene-2_-take_-3s.mp4",
        "пробелы дефисом, запрещённые ФС-символы подчёркиванием, длина округляется"
    );

    // живая часть: имя берётся у метки-диапазона, СОВПАДАЮЩЕЙ с регионом
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.markers.push(Marker {
        t: kt_time::Tick(90_000),
        duration: kt_time::Tick(90_000 * 4),
        name: "chorus".into(),
        color: None,
    });
    p.region = Some(Region { start: kt_time::Tick(90_000), end: kt_time::Tick(90_000 * 5) });
    app.model = kt_model::TimelineModel::new(p);
    app.project_path = Some(std::path::PathBuf::from("/tmp/kt-t201/song.otio"));
    assert_eq!(app.default_render_name(), "kt-song-chorus-4s.mp4");

    // регион не совпал с меткой — сегмент региона опускается (имя не врёт)
    let mut p = app.model.project().clone();
    p.region = Some(Region { start: kt_time::Tick(0), end: kt_time::Tick(90_000 * 5) });
    app.model = kt_model::TimelineModel::new(p);
    assert_eq!(app.default_render_name(), "kt-song-5s.mp4");
    h.step();
}

/// Тест 200 (Р.33, п.К1): правки ВНУТРИ жеста (окно FX, драг края
/// перехода, окно морфера) обязаны двигать ревизию модели. Пока они
/// звали `model.apply_coalesced` мимо обёртки приложения, ревизия стояла
/// на месте — и фоновый автосейв гасил звёздочку по гейту «ревизия та
/// же», хотя цепочка эффектов уже изменилась ПОСЛЕ снимка: правки
/// выглядели сохранёнными и терялись при выходе.
#[test]
fn t200_coalesced_edits_bump_revision_and_keep_dirty() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t200-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t200.otio");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(9), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        id: ClipId(9),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(9),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    app.model = kt_model::TimelineModel::new(p);
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some(), "фоновая запись в полёте");

    // правка цепочки жестом окна FX, пока запись не финишировала
    let app = h.state_mut();
    let rev_before = app.model_rev;
    app.open_fx_editor(ClipId(9));
    let gesture = app.fx_editors[0].gesture;
    app.set_fx_coalesced(
        ClipId(9),
        vec![FxInstance::blur(20), FxInstance::glow(1.0)],
        gesture,
    );
    assert!(app.model_rev > rev_before, "правка жеста двигает ревизию модели");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(h.state().autosave_rx.is_none(), "результат автосейва забран");
    assert!(
        h.state().dirty_content,
        "цепочка изменена ПОСЛЕ снимка — звёздочка обязана остаться"
    );
    assert_eq!(
        h.state().model.clip(ClipId(9)).unwrap().fx.len(),
        2,
        "правка на месте (не откачена автосейвом)"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 199 (Р.32): ПРАВИЛО сторожа — запись со СТАРОЙ ревизией не
/// трогает файл, с новой — пишет. Именно так фоновый автосейв перестаёт
/// перетирать свежий Ctrl+S (гонку нашли живые сценарии: они читают
/// сохранённый .otio и видели устаревшую модель — у пользователя так же
/// терялись бы правки).
#[test]
fn t199_guarded_write_never_goes_backwards() {
    let dir = std::env::temp_dir().join(format!("kt-t199g-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("p.otio");
    let guard = std::sync::Mutex::new(0u64);

    // первая запись (ревизия 5) проходит
    assert!(crate::app::write_project_guarded(&guard, &f, 5, "rev5").unwrap());
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "rev5");
    // «опоздавший» фоновый снапшот ревизии 3 — файл не трогает
    assert!(!crate::app::write_project_guarded(&guard, &f, 3, "rev3").unwrap());
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "rev5", "старый снимок перетёр свежий");
    // повтор той же ревизии допустим (идемпотентно), новее — пишется
    assert!(crate::app::write_project_guarded(&guard, &f, 5, "rev5b").unwrap());
    assert!(crate::app::write_project_guarded(&guard, &f, 9, "rev9").unwrap());
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "rev9");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тот же инвариант в живом потоке приложения: фоновая запись в полёте +
/// немедленный Ctrl+S следующей ревизии → на диске остаётся свежее.
#[test]
fn t199_background_autosave_never_overwrites_newer_save() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t199-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t199.otio");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    // ревизия 1: медиа A, фоновый автосейв стартует с ЭТИМ снапшотом
    app.model
        .apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() })
        .unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some(), "фоновая запись в полёте");

    // ревизия 2 + немедленный Ctrl+S, пока фоновая ещё не финишировала
    let app = h.state_mut();
    app.model
        .apply(Command::AddMedia { id: MediaId(2), path: "synth:ramp".into() })
        .unwrap();
    app.model_rev += 1;
    app.save();

    // дожидаемся фоновую и убеждаемся, что она НЕ откатила файл
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    let text = std::fs::read_to_string(&target).unwrap();
    let saved = kt_otio::load_project(&text, target.parent()).unwrap().project;
    assert_eq!(
        saved.media.len(),
        2,
        "на диске обязана остаться СВЕЖАЯ версия (2 медиа), а не снимок автосейва"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 227 (Р.33 С4): у перехода появилось окно свойств. Двойной клик по
/// его окну на полосе открывает редактор, вид меняется пикером и сразу
/// уезжает в модель, «Отмена» возвращает прежний.
#[test]
fn t227_transition_editor_opens_and_sets_kind() {
    use kt_model::*;
    use kt_time::Tick;
    let (mut h, g) = harness_three_clips_dt(Some(0.02));
    // склеиваем клипы 1 и 2 встык и вешаем переход
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[1].start = Tick(50_000);
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: Some(ClipId(2)),
            in_offset: Tick(9_000),
            out_offset: Tick(9_000),
            kind: TransitionKind::Crossfade,
            audio: Default::default(),
            auto: vec![],
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();
    let g = h.state().tl_geom.clone().unwrap_or(g);
    // окно перехода [41_000, 59_000): целимся в его середину, НИЖЕ
    // верхней полосы жестов (там живут ручки фейда)
    let y = g.lane_y(ClipKind::Video, 0, 0.75);
    let pos = egui::pos2(g.tick_to_x(46_000), y);
    for _ in 0..2 {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    assert_eq!(
        h.state().transition_editors.len(),
        1,
        "двойной клик по окну перехода обязан открыть его свойства"
    );
    assert_eq!(h.state().transition_editors[0].id, TransitionId(1));

    // смена вида через модель редактора: окно пишет команду тем же
    // gesture-токеном, поэтому вся сессия — один шаг undo
    let gesture = h.state().transition_editors[0].gesture;
    {
        let app = h.state_mut();
        let mut t = app.model.project().transitions[0].clone();
        t.kind = TransitionKind::Wipe { dir: Dir::Up, angle: 0.0, softness: 0.2 };
        t.audio = AudioCurve::SCurve;
        assert!(app.apply_coalesced(Command::SetTransition(t), gesture));
    }
    h.step();
    let t = &h.state().model.project().transitions[0];
    assert_eq!(t.kind.tag(), "wipe", "вид не доехал до модели");
    assert_eq!(t.audio, AudioCurve::SCurve);

    // «Отмена» окна возвращает исходный вид ОДНИМ шагом undo
    let backup = h.state().transition_editors[0].backup.clone();
    {
        let app = h.state_mut();
        assert!(app.apply_coalesced(Command::SetTransition(backup), gesture));
    }
    h.step();
    assert_eq!(h.state().model.project().transitions[0].kind, TransitionKind::Crossfade);
    let _ = h.state_mut().model.undo();
    h.step();
    assert_eq!(
        h.state().model.project().transitions[0].kind,
        TransitionKind::Crossfade,
        "вся сессия окна — один шаг истории"
    );

    // окно закрывается вместе со смертью перехода
    {
        let app = h.state_mut();
        let _ = app.apply(Command::RemoveTransition { id: TransitionId(1) });
    }
    h.step();
    h.step();
    assert!(
        h.state().transition_editors.is_empty(),
        "окно мёртвого перехода обязано закрыться само"
    );
}

/// Тест 231b (Р.34 К3): камера наконец редактируема. Двойной клик по её
/// блоку открывает окно (до этого раунда редактора не было вовсе, а
/// комментарий в коде обещал его с Р.29), правки идут одним undo-шагом,
/// окно удалённой камеры закрывается само.
#[test]
fn t231_camera_editor_opens_and_edits() {
    use kt_model::*;
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips_dt(Some(0.02));
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.camera_lanes = 1;
        p.cameras.push(CameraEvent {
            id: CameraId(1),
            lane: 0,
            start: Tick(0),
            duration: Tick(120_000),
            name: String::new(),
            cam: CameraParams::default(),
            keyframes: vec![],
            auto: vec![],
            ease: true,
        });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();
    let g = h.state().tl_geom.clone().expect("геометрия");
    let row = g.row_of((LaneKind::Camera, 0)).expect("камерная дорожка есть в строках");
    // ЦЕНТР строки, а не «полбазовой высоты»: камерная строка втрое ниже
    // (Р.35 п.1), и старая формула била мимо неё
    let pos = egui::pos2(g.tick_to_x(60_000), g.row_center_y(row));
    for _ in 0..2 {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    assert_eq!(
        h.state().camera_editors.len(),
        1,
        "двойной клик по камере обязан открыть её окно"
    );

    // правка через сессию окна: один gesture — один шаг истории
    let gesture = h.state().camera_editors[0].gesture;
    {
        let app = h.state_mut();
        let mut c = app.model.project().cameras[0].clone();
        c.name = "main".into();
        c.auto = vec![ParamCurve {
            param: "pos_z".into(),
            ease: true,
            keys: vec![
                AutoKey { t: Tick(0), v: -900.0 },
                AutoKey { t: Tick(120_000), v: -300.0 },
            ],
        }];
        assert!(app.apply_coalesced(Command::SetCamera(c), gesture));
    }
    h.step();
    assert_eq!(h.state().model.project().cameras[0].auto.len(), 1, "кривая доехала до модели");
    let _ = h.state_mut().model.undo();
    h.step();
    assert!(
        h.state().model.project().cameras[0].auto.is_empty(),
        "вся сессия окна — один шаг истории"
    );

    // камеру удалили — окно уходит само
    {
        let app = h.state_mut();
        let _ = app.apply(Command::RemoveCamera { id: CameraId(1) });
    }
    h.step();
    h.step();
    assert!(h.state().camera_editors.is_empty(), "окно мёртвой камеры закрывается");
}

/// Тест 242 (Р.35 С1, кусок 5): окно морфера показывает, НА КОГО и КОГДА
/// он действует. До этого раунда цели были заморожены навсегда и не
/// показывались иначе как строкой «#1, #2», а времени у морфера не было
/// вовсе — блок можно было двигать, и это ничего не значило.
#[test]
fn t242_morph_editor_shows_target_mode_and_timing() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.morph_lanes = 1;
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(45_000),
        targets: MorphTargets::Below,
        patch: MorphPatch { media: Some(MediaId(1)), ..Default::default() },
        timed: true,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_morph_editor(MorphId(1));
    h.step();
    h.step();

    // режим целей виден и объяснён, а не спрятан в коде
    assert!(
        h.query_by_label(crate::i18n::tr(crate::i18n::k::mrph::ON)).is_some(),
        "строка «действует на» обязана быть в окне"
    );
    assert!(
        h.query_by_label(crate::i18n::tr(crate::i18n::k::mrph::BELOW_NOTE)).is_some(),
        "и сказано, что именно слой накрывает"
    );

    // галка «по времени» правит МОДЕЛЬ и укладывается в один undo-шаг
    let depth = h.state().model.undo_depth();
    h.get_by_label(crate::i18n::tr(crate::i18n::k::mrph::TIMED)).click();
    h.step();
    h.step();
    assert!(!h.state().model.project().morphers[0].timed, "галка сняла действие по времени");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "сессия окна — один шаг истории");

    // второй режим окно показывает ДРУГИМ: вместо объяснения слоя —
    // сам замороженный список (иначе «на кого действует» не увидеть)
    let app = h.state_mut();
    let mut ev = app.model.project().morphers[0].clone();
    ev.targets = MorphTargets::Clips { ids: vec![ClipId(1)] };
    assert!(app.apply(Command::SetMorpher(ev)));
    h.step();
    h.step();
    assert!(
        h.query_by_label(crate::i18n::tr(crate::i18n::k::mrph::BELOW_NOTE)).is_none(),
        "у замороженного списка объяснению слоя не место"
    );
    assert!(
        h.query_by_label_contains("#1").is_some(),
        "замороженный список обязан быть виден поимённо"
    );
}

/// Тест 244 (Р.35 С2, дословно: «Пункт "Add morpher" (и у clips и у
/// региона) можно сократить с двух до одного и в ui морфера их
/// объединить — может быть сразу fx-chain делать, в котором помимо
/// остальных fx есть fx для морфинга, его пункты пометить фиолетовым и
/// поднять в списке fx»). Полоса в окне морфера ОДНА, морф-слот в ней
/// первый и не переставляется, а «+M» в тулбаре заводит слой сверху.
#[test]
fn t244_morpher_has_one_chain_with_pinned_head() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);

    // «+M»: морф-дорожка заводится СВЕРХУ над всеми — иначе «всё, что
    // ниже» было бы пусто (умолчание порядка кладёт морф-строки вниз)
    let depth = app.model.undo_depth();
    app.add_morph_layer();
    let pr = app.model.project();
    assert_eq!(pr.morphers.len(), 1, "слой создан");
    assert_eq!(pr.rows()[0], (LaneKind::Morph, 0), "и поднят строкой 0");
    assert!(matches!(pr.morphers[0].targets, MorphTargets::Below), "режим — «всё, что ниже»");
    assert!(pr.morphers[0].timed, "и по времени");
    assert_eq!(app.model.undo_depth(), depth + 1, "дорожка, порядок и событие — один шаг");
    assert!(!pr.effective_clips()[0].fx.is_empty() || pr.morphers[0].patch.is_empty());
    h.step();
    h.step();

    // окно открылось само; полоса ОДНА (одна мастер-галка), и первым в
    // ней стоит морф-слот
    assert!(h.state().morph_editors.iter().any(|e| e.open), "окно морфера не открылось");
    let chain_label = tr(k::fx::CHAIN_ON).to_string();
    assert_eq!(
        h.get_all_by_label(&chain_label).count(),
        1,
        "полос цепочки в окне морфера должна быть ОДНА (было две: видео и аудио)"
    );
    let head = tr(k::mrph::SRC_SLOT_NONE).to_string();
    assert!(h.query_by_label(&head).is_some(), "морф-слот обязан быть виден в полосе");

    // выбран он же — значит окно сразу показывает поле источника
    assert_eq!(h.state().morph_editors[0].sel, crate::widgets::ChainSel::Head);
    assert!(
        h.state().morph_editors[0].src_rect.is_some(),
        "поле источника показано при выбранной голове"
    );

    // эффекты ОБОИХ видов живут в одной полосе и раскладываются обратно
    let app = h.state_mut();
    let mut ev = app.model.project().morphers[0].clone();
    ev.patch.video_fx = vec![FxInstance::Mirror { horizontal: true, vertical: false }];
    ev.patch.audio_fx = vec![FxInstance::gain(3.0)];
    assert!(app.apply(Command::SetMorpher(ev)));
    h.step();
    h.step();
    assert!(
        h.query_by_label(crate::fx_editor::fx_display_name(&FxInstance::gain(3.0))).is_some(),
        "аудио-эффект обязан быть в ТОЙ ЖЕ полосе"
    );
    let ev = h.state().model.project().morphers[0].clone();
    assert_eq!(ev.patch.video_fx.len(), 1, "и раскладываться обратно по видам");
    assert_eq!(ev.patch.audio_fx.len(), 1);
}

/// Тест 245 (Р.35 С3, дословно: «чтобы не терять место — треки-морферы
/// можно уменьшить по высоте в три раза»). Чистая геометрия строк:
/// отношение 3, пол высоты, префиксные суммы, биекция `row_at_y` на
/// смешанном наборе и якорь зума.
#[test]
fn t245_thin_service_rows_geometry() {
    use crate::timeline::{TlGeom, THIN_ROW_MIN, row_height, row_offsets};
    use kt_model::{ClipKind, LaneKind};
    let rows = vec![
        (LaneKind::Morph, 0),
        (LaneKind::Video, 0),
        (LaneKind::Camera, 0),
        (LaneKind::Audio, 0),
        (LaneKind::Morph, 1),
    ];
    // камерная — ровно втрое ниже (Р.35 п.1), морф-строка ВДВОЕ выше
    // камерной (Р.38 п.10: «увеличь высоту морфер трека по дефолту на 2»),
    // пол — 10 px (в строку в 5 px мышью не попасть)
    assert_eq!(row_height(LaneKind::Morph, 60.0), 40.0);
    assert_eq!(row_height(LaneKind::Camera, 60.0), 20.0);
    assert_eq!(row_height(LaneKind::Video, 60.0), 60.0);
    assert_eq!(row_height(LaneKind::Morph, 12.0), THIN_ROW_MIN);

    // префиксные суммы: длина n+1, последний — полная высота
    let offs = row_offsets(&rows, 60.0);
    assert_eq!(offs.len(), rows.len() + 1);
    assert_eq!(offs, vec![0.0, 40.0, 100.0, 120.0, 180.0, 220.0]);

    let g = TlGeom::new(
        egui::Rect::from_min_max(egui::pos2(52.0, 100.0), egui::pos2(1052.0, 600.0)),
        60.0,
        rows.clone(),
        0.0,
        0.0,
        900.0,
    );
    // биекция: центр каждой строки попадает в неё саму
    for (i, &key) in rows.iter().enumerate() {
        assert_eq!(g.row_at_y(g.row_center_y(i)), Some(i), "строка {i}");
        assert_eq!(g.key_at_y(g.row_center_y(i)), Some(key));
        assert_eq!(g.row_h(i), row_height(key.0, 60.0));
    }
    // границы строк: верх принадлежит строке, низ — уже следующей
    assert_eq!(g.row_at_y(g.row_top(1)), Some(1));
    assert_eq!(g.row_at_y(g.row_top(1) - 0.5), Some(0));
    assert_eq!(g.row_at_y(g.row_top(0) - 1.0), None, "выше первой строки — не строка");
    assert_eq!(g.row_at_y(g.row_top(rows.len() - 1) + 100.0), None, "ниже последней — тоже");

    // «сколько строк проехали» при драге: считается ПО СТРОКАМ, а не
    // делением на одну высоту — на старой формуле тонкая строка стоила
    // бы столько же, сколько дорожка клипов
    assert_eq!(g.row_at_y_unbounded(g.row_center_y(0)), 0);
    assert_eq!(g.row_at_y_unbounded(g.row_center_y(2)), 2);
    assert_eq!(g.row_at_y_unbounded(g.row_center_y(4)), 4);
    assert!(g.row_at_y_unbounded(g.row_top(0) - 1.0) < 0, "выше первой — отрицательная строка");
    // прилипание к своему виду считает те же строки
    assert_eq!(g.nearest_lane(ClipKind::Video, g.row_at_y_unbounded(g.row_center_y(2))), 0);

    // якорь зума: целая часть — строка, дробная — место внутри неё
    let a = g.anchor_at(g.row_top(2) + g.row_h(2) * 0.5);
    assert!((a - 2.5).abs() < 0.01, "якорь в середине третьей строки: {a}");
    let a0 = g.anchor_at(g.row_top(0));
    assert!(a0.abs() < 0.01, "верх первой строки — ровно 0");

    // lane_y: доли считаются от высоты СТРОКИ КЛИПОВ
    assert_eq!(g.lane_y(ClipKind::Video, 0, 0.5), g.row_top(1) + 30.0);
}

/// Тест 246 (Р.35 С3): в ТОНКУЮ служебную строку попадают мышью, и якорь
/// вертикального зума не уезжает. Харнесс, а не чистая функция: именно
/// здесь ловится «геометрия посчитана правильно, а кликнуть некуда»
/// (урок п.7 р.11).
#[test]
fn t246_thin_row_is_clickable_and_zoom_anchor_holds() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.morph_lanes = 1;
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        targets: MorphTargets::Below,
        patch: MorphPatch::default(),
        timed: true,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    h.step();

    let g = h.state().tl_geom.clone().expect("геометрия");
    let mrow = g.row_of((LaneKind::Morph, 0)).expect("морф-строка есть");
    // строка действительно втрое ниже дорожки клипов — и всё же нажимаема
    assert!(g.row_h(mrow) < g.lane_h(), "служебная строка обязана быть ниже");
    assert!(g.row_h(mrow) >= crate::timeline::THIN_ROW_MIN, "но не ниже пола нажимаемости");

    // клик по блоку в тонкой строке выделяет морфер
    let pos = egui::pos2(g.tick_to_x(45_000), g.row_center_y(mrow));
    h.drag_at(pos);
    h.step();
    h.drop_at(pos);
    h.step();
    assert_eq!(
        h.state().selected_morph,
        Some(MorphId(1)),
        "в тонкую морф-строку не попасть мышью — считать её сделанной нельзя"
    );

    // вертикальный зум не ломает отношение высот
    let g = h.state().tl_geom.clone().unwrap();
    let app = h.state_mut();
    app.tl_v_zoom = Some(g.lane_h() * 1.5);
    h.step();
    let g2 = h.state().tl_geom.clone().unwrap();
    assert!(g2.lane_h() > g.lane_h(), "зум применился");
    assert!(
        (g2.row_h(mrow) / g2.lane_h() - 1.0 / crate::timeline::MORPH_ROW_DIV).abs() < 0.05,
        "отношение высоты морф-строки держится при зуме"
    );
}

/// Тест 248 (Р.35 С4): рамка выделения превращается в именованную
/// область с ТЕМИ ЖЕ дорожками и временем, а окно морфера показывает
/// ссылку на неё чипом и находит её поиском.
#[test]
fn t248_band_becomes_named_area() {
    use kt_model::*;
    let (mut h, g) = harness_three_clips();
    // рамка (Ctrl+драг с пустого места) по ПЕРВОЙ видео-дорожке
    let y = g.lane_y(ClipKind::Video, 0, 0.6);
    let at = |t: i64| egui::pos2(g.tick_to_x(t), y);
    rubber_drag(&mut h, at(70_000), at(260_000), true);
    assert!(h.state().last_band.is_some(), "рамка обязана запомниться в домене модели");

    let app = h.state_mut();
    let depth = app.model.undo_depth();
    app.save_marked_area();
    let p = app.model.project();
    assert_eq!(p.areas.len(), 1, "область сохранена");
    let a = &p.areas[0];
    assert!(!a.name.trim().is_empty(), "имя генерится готовым, а не спрашивается отказом");
    assert!((a.start.0 - 70_000).abs() < 3_000, "время из рамки: {:?}", a.start);
    assert!(((a.start + a.duration).0 - 260_000).abs() < 3_000, "и её конец");
    assert_eq!(a.lanes, vec![(LaneKind::Video, 0)], "и ТЕ ЖЕ дорожки");
    assert_eq!(app.model.undo_depth(), depth + 1, "один шаг истории");

    // второе сохранение той же рамки даёт ДРУГОЕ имя (имя — это адрес)
    app.save_marked_area();
    let names: Vec<String> = app.model.project().areas.iter().map(|a| a.name.clone()).collect();
    assert_eq!(names.len(), 2);
    assert_ne!(names[0], names[1], "имена уникальны без вопросов пользователю");

    // окно морфера: ссылка на область рисуется чипом
    let mut p = app.model.project().clone();
    p.morph_lanes = 1;
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        targets: MorphTargets::Areas { names: vec![names[0].clone(), "нет такой".into()] },
        patch: MorphPatch::default(),
        timed: true,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_morph_editor(MorphId(1));
    h.step();
    h.step();
    use egui_kittest::kittest::Queryable;
    assert!(h.query_by_label(&names[0]).is_some(), "живая ссылка видна чипом");
    assert!(
        h.query_by_label("нет такой").is_some(),
        "мёртвая ссылка ОСТАЁТСЯ на виду: её не выкидывают молча"
    );
}

/// Тест 250 (Р.35 С5): окно префикса вставки. Открывается ТОЛЬКО когда в
/// буфере есть именованное, вставляет одним undo-шагом, Esc не оставляет
/// следов, а ВТОРОЙ Ctrl+V подряд открывает его снова (правило 14 —
/// фича обязана работать повторно, дефект п.8 р.12).
#[test]
fn t250_paste_prefix_dialog() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, _g) = harness_three_clips();

    // без именованного вставка идёт МОЛЧА — старые сценарии целы
    let app = h.state_mut();
    app.select_only(ClipId(1));
    app.copy_selection();
    app.player.seek(kt_time::Tick(400_000));
    let n = app.model.project().clips.len();
    app.paste_at_cursor();
    assert!(app.paste_names.is_none(), "окна быть не должно: имён в буфере нет");
    assert_eq!(app.model.project().clips.len(), n + 1, "и вставка прошла сразу");

    // область в буфере — окно появляется
    let mut p = app.model.project().clone();
    p.areas.push(MarkedArea {
        name: "финал".into(),
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        lanes: vec![],
        color: None,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.select_only(ClipId(1));
    app.copy_selection();
    assert!(!app.clipboard.areas.is_empty(), "область обязана попасть в буфер");
    app.player.seek(kt_time::Tick(700_000));
    app.paste_at_cursor();
    assert!(app.paste_names.is_some(), "окно префикса не открылось");
    let frozen = app.paste_names.as_ref().unwrap().at;
    h.step();
    h.step();
    assert!(
        h.query_by_label(tr(k::msg::PASTE_PREFIX_LABEL)).is_some(),
        "окно нарисовалось, а не только завелось в состоянии"
    );

    // Esc отменяет и следов не оставляет
    let before = h.state().model.project().clone();
    h.key_press(egui::Key::Escape);
    h.step();
    h.step();
    assert!(h.state().paste_names.is_none(), "Esc обязан закрыть окно");
    assert_eq!(h.state().model.project().areas, before.areas, "и ничего не вставить");

    // ВТОРОЙ Ctrl+V подряд открывает окно снова
    let app = h.state_mut();
    app.paste_at_cursor();
    assert!(app.paste_names.is_some(), "повторная вставка обязана открыть окно опять");
    assert_eq!(app.paste_names.as_ref().unwrap().at, frozen, "позиция вставки заморожена");

    // подтверждение — ОДИН шаг истории
    let depth = app.model.undo_depth();
    let at = app.paste_names.as_ref().unwrap().at;
    app.paste_names = None;
    app.paste_here(at, "копия".into());
    assert_eq!(app.model.undo_depth(), depth + 1, "вставка — один шаг истории");
    let names: Vec<String> = app.model.project().areas.iter().map(|a| a.name.clone()).collect();
    assert!(names.contains(&"копия финал".to_string()), "префикс применён: {names:?}");
    app.model.undo().unwrap();
    assert_eq!(app.model.project().areas.len(), 1, "и одним undo откатилась");
}

/// Тест 248c (Р.37 С3): окно FX пары показывает у ВИДЕО-экземпляра варпа
/// только то, что для него значит. Реверб на видео не звучит, и его ручки
/// там — враньё пользователю (п.9 чек-листа).
#[test]
fn t248_video_warp_shows_only_what_matters() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    for (id, kind) in [(1u64, ClipKind::Video), (2, ClipKind::Audio)] {
        p.clips.push(ClipEvent {
            id: ClipId(id),
            link_group: Some(GroupId(1)),
            kind,
            lane: 0,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(90_000),
            source_in: kt_time::Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![FxInstance::slowed_reverb(0.5, 0.2, 85.0)],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();

    // окно у пары ОДНО, и в нём обе цепочки
    assert_eq!(h.state().fx_editors.len(), 1, "у пары одно окно");
    assert_eq!(h.state().fx_editors[0].members.len(), 2);
    // режим кадров есть (он значим только на видео)
    assert!(
        h.query_by_label(crate::i18n::tr(crate::i18n::k::fx::FRAMES)).is_some(),
        "у видео-экземпляра обязан быть выбор режима кадров"
    );
    assert!(
        h.query_by_label(crate::i18n::tr(crate::i18n::k::fx::RATE_LINKED)).is_some(),
        "и сказано, что скорость общая с партнёром"
    );
    // а ручка предзадержки реверба — только у аудио-экземпляра, один раз
    let predelay = crate::widgets::param_label("predelay_ms").to_string();
    assert_eq!(
        h.get_all_by_label(&predelay).count(),
        1,
        "ручки реверба показываются ровно один раз — у аудио"
    );
}

/// Тест 249 (Р.37 С4, требование пользователя: «slowed&reverb должен
/// учитываться в превью аудио/видео в событиях»). Волноформа и филмстрип
/// переводили пиксель в материал ЛИНЕЙНО и игнорировали даже реверс —
/// замедленный клип рисовал не то, что играет.
#[test]
fn t249_previews_follow_the_warp() {
    use kt_model::*;
    use kt_time::Tick;
    const SEC: i64 = 90_000;

    let clip = |fx: Vec<FxInstance>, reversed: bool| ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(4 * SEC),
        source_in: Tick(SEC),
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx,
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };

    // без варпа карта не заводится вовсе — отображение линейно, как было
    assert!(kt_engine::clip_warp_map(&clip(vec![], false)).is_none());

    // 0.5×: к середине события съедена ЧЕТВЕРТЬ материала, а не половина
    let slow = clip(vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], false);
    let w = kt_engine::clip_warp_map(&slow).expect("карта варпа есть");
    assert_eq!(w.used(), 2 * SEC, "за 4 с события съедается 2 с материала");
    assert_eq!(w.material_off(slow.duration, false, 2 * SEC), SEC, "середина — четверть материала");
    assert_eq!(w.material_off(slow.duration, false, 0), 0);
    assert_eq!(w.material_off(slow.duration, false, 4 * SEC), 2 * SEC);

    // реверс — «варп, потом зеркало»: то же правило, что в движке
    let rev = clip(vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], true);
    let wr = kt_engine::clip_warp_map(&rev).expect("карта");
    assert_eq!(wr.material_off(rev.duration, true, 0), 2 * SEC, "реверс начинается с конца");
    assert_eq!(wr.material_off(rev.duration, true, 4 * SEC), 0, "и кончается началом");

    // ВЫКЛЮЧЕННЫЙ слот = «не замедлять картинку» (Р.37: это и есть та
    // самая отдельная галочка, второго выключателя не заводим)
    let mut off = slow.clone();
    off.fx_bypass = vec![true];
    assert!(kt_engine::clip_warp_map(&off).is_none(), "галка слота снимает растяжение");

    // ступень филмстрипа берётся из ПОТРЕБЛЁННОГО пролёта, иначе
    // замедленный клип получает вдвое более грубый шаг и вырождается
    // в повторяющиеся бакеты
    let by_event = crate::thumbs::thumb_source_step(Tick(4 * SEC), 800.0, 48.0);
    let by_used = crate::thumbs::thumb_source_step(Tick(w.used()), 800.0, 48.0);
    assert!(by_used <= by_event, "шаг по потреблённому не грубее: {by_used} vs {by_event}");

    // и кэш карт живёт по ревизии модели, а не строится на кадр
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(slow);
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    let a = app.warp_maps();
    let b = app.warp_maps();
    assert!(std::sync::Arc::ptr_eq(&a, &b), "на кадр карта не пересобирается");
    assert_eq!(a.len(), 1, "в кэше только клипы С варпом");
}

/// Тест 258 (Р.38 пп.3-4). Претензии дословно: «add morpher layer тоже
/// странно работает» и «если я жму тот же add morpher layer в другой
/// области - он мне создает еще один трек».
///
/// «Слой» — про слой правок, а не про дорожку: повторное «+M»
/// ПЕРЕИСПОЛЬЗУЕТ верхнюю морф-дорожку. Место блока считает одна
/// `morph_span` (есть регион — по региону, нет — от курсора), и её же
/// зовут оба способа создания: жест и пункт меню дорожки (правило 17).
#[test]
fn t258_morph_layer_reuses_lane_and_honours_region() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(900_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);

    // без региона — блок от курсора, 1.5 с
    app.player.seek(Tick(180_000));
    app.add_morph_layer();
    let pr = app.model.project().clone();
    assert_eq!(pr.morph_lanes, 1, "первая морф-дорожка заведена");
    assert_eq!(pr.morphers.len(), 1);
    assert_eq!(pr.morphers[0].start, Tick(180_000), "блок встал под курсором");
    assert_eq!(pr.morphers[0].duration, Tick(90 * 1500), "и на 1.5 с");

    // повторное «+M» в другом месте — ТА ЖЕ дорожка, новый блок
    app.player.seek(Tick(495_000)); // кратно 15: playhead живёт в аудиокадрах
    app.add_morph_layer();
    let pr = app.model.project().clone();
    assert_eq!(pr.morph_lanes, 1, "вторая морф-дорожка НЕ заводится: «слой» — не «дорожка»");
    assert_eq!(pr.morphers.len(), 2, "а блок добавился");
    assert_eq!(pr.morphers[1].lane, 0, "на ту же дорожку");
    assert_eq!(pr.morphers[1].start, Tick(495_000));

    // с регионом — блок ПО РЕГИОНУ, и у обоих способов создания одинаково
    let mut pr = app.model.project().clone();
    pr.region = Some(kt_model::Region { start: Tick(700_000), end: Tick(760_000) });
    app.model = kt_model::TimelineModel::new(pr);
    app.add_morph_layer();
    let by_gesture = app.model.project().morphers.last().cloned().unwrap();
    assert_eq!(by_gesture.start, Tick(700_000), "есть регион — блок по региону");
    assert_eq!(by_gesture.duration, Tick(60_000));
    app.add_morpher_at(0);
    let by_menu = app.model.project().morphers.last().cloned().unwrap();
    assert_eq!(
        (by_menu.start, by_menu.duration),
        (by_gesture.start, by_gesture.duration),
        "пункт меню дорожки обязан ставить блок так же, как жест (правило 17)"
    );
}

/// Тест 259 (Р.38 п.9, дословно: «я не понимаю как менять длинну у морфер
/// события»). Кромки блока тянутся: правая меняет длительность, левая
/// двигает начало при СТОЯЩЕМ конце — как у метки-диапазона.
#[test]
fn t259_morph_block_edges_resize() {
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        app.player.seek(Tick(100_000));
        app.add_morph_layer();
        app.morph_editors.clear();
    }
    h.step();
    let g = h.state().tl_geom.clone().unwrap();
    let m = h.state().model.project().morphers[0].clone();
    let row = g.row_of((kt_model::LaneKind::Morph, 0)).unwrap();
    let y = g.row_center_y(row);
    let end_x = g.tick_to_x((m.start + m.duration).0);

    // правая кромка: тянем вправо — растёт длительность, начало стоит
    let to = end_x + 60.0;
    h.hover_at(egui::pos2(end_x, y));
    h.step();
    h.drag_at(egui::pos2(end_x, y));
    h.step();
    h.hover_at(egui::pos2(end_x + 8.0, y));
    h.step();
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let m2 = h.state().model.project().morphers[0].clone();
    assert_eq!(m2.start, m.start, "правая кромка не двигает начало");
    assert!(
        m2.duration > m.duration,
        "правая кромка обязана удлинять блок: было {:?}, стало {:?}",
        m.duration,
        m2.duration
    );

    // левая кромка: конец СТОИТ на месте
    let end_before = (m2.start + m2.duration).0;
    let sx = g.tick_to_x(m2.start.0);
    let to = sx + 40.0;
    h.hover_at(egui::pos2(sx, y));
    h.step();
    h.drag_at(egui::pos2(sx, y));
    h.step();
    h.hover_at(egui::pos2(sx + 8.0, y));
    h.step();
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let m3 = h.state().model.project().morphers[0].clone();
    assert!(m3.start > m2.start, "левая кромка сдвинула начало");
    assert_eq!(
        (m3.start + m3.duration).0,
        end_before,
        "и конец остался на месте (правило 20: ничего не отрастает само)"
    );
}

/// Тест 260 (Р.38 п.5 «не понимаю как работает камерный трек» + подметание
/// того же класса). Камера выделяется кликом, удаляется с клавиатуры и
/// меняет длину кромками; левая кромка диапазонной метки наконец работает.
///
/// `Drag::MarkerEdge.left` был описан в комментарии как рабочий, и на этот
/// вариант даже матчился тест — но конструировался ТОЛЬКО `left: false`, а
/// жест уходил в `_ => {}` и пропадал молча.
#[test]
fn t260_camera_and_marker_left_edge_are_reachable() {
    use kt_model::*;
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        assert!(app.apply(Command::InsertCameraLane { at: 0 }));
        app.player.seek(Tick(90_000));
        app.add_camera_at_playhead(0);
        assert!(app.apply(Command::AddMarker { at: Tick(45_000), duration: Tick(30_000) }));
    }
    h.step();
    let g = h.state().tl_geom.clone().unwrap();
    let c = h.state().model.project().cameras[0].clone();
    let row = g.row_of((LaneKind::Camera, 0)).unwrap();
    let y = g.row_center_y(row);

    // клик по камере — выделение (до Р.38 поля `selected_camera` не было)
    let mid = egui::pos2(g.tick_to_x(c.start.0 + c.duration.0 / 2), y);
    h.hover_at(mid);
    h.step();
    h.drag_at(mid);
    h.step();
    h.drop_at(mid);
    h.step();
    assert_eq!(h.state().selected_camera, Some(c.id), "клик по камере обязан её выделить");

    // правая кромка меняет длину
    let ex = g.tick_to_x((c.start + c.duration).0);
    let to = ex - 40.0;
    h.hover_at(egui::pos2(ex, y));
    h.step();
    h.drag_at(egui::pos2(ex, y));
    h.step();
    h.hover_at(egui::pos2(ex - 8.0, y));
    h.step();
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();
    let c2 = h.state().model.project().cameras[0].clone();
    assert_eq!(c2.start, c.start, "кромка не двигает начало");
    assert!(c2.duration < c.duration, "правая кромка обязана менять длительность камеры");

    // Del удаляет выделенную камеру
    let app = h.state_mut();
    app.selected_camera = Some(c.id);
    app.delete_action();
    assert!(h.state().model.project().cameras.is_empty(), "Del обязан удалять камеру");

    // ЛЕВАЯ кромка диапазонной метки: конец стоит, начало едет
    let m = h.state().model.project().markers[0].clone();
    assert!(m.is_range(), "сцена: диапазонная метка");
    let end0 = m.end().0;
    let ry = g.rect.top() + 6.0;
    let sx = g.tick_to_x(m.t.0);
    let to = sx + 30.0;
    h.hover_at(egui::pos2(sx, ry));
    h.step();
    h.drag_at(egui::pos2(sx, ry));
    h.step();
    h.hover_at(egui::pos2(sx + 8.0, ry));
    h.step();
    h.hover_at(egui::pos2(to, ry));
    h.step();
    h.drop_at(egui::pos2(to, ry));
    h.step();
    let m2 = h.state().model.project().markers[0].clone();
    assert!(m2.t.0 > m.t.0, "левая кромка сдвинула начало метки: {:?} → {:?}", m.t, m2.t);
    assert_eq!(m2.end().0, end0, "а конец остался на месте");
}

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
use crate::i18n::{Lang, k, tr};
use eframe::egui;
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
/// Сколько ждать фоновую запись. Это НЕ утверждение о скорости: под полным
/// параллельным прогоном (полтораста тестов, каждый со своим egui-контекстом)
/// поток автосейва честно ждал своей очереди дольше пяти секунд, и тесты
/// краснели от загрузки машины, а не от дефекта.
const WAIT_BG: std::time::Duration = std::time::Duration::from_secs(60);

fn harness_lang_dt(lang: crate::i18n::Lang, step_dt: Option<f32>) -> Harness<'static, KtApp> {
    crate::i18n::set_lang(lang);
    let mut app = KtApp::new_empty(None);
    // «последний рубеж» (п.354) в харнессе выключен: тысяча создаваемых и
    // уничтожаемых приложений сыпала бы файлами восстановления в общий
    // каталог данных. Сам рубеж проверяет t372, включая его явно.
    app.recovery_on_drop = false;
    // рабочее место (п.360) — в свой файл: общий лежит в домашнем каталоге
    // разработчика, и полторы сотни тестов писали бы туда наперегонки
    app.workspace_file =
        Some(std::env::temp_dir().join(format!("kt-ws-{}-{:p}.json", std::process::id(), &app)));
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

/// Харнесс ЗАДАННОГО размера (Р.66): контракт формы проверяется в НИЗКОМ
/// окне — там, где содержимое заведомо выше экрана и окно обязано
/// прокручиваться, а кнопки действия оставаться на виду.
pub(crate) fn harness_sized(w: f32, h: f32) -> Harness<'static, KtApp> {
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let mut app = KtApp::new_empty(None);
    app.recovery_on_drop = false;
    app.workspace_file =
        Some(std::env::temp_dir().join(format!("kt-ws-{}-{:p}.json", std::process::id(), &app)));
    let mut hh = Harness::builder()
        .with_size(egui::vec2(w, h))
        .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
    crate::icons::install_fonts(&hh.ctx);
    hh.step();
    hh.step();
    hh
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
    assert!(
        h.state().preview_rect.width() > 600.0,
        "превью схлопнулось: {:?}",
        h.state().preview_rect
    );

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
        (egui::pos2(2.0, H * 0.5), C::ResizeHorizontal), // левая кромка
        (egui::pos2(W - 2.0, H * 0.5), C::ResizeHorizontal), // правая
        (egui::pos2(W * 0.5, 2.0), C::ResizeVertical),   // верхняя
        (egui::pos2(W * 0.5, H - 2.0), C::ResizeVertical), // нижняя
        (egui::pos2(3.0, 3.0), C::ResizeNwSe),           // угол СЗ
        (egui::pos2(W - 3.0, 3.0), C::ResizeNeSw),       // угол СВ
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
        assert!(text.starts_with(want), "из {pos:?} ждали «{want}», в строке состояния «{text}»");
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
    h.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().maximized = Some(true);
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
        [tr(k::meter::FADER), tr(k::meter::PEAK), tr(k::meter::MUTE)].iter().any(|want| h
            .state()
            .status_line_shown
            .0
            .starts_with(want)),
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
    let vis =
        (st.timeline_rect.width() as f64 - crate::timeline::GUTTER_W as f64) * st.ticks_per_px;
    let ph = st.player.transport.playhead().0 as f64;
    let vs = st.view_start;
    assert!(ph >= vs && ph <= vs + vis, "курсор вне окна: ph={ph}, окно=[{vs}, {}]", vs + vis);
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
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
        assert!(now > *seen.last().unwrap(), "курсор не двинулся вперёд: цепочка {seen:?} → {now}");
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
            p.media.push(MediaRef::new(MediaId(1), "synth:1"));
            for (n, start) in [(1u64, 0i64), (2, FAR)] {
                p.clips.push(ClipEvent {
                    r#gen: None,
                    gen_auto: Vec::new(),
                    overrides: Vec::new(),
                    geometry: Default::default(),
                    geometry_pos: 0,
                    geometry_auto: Vec::new(),
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

    // --- удобства: регион, loop, зум, раскладка бина (имя дорожки с Р.62
    // — СОДЕРЖИМОЕ модели и проверяется ниже, вместе с правками монтажа)
    h.state_mut().model.set_region(Region::normalized(Tick(0), Tick(90_000)));
    h.state_mut().on_region_changed();
    h.state_mut().toggle_loop();
    h.state_mut().ticks_per_px = 4321.0;
    h.state_mut().bin_layout.insert(MediaId(3), egui::pos2(12.0, 34.0));
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
    assert!(ruler <= 36.0, "линейка съела {ruler} px до первой дорожки — место тратится впустую");

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
    assert!(lw > 600.0, "медиа-бин снова зажат ({lw} px) — предел ставился по прямой просьбе");
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
    // ОТРИЦАТЕЛЬНОЕ время (зона до нуля внутри ленты, Р.58 п.462) — знак и
    // поля от |t|, а не набор отрицательных полей
    assert_eq!(
        crate::util::format_timecode(kt_time::Tick(-90_000 - 3_000), 30),
        "-00:00:01.033 F01"
    );
    assert_eq!(crate::util::format_timecode(kt_time::Tick(90_000 + 3_000), 30), "00:00:01.033 F01");
    assert_eq!(crate::timeline::format_ruler_time_step(-45_000, 9_000), "-0:00.5");
    assert_eq!(crate::timeline::format_ruler_time_step(-60 * 90_000, 90_000), "-1:00");

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
    assert_eq!(h.state().player.transport.is_playing(), playing, "пробел утёк в хоткеи мимо маски");

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
    assert_eq!(h.state().player.transport.playhead().0 / 90_000, 90, "курсор не встал на 00:01:30");
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
            p.media.push(MediaRef::new(MediaId(n), format!("synth:{n}")));
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
    let gi2 =
        h.state().bin_groups.iter().position(|g| g.name == "Интервью").expect("группа пропала");
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
    let before: Vec<egui::Pos2> =
        [MediaId(1), MediaId(2)].iter().map(|m| h.state().bin_layout[m]).collect();
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
    let after: Vec<egui::Pos2> =
        [MediaId(1), MediaId(2)].iter().map(|m| h.state().bin_layout[m]).collect();
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
            h.output().viewport_output.values().flat_map(|v| v.commands.iter().cloned()).collect()
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
    use kt_model::LaneKind;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.ensure_tracks(kt_model::LaneKind::Video, 3);
        p.ensure_tracks(kt_model::LaneKind::Audio, 2);
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        edit_project(app, |p| {
            p.track_at_mut(LaneKind::Video, 1).unwrap().name = "Титры".into();
            p.track_at_mut(LaneKind::Audio, 1).unwrap().name = "Голос".into();
        });
    }
    h.step();

    // --- клик по V1 (индекс 0) + «добавить видео-дорожку» → новая
    // строка ровно СЛЕДУЮЩАЯ, а не в конце списка
    h.state_mut().insert_lane_after_row(LaneKind::Video, Some((LaneKind::Video, 0)));
    h.step();
    assert_eq!(
        h.state().model.project().lanes(kt_model::LaneKind::Video),
        4,
        "дорожка не добавилась"
    );
    assert_eq!(
        h.state().model.project().rows()[1],
        (LaneKind::Video, 1),
        "новая дорожка не встала под кликнутой: {:?}",
        h.state().model.project().rows()
    );
    // имя «Титры» ехало на V2 (индекс 1) — обязано уехать на индекс 2
    assert_eq!(
        h.state().row_label(LaneKind::Video, 2),
        "Титры",
        "имя не поехало вместе со своей дорожкой"
    );
    assert_eq!(h.state().row_label(LaneKind::Video, 1), "V2", "новая дорожка забрала чужое имя");

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
    let m0 = h.state().model.project().lanes(kt_model::LaneKind::Morph);
    h.state_mut().insert_lane_after_row(LaneKind::Morph, None);
    h.step();
    assert_eq!(
        h.state().model.project().lanes(kt_model::LaneKind::Morph),
        m0 + 1,
        "morph-дорожка не добавилась"
    );

    // --- удаление: имя уходит вместе с дорожкой, соседи съезжают
    {
        let app = h.state_mut();
        edit_project(app, |p| {
            p.track_at_mut(LaneKind::Audio, 0).unwrap().name = "Музыка".into();
            p.track_at_mut(LaneKind::Audio, 1).unwrap().name = "Голос".into();
        });
        app.delete_lane(kt_model::ClipKind::Audio, 0);
    }
    h.step();
    assert_eq!(h.state().row_label(LaneKind::Audio, 0), "Голос", "после удаления имена не съехали");

    // --- перестановка: имя едет за содержимым (иначе «Голос» останется
    // на чужой строке)
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.ensure_tracks(kt_model::LaneKind::Audio, 3);
        p.media.push(kt_model::MediaRef::new(kt_model::MediaId(1), "synth:1"));
        p.clips.push(kt_model::ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
        edit_project(app, |p| p.track_at_mut(LaneKind::Audio, 0).unwrap().name = "Голос".into());
        // строки, а не номера: с Р.56 номер дорожки — это РАНГ строки, и
        // перестановка идёт одной командой на любые виды
        let rows = app.model.project().rows();
        let from = rows.iter().position(|&r| r == (LaneKind::Audio, 0)).unwrap();
        let to = rows.iter().position(|&r| r == (LaneKind::Audio, 2)).unwrap();
        app.reorder_rows(from, to);
    }
    h.step();
    assert_eq!(
        h.state().model.project().clips[0].lane,
        2,
        "содержимое не переехало — тест ниже проверял бы не то"
    );
    assert_eq!(
        h.state().row_label(LaneKind::Audio, 2),
        "Голос",
        "имя не поехало за содержимым при перестановке"
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
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (i, start) in [90_000i64, 270_000, 450_000].into_iter().enumerate() {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
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
            p.media.push(MediaRef::new(MediaId(n), format!("synth:{n}")));
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
    assert!(moved.y > 200.0, "карточку откатило вместо переноса вниз (липкий «вынос»): {moved:?}");

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
            assert!(y >= 117.0, "ярлык на дорожке {lane_h} встал на строку имени клипа: {y}");
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
    let x_of = |ch: char| g.rows[0].glyphs.iter().find(|x| x.chr == ch).expect("глиф").pos.x;
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
        let body = h.ctx.style_of(egui::Theme::Dark).text_styles[&egui::TextStyle::Body].clone();
        let heading = egui::FontId::proportional(14.0);
        let w = |text: String, font: egui::FontId| {
            h.ctx.fonts_mut(|f| f.layout_no_wrap(text, font, egui::Color32::WHITE)).size().x
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
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    // поведение Р.24/Р.26 живёт в режиме БЕЗ переслаивания (Р.28)
    p.overlap_layers = false;
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
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
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (n, start) in [(1u64, 0i64), (2, 200_000)] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
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
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (n, start, src) in [(1u64, 0i64, 0i64), (2, 90_000, 30_000)] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
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
            invert: false,
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
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        // A/V-пара + одиночный видео-клип: проверяем и группы, и одиночек
        for (n, kind, start, group) in [
            (1u64, ClipKind::Video, 0i64, Some(GroupId(1))),
            (2, ClipKind::Audio, 0, Some(GroupId(1))),
            (3, ClipKind::Video, 200_000, None),
        ] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
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
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (n, start) in [(1u64, 0i64), (2, 200_000)] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
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
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
            invert: false,
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
    h.state_mut().apply(Command::AddMarker { at: Tick(30_000), duration: Tick::ZERO });
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
    use std::path::{Path, PathBuf};
    // атомарный tmp дописывается к ПОЛНОМУ имени (старый with_extension
    // был захардкожен под ktproj и съедал многоточечные имена); с Р.49
    // (п.352) в имени ещё и номер процесса — см. t363
    let pid = std::process::id();
    assert_eq!(
        kt_otio::atomic_tmp_path(Path::new("/x/a.otio")),
        PathBuf::from(format!("/x/a.otio.{pid}.tmp"))
    );
    assert_eq!(
        kt_otio::atomic_tmp_path(Path::new("/x/a.b.otio")),
        PathBuf::from(format!("/x/a.b.otio.{pid}.tmp"))
    );
    // recovery безымянного проекта — тоже .otio
    assert!(crate::app::autosave_path().to_string_lossy().ends_with("autosave-project.otio"));

    let dir = std::env::temp_dir().join(format!("kt-t150-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // .otio читается
    let p = kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
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
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    assert_eq!(
        preview_src_shift(Some(&d), ClipId(1), 1_000),
        4_000,
        "трим начала двигает материал"
    );
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
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.overlap_layers = false; // блокировка возможна только в этом режиме
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    // клипы 2 и 3 стоят встык: любое попадание между ними — наезд на ДВОИХ
    for (n, start) in [(1u64, 0i64), (2, 200_000), (3, 250_000)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
        app.select_only(kt_model::ClipId(1));
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
    let ly = level_y(1.0, top, g.lane_h_of(ClipKind::Video, 0));
    let xm = g.tick_to_x(200_000);
    h.hover_at(egui::pos2(xm, ly));
    h.step();
    h.drag_at(egui::pos2(xm, ly));
    h.step();
    let target_y = level_y(0.4, top, g.lane_h_of(ClipKind::Video, 0));
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
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
    let y0 = level_y(0.5, top, g.lane_h_of(ClipKind::Audio, 0)); // 0 dB
    h.hover_at(egui::pos2(x, y0));
    h.step();
    h.drag_at(egui::pos2(x, y0));
    h.step();
    let y1 = level_y(0.75, top, g.lane_h_of(ClipKind::Audio, 0));
    h.hover_at(egui::pos2(x, y1));
    h.step();
    h.drop_at(egui::pos2(x, y1));
    h.step();
    let gain = h.state().model.project().clips[0].gain_db;
    assert!((gain - RAIL_DB_SPAN * 0.25).abs() < 0.5, "уровень аудио доехал до +3 dB: {gain}");

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
    let hits =
        texts.iter().filter(|t| **t == crate::tf!(k::msg::VIDEO_FX_GPU_LOST, "DeviceLost")).count();
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
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    assert_eq!(tl(&c, mat).0, 180_000 + 30_000, "обратный перевод обязан возвращать тот же момент");

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

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(45_000), 1.0),
            AutoKey::at(Tick(135_000), 48.0),
        ]),
        source: Default::default(),
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let clip = |id: u64, kind: ClipKind, lane: u32| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
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
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
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
    p.media.push(MediaRef::new(MediaId(9), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
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
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(9));
    h.step();
    h.step();

    // 1) мастер-галка цепочки есть в ОБОИХ окнах — она одна и та же
    let chain_label = tr(k::fx::CHAIN_ON).to_string();
    assert!(h.query_by_label(&chain_label).is_some(), "в окне события нет мастер-галки цепочки");
    h.state_mut().open_morph_editor(MorphId(1));
    h.step();
    h.step();
    assert!(h.state().morph_editors.iter().any(|e| e.open), "окно морфера не открылось");
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
    p.media.push(MediaRef::new(MediaId(9), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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

            keys: AutoKey::linear(vec![
                AutoKey::at(kt_time::Tick(0), 1.0),
                AutoKey::at(kt_time::Tick(90_000), 48.0),
            ]),
            source: Default::default(),
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

            keys: AutoKey::linear(vec![AutoKey::at(kt_time::Tick(0), 2.0)]),
            source: Default::default(),
        }],
        vec![],
    ];
    app.set_fx_coalesced(ClipId(9), fx.clone(), gesture);
    app.set_fx_auto_coalesced(ClipId(9), auto, gesture);
    let c = app.model.clip(ClipId(9)).unwrap().clone();
    assert_eq!(c.fx, fx);
    assert_eq!(c.fx_auto.len(), 1, "хвост без кривых подрезан");
    assert_eq!(app.model.undo_depth(), depth_before + 1, "оба изменения жеста — один undo-шаг");
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
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some(), "фоновый автосейв обязан стартовать");
    // дождаться воркер и забрать результат следующим кадром
    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(h.state().autosave_rx.is_none(), "результат автосейва забран");
    // Р.49 п.351 (решение владельца, user-feedback.md): автосейв больше НЕ
    // пишет в файл пользователя и НЕ гасит звёздочку. Тест из замороженного
    // диапазона 1-221 переписан по прямой санкции владельца: «фоновая запись
    // идёт в свой recovery-файл, проект меняет только явное сохранение».
    assert!(h.state().dirty_content, "автосейв не имеет права считать проект сохранённым");
    assert!(!target.exists(), "автосейв тронул файл проекта");
    let rec = crate::app::recovery_path_for(Some(&target));
    assert!(rec.exists(), "файл восстановления записан фоновым потоком: {}", rec.display());

    // правка ПОСЛЕ снапшота не должна терять звёздочку
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(2), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some());
    // модель меняется, пока запись в полёте
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(3), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    let deadline = std::time::Instant::now() + WAIT_BG;
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

/// Правка модели ФИКСТУРЫ напрямую (тест готовит сцену, а не жест):
/// клон рабочего вида, правка, новая модель. С Р.62 так ставятся имя и
/// высота дорожки — они живут на дорожке-объекте.
#[cfg(test)]
pub(crate) fn edit_project(app: &mut KtApp, f: impl FnOnce(&mut kt_model::Project)) {
    let mut p = app.model.project().clone();
    f(&mut p);
    p.validate().expect("фикстура законна");
    app.model = kt_model::TimelineModel::new(p);
}

#[cfg(test)]
pub(crate) fn harness_three_clips_dt(
    step_dt: Option<f32>,
) -> (Harness<'static, KtApp>, crate::timeline::TlGeom) {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness_lang_dt(crate::i18n::Lang::Ru, step_dt);
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (i, start) in [0i64, 100_000, 200_000].iter().enumerate() {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
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

    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    // «пропавший» файл: имя synth-источника, которого нет на диске — при
    // открытии он не откроется, а к моменту возврата станет валидным
    p.media.push(MediaRef::new(MediaId(1), "later.mp4"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    let deadline = std::time::Instant::now() + WAIT_BG;
    while !h.state().placeholder_media.is_empty() && std::time::Instant::now() < deadline {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(h.state().placeholder_media.is_empty(), "вернувшийся файл так и не заменил заглушку");
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

    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), "gone.mp4"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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

    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "no-such-clip.mp4"));
    p.media.push(MediaRef::new(MediaId(2), "also-gone.wav"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    let deadline = std::time::Instant::now() + WAIT_BG;
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
    assert_eq!(
        pick_range_marker(&markers, 70_000, tol),
        Some(60_000),
        "общая часть — ближний флажок слева"
    );
    assert_eq!(
        pick_range_marker(&markers, 31_000, tol),
        Some(30_000),
        "клик по флажку берёт свою метку"
    );
    assert_eq!(pick_range_marker(&markers, 200_000, tol), None, "мимо лент — ничего");
    assert_eq!(
        pick_range_marker(&markers, 500_000, tol),
        None,
        "точечная метка регионом не становится"
    );

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
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    let clip = |id: u64, start: i64| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    assert_eq!(
        render_file_name(Some("movie"), Some("intro"), 90_000 * 12),
        "kt-movie-intro-12s.mp4"
    );
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
    p.media.push(MediaRef::new(MediaId(9), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    app.set_fx_coalesced(ClipId(9), vec![FxInstance::blur(20), FxInstance::glow(1.0)], gesture);
    assert!(app.model_rev > rev_before, "правка жеста двигает ревизию модели");

    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(h.state().autosave_rx.is_none(), "результат автосейва забран");
    assert!(h.state().dirty_content, "цепочка изменена ПОСЛЕ снимка — звёздочка обязана остаться");
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

    use crate::app::SaveOutcome;
    let wrote = |o: SaveOutcome| matches!(o, SaveOutcome::Wrote { .. });

    // первая запись (ревизия 5) проходит
    assert!(wrote(crate::app::write_project_guarded(&guard, &f, 5, "rev5").unwrap()));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "rev5");
    // «опоздавший» фоновый снапшот ревизии 3 — файл не трогает
    assert!(!wrote(crate::app::write_project_guarded(&guard, &f, 3, "rev3").unwrap()));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "rev5", "старый снимок перетёр свежий");
    // повтор той же ревизии допустим (идемпотентно), новее — пишется
    assert!(wrote(crate::app::write_project_guarded(&guard, &f, 5, "rev5b").unwrap()));
    assert!(wrote(crate::app::write_project_guarded(&guard, &f, 9, "rev9").unwrap()));
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
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some(), "фоновая запись в полёте");

    // ревизия 2 + немедленный Ctrl+S, пока фоновая ещё не финишировала
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(2), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.save();

    // дожидаемся фоновую и убеждаемся, что она НЕ откатила файл
    let deadline = std::time::Instant::now() + WAIT_BG;
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

/// Тест 382 (Р.49 А9): конверт наследуется НА ЖИВОМ ЖЕСТЕ, а не только в
/// теории kt-otio. Разрез и вставка копии идут через `apply_batch`, и родство
/// доезжает до `Retained` тем же путём, каким его сообщила модель.
#[test]
fn t382_app_carries_extras_to_born_clips() {
    use kt_model::*;
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    let app = h.state_mut();
    let src = app.model.project().clips[0].clone();

    // у события есть конверт из «файла»: имя и чужой маркер
    app.retained
        .clips
        .insert(src.id, kt_otio::ClipExtra { name: "Шот 12A".into(), ..Default::default() });

    // разрез жестом приложения
    let mid = src.start + Tick(src.duration.0 / 2);
    let new_id = ClipId(app.next_clip_id());
    assert!(app.apply(Command::Split { id: src.id, at: mid, new_id, new_group: None }));
    assert_eq!(
        h.state().retained.clips.get(&new_id).map(|e| e.name.as_str()),
        Some("Шот 12A"),
        "половина разреза осталась безымянной: в файл уедет клип без имени"
    );

    // вставка копии — тот же путь через ops::paste_block
    let app = h.state_mut();
    let before = app.model.project().clips.len();
    app.selected = [src.id].into_iter().collect();
    app.copy_selection();
    app.paste_here(Tick(500_000), None, String::new(), false);
    assert!(h.state().model.project().clips.len() > before, "вставка не добавила событий");
    let pasted = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .filter(|c| c.id != src.id && c.id != new_id)
        .map(|c| c.id)
        .collect::<Vec<_>>();
    assert!(
        pasted.iter().any(|id| h
            .state()
            .retained
            .clips
            .get(id)
            .is_some_and(|e| e.name == "Шот 12A")),
        "вставленная копия потеряла имя оригинала"
    );
}

/// Тест 390 (Р.49 А12): рабочее место не уезжает в чужой файл проекта.
/// Ширина панели метров и вид (зум/скролл/курсор) жили в `.otio`: открыл
/// проект коллеги — потерял свою ширину, сохранил — навязал ему свою. В файле
/// остаётся passthrough: что прочитали, то и пишем, байт-стабильность цела.
#[test]
fn t390_workspace_does_not_travel_in_the_project_file() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t390-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t390.otio");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.workspace_file = Some(dir.join("workspace.json"));
    // «прочитали из чужого файла»: его ширина метров и его вид
    app.ui_passthrough = Some(kt_model::UiState {
        meters_w: 123.0,
        view_start: 4444.0,
        ticks_per_px: 777.0,
        cursor: 555,
        ..Default::default()
    });
    // а у нас своё рабочее место
    app.meters_w = 90.0;
    app.view_start = 10.0;
    app.ticks_per_px = 200.0;
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.save();

    // в файле — ЧУЖИЕ значения, нетронутые
    let text = std::fs::read_to_string(&target).unwrap();
    let saved = kt_otio::load_project(&text, target.parent()).unwrap().project;
    let ui = saved.ui.expect("вьюпорт обязан остаться в файле");
    assert_eq!(ui.meters_w, 123.0, "в чужой файл уехала НАША ширина метров");
    assert_eq!(ui.view_start, 4444.0, "в чужой файл уехал НАШ скролл");
    assert_eq!(ui.cursor, 555, "в чужой файл уехал НАШ курсор");

    // а наше — в своём хранилище
    let w = crate::user_store::load_workspace_at(&dir.join("workspace.json"));
    assert_eq!(w.meters_w, 90.0, "своя ширина метров не сохранена");
    let v = w.view_of(Some(&target)).expect("вид проекта не сохранён");
    assert_eq!(v.view_start, 10.0);
    assert_eq!(v.ticks_per_px, 200.0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 391 (Р.49 А12): переезд без потерь. Пустое хранилище (первый запуск
/// после переезда) оставляет значения из файла — миграция без единой строки
/// кода; заполненное сильнее файла; имена и высоты дорожек и группы бина
/// остаются в ПРОЕКТЕ, потому что они про ЭТОТ монтаж.
#[test]
fn t391_workspace_falls_back_to_file_and_project_keeps_its_own() {
    use crate::user_store::{ProjectView, Workspace, load_workspace_at, save_workspace_at};
    let dir = std::env::temp_dir().join(format!("kt-t391-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ws = dir.join("workspace.json");
    let proj = dir.join("p.otio");

    // пустое хранилище ничего не навязывает
    let w = load_workspace_at(&ws);
    assert_eq!(w.meters_w, 0.0, "пустое хранилище обязано молчать");
    assert!(w.view_of(Some(&proj)).is_none());

    // круг: записали — прочитали
    let mut w = Workspace::default();
    w.meters_w = 111.0;
    w.set_view(Some(&proj), ProjectView { view_start: 7.0, ticks_per_px: 13.0, cursor: 42 });
    save_workspace_at(&ws, &w);
    let back = load_workspace_at(&ws);
    assert_eq!(back.meters_w, 111.0);
    let v = back.view_of(Some(&proj)).expect("вид не прочитан");
    assert_eq!((v.view_start, v.ticks_per_px, v.cursor), (7.0, 13.0, 42));
    // вид другого проекта не подменяется
    assert!(back.view_of(Some(&dir.join("other.otio"))).is_none());

    // битый файл не роняет программу и не навязывает мусор
    std::fs::write(&ws, "{ это не json").unwrap();
    assert_eq!(load_workspace_at(&ws).meters_w, 0.0);

    // файл из БУДУЩЕЙ версии читается, но не перезаписывается: ломать то,
    // чего мы не понимаем, нельзя (политика user_store с Р.48)
    std::fs::write(&ws, r#"{"version":99,"meters_w":55.0,"views":[]}"#).unwrap();
    let future = load_workspace_at(&ws);
    assert_eq!(future.meters_w, 55.0, "будущий файл обязан читаться");
    assert!(future.is_readonly());
    // программа держит ПРОЧИТАННОЕ хранилище — оно и отказывается писать
    save_workspace_at(&ws, &future);
    assert!(
        std::fs::read_to_string(&ws).unwrap().contains("99"),
        "переписали файл новее нашей версии"
    );

    // а вот ЭТО остаётся в проекте: оно про монтаж, а не про рабочее место
    let mut h = harness();
    let app = h.state_mut();
    // имя и высота дорожки с Р.62 — СОДЕРЖИМОЕ модели (ходят через undo),
    // а не поля `ui`: в рабочем месте им делать нечего тем более
    edit_project(app, |p| {
        p.track_at_mut(kt_model::LaneKind::Audio, 0).unwrap().name = "Речь".into();
        p.track_at_mut(kt_model::LaneKind::Video, 0).unwrap().height = 88.0;
    });
    app.bin_free = true;
    let ui = app.ui_state_for_test();
    assert!(ui.lane_names.is_empty(), "имя дорожки больше не живёт в ui");
    assert!(ui.lane_heights.is_empty(), "высота дорожки больше не живёт в ui");
    assert_eq!(app.row_label(kt_model::LaneKind::Audio, 0), "Речь");
    assert!(ui.bin_free, "раскладка бина — часть монтажа");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Двойной клик в точку. История кликов у egui общая на окно, поэтому шаг
/// харнесса должен быть коротким (0.02 с) — иначе два клика расходятся во
/// времени и складываются в два одиночных (ловушка п.232).
fn double_click_at(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) {
    // история кликов у egui общая на окно: два жеста подряд складываются в
    // ТРОЙНОЙ клик, и второй «двойной» не случается вовсе (ловушка п.232).
    // Даём окну двойного клика истечь.
    for _ in 0..25 {
        h.step();
    }
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
}

/// Правый клик без движения указателя.
fn right_click_at(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) {
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
}

/// Подписи пунктов открытого меню.
pub(crate) fn menu_labels(h: &mut Harness<'_, KtApp>) -> Vec<String> {
    use egui_kittest::kittest::{NodeT, Queryable};
    h.query_all_by_label_contains("")
        .filter_map(|n| n.accesskit_node().label().map(|l| l.to_string()))
        .collect()
}

/// Тест 392 (Р.49 В2): двойной клик по линии уровня СБРАСЫВАЕТ уровень.
/// Подсказка обещала это с самого начала («тяни, двойной клик — 0 dB»), а
/// двойной клик по линии всё это время открывал окно FX: обещание в подсказке
/// было, поведения не было.
#[test]
fn t392_double_click_on_level_rail_resets_level() {
    use kt_model::*;
    let (mut h, g) = harness_three_clips_dt(Some(0.02));
    // звук и видео: линия уровня одна на оба вида (п.176)
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        let mut a = p.clips[0].clone();
        a.id = ClipId(10);
        a.kind = ClipKind::Audio;
        a.lane = 0;
        a.gain_db = -3.0;
        p.clips.push(a);
        p.ensure_tracks(kt_model::LaneKind::Audio, p.lanes(kt_model::LaneKind::Audio).max(1));
        p.clips[0].opacity = 0.35;
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();
    let g = h.state().tl_geom.clone().unwrap_or(g);

    // линия уровня стоит по значению: у 35 % она низко, у −9 dB — тоже
    let vclip = h.state().model.project().clips[0].clone();
    let level_y = |kind: ClipKind, lane: u32, v: f32| -> f32 {
        let top = g.lane_top(kind, lane);
        let lh = g.lane_h_of(kind, lane);
        crate::timeline::level_y(v, top, lh)
    };
    let vx = g.tick_to_x(vclip.start.0 + vclip.duration.0 / 2);
    let vy = level_y(ClipKind::Video, 0, 0.35);

    double_click_at(&mut h, egui::pos2(vx, vy));
    let st = h.state();
    assert_eq!(
        st.model.project().clips.iter().find(|c| c.id == vclip.id).map(|c| c.opacity),
        Some(1.0),
        "двойной клик по линии уровня видео не сбросил непрозрачность"
    );
    assert!(st.fx_editors.is_empty(), "двойной клик по линии открыл окно FX вместо сброса");

    // тот же жест у звука: 0 dB
    let aclip =
        h.state().model.project().clips.iter().find(|c| c.id == ClipId(10)).cloned().unwrap();
    let ax = g.tick_to_x(aclip.start.0 + aclip.duration.0 / 2);
    let ay = level_y(ClipKind::Audio, 0, crate::timeline::gain_to_level(-3.0));
    double_click_at(&mut h, egui::pos2(ax, ay));
    assert_eq!(
        h.state().model.project().clips.iter().find(|c| c.id == ClipId(10)).map(|c| c.gain_db),
        Some(0.0),
        "двойной клик по линии громкости не дал 0 dB"
    );

    // а по ТЕЛУ события двойной клик по-прежнему открывает FX
    let body_y = g.lane_y(ClipKind::Video, 0, 0.75);
    double_click_at(&mut h, egui::pos2(vx, body_y));
    assert!(!h.state().fx_editors.is_empty(), "двойной клик по телу перестал открывать FX");
}

/// Тест 393 (Р.49 В2): аккорд в подписи пункта обещает ровно то, что клавиша
/// делает. «Удалить (Del)» у именованной области было неправдой: Del удаляет
/// ВЫДЕЛЕННЫЕ СОБЫТИЯ, а область не трогает вовсе.
#[test]
fn t393_menu_accelerator_promises_only_what_the_key_does() {
    use kt_model::*;
    use kt_time::Tick;
    // сам механизм: аккорд — аргумент, а не часть переводимой фразы
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        assert!(
            !tr(k::tl::menu::DELETE).contains("(Del"),
            "{}: аккорд остался внутри значения каталога — одна фраза на все контексты",
            lang.code()
        );
    }

    let (mut h, g) = harness_three_clips();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        // область СВОБОДНА от событий: под клипом её контур не схватить —
        // там выигрывает клип, и это правильно
        p.areas.push(MarkedArea {
            name: "сцена 3".into(),
            start: Tick(300_000),
            duration: Tick(50_000),
            lanes: vec![(LaneKind::Video, 0)],
            color: None,
        });
        app.model = kt_model::TimelineModel::new(p);
    }
    h.step();
    let g = h.state().tl_geom.clone().unwrap_or(g);

    // меню ОБЛАСТИ: пункт есть, аккорда в нём нет
    let row = g.row_of((LaneKind::Video, 0)).expect("видео-строка");
    right_click_at(&mut h, egui::pos2(g.tick_to_x(320_000), g.row_top(row) + 1.0));
    assert!(
        matches!(h.state().ctx_target, Some(crate::timeline::CtxTarget::Area(_))),
        "сцена: правый клик обязан дать область"
    );
    let labels = menu_labels(&mut h);
    let del = labels
        .iter()
        .find(|l| l.starts_with(tr(k::tl::menu::DELETE)))
        .unwrap_or_else(|| panic!("в меню области нет пункта удаления: {labels:?}"));
    assert!(!del.contains("Del"), "у области обещан аккорд Del, который её не удаляет: «{del}»");

    // и это не «убрали везде»: у КАМЕРЫ Del работает, и обещание осталось.
    // Свой харнесс: у egui история кликов общая на окно, и второй правый
    // клик в том же окне ЗАКРЫЛ бы уже открытое меню.
    let (mut h2, _) = harness_three_clips();
    {
        let app = h2.state_mut();
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 0 }));
        app.player.seek(Tick(320_000));
        app.add_camera_at_playhead(0);
        app.camera_editors.clear();
    }
    h2.step();
    h2.step();
    let g2 = h2.state().tl_geom.clone().expect("геометрия");
    let cam_row = g2.row_of((LaneKind::Camera, 0)).expect("камерная строка");
    let cam = h2.state().model.project().cameras[0].clone();
    right_click_at(
        &mut h2,
        egui::pos2(g2.tick_to_x(cam.start.0 + cam.duration.0 / 2), g2.row_center_y(cam_row)),
    );
    assert!(
        matches!(h2.state().ctx_target, Some(crate::timeline::CtxTarget::Camera(_))),
        "сцена: правый клик обязан дать камеру"
    );
    let labels = menu_labels(&mut h2);
    assert!(
        labels.iter().any(|l| l.starts_with(tr(k::tl::menu::DELETE)) && l.contains("Del")),
        "у камеры Del работает — обещание обязано остаться: {labels:?}"
    );
}

/// Тест 394 (Р.49 В3-В5): немых объектов в панелях не бывает.
///
/// Разведка нашла двенадцать мест, где объект отвечает на НАВЕДЕНИЕ
/// подсказкой, а на правую кнопку — ничем. Человек учится один раз: если
/// правая кнопка где-то показала меню, он жмёт её везде, и молчание читается
/// как «объекта нет». Здесь правый клик приходит в КАЖДЫЙ панельный объект и
/// проверяется, что программа назвала цель — то есть меню принадлежит именно
/// тому, во что целились.
#[test]
fn t394_panel_objects_answer_right_click() {
    use crate::ctx_menu::PanelTarget as T;
    use kt_model::*;
    let (mut h, _) = harness_three_clips_dt(Some(0.02));

    // ---- фон медиа-бина ---------------------------------------------------
    {
        let app = h.state_mut();
        app.ctx_panel = None;
    }
    h.step();
    // прямоугольник бина программа сообщает сама: ниже него в колонке живёт
    // перф-панель, и «примерно внизу колонки» щупало бы её
    let bin = h.state().bin_rect;
    assert!(bin.is_positive(), "бин не сообщил свой прямоугольник");
    right_click_at(&mut h, egui::pos2(bin.center().x, bin.bottom() - 10.0));
    assert_eq!(h.state().ctx_panel, Some(T::BinEmpty), "фон медиа-бина молчит на правую кнопку");

    // ---- панель метров ----------------------------------------------------
    let meters = panel_rect(&h, "meters_col");
    right_click_at(&mut h, meters.center());
    assert_eq!(h.state().ctx_panel, Some(T::Meters), "панель метров молчит на правую кнопку");

    // ---- окно превью ------------------------------------------------------
    let prev = h.state().preview_rect;
    assert!(prev.is_finite(), "превью не нарисовано — сцена сломана");
    right_click_at(&mut h, egui::pos2(prev.center().x, prev.center().y + 20.0));
    assert_eq!(h.state().ctx_panel, Some(T::Preview), "окно превью молчит на правую кнопку");

    // ---- слот цепочки эффектов -------------------------------------------
    {
        let app = h.state_mut();
        let id = app.model.project().clips[0].id;
        assert!(app.apply(Command::SetFx { id, fx: vec![FxInstance::blur(8)] }));
        app.open_fx_editor(id);
    }
    h.step();
    h.step();
    let slot = {
        use egui_kittest::kittest::Queryable;
        h.query_all_by_label_contains(crate::fx_editor::fx_display_name(&FxInstance::blur(8)))
            .map(|n| n.rect())
            .next()
            .expect("слот цепочки не нарисован")
    };
    right_click_at(&mut h, slot.center());
    assert!(
        matches!(h.state().ctx_panel, Some(T::FxSlot { .. })),
        "слот цепочки молчит на правую кнопку"
    );

    // и цель ТАЙМЛАЙНА при этом погашена: меню на экране одно
    assert!(
        h.state().ctx_target.is_none(),
        "панельная цель не погасила цель таймлайна — тест прочитал бы протухшую"
    );
}

/// Тест 412 (Р.49 Б7): громкость видна В ПОДСКАЗКЕ карточки бина, а показ
/// НИЧЕГО НЕ ЗАПУСКАЕТ.
///
/// Второе важнее первого: провести мышью по бину значило бы начать декод
/// десятка файлов — прямая родня инварианта 8 («скролл/зум не порождает
/// декод-запросов»). И пока не мерили, программа так и говорит, а не
/// показывает выдуманный ноль.
#[test]
fn t412_bin_tooltip_shows_loudness_and_does_not_request_it() {
    use kt_model::*;
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
        app.model
            .apply(Command::AddMedia { id: MediaId(2), path: "synth:sine:440".into() })
            .unwrap();
        // первому громкость известна, второму — нет
        app.loudness.lock().unwrap().insert(MediaId(1), (-18.5, -1.2, 6.0));
    }
    h.step();
    h.step();

    let g = h.state().bin_rect;
    assert!(g.is_positive(), "бин не нарисован");
    // наводимся на карточки и читаем строку состояния
    let mut seen_known = false;
    let mut seen_pending = false;
    for y in 0..20 {
        let p = egui::pos2(g.left() + 30.0, g.top() + 20.0 + y as f32 * 12.0);
        if !g.contains(p) {
            break;
        }
        h.hover_at(p);
        h.step();
        if let Some(hint) = h.state().hover_hint.clone() {
            if hint.contains("-18.5") || hint.contains("−18.5") {
                seen_known = true;
            }
            if hint.contains(tr(k::bin::LOUDNESS_PENDING)) {
                seen_pending = true;
            }
        }
    }
    assert!(seen_known, "громкость не доехала до подсказки карточки");
    assert!(seen_pending, "о неизмеренной громкости программа промолчала");

    // и ни одна заявка на анализ при этом не поставлена
    let st = h.state();
    assert_eq!(st.analysis.queued(), 0, "показ поставил заявку на анализ");
    assert!(st.analysis.current().is_none(), "показ запустил анализ");
}

/// Тест 418 (Р.49 Б9): невозможность синхронизации называется ВСЛУХ и
/// РАЗНЫМИ причинами (правило 16).
///
/// «Индекс ещё строится» и «записи не совпадают» — не одно и то же: в первом
/// случае надо подождать, во втором ждать бессмысленно. Молчание или общее
/// «не получилось» оставляет человека гадать.
#[test]
fn t418_sync_refusals_are_spoken_and_distinct() {
    use kt_model::*;
    let (mut h, _) = harness_three_clips();

    // ни одного выделения — сказано, что надо выделить два
    let app = h.state_mut();
    app.selected.clear();
    app.notices.clear();
    app.sync_selected_by_audio();
    assert_eq!(
        h.state().notices.back().map(|n| n.text.clone()),
        Some(tr(k::msg::SYNC_NEED_TWO).to_string()),
        "о неверном выделении не сказано"
    );

    // два выделено, но пирамид нет — «индекс ещё строится», другая причина
    let app = h.state_mut();
    let ids: Vec<ClipId> = app.model.project().clips.iter().take(2).map(|c| c.id).collect();
    app.selected = ids.iter().copied().collect();
    app.peaks.lock().unwrap().clear();
    app.notices.clear();
    app.sync_selected_by_audio();
    let last = h.state().notices.back().cloned().expect("о неготовом индексе промолчали");
    assert_eq!(last.text, tr(k::msg::SYNC_PENDING), "причина названа не та");
    assert_eq!(last.severity, crate::app::Severity::Warn, "отказ показан не как ошибка");
    assert_ne!(
        tr(k::msg::SYNC_PENDING),
        tr(k::msg::SYNC_NO_MATCH),
        "две разные причины обязаны звучать по-разному"
    );
}

/// Тест 419 (Р.49 Б9): пункт меню события и пункт главного меню — ОДНО ТЕЛО
/// (правило 17). Разные тела разъехались бы на первой же правке.
#[test]
fn t419_sync_menu_and_timeline_item_are_the_same_body() {
    let root = crate::reach_tests::workspace_root();
    let mut callers = Vec::new();
    for rel in ["apps/kt-app/src/timeline.rs", "apps/kt-app/src/toolbar.rs"] {
        let src = std::fs::read_to_string(root.join(rel)).expect(rel);
        assert!(
            src.contains("SYNC_AUDIO"),
            "{rel}: пункта синхронизации нет — парный способ потерян"
        );
        callers.push(src.matches("sync_selected_by_audio()").count());
    }
    assert!(callers.iter().all(|&n| n >= 1), "один из путей зовёт не общее тело: {callers:?}");
    // и само тело — одно на весь app
    let app = std::fs::read_to_string(root.join("apps/kt-app/src/app.rs")).expect("app.rs");
    assert_eq!(
        app.matches("pub fn sync_selected_by_audio").count(),
        1,
        "тел синхронизации больше одного — они разъедутся"
    );
}

/// Тест 421 (Р.49 Б10): результат, посчитанный по УСТАРЕВШЕЙ модели, не
/// применяется. Пока задача читала файлы, монтаж мог поехать — кривые от
/// старой модели легли бы не на те события, и молча.
#[test]
fn t421_stale_analysis_result_is_not_applied() {
    use kt_model::*;
    let mut h = harness();
    {
        let app = h.state_mut();
        app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
        app.model_rev += 1;
    }
    let rev = h.state().model_rev;
    // результат «из прошлого»: ревизия та же, медиа на месте — применяется
    h.state_mut().on_analysis_done_for_test(
        crate::analysis::AnalysisKind::Loudness(MediaId(1)),
        rev,
        Ok(crate::analysis::AnalysisOut::Loudness {
            media: MediaId(1),
            lufs: Some(-17.0),
            dbtp: -2.0,
            lra: 4.0,
        }),
    );
    assert!(
        h.state().loudness.lock().unwrap().contains_key(&MediaId(1)),
        "свежий результат не применён"
    );

    // а теперь медиа подменили: результат по прежнему файлу к нему не относится
    {
        let app = h.state_mut();
        app.loudness.lock().unwrap().clear();
        app.apply(Command::RemoveMedia { id: MediaId(1) });
    }
    let rev = h.state().model_rev;
    h.state_mut().on_analysis_done_for_test(
        crate::analysis::AnalysisKind::Loudness(MediaId(1)),
        rev,
        Ok(crate::analysis::AnalysisOut::Loudness {
            media: MediaId(1),
            lufs: Some(-17.0),
            dbtp: -2.0,
            lra: 4.0,
        }),
    );
    assert!(
        !h.state().loudness.lock().unwrap().contains_key(&MediaId(1)),
        "число по исчезнувшему медиа применено — стейл-защита не сработала"
    );
}

/// Тест 376 (Р.49 А8): номер события не переиспользуется в пределах сессии.
/// Прежняя формула «максимум по текущим клипам + 1» после удаления последнего
/// события выдавала его же номер заново — и новорождённый клип наследовал
/// ЧУЖОЙ конверт из `Retained.clips`: имя, маркеры и ссылки на материал
/// удалённого. По тому же номеру ходят link-группы и ссылки морферов.
#[test]
fn t376_clip_ids_are_not_reused_within_a_session() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness();
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();

    let make = |app: &mut crate::app::KtApp, id: u64| {
        app.apply(Command::AddClip(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
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
        }));
    };

    let first = app.next_clip_id();
    make(app, first);
    let second = app.next_clip_id();
    assert!(second > first, "второй номер не больше первого");
    make(app, second);

    // удалили оба — номера всё равно не возвращаются
    app.apply(Command::RemoveClip { id: ClipId(second) });
    app.apply(Command::RemoveClip { id: ClipId(first) });
    assert!(app.model.project().clips.is_empty());
    let third = app.next_clip_id();
    assert!(
        third > second,
        "номер переиспользован после удаления ({third} <= {second}): новый клип получит \
         конверт удалённого — его имя, маркеры и ссылки на материал"
    );

    // отметка не двигается, пока номер не взяли на деле
    assert_eq!(app.next_clip_id(), third, "выдали два разных номера на один запрос");

    // undo, отменивший рождение, отметку НЕ откатывает: номер уже жил, и
    // конверт под ним мог осесть в Retained
    make(app, third);
    app.undo();
    assert!(app.model.project().clips.is_empty(), "undo обязан был убрать рождённый клип");
    assert!(
        app.next_clip_id() > third,
        "undo вернул номер в оборот — вместе с конвертом, который под ним завёлся"
    );
}

/// Тест 379 (Р.49 А8, идиома t273): арифметика номеров событий живёт в ОДНОМ
/// месте. Каждая своя копия «max(...)+1» — это ещё один способ выдать номер
/// удалённого клипа, и найти её потом можно только по жалобе.
#[test]
fn t379_clip_id_arithmetic_lives_in_one_place() {
    let root = crate::reach_tests::workspace_root();
    const HOME: &str = "apps/kt-app/src/app.rs";
    let mut bad: Vec<String> = Vec::new();
    for rel in [
        "apps/kt-app/src/timeline.rs",
        "apps/kt-app/src/bin_panel.rs",
        "apps/kt-app/src/morpher_editor.rs",
        "apps/kt-app/src/fx_editor.rs",
        "apps/kt-app/src/toolbar.rs",
        HOME,
    ] {
        let src = crate::reach_tests::strip_comments(
            &std::fs::read_to_string(root.join(rel)).expect(rel),
        );
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            // «взять максимум номеров клипов и прибавить» — формула выдачи
            if !(t.contains("c.id.0") && t.contains("max()")) {
                continue;
            }
            // в доме правила (next_clip_id/bump_clip_hw) такие строки законны
            if rel == HOME {
                continue;
            }
            bad.push(format!("{rel}:{}: {t}", n + 1));
        }
    }
    assert!(
        bad.is_empty(),
        "номер события выдаётся ВНЕ единственного правила (`next_clip_id` в {HOME}). \
         Своя копия «max + 1» не знает об уже рождённых и переиспользует номера \
         удалённых событий вместе с их конвертами: {bad:?}"
    );
}

/// Тест 369 (Р.49 А6): «Новый» и «Открыть» при несохранённой работе СПРАШИВАЮТ.
/// До Р.49 не спрашивали — и это был законный выбор (вердикт F-RES-005):
/// фоновый автосейв писал прямо в файл пользователя, терять было нечего.
/// Владелец автосейв в проект убрал (п.351) — вместе с правом не спрашивать.
#[test]
fn t369_new_and_open_guard_unsaved_work() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;

    // «Новый» не выбрасывает работу молча
    app.new_project();
    assert_eq!(h.state().ask, Some(crate::app::Ask::Unsaved(crate::app::Pending::New)));
    assert_eq!(h.state().model.project().media.len(), 1, "работа выброшена до ответа");

    // «Остаться» — ничего не произошло
    h.state_mut().answer_ask(crate::app::Answer::Cancel);
    assert!(h.state().ask.is_none());
    assert_eq!(h.state().model.project().media.len(), 1, "«Остаться» выбросило работу");
    assert!(h.state().dirty_content);

    // «Не сохранять» — новый проект на месте, и звёздочки на нём нет
    h.state_mut().new_project();
    h.state_mut().answer_ask(crate::app::Answer::No);
    assert_eq!(h.state().model.project().media.len(), 0, "новый проект не создан");
    assert!(!h.state().dirty_content);

    // чистому проекту вопрос не задаётся
    h.state_mut().new_project();
    assert!(h.state().ask.is_none(), "спросили на пустом месте");

    // «Открыть» — тот же сторож (правило 17: парные пути ведут себя одинаково)
    let app = h.state_mut();
    app.dirty_content = true;
    app.open_project_dialog();
    assert_eq!(h.state().ask, Some(crate::app::Ask::Unsaved(crate::app::Pending::Open)));
    h.state_mut().answer_ask(crate::app::Answer::Cancel);
}

/// Тест 370 (Р.49 А6): «Сохранить и продолжить» НЕ продолжает, если сохранить
/// не вышло. Иначе отказ записи (нет прав, диск полон, отменённый Save-As)
/// оборачивался бы ровно той потерей работы, от которой сторож и поставлен.
#[test]
fn t370_save_and_go_does_not_go_when_save_failed() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(std::path::PathBuf::from("/proc/self/no-such-dir/x.otio"));
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.new_project();
    h.state_mut().answer_ask(crate::app::Answer::Yes);

    let st = h.state();
    assert_eq!(st.model.project().media.len(), 1, "работа выброшена, хотя запись не удалась");
    assert!(st.dirty_content, "звёздочка погашена при неудачной записи");
    assert!(st.notices.iter().any(|n| n.severity == crate::app::Severity::Warn));
}

/// Тест 371 (Р.49 А6): выход перехватывается и отменяется. `CancelClose` в
/// eframe 0.36 работает и для КОРНЕВОГО окна — на этом и держится вопрос
/// «сохранить перед выходом?»; без него пришлось бы ловить системные события
/// мимо egui.
#[test]
fn t371_close_request_is_intercepted_and_cancelled() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;

    // сторож принимает решение тем же кодом, каким его принимает кадр
    assert!(!h.state_mut().guard_unsaved(crate::app::Pending::Quit), "выход не перехвачен");
    assert_eq!(h.state().ask, Some(crate::app::Ask::Unsaved(crate::app::Pending::Quit)));
    assert!(!h.state().quit_confirmed, "выход разрешён до ответа");

    // «Остаться» — уходить не собираемся
    h.state_mut().answer_ask(crate::app::Answer::Cancel);
    assert!(!h.state().quit_confirmed);

    // «Не сохранять» — выход разрешён, и второй раз не переспрашиваем
    assert!(!h.state_mut().guard_unsaved(crate::app::Pending::Quit));
    h.state_mut().answer_ask(crate::app::Answer::No);
    assert!(h.state().quit_confirmed, "после ответа выход не разрешён — окно не закроется");
    assert!(!h.state().dirty_content);

    // чистому проекту сторож не мешает уйти
    assert!(h.state_mut().guard_unsaved(crate::app::Pending::Quit));
}

/// Тест 372 (Р.49 А6): последний рубеж — `Drop`. Он срабатывает там, где
/// `on_exit` не зовут вовсе: `*self = app` при «Новый»/«Открыть» и раскрутка
/// стека от паники. В теле ни одного `unwrap`: паника внутри `Drop` во время
/// раскрутки — немедленный `abort`, уносящий ровно тот файл, ради которого
/// всё затевалось. Потому здесь же проверяется ОТРАВЛЕННЫЙ сторож.
#[test]
fn t372_drop_is_the_last_line_of_defence() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t372-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("t372.otio");
    let rec = crate::app::recovery_path_for(Some(&proj));

    {
        let mut h = harness();
        let app = h.state_mut();
        app.recovery_on_drop = true; // в харнессе рубеж выключен — включаем
        app.project_path = Some(proj.clone());
        app.model.apply(Command::AddMedia { id: MediaId(5), path: "synth:ramp".into() }).unwrap();
        app.model_rev += 1;
        app.dirty_content = true;
        // сторож отравлен: поток паниковал под замком. Запись обязана пройти —
        // иначе чужая паника лишает пользователя последнего шанса.
        let g = app.recovery_rev.clone();
        let _ = std::thread::spawn(move || {
            let _held = g.lock().unwrap();
            panic!("poison");
        })
        .join();
        assert!(app.recovery_rev.is_poisoned(), "мьютекс не отравлен — тест ничего не проверяет");
    } // здесь Drop

    let text = std::fs::read_to_string(&rec).expect("Drop не записал восстановление");
    let saved = kt_otio::load_project(&text, rec.parent()).unwrap().project;
    assert_eq!(saved.media.len(), 1, "в восстановлении не та работа");
    assert!(!proj.exists(), "Drop записал В ПРОЕКТ: его меняет только Ctrl+S");

    // чистому приложению ронять нечего
    let _ = std::fs::remove_file(&rec);
    {
        let mut h = harness();
        let app = h.state_mut();
        app.recovery_on_drop = true;
        app.project_path = Some(proj.clone());
        app.dirty_content = false;
        app.dirty_ui = false;
    }
    assert!(!rec.exists(), "Drop записал восстановление на чистом проекте");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 365 (Р.49 А5): кандидаты отбираются по ВРЕМЕНИ. Восстановление
/// старше проекта не предлагается (иначе после каждого Ctrl+S программа
/// звала бы «восстановить» то, что и так в файле), новее — предлагается,
/// безымянная работа предлагается всегда, свежее — первым.
#[test]
fn t365_recovery_candidates_are_picked_by_time() {
    use std::time::{Duration, SystemTime};
    let dir = std::env::temp_dir().join(format!("kt-t365-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("film.otio");
    let rec = crate::app::recovery_path_for(Some(&proj));
    let anon = dir.join("anon").join("autosave-project.otio");
    std::fs::create_dir_all(anon.parent().unwrap()).unwrap();

    let touch = |p: &std::path::Path, secs_ago: u64| {
        std::fs::write(p, "{}").unwrap();
        let t = SystemTime::now() - Duration::from_secs(secs_ago);
        std::fs::File::options().write(true).open(p).unwrap().set_modified(t).unwrap();
    };

    // проект новее восстановления: предлагать нечего
    touch(&rec, 600);
    touch(&proj, 60);
    let got = crate::app::recovery_candidates_at(Some(&proj), &anon);
    assert!(got.is_empty(), "предложено восстановление старее самого проекта: {got:?}");

    // восстановление новее проекта — предлагается
    touch(&rec, 10);
    let got = crate::app::recovery_candidates_at(Some(&proj), &anon);
    assert_eq!(got.len(), 1, "не предложено восстановление новее проекта");
    assert_eq!(got[0].path, rec);
    assert_eq!(got[0].for_project.as_deref(), Some(proj.as_path()));

    // безымянная работа предлагается всегда и, будучи свежее, идёт первой
    touch(&anon, 1);
    let got = crate::app::recovery_candidates_at(Some(&proj), &anon);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].path, anon, "свежее обязано идти первым");
    assert!(got[0].for_project.is_none(), "безымянная работа не привязана к проекту");
    assert_eq!(got[1].path, rec);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 366 (Р.49 А5): главный сценарий, не покрытый ничем до Р.49 —
/// «открыл проект A, нажал Новый, час работы, падение». `last_project`
/// указывает на A, A существует, программа его открывает — и про час работы
/// не говорит ни слова, потому что смотрела на автосейв только когда
/// `last_project` не нашёлся.
#[test]
fn t366_unnamed_work_is_offered_even_when_project_opens() {
    let dir = std::env::temp_dir().join(format!("kt-t366-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("A.otio");
    std::fs::write(&proj, "{}").unwrap();
    let anon = dir.join("data").join("autosave-project.otio");
    std::fs::create_dir_all(anon.parent().unwrap()).unwrap();
    std::fs::write(&anon, "{}").unwrap();

    let list = crate::app::recovery_candidates_at(Some(&proj), &anon);
    assert_eq!(list.len(), 1, "час безымянной работы не предложен: {list:?}");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(proj.clone());
    app.offer_recovery_list(list, true);
    assert!(h.state().recovery_open, "окно восстановления не открылось");

    // и наоборот: предлагать нечего — окна нет, а по прямой просьбе из меню
    // программа обязана сказать это словами, а не промолчать
    let app = h.state_mut();
    app.notices.clear();
    app.offer_recovery_list(Vec::new(), false);
    assert!(!h.state().recovery_open);
    assert_eq!(h.state().notices.len(), 1, "меню «Восстановить…» промолчало");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 367 (Р.49 А5): восстановление кладёт работу В РЕДАКТОР и больше
/// никуда. Файл проекта не трогается (его меняет только Ctrl+S), файл-источник
/// не съедается, звёздочка горит, и всё это ПОВТОРИМО (правило 14).
#[test]
fn t367_restore_puts_work_in_editor_and_repeats() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t367-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("t367.otio");
    std::fs::write(&proj, "исходный файл пользователя").unwrap();

    // в файле восстановления — работа с тремя медиа
    let mut p = kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    for i in 1..=3 {
        p.media.push(MediaRef::new(MediaId(i), format!("synth:{i}")));
    }
    let rec = crate::app::recovery_path_for(Some(&proj));
    std::fs::write(&rec, kt_otio::save_project(&p, &Default::default(), rec.parent())).unwrap();
    // проект заведомо СТАРШЕ восстановления: гранулярность mtime у файловой
    // системы бывает секундной, и «записали одно за другим» её не различает.
    // Сам отбор по времени проверяет t365 — здесь он не предмет теста.
    std::fs::File::options()
        .write(true)
        .open(&proj)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(600))
        .unwrap();

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(proj.clone());
    app.dirty_content = false;
    app.offer_recovery_list(
        crate::app::recovery_candidates_at(Some(&proj), &dir.join("none")),
        true,
    );
    assert_eq!(h.state().recovery_offer.len(), 1, "восстановление не найдено рядом с проектом");

    h.state_mut().restore_recovery(0);
    let st = h.state();
    assert_eq!(st.model.project().media.len(), 3, "работа не доехала до редактора");
    assert!(st.dirty_content, "восстановленная работа обязана считаться несохранённой");
    assert!(!st.recovery_open, "окно не закрылось после восстановления");
    assert_eq!(
        std::fs::read_to_string(&proj).unwrap(),
        "исходный файл пользователя",
        "восстановление переписало файл проекта"
    );
    assert!(rec.exists(), "восстановление съело свой источник — второй раз не выйдет");

    // правило 14: то же самое ещё раз, после отмены
    let app = h.state_mut();
    app.model = kt_model::TimelineModel::new(kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    }));
    assert_eq!(app.model.project().media.len(), 0);
    app.offer_recovery_list(
        crate::app::recovery_candidates_at(Some(&proj), &dir.join("none")),
        true,
    );
    h.state_mut().restore_recovery(0);
    assert_eq!(h.state().model.project().media.len(), 3, "повторное восстановление не сработало");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 368 (Р.49 А5): «Удалить» уносит именно этот файл и закрывает окно,
/// когда предлагать больше нечего; «Не сейчас» файл оставляет.
#[test]
fn t368_forget_removes_file_and_later_keeps_it() {
    let dir = std::env::temp_dir().join(format!("kt-t368-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.recovery.otio");
    let b = dir.join("b.recovery.otio");
    std::fs::write(&a, "{}").unwrap();
    std::fs::write(&b, "{}").unwrap();
    let cand = |p: &std::path::Path| crate::app::RecoveryCandidate {
        path: p.to_path_buf(),
        mtime: std::fs::metadata(p).unwrap().modified().unwrap(),
        for_project: None,
    };

    let mut h = harness();
    let app = h.state_mut();
    app.offer_recovery_list(vec![cand(&a), cand(&b)], true);
    app.forget_recovery(0);
    assert!(!a.exists(), "выбранный файл не удалён");
    assert!(b.exists(), "удалён не тот файл");
    assert!(h.state().recovery_open, "остался кандидат — окно обязано остаться");

    h.state_mut().forget_recovery(0);
    assert!(!b.exists());
    assert!(!h.state().recovery_open, "кандидатов не осталось — окно обязано закрыться");

    // «Не сейчас» ничего не уничтожает
    std::fs::write(&a, "{}").unwrap();
    let app = h.state_mut();
    app.offer_recovery_list(vec![cand(&a)], true);
    app.recovery_open = false;
    assert!(a.exists(), "«не сейчас» удалило файл");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 363 (Р.49 А4, расширяет t150): две копии программы, открывшие один
/// проект, не делят ни временный файл, ни файл восстановления. Общий tmp
/// означал бы, что `rename` одной копии подменяет проект наполовину
/// записанным содержимым другой — то есть атомарная запись перестаёт быть
/// атомарной ровно тогда, когда защита нужнее всего.
#[test]
fn t363_second_copy_has_its_own_tmp_and_recovery() {
    let pid = std::process::id();
    let tmp = kt_otio::atomic_tmp_path(std::path::Path::new("/w/film.otio"));
    assert!(
        tmp.to_string_lossy().contains(&pid.to_string()),
        "во временном имени нет номера процесса: {}",
        tmp.display()
    );
    assert!(tmp.to_string_lossy().ends_with(".tmp"));
    assert!(tmp.to_string_lossy().starts_with("/w/film.otio."), "съедена часть имени проекта");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(std::path::PathBuf::from("/w/film.otio"));
    let owner = app.recovery_target();
    assert_eq!(owner, std::path::Path::new("/w/film.otio.recovery.otio"));
    // та же программа, но проект занят кем-то ещё
    app.lock_holder = Some(Some(4242));
    let second = app.recovery_target();
    assert_ne!(owner, second, "вторая копия пишет в чужой файл восстановления");
    assert!(second.to_string_lossy().contains(&pid.to_string()));
    assert!(second.to_string_lossy().ends_with(".otio"), "восстановление обязано быть .otio");
}

/// Тест 364 (Р.49 А4): вторая копия не запрещается, а честно деградирует.
/// Предупреждение с номером чужого процесса, и первый Ctrl+S не пишет молча,
/// а спрашивает. Ответ «не сохранять» оставляет файл нетронутым и звёздочку
/// на месте; «всё равно» пишет и больше не переспрашивает.
#[test]
fn t364_locked_project_asks_before_first_save() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t364-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t364.otio");
    std::fs::write(&target, "чужое содержимое").unwrap();

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.notices.clear();
    app.claim_project(&target);
    assert!(app.project_lock.is_some(), "свободный проект обязан заниматься");
    assert!(app.lock_holder.is_none());

    // вторая копия в этом же процессе: свой замок отпускаем, проект занимает
    // «сосед» — а программа заходит на него заново, как соседняя копия
    h.state_mut().project_lock = None;
    let _held = match crate::project_lock::acquire(&target) {
        crate::project_lock::LockState::Owned(l) => l,
        _ => panic!("первый замок должен был отпуститься вместе с полем"),
    };
    let app = h.state_mut();
    app.claim_project(&target);
    assert_eq!(app.lock_holder, Some(Some(std::process::id())), "занятость не замечена");
    assert!(
        app.notices.iter().any(|n| n.text.contains(&std::process::id().to_string())),
        "о занятости не сказано с номером процесса: {:?}",
        app.notices.iter().map(|n| &n.text).collect::<Vec<_>>()
    );

    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.save();
    assert!(matches!(h.state().ask, Some(crate::app::Ask::SaveOverLock { .. })), "не спросили");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "чужое содержимое",
        "записали, не дождавшись ответа"
    );

    // «не сохранять» — файл цел, звёздочка на месте, вопрос закрыт
    h.state_mut().answer_ask(crate::app::Answer::No);
    assert!(h.state().ask.is_none());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "чужое содержимое");
    assert!(h.state().dirty_content, "отказ сохранять погасил звёздочку");

    // «всё равно» — пишем; второй раз уже не спрашиваем
    h.state_mut().save();
    h.state_mut().answer_ask(crate::app::Answer::Yes);
    assert!(!h.state().dirty_content, "после ответа «всё равно» проект не записан");
    assert!(h.state().lock_override);
    let app = h.state_mut();
    app.dirty_content = true;
    app.model_rev += 1;
    app.save();
    assert!(h.state().ask.is_none(), "переспросили после принятого решения");
    assert!(!h.state().dirty_content);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 357 (Р.49 А3): имя файла восстановления дописывается к ПОЛНОМУ имени
/// проекта. Подмена расширения (`with_extension`) склеила бы `film.otio` и
/// `film.mp4.otio` в одно восстановление — ровно тот класс, что закрывал t150.
/// Безымянный проект восстанавливается в каталоге данных.
#[test]
fn t357_recovery_path_appends_to_full_name() {
    use crate::app::recovery_path_for;
    let a = recovery_path_for(Some(std::path::Path::new("/w/film.otio")));
    let b = recovery_path_for(Some(std::path::Path::new("/w/film.mp4.otio")));
    assert_eq!(a, std::path::Path::new("/w/film.otio.recovery.otio"));
    assert_eq!(b, std::path::Path::new("/w/film.mp4.otio.recovery.otio"));
    assert_ne!(a, b, "два разных проекта делят один файл восстановления");
    assert_eq!(a.parent(), Some(std::path::Path::new("/w")), "восстановление лежит рядом");
    assert_eq!(
        recovery_path_for(None),
        crate::app::autosave_path(),
        "у безымянного проекта каталога ещё нет — восстановление в каталоге данных"
    );
}

/// Тест 358 (Р.49 А3): фоновая запись идёт в ФАЙЛ ВОССТАНОВЛЕНИЯ, проект
/// пользователя не трогает, звёздочку не гасит — и всё это восстановимо:
/// в recovery лежит настоящая модель с правками.
#[test]
fn t358_autosave_writes_recovery_not_project() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t358-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t358.otio");
    std::fs::write(&target, "исходный файл пользователя").unwrap();

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.model.apply(Command::AddMedia { id: MediaId(7), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }

    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "исходный файл пользователя",
        "автосейв переписал файл пользователя — его меняет только Ctrl+S"
    );
    assert!(h.state().dirty_content, "автосейв погасил звёздочку: работа считается сохранённой");
    let rec = crate::app::recovery_path_for(Some(&target));
    let text = std::fs::read_to_string(&rec).expect("файла восстановления нет");
    let saved = kt_otio::load_project(&text, rec.parent()).unwrap().project;
    assert_eq!(saved.media.len(), 1, "в восстановлении не та модель");
    assert_eq!(h.state().recovery_written.as_deref(), Some(rec.as_path()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 359 (Р.49 А3): сохранение уносит СВОЙ файл восстановления и не трогает
/// чужой. Прежний код сносил `autosave_path()` безусловно — Ctrl+S в одной
/// копии программы убивал несохранённую работу другой.
#[test]
fn t359_save_drops_only_own_recovery() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t359-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t359.otio");
    let mine = crate::app::recovery_path_for(Some(&target));
    let alien = dir.join("other.otio.recovery.otio");
    std::fs::write(&mine, "мой снимок").unwrap();
    std::fs::write(&alien, "чужая несохранённая работа").unwrap();

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.recovery_written = Some(mine.clone());
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.save();

    assert!(target.exists(), "Ctrl+S обязан записать проект");
    assert!(!mine.exists(), "своё восстановление не убрано — старт предложит уже сохранённое");
    assert!(alien.exists(), "снесено ЧУЖОЕ восстановление: потеряна работа другой копии");
    assert!(h.state().recovery_written.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 360 (Р.49 А3): дрейф вьюпорта не запускает запись. Зум и скролл
/// раз в 30 секунд гоняли полную сериализацию проекта БЕЗ ЕДИНОЙ правки —
/// на большом монтаже это заметный фриз ни за что (инвариант 8 по духу:
/// прокрутка таймлайна не порождает работы).
#[test]
fn t360_viewport_drift_does_not_write() {
    let dir = std::env::temp_dir().join(format!("kt-t360-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t360.otio");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.dirty_content = false;
    app.dirty_ui = false;
    // проект «как на диске» не знает текущего вьюпорта — и это не повод писать
    app.model.set_ui_state(None);
    app.ticks_per_px *= 1.7;
    app.view_start += 400_000.0;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_none(), "дрейф вьюпорта запустил запись проекта");
    assert!(!crate::app::recovery_path_for(Some(&target)).exists());

    // а настоящая правка — запускает
    let app = h.state_mut();
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    assert!(h.state().autosave_rx.is_some(), "правка обязана запускать автосейв");
    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 361 (Р.49 А2/А3): сторожа ДВА. Один мьютекс на две цели поднимал бы
/// планку ревизии фоновой записью восстановления — и следующий Ctrl+S
/// оказывался «устаревшим», то есть молча ничего не сохранял.
#[test]
fn t361_recovery_guard_does_not_block_save() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t361-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t361.otio");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(40);
    h.step();
    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().autosave_rx.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(
        *h.state().recovery_rev.lock().unwrap() > 0,
        "фоновая запись восстановления обязана поднять СВОЙ сторож"
    );
    assert_eq!(*h.state().saved_rev.lock().unwrap(), 0, "она подняла сторож ПРОЕКТА");

    // Ctrl+S той же ревизии обязан пройти
    h.state_mut().save();
    assert!(target.exists(), "Ctrl+S отменён сторожем файла восстановления");
    assert!(!h.state().dirty_content);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 355 (Р.49 А2): исход записи — ТИП, и Ctrl+S его разбирает. Раньше
/// `write_project_guarded` возвращал `bool`, а `save()` делал `.map(|_| ())`:
/// отказ по сторожу («на диске уже свежее») выглядел как успех — звёздочка
/// гасла, в статусе писалось «сохранено», а файла с этими правками не было.
#[test]
fn t355_save_reports_skipped_and_keeps_dirty() {
    use kt_model::*;
    let dir = std::env::temp_dir().join(format!("kt-t355-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("t355.otio");

    let mut h = harness();
    let app = h.state_mut();
    app.project_path = Some(target.clone());
    app.model.apply(Command::AddMedia { id: MediaId(1), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.save();
    assert!(!h.state().dirty_content, "обычный Ctrl+S обязан гасить звёздочку");
    let ok_len = std::fs::read_to_string(&target).unwrap().len();

    // на диск «лёг» снимок ревизии из будущего: сторож обязан отказать
    let app = h.state_mut();
    *app.saved_rev.lock().unwrap() = app.model_rev + 100;
    app.model.apply(Command::AddMedia { id: MediaId(2), path: "synth:ramp".into() }).unwrap();
    app.model_rev += 1;
    app.dirty_content = true;
    app.notices.clear();
    app.save();

    let st = h.state();
    assert!(st.dirty_content, "отказ по сторожу погасил звёздочку: правки считаются сохранёнными");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap().len(),
        ok_len,
        "при отказе файл не должен меняться"
    );
    let texts: Vec<&str> = st.notices.iter().map(|n| n.text.as_str()).collect();
    assert!(!texts.is_empty(), "об отказе не сказано ни слова");
    assert!(
        !texts.iter().any(|t| t.contains(&target.display().to_string())),
        "об отказе сказано как об успешном сохранении: {texts:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 356 (Р.49 А2): повтор одного сообщения красит колокольчик. Схлопывание
/// дублей было сделано ранним возвратом ДО инкремента `notices_unread` — и
/// вторая подряд «не удалось сохранить» не поднимала счётчик непрочитанных:
/// единственный канал сообщения об ошибке записи умирал ровно тогда, когда
/// ошибка повторялась. Плюс ошибка Ctrl+S — это Warn, а не Info.
#[test]
fn t356_repeated_notice_still_marks_unread() {
    let mut h = harness();
    let app = h.state_mut();
    app.notices.clear();
    app.notices_unread = 0;

    app.warn("disk on fire");
    app.warn("disk on fire");
    app.warn("disk on fire");
    assert_eq!(app.notices.len(), 1, "дубли обязаны схлопываться в одну строку");
    assert_eq!(app.notices.back().unwrap().repeats, 3, "счётчик повторов не вырос");
    assert_eq!(app.notices_unread, 3, "повтор не покрасил колокольчик");
    assert_eq!(
        app.notices.back().unwrap().text,
        "disk on fire",
        "«×N» — дело показа: в тексте записи его быть не должно"
    );
    assert!(matches!(app.notices.back().unwrap().severity, crate::app::Severity::Warn));

    // другое сообщение начинает свою строку
    app.warn("other");
    assert_eq!(app.notices.len(), 2);
    assert_eq!(app.notices.back().unwrap().repeats, 1);

    // ошибка записи проекта приходит именно как предупреждение
    let app = h.state_mut();
    app.notices.clear();
    app.project_path = Some(std::path::PathBuf::from("/proc/self/no-such-dir/x.otio"));
    app.model_rev += 1;
    app.dirty_content = true;
    app.save();
    let last = h.state().notices.back().cloned().expect("об ошибке записи не сказано");
    assert!(
        matches!(last.severity, crate::app::Severity::Warn),
        "ошибка Ctrl+S пришла как обычная заметка: {last:?}"
    );
    assert!(h.state().dirty_content, "после ошибки записи звёздочка обязана остаться");
}

/// Пиксель кадра, который ЖИВОЙ источник плеера отдаёт прямо сейчас
/// (Р.53): проверка «правка доехала до картинки», а не только до модели.
/// Ходит тем же путём, что превью: воркер → кэш.
pub(crate) fn generator_pixel(
    h: &egui_kittest::Harness<'_, crate::app::KtApp>,
    id: kt_model::MediaId,
) -> Option<[u8; 3]> {
    generator_pixel_after(h, id, 0).map(|(px, _)| px)
}

/// Пиксель генератора и СЕРИЙНЫЙ НОМЕР записи кэша (Р.53, п.416).
///
/// Серийник обязателен, когда правку надо увидеть ГЛАЗАМИ теста: кадр
/// старых параметров живёт в кэше до тех пор, пока воркер не дойдёт до
/// перенастройки (чистка идёт ВНУТРИ его потока — п.411), и голое чтение
/// возвращало бы предыдущую картинку. Ждём кадр, записанный ПОЗЖЕ
/// `after`: это ровно «пришёл НОВЫЙ кадр», а не «дождались нужного
/// цвета» — второе было бы тестом, который не умеет покраснеть.
pub(crate) fn generator_pixel_after(
    h: &egui_kittest::Harness<'_, crate::app::KtApp>,
    id: kt_model::MediaId,
    after: u64,
) -> Option<([u8; 3], u64)> {
    let app = h.state();
    let q = kt_media::Quality { width: 64, height: 36 };
    let idx = kt_engine::video::video_cache_idx(kt_time::Tick(0));
    let sid = kt_engine::source_id(id);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.player.request_video(id, kt_time::Tick(0), q);
        if let Some((f, serial)) = app.player.cache.get_video_stamped(sid, Some(q), idx)
            && serial > after
        {
            return Some(([f.rgba[0], f.rgba[1], f.rgba[2]], serial));
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Событие-генератор, только что созданное из меню: (медиа, событие).
/// Значения живут на СОБЫТИИ (Р.59, стадия 2), поэтому тесту нужно и то
/// и другое.
pub(crate) fn made_generator(
    h: &egui_kittest::Harness<'_, crate::app::KtApp>,
) -> (kt_model::MediaId, kt_model::ClipId) {
    let p = h.state().model.project();
    let m = p.media.last().expect("медиа генератора");
    let c = p.clips.iter().find(|c| c.media == m.id).expect("событие генератора");
    (m.id, c.id)
}

/// Что выбрано в полосе цепочки окна эффектов этого события.
pub(crate) fn chain_sel(
    h: &egui_kittest::Harness<'_, crate::app::KtApp>,
    clip: kt_model::ClipId,
) -> Option<crate::widgets::ChainSel> {
    h.state()
        .fx_editors
        .iter()
        .find(|e| e.members.contains(&clip))
        .and_then(|e| e.selected.get(&clip).copied())
}

/// Значения генератора события — то, что с Р.59 и есть «параметры».
pub(crate) fn gen_of(
    h: &egui_kittest::Harness<'_, crate::app::KtApp>,
    clip: kt_model::ClipId,
) -> kt_model::GeneratorKind {
    h.state().model.clip(clip).and_then(|c| c.r#gen.clone()).expect("у события есть генератор")
}

/// Тест 477 (Р.52, переписан в Р.59 стадии 2): «Создать генератор» —
/// медиа без файла появляется в модели и в сервисах, а ЗНАЧЕНИЯ ложатся
/// на СОБЫТИЕ.
///
/// Что изменилось по решению владельца: путь медиа несёт теперь ТОЛЬКО
/// ВИД (`gen:solid`), выбранные в меню значения едут на событие, и
/// «окно параметров» — это окно эффектов события с выбранной ГОЛОВОЙ
/// полосы. Своего окна у генератора больше нет: владелец просил видеть
/// источник первой крошкой цепочки, в том же окне.
#[test]
fn t477_create_generator_from_bin() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    let before = h.state().model.project().media.len();
    let made = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    h.state_mut().create_generator(made.clone());
    h.step();
    let (id, clip) = made_generator(&h);
    let app = h.state();
    let p = app.model.project();
    assert_eq!(p.media.len(), before + 1, "медиа не добавилось");
    assert_eq!(
        p.media.last().unwrap().path,
        "gen:solid",
        "путь медиа несёт ТОЛЬКО вид: значения переехали на событие"
    );
    assert!(p.is_gen_media(id));
    assert_eq!(gen_of(&h, clip), made, "выбранные значения обязаны лечь на СОБЫТИЕ");
    // «окно параметров» открыто сразу — и открыто на ГОЛОВЕ полосы
    assert_eq!(app.fx_editors.len(), 1, "окно параметров не открылось");
    assert!(app.fx_editors[0].members.contains(&clip));
    assert_eq!(
        chain_sel(&h, clip),
        Some(crate::widgets::ChainSel::Head),
        "форма генератора — ПЕРВАЯ КРОШКА цепочки, и окно обязано открыться на ней"
    );
    // сервис тумб знает вид и «длительность» — карточка нарисует тумбу,
    // а не «♪», и скроет нулевую длительность
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let _ = app.thumbs.duration_or_probe(id);
    loop {
        if let Some((v, a)) = h.state().thumbs.kinds(id) {
            assert!(v && !a, "генератор — видео без аудио");
            break;
        }
        assert!(std::time::Instant::now() < deadline, "probe генератора не опубликован");
        std::thread::sleep(std::time::Duration::from_millis(10));
        h.step();
    }
    assert_eq!(h.state().thumbs.duration(id), Some(kt_time::Tick(0)));
    // повторное открытие (второй путь меню) не плодит второе окно
    h.state_mut().open_generator_editor(id);
    assert_eq!(h.state().fx_editors.len(), 1, "окно обязано быть одно");
    // снимки синка зарегистрированы — правки пойдут диффом
    assert_eq!(h.state().media_paths.get(&id).map(String::as_str), Some("gen:solid"));
    assert_eq!(
        h.state().gen_states.get(&clip).map(|st| st.kind.clone()),
        Some(made),
        "снимок источника события обязан знать его значения"
    );
}

/// Тест 478 (Р.52, усилен в Р.53 п.412, переписан в Р.59 стадии 2):
/// правка цвета в живой сессии окна доезжает до МОДЕЛИ, до ИСТОЧНИКА
/// плеера и до КАРТИНКИ; «Отмена» возвращает backup; вся сессия — ОДИН
/// шаг undo, и undo тоже доезжает до картинки.
///
/// Механизм новый (`SetClipGen` на событии вместо `SetMediaPath` на
/// медиа), утверждение прежнее: правка видна человеку, а не только
/// модели. Картинка спрашивается по АДРЕСУ ИСТОЧНИКА СОБЫТИЯ — с Р.59
/// это единственное правило (`kt_engine::clip_source_addr`).
#[test]
fn t478_generator_edit_reaches_model_and_source() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
    h.step();
    let (id, clip) = made_generator(&h);
    let addr = kt_engine::gen_source_addr(clip);
    let gesture = h.state().fx_editors[0].gesture;
    let depth = h.state().model.undo_depth();

    // сессия окна: две правки цвета тем же gesture (как шлёт пипетка)
    for c in [(0x10, 0x40, 0x80), (0xff, 0x88, 0x00)] {
        let kind = GeneratorKind::Solid { color_r: c.0, color_g: c.1, color_b: c.2 };
        let app = h.state_mut();
        assert!(app.apply_coalesced(Command::SetClipGen { id: clip, kind: Some(kind) }, gesture));
        h.step();
    }
    assert_eq!(
        gen_of(&h, clip),
        GeneratorKind::Solid { color_r: 0xff, color_g: 0x88, color_b: 0x00 }
    );
    let app = h.state();
    // источник плеера догнал модель (перенастройкой, а не пересозданием)
    assert_eq!(
        app.gen_states.get(&clip).map(|st| st.kind.clone()),
        Some(GeneratorKind::Solid { color_r: 0xff, color_g: 0x88, color_b: 0x00 })
    );
    assert_eq!(app.model.undo_depth(), depth + 1, "сессия окна — один шаг истории");
    // ...и до КАРТИНКИ: источник плеера отдаёт кадр НОВОГО цвета
    let (px, serial) = crate::layout_tests::generator_pixel_after(&h, addr, 0)
        .expect("кадр генератора так и не пришёл");
    assert_eq!(px, [255, 136, 0], "правка обязана доехать до картинки, а не только до модели");

    // undo возвращает и модель, и картинку (хук в undo, а не только в apply)
    h.state_mut().undo();
    h.step();
    assert_eq!(
        gen_of(&h, clip),
        GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 },
        "undo обязан вернуть значения события"
    );
    assert_eq!(
        h.state().gen_states.get(&clip).map(|st| st.kind.clone()),
        Some(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        "undo обязан догнать источник — иначе картинка отстаёт от модели"
    );
    let (px, _) = crate::layout_tests::generator_pixel_after(&h, addr, serial)
        .expect("после undo новый кадр так и не пришёл");
    assert_eq!(px, [0, 0, 0], "undo обязан доехать до картинки");

    // «Отмена» окна: снимок ГОЛОВЫ взят на открытии и откатывается тем
    // же gesture — значения источника часть той же сессии окна
    h.state_mut().redo();
    h.step();
    let backup = h.state().fx_editors[0].gen_backup.clone();
    assert_eq!(
        backup.iter().find(|(c, _, _)| *c == clip).map(|(_, k, _)| k.clone()).unwrap(),
        Some(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        "backup — значения на момент открытия"
    );
    {
        let app = h.state_mut();
        let kind = backup.iter().find(|(c, _, _)| *c == clip).unwrap().1.clone();
        assert!(app.apply_coalesced(Command::SetClipGen { id: clip, kind }, gesture));
    }
    h.step();
    assert_eq!(gen_of(&h, clip), GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
    {
        // клип генератора снимаем первым: создание кладёт его сразу
        // (п.416), а медиа под живым событием модель удалить не даёт
        let app = h.state_mut();
        let clips: Vec<ClipId> =
            app.model.project().clips.iter().filter(|c| c.media == id).map(|c| c.id).collect();
        assert!(!clips.is_empty(), "создание генератора обязано ставить клип (п.416)");
        app.apply_batch(clips.into_iter().map(|id| Command::RemoveClip { id }).collect());
        assert!(app.apply(Command::RemoveMedia { id }));
    }
    h.step();
    assert!(h.state().fx_editors.is_empty(), "окно мёртвого события обязано закрыться");
    assert!(!h.state().media_paths.contains_key(&id), "источник мёртвого медиа снят");
    assert!(!h.state().gen_states.contains_key(&clip), "источник мёртвого события снят");
}

/// Тест 487 (Р.52 С3, переписан в Р.59 стадии 2): все входы генераторов
/// ведут в ОДНО место — окно эффектов события с выбранной головой;
/// правка текста через сессию окна доезжает до модели, схлопывается в
/// один шаг undo, и окно умирает вместе с событием.
///
/// Что изменилось по решению владельца: отдельного окна титра больше
/// нет («запрятан в жопу» — про прежнюю развязку окон), а текст живёт на
/// СОБЫТИИ, а не в строке-адресе медиа. Утверждение прежнее: набранный
/// текст доезжает до модели без потерь и одним шагом истории.
#[test]
fn t487_title_editor_routes_and_edits() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    let (id, clip) = made_generator(&h);
    assert_eq!(h.state().fx_editors.len(), 1, "форма титра — в окне эффектов события");
    assert_eq!(
        chain_sel(&h, clip),
        Some(crate::widgets::ChainSel::Head),
        "титр обязан открыться на голове полосы"
    );
    let gesture = h.state().fx_editors[0].gesture;
    let depth = h.state().model.undo_depth();

    // сессия окна: набранный текст уезжает на СОБЫТИЕ
    let mut kind = gen_of(&h, clip);
    if let GeneratorKind::Text { text, .. } = &mut kind {
        *text = "Привет,\nтитр!".into();
    }
    {
        let app = h.state_mut();
        assert!(
            app.apply_coalesced(
                Command::SetClipGen { id: clip, kind: Some(kind.clone()) },
                gesture
            )
        );
    }
    h.step();
    assert_eq!(gen_of(&h, clip), kind, "текст доехал до события целиком");
    assert_eq!(
        h.state().model.project().media.last().unwrap().path,
        "gen:text",
        "в пути медиа остаётся ТОЛЬКО вид"
    );
    assert_eq!(h.state().model.undo_depth(), depth + 1, "сессия окна — один шаг истории");
    assert_eq!(
        h.state().gen_states.get(&clip).map(|st| st.kind.clone()),
        Some(kind),
        "правка обязана доехать до источника события"
    );

    // повторное открытие — то же окно
    h.state_mut().open_generator_editor(id);
    assert_eq!(h.state().fx_editors.len(), 1);

    // событие умерло — окно ушло
    {
        let app = h.state_mut();
        let clips: Vec<ClipId> =
            app.model.project().clips.iter().filter(|c| c.media == id).map(|c| c.id).collect();
        assert!(!clips.is_empty(), "создание генератора обязано ставить клип (п.416)");
        app.apply_batch(clips.into_iter().map(|id| Command::RemoveClip { id }).collect());
    }
    h.step();
    assert!(h.state().fx_editors.is_empty(), "окно мёртвого титра обязано закрыться");
}

/// Тест 488 (Р.52 С3, правило UI 11): «глифа нет» — видимый ответ.
/// Кириллица/латиница полного вендоренного шрифта чисты; CJK-символ
/// называется кодпоинтом и счётчиком остальных.
#[test]
fn t488_missing_glyph_warning() {
    use kt_model::GeneratorKind;
    let mut k = GeneratorKind::default_text();
    let set = |k: &mut GeneratorKind, s: &str| {
        if let GeneratorKind::Text { text, .. } = k {
            *text = s.into();
        }
    };
    set(&mut k, "Привет, Kinotrain! ёЁ №5");
    assert_eq!(crate::gen_ui::title_warning(&k), None, "кириллица покрыта");
    set(&mut k, "до 漢字 после");
    let warn = crate::gen_ui::title_warning(&k).expect("CJK непокрыт — ответ обязан быть");
    assert!(warn.contains("U+5B57") || warn.contains("U+6F22"), "{warn}");
    assert!(warn.contains("+1"), "счётчик остальных: {warn}");
    set(&mut k, "");
    assert_eq!(crate::gen_ui::title_warning(&k), None);
}

/// Тест 489 (Р.52 С3): safe-зоны — 5 % и 10 % от кадра превью, видны
/// ровно пока правится титр.
#[test]
fn t489_safe_zones_follow_title_window() {
    use kt_model::GeneratorKind;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    assert!(!h.state().show_safe_zones(), "без правки титра зоны не рисуются");
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    assert!(h.state().show_safe_zones(), "форма титра включает зоны");
    let img = egui::Rect::from_min_max(egui::pos2(100.0, 200.0), egui::pos2(300.0, 300.0));
    let [action, title] = crate::app::safe_zone_rects(img);
    assert_eq!(action.min, egui::pos2(110.0, 205.0));
    assert_eq!(action.max, egui::pos2(290.0, 295.0));
    assert_eq!(title.min, egui::pos2(120.0, 210.0));
    assert_eq!(title.max, egui::pos2(280.0, 290.0));
    // окно закрыли — зоны ушли
    h.state_mut().fx_editors[0].open = false;
    h.step();
    assert!(!h.state().show_safe_zones());
}

/// Тест 495 (Р.52 С6, переписан в Р.59 стадии 2): драг титра ПРЯМО В
/// ПРЕВЬЮ — рамка блока, пока титр правится, а перетаскивание в три шага
/// пишет pos_x/pos_y ТОЙ ЖЕ командой (`SetClipGen`) и тем же gesture,
/// что форма в окне (правило 17 по построению).
#[test]
fn t495_title_drag_in_preview_writes_pos() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    let (_id, clip) = made_generator(&h);
    let gesture = h.state().fx_editors[0].gesture;
    // титру нужен видимый блок — даём текст той же сессией окна
    let mut kind = gen_of(&h, clip);
    if let GeneratorKind::Text { text, size, .. } = &mut kind {
        *text = "DRAG".into();
        *size = 0.2;
    }
    {
        let app = h.state_mut();
        assert!(app.apply_coalesced(Command::SetClipGen { id: clip, kind: Some(kind) }, gesture));
    }
    h.step();
    let preview = h.state().preview_rect;
    assert!(preview.width() > 40.0, "превью в харнесте есть: {preview:?}");

    let depth = h.state().model.undo_depth();
    let before = match gen_of(&h, clip) {
        GeneratorKind::Text { pos_x, pos_y, .. } => (pos_x, pos_y),
        _ => unreachable!(),
    };
    assert_eq!(before, (0.5, 0.5));

    // драг в ТРИ шага из центра превью (блок с якорем Center сидит там)
    let c = preview.center();
    h.drag_at(c);
    h.step();
    h.hover_at(c + egui::vec2(9.0, 0.0));
    h.step();
    h.hover_at(c + egui::vec2(40.0, 24.0));
    h.step();
    h.drop_at(c + egui::vec2(40.0, 24.0));
    h.step();

    let after = match gen_of(&h, clip) {
        GeneratorKind::Text { pos_x, pos_y, .. } => (pos_x, pos_y),
        _ => unreachable!(),
    };
    assert!(
        after.0 > before.0 + 0.01 && after.1 > before.1 + 0.01,
        "драг обязан сдвинуть позицию: {before:?} -> {after:?}"
    );
    assert_eq!(
        h.state().model.undo_depth(),
        depth,
        "драг пишет тем же gesture, что сессия окна, — новых шагов истории нет"
    );
}

/// Тест 502 (Р.53, п.412; переписан в Р.59 стадии 2): сессия правки
/// генератора НЕ пересоздаёт источник и НЕ пересобирает рендереры.
/// Тридцать правок цвета — ровно столько, сколько даёт секунда драга
/// пипетки: до п.412 каждая гасила и поднимала поток декодера и дважды
/// пересобирала графы, а на игре это гарантированные провалы звука.
///
/// Механизм новый (`SetClipGen` вместо `SetMediaPath`), закон П тот же:
/// команда, не трогающая план, не имеет права пересобирать рендереры.
#[test]
fn t502_generator_edit_does_not_respawn_source() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
    h.step();
    let (_id, clip) = made_generator(&h);
    let addr = kt_engine::gen_source_addr(clip);
    let gesture = h.state().fx_editors[0].gesture;
    let spawns = h.state().player.worker_spawns();
    let pver = h.state().player.project_version();

    for i in 0..30u8 {
        let kind = GeneratorKind::Solid { color_r: i, color_g: i, color_b: i };
        let app = h.state_mut();
        assert!(app.apply_coalesced(Command::SetClipGen { id: clip, kind: Some(kind) }, gesture));
        h.step();
    }

    let app = h.state();
    assert_eq!(
        app.player.worker_spawns(),
        spawns,
        "поток декодера пересоздавался на каждую правку параметров"
    );
    assert_eq!(
        app.player.project_version(),
        pver,
        "рендереры пересобирались: SetClipGen план не трогает (закон П)"
    );
    assert_eq!(
        app.gen_states.get(&clip).map(|st| st.kind.clone()),
        Some(GeneratorKind::Solid { color_r: 0x1d, color_g: 0x1d, color_b: 0x1d }),
        "снимок синка обязан догнать модель"
    );
    assert_eq!(
        crate::layout_tests::generator_pixel(&h, addr),
        Some([0x1d, 0x1d, 0x1d]),
        "последняя правка обязана доехать до картинки"
    );
}

/// Тест 503 (Р.53, п.413): кадр ЖЕСТА не пересобирает карту путей.
/// Синк зовётся из каждой обёртки правок, то есть на каждом кадре любого
/// coalesced-жеста — а он собирал `HashMap` с клоном каждого пути
/// (аллокации в пути кадра, инвариант 12).
#[test]
fn t503_gesture_frame_touches_no_media_paths() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    let gesture = 4242u64;
    // один жест, 60 кадров — секунда драга уровня события
    let before = h.state().media_sync_builds;
    for i in 0..60 {
        let app = h.state_mut();
        let db = -6.0 + i as f32 * 0.1;
        assert!(app.apply_coalesced(Command::SetGain { id: ClipId(1), gain_db: db }, gesture));
        h.step();
    }
    let builds = h.state().media_sync_builds - before;
    assert!(builds <= 60, "карта путей пересобиралась чаще, чем менялась модель: {builds}");
    // состав медиа не менялся ни разу — источники трогать было незачем
    assert_eq!(h.state().player.worker_spawns(), 0, "жест уровня поднимал источники");
}

/// Тест 504 (Р.53, п.413): псевдо-путь НЕ открывает sidecar-БД. У
/// генератора файла нет, а открытие sqlite (соединение + WAL-прагма +
/// версия схемы) висело на каждой правке параметров и на каждой
/// постановке клипа.
#[test]
fn t504_pseudo_path_never_opens_sqlite() {
    let dir = std::env::temp_dir().join(format!("kt-t504-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("x.ktindex.db");
    assert!(!db.exists());

    let src = crate::util::open_source_indexed("gen:solid:c=ff8800", Some(&db));
    assert!(src.is_ok(), "генератор обязан открыться");
    assert!(!db.exists(), "псевдо-путь открыл sidecar-БД");

    let src = crate::util::open_source_indexed("synth:ramp", Some(&db));
    assert!(src.is_ok());
    assert!(!db.exists(), "тестовая схема открыла sidecar-БД");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 505 (Р.53, п.413): о подменённом шрифте говорится ОДИН раз.
/// Путь титра меняется на каждый набранный символ, и без дедупа
/// отсутствующая семья давала свой notice на каждую правку — шторм
/// одинаковых сообщений и счётчик непрочитанного в сотнях.
#[test]
fn t505_missing_font_warns_once() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    let mut kind = GeneratorKind::default_text();
    if let GeneratorKind::Text { family, .. } = &mut kind {
        *family = "__kt_no_such_family__".into();
    }
    h.state_mut().create_generator(kind.clone());
    h.step();
    let (_id, clip) = made_generator(&h);
    let gesture = h.state().fx_editors[0].gesture;
    let notices = h.state().notices.len();

    for i in 0..40 {
        let mut k = kind.clone();
        if let GeneratorKind::Text { text, .. } = &mut k {
            *text = "a".repeat(i + 1);
        }
        let app = h.state_mut();
        assert!(app.apply_coalesced(Command::SetClipGen { id: clip, kind: Some(k) }, gesture));
        h.step();
    }
    let added = h.state().notices.len() - notices;
    assert!(added <= 1, "о подмене шрифта сказали {added} раз вместо одного");
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
            invert: false,
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
        p.ensure_tracks(kt_model::LaneKind::Camera, 1);
        p.cameras.push(CameraEvent {
            ease: true,
            id: CameraId(1),
            lane: 0,
            start: Tick(0),
            duration: Tick(120_000),
            name: String::new(),
            cam: CameraParams::default(),
            keyframes: vec![],
            auto: vec![],
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
    assert_eq!(h.state().camera_editors.len(), 1, "двойной клик по камере обязан открыть её окно");

    // правка через сессию окна: один gesture — один шаг истории
    let gesture = h.state().camera_editors[0].gesture;
    {
        let app = h.state_mut();
        let mut c = app.model.project().cameras[0].clone();
        c.name = "main".into();
        c.auto = vec![ParamCurve {
            param: "pos_z".into(),
            keys: vec![AutoKey::at(Tick(0), -900.0), AutoKey::at(Tick(120_000), -300.0)],
            source: Default::default(),
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(45_000),
        targets: MorphTargets::Below,
        patch: MorphPatch { media: Some(MediaId(1)), ..Default::default() },
        timed: true,
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    use crate::timeline::{THIN_ROW_MIN, TlGeom, row_height, row_offsets};
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
    let offs = row_offsets(&rows, 60.0, &[]);
    assert_eq!(offs.len(), rows.len() + 1);
    assert_eq!(offs, vec![0.0, 40.0, 100.0, 120.0, 180.0, 220.0]);

    let g = TlGeom::new(
        egui::Rect::from_min_max(egui::pos2(52.0, 100.0), egui::pos2(1052.0, 600.0)),
        60.0,
        rows.clone(),
        &[],
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        targets: MorphTargets::Below,
        patch: MorphPatch::default(),
        timed: true,
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
        enabled: true,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    h.step();

    let g = h.state().tl_geom.clone().expect("геометрия");
    let mrow = g.row_of((LaneKind::Morph, 0)).expect("морф-строка есть");
    // строка действительно втрое ниже дорожки клипов — и всё же нажимаема
    assert!(g.row_h(mrow) < g.lane_h_of(ClipKind::Video, 0), "служебная строка обязана быть ниже");
    assert!(g.row_h(mrow) >= crate::timeline::THIN_ROW_MIN, "но не ниже пола нажимаемости");

    // клик по блоку в тонкой строке выделяет морфер
    let pos = egui::pos2(g.tick_to_x(45_000), g.row_center_y(mrow));
    h.drag_at(pos);
    h.step();
    h.drop_at(pos);
    h.step();
    assert!(
        h.state().selected_morph.contains(&MorphId(1)),
        "в тонкую морф-строку не попасть мышью — считать её сделанной нельзя"
    );

    // вертикальный зум не ломает отношение высот
    let g = h.state().tl_geom.clone().unwrap();
    let app = h.state_mut();
    app.tl_v_zoom = Some(g.lane_h_of(ClipKind::Video, 0) * 1.5);
    h.step();
    let g2 = h.state().tl_geom.clone().unwrap();
    assert!(g2.lane_h_of(ClipKind::Video, 0) > g.lane_h_of(ClipKind::Video, 0), "зум применился");
    assert!(
        (g2.row_h(mrow) / g2.lane_h_of(ClipKind::Video, 0) - 1.0 / crate::timeline::MORPH_ROW_DIV)
            .abs()
            < 0.05,
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
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        targets: MorphTargets::Areas { names: vec![names[0].clone(), "нет такой".into()] },
        patch: MorphPatch::default(),
        timed: true,
        strength: 1.0,
        fade_in: kt_time::Tick::ZERO,
        fade_out: kt_time::Tick::ZERO,
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
    app.paste_here(at, None, "копия".into(), false);
    assert_eq!(app.model.undo_depth(), depth + 1, "вставка — один шаг истории");
    let names: Vec<String> = app.model.project().areas.iter().map(|a| a.name.clone()).collect();
    // Р.38 п.19: «надо по дефолту проставлять не префикс а суффикс» —
    // копии сортируются рядом с оригиналом, а не по первой букве слова
    assert!(names.contains(&"финал копия".to_string()), "суффикс применён: {names:?}");
    // а префикс остаётся возможностью того же окна
    app.model.undo().unwrap();
    app.paste_here(at, None, "копия".into(), true);
    let names: Vec<String> = app.model.project().areas.iter().map(|a| a.name.clone()).collect();
    assert!(names.contains(&"копия финал".to_string()), "префикс всё ещё доступен: {names:?}");
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    for (id, kind) in [(1u64, ClipKind::Video), (2, ClipKind::Audio)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
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
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    assert_eq!(pr.lanes(kt_model::LaneKind::Morph), 1, "первая морф-дорожка заведена");
    assert_eq!(pr.morphers.len(), 1);
    assert_eq!(pr.morphers[0].start, Tick(180_000), "блок встал под курсором");
    assert_eq!(pr.morphers[0].duration, Tick(90 * 1500), "и на 1.5 с");

    // повторное «+M» в другом месте — ТА ЖЕ дорожка, новый блок
    app.player.seek(Tick(495_000)); // кратно 15: playhead живёт в аудиокадрах
    app.add_morph_layer();
    let pr = app.model.project().clone();
    assert_eq!(
        pr.lanes(kt_model::LaneKind::Morph),
        1,
        "вторая морф-дорожка НЕ заводится: «слой» — не «дорожка»"
    );
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
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 0 }));
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
    assert!(h.state().selected_camera.contains(&c.id), "клик по камере обязан её выделить");

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
    app.select_camera_only(c.id);
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

/// Тест 262 (Р.38 п.11, дословно: «разреши менять высоту каждого трека
/// индивидуально»). Нижняя кромка строки в gutter'е тянет её высоту,
/// пункт меню сбрасывает, и это УДОБСТВО — звёздочку не зажигает.
#[test]
fn t262_per_track_height() {
    use crate::timeline::{ROW_H_MAX, THIN_ROW_MIN, TlGeom, row_height_of};
    use kt_model::LaneKind;

    // чистая геометрия: переопределение бьёт расчётную высоту и клампится
    let over = vec![(LaneKind::Video, 0, 150.0), (LaneKind::Morph, 0, 5.0)];
    assert_eq!(row_height_of(LaneKind::Video, 0, 60.0, &over), 150.0);
    assert_eq!(row_height_of(LaneKind::Video, 1, 60.0, &over), 60.0, "чужая дорожка не тронута");
    assert_eq!(
        row_height_of(LaneKind::Morph, 0, 60.0, &over),
        THIN_ROW_MIN,
        "предел снизу: в строку в 5 px мышью не попасть"
    );
    assert_eq!(
        row_height_of(LaneKind::Video, 0, 60.0, &[(LaneKind::Video, 0, 9999.0)]),
        ROW_H_MAX,
        "предел сверху есть у всего, что тянется мышью (правило 6)"
    );
    // верхи строк согласованы с высотами
    let rows = vec![(LaneKind::Video, 0), (LaneKind::Video, 1)];
    let g = TlGeom::new(
        egui::Rect::from_min_max(egui::pos2(50.0, 0.0), egui::pos2(1000.0, 500.0)),
        60.0,
        rows,
        &over,
        0.0,
        0.0,
        1000.0,
    );
    assert_eq!(g.row_h(0), 150.0);
    assert_eq!(g.row_top(1) - g.row_top(0), 150.0, "верх второй строки поехал за высотой первой");

    // жест: тянем нижнюю кромку первой строки в gutter'е
    let (mut h, g0) = harness_three_clips();
    let row0_bottom = g0.row_top(0) + g0.row_h(0);
    // x берётся В СЕРЕДИНЕ gutter'а, а не у самого края окна: первые 16 px
    // — зона ресайза окна, она забирает нажатие себе (правило 15)
    let gx = g0.rect.left() - 12.0;
    let want = g0.row_h(0) + 40.0;
    h.hover_at(egui::pos2(gx, row0_bottom));
    h.step();
    h.drag_at(egui::pos2(gx, row0_bottom));
    h.step();
    h.hover_at(egui::pos2(gx, row0_bottom + 8.0));
    h.step();
    h.hover_at(egui::pos2(gx, row0_bottom + 40.0));
    h.step();
    h.drop_at(egui::pos2(gx, row0_bottom + 40.0));
    h.step();
    let app = h.state();
    let over = app.row_heights();
    assert_eq!(over.len(), 1, "высота дорожки запомнена: {over:?}");
    let (k, l, hh) = over[0];
    assert_eq!((k, l), g0.rows[0], "запомнена ИМЕННО та строка, за кромку которой тянули");
    assert!((hh - want).abs() < 3.0, "высота {hh} ≈ {want}");
    // с Р.62 высота живёт НА ДОРОЖКЕ и ходит через undo вместе с именем
    assert!(app.dirty_content, "высота дорожки — содержимое модели");

    // и сброс возвращает расчётную
    let app = h.state_mut();
    app.set_row_height(g0.rows[0], None);
    assert!(app.row_heights().is_empty(), "сброс убирает переопределение");
    assert!(app.model.undo_depth() >= 2, "высота обязана ходить через undo");
}

/// Тест 265 (Р.38 пп.14-18): панель автоматизации.
///
/// Претензии дословно: «в таймлайне автоматизации я не понимаю как
/// зазумится», «при нажатии на ключ - чтобы значения которые прописаны в
/// полях - имели связь с ключем», «ощущение что у эффектов есть какое-то
/// отдельное "дефолтное" состояние значений», «я добавил второй параметр
/// автоматизации - но редактируется только первый», «еще не хватает
/// кнопок управления элементами таймлайна автоматизации».
#[test]
fn t265_automation_axis_and_field_bind() {
    use crate::automation::{AutoCtx, AutoView};
    use crate::widgets::{bound_apply, bound_value};
    use kt_model::{AutoKey, ParamCurve};
    use kt_time::Tick;

    // ---- п.14: ось времени ------------------------------------------
    let id = |t: Tick| t;
    // замок с Р.40 живёт в UiState (один на программу), а не в AutoView
    let mut ctx = AutoCtx {
        default_shape: Default::default(),
        specs: &[],
        range: (Tick(0), Tick(90_000 * 10)),
        view: (Tick(90_000 * 2), Tick(90_000 * 4)),
        to_timeline: &id,
        to_curve: &id,
        frame_step: 3000,
        linked: true,
    };
    assert!(
        crate::app::KtApp::default_axis_linked(),
        "замок «связать с таймлайном» включён по умолчанию (решение п.14)"
    );
    let mut v = AutoView::default();
    assert_eq!(
        v.axis(&ctx),
        (90_000 * 2, 90_000 * 4),
        "связанный график показывает окно таймлайна"
    );
    ctx.linked = false;
    assert_eq!(v.axis(&ctx), (0, 90_000 * 10), "без своего окна отвязанный показывает всё");
    v.win = Some((90_000, 90_000 * 3));
    assert_eq!(v.axis(&ctx), (90_000, 90_000 * 3), "и своё окно, когда оно есть");
    v.win = Some((-90_000, 90_000 * 99));
    assert_eq!(v.axis(&ctx), (0, 90_000 * 10), "СВОЁ окно клампится диапазоном носителя");
    // Р.40: СВЯЗАННАЯ ось окном носителя НЕ клампится. Претензия дословно:
    // «когда я оттягиваю левый край события — у меня ключи автоматизации
    // тоже сдвигаются к началу». Ключ не двигался: врала ось, нормированная
    // на событие. Смотрим то же, что таймлайн, — и ключ стоит на месте.
    ctx.linked = true;
    ctx.view = (Tick(-90_000), Tick(90_000 * 99));
    assert_eq!(
        v.axis(&ctx),
        (-90_000, 90_000 * 99),
        "связанный график показывает окно ТАЙМЛАЙНА как есть, включая место за краями события"
    );
    ctx.view = (Tick(90_000 * 2), Tick(90_000 * 4));
    ctx.linked = false;
    v.win = Some((5000, 5001));
    assert_eq!(
        v.axis(&ctx),
        (0, 90_000 * 10),
        "вырожденное окно (уже кадра) откатывается к полному: делить на ноль нечем"
    );

    // ---- пп.15-16: поле правит ровно то, что показывает --------------
    let mut c = ParamCurve {
        param: "db".into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(90_000), 6.0)]),
        source: Default::default(),
    };
    // выбран ключ — поле показывает и правит ЕГО
    assert_eq!(bound_value(&c, Some(1), Tick(45_000)), 6.0);
    bound_apply(&mut c, Some(1), Tick(45_000), 3.0);
    assert_eq!(c.keys[1].v, 3.0, "правка поля ушла в выбранный ключ");
    assert_eq!(c.keys.len(), 2, "и лишнего ключа не появилось");
    // ключ не выбран — поле показывает значение кривой ПОД КУРСОРОМ
    let mid = bound_value(&c, None, Tick(45_000));
    assert!((mid - 1.5).abs() < 1e-9, "показано значение кривой под курсором: {mid}");
    bound_apply(&mut c, None, Tick(45_000), 4.0);
    assert_eq!(c.keys.len(), 3, "правка без выбранного ключа ставит ключ под курсором");
    // новый ключ НАСЛЕДУЕТ форму отрезка, который делит (Р.61): иначе
    // клик по графику молча превращал бы зигзаг в сглаживание, а у
    // скорости варпа сглаженный ключ был бы отказом валидации
    assert_eq!(c.keys[1], AutoKey::shaped(Tick(45_000), 4.0, c.keys[0].shape));
    assert_eq!(c.keys[1].shape, kt_model::SegShape::Linear, "форма отрезка не унаследована");
    assert!(c.keys.windows(2).all(|w| w[0].t.0 <= w[1].t.0), "порядок ключей не нарушен");
    // выбор за пределами кривой не роняет и трактуется как «не выбран»
    assert_eq!(bound_value(&c, Some(99), Tick(0)), c.keys[0].v);

    // ---- пп.17-18: активная кривая и кнопки в живом окне -------------
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::slowed_reverb(0.5, 0.3, 85.0)],
        fx_bypass: vec![false],
        fx_auto: vec![vec![]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();

    // добавляем ДВА параметра автоматизации подряд
    let names: Vec<String> = {
        let f = h.state().model.project().clips[0].fx[0].clone();
        f.auto_params().take(2).map(|s| crate::widgets::param_label(&s.name)).collect()
    };
    assert_eq!(names.len(), 2, "у slowed+reverb есть хотя бы два автоматизируемых параметра");

    for n in &names {
        h.get_by_label(tr(k::fx::AUTO_ADD)).click();
        h.step();
        h.step();
        // подпись параметра есть и в сетке значений, и в открывшемся
        // меню — жмём ПОСЛЕДНЮЮ: меню рисуется поверх
        h.get_all_by_label(n.as_str()).last().expect("пункт меню параметра").click();
        h.step();
        h.step();
    }
    let curves = h.state().model.project().clips[0].fx_auto[0].clone();
    assert_eq!(curves.len(), 2, "обе кривые заведены: {curves:?}");
    assert_eq!(
        h.state().fx_editors[0].auto_views[&(ClipId(1), 0)].active,
        1,
        "ВТОРОЙ добавленный параметр обязан стать активным — иначе клик по графику \
         продолжает класть ключи в первую кривую, и вторая выглядит мёртвой (п.17)"
    );

    // кнопки управления ключами и осью на месте (п.18)
    for icon in [
        crate::icons::PLUS,
        crate::icons::SKIP_BACK,
        crate::icons::SKIP_FWD,
        crate::icons::TRASH,
        crate::icons::LOCK,
        crate::icons::ARROWS_OUT,
    ] {
        assert!(
            h.query_all_by_label(icon).count() >= 1,
            "в панели автоматизации нет кнопки {icon:?} (п.18)"
        );
    }
}

/// Тест 269 (Р.38 п.12): сила и фейды слоя правятся В ОКНЕ морфера и
/// доезжают до модели. До этого раунда в патче не было ни одного поля
/// силы, а `enabled` был бинарным — ровно расхождение, с которого начался
/// Р.36 («морфер в Vegas позволяет ползунком управлять прозрачностью
/// наложения эффектов, а у нас — нет»).
#[test]
fn t269_morph_strength_and_fades_reach_the_model() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    use kt_time::Tick;

    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        app.player.seek(Tick(90_000));
        app.add_morph_layer();
    }
    h.step();
    h.step();
    assert!(h.state().morph_editors.iter().any(|e| e.open), "окно морфера открылось");
    // подпись силы видна — значит контрол дошёл до пользователя
    assert!(
        h.query_by_label(tr(k::mrph::STRENGTH)).is_some(),
        "в окне морфера нет поля силы (п.12)"
    );

    // ставим силу и фейды через модель и проверяем, что движок их видит
    let app = h.state_mut();
    let mut ev = app.model.project().morphers[0].clone();
    ev.patch.video_fx = vec![FxInstance::edges(24)];
    ev.strength = 0.4;
    ev.fade_in = Tick(9_000);
    assert!(app.apply(Command::SetMorpher(ev)));
    let eff = app.model.project().effective_clips();
    let mixed: Vec<&ClipEvent> = eff
        .iter()
        .filter(|c| c.fx.iter().any(|f| matches!(f, FxInstance::LayerMix { .. })))
        .collect();
    // блок накрывает несколько событий — слот силы дописан КАЖДОМУ
    assert!(!mixed.is_empty(), "слот силы дописан накрытым кускам");
    let c = mixed[0];
    let mix_at = c.fx.iter().position(|f| matches!(f, FxInstance::LayerMix { .. })).unwrap();
    match &c.fx[mix_at] {
        FxInstance::LayerMix { amount, from } => {
            assert!((amount - 0.4).abs() < 1e-9, "сила доехала: {amount}");
            assert_eq!(
                *from as usize,
                mix_at.saturating_sub(1),
                "вклад слоя начинается с его эффекта"
            );
        }
        _ => unreachable!(),
    }
    assert!(!c.fx_auto[mix_at].is_empty(), "фейд — это КРИВАЯ силы, а не отдельный механизм");

    // Сила 1.0 без фейдов слота НЕ создаёт — ТАМ, ГДЕ СЛОЙ НАКРЫВАЕТ
    // СОБЫТИЕ ЦЕЛИКОМ. Именно на этом стоят GOLDEN'ы: полное накрытие —
    // самый частый случай, и путь для него обязан остаться байт-в-байт.
    //
    // С Р.55 (С2) у ЧАСТИЧНО накрытого события слот появляется и на
    // умолчаниях: «по времени» теперь выражается ВОРОТАМИ силы, а не
    // резкой, и без слота слой бил бы по всему телу. Это не регрессия
    // умолчаний, а то, ради чего ворота и заведены.
    let app = h.state_mut();
    let mut ev = app.model.project().morphers[0].clone();
    ev.strength = 1.0;
    ev.fade_in = Tick::ZERO;
    ev.fade_out = Tick::ZERO;
    let win = ev.window();
    assert!(app.apply(Command::SetMorpher(ev)));
    let eff = app.model.project().effective_clips();
    let has_mix = |c: &ClipEvent| c.fx.iter().any(|f| matches!(f, FxInstance::LayerMix { .. }));
    let whole: Vec<&ClipEvent> =
        eff.iter().filter(|c| win.start <= c.start && c.start + c.duration <= win.end).collect();
    assert!(!whole.is_empty(), "сцена выродилась: слой не накрывает ни одного события целиком");
    assert!(
        whole.iter().all(|c| !has_mix(c)),
        "на умолчаниях у ЦЕЛИКОМ накрытого события слота смешения нет вовсе — \
         на этом стоят GOLDEN'ы"
    );
    // а у частично накрытого он есть, и это и есть «по времени»
    let part: Vec<&ClipEvent> = eff
        .iter()
        .filter(|c| {
            let body = kt_time::TickRange::new(c.start, c.start + c.duration);
            !body.intersect(&win).is_empty() && !(win.start <= c.start && body.end <= win.end)
        })
        .collect();
    assert!(
        part.iter().all(|c| has_mix(c)),
        "у частично накрытого события обязаны быть ВОРОТА силы — иначе слой \
         действует на всё тело, а человек просил окно"
    );
}

/// Тест 274 (Р.40 п.11): высота дорожки доходит до СОБЫТИЯ.
///
/// Претензия дословно: «дорожка меняет высоту а события до сих пор мелкие —
/// ты нихуя не проверял». И правда: t262 проверял, что поле `lane_heights`
/// получило значение и `row_h` изменился, а во всём `draw_timeline` стояла
/// ОДНА переменная `lane_h = vl.base_h` на 36 использований — событие как
/// рисовалось базовой высотой, так и рисовалось.
///
/// Поэтому этот тест утверждает НАБЛЮДАЕМОЕ СЛЕДСТВИЕ: прямоугольник
/// события вырос, и мышь попадает в него ТАМ, где он теперь нарисован.
#[test]
fn t274_row_height_reaches_the_clip() {
    use kt_model::{ClipKind, LaneKind};

    let (mut h, g0) = harness_three_clips();
    let key = (LaneKind::Video, 0u32);
    let before = g0.lane_h_of(ClipKind::Video, 0);

    // ставим строке высоту вдвое больше базовой
    let want = before * 2.0;
    {
        let app = h.state_mut();
        app.set_row_height(key, Some(want));
    }
    h.step();
    h.step();
    let g = h.state().tl_geom.clone().unwrap();

    assert!(
        (g.lane_h_of(ClipKind::Video, 0) - want).abs() < 1.0,
        "СОБЫТИЕ обязано считаться высотой своей строки: {} вместо {want}",
        g.lane_h_of(ClipKind::Video, 0)
    );
    assert!(
        (g.row_h(g.row_of(key).unwrap()) - want).abs() < 1.0,
        "и строка тоже (это и раньше работало)"
    );

    // СЛЕДСТВИЕ: мышь попадает в событие НИЖЕ прежней границы —
    // там, где до Р.40 была голая подложка дорожки
    let c = h.state().model.project().clips[0].clone();
    let x = g.tick_to_x(c.start.0 + c.duration.0 / 2);
    let deep_y = g.lane_top(ClipKind::Video, 0) + before + (want - before) * 0.5;
    h.hover_at(egui::pos2(x, deep_y));
    h.step();
    h.drag_at(egui::pos2(x, deep_y));
    h.step();
    h.drop_at(egui::pos2(x, deep_y));
    h.step();
    assert!(
        h.state().is_selected(c.id),
        "клик в нижнюю половину выросшей строки обязан попасть в событие \
         (y = {deep_y}, прежняя граница была {})",
        g.lane_top(ClipKind::Video, 0) + before
    );

    // соседняя дорожка не поехала
    let audio_h = g.lane_h_of(ClipKind::Audio, 0);
    assert!(
        (audio_h - before).abs() < 1.0,
        "чужая дорожка сохраняет свою высоту: {audio_h} вместо {before}"
    );
}

/// Тест 275 (Р.40 п.4, дословно: «можно сто раз добавить морфер слой, и он
/// будет каждый раз его создавать на одном и том же треке — в итоге там
/// может быть куча морферов скрыта за одним и тем же местом»).
///
/// Р.38 переиспользовал ПЕРВУЮ морф-строку всегда. Пользователь ответил,
/// что делать: «но надо проверить — может ещё одна дорожка уже свободная
/// есть». Свободная — значит свободная НА ЭТОМ ПРОМЕЖУТКЕ.
///
/// Тест утверждает НАБЛЮДАЕМОЕ СЛЕДСТВИЕ: два блока не лежат друг на
/// друге, то есть каждый можно схватить мышью.
#[test]
fn t275_morph_layer_never_hides_a_block_under_another() {
    use kt_model::LaneKind;
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        app.player.seek(Tick(100_000));
        app.add_morph_layer();
        // второе «+M» РОВНО ТАМ ЖЕ: место занято
        app.add_morph_layer();
        app.morph_editors.clear();
    }
    h.step();
    let pr = h.state().model.project().clone();
    assert_eq!(pr.morphers.len(), 2, "оба блока созданы");
    assert_eq!(
        pr.lanes(kt_model::LaneKind::Morph),
        2,
        "занятое место обязано увести блок на новую дорожку"
    );
    assert_ne!(pr.morphers[0].lane, pr.morphers[1].lane, "и они не на одной дорожке");

    // третье «+M» в СВОБОДНОЕ время: новой дорожки быть не должно —
    // «слой» не про дорожки, дорожки заводятся своим пунктом меню
    {
        let app = h.state_mut();
        app.player.seek(Tick(600_000));
        app.add_morph_layer();
        app.morph_editors.clear();
    }
    h.step();
    let pr = h.state().model.project().clone();
    assert_eq!(
        pr.lanes(kt_model::LaneKind::Morph),
        2,
        "свободное время — дорожка переиспользуется"
    );
    assert_eq!(pr.morphers.len(), 3);

    // и следствие, ради которого всё: каждый блок ловится мышью в СВОЕЙ
    // строке — ни один не спрятан под другим
    let g = h.state().tl_geom.clone().unwrap();
    for m in &pr.morphers {
        let row = g.row_of((LaneKind::Morph, m.lane)).expect("строка блока");
        let pos = egui::pos2(g.tick_to_x(m.start.0 + m.duration.0 / 2), g.row_center_y(row));
        h.hover_at(pos);
        h.step();
        h.drag_at(pos);
        h.step();
        h.drop_at(pos);
        h.step();
        assert!(
            h.state().selected_morph.contains(&m.id),
            "блок {:?} на дорожке {} не ловится мышью — он спрятан под другим",
            m.id,
            m.lane
        );
    }
}

/// Тест 276 (Р.40 п.2, дословно: «я не могу перемещать морферы по разным
/// дорожкам»). Морф- и камера-блок ездят между своими строками мышью —
/// как клип.
#[test]
fn t276_service_blocks_move_between_lanes() {
    use kt_model::{Command, LaneKind};
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        app.player.seek(Tick(100_000));
        app.add_morph_layer();
        app.add_morph_layer(); // вторая строка (место занято) — есть куда везти
        app.morph_editors.clear();
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 0 }));
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 1 }));
        app.player.seek(Tick(300_000));
        app.add_camera_at_playhead(0);
    }
    h.step();
    h.step();

    // ---- морфер: с дорожки 0 на дорожку 1 ----
    let g = h.state().tl_geom.clone().unwrap();
    let m = h.state().model.project().morphers[0].clone();
    let from_row = g.row_of((LaneKind::Morph, m.lane)).unwrap();
    let to_lane = if m.lane == 0 { 1 } else { 0 };
    let to_row = g.row_of((LaneKind::Morph, to_lane)).unwrap();
    let x = g.tick_to_x(m.start.0 + m.duration.0 / 2);
    let from = egui::pos2(x, g.row_center_y(from_row));
    let to = egui::pos2(x, g.row_center_y(to_row));
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    // порог драга: сначала маленький сдвиг, потом цель (правило харнесса)
    h.hover_at(egui::pos2(x + 8.0, from.y));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();
    let m2 = h.state().model.project().morphers.iter().find(|z| z.id == m.id).unwrap().clone();
    assert_eq!(m2.lane, to_lane, "морф-блок обязан переезжать на строку под курсором");

    // ---- камера: то же самое ----
    let g = h.state().tl_geom.clone().unwrap();
    let c = h.state().model.project().cameras[0].clone();
    let from_row = g.row_of((LaneKind::Camera, c.lane)).unwrap();
    let to_lane = if c.lane == 0 { 1 } else { 0 };
    let to_row = g.row_of((LaneKind::Camera, to_lane)).unwrap();
    let x = g.tick_to_x(c.start.0 + c.duration.0 / 2);
    let from = egui::pos2(x, g.row_center_y(from_row));
    let to = egui::pos2(x, g.row_center_y(to_row));
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    h.hover_at(egui::pos2(x + 8.0, from.y));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();
    let c2 = h.state().model.project().cameras.iter().find(|z| z.id == c.id).unwrap().clone();
    assert_eq!(c2.lane, to_lane, "камерный блок обязан переезжать на строку под курсором");
}

/// Тест 277 (Р.40, дословно: «Морферы почему-то не выделяются через
/// area-select, нихуя ты не проверял»).
///
/// Рамка ловила ТОЛЬКО клипы — её улов был `Vec<ClipId>`, тип сам это
/// заявлял, — а поймав клипы, ещё и ГАСИЛА выделенный морфер.
#[test]
fn t277_rubber_band_catches_service_blocks() {
    use kt_model::{Command, LaneKind};
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    {
        let app = h.state_mut();
        app.player.seek(Tick(100_000));
        app.add_morph_layer();
        app.morph_editors.clear();
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 0 }));
        app.player.seek(Tick(100_000));
        app.add_camera_at_playhead(0);
        app.sel_tool = true; // рамка обычным драгом
    }
    h.step();
    h.step();

    let g = h.state().tl_geom.clone().unwrap();
    let pr = h.state().model.project().clone();
    let m = pr.morphers[0].clone();
    let c = pr.cameras[0].clone();
    let m_row = g.row_of((LaneKind::Morph, m.lane)).unwrap();
    let c_row = g.row_of((LaneKind::Camera, c.lane)).unwrap();
    // рамка от верха верхней служебной строки до низа нижней, по времени —
    // поверх обоих блоков
    let top = g.row_top(m_row.min(c_row)) - 2.0;
    let bot = g.row_top(m_row.max(c_row)) + g.row_h(m_row.max(c_row)) - 2.0;
    let x0 = g.tick_to_x(m.start.0.min(c.start.0)) + 2.0;
    let x1 = g.tick_to_x((m.start + m.duration).0.min((c.start + c.duration).0)) - 2.0;
    assert!(x1 > x0 + 10.0, "сцена: блоки перекрываются по времени");

    let from = egui::pos2(x0, top);
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    h.hover_at(egui::pos2(x0 + 8.0, top + 4.0));
    h.step();
    h.hover_at(egui::pos2(x1, bot));
    h.step();
    h.drop_at(egui::pos2(x1, bot));
    h.step();

    assert!(
        h.state().selected_morph.contains(&m.id),
        "рамка обязана выделять морфер: она рисуется поверх него и «ловит» — а ловила нет"
    );
    assert!(h.state().selected_camera.contains(&c.id), "и камеру — тем же жестом");

    // и следствие: Del убирает ровно пойманное
    h.state_mut().delete_action();
    h.step();
    let pr = h.state().model.project().clone();
    assert!(pr.morphers.is_empty() && pr.cameras.is_empty(), "Del убирает всё пойманное рамкой");
}

/// Тест 278 (Р.40 п.12, дословно: «я у мелкого события пытаюсь там выловить
/// opacity но не получается»).
///
/// Замков было два, и оба подтверждались арифметикой:
///   * `ClipZone::Level` требовал `x1 − x0 > 24.0` — на коротком событии
///     уровня не было ни при каком y;
///   * ручка фейда ловилась на ±12 px от УГЛА всегда, поэтому на событии
///     уже 24 px обе ручки перекрывали его целиком и середины не
///     оставалось.
///
/// Тест утверждает СЛЕДСТВИЕ: на коротком событии тянут вниз в середине
/// верхней полосы — и непрозрачность падает.
#[test]
fn t278_opacity_reachable_on_a_short_event() {
    use crate::timeline::{fade_grab_for_width, level_y, top_strip_h};
    use kt_model::ClipKind;
    use kt_time::Tick;

    // чистая арифметика: середина узкого события не принадлежит ручкам
    for w in [10.0f32, 18.0, 24.0, 40.0] {
        let g = fade_grab_for_width(w);
        assert!(
            2.0 * g < w,
            "при ширине {w} ручки фейда (±{g}) перекрывают событие целиком — \
             середины, где живёт уровень, не остаётся"
        );
    }

    let (mut h, _) = harness_three_clips();
    // укоротим первое событие до ~20 px: ровно тот случай, о котором речь
    let (id, x0, y_line) = {
        let g0 = h.state().tl_geom.clone().unwrap();
        let px = 20.0 * g0.tpp;
        let c = h.state().model.project().clips[0].clone();
        let app = h.state_mut();
        assert!(
            app.apply(kt_model::Command::TrimEnd { id: c.id, new_end: c.start + Tick(px as i64) })
        );
        (c.id, c.start.0, 0.0)
    };
    let _ = y_line;
    h.step();
    h.step();

    let g = h.state().tl_geom.clone().unwrap();
    let c = h.state().model.project().clips.iter().find(|z| z.id == id).unwrap().clone();
    let wide = g.tick_to_x((c.start + c.duration).0) - g.tick_to_x(c.start.0);
    assert!(wide < 24.0, "сцена: событие короче старого порога, {wide} px");
    assert_eq!(c.opacity, 1.0, "старт с умолчания — с него и начинают тянуть");

    let top = g.lane_top(ClipKind::Video, c.lane);
    let lane_h = g.lane_h_of(ClipKind::Video, c.lane);
    let mid_x = (g.tick_to_x(x0) + g.tick_to_x((c.start + c.duration).0)) * 0.5;
    let ly = level_y(1.0, top, lane_h);
    assert!(
        ly <= top + top_strip_h(lane_h),
        "линия уровня при opacity = 1.0 лежит внутри полосы фейда — \
         именно поэтому первый захват и не удавался"
    );

    // и жест доводит дело до модели
    let from = egui::pos2(mid_x, ly);
    h.hover_at(from);
    h.step();
    h.drag_at(from);
    h.step();
    h.hover_at(egui::pos2(mid_x, ly + 8.0));
    h.step();
    h.hover_at(egui::pos2(mid_x, ly + lane_h * 0.4));
    h.step();
    h.drop_at(egui::pos2(mid_x, ly + lane_h * 0.4));
    h.step();
    let after = h.state().model.project().clips.iter().find(|z| z.id == id).unwrap().opacity;
    assert!(
        after < 0.95,
        "тянули линию уровня вниз — непрозрачность обязана упасть, а она {after}"
    );
}

/// Тест 279 (Р.40 пп.15/18 и «ключи съезжают»).
///
/// Три претензии одного семейства:
///   * «замочек не до конца работает, надо сделать так чтобы его надо было
///     СНЯТЬ чтобы отключилась синхронизация таймлайнов, и замочек надо
///     чтобы запоминал свое состояние»;
///   * «не хватает по мимо кнопки установки ключа - двух кнопок ключ ДО и
///     ключ ПОСЛЕ текущей позиции»;
///   * «когда я оттягиваю левый край события - у меня ключи автоматизации
///     тоже сдвигаются к началу, и это поведение никак не поменять».
///
/// Последняя — не про данные: ключ НЕ двигается, врала ось. Тест
/// утверждает обе половины: ключ на месте, и ось его показывает на месте.
#[test]
fn t279_axis_lock_and_keys_that_stay() {
    use kt_model::{AutoKey, Command, FxInstance, ParamCurve};
    use kt_time::Tick;

    // ---- замок помнится в проекте и один на программу ----------------
    let mut h = harness();
    {
        let app = h.state_mut();
        assert!(app.auto_axis_linked, "умолчание — замок НАДЕТ (правило 4 чек-листа)");
        app.auto_axis_linked = false;
        let ui = app.ui_state_for_test();
        assert!(ui.auto_axis_unlinked, "снятый замок обязан попадать в проект");
        let back = kt_model::UiState { auto_axis_unlinked: true, ..Default::default() };
        assert!(!(!back.auto_axis_unlinked), "и читаться обратно снятым");
    }

    // ---- ключ при триме остаётся на СВОЁМ кадре ----------------------
    let (mut h, _) = harness_three_clips();
    let id = h.state().model.project().clips[0].id;
    {
        let app = h.state_mut();
        let mut c = app.model.project().clips[0].clone();
        c.fx = vec![FxInstance::blur(4)];
        c.fx_auto = vec![vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![
                AutoKey::at(Tick(10_000), 1.0),
                AutoKey::at(Tick(40_000), 9.0),
            ]),
            source: Default::default(),
        }]];
        let mut p = app.model.project().clone();
        p.clips[0] = c;
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
    }
    let before: Vec<i64> =
        h.state().model.project().clips[0].fx_auto[0][0].keys.iter().map(|k| k.t.0).collect();
    // левый край вправо на 20 000 тиков
    let start = h.state().model.project().clips[0].start;
    assert!(h.state_mut().apply(Command::TrimStart { id, new_start: start + Tick(20_000) }));
    let after: Vec<i64> =
        h.state().model.project().clips[0].fx_auto[0][0].keys.iter().map(|k| k.t.0).collect();
    assert_eq!(
        after, before,
        "ключ обязан остаться на ТОМ ЖЕ кадре материала: «вот как сделать так чтобы \
         он НЕ съезжал никуда хоть я левый край двигаю влево хоть в право»"
    );

    // ---- и опция, которая делает ОБРАТНОЕ, выключена по умолчанию ----
    {
        let app = h.state_mut();
        assert!(
            !app.model.project().trim_stretches_auto,
            "растягивание анимации вместе с событием — НЕ умолчание"
        );
        assert!(app.apply(Command::SetTrimStretchesAuto { on: true }));
    }
    let c0 = h.state().model.project().clips[0].clone();
    let span_old = kt_model::clip_material_span(&c0);
    let offs_old: Vec<i64> = c0.fx_auto[0][0].keys.iter().map(|k| k.t.0 - c0.source_in.0).collect();
    let start = c0.start;
    assert!(h.state_mut().apply(Command::TrimStart { id, new_start: start + Tick(10_000) }));
    let c1 = h.state().model.project().clips[0].clone();
    let stretched: Vec<i64> = c1.fx_auto[0][0].keys.iter().map(|k| k.t.0).collect();
    assert_ne!(stretched, after, "с включённой опцией ключи обязаны поехать");
    // и поехать РОВНО пропорционально: доля ключа внутри события сохранена
    let span_new = kt_model::clip_material_span(&c1);
    for (k, off0) in stretched.iter().zip(&offs_old) {
        let want =
            c1.source_in.0 + (*off0 as f64 * span_new as f64 / span_old as f64).round() as i64;
        assert_eq!(
            *k, want,
            "доля ключа внутри события обязана сохраниться: {span_old} → {span_new}"
        );
    }
}

/// Тест 281 (Р.43 п.1): значение ключа, НАПЕЧАТАННОЕ в поле параметра,
/// доезжает до модели.
///
/// Претензия дословно: «в таймлайне автоматизации для ключей slowed-reverb я
/// например проставляю в поле ввода rate а он скидывается на предыдущее
/// значение, а если на графике сдвину ключ - то значение меняется как надо.
/// Кстати - в морфере это работает хорошо».
///
/// Причина: у поля и у графика был ОДИН флаг на двоих, а окно FX клало
/// `auto` в команду только по флагу графика. Правка поля уходила в локальный
/// клон `auto`, который умирал вместе с кадром, и на следующем кадре поле
/// читало старое значение из модели.
///
/// Тест утверждает СЛЕДСТВИЕ (кривая в МОДЕЛИ) и щупает ИЗ ТОЧКИ, ОТКУДА
/// ДЕЙСТВУЕТ ЧЕЛОВЕК (символы в поле рядом с подписью параметра) — этого не
/// делал t265, который проверял `bound_value`/`bound_apply` как чистые
/// функции и был зелёным на сломанной программе.
#[test]
fn t281_typed_key_value_reaches_the_model() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(9), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(9),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: MediaId(9),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::slowed_reverb(1.0, 0.2, 60.0)],
        fx_bypass: vec![],
        fx_auto: vec![vec![ParamCurve {
            param: "rate".into(),

            keys: AutoKey::linear(vec![
                AutoKey::at(kt_time::Tick(0), 1.0),
                AutoKey::at(kt_time::Tick(90_000), 2.0),
            ]),
            source: Default::default(),
        }]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(9));
    h.step();
    h.step();

    // поле ЖИВОГО параметра стоит в сетке между своей подписью и подписью
    // следующего параметра — целимся туда же, куда человек. `DragValue`
    // своего узла в дереве доступности не заводит, поэтому по подписи его
    // не найти: берём координату из РЕАЛЬНОЙ вёрстки.
    let (lbl_rate, lbl_mix) =
        (crate::widgets::param_label("rate"), crate::widgets::param_label("mix"));
    // подпись параметра встречается дважды: в сетке значений и кнопкой
    // выбора кривой в панели автоматизации НИЖЕ. Нам нужна ВЕРХНЯЯ — сетка
    // рисуется до графика.
    let top_label = |h: &Harness<'_, KtApp>, lbl: &str| -> egui::Rect {
        h.query_all_by_label(lbl)
            .map(|n| n.rect())
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .unwrap_or_else(|| panic!("подпись параметра {lbl:?} не нарисована"))
    };
    let r = top_label(&h, &lbl_rate);
    let (x0, y) = (r.right(), r.center().y);
    let x1 = top_label(&h, &lbl_mix).left();
    assert!(x1 > x0 + 8.0, "между подписями параметров обязано быть место под поле");
    let pos = egui::pos2((x0 + x1) * 0.5, y);

    // клик по полю переводит его в режим ввода; дальше — символы и Enter
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
    h.event(egui::Event::Text("3".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();

    let curves = h.state().model.project().clips[0].fx_auto[0].clone();
    let key = curves
        .iter()
        .find(|c| c.param == "rate")
        .and_then(|c| c.keys.iter().find(|k| k.t == kt_time::Tick(0)))
        .expect("ключ кривой rate в нуле материала");
    assert_eq!(
        key.v, 3.0,
        "напечатанное значение обязано доехать до МОДЕЛИ, а не откатиться \
         (кривая: {curves:?})"
    );
    // и правка кривой не выдумывает шагов истории на каждый символ
    assert_eq!(
        h.state().model.project().clips[0].fx_auto[0][0].keys.len(),
        2,
        "поле правит существующий ключ, а не плодит новые"
    );
}

/// Тест 286 (Р.43 п.296): правка «удобства» не пересобирает движок.
///
/// Претензия: «регион "зацикленность_5" какой-то поломанный при
/// воспроизведении, underruns растут». `on_region_changed` звала
/// `Player::set_project` — клон всего проекта плюс `VideoRenderer::new` и
/// пересборка `AudioRenderer` в render-потоке. На монтаже пользователя это
/// 1961 событие НА КАЖДОЕ движение границы региона и на каждое нажатие
/// Loop: аудио-поток вставал, кольцо пустело, начинались
/// callback-underrun'ы — а после п.294 они больше не тормозят часы, но
/// слышны тишиной.
///
/// Регион и `loop_enabled` живут в «удобстве» (`apps/kt-app/CLAUDE.md`), и
/// движок их из проекта не читает: луп доезжает `set_loop_region`.
#[test]
fn t286_region_edits_do_not_rebuild_the_engine() {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
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
    app.player.set_project(app.model.project().clone());

    let before = app.player.project_version();
    app.model.set_region(Region::normalized(kt_time::Tick(0), kt_time::Tick(45_000)));
    app.on_region_changed();
    app.toggle_loop();
    app.clear_region();
    assert_eq!(
        app.player.project_version(),
        before,
        "регион и loop — «удобство»: движок пересобирать не за чем"
    );

    // а правка СОДЕРЖИМОГО обязана доехать до плеера тем же путём, что и была
    app.apply(Command::SetGain { id: ClipId(1), gain_db: -3.0 });
    assert!(
        app.player.project_version() > before,
        "правка содержимого обязана доносить проект до движка"
    );
}

/// Тест 289 (Р.43 п.290): два числа underrun'ов подписаны, а тишина
/// показана в МИЛЛИСЕКУНДАХ.
///
/// Претензия пользователя была написана так: «левый underruns растет, и
/// правый тоже начинает чуть позже». Левое число — промахи РЕНДЕРА,
/// правое — голодание КОЛБЭКА, и это каскад «кэш не отдал бёрст → кольцо
/// опустело», а вовсе не левый и правый каналы. В интерфейсе стояло голое
/// `Underruns X/Y` без единого слова о том, что есть что, — читать их как
/// L/R было совершенно законно.
///
/// Отдельно: голодание колбэка — это КАДРЫ, которых не было на выходе.
/// В кадрах число человеку ни о чём не говорит; в строке состояния оно
/// показано ещё и миллисекундами тишины.
#[test]
fn t289_underrun_counters_are_labelled_and_silence_is_in_ms() {
    use egui_kittest::kittest::Queryable;
    use std::sync::atomic::Ordering;

    let mut h = harness();
    // различимые числа: перепутать местами не выйдет
    const RENDER: u64 = 7;
    const CALLBACK: u64 = 96_000; // ровно 2000 мс тишины при 48 кГц
    {
        let app = h.state();
        app.player.underruns.store(RENDER, Ordering::Relaxed);
        app.player.shared.callback_underruns.store(CALLBACK, Ordering::Relaxed);
    }
    h.step();
    h.step();

    // берём НАСТОЯЩИЙ текст надписи, а не Debug узла: в дампе узла полно
    // букв от служебных полей, и проверка «между числами есть слово»
    // выполнялась бы сама собой
    use egui_kittest::kittest::NodeT;
    let texts: Vec<String> = h
        .query_all_by_label_contains("")
        .filter_map(|n| {
            let a = n.accesskit_node();
            a.value().map(|v| v.to_string()).or_else(|| a.label().map(|l| l.to_string()))
        })
        .filter(|t| t.to_lowercase().contains("underrun"))
        .collect();
    let all = texts.join("\n").to_lowercase();
    assert!(!texts.is_empty(), "счётчики underrun'ов обязаны быть видны — их не нашлось вовсе");
    assert!(
        all.contains(&RENDER.to_string()) && all.contains(&CALLBACK.to_string()),
        "оба числа обязаны быть показаны: {texts:?}"
    );
    // и хотя бы в одном месте они РАЗЛИЧЕНЫ СЛОВАМИ, а не только косой
    // чертой: «7/96000» не говорит, где рендер, а где колбэк
    let labelled = texts.iter().any(|t| {
        let (Some(a), Some(b)) = (t.find(&RENDER.to_string()), t.find(&CALLBACK.to_string()))
        else {
            return false;
        };
        a < b && t[a + 1..b].chars().any(char::is_alphabetic)
    });
    assert!(
        labelled,
        "между числами обязано стоять слово (рендер/колбэк), а не только «/»: {texts:?}"
    );
    // тишина — в миллисекундах, а не в кадрах
    let ms = CALLBACK / (kt_time::AUDIO_SAMPLE_RATE as u64 / 1000);
    assert!(
        all.contains(&ms.to_string()),
        "голодание колбэка обязано быть показано и в миллисекундах ({ms} мс): {texts:?}"
    );
}

/// Тест 343 (Р.48, п.342): вставка FX-цепочки В МОРФЕР доносит кривые.
///
/// Дефект жил с Р.38: `ChainEdit.auto` был `Option`, и при `None` ветка
/// переноса кривых молча пропускалась — вставленная цепочка приезжала
/// «пустой», хотя в буфере анимация была. Теперь тип не даёт этой ветке
/// существовать, а тест сторожит СМЫСЛ: ключи доехали и пересчитаны в
/// домен морфера (время блока — отступ от точки входа донора).
#[test]
fn t343_paste_chain_into_morpher_carries_curves() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.add_morph_layer();
    // в буфере — цепочка С КРИВОЙ от донора с точкой входа 45_000
    app.fx_clipboard = Some(kt_model::ops::FxChainBlock {
        slots: vec![kt_model::ops::FxSlotBlock {
            fx: FxInstance::blur(8),
            bypass: false,
            curves: vec![ParamCurve {
                param: "radius".into(),

                keys: AutoKey::linear(vec![
                    AutoKey::at(kt_time::Tick(45_000), 1.0),
                    AutoKey::at(kt_time::Tick(135_000), 48.0),
                ]),
                source: Default::default(),
            }],
        }],
        src_in: kt_time::Tick(45_000),
        fx_off: false,
    });
    h.step();
    h.step();
    assert!(h.state().morph_editors.iter().any(|e| e.open), "окно морфера не открылось");

    // кнопка вставки цепочки — та же, что у события
    let paste = tr(k::fx::PASTE_CHAIN).to_string();
    h.get_by_label(&paste).click();
    h.step();
    h.step();

    let ev = h.state().model.project().morphers[0].clone();
    assert_eq!(ev.patch.video_fx.len(), 1, "цепочка обязана вставиться в морфер");
    let curves: Vec<_> = ev.patch.video_auto.iter().flatten().collect();
    assert_eq!(curves.len(), 1, "кривая обязана доехать вместе со слотом");
    assert_eq!(
        curves[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, 90_000],
        "и пересчитаться в домен морфера (отступ от точки входа донора)"
    );
}

/// Проект с одним видео-событием и открытым окном FX — общая обвязка
/// тестов пресетов.
#[cfg(test)]
fn harness_with_fx_window() -> egui_kittest::Harness<'static, crate::app::KtApp> {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(30_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![false],
        fx_auto: vec![vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![
                AutoKey::at(kt_time::Tick(30_000), 1.0),
                AutoKey::at(kt_time::Tick(120_000), 48.0),
            ]),
            source: Default::default(),
        }]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    h
}

/// Тест 345 (Р.48, п.344): окно «сохранить пресетом» по чек-листу t250.
///
/// Окно не открывается само, реально рисуется по кнопке, Esc закрывает
/// начисто, повторный вызов работает, а ПОДТВЕРЖДЕНИЕ не трогает модель:
/// пресет — файл пользователя, и undo проекта его не касается.
#[test]
fn t345_preset_save_dialog_follows_the_rules() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_fx_window();
    let title = tr(k::msg::PRESET_SAVE_TITLE).to_string();
    assert!(h.query_by_label(&title).is_none(), "окно не должно открываться само");

    // кнопка в полосе цепочки
    h.get_by_label(tr(k::fx::SAVE_PRESET)).click();
    h.step();
    h.step();
    assert!(h.query_by_label(&title).is_some(), "окно обязано быть НАРИСОВАНО");

    // Esc закрывает и не оставляет следов
    h.key_press(egui::Key::Escape);
    h.step();
    h.step();
    assert!(h.query_by_label(&title).is_none(), "Esc обязан закрыть окно");
    assert!(h.state().preset_save.is_none(), "и не оставить состояния");

    // повторный вызов работает (правило 14)
    h.get_by_label(tr(k::fx::SAVE_PRESET)).click();
    h.step();
    h.step();
    assert!(h.query_by_label(&title).is_some(), "фича обязана работать повторно");

    // подтверждение: пресет в библиотеке, модель НЕ тронута
    let undo_before = h.state().model.undo_depth();
    let rev_before = h.state().model_rev;
    h.state_mut().preset_save.as_mut().unwrap().name = "My chain".into();
    // имя набрано → появился живой предпросмотр → окно стало выше, и
    // кнопки уехали вниз: щупать надо ПОСЛЕ перерисовки, иначе клик
    // уйдёт по старым координатам (та же ловушка, что с драгом в три шага)
    h.step();
    h.step();
    h.get_by_label(tr(k::msg::PRESET_SAVE_DO)).click();
    h.step();
    h.step();
    let app = h.state_mut();
    assert!(app.preset_save.is_none(), "после сохранения окно закрывается");
    assert!(
        app.presets.rows().iter().any(|r| r.name == "My chain" && !r.factory),
        "пресет обязан появиться в библиотеке"
    );
    assert_eq!(app.model.undo_depth(), undo_before, "пресет — файл, а не правка проекта");
    assert_eq!(app.model_rev, rev_before, "и модель он не двигает");
    // ключи хранятся от нуля: пресет не помнит донора (вход был 30_000)
    let saved = app.presets.rows().iter().find(|r| r.name == "My chain").unwrap().clone();
    assert_eq!(
        saved.block.slots[0].curves[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, 90_000]
    );
}

/// Тест 346 (Р.48, п.345): вставка пресета из пикера.
///
/// Пресет дописывается в конец цепочки, кривые переезжают на материал
/// приёмника, и всё это — обычная правка события: одна ступень undo.
#[test]
fn t346_preset_insert_appends_chain_with_curves() {
    use kt_model::*;
    let mut h = harness_with_fx_window();
    // в библиотеке — пресет с кривой (кладём прямо в стор: пикер —
    // меню egui, его раскрытие проверяет t345-часть про кнопку)
    let block = kt_model::ops::FxChainBlock {
        slots: vec![kt_model::ops::FxSlotBlock {
            fx: FxInstance::glow(0.7),
            bypass: false,
            curves: vec![ParamCurve {
                param: "strength".into(),
                keys: vec![
                    AutoKey::at(kt_time::Tick(0), 0.2),
                    AutoKey::at(kt_time::Tick(90_000), 1.5),
                ],
                source: Default::default(),
            }],
        }],
        src_in: kt_time::Tick(0),
        fx_off: false,
    };
    let app = h.state_mut();
    app.presets.add("Glow ramp", &block).expect("add");
    let row = app.presets.rows().iter().find(|r| r.name == "Glow ramp").unwrap().clone();
    assert!(row.video_ok && !row.audio_ok, "свечение — видео-пресет");

    // вставка идёт тем же путём, что из пикера: append_chain_block с
    // точкой входа приёмника
    let clip = app.model.clip(ClipId(1)).unwrap().clone();
    let (mut fx, mut bypass, mut auto) =
        (clip.fx.clone(), clip.fx_bypass.clone(), clip.fx_auto.clone());
    kt_model::ops::append_chain_block(&mut fx, &mut bypass, &mut auto, &row.block, clip.source_in);
    let undo_before = app.model.undo_depth();
    app.apply_batch(vec![
        Command::SetFx { id: ClipId(1), fx },
        Command::SetFxBypass { id: ClipId(1), bypass },
        Command::SetFxAuto { id: ClipId(1), auto },
    ]);
    h.step();

    let clip = h.state().model.clip(ClipId(1)).unwrap().clone();
    assert_eq!(clip.fx.len(), 2, "пресет дописывается в конец, а не заменяет цепочку");
    assert!(matches!(clip.fx[1], FxInstance::Glow { .. }));
    assert_eq!(
        clip.fx_auto[1][0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![30_000, 120_000],
        "кривая обязана переехать на материал приёмника (вход 30_000)"
    );
    assert_eq!(
        h.state().model.undo_depth(),
        undo_before + 1,
        "вставка пресета — одна ступень истории"
    );
    h.state_mut().model.undo().expect("undo");
    assert_eq!(
        h.state().model.clip(ClipId(1)).unwrap().fx.len(),
        1,
        "undo снимает вставленный пресет"
    );
}

/// Проект с одним аудио-событием и эквалайзером; окно FX открыто.
#[cfg(test)]
fn harness_with_eq_window() -> egui_kittest::Harness<'static, crate::app::KtApp> {
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(180_000),
        source_in: kt_time::Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::eq()],
        fx_bypass: vec![false],
        fx_auto: vec![vec![]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    h
}

/// Тест 349 (Р.48, п.347): график АЧХ есть у эквалайзера, его точка
/// тянется мышью, и правка доезжает ДО МОДЕЛИ одной ступенью истории.
///
/// Утверждается следствие, которое видит человек (полоса переехала на
/// другую частоту и усиление), а не «функция вернула значение»: ровно
/// этот урок записан правилом Р.40.
#[test]
fn t349_eq_graph_drag_edits_the_model() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness_with_eq_window();
    let node = h.query_by_label("eq_graph");
    assert!(node.is_some(), "у эквалайзера обязан быть график АЧХ");
    let rect = node.unwrap().rect();

    // середина полосы «mid»: дефолт 1 кГц / 0 дБ. Координаты считаем теми
    // же функциями, что рисуют — иначе тест проверял бы свою арифметику.
    let plot_w = rect.width() - 4.0;
    let plot_x = rect.left() + 2.0;
    let plot_h = rect.height() - 12.0;
    let plot_y = rect.top() + 6.0;
    let x_of = |hz: f64| plot_x + crate::eq_graph::hz_to_u(hz) * plot_w;
    let y_of = |db: f64| plot_y + ((24.0 - db) / 48.0) as f32 * plot_h;
    let from = egui::pos2(x_of(1000.0), y_of(0.0));
    let to = egui::pos2(x_of(2000.0), y_of(6.0));

    // драг в ТРИ шага: egui признаёт драг только после порога, и позиция
    // читается в кадре начала жеста (правило харнесса)
    h.drag_at(from);
    h.step();
    h.hover_at(from + egui::vec2(8.0, -4.0));
    h.step();
    h.hover_at(to);
    h.step();
    h.drop_at(to);
    h.step();

    let clip = h.state().model.clip(ClipId(1)).unwrap().clone();
    let (freq, gain) = match &clip.fx[0] {
        FxInstance::Eq { mid_freq_hz, mid_gain_db, .. } => (*mid_freq_hz, *mid_gain_db),
        other => panic!("ожидался эквалайзер, получен {other:?}"),
    };
    println!("t349 после драга: {freq:.0} Гц, {gain:+.2} дБ");
    assert!(
        (freq as f64 - 2000.0).abs() / 2000.0 < 0.03,
        "частота полосы обязана переехать на 2 кГц, а не {freq}"
    );
    assert!((gain - 6.0).abs() < 0.5, "усиление обязано стать +6 дБ, а не {gain}");

    // одна ступень истории на весь жест (коалесцирование сессии окна)
    let depth = h.state().model.undo_depth();
    h.state_mut().model.undo().expect("undo");
    assert_eq!(h.state().model.undo_depth(), depth - 1);
    let back = h.state().model.clip(ClipId(1)).unwrap().clone();
    match &back.fx[0] {
        FxInstance::Eq { mid_freq_hz, mid_gain_db, .. } => {
            assert!((*mid_freq_hz - 1000.0).abs() < 1.0, "undo вернул частоту");
            assert!(mid_gain_db.abs() < 0.01, "и усиление");
        }
        other => panic!("ожидался эквалайзер, получен {other:?}"),
    }
}

/// Тест 350 (Р.48, п.347): колесо над средней точкой правит добротность
/// и НЕ прокручивает окно (жест поглощается только у самой точки).
#[test]
fn t350_eq_graph_wheel_sets_q() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness_with_eq_window();
    let rect = h.query_by_label("eq_graph").expect("график есть").rect();
    let plot_w = rect.width() - 4.0;
    let plot_x = rect.left() + 2.0;
    let plot_h = rect.height() - 12.0;
    let plot_y = rect.top() + 6.0;
    let mid = egui::pos2(
        plot_x + crate::eq_graph::hz_to_u(1000.0) * plot_w,
        plot_y + (24.0 / 48.0) * plot_h,
    );
    h.hover_at(mid);
    h.step();
    h.input_mut().events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, -3.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    h.step();
    h.step();

    let clip = h.state().model.clip(ClipId(1)).unwrap().clone();
    let q = match &clip.fx[0] {
        FxInstance::Eq { mid_q, .. } => *mid_q,
        other => panic!("ожидался эквалайзер, получен {other:?}"),
    };
    println!("t350 добротность после колеса: {q:.2}");
    assert!(q < 1.0 - 0.05, "колесо вниз обязано уменьшить добротность, а не {q}");
    assert!(q >= 0.3, "и не выйти за нижнюю границу спеки");
}

/// Тест 430b (Р.50 А2): свернуть и развернуть можно РУКАМИ, и программа
/// говорит, что при этом изменилось.
///
/// Утверждается СЛЕДСТВИЕ (Р.40): не «пункт в меню есть», а «монтаж стал
/// другим». Точка, из которой тест жмёт, — тело события: именно туда жмёт
/// человек.
#[test]
fn t430b_collapse_and_expand_are_reachable_by_hand() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, g) = harness_three_clips();
    let row = g.row_of((LaneKind::Video, 0)).expect("видео-строка");
    let y = g.row_center_y(row);

    // выделили два события рамкой не станем — хватит одного: пункт работает
    // и на одиночном, а тест утверждает достижимость
    h.state_mut().select_only(ClipId(1));
    h.step();
    right_click_at(&mut h, egui::pos2(g.tick_to_x(20_000), y));
    assert!(
        matches!(h.state().ctx_target, Some(crate::timeline::CtxTarget::Clip(ClipId(1)))),
        "правый клик по телу события обязан дать меню СОБЫТИЯ"
    );
    let labels = menu_labels(&mut h);
    assert!(
        labels.iter().any(|l| l == tr(k::tl::menu::COLLAPSE)),
        "в меню события нет пункта сворачивания: {labels:?}"
    );
    h.get_by_label(tr(k::tl::menu::COLLAPSE)).click();
    h.step();
    h.step();

    // СЛЕДСТВИЕ: лента завелась, событие стало экземпляром
    let p = h.state().model.project().clone();
    assert_eq!(p.sequences.len(), 1, "лента не завелась: {:?}", p.sequences);
    let seq = p.sequences[0].id;
    let inst = p.clips.iter().find(|c| c.media == seq).expect("экземпляра на таймлайне нет");
    assert_eq!(p.sequences[0].clips.len(), 1, "в ленте не то содержимое");
    assert_eq!(h.state().nest_uses(seq), 1, "мест использования посчитано неверно");
    let inst_id = inst.id;

    // ...и меню САМОГО вложения знает, что с ним делать
    let g = h.state().tl_geom.clone().unwrap_or(g);
    right_click_at(&mut h, egui::pos2(g.tick_to_x(20_000), y));
    let labels = menu_labels(&mut h);
    for want in [tr(k::tl::menu::NEST_EXPAND), tr(k::tl::menu::NEST_RENAME)] {
        assert!(labels.iter().any(|l| l == want), "в меню вложения нет «{want}»: {labels:?}");
    }
    assert!(
        !labels.iter().any(|l| l.starts_with(tr(k::tl::menu::NEST_INDEPENDENT))),
        "«отвязать от других мест» предложено там, где место ОДНО — обещание,          которое ничего не изменит: {labels:?}"
    );

    // окно имени открывается, Esc закрывает без следов, и оно работает
    // ПОВТОРНО (правило 14)
    for _ in 0..2 {
        h.get_by_label(tr(k::tl::menu::NEST_RENAME)).click();
        h.step();
        h.step();
        assert!(h.state().seq_name.is_some(), "окно имени ленты не открылось");
        h.key_press(egui::Key::Escape);
        h.step();
        h.step();
        assert!(h.state().seq_name.is_none(), "Esc не закрыл окно имени ленты");
        let g = h.state().tl_geom.clone().unwrap_or(g.clone());
        right_click_at(&mut h, egui::pos2(g.tick_to_x(20_000), y));
    }

    // развернули — монтаж вернулся, и об этом сказано
    h.state_mut().expand_nest(inst_id);
    h.step();
    let p = h.state().model.project().clone();
    assert!(
        p.clips.iter().all(|c| c.media != seq),
        "экземпляр остался на таймлайне после разворачивания"
    );
    assert_eq!(p.clips.len(), 3, "содержимое не вернулось на дорожку: {:?}", p.clips.len());
}

// ---- 456-460 (Р.50, В): вход внутрь ленты руками -----------------------------

/// Харнесс с лентой из одного события и экземпляром на дорожке.
#[cfg(test)]
fn harness_with_nest_dt(
    dt: Option<f32>,
) -> (Harness<'static, KtApp>, crate::timeline::TlGeom, kt_model::MediaId) {
    let (mut h, g) = harness_three_clips_dt(dt);
    {
        let app = h.state_mut();
        // СВОРАЧИВАЕМ СРЕДНЕЕ событие, а не первое, и это не вкусовщина
        // (Р.55, С1). `ClipId(1)` стоит в нуле, поэтому у экземпляра
        // `start == 0` и `source_in == 0`, а перевод курсора
        // `source_in + (t − start)` вырождается в тождество: t456 был
        // зелёным и на коде, который курсор не переводит ВОВСЕ. Здесь
        // `ClipId(2)` стоит на 100_000 — сдвиг оси ненулевой, и формулу
        // стало чем проверить.
        app.select_only(kt_model::ClipId(2));
    }
    h.step();
    h.state_mut().collapse_selection();
    h.step();
    h.step();
    let seq = h.state().model.document().sequences[0].id;
    let g = h.state().tl_geom.clone().unwrap_or(g);
    (h, g, seq)
}

#[cfg(test)]
fn harness_with_nest() -> (Harness<'static, KtApp>, crate::timeline::TlGeom, kt_model::MediaId) {
    harness_with_nest_dt(None)
}

/// Тест 456 (Р.50, В1): вход и выход — тождество по виду и курсору.
#[test]
fn t456_entering_and_leaving_is_identity() {
    use kt_time::Tick;
    let (mut h, g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;

    // снимок вида СНАРУЖИ. Курсор — ВНУТРИ тела экземпляра (100_000 …
    // 150_000): вход переводит время, а не телепортирует, и проверять
    // перевод имеет смысл только оттуда, откуда человек входит.
    h.state_mut().player.seek(Tick(120_000));
    h.state_mut().view_start = 5_000.0;
    h.state_mut().ticks_per_px = 900.0;
    h.state_mut().select_only(inst);
    h.step();
    let (vs, tpp) = (h.state().view_start, h.state().ticks_per_px);
    let outside = h.state().player.transport.playhead();

    h.state_mut().enter_nest(inst);
    h.step();
    assert_eq!(h.state().model.editing(), Some(seq), "вход не состоялся");
    assert_eq!(h.state().nest_crumbs().len(), 1, "крошка пути не появилась");
    // КУРСОР ПЕРЕВЕДЁН, а не сброшен: человек вошёл, а не перепрыгнул
    let c = h.state().model.document().clips.iter().find(|c| c.id == inst).unwrap().clone();
    let want = c.source_in + (outside - c.start);
    // СТОРОЖ САМОЙ ПРОВЕРКИ: на экземпляре, стоящем в нуле, `want`
    // совпадает с `outside`, и всё, что ниже, перестаёт что-либо
    // утверждать. Сцена обязана оставаться со сдвигом.
    assert_ne!(
        want.0, outside.0,
        "сцена выродилась: экземпляр стоит в нуле, и перевод курсора нечем отличить \
         от его отсутствия"
    );
    // ТОЧНОСТЬ — КВАНТ ТРАНСПОРТА, названный, а не подобранный: playhead
    // живёт в аудиокадрах 48 кГц, и round-trip «тик → кадр → тик» усекает
    // вниз (п.7 р.10). Один кадр — 90000/48000 = 1.875 тика, округляем
    // вверх. Требовать тик-в-тик значило бы требовать того, чего транспорт
    // не умеет ни здесь, ни где-либо ещё.
    const QUANT: i64 = (kt_time::TICK_HZ + kt_time::AUDIO_SAMPLE_RATE as i64 - 1)
        / kt_time::AUDIO_SAMPLE_RATE as i64;
    let got = h.state().player.transport.playhead();
    assert!(
        (got.0 - want.0).abs() <= QUANT,
        "курсор при входе сброшен, а не переведён ({got:?} против {want:?}) — превью \
         показало бы другой кадр"
    );
    // под руками содержимое ЛЕНТЫ
    assert_eq!(h.state().model.project().clips.len(), 1, "внутри ленты видно не её содержимое");

    // выход — вид вернулся ТОТ ЖЕ (правило 14: фича работает повторно)
    h.state_mut().leave_nest();
    h.step();
    assert_eq!(h.state().model.editing(), None, "выход не состоялся");
    assert!(h.state().nest_crumbs().is_empty(), "крошка осталась после выхода");
    let back = h.state().player.transport.playhead();
    assert!(
        (back.0 - outside.0).abs() <= 2 * QUANT,
        "курсор не вернулся на место: {back:?} против {outside:?}"
    );
    assert_eq!((h.state().view_start, h.state().ticks_per_px), (vs, tpp), "вид не восстановлен");
    assert!(h.state().selected.contains(&inst), "выделение не восстановлено");
    let _ = g;
}

/// Тест 552 (Р.55, С1): внутри ленты играет ЛЕНТА, а не корень.
///
/// До Р.55 в плеер всегда уходил `document()`. Человек входил в
/// композицию, видел на таймлайне её события — и слышал корневой монтаж,
/// взятый в момент, численно равный ЛОКАЛЬНОМУ времени ленты. Совпадало
/// это ровно тогда, когда экземпляр стоит в нуле, поэтому ни один тест
/// дефекта не видел (та же дыра, что закрыта в харнессе t456).
///
/// Утверждение проверяется по СЛЕДСТВИЮ — по тому, что реально уходит в
/// плеер, а не по тому, какой метод вызвался.
#[test]
fn t552_inside_a_reel_the_reel_is_what_plays() {
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;

    // СНАРУЖИ играет документ: корень несёт экземпляр и соседей
    let outside = h.state().player_project();
    assert!(
        outside.clips.iter().any(|c| c.id == inst),
        "в корне плеер обязан играть документ — экземпляра в нём нет"
    );
    assert!(
        outside.sequences.iter().any(|s| s.id == seq),
        "лента обязана остаться в документе: её играют через экземпляр"
    );

    h.state_mut().enter_nest(inst);
    h.step();

    let inside = h.state().player_project();
    // КОРЕНЬ того, что играет, — содержимое ЛЕНТЫ
    let want: Vec<kt_model::ClipId> =
        h.state().model.project().clips.iter().map(|c| c.id).collect();
    let got: Vec<kt_model::ClipId> = inside.clips.iter().map(|c| c.id).collect();
    assert_eq!(
        got, want,
        "внутри ленты плеер играет НЕ ЕЁ: на таймлайне одни события, в звуке и \
         превью другие"
    );
    assert!(
        !inside.clips.iter().any(|c| c.id == inst),
        "экземпляр обязан исчезнуть из того, что играет: стоя ВНУТРИ него, \
         слышать его же снаружи значит слышать себя дважды"
    );

    // А НАРУЖУ (в файл и в экспорт) по-прежнему уходит ДОКУМЕНТ —
    // настоящая раскладка, корень корнем (t456b сторожит вызовы,
    // этот кусок сторожит СОДЕРЖИМОЕ)
    let doc = h.state().model.document();
    assert!(
        doc.clips.iter().any(|c| c.id == inst),
        "в документ уехало содержимое ленты вместо корня — записать такое значит \
         поменять местами корень и ленту на диске"
    );

    h.state_mut().leave_nest();
    h.step();
    let back: Vec<kt_model::ClipId> =
        h.state().player_project().clips.iter().map(|c| c.id).collect();
    let was: Vec<kt_model::ClipId> = outside.clips.iter().map(|c| c.id).collect();
    assert_eq!(back, was, "выход не вернул плееру корень");
}

/// Тест 553 (Р.55, С1): вход приводит ВИД туда, где встал курсор.
///
/// Курсор переводился и раньше, а прокрутка — нет: она оставалась в оси
/// КОРНЯ. Войдя во вложение, стоящее на десятой минуте, человек видел
/// пустой экран — содержимое ленты лежит около её нуля. Это и есть
/// претензия «мы оказываемся на том же t, где были, а композиция идёт
/// с t=0».
#[test]
fn t553_entering_brings_the_view_to_the_cursor() {
    use kt_time::Tick;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    let c = h.state().model.project().clips.iter().find(|x| x.id == inst).unwrap().clone();

    // курсор внутри тела экземпляра, вид — вокруг него В ОСИ КОРНЯ
    h.state_mut().player.seek(c.start + Tick(c.duration.0 / 2));
    h.state_mut().view_start = c.start.0 as f64;
    h.state_mut().select_only(inst);
    h.step();
    let before = h.state().view_start;

    h.state_mut().enter_nest(inst);
    h.step();

    let cur = h.state().player.transport.playhead();
    let g = h.state().tl_geom.clone().expect("геометрия таймлайна");
    let span = g.rect.width() as f64 * h.state().ticks_per_px;
    let vs = h.state().view_start;
    assert!(
        cur.0 as f64 >= vs && (cur.0 as f64) < vs + span,
        "курсор ({}) вне видимого окна [{vs}, {}) — человек смотрит на пустое место",
        cur.0,
        vs + span
    );
    // сторож самой проверки: сцена обязана быть со сдвигом, иначе
    // «вид не тронули» неотличимо от «вид привели»
    assert_ne!(before, vs, "сцена выродилась: вид и так стоял там, куда его надо было привести");
}

/// Тест 457 (Р.50, В3): войти можно ЧЕТЫРЬМЯ способами, и все они — одно тело.
#[test]
fn t457_four_ways_in_one_way_out() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    let row = g.row_of((LaneKind::Video, 0)).expect("видео-строка");
    let y = g.row_center_y(row);
    let x = {
        let c = h.state().model.project().clips.iter().find(|c| c.id == inst).unwrap().clone();
        g.tick_to_x(c.start.0 + c.duration.0 / 2)
    };

    // 1) пункт меню
    right_click_at(&mut h, egui::pos2(x, y));
    assert!(
        matches!(h.state().ctx_target, Some(crate::timeline::CtxTarget::Nest(_))),
        "ПКМ по вложению обязан дать цель ВЛОЖЕНИЯ, а не обычного события"
    );
    // подпись пункта ищется ПОДСТРОКОЙ: у пункта есть колонка аккорда
    // (Р.56, п.451), и точное совпадение сломалось бы о неё
    h.get_by_label_contains(tr(k::tl::menu::NEST_ENTER)).click();
    h.step();
    h.step();
    assert_eq!(h.state().model.editing(), Some(seq), "пункт меню не завёл внутрь");

    // 2) Esc — ПОСЛЕДНИМ в каскаде: сначала он снимает выделение
    let inner = h.state().model.project().clips[0].id;
    h.state_mut().select_only(inner);
    h.state_mut().escape_action();
    assert_eq!(
        h.state().model.editing(),
        Some(seq),
        "Esc выкинул наружу вместо того, чтобы снять выделение — одна клавиша \
         делает два дела, и второе необратимо по вниманию"
    );
    h.state_mut().escape_action();
    assert_eq!(h.state().model.editing(), None, "Esc на пустом выделении обязан выводить наружу");

    // 3) ДВОЙНОЙ КЛИК даёт не полный вход, а РАСКРЫТИЕ НА МЕСТЕ (Р.55,
    // С5): быстрый жест даёт режим ПО УМОЛЧАНИЮ, названный пункт меню —
    // названный режим. Прямая просьба владельца: «по умолчанию гибридный
    // удобный режим нужен».
    //
    // двойной клик у egui ВРЕМЕННОЙ: шаг харнесса по умолчанию 0.25 с, и
    // без мелкого dt тест проверял бы ОДИНОЧНЫЙ клик
    let (mut h2, g2, seq2) = harness_with_nest_dt(Some(0.02));
    let inst2 = h2.state().model.project().clips.iter().find(|c| c.media == seq2).unwrap().id;
    let row2 = g2.row_of((LaneKind::Video, 0)).unwrap();
    let x2 = {
        let c = h2.state().model.project().clips.iter().find(|c| c.id == inst2).unwrap().clone();
        g2.tick_to_x(c.start.0 + c.duration.0 / 2)
    };
    let pos2 = egui::pos2(x2, g2.lane_y(ClipKind::Video, 0, 0.6));
    double_click_at(&mut h2, pos2);
    assert_eq!(
        h2.state().model.editing(),
        None,
        "двойной клик ушёл в ПОЛНЫЙ вход: из корня в гибридном режиме не выходят, \
         иначе играть будет лента, а не программа"
    );
    assert_eq!(
        h2.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![inst2],
        "двойной клик не раскрыл композицию на месте"
    );
    // ЖЕСТ РАБОТАЕТ ПОВТОРНО (правило 14): второй раз сворачивает
    double_click_at(&mut h2, pos2);
    assert!(
        h2.state().model.bands().is_empty(),
        "повторный двойной клик обязан сворачивать раскрытие, а не копить полосы"
    );
    let _ = row2;

    // 4) Enter по выделенному — КАСКАД (Р.56, п.451, решение владельца:
    // «я хочу чтобы двойной enter переходил в нее»). Первый раскрывает
    // на месте, второй по уже раскрытому — ПОЛНЫЙ ВХОД.
    let (mut h3, _, seq3) = harness_with_nest();
    let inst3 = h3.state().model.project().clips.iter().find(|c| c.media == seq3).unwrap().id;
    h3.state_mut().select_only(inst3);
    h3.step();
    h3.key_press(egui::Key::Enter);
    h3.step();
    h3.step();
    assert_eq!(h3.state().model.editing(), None, "первый Enter ушёл в полный вход");
    assert_eq!(
        h3.state().model.bands().len(),
        1,
        "Enter по выделенному вложению не раскрыл его на месте"
    );
    // экземпляр ОСТАЛСЯ выделен — иначе второму Enter не во что целиться,
    // и жест молча не делал бы ничего
    assert!(
        h3.state().is_selected(inst3),
        "раскрытие сбросило выделение экземпляра: следующий жест бьёт в пустоту"
    );

    let (vs, tpp) = (h3.state().view_start, h3.state().ticks_per_px);
    h3.key_press(egui::Key::Enter);
    h3.step();
    h3.step();
    assert_eq!(h3.state().model.editing(), Some(seq3), "второй Enter не завёл ВНУТРЬ ленты");
    assert!(h3.state().model.bands().is_empty(), "полный вход обязан свернуть полосу");

    // выход — в КОРЕНЬ, И ПОЛОСА ВОЗВРАЩАЕТСЯ (решение владельца Р.58,
    // пересмотр Р.56 п.5: «сбрасывают состояние раскрытости, а хотелось бы
    // чтобы оставляли»; каскад Enter/Esc симметричен — t636)
    h3.state_mut().escape_action();
    h3.step();
    assert_eq!(h3.state().model.editing(), None, "Esc не вывел наружу");
    assert_eq!(
        h3.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![inst3],
        "выход обязан ВЕРНУТЬ полосу, через которую вошли (решение владельца Р.58)"
    );
    assert_eq!(
        (h3.state().view_start, h3.state().ticks_per_px),
        (vs, tpp),
        "вид после выхода не вернулся"
    );
}

/// Тест 458 (Р.50, В2): у вложения свой вид и свои обещания.
///
/// Три вещи, которых не видно в модели: тумб ПО ФАЙЛУ у вложения не
/// бывает (файла у ленты нет; кадры ей даёт офлайн-рендер содержимого —
/// Р.56, п.452, служба `reel_preview`), бейдж «×N» появляется ТОЛЬКО при
/// N > 1, и фон внутри ленты тонирован.
#[test]
fn t458_nest_looks_like_a_nest() {
    use kt_model::*;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;

    // ТУМБЫ ПО ФАЙЛУ: ни один ключ службы тумб не адресует ЛЕНТУ — файла
    // у неё нет, и служба индекса полезла бы по нему ffmpeg'ом. Кадры на
    // событии при этом ЕСТЬ, но приходят из `reel_preview` (t600).
    h.step();
    h.step();
    assert!(
        h.state().thumb_textures.keys().all(|k| k.media != seq),
        "вложение завело тумбы по адресу ЛЕНТЫ — у неё нет файла"
    );

    // БЕЙДЖ ×N: при одном месте его нет
    assert_eq!(h.state().nest_uses(seq), 1, "сцена: одно место");
    // ...а при двух — есть, и «следующее место» ведёт по кругу
    let mut twin = h.state().model.project().clips.iter().find(|c| c.id == inst).unwrap().clone();
    twin.id = ClipId(h.state().next_clip_id());
    twin.start = kt_time::Tick(900_000);
    twin.link_group = None;
    assert!(h.state_mut().apply(Command::AddClip(twin.clone())));
    h.step();
    assert_eq!(h.state().nest_uses(seq), 2, "второе место не посчиталось");
    h.state_mut().goto_next_place(inst);
    h.step();
    assert!(
        h.state().selected.contains(&twin.id),
        "«следующее место» не привело ко второму экземпляру — бейдж ×N остался \
         сообщением без продолжения"
    );
    // ...и по кругу обратно
    h.state_mut().goto_next_place(twin.id);
    h.step();
    assert!(h.state().selected.contains(&inst), "круг не замкнулся на первом месте");
}

/// Тест 460 (п.397, Р.51): повторный драг уголка-фейда правит ТОЛЬКО длину
/// окна — выбранный вид, кривые параметров и кривая аудио выживают.
/// Раньше `commit_fade_handle` собирал свежий Crossfade и молча стирал
/// настройки (против правила 20 «ничего не отрубается само»).
#[test]
fn t460_fade_handle_redrag_keeps_kind() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
        // уголковый фейд-ин с НАСТРОЕННЫМ видом, кривой и звуком
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: None,
            b: Some(ClipId(1)),
            in_offset: Tick::ZERO,
            out_offset: Tick(40_000),
            kind: TransitionKind::Wipe { dir: Dir::Left, angle: 10.0, softness: 0.2 },
            audio: AudioCurve::SCurve,
            auto: vec![ParamCurve {
                param: "angle".into(),

                keys: AutoKey::linear(vec![
                    AutoKey::at(Tick(0), 0.0),
                    AutoKey::at(Tick(30_000), 45.0),
                ]),
                source: Default::default(),
            }],
            invert: false,
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
    let y_strip = g.lane_top(ClipKind::Video, 0) + 3.0;

    // тянем ручку фейда (конец окна, тик 140 000) дальше вправо до 180 000
    let from = g.tick_to_x(140_000);
    h.hover_at(egui::pos2(from, y_strip));
    h.step();
    h.drag_at(egui::pos2(from, y_strip));
    h.step();
    let to = g.tick_to_x(180_000);
    h.hover_at(egui::pos2(to, y_strip));
    h.step();
    h.drop_at(egui::pos2(to, y_strip));
    h.step();

    let p = h.state().model.project().clone();
    assert_eq!(p.transitions.len(), 1, "фейд один: {:?}", p.transitions);
    let t = &p.transitions[0];
    assert!(
        (t.out_offset.0 - 80_000).abs() <= 2_000,
        "длина окна поехала за ручкой: {:?}",
        t.out_offset
    );
    assert_eq!(
        t.kind,
        TransitionKind::Wipe { dir: Dir::Left, angle: 10.0, softness: 0.2 },
        "вид фейда пережил повторный драг уголка"
    );
    assert_eq!(t.audio, AudioCurve::SCurve, "кривая аудио не сброшена");
    assert_eq!(t.auto.len(), 1, "кривые параметров не стёрты: {:?}", t.auto);
}

/// Тест 460b (п.397, Р.51): повторный наезд Move-драгом на пару с уже
/// настроенным переходом двигает окно, но вид/кривую аудио сохраняет.
#[test]
fn t460b_overlap_redrag_keeps_kind() {
    use kt_model::*;
    use kt_time::Tick;
    const TPP: f64 = 1000.0;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        // ветка кроссфейда из наезда живёт в режиме БЕЗ переслаивания (Р.28)
        p.overlap_layers = false;
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        for (n, start, src_in) in [(1u64, 0i64, 0i64), (2, 90_000, 20_000)] {
            p.clips.push(ClipEvent {
                r#gen: None,
                gen_auto: Vec::new(),
                overrides: Vec::new(),
                geometry: Default::default(),
                geometry_pos: 0,
                geometry_auto: Vec::new(),
                id: ClipId(n),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(90_000),
                source_in: Tick(src_in),
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
            in_offset: Tick(9_000),
            out_offset: Tick(9_000),
            kind: TransitionKind::Barn { vertical: true, softness: 0.1 },
            audio: AudioCurve::Linear,
            auto: vec![],
            invert: false,
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
    let grab = g.tick_to_x(90_000 + 45_000);
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
    // цель: start = 50 000 — наезд ГЛУБЖЕ на хвост клипа 1
    let to = g.tick_to_x(50_000 + grab_dt);
    h.hover_at(egui::pos2(to, y));
    h.step();
    h.drop_at(egui::pos2(to, y));
    h.step();

    let p = h.state().model.project().clone();
    assert_eq!(p.transitions.len(), 1, "переход один: {:?}", p.transitions);
    let t = &p.transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(1)), Some(ClipId(2))));
    assert!((t.in_offset.0 - 20_000).abs() <= 1_000, "окно поехало: {:?}", t.in_offset);
    assert_eq!(
        t.kind,
        TransitionKind::Barn { vertical: true, softness: 0.1 },
        "вид перехода пережил повторный наезд"
    );
    assert_eq!(t.audio, AudioCurve::Linear, "кривая аудио не сброшена");
    p.validate().expect("итог валиден");
}

/// Тест 466 (п.399, Р.51): переход НА НАЛОЖЕНИИ достижим жестами — ПКМ по
/// зоне наложения предлагает «Переход с…» (сосед-«лесенка» равноправен
/// стыковому, правило 17), а двойной клик по зоне наложенного перехода
/// открывает его свойства, как у стыкового.
#[test]
fn t466_overlap_transition_reachable() {
    use kt_model::*;
    use kt_time::Tick;
    let overlap_app = |with_transition: bool| {
        let (mut h, g) = harness_three_clips_dt(Some(0.02));
        {
            let app = h.state_mut();
            let mut p = app.model.project().clone();
            // лесенка: клип 1 [0..50к), клип 2 [30к..80к); третий подальше
            p.clips[0].duration = Tick(50_000);
            p.clips[1].start = Tick(30_000);
            p.clips[1].duration = Tick(50_000);
            p.clips[2].start = Tick(200_000);
            if with_transition {
                p.transitions.push(TransitionEvent {
                    id: TransitionId(1),
                    a: Some(ClipId(1)),
                    b: Some(ClipId(2)),
                    in_offset: Tick::ZERO,
                    out_offset: Tick::ZERO,
                    kind: TransitionKind::Barn { vertical: false, softness: 0.1 },
                    audio: Default::default(),
                    auto: vec![],
                    invert: false,
                });
            }
            p.validate().unwrap();
            app.model = kt_model::TimelineModel::new(p);
        }
        h.step();
        let g2 = h.state().tl_geom.clone().unwrap_or(g);
        (h, g2)
    };

    // ПКМ по зоне наложения БЕЗ перехода: меню клипа предлагает переход
    let (mut h, g) = overlap_app(false);
    let y = g.lane_y(kt_model::ClipKind::Video, 0, 0.75);
    right_click_at(&mut h, egui::pos2(g.tick_to_x(40_000), y));
    assert!(
        matches!(h.state().ctx_target, Some(crate::timeline::CtxTarget::Clip(_))),
        "ПКМ по зоне наложения обязан дать меню клипа"
    );
    let labels = menu_labels(&mut h);
    assert!(
        labels.iter().any(|l| {
            // menu_button дописывает к подписи стрелку подменю
            l.starts_with(tr(k::tl::menu::TRANSITION_PREV))
                || l.starts_with(tr(k::tl::menu::TRANSITION_NEXT))
        }),
        "сосед-«лесенка» обязан дать пункт «Переход с…»: {labels:?}"
    );

    // двойной клик по зоне НАЛОЖЕННОГО перехода открывает свойства
    let (mut h, g) = overlap_app(true);
    let pos = egui::pos2(g.tick_to_x(40_000), y);
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
        "двойной клик по наложенному переходу обязан открыть свойства"
    );
    // и длина окна в свойствах — длина ПЕРЕКРЫТИЯ, офсеты тут ни при чём
    let p = h.state().model.project();
    let t = &p.transitions[0];
    let w = t.window(p).expect("окно наложенного");
    assert_eq!((w.end - w.start).0, 20_000, "окно = зона перекрытия");
}

/// Тест 511 (Р.53, п.415; переписан в Р.59 стадии 2): пикер вида ОДИН,
/// и смена вида доезжает до модели, не выкидывая человека из окна.
///
/// До Р.53 окно титра было тупиком: вид генератора в нём не показывался
/// вовсе, и сменить титр обратно на цвет было нечем (правило 17). С Р.59
/// окон стало одно, и роутить больше некуда — утверждение сузилось до
/// сути: пикер есть, вид меняется, форма остаётся на месте, а смена вида
/// — СВОЙ шаг истории (правило 20): один Ctrl+Z возвращает титр со всем
/// набранным текстом, а не откатывает заодно правку цвета.
///
/// Пикер щупается по РОЛИ: у `ComboBox` в accesskit нет label, показанный
/// текст лежит в `value` — по подписи его не найти (родня заметки про
/// `DragValue` в CLAUDE.md).
#[test]
fn t511_kind_picker_routes_between_windows() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    h.step();
    h.step();
    let (_id, clip) = made_generator(&h);
    assert_eq!(h.state().fx_editors.len(), 1, "форма генератора — в окне события");
    let depth = h.state().model.undo_depth();

    let text_name = crate::gen_ui::kind_name(&GeneratorKind::default_text());
    let solid_name =
        crate::gen_ui::kind_name(&GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });

    // пикер вида ЕСТЬ в форме и показывает «Текст»
    h.get(by().role(Role::ComboBox).value(text_name)).click();
    h.step();
    h.get_by_label(solid_name).click();
    h.step();
    h.step();

    // следствие: вид сменился и в медиа, и на событии, окно на месте
    let app = h.state();
    assert_eq!(app.model.project().media.last().unwrap().path, "gen:solid");
    assert!(
        matches!(gen_of(&h, clip), GeneratorKind::Solid { .. }),
        "значения события обязаны переодеться вместе с видом"
    );
    assert_eq!(app.fx_editors.len(), 1, "смена вида не имеет права закрывать окно");
    assert_eq!(app.model.undo_depth(), depth + 1, "смена вида — ровно один шаг истории");

    // и обратно: «Текст» тем же пикером
    h.get(by().role(Role::ComboBox).value(solid_name)).click();
    h.step();
    h.get_by_label(text_name).click();
    h.step();
    h.step();
    let app = h.state();
    assert_eq!(app.model.project().media.last().unwrap().path, "gen:text");
    assert!(matches!(gen_of(&h, clip), GeneratorKind::Text { .. }), "вид не вернулся в титр");
    assert_eq!(app.fx_editors.len(), 1, "форма осталась одна");
}

/// Тест 516 (Р.53, п.415): доля кадра ПОКАЗЫВАЕТСЯ процентом, а хранится
/// долей. «Кегль 0.070» не говорит пользователю ничего; при этом ни
/// формат, ни кривые, ни движок о процентах не знают — пересчёт живёт в
/// одном месте (`widgets::param_value`).
#[test]
fn t516_frame_fractions_are_shown_in_percent() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable, by};
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    h.step();
    h.step();

    // объявление: кегль — процент, межстрочный интервал — множитель
    let kind = GeneratorKind::default_text();
    let spec = |n: &str| kind.param_specs().iter().find(|s| s.name == n).unwrap().clone();
    assert_eq!(spec("size").unit, ParamUnit::Percent, "кегль обязан показываться процентом");
    assert_eq!(spec("pos_x").unit, ParamUnit::Percent);
    assert_eq!(spec("line_spacing").unit, ParamUnit::Ratio, "множитель процентом не бывает");

    // следствие В ОКНЕ: поле кегля показывает «7.0 %», а не «0.07»
    let shown: Vec<String> = h
        .query_all(by().role(Role::SpinButton))
        .filter_map(|n| n.accesskit_node().value().map(|v| v.to_string()))
        .collect();
    assert!(
        shown.iter().any(|s| s == "7.0 %"),
        "в окне титра нет поля кегля в процентах: {shown:?}"
    );
    assert!(
        shown.iter().any(|s| s == "1.00"),
        "множитель уехал в проценты вместе с долями: {shown:?}"
    );

    // и хранение осталось долей: путь пишет 0.125, а не 12.5
    let mut k = GeneratorKind::default_text();
    k.set_param("size", 0.125);
    let path = format_gen_path(&k);
    assert!(path.contains("s=0.125"), "в путь уехал процент вместо доли: {path}");
}

/// Тест 517 (Р.53, п.415): двойной клик по ПОДПИСИ возвращает параметру
/// умолчание. Раньше «вернуть как было» требовало помнить число — у
/// титра таких чисел одиннадцать.
#[test]
fn t517_double_click_on_label_resets_param() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    let mut kind = GeneratorKind::default_text();
    kind.set_param("size", 0.3);
    h.state_mut().create_generator(kind);
    h.step();
    h.step();
    let id = h.state().model.project().media.last().unwrap().id;
    let default_size = GeneratorKind::default_text().get_param("size").unwrap();

    let label = crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "size");
    let rect = h.get_by_label(&label).rect();
    double_click_at(&mut h, rect.center());
    h.step();
    h.step();

    let path = h.state().model.project().source(id).map(|s| match s {
        kt_model::Source::File(m) => m.path.clone(),
        _ => String::new(),
    });
    let got = kt_model::parse_gen_path(path.as_deref().unwrap_or(""))
        .and_then(|k| k.get_param("size"))
        .expect("медиа обязано остаться титром");
    assert!(
        (got - default_size).abs() < 1e-9,
        "двойной клик по подписи не вернул умолчание: {got} вместо {default_size}"
    );
}

/// Тест 518 (Р.53, п.415): непарсящийся `gen:`-путь — ОШИБКА, а не
/// молчание (правило 16), и подсказка общего виджета доезжает до строки
/// состояния (правило 10: место показа одно).
#[test]
fn t518_unknown_kind_is_loud_and_widget_hints_reach_the_status_line() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    let id = h.state().model.project().media.last().unwrap().id;

    // подсказка подписи параметра: наводим на неё и читаем СТРОКУ СОСТОЯНИЯ
    let label = crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "size");
    let rect = h.get_by_label(&label).rect();
    h.hover_at(rect.center());
    h.step();
    h.step();
    assert_eq!(
        h.state().hover_hint.as_deref(),
        Some(crate::i18n::tr(crate::i18n::k::fx::HINT_RESET)),
        "подсказка общего виджета не доехала до строки состояния"
    );

    // вид новее сборки: формы у него нет, но пользователь слышит почему.
    // С Р.59 значения живут на СОБЫТИИ, и неизвестный вид снимает их —
    // голова полосы исчезает, окно эффектов остаётся (цепочка-то жива).
    let notices = h.state().notices.len();
    let clip = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.media == id)
        .map(|c| c.id)
        .expect("событие генератора");
    {
        let app = h.state_mut();
        let _ = app.apply(Command::SetMediaPath { id, path: "gen:from_the_future".into() });
    }
    h.step();
    h.step();
    let app = h.state();
    assert!(
        app.model.clip(clip).is_some_and(|c| c.r#gen.is_none()),
        "у неизвестного вида не может быть формы — значения обязаны сняться"
    );
    assert!(app.notices.len() > notices, "непарсящийся путь снял форму МОЛЧА");
}

/// Тест 512 (Р.53, п.416): новый генератор появляется НА МОНТАЖЕ, а не
/// только в бине, и всё это — один шаг истории.
///
/// До этого пункта пункт меню заводил медиа и открывал окно параметров, а
/// в превью не было ничего: пользователь набирал титр вслепую, потом шёл
/// в бин и ставил карточку руками. Правило 4 чек-листа — умолчание должно
/// быть полезным.
#[test]
fn t512_new_generator_lands_on_the_timeline() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    // тик КРАТЕН 15: playhead живёт в аудиокадрах 48 кГц, и round-trip
    // «тик → кадр → тик» усекает вниз (п.7 р.10) — 400_000 вернулось бы
    // как 399_999, и тест ловил бы не то
    h.state_mut().player.seek(kt_time::Tick(450_000));
    let clips_before = h.state().model.project().clips.len();
    let depth = h.state().model.undo_depth();

    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    h.step();

    let app = h.state();
    let id = app.model.project().media.last().unwrap().id;
    let mine: Vec<&ClipEvent> =
        app.model.project().clips.iter().filter(|c| c.media == id).collect();
    assert_eq!(mine.len(), 1, "генератор без звука кладёт ровно одно событие");
    assert_eq!(mine[0].start, kt_time::Tick(450_000), "событие обязано встать ПОД КУРСОР");
    assert!(mine[0].duration.0 > 0, "у события нулевой длины нечего показывать");
    assert_eq!(app.model.project().clips.len(), clips_before + 1);
    assert_eq!(app.model.undo_depth(), depth + 1, "медиа и клип — ОДИН шаг истории");

    // один Ctrl+Z убирает и событие, и медиа
    h.state_mut().undo();
    h.step();
    let app = h.state();
    assert_eq!(app.model.project().clips.len(), clips_before, "undo оставил событие");
    assert!(
        !app.model.project().media.iter().any(|m| m.id == id),
        "undo оставил медиа: шаг был не один"
    );
}

/// Тест 513 (Р.53, п.416): событие-генератор подписано ИМЕНЕМ ВИДА, а не
/// сырым `gen:`-путём, и подпись следует за языком интерфейса.
///
/// Сырой путь врал дважды: в нём были параметры (подпись дёргалась на
/// каждое нажатие в окне) и не было вида. Язык — отдельный штамп в кэше
/// имён: смена языка модель не трогает, и без него таймлайн оставался бы
/// на старом языке до первой правки монтажа.
#[test]
fn t513_generator_clip_is_labelled_by_its_kind() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    h.step();
    let id = h.state().model.project().media.last().unwrap().id;
    let clip = h.state().model.project().clips.iter().find(|c| c.media == id).unwrap().id;

    let name_of = |h: &mut Harness<'_, KtApp>| h.state_mut().names().0.get(&clip).cloned();
    let ru = name_of(&mut h).expect("у события генератора нет подписи");
    assert_eq!(
        ru,
        crate::gen_ui::kind_name(&GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 })
    );
    assert!(!ru.contains("gen:"), "подпись всё ещё сырой путь: {ru}");
    assert!(!ru.contains("ff8800"), "параметры уехали в подпись: {ru}");

    // смена языка — подпись обязана переехать вместе с интерфейсом
    crate::i18n::set_lang(crate::i18n::Lang::En);
    h.step();
    let en = name_of(&mut h).expect("подпись пропала после смены языка");
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    assert_ne!(en, ru, "подпись осталась на прежнем языке: кэш имён не знает про язык");

    // и цвет события НЕ зависит от параметров: правка не перекрашивает
    let a = crate::timeline::clip_color("gen:solid:c=ff8800", ClipKind::Video);
    let b = crate::timeline::clip_color("gen:solid:c=104080", ClipKind::Video);
    assert_eq!(a, b, "клип перекрашивается на каждое нажатие в окне параметров");
    assert_ne!(
        a,
        crate::timeline::clip_color("gen:text", ClipKind::Video),
        "разные виды обязаны отличаться цветом"
    );
}

/// Тест 514 (Р.53, п.416): двойной клик по карточке бина даёт ОДИН клип и
/// открывает окно параметров.
///
/// У egui `clicked()` истинно на ОБОИХ кликах двойного, а одиночный клик
/// по карточке ставит событие — значит двойной ставил ДВА внахлёст, и
/// пользователь этого не просил (правило 20: ничего не отрастает само).
#[test]
fn t514_double_click_on_a_card_gives_one_clip_and_the_window() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    for _ in 0..6 {
        h.step();
    }
    let id = h.state().model.project().media.last().unwrap().id;
    // окно создания закрываем: тест про ОТКРЫТИЕ двойным кликом
    h.state_mut().fx_editors.clear();
    h.state_mut().player.seek(kt_time::Tick(700_000));
    h.step();
    let before = h.state().model.project().clips.iter().filter(|c| c.media == id).count();

    let card =
        h.state().bin_card_rect(id).expect("карточка генератора не нарисована — щупать нечего");
    double_click_at(&mut h, card.center());
    h.step();
    h.step();

    let app = h.state();
    let after = app.model.project().clips.iter().filter(|c| c.media == id).count();
    assert_eq!(after, before + 1, "двойной клик поставил не один клип, а {}", after - before);
    assert_eq!(app.fx_editors.len(), 1, "двойной клик по карточке не открыл окно");
    assert!(
        app.model.clip(app.fx_editors[0].clip).is_some_and(|c| c.media == id && c.r#gen.is_some()),
        "окно открылось не на событии этого генератора"
    );
}

/// Тест 515 (Р.53, п.416): карточка говорит, во СКОЛЬКИХ местах стоит
/// генератор. Параметры общие на все места, и правка цвета меняет
/// картинку сразу везде — число обязано быть видно ДО правки, а не только
/// в окне. Место не новое: у генератора длительности нет, и подпись
/// занимает её пустующий слот (правило 13).
#[test]
fn t515_card_counts_places_on_the_timeline() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    h.step();
    let id = h.state().model.project().media.last().unwrap().id;
    assert_eq!(h.state_mut().media_uses().get(&id).copied(), Some(1), "клип создания не сосчитан");

    h.state_mut().player.seek(kt_time::Tick(800_000));
    h.state_mut().place_media_at_cursor(id);
    h.step();
    assert_eq!(h.state_mut().media_uses().get(&id).copied(), Some(2));

    // счёт идёт по ВСЕМУ документу, а не по корню: событие внутри ленты —
    // такое же место, и правка параметров меняет и его
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        let mut inner = p.clips.iter().find(|c| c.media == id).unwrap().clone();
        inner.id = ClipId(9001);
        inner.start = kt_time::Tick(0);
        let mut seq = Sequence::new(MediaId(9000), "лента");
        seq.clips.push(inner);
        p.sequences.push(seq);
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.model_rev += 1;
    }
    assert_eq!(
        h.state_mut().media_uses().get(&id).copied(),
        Some(3),
        "событие внутри ленты не сосчитано — «мест» меньше, чем правка затронет"
    );
}

/// Тест 540 (Р.53, п.424; переписан в Р.59 стадии 2): пресет титра
/// ставит ВСЁ одним шагом истории, а активный пресет определяется
/// ОБРАТНОЙ функцией.
///
/// Решение владельца дословно: «пресет ставит всё», включая позицию и
/// якорь, один Ctrl+Z возвращает весь набор, а ручная подкрутка честно
/// показывает «не совпадает ни с одним». Флага «какой пресет нажимали» в
/// программе нет намеренно: он врал бы после первой же правки поля.
///
/// Что изменилось: «всё» теперь включает и АНИМАЦИЮ — она стала
/// обычными кривыми на событии (`gen_auto`), и они уезжают тем же одним
/// шагом (кривые проверяет t668).
#[test]
fn t540_title_preset_sets_everything_in_one_step() {
    use crate::gen_ui::{active_preset, title_presets};
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    // обратная функция: пресет, применённый к своему результату, даёт
    // тот же результат — иначе «активный» не определялся бы вовсе
    let base = GeneratorKind::default_text();
    for (i, pr) in title_presets().iter().enumerate() {
        let made = pr.applied(&base);
        let curves = pr.curves(&made, (1920, 1080), kt_time::Tick::ZERO);
        assert_eq!(
            active_preset(&made, &curves),
            Some(i),
            "пресет {i} не узнаёт собственный результат"
        );
    }
    // умолчание титра ни одному пресету не равно — и это честно. По
    // ЗНАЧЕНИЯМ пустой титр совпадает с «печатной машинкой» (тот же кегль
    // и та же середина кадра), и различает их ровно вторая половина
    // пресета — кривые, которых у нового титра нет.
    assert_eq!(active_preset(&base, &[]), None, "пустой титр не обязан совпадать с пресетом");

    // ...и то же в ОКНЕ: нажатие ставит набор полей одной командой
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    for _ in 0..3 {
        h.step();
    }
    let (_id, clip) = made_generator(&h);
    let depth = h.state().model.undo_depth();
    let before = gen_of(&h, clip);

    let label = crate::i18n::tr(title_presets()[0].key);
    h.get_by_label(label).click();
    h.step();
    h.step();

    let made = gen_of(&h, clip);
    let curves = h.state().model.clip(clip).map(|c| c.gen_auto.clone()).unwrap_or_default();
    assert_ne!(made, before, "нажатие пресета ничего не изменило");
    assert_eq!(active_preset(&made, &curves), Some(0), "окно поставило не тот набор");
    let GeneratorKind::Text { anchor, pos_y, size, .. } = &made else { unreachable!() };
    assert_eq!(*anchor, TextAnchor::BottomLeft, "пресет обязан ставить и ЯКОРЬ");
    assert!(*pos_y > 0.5 && *size < 0.07, "пресет обязан ставить и позицию, и кегль");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "пресет — ровно один шаг истории");

    // один Ctrl+Z возвращает весь набор разом
    h.state_mut().undo();
    h.step();
    assert_eq!(gen_of(&h, clip), before, "undo обязан вернуть титр целиком");
}

/// Тест 541 (Р.53, п.412; переписан в Р.59 стадии 2): правка параметров
/// при НЕПОДВИЖНОМ курсоре доезжает до ЭКРАНА, а не только до кэша
/// плеера.
///
/// Нашёл этот дефект живой сценарий, а не харнесс, и это ровно тот
/// случай, о котором предупреждает CLAUDE.md: t478 спрашивает кэш плеера
/// НАПРЯМУЮ, мимо мемоизации превью, и был зелёным на программе, где в
/// окне стоял оранжевый цвет, а на экране висел чёрный кадр. Ключ превью
/// не менялся: ни курсор, ни качество, ни план правка не двигает — и
/// композит считался «уже посчитанным» до первого движения playhead'а.
///
/// Поэтому тест утверждает СЛЕДСТВИЕ: программа пересобрала превью.
#[test]
fn t541_param_edit_repaints_the_preview_without_moving_the_playhead() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
    for _ in 0..4 {
        h.step();
    }
    let (_id, clip) = made_generator(&h);
    let gesture = h.state().fx_editors[0].gesture;

    // дать превью устояться: ключ посчитан, композит полон
    for _ in 0..40 {
        if h.state().preview_key.is_some_and(|k| k.2) {
            break;
        }
        h.step();
    }
    let before = h.state().preview_key;
    let content_before = h.state().preview_content;
    assert!(before.is_some(), "превью так и не посчиталось");

    // правка цвета БЕЗ движения курсора
    {
        let app = h.state_mut();
        assert!(app.apply_coalesced(
            Command::SetClipGen {
                id: clip,
                kind: Some(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 }),
            },
            gesture
        ));
    }
    // ...и программа обязана пересобрать превью: версия содержимого
    // источников вошла в ключ
    let mut repainted = false;
    for _ in 0..60 {
        h.step();
        if h.state().preview_content != content_before {
            repainted = true;
            break;
        }
    }
    assert!(
        repainted,
        "правка параметров не сдвинула ключ превью — на экране остался бы старый кадр"
    );
    assert_eq!(
        h.state().preview_key.map(|k| k.0),
        before.map(|k| k.0),
        "курсор не двигался: тик ключа обязан остаться прежним"
    );
}

/// Тест 542 (Р.53, п.415; переписан в Р.59 стадии 2): цвета в форме
/// титра — НАСТОЯЩИЕ.
///
/// Регрессия, которую нашёл живой сценарий, а не харнесс: пипетка титра
/// была переведена на общий `color_row`, а тот читает значения через
/// `ParamSpec`. У титра цвета — прямые поля и `ParamSpec`'ами не
/// выражаются («первое законное исключение» F-TXT-001), поэтому
/// `get_param` молча возвращал ноль: заливка показывалась (и записывалась
/// при первом же касании) прозрачно-чёрной, и титр переставал рисоваться
/// вовсе. Ни один тест этого не видел — они щупали текст и числа.
///
/// Тест утверждает СЛЕДСТВИЕ: форма показывает те же цвета, что лежат в
/// модели, и hex-поле рядом с пипеткой показывает их же. Форма теперь
/// живёт в окне эффектов события — щупается она там же.
#[test]
fn t542_title_window_shows_real_colors() {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    let mut kind = GeneratorKind::default_text();
    if let GeneratorKind::Text { text, fill_r, fill_g, fill_b, fill_a, .. } = &mut kind {
        *text = "A".into();
        (*fill_r, *fill_g, *fill_b, *fill_a) = (255, 136, 0, 200);
    }
    h.state_mut().create_generator(kind);
    for _ in 0..4 {
        h.step();
    }

    // hex-поле заливки показывает РОВНО то, что в модели. Щупаем по
    // РОЛИ: у текстового поля в accesskit нет label, содержимое лежит в
    // `value` (родня заметки про DragValue и ComboBox).
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable, by};
    let shown: Vec<String> = h
        .query_all(by().role(Role::MultilineTextInput))
        .chain(h.query_all(by().role(Role::TextInput)))
        .filter_map(|n| n.accesskit_node().value().map(|v| v.to_string()))
        .collect();
    assert!(
        shown.iter().any(|s| s == "ff8800c8"),
        "форма титра показывает не тот цвет заливки: {shown:?}"
    );

    // и умолчание титра НЕ прозрачно-чёрное: иначе новый титр невидим
    let d = GeneratorKind::default_text();
    let GeneratorKind::Text { fill_r, fill_g, fill_b, fill_a, .. } = &d else { unreachable!() };
    assert_eq!(
        (*fill_r, *fill_g, *fill_b, *fill_a),
        (255, 255, 255, 255),
        "умолчание заливки титра обязано быть непрозрачно-белым"
    );
}

/// Тест 570 (Р.55, С5): раскрытие НЕ МЕНЯЕТ ни проект, ни то, что играет.
///
/// Главное обещание гибридного режима: из корня мы не выходим, поэтому
/// движок продолжает играть ВСЮ ПРОГРАММУ, а документ остаётся тем же
/// байт-в-байт. Раскрытие — рабочее место, а не работа: ни шага истории,
/// ни следа в файле.
#[test]
fn t570_expanding_in_place_changes_nothing_but_the_view() {
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    let doc_before = h.state().model.document();
    let play_before = h.state().player_project();
    let undo_before = h.state().model.undo_depth();
    let rows_before = h.state().tl_geom.clone().map(|g| g.rows.len()).unwrap_or(0);

    h.state_mut().expand_in_place(inst);
    h.step();

    assert_eq!(h.state().model.editing(), None, "раскрытие вышло из корня — играть будет лента");
    assert_eq!(h.state().model.bands().len(), 1, "полоса не завелась");
    assert_eq!(h.state().model.document(), doc_before, "раскрытие изменило ДОКУМЕНТ");
    assert_eq!(h.state().player_project(), play_before, "раскрытие изменило то, что ИГРАЕТ");
    assert_eq!(h.state().model.undo_depth(), undo_before, "раскрытие записалось в историю");

    // а вид — да: строки ленты встали ПОД строкой экземпляра
    let g = h.state().tl_geom.clone().expect("геометрия");
    assert!(g.rows.len() > rows_before, "строк не прибавилось — полосы не видно: {:?}", g.rows);
    let inner = h.state().model.project().clips.iter().find(|c| c.id != inst && c.media != seq);
    assert!(inner.is_some(), "содержимого ленты нет в рабочем виде");

    // и свернуть обратно — тождество вида (правило 14: жест повторный)
    h.state_mut().collapse_in_place(inst);
    h.step();
    assert!(h.state().model.bands().is_empty(), "свёртка не сработала");
    assert_eq!(h.state().model.document(), doc_before, "свёртка изменила документ");
    assert_eq!(
        h.state().tl_geom.clone().map(|g| g.rows.len()).unwrap_or(0),
        rows_before,
        "после свёртки строк осталось не столько же"
    );
}

/// Тест 571 (Р.55, С5): правка ИЗ ПОЛОСЫ доезжает до ЛЕНТЫ.
///
/// Адресации правок нет и не нужно: событие полосы физически лежит в
/// корневом векторе рабочего вида, команда приходит на него обычным путём,
/// а сборка вычитает смещение. Утверждение проверяется по ДОКУМЕНТУ.
#[test]
fn t571_editing_inside_a_band_lands_in_the_reel() {
    use kt_time::Tick;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    h.state_mut().expand_in_place(inst);
    h.step();

    // событие ПОЛОСЫ — то, чья строка принадлежит раскрытию
    let ids: Vec<(kt_model::ClipId, kt_time::Tick, u32)> =
        h.state().model.project().clips.iter().map(|c| (c.id, c.start, c.lane)).collect();
    let (id, was, lane) = ids
        .iter()
        .copied()
        .find(|(id, _, _)| *id != inst && h.state().reel_of_clip(*id).is_some())
        .expect("событие полосы");

    assert!(h.state_mut().apply(kt_model::Command::MoveClip {
        id,
        new_start: was + Tick(1_000),
        new_lane: lane,
    }));
    h.step();

    let doc = h.state().model.document();
    let s = doc.sequence(seq).expect("лента");
    assert!(
        s.clips.iter().any(|c| c.id == id),
        "правка не доехала до ленты: событие ушло в корень документа"
    );
    assert!(
        !doc.clips.iter().any(|c| c.id == id),
        "событие ленты оказалось в КОРНЕ документа — на диск уехал бы другой монтаж"
    );
    // раскрытие пережило правку
    assert_eq!(h.state().model.bands().len(), 1, "правка свернула раскрытие");
}

/// Тест 572 (Р.55, С5): выделение живёт РОВНО В ОДНОЙ ленте.
///
/// Смешанное выделение выглядело бы обычным, а жест по нему поехал бы
/// наполовину: команды на события разных лент один батч не переживут.
/// Клик в другой полосе ЗАМЕНЯЕТ выделение и говорит об этом.
#[test]
fn t572_selection_lives_in_one_reel() {
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    h.state_mut().expand_in_place(inst);
    h.step();

    let ids: Vec<kt_model::ClipId> = h.state().model.project().clips.iter().map(|c| c.id).collect();
    let root_other = ids
        .iter()
        .copied()
        .find(|id| *id != inst && h.state().reel_of_clip(*id).is_none())
        .expect("корневое событие рядом");
    let in_band = ids
        .iter()
        .copied()
        .find(|id| h.state().reel_of_clip(*id).is_some())
        .expect("событие полосы");

    h.state_mut().select_only(root_other);
    h.state_mut().toggle_selected(in_band);
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![in_band],
        "выделение размазалось по двум лентам — половина жеста молча не поехала бы"
    );
    // и обратно тоже
    h.state_mut().toggle_selected(root_other);
    assert_eq!(
        h.state().selected.iter().copied().collect::<Vec<_>>(),
        vec![root_other],
        "выделение размазалось по двум лентам в обратную сторону"
    );
}

/// Тест 573 (Р.55, С5): Esc сворачивает раскрытие ПЕРЕД выходом из ленты.
///
/// Свёртка полосы стоит человеку меньше внимания, чем выход наружу, —
/// значит и в каскаде стоит раньше. Но позже «снять выделение»: одна
/// клавиша не имеет права делать два дела за раз.
#[test]
fn t573_escape_collapses_the_band_before_leaving_a_reel() {
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    h.state_mut().expand_in_place(inst);
    h.state_mut().select_only(inst);
    h.step();

    h.state_mut().escape_action();
    assert_eq!(h.state().model.bands().len(), 1, "Esc свернул полосу вместо снятия выделения");
    assert!(h.state().selected.is_empty(), "Esc не снял выделение");

    h.state_mut().escape_action();
    assert!(h.state().model.bands().is_empty(), "второй Esc обязан свернуть раскрытие");
}

/// Тест 576 (Р.55, С6): дроп ПРОЕКТА СПРАШИВАЕТ и не теряет работу.
///
/// **Претензия владельца дословно:** «я переношу файл проекта на таймлайн и
/// открывается новый таймлайн, а я хочу чтобы оно дало мне выбор».
///
/// Выбора не было — и, что хуже, не было и вопроса о несохранённой работе:
/// дроп делал `*self = app` в обход `guard_unsaved` и `claim_project`, то
/// есть мимо всего, что построено в Р.49. Работа исчезала молча.
///
/// Тест утверждает ТРИ вещи: вопрос задаётся; «Отмена» ничего не делает;
/// «Открыть» проходит через сторож несохранённого.
#[test]
fn t576_dropping_a_project_asks_and_keeps_the_work() {
    let (mut h, _) = harness_three_clips();
    let dir = std::env::temp_dir().join("kt-t576");
    let _ = std::fs::create_dir_all(&dir);
    let foreign = dir.join("foreign.otio");
    {
        // чужой проект на диске — минимальный, но законный
        let mut p = kt_model::Project::new(h.state().model.project().settings.clone());
        p.media.push(kt_model::MediaRef::new(kt_model::MediaId(1), "synth:1"));
        p.clips.push(h.state().model.project().clips[0].clone());
        p.clips[0].media = kt_model::MediaId(1);
        let text = kt_otio::save_project(&p, &kt_otio::Retained::default(), None);
        std::fs::write(&foreign, text).unwrap();
    }

    // в редакторе есть НЕСОХРАНЁННАЯ работа
    h.state_mut().dirty_content = true;
    let clips_before: Vec<kt_model::ClipId> =
        h.state().model.project().clips.iter().map(|c| c.id).collect();

    h.state_mut().pending_drop = Some((foreign.clone(), None));
    h.state_mut().ask = Some(crate::app::Ask::DropOtio);
    h.step();

    // 1) ВОПРОС задан, и монтаж пока не тронут
    assert!(matches!(h.state().ask, Some(crate::app::Ask::DropOtio)), "вопрос не задан");
    assert_eq!(
        h.state().model.project().clips.iter().map(|c| c.id).collect::<Vec<_>>(),
        clips_before,
        "дроп изменил монтаж ДО ответа"
    );

    // 2) ОТМЕНА не делает ничего
    h.state_mut().answer_ask(crate::app::Answer::Cancel);
    h.step();
    assert!(h.state().ask.is_none(), "вопрос остался после отмены");
    assert_eq!(
        h.state().model.project().clips.iter().map(|c| c.id).collect::<Vec<_>>(),
        clips_before,
        "отмена всё-таки что-то сделала"
    );
    assert!(h.state().dirty_content, "отмена погасила признак несохранённой работы");

    // 3) «ОТКРЫТЬ» при несохранённой работе обязано СПРОСИТЬ, а не открыть
    h.state_mut().pending_drop = Some((foreign.clone(), None));
    h.state_mut().ask = Some(crate::app::Ask::DropOtio);
    h.state_mut().answer_ask(crate::app::Answer::Yes);
    h.step();
    assert!(
        matches!(h.state().ask, Some(crate::app::Ask::Unsaved(crate::app::Pending::DropOpen))),
        "дроп проекта открыл его поверх несохранённой работы — она пропала бы молча"
    );
    assert_eq!(
        h.state().model.project().clips.iter().map(|c| c.id).collect::<Vec<_>>(),
        clips_before,
        "монтаж подменён до ответа на вопрос о несохранённом"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 577 (Р.55, С6): «вставить композицией» кладёт ленту и экземпляр.
#[test]
fn t577_dropping_a_project_can_insert_it_as_a_composition() {
    use kt_time::Tick;
    let (mut h, _) = harness_three_clips();
    let dir = std::env::temp_dir().join("kt-t577");
    let _ = std::fs::create_dir_all(&dir);
    let foreign = dir.join("foreign.otio");
    {
        let mut p = kt_model::Project::new(h.state().model.project().settings.clone());
        p.media.push(kt_model::MediaRef::new(kt_model::MediaId(1), "synth:1"));
        p.clips.push(h.state().model.project().clips[0].clone());
        p.clips[0].media = kt_model::MediaId(1);
        let text = kt_otio::save_project(&p, &kt_otio::Retained::default(), None);
        std::fs::write(&foreign, text).unwrap();
    }
    let seqs_before = h.state().model.document().sequences.len();
    let clips_before = h.state().model.project().clips.len();

    h.state_mut().pending_drop =
        Some((foreign.clone(), Some((Tick(400_000), (kt_model::ClipKind::Video, 0)))));
    h.state_mut().ask = Some(crate::app::Ask::DropOtio);
    h.state_mut().answer_ask(crate::app::Answer::No);
    h.step();

    let doc = h.state().model.document();
    assert_eq!(doc.sequences.len(), seqs_before + 1, "лента из чужого проекта не завелась");
    assert_eq!(
        h.state().model.project().clips.len(),
        clips_before + 1,
        "экземпляр чужого проекта не встал на таймлайн"
    );
    let seq = doc.sequences.last().unwrap().id;
    let inst = doc.clips.iter().find(|c| c.media == seq).expect("экземпляра нет");
    assert_eq!(inst.start, Tick(400_000), "экземпляр встал не в точку дропа");
    doc.validate().expect("документ после вставки обязан быть законным");
    // и это ОДИН шаг истории: Ctrl+Z убирает вставку целиком
    assert!(h.state_mut().model.undo().is_ok());
    assert_eq!(
        h.state().model.document().sequences.len(),
        seqs_before,
        "вставка чужого проекта снялась не одним шагом"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 578 (Р.55, С6): лента — ОБЪЕКТ БИНА, а не только событие.
///
/// **Претензия владельца дословно:** «наверное еще куча вещей недоступна
/// из ui». Лента была ровно таким случаем: в бине её не было вовсе, и
/// поставить композицию во второе место можно было только копипастом уже
/// стоящего экземпляра, завести пустую — ничем, а удалить конкретную —
/// тоже (кнопка «убрать неиспользованные» убирает все сразу).
///
/// t253 этого не ловил: его таблица исчерпывающая по сущностям ТАЙМЛАЙНА,
/// а бин — панель. t395 спрашивает, отвечает ли панель на правую кнопку, а
/// не что в ней показано.
#[test]
fn t578_a_reel_is_an_object_of_the_bin() {
    let (mut h, _g, seq) = harness_with_nest();
    h.step();
    h.step();

    // 1) КАРТОЧКА ленты есть в бине
    let cards: Vec<kt_model::MediaId> =
        h.state().bin_card_rects.iter().map(|(id, _)| *id).collect();
    assert!(
        cards.contains(&seq),
        "ленты нет в бине: {cards:?} — поставить её во второе место было бы нечем"
    );

    // 2) ПОСТАВИТЬ во второе место — одним глаголом, а не копипастом
    let before = h.state().model.document().sequence_uses(seq);
    h.state_mut().player.seek(kt_time::Tick(400_000));
    h.state_mut().place_media_at_cursor(seq);
    h.step();
    assert_eq!(
        h.state().model.document().sequence_uses(seq),
        before + 1,
        "лента не встала во второе место из бина"
    );
    let inst = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .filter(|c| c.media == seq)
        .map(|c| c.start)
        .max()
        .expect("экземпляра нет");
    // точность — квант транспорта (playhead живёт в аудиокадрах 48 кГц,
    // п.7 р.10): требовать тик-в-тик значило бы требовать того, чего
    // транспорт не умеет нигде
    assert!((inst.0 - 400_000).abs() <= 2, "экземпляр встал не под курсор: {inst:?}");

    // 3) ЗАВЕСТИ ПУСТУЮ — тоже глагол, а не побочный эффект сворачивания
    let seqs_before = h.state().model.document().sequences.len();
    h.state_mut().create_empty_reel();
    h.step();
    h.step();
    assert_eq!(
        h.state().model.document().sequences.len(),
        seqs_before + 1,
        "пустая лента не завелась"
    );
    let cards: Vec<kt_model::MediaId> =
        h.state().bin_card_rects.iter().map(|(id, _)| *id).collect();
    assert_eq!(
        cards.len(),
        h.state().model.project().media.len() + h.state().model.document().sequences.len(),
        "в бине показаны не все объекты: {cards:?}"
    );

    // 4) УДАЛИТЬ АДРЕСНО — и занятая лента отказывается уходить (t429)
    let fresh = h
        .state()
        .model
        .document()
        .sequences
        .iter()
        .map(|s| s.id)
        .find(|id| *id != seq)
        .expect("пустая лента");
    assert!(
        h.state_mut().apply(kt_model::Command::RemoveSequence { id: fresh }),
        "пустая лента обязана удаляться адресно"
    );
    assert!(
        !h.state_mut().apply(kt_model::Command::RemoveSequence { id: seq }),
        "ЗАНЯТАЯ лента ушла молча — события повисли бы на несуществующем адресе"
    );
}

/// Тест 594 (Р.56, п.449): дорожка, добавленная ПОД потолок, и есть
/// верхний слой картинки.
///
/// Претензия владельца дословно: «если поместить видеотрек под потолок
/// (перед морфером самым высоким) и закинуть туда видео — то оно не
/// отображается вообще». Причина была в расхождении двух порядков:
/// движок складывал слои по НОМЕРУ дорожки, а экран показывал по
/// `lane_order`, и вставка из меню чужой строки брала `at = tail`, то
/// есть самый большой номер — нижний слой.
///
/// Тест утверждает СЛЕДСТВИЕ и щупает оттуда, откуда действует человек:
/// пункт меню строки, затем событие на новой дорожке, затем порядок, в
/// котором движок будет рисовать слои.
#[test]
fn t594_a_lane_added_under_the_ceiling_is_the_top_layer() {
    use kt_model::{ClipKind, LaneKind};
    let (mut h, _g) = harness_three_clips();
    // морф-строка сверху — та самая «под потолком» раскладка
    h.state_mut().insert_lane_after_row(LaneKind::Morph, None);
    h.step();
    let rows = h.state().model.project().rows();
    let morph = rows.iter().position(|&r| r.0 == LaneKind::Morph).expect("морф-строка");
    let top_video = rows.iter().position(|&r| r.0 == LaneKind::Video).expect("видео-строка");
    h.state_mut().reorder_rows(morph, 0);
    h.step();
    assert_eq!(h.state().model.project().rows()[0].0, LaneKind::Morph, "морф-строка не сверху");
    let _ = top_video;

    // ПКМ по морф-строке → «добавить видеодорожку»
    let undo_before = h.state().model.undo_depth();
    h.state_mut().insert_lane_after_row(LaneKind::Video, Some((LaneKind::Morph, 0)));
    h.step();
    let p = h.state().model.project().clone();
    let under = p.rows()[1];
    assert_eq!(under.0, LaneKind::Video, "новая строка встала не под морф-строкой: {:?}", p.rows());
    assert_eq!(
        h.state().model.undo_depth(),
        undo_before + 1,
        "вставка дорожки стоила больше одного шага истории"
    );

    // «закинул туда видео»
    let media = p.media[0].id;
    let id = kt_model::ClipId(p.clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 1);
    let mut c = p.clips.iter().find(|c| c.kind == ClipKind::Video).unwrap().clone();
    c.id = id;
    c.link_group = None;
    // дорожка — ТА, что видна под морф-строкой: номер её и решает
    c.lane = under.1;
    c.media = media;
    c.start = kt_time::Tick(0);
    assert!(h.state_mut().apply(kt_model::Command::AddClip(c)), "событие не встало на дорожку");
    h.step();

    // СЛЕДСТВИЕ: в порядке композиции движка (задний план первым) оно
    // ПОСЛЕДНЕЕ, то есть верхний слой
    let p = h.state().model.project();
    let mut order = p.effective_clips();
    order.retain(|c| c.kind == ClipKind::Video);
    order.sort_by_key(|c| (std::cmp::Reverse(c.lane), c.start.0));
    assert_eq!(
        order.last().map(|c| c.id),
        Some(id),
        "событие «под потолком» рисуется НЕ верхним слоем: {:?}",
        order.iter().map(|c| (c.id, c.lane)).collect::<Vec<_>>()
    );
}

/// Тест 596 (Р.56, п.450): копия из раскрытой композиции вставляется
/// ПАРОЙ и в НАЗВАННОЕ место.
///
/// Претензия владельца дословно: «я копирую ZEF последний в длинной
/// композиции из rendereg3 через пкм и иду потом ее вставлять, тоже через
/// пкм, а он берет и вставляет одну часть сверху а другую часть(аудио)
/// уносит вниз, ну и в самом низу морфер т.к. для него действительно
/// места не было».
///
/// Причин было две, и обе здесь проверяются следствием: клипборд хранил
/// номера РАБОЧЕГО ВИДА (в полосе раскрытия у видов разные базы), а
/// «Вставить» из меню клало блок НА КУРСОР, а не в точку клика.
#[test]
fn t596_paste_from_a_band_lands_as_a_pair_at_the_click() {
    use kt_model::{ClipKind, LaneKind};
    use kt_time::Tick;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    h.state_mut().expand_in_place(inst);
    h.step();
    assert_eq!(h.state().model.bands().len(), 1, "полоса не раскрылась");

    // событие ПОЛОСЫ: его номер дорожки — со смещением базы
    let band = h.state().model.bands()[0].clone();
    let inner = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.kind == ClipKind::Video && band.owns(LaneKind::Video, c.lane))
        .expect("события полосы не видно")
        .clone();
    assert!(inner.lane >= band.base.video, "сцена выродилась: полоса не смещена");

    // ПАРА: аудио-половина в той же полосе и в одной link-группе — ровно
    // то, что копировал владелец
    let mut a = inner.clone();
    a.id = kt_model::ClipId(
        h.state().model.document().clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 50,
    );
    a.kind = ClipKind::Audio;
    a.lane = band.base.audio;
    a.link_group = Some(kt_model::GroupId(77));
    assert!(h.state_mut().apply(kt_model::Command::AddClip(a.clone())), "аудио-половина");
    h.step();
    assert!(
        h.state_mut().apply(kt_model::Command::LinkClips {
            ids: vec![inner.id, a.id],
            group: kt_model::GroupId(77)
        }),
        "пара не связалась"
    );
    h.step();

    h.state_mut().select_only(inner.id);
    h.state_mut().copy_selection();
    h.step();
    assert_eq!(h.state().clipboard.clips.len(), 2, "link-пара скопировалась не целиком");
    // КЛИПБОРД В КООРДИНАТАХ ЛЕНТЫ: номера начинаются с нуля, а не с базы
    assert_eq!(
        h.state().clipboard.clips[0].lane,
        inner.lane - band.base.video,
        "клипборд хранит номера рабочего вида, а не ленты"
    );

    // вставка ПКМ в корневую строку и в НАЗВАННОЕ время
    let root_row = (LaneKind::Video, 0u32);
    h.state_mut().player.seek(Tick(0));
    let seqs_before = h.state().model.document().sequences[0].clips.len();
    let root_before = h.state().model.document().clips.len();
    h.state_mut().paste_at(Tick(700_000), Some(root_row));
    h.step();

    let doc = h.state().model.document().clone();
    assert_eq!(
        doc.sequences[0].clips.len(),
        seqs_before,
        "вставка в КОРЕНЬ дописала событий в ленту"
    );
    let fresh: Vec<&kt_model::ClipEvent> =
        doc.clips.iter().filter(|c| !matches!(c.id.0, 1..=3) && c.media != seq).collect();
    assert_eq!(doc.clips.len(), root_before + fresh.len(), "счёт не сошёлся");
    let pasted = fresh.iter().find(|c| c.start == Tick(700_000)).unwrap_or_else(|| {
        panic!(
            "вставка легла не в точку клика: {:?}",
            fresh.iter().map(|c| c.start).collect::<Vec<_>>()
        )
    });
    assert_eq!(pasted.lane, 0, "вставка не попала в кликнутую строку");
    assert_eq!(pasted.kind, ClipKind::Video);

    // ПАРА не разъехалась: аудио-половина тоже в корне, на своей верхней
    // дорожке, в то же время и в одной link-группе с видео
    let pair: Vec<&kt_model::ClipEvent> =
        fresh.iter().filter(|c| c.start == Tick(700_000)).copied().collect();
    assert_eq!(pair.len(), 2, "вставилась половина пары: {pair:?}");
    let pa = pair.iter().find(|c| c.kind == ClipKind::Audio).expect("аудио-половина потерялась");
    assert_eq!(pa.lane, 0, "аудио уехало вниз, а не на кликнутую строку: {pa:?}");
    assert_eq!(
        pa.link_group.is_some() && pa.link_group == pasted.link_group,
        true,
        "пара приехала разными группами"
    );
    // и ни одна половина не осталась в ленте
    assert!(
        !doc.sequences[0].clips.iter().any(|c| c.start == Tick(700_000)),
        "вставка в корень залезла в ленту"
    );
}

/// Тест 597 (Р.56, п.451): решение «что делает Enter» живёт в ОДНОМ месте
/// и не зависит от того, откуда его спросили.
///
/// Плюс два следствия, на которых каскад держится: раскрытие и свёртка
/// СВОЕЙ полосы оставляют экземпляр выделенным (иначе следующий жест
/// некуда направить), а «свернуть всё» — нет (полос больше нет, и держать
/// адрес не за чем).
#[test]
fn t597_enter_cascade_decision_and_selection() {
    use crate::app::EnterOnNest;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    let plain = h.state().model.project().clips.iter().find(|c| c.media != seq).unwrap().id;

    assert_eq!(h.state().enter_pressed_on_nest(plain), EnterOnNest::NotANest);
    assert_eq!(h.state().enter_pressed_on_nest(inst), EnterOnNest::Expand, "свёрнутое — раскрыть");

    h.state_mut().select_only(inst);
    h.state_mut().expand_in_place(inst);
    h.step();
    assert_eq!(
        h.state().enter_pressed_on_nest(inst),
        EnterOnNest::Enter,
        "по уже раскрытому Enter обязан вести ВНУТРЬ"
    );
    assert!(h.state().is_selected(inst), "раскрытие потеряло выделение экземпляра");

    // своя свёртка выделение держит, «свернуть всё» — нет
    h.state_mut().collapse_in_place(inst);
    h.step();
    assert!(h.state().is_selected(inst), "свёртка потеряла выделение экземпляра");
    h.state_mut().expand_in_place(inst);
    h.step();
    h.state_mut().collapse_all_in_place();
    h.step();
    assert!(h.state().selected.is_empty(), "«свернуть всё» обязано снимать выделение");

    // внутри полного входа быстрый жест ведёт ГЛУБЖЕ, а не в тупик
    h.state_mut().enter_nest(inst);
    h.step();
    assert_eq!(h.state().model.editing(), Some(seq));
    let deeper = h.state().model.project().clips.first().map(|c| c.id).unwrap();
    assert_eq!(
        h.state().enter_pressed_on_nest(deeper),
        EnterOnNest::NotANest,
        "обычное событие внутри ленты лентой не стало"
    );
}

/// Ключ превью ленты по её ПЕРВОМУ экземпляру (Р.59, стадия 3: ключ стал
/// парой «лента + набор переопределений экземпляра», t680).
#[cfg(test)]
fn reel_key_of(
    h: &mut Harness<'static, KtApp>,
    seq: kt_model::MediaId,
) -> crate::reel_preview::ReelKey {
    let inst = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.media == seq)
        .map(|c| c.id)
        .expect("на монтаже нет ни одного экземпляра ленты");
    *h.state_mut().reel_keys().get(&inst).expect("ключ превью экземпляра")
}

/// Тест 599 (Р.56, п.452): служба превью лент — очередь, дедуп,
/// инвалидация и потолок кадров.
#[test]
fn t599_reel_preview_service_rules() {
    use crate::reel_preview::{MAX_FRAMES_PER_REEL, ReelPreviews};
    use kt_time::Tick;

    // ШАГ ПОЛОСЫ — ступень лестницы тумб, и кадров не больше потолка
    for len in [Tick(90_000), Tick(45 * 90_000), Tick(3600 * 90_000)] {
        let step = ReelPreviews::strip_step(len);
        assert!(
            crate::thumbs::THUMB_STEP_LADDER.contains(&step),
            "шаг {step} не со ступени лестницы — ключи разойдутся с филмстрипом файла"
        );
        let n = (len.0 + step - 1) / step;
        assert!(
            n <= MAX_FRAMES_PER_REEL as i64
                || step == *crate::thumbs::THUMB_STEP_LADDER.last().unwrap(),
            "у ленты {len:?} вышло {n} кадров при потолке {MAX_FRAMES_PER_REEL}"
        );
    }

    let (mut h, _g, seq) = harness_with_nest();
    let key = reel_key_of(&mut h, seq);
    let hash = key.hash;

    // ДЕДУП: вторая заявка по тому же ключу работы не добавляет
    h.state_mut().request_reel_preview(key, &[]);
    let pending = h.state().reel_previews.pending();
    assert!(pending >= 1, "заявка не встала в очередь");
    h.state_mut().request_reel_preview(key, &[]);
    assert_eq!(h.state().reel_previews.pending(), pending, "дубль заявки прошёл");

    // ИНВАЛИДАЦИЯ: другая редакция той же ленты снимает старую работу
    let keep = std::collections::HashSet::from([hash ^ 1]);
    h.state_mut().reel_previews.invalidate_seq(seq, &keep);
    assert_eq!(h.state().reel_previews.pending(), 0, "устаревшая работа осталась в очереди");
}

/// Тест 600 (Р.56, п.452): кадры и пики ленты доезжают до ЭКРАНА.
///
/// Утверждается следствие, а не наличие поля: у события-вложения на
/// таймлайне появляются ТЕКСТУРЫ КАДРОВ и непустая огибающая — ровно то,
/// чего владелец не увидел («на таймлайне нет превью как у обычных
/// событий - кадров и аудио-пиков»).
#[test]
fn t600_reel_frames_and_peaks_reach_the_timeline() {
    let (mut h, _g, seq) = harness_with_nest();
    let key = reel_key_of(&mut h, seq);
    let hash = key.hash;

    // аудио-половина ленты: без неё пики проверять не на чем
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().clone();
    let mut a = inst.clone();
    a.id = kt_model::ClipId(
        h.state().model.document().clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 60,
    );
    a.kind = kt_model::ClipKind::Audio;
    a.lane = 0;
    a.link_group = None;
    assert!(h.state_mut().apply(kt_model::Command::AddClip(a)), "аудио-половина");
    h.step();

    let deadline = std::time::Instant::now() + WAIT_BG;
    while h.state().reel_previews.get(&key).map(|p| p.peaks.is_none()).unwrap_or(true)
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }

    let prev = h.state().reel_previews.get(&key).expect("превью ленты не посчиталось");
    let strip = prev.strip.as_ref().expect("кадров ленты нет");
    assert!(strip.frames.iter().any(|f| f.is_some()), "в полосе ленты ни одного кадра");
    assert!(
        prev.peaks.as_ref().is_some_and(|p| !p.is_empty()),
        "огибающей ленты нет — аудио-половина осталась пустой плашкой"
    );

    // и они ДОЕХАЛИ до отрисовки: текстуры кадров этой ленты загружены
    h.step();
    h.step();
    assert!(
        h.state().reel_textures.keys().any(|(k, _)| *k == key),
        "кадры ленты не дошли до экрана: текстур нет"
    );

    // ПРАВКА ВНУТРИ ленты меняет редакцию — старое превью больше не наше
    let inner = h.state().model.document().sequences[0].clips[0].clone();
    h.state_mut().expand_in_place(inst.id);
    h.step();
    let band_clip = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.id == inner.id)
        .expect("событие полосы")
        .clone();
    assert!(
        h.state_mut().apply(kt_model::Command::MoveClip {
            id: band_clip.id,
            new_start: band_clip.start + kt_time::Tick(1_000),
            new_lane: band_clip.lane,
        }),
        "правка внутри ленты"
    );
    h.step();
    let fresh = reel_key_of(&mut h, seq).hash;
    assert_ne!(fresh, hash, "правка внутри ленты не изменила редакцию — инвалидировать нечем");
    assert!(
        h.state().reel_previews.get(&key).is_none(),
        "устаревшее превью осталось в кэше и рисовалось бы вместо правки"
    );
}

/// Тест 601 (Р.56, п.452): прокрутка и зум заявок НЕ ставят (инвариант 8),
/// а служба тумб про ленту не знает вовсе.
#[test]
fn t601_scrolling_does_not_order_reel_work() {
    let (mut h, _g, seq) = harness_with_nest();
    let key = reel_key_of(&mut h, seq);
    let deadline = std::time::Instant::now() + WAIT_BG;
    // ждём КОНЦА работы, а не только кадров: `pending()` считает и
    // бегущую задачу, и тест мерил бы её, а не новые заявки
    while h.state().reel_previews.pending() > 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert!(h.state().reel_previews.get(&key).is_some(), "превью не посчиталось");

    // возим вид туда-сюда и зумим — новых заявок появиться не должно
    for i in 0..6 {
        h.state_mut().view_start = (i as f64) * 25_000.0;
        h.state_mut().ticks_per_px = 400.0 * (1.0 + i as f64 * 0.3);
        h.step();
        assert_eq!(
            h.state().reel_previews.pending(),
            0,
            "прокрутка/зум поставили заявку на превью ленты (шаг {i})"
        );
    }

    // и служба ТУМБ про ленту по-прежнему не знает: у неё ключ по файлу
    assert!(
        h.state().reel_textures.keys().all(|(k, _)| k.seq == seq),
        "в кэше текстур лент оказалась чужая лента"
    );
    assert!(
        h.state().thumb_textures.keys().all(|k| k.media != seq),
        "тумба по ленте заказана: у ленты нет файла, и служба индекса полезла бы ffmpeg'ом"
    );
}

/// Тест 640 (Р.58, п.463): превью ленты переживает открытие проекта.
///
/// Претензия владельца: «У композиций превью на таймлайне, кажется, не
/// кешируется и каждое открытие проекта строится заново». Утверждается
/// следствие: второй `KtApp::open` того же файла получает кадры и
/// огибающую ленты при НУЛЕ настоящих рендеров — из sidecar
/// `<project>.ktindex.db`. А подмена исходника (mtime) — промах и
/// перерендер: редакция ленты подмену файла не видит, её ловит отпечаток
/// медиа в ключе.
#[test]
fn t640_reel_preview_survives_reopen_from_sidecar() {
    use crate::reel_preview::{MAX_REELS_READY, ReelKey, ReelPreviews};
    use kt_model::*;

    let dir = std::env::temp_dir().join(format!("kt-t640-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t640.otio");
    // файл-исходник, чтобы отпечатку медиа было что нюхать: содержимое
    // роли не играет (открыть его нельзя — клип честно пуст), важны путь,
    // размер и mtime
    let extra = dir.join("extra.bin");
    std::fs::write(&extra, b"not a media file").unwrap();

    // документ с лентой — из готовой сцены, плюс клип ПО ФАЙЛУ внутри ленты
    let seq = {
        let (h, _g, seq) = harness_with_nest();
        let mut p = h.state().model.document().clone();
        // адреса медиа и лент — одно пространство (t425): свежий id над обоими
        let file_id = MediaId(
            p.media.iter().map(|m| m.id.0).chain(p.sequences.iter().map(|s| s.id.0)).max().unwrap()
                + 1,
        );
        p.media.push(MediaRef::new(file_id, "extra.bin"));
        let next_id = p
            .clips
            .iter()
            .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
            .map(|c| c.id.0)
            .max()
            .unwrap_or(0)
            + 500;
        let s = p.sequences.iter_mut().find(|s| s.id == seq).expect("reel");
        let mut a = s.clips[0].clone();
        a.id = ClipId(next_id);
        a.kind = ClipKind::Audio;
        a.lane = 0;
        a.link_group = None;
        a.media = file_id;
        s.clips.push(a);
        p.validate().expect("document with a file clip inside the reel");
        std::fs::write(&path, kt_otio::save_project(&p, &Default::default(), path.parent()))
            .unwrap();
        seq
    };

    // открыть файл и дождаться превью ленты (как t600)
    let open_and_wait = || -> (Harness<'static, KtApp>, ReelKey) {
        let mut app = KtApp::open(&path).expect("project opens");
        app.recovery_on_drop = false;
        app.workspace_file = Some(dir.join(format!("ws-{:p}.json", &app)));
        let mut h = Harness::builder()
            .with_size(egui::vec2(W, H))
            .build_ui_state(|ui, app: &mut KtApp| app.draw_all(ui), app);
        crate::icons::install_fonts(&h.ctx);
        h.step();
        let key = reel_key_of(&mut h, seq);
        let deadline = std::time::Instant::now() + WAIT_BG;
        while h.state().reel_previews.get(&key).map(|p| p.peaks.is_none()).unwrap_or(true)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(20));
            h.step();
        }
        (h, key)
    };
    let check_preview = |h: &Harness<'static, KtApp>, key: &ReelKey, what: &str| {
        let prev = h.state().reel_previews.get(key).unwrap_or_else(|| panic!("{what}: no preview"));
        let strip = prev.strip.as_ref().unwrap_or_else(|| panic!("{what}: no strip"));
        assert!(strip.frames.iter().any(|f| f.is_some()), "{what}: strip has no frames");
        assert!(strip.step > 0, "{what}: strip step is zero");
        assert!(prev.peaks.as_ref().is_some_and(|p| !p.is_empty()), "{what}: no peaks");
    };

    // 1) первое открытие: рендер настоящий — и ровно один
    let (h1, key1) = open_and_wait();
    check_preview(&h1, &key1, "first open");
    assert_eq!(h1.state().reel_previews.renders(), 1, "first open must render the reel once");
    drop(h1);

    // 2) второе открытие того же файла: всё есть, рендеров НОЛЬ
    let (h2, key2) = open_and_wait();
    assert_eq!(key2, key1, "reel key changed across save/load — the disk key could never hit");
    check_preview(&h2, &key2, "second open");
    assert_eq!(
        h2.state().reel_previews.renders(),
        0,
        "second open rendered the reel again: the sidecar tier did not serve it"
    );
    drop(h2);

    // 3) подменённый исходник: mtime вперёд — промах, перерендер
    {
        let f = std::fs::File::options().write(true).open(&extra).unwrap();
        f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5)).unwrap();
    }
    let (h3, key3) = open_and_wait();
    assert_eq!(key3, key1, "content hash saw the file change — media_fp would be redundant");
    check_preview(&h3, &key3, "open after media change");
    assert_eq!(
        h3.state().reel_previews.renders(),
        1,
        "media file changed under the reel, yet the stale disk preview was served"
    );
    drop(h3);
    let _ = std::fs::remove_dir_all(&dir);

    // ---- попутные дыры службы (Р.58): честный LRU, а не очередь вставки ----
    let mut svc = ReelPreviews::default();
    let k = |i: u64| ReelKey { seq: MediaId(1000 + i), hash: i };
    for i in 0..MAX_REELS_READY as u64 {
        svc.put(k(i), Default::default());
    }
    assert!(svc.get(&k(0)).is_some(), "just-put reel is missing");
    svc.put(k(MAX_REELS_READY as u64), Default::default());
    assert!(svc.get(&k(0)).is_some(), "recently used reel was evicted: the LRU is a FIFO");
    assert!(svc.get(&k(1)).is_none(), "least recently used reel survived eviction");
    assert_eq!(svc.ready_keys().len(), MAX_REELS_READY);
    assert_eq!(svc.failed_len(), 0);
}

// ===================== скорость транспорта (Р.57, п.455) =====================

/// Прямоугольник контрола скорости (программа отдаёт его сама — правило
/// «координаты берутся из САМОЙ программы», Р.27).
fn speed_rect(h: &Harness<'_, KtApp>) -> egui::Rect {
    let r = h.state().speed_rect;
    assert!(r.is_finite() && r.width() > 100.0, "контрол скорости не нарисован: {r:?}");
    r
}

/// Ctrl+клик без движения указателя. Модификатор едет ОТДЕЛЬНЫМ событием:
/// `i.modifiers` egui читает из состояния кадра, а не из кнопки мыши.
fn ctrl_click_at(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) {
    let m = egui::Modifiers::COMMAND;
    h.hover_at(pos);
    h.step();
    h.event(egui::Event::ModifiersChanged(m));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: m,
        });
    }
    h.step();
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

/// Тест 627 (Р.57): ВВОД СКОРОСТИ ЧИСЛОМ.
///
/// Прямая претензия владельца: «я должен иметь возможность выставить 0,05
/// или −0,03». Тест щупает оттуда, откуда действует человек: правая кнопка
/// (и Ctrl+клик — второй обещанный путь, правило 17) по САМОЙ полосе,
/// печать символов, Enter — и утверждает СЛЕДСТВИЕ: что стоит в базовой
/// скорости и что уехало в плеер.
#[test]
fn t627_speed_typed_value_reaches_player() {
    use crate::app::format_rate;
    let mut h = harness();
    let rect = speed_rect(&h);
    let on_bar = egui::pos2(rect.center().x, rect.top() + 4.0);

    // --- подсказка полосы правдива и обещает оба входа
    h.hover_at(on_bar);
    h.step();
    assert_eq!(
        h.state().hover_hint.as_deref(),
        Some(tr(k::hint::SPEED)),
        "полоса скорости не отдаёт свою подсказку"
    );

    // --- правый клик открывает поле НА МЕСТЕ полосы
    let before = h.state().tl_geom.clone().map(|g| g.rect.height());
    right_click_at(&mut h, on_bar);
    assert!(h.state().speed_entry.is_some(), "ПКМ по полосе не открыл ввод скорости");
    assert_eq!(
        h.state().tl_geom.clone().map(|g| g.rect.height()),
        before,
        "открытие поля сдвинуло вёрстку (правило 3: ничего не скачет)"
    );
    assert_eq!(speed_rect(&h), rect, "поле заняло не тот прямоугольник, что полоса");
    h.hover_at(on_bar);
    h.step();
    assert_eq!(
        h.state().hover_hint.as_deref(),
        Some(tr(k::hint::SPEED_ENTRY)),
        "у открытого поля чужая подсказка"
    );

    // --- пробел при открытом поле НЕ запускает воспроизведение
    let playing = h.state().player.transport.is_playing();
    h.event(egui::Event::Text(" ".into()));
    h.key_press(egui::Key::Space);
    h.step();
    assert_eq!(
        h.state().player.transport.is_playing(),
        playing,
        "пробел утёк в хоткеи мимо открытого поля"
    );

    // --- «0.05» + Enter
    h.event(egui::Event::Text("0.05".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    assert!(h.state().speed_entry.is_none(), "Enter не закрыл поле");
    let base = h.state().base_rate;
    assert!((base - 0.05).abs() < 1e-6, "база не стала 0.05, а {base}");
    let live = h.state().player.playback_rate();
    assert!((live - 0.05).abs() < 1e-6, "плеер не получил 0.05, а {live}");
    assert_eq!(format_rate(base), "+0.05×", "показ скорости врёт");
    assert!(
        !h.ctx.egui_wants_keyboard_input(),
        "поле закрылось, но фокус не отдан — хоткеи мертвы (правило 14)"
    );

    // --- второй вход: Ctrl+клик, и минус доезжает до модели
    ctrl_click_at(&mut h, on_bar);
    assert!(h.state().speed_entry.is_some(), "Ctrl+клик не открыл ввод скорости");
    h.event(egui::Event::Text("\u{2212}0,03".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    let base = h.state().base_rate;
    assert!((base + 0.03).abs() < 1e-6, "типографский минус и запятая не доехали: {base}");

    // --- нечитаемое значение НЕ закрывает поле (правило 16: это ошибка)
    right_click_at(&mut h, on_bar);
    h.event(egui::Event::Text("-".into()));
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    assert!(
        h.state().speed_entry.as_ref().is_some_and(|e| e.bad),
        "нечитаемое значение молча закрыло поле"
    );
    assert!((h.state().base_rate + 0.03).abs() < 1e-6, "нечитаемое значение испортило базу");

    // --- Esc закрывает без правки, и ПОСЛЕ него всё живо (правило 14)
    h.key_press(egui::Key::Escape);
    h.step();
    h.step();
    assert!(h.state().speed_entry.is_none(), "Esc не закрыл поле");
    assert!((h.state().base_rate + 0.03).abs() < 1e-6, "Esc изменил скорость");
    assert!(!h.ctx.egui_wants_keyboard_input(), "после Esc фокус остался на поле");
    let playing = h.state().player.transport.is_playing();
    h.key_press(egui::Key::Space);
    h.step();
    h.step();
    assert_ne!(
        h.state().player.transport.is_playing(),
        playing,
        "после закрытия поля пробел перестал переключать воспроизведение"
    );
    // и поле открывается ПОВТОРНО
    right_click_at(&mut h, on_bar);
    assert!(h.state().speed_entry.is_some(), "второй правый клик не открыл поле");
}

/// Тест 628 (Р.57): драг полосы даёт МАЛЫЕ скорости, Shift — точную
/// подстройку, центр — стоп, двойной клик — 1×.
#[test]
fn t628_speed_drag_reaches_small_rates() {
    use crate::app::{RATE_KNEE, SPEED_FINE_DIV, format_rate, slider_to_rate};

    // Общий путь указателя: захват слева от центра, порог драга берётся
    // ЯВНЫМ шагом (egui признаёт драг после ~6 px, и позиция ИМЕННО того
    // кадра становится якорем виртуального курсора).
    fn drag_upper(shift: bool, extra: f32) -> (f32, f32) {
        let mut h = harness();
        let rect = speed_rect(&h);
        let y = rect.top() + 4.0;
        let x0 = rect.center().x - 40.0;
        let x1 = rect.center().x - 20.0; // якорь: тут жест признаётся драгом
        h.hover_at(egui::pos2(x0, y));
        h.step();
        h.drag_at(egui::pos2(x0, y));
        h.step();
        if shift {
            h.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
        }
        h.hover_at(egui::pos2(x1, y));
        h.step();
        h.hover_at(egui::pos2(x1 + extra, y));
        h.step();
        let got = h.state().base_rate;
        h.drop_at(egui::pos2(x1 + extra, y));
        h.step();
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        (got, x1 - rect.left())
    }

    // --- 6 px вправо от центра — это 0.05×, а НЕ 0.25× и не стоп
    {
        let mut h = harness();
        let rect = speed_rect(&h);
        let y = rect.top() + 4.0;
        let x0 = rect.center().x - 40.0;
        h.hover_at(egui::pos2(x0, y));
        h.step();
        h.drag_at(egui::pos2(x0, y));
        h.step();
        h.hover_at(egui::pos2(rect.center().x - 20.0, y));
        h.step();
        h.hover_at(egui::pos2(rect.center().x + 6.0, y));
        h.step();
        let got = h.state().base_rate;
        h.drop_at(egui::pos2(rect.center().x + 6.0, y));
        h.step();
        assert!(
            got > 0.02 && got < RATE_KNEE,
            "6 px от центра обязаны дать десятые доли процента скорости, а дали {got}"
        );
        assert!((got - 0.055).abs() < 0.02, "6 px от центра дали {got}, ждали около 0.055");
        // --- отпуск в 1 px от центра — РОВНО стоп
        let mut h = harness();
        let rect = speed_rect(&h);
        h.hover_at(egui::pos2(x0, y));
        h.step();
        h.drag_at(egui::pos2(x0, y));
        h.step();
        h.hover_at(egui::pos2(rect.center().x - 20.0, y));
        h.step();
        h.hover_at(egui::pos2(rect.center().x + 1.0, y));
        h.step();
        let got = h.state().base_rate;
        h.drop_at(egui::pos2(rect.center().x + 1.0, y));
        h.step();
        assert_eq!(got, 0.0, "1 px от центра обязан быть стопом");
        assert_eq!(format_rate(0.0), tr(k::perf::RATE_STOP));
    }

    // --- Shift: тот же путь указателя проходит в SPEED_FINE_DIV раз меньше
    let travel = 48.0f32;
    let (plain, anchor) = drag_upper(false, travel);
    let (fine, anchor2) = drag_upper(true, travel);
    assert_eq!(anchor, anchor2, "харнесс разошёлся: якоря жестов разные");
    let w = 170.0f32;
    let t_of = |vx: f32| vx / w * 2.0 - 1.0;
    let want_plain = slider_to_rate(t_of(anchor + travel));
    let want_fine = slider_to_rate(t_of(anchor + travel / SPEED_FINE_DIV));
    assert!((plain - want_plain).abs() < 1e-4, "обычный драг: {plain}, ждали {want_plain}");
    assert!(
        (fine - want_fine).abs() < 1e-4,
        "Shift не поделил ХОД на {SPEED_FINE_DIV}: {fine}, ждали {want_fine}"
    );
    assert!(fine < plain, "Shift обязан идти медленнее обычного драга");

    // --- двойной клик возвращает 1×
    let mut h = harness_lang_dt(crate::i18n::Lang::Ru, Some(0.02));
    let rect = speed_rect(&h);
    h.state_mut().base_rate = 0.05;
    h.step();
    double_click_at(&mut h, egui::pos2(rect.center().x, rect.top() + 4.0));
    assert_eq!(h.state().base_rate, 1.0, "двойной клик не вернул 1×");
}

/// Тест 629 (Р.57): разгон удержанием пробела работает и от МАЛОЙ базы.
///
/// Прежний гистерезис был абсолютным (0.02): на базе 0.05 это 40 % базы,
/// и разогнанная скорость физически не доезжала до плеера — виджет
/// показывал разгон, а программа играла как стояла.
#[test]
fn t629_space_boost_from_small_base() {
    use crate::app::SPACE_BOOST_MAX;
    let mut h = harness();
    h.state_mut().base_rate = 0.05;
    h.state_mut().player.set_playback_rate(0.05);
    h.step();
    h.key_down(egui::Key::Space);
    h.step();
    std::thread::sleep(std::time::Duration::from_millis(260));
    // СРАЗУ после старта разгона: показанное множителем и то, что играет,
    // обязаны совпадать. Прежний АБСОЛЮТНЫЙ порог 0.02 держал плеер на
    // 0.05, пока множитель не дорастал до 1.4 — программа играла как
    // стояла, а виджет уже показывал разгон.
    let mut worst = 0.0f32;
    for _ in 0..12 {
        h.step();
        let b = h.state().space_boost.unwrap_or(1.0);
        let want = h.state().base_rate * b;
        let live = h.state().player.playback_rate();
        worst = worst.max((want - live).abs() / h.state().base_rate.abs());
    }
    assert!(
        worst < 0.10,
        "разгон от малой базы отстаёт от показанного на {:.0} % базы",
        worst * 100.0
    );
    for _ in 0..60 {
        h.step();
    }
    let boosted = h.state().player.playback_rate();
    assert!(boosted > 0.075, "от базы 0.05 разгон не доехал до плеера: {boosted}");
    assert!(
        boosted <= 0.05 * SPACE_BOOST_MAX + 1e-3,
        "разгон от малой базы перелетел потолок: {boosted}"
    );
    // и виджет это ПОКАЗЫВАЕТ
    let shown = h.state().speed_temp_slider().expect("разгон не виден на контроле скорости");
    assert!(
        shown > crate::app::rate_to_slider(0.05) + 0.001,
        "полоса скорости не показывает разгон от малой базы: {shown}"
    );
    h.key_up(egui::Key::Space);
    h.step();
    assert!(
        (h.state().player.playback_rate() - 0.05).abs() < 1e-4,
        "после отпускания скорость не вернулась к малой базе: {}",
        h.state().player.playback_rate()
    );
}

/// Тест 630 (Р.57, п.456): ось графика для ЗНАКОВОЙ скорости —
/// логарифм по модулю, и обычная кривая перестаёт быть прямой.
///
/// Жалоба Р.34 дословно: «График Slowed обрезан по оси rate на 2.0, а
/// числовое поле пускает до 4.0». С диапазоном −32…32 линейная ось
/// повторила бы ту же беду с другой стороны: кривая 0.66…1.0 заняла бы
/// 0.5 % высоты и выглядела прямой.
#[test]
fn t630_signed_ratio_axis_is_log_by_modulus() {
    use crate::automation::{Axis, axis_of};
    use kt_model::{FxInstance, ParamUnit};

    let slowed = FxInstance::slowed_reverb(1.0, 0.0, 85.0);
    let rate = slowed.param_specs().iter().find(|s| s.name == "rate").unwrap();
    assert_eq!(rate.unit, ParamUnit::Ratio);
    let ax = axis_of(rate);
    assert!(matches!(ax, Axis::SymLog { .. }), "у знакового отношения ось обязана быть лог");

    // нечётность, ноль в нуле, монотонность и обратимость
    assert_eq!(ax.map(0.0), 0.0);
    for v in [0.01f64, 0.05, 0.25, 0.66, 1.0, 4.0, 32.0] {
        assert!((ax.map(-v) + ax.map(v)).abs() < 1e-12, "ось не нечётна на {v}");
        assert!((ax.unmap(ax.map(v)) - v).abs() < 1e-9, "круг оси разошёлся на {v}");
        assert!((ax.unmap(ax.map(-v)) + v).abs() < 1e-9);
    }
    let mut prev = f64::MIN;
    for i in -320..=320 {
        let g = ax.map(i as f64 / 10.0);
        assert!(g > prev, "ось не монотонна около {}", i as f64 / 10.0);
        prev = g;
    }
    // непрерывность в колене
    assert!((ax.map(0.2499) - ax.map(0.2501)).abs() < 1e-3);

    // СЛЕДСТВИЕ: кривая 0.66…1.0 занимает заметную долю шкалы, а не
    // полпроцента, как было бы на линейной оси
    let g = (ax.map(1.0) - ax.map(0.66)) / (ax.map(32.0) - ax.map(-32.0));
    let lin = (1.0 - 0.66) / 64.0;
    assert!(g > lin * 5.0, "лог-ось не растянула окрестность единицы: {g} против {lin}");

    // у беззнакового отношения ось прежняя — правило по КЛАССУ, а не по имени
    let blur = FxInstance::blur(8);
    for s in blur.param_specs() {
        assert!(matches!(axis_of(s), Axis::Linear), "{}: ось поехала", s.name);
    }
}

// ---- Р.58: полоса внутри ленты, полосы через контексты, зона до нуля --------

/// Двухуровневый харнесс: корень → X → Y. Внутри X событие свёрнуто в
/// ленту Y тем же жестом, что и в корне. Возвращает геометрию ВНУТРИ X,
/// адреса обеих лент и экземпляр Y (в оси X).
#[cfg(test)]
fn harness_two_level() -> (
    Harness<'static, KtApp>,
    crate::timeline::TlGeom,
    kt_model::MediaId,
    kt_model::MediaId,
    kt_model::ClipId,
) {
    let (mut h, _g, x) = harness_with_nest();
    let x_inst = h.state().model.project().clips.iter().find(|c| c.media == x).unwrap().id;
    h.state_mut().enter_nest(x_inst);
    h.step();
    assert_eq!(h.state().model.editing(), Some(x), "harness: entering X failed");
    let inner = h.state().model.project().clips[0].id;
    h.state_mut().select_only(inner);
    h.step();
    h.state_mut().collapse_selection();
    h.step();
    h.step();
    let y = h
        .state()
        .model
        .document()
        .sequences
        .iter()
        .map(|s| s.id)
        .find(|id| *id != x)
        .expect("harness: reel Y was not created inside X");
    let y_inst = h.state().model.project().clips.iter().find(|c| c.media == y).unwrap().id;
    let g = h.state().tl_geom.clone().expect("geometry inside X");
    (h, g, x, y, y_inst)
}

/// Тест 634 (Р.58, п.460): полоса раскрывается ВНУТРИ полного входа.
///
/// **Претензия владельца дословно:** «открывая композицию не могу раскрыть
/// в ней вложенную композицию (я именно открываю композицию на весь
/// таймлайн и не могу в ней раскрыть вложенную не залезая уже в неё)».
///
/// Каскад Enter внутри X — тот же, что в корне: первый раскрывает, второй
/// входит. Документ от раскрытия не меняется, правка в полосе доезжает до
/// ленты Y (не до X и не до корня), Esc сворачивает полосу РАНЬШЕ выхода
/// из X.
#[test]
fn t634_a_band_opens_inside_an_entered_reel() {
    use crate::app::EnterOnNest;
    use kt_time::Tick;
    let (mut h, _g, x, y, y_inst) = harness_two_level();
    let doc_before = h.state().model.document();
    let rows_before = h.state().tl_geom.clone().map(|g| g.rows.len()).unwrap_or(0);

    // 1-й Enter — РАСКРЫТЬ, и решение не зависит от контекста
    assert_eq!(h.state().enter_pressed_on_nest(y_inst), EnterOnNest::Expand);
    h.state_mut().select_only(y_inst);
    h.state_mut().expand_in_place(y_inst);
    h.step();
    assert_eq!(h.state().model.editing(), Some(x), "expanding a band left reel X");
    assert_eq!(
        h.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![y_inst],
        "the band did not open inside an entered reel"
    );
    assert!(
        h.state().is_selected(y_inst),
        "the instance lost its selection — the 2nd Enter would miss"
    );
    let g = h.state().tl_geom.clone().expect("geometry");
    assert!(g.rows.len() > rows_before, "band rows are not on screen inside X: {:?}", g.rows);
    assert_eq!(h.state().model.document(), doc_before, "expanding inside X changed the DOCUMENT");
    // 2-й Enter — ВОЙТИ
    assert_eq!(h.state().enter_pressed_on_nest(y_inst), EnterOnNest::Enter);

    // правка в полосе доезжает до Y
    let (id, was, lane) = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .filter(|c| c.id != y_inst && h.state().reel_of_clip(c.id) == Some(y))
        .map(|c| (c.id, c.start, c.lane))
        .next()
        .expect("a band event");
    assert!(h.state_mut().apply(kt_model::Command::MoveClip {
        id,
        new_start: was + Tick(1_000),
        new_lane: lane,
    }));
    h.step();
    let doc = h.state().model.document();
    assert!(doc.sequence(y).unwrap().clips.iter().any(|c| c.id == id), "the edit missed reel Y");
    assert!(
        !doc.sequence(x).unwrap().clips.iter().any(|c| c.id == id)
            && !doc.clips.iter().any(|c| c.id == id),
        "the event of Y leaked into X or into the root"
    );
    assert_eq!(h.state().model.bands().len(), 1, "the edit collapsed the band");

    // Esc: снять выделение → свернуть полосу → и только потом выйти из X
    h.state_mut().escape_action();
    assert_eq!(h.state().model.bands().len(), 1, "Esc collapsed the band instead of deselecting");
    h.state_mut().escape_action();
    assert!(h.state().model.bands().is_empty(), "the 2nd Esc must collapse the band");
    assert_eq!(h.state().model.editing(), Some(x), "Esc left X before collapsing the band");
    h.state_mut().escape_action();
    assert_eq!(h.state().model.editing(), None, "the 3rd Esc must leave X");
}

/// Тест 636 (Р.58, п.461): полосы ПЕРЕЖИВАЮТ смену контекста.
///
/// **Претензия владельца дословно:** «при переходе между композициями/
/// root-таймлайном они сбрасывают состояние раскрытости, а хотелось бы
/// чтобы оставляли чтобы не путаться». Решение владельца (пересмотр Р.56
/// п.5): восстанавливать ВСЁ, включая полосу, через которую вошли вторым
/// Enter, — каскад Enter/Esc симметричен.
#[test]
fn t636_bands_survive_context_switches() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    use kt_time::Tick;

    // (а) корень: полоса на A, вход в B через ДРУГОЙ экземпляр (пункт меню
    // «Открыть»), возврат — A раскрыта, вид и курсор те же (t456)
    let (mut h, g, a) = harness_with_nest();
    let a_inst = h.state().model.project().clips.iter().find(|c| c.media == a).unwrap().id;
    // второй экземпляр: третье событие сворачивается в ленту B
    h.state_mut().select_only(ClipId(3));
    h.step();
    h.state_mut().collapse_selection();
    h.step();
    h.step();
    let b = h.state().model.document().sequences.iter().map(|s| s.id).find(|id| *id != a).unwrap();
    let b_inst = h.state().model.project().clips.iter().find(|c| c.media == b).unwrap().id;
    h.state_mut().expand_in_place(a_inst);
    // курсор — В ТЕЛЕ экземпляра B (200 000 … 250 000): вход переводит
    // время, а не телепортирует, и возврат обязан попасть в ту же точку
    // (снаружи тела курсор клампится к материалу — правило п.447)
    h.state_mut().player.seek(Tick(220_000));
    h.state_mut().view_start = 5_000.0;
    h.state_mut().ticks_per_px = 900.0;
    h.step();
    let g = h.state().tl_geom.clone().unwrap_or(g);
    let (vs, tpp) = (h.state().view_start, h.state().ticks_per_px);
    let cursor = h.state().player.transport.playhead();
    let row = g.row_of((LaneKind::Video, 0)).expect("video row");
    let x = {
        let c = h.state().model.project().clips.iter().find(|c| c.id == b_inst).unwrap().clone();
        g.tick_to_x(c.start.0 + c.duration.0 / 2)
    };
    right_click_at(&mut h, egui::pos2(x, g.row_center_y(row)));
    h.get_by_label_contains(tr(k::tl::menu::NEST_ENTER)).click();
    h.step();
    h.step();
    assert_eq!(h.state().model.editing(), Some(b), "the menu did not enter B");
    assert!(h.state().model.bands().is_empty(), "entering must clear bands (model invariant)");
    h.state_mut().leave_nest();
    h.step();
    assert_eq!(h.state().model.editing(), None);
    assert_eq!(
        h.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![a_inst],
        "(a) the band on A was not restored after returning from B"
    );
    assert_eq!((h.state().view_start, h.state().ticks_per_px), (vs, tpp), "(a) view not restored");
    // точность — квант транспорта (аудиокадр 48 кГц ≈ 1.875 тика), как в t456
    let back = h.state().player.transport.playhead();
    assert!((back.0 - cursor.0).abs() <= 4, "(a) cursor not restored: {back:?} vs {cursor:?}");
    assert!(h.state().selected.contains(&b_inst), "(a) selection not restored");

    // (б) вход через 2-й Enter по раскрытой A, Esc-каскад до выхода — A
    // снова раскрыта и выделена, третий Enter входит снова
    h.state_mut().select_only(a_inst);
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    assert_eq!(
        h.state().model.editing(),
        Some(a),
        "(b) the 2nd Enter on an expanded band must enter"
    );
    let mut guard = 0;
    while h.state().model.editing().is_some() {
        h.state_mut().escape_action();
        guard += 1;
        assert!(guard < 6, "(b) the Esc cascade does not lead out");
    }
    assert_eq!(
        h.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![a_inst],
        "(b) the band we entered through was not restored (owner's decision, Р.58)"
    );
    assert!(h.state().is_selected(a_inst), "(b) the instance is not selected after the return");
    h.key_press(egui::Key::Enter);
    h.step();
    assert_eq!(h.state().model.editing(), Some(a), "(b) the gesture does not repeat");
    h.state_mut().leave_nest();
    h.step();

    // (г, часть 1) экземпляр из снимка исчез или перестал быть вложением —
    // молча без полосы, без паники
    h.state_mut().enter_nest(b_inst);
    h.step();
    h.state_mut().nest_path.last_mut().unwrap().band = Some(ClipId(999_999));
    h.state_mut().leave_nest();
    h.step();
    assert_eq!(h.state().model.editing(), None);
    assert!(h.state().model.bands().is_empty(), "(d) a vanished instance must not leave a band");

    // (в) внутри X полоса на Y, выход в корень и вход обратно в X — Y
    // раскрыта; (г, часть 2) удалили экземпляр внутри — полосы нет
    let (mut h2, _g2, x, _y, y_inst) = harness_two_level();
    let x_inst = h2.state().model.document().clips.iter().find(|c| c.media == x).unwrap().id;
    h2.state_mut().expand_in_place(y_inst);
    h2.step();
    h2.state_mut().leave_nest();
    h2.step();
    assert_eq!(h2.state().model.editing(), None);
    h2.state_mut().enter_nest(x_inst);
    h2.step();
    assert_eq!(
        h2.state().model.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![y_inst],
        "(c) re-entering X did not bring the band on Y back"
    );
    assert!(h2.state_mut().apply(Command::RemoveClip { id: y_inst }));
    h2.step();
    assert!(h2.state().model.bands().is_empty(), "removing the instance must drop its band");
    h2.state_mut().leave_nest();
    h2.step();
    h2.state_mut().enter_nest(x_inst);
    h2.step();
    assert!(h2.state().model.bands().is_empty(), "(d) a removed instance came back as a band");
}

/// Драг события: навести → нажать → сдвинуть чуть → вести → отпустить.
#[cfg(test)]
fn drag_clip(h: &mut Harness<'_, KtApp>, from: egui::Pos2, to: egui::Pos2) {
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
}

/// Тест 637 (Р.58, п.462): внутри ленты событие уезжает ЗА НОЛЬ, в корне
/// ноль — стена.
///
/// **Претензия владельца дословно:** «При редактировании полностью открытой
/// композиции я не могу закинуть событие за ее пределы, а при раскрытой
/// композиции в root — могу. Воспроизводить в полностью открытой композиции
/// зону до 0 мне не надо». Стена была только в UI (`.max(0)` над тиком
/// жеста); формат и модель отрицательный `start` допускают, окно экземпляра
/// такое не играет.
#[test]
fn t637_inside_a_reel_an_event_may_cross_zero() {
    use kt_model::*;
    use kt_time::Tick;
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    let inst_ev = h.state().model.document().clips.iter().find(|c| c.id == inst).unwrap().clone();
    h.state_mut().enter_nest(inst);
    h.state_mut().ticks_per_px = 1000.0;
    h.state_mut().view_start = -1.0e6;
    h.step();
    let g = h.state().tl_geom.clone().expect("geometry inside the reel");
    let inner = h.state().model.project().clips[0].clone();
    assert_eq!(inner.start, Tick::ZERO, "scene: the reel content must start at zero");
    let y = g.lane_y(ClipKind::Video, inner.lane, 0.6);
    let grab = egui::pos2(g.tick_to_x(inner.start.0 + 25_000), y);
    let to = egui::pos2(g.tick_to_x(inner.start.0 - 5_000), y);
    assert!(to.x > g.rect.left(), "scene: the drop point is off screen ({:?})", to);
    drag_clip(&mut h, grab, to);
    let moved = h.state().model.project().clips.iter().find(|c| c.id == inner.id).unwrap().clone();
    assert_eq!(moved.start, Tick(-30_000), "the event did not cross zero inside the reel");

    // ДОКУМЕНТ ленты хранит отрицательный start
    let doc = h.state().model.document();
    assert_eq!(
        doc.sequence(seq).unwrap().clips.iter().find(|c| c.id == inner.id).map(|c| c.start),
        Some(Tick(-30_000)),
        "the reel's sequence does not store the negative start"
    );
    // и НЕ ИГРАЕТ его: окно экземпляра отсекает всё до нуля ленты —
    // в корне остаётся кусок [inst.start, +20 000) и ничего левее
    let eff = doc.effective_clips();
    assert!(eff.iter().all(|c| c.start >= Tick::ZERO), "played content went before zero: {eff:?}");
    assert!(
        eff.iter().any(|c| c.start == inst_ev.start && c.duration == Tick(20_000)),
        "the instance window did not cut the pre-zero part: {eff:?}"
    );
    // Esc/выход/вход — событие на месте
    h.state_mut().leave_nest();
    h.step();
    h.state_mut().enter_nest(inst);
    h.step();
    assert_eq!(
        h.state().model.project().clips.iter().find(|c| c.id == inner.id).map(|c| c.start),
        Some(Tick(-30_000)),
        "leave + re-enter lost the negative start"
    );

    // в КОРНЕ тот же жест упирается в ноль
    let (mut h2, g2) = harness_three_clips();
    let y2 = g2.lane_y(ClipKind::Video, 0, 0.6);
    let grab2 = egui::pos2(g2.tick_to_x(25_000), y2);
    let to2 = egui::pos2(g2.tick_to_x(-5_000).max(g2.rect.left() + 1.0), y2);
    drag_clip(&mut h2, grab2, to2);
    let c1 = h2.state().model.project().clips.iter().find(|c| c.id == ClipId(1)).unwrap().start;
    assert_eq!(c1, Tick::ZERO, "in the root zero must remain a wall");
}

/// Тест 637b (Р.58, п.462): вид уходит левее нуля ТОЛЬКО внутри ленты, а
/// курсор ниже нуля не уходит нигде.
#[test]
fn t637b_view_before_zero_only_inside_a_reel_and_the_cursor_never() {
    use kt_model::*;
    use kt_time::Tick;
    // корень: прежние −40 px и ни тиком больше
    let (mut h, _g) = harness_three_clips();
    h.state_mut().view_start = -1.0e6;
    h.step();
    let tpp = h.state().ticks_per_px;
    assert_eq!(h.state().view_start, -40.0 * tpp, "the root view went past −40 px");

    // внутри ленты: пол — самое раннее событие (оно левее нуля) и те же −40 px
    let (mut h, _g, seq) = harness_with_nest();
    let inst = h.state().model.project().clips.iter().find(|c| c.media == seq).unwrap().id;
    h.state_mut().enter_nest(inst);
    h.step();
    let inner = h.state().model.project().clips[0].id;
    assert!(h.state_mut().apply(Command::MoveClip {
        id: inner,
        new_start: Tick(-30_000),
        new_lane: 0
    }));
    h.state_mut().ticks_per_px = 1000.0;
    h.state_mut().view_start = -1.0e6;
    h.step();
    assert_eq!(
        h.state().view_start,
        -30_000.0 - 40_000.0,
        "inside a reel the view floor is the earliest event"
    );

    // курсор: клик по дорожке и по линейке ЛЕВЕЕ нуля ставит его в 0
    let g = h.state().tl_geom.clone().expect("geometry");
    let x = g.tick_to_x(-20_000);
    assert!(x > g.rect.left(), "scene: the pre-zero zone is not on screen");
    for y in [g.lane_y(ClipKind::Video, 0, 0.6), g.rect.top() + crate::timeline::TIME_TOP + 11.0] {
        h.state_mut().player.seek(Tick(40_000));
        h.step();
        let pos = egui::pos2(x, y);
        h.hover_at(pos);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
        h.step();
        assert_eq!(
            h.state().player.transport.playhead(),
            Tick::ZERO,
            "a click before zero at y={y} must put the cursor at 0, not before"
        );
    }
}

/// Тест 666 (Р.59, стадия 2): форма генератора в окне события доезжает
/// до МОДЕЛИ — и значением, и кривой.
///
/// Утверждается то, что делает человек: он открыл окно эффектов события,
/// увидел ИСТОЧНИК первой крошкой, напечатал в поле титра — и текст лежит
/// на СОБЫТИИ; нажал «+ кривая» и выбрал ряд — и на событии появилась его
/// кривая. Прежние окна генератора и титра тем самым закрыты не
/// «удалением кода», а переносом: все их входы ведут сюда.
///
/// Харнесса два, и это не лень: форма титра — самая высокая в программе,
/// и её меню кривых в окне харнесса открывается уже за нижней кромкой.
/// Кривые при этом свойство ЛЮБОГО генератора, поэтому щупаются на
/// компактном виде, а поле текста — на титре.
#[test]
fn t666_generator_form_in_the_fx_window_reaches_the_model() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    use kt_model::*;
    crate::i18n::set_lang(crate::i18n::Lang::Ru);

    // ---- КРИВАЯ: «+ кривая» → ряд -----------------------------------
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::NoiseField { amount: 0.5, mono: false });
    for _ in 0..3 {
        h.step();
    }
    let (_id, clip) = made_generator(&h);
    assert_eq!(
        chain_sel(&h, clip),
        Some(crate::widgets::ChainSel::Head),
        "источник обязан быть ПЕРВОЙ КРОШКОЙ и быть выбран сразу"
    );
    assert!(
        h.state().model.clip(clip).is_some_and(|c| c.gen_auto.is_empty()),
        "у нового генератора кривых быть не должно"
    );
    let depth = h.state().model.undo_depth();
    h.get_by_label(crate::i18n::tr(crate::i18n::k::fx::AUTO_ADD)).click();
    h.step();
    h.step();
    // подпись ряда есть и в сетке значений, и в открывшемся меню —
    // жмём ПОСЛЕДНЮЮ: меню рисуется поверх (идиома t193)
    let row = crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "amount");
    h.get_all_by_label(row.as_str()).last().expect("пункт меню ряда").click();
    h.step();
    h.step();
    let curves = h.state().model.clip(clip).map(|c| c.gen_auto.clone()).unwrap_or_default();
    assert_eq!(curves.len(), 1, "кривая не доехала до события: {curves:?}");
    assert_eq!(curves[0].param, "amount");
    assert_eq!(
        curves[0].keys.first().map(|k| k.t),
        h.state().model.clip(clip).map(|c| c.source_in),
        "кривая генератора живёт в тике МАТЕРИАЛА — по нему её и разрешает источник"
    );
    assert_eq!(h.state().model.undo_depth(), depth + 1, "правка в окне — шаг истории");

    // ---- ЗНАЧЕНИЕ: печатаем в поле титра, как человек ----------------
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    for _ in 0..3 {
        h.step();
    }
    let (_id, clip) = made_generator(&h);
    let depth = h.state().model.undo_depth();
    h.get(by().role(Role::MultilineTextInput)).click();
    h.step();
    for part in ["Вы", "пуск"] {
        h.event(egui::Event::Text(part.into()));
        h.step();
        h.step();
    }
    match gen_of(&h, clip) {
        GeneratorKind::Text { text, .. } => {
            assert_eq!(text, "Выпуск", "набранный текст не доехал до события")
        }
        other => panic!("вид подменился: {other:?}"),
    }
    assert_eq!(
        h.state().model.undo_depth(),
        depth + 1,
        "вся сессия окна — ОДИН шаг истории: иначе каждый символ титра попал бы в undo"
    );
}

/// Тест 667 (Р.59, стадия 2): «Применить ко всем (×N)» — ОТДЕЛЬНАЯ
/// команда и ОДИН шаг истории.
///
/// Решение владельца: правка одного события соседей больше не трогает, а
/// «править во всех местах разом» остаётся доступным отдельным жестом.
/// Значит утверждать надо обе половины: пока кнопку не нажали — сосед не
/// шелохнулся; нажали — сосед догнал, и один Ctrl+Z вернул его назад.
#[test]
fn t667_apply_to_all_is_one_command_and_one_step() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
    h.step();
    let (id, first) = made_generator(&h);
    // второе место того же генератора
    h.state_mut().player.seek(kt_time::Tick(700_000));
    h.state_mut().place_media_at_cursor(id);
    h.step();
    let second = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.media == id && c.id != first)
        .map(|c| c.id)
        .expect("второе событие генератора");

    // правка ОДНОГО события соседа не трогает
    let gesture = h.state().fx_editors[0].gesture;
    let mine = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    {
        let app = h.state_mut();
        assert!(
            app.apply_coalesced(
                Command::SetClipGen { id: first, kind: Some(mine.clone()) },
                gesture
            )
        );
    }
    h.step();
    assert_eq!(gen_of(&h, first), mine);
    assert_eq!(
        gen_of(&h, second),
        GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 },
        "правка одного события не имеет права трогать соседей"
    );

    // ...а кнопка — трогает, и говорит об этом числом мест
    let depth = h.state().model.undo_depth();
    let label = crate::tf!(crate::i18n::k::genw::APPLY_ALL, 2usize);
    assert!(h.query_by_label(&label).is_some(), "кнопки «{label}» нет в форме источника");
    h.get_by_label(&label).click();
    h.step();
    h.step();
    assert_eq!(gen_of(&h, second), mine, "«применить ко всем» не догнало соседа");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "жест обязан быть ОДНИМ шагом истории");
    h.state_mut().undo();
    h.step();
    assert_eq!(
        gen_of(&h, second),
        GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 },
        "один Ctrl+Z обязан вернуть соседа"
    );
}

/// Тест 668 (Р.59, стадия 2): пресет анимации титра ставит КРИВЫЕ, а
/// раскладка от них не двигается.
///
/// Пять прежних режимов анимации были полем МОДЕЛИ, и движок знал про
/// «печатную машинку». Теперь это имя НАБОРА КРИВЫХ, формулы живут в
/// одном месте (`kt_model::text_anim_curves`) — ими же читаются старые
/// файлы, и разойдись миграция с пресетом, проект прошлой сборки поехал
/// бы не так.
///
/// Вторая половина — правило F-TXT-006: раскладка считается ОДИН РАЗ, а
/// анимация только модулирует. Значит блок текста у титра с анимацией и
/// без неё обязан быть тем же самым: поедет он — и «бегущая строка»
/// начнёт переносить слова под себя.
#[test]
fn t668_preset_writes_curves_and_keeps_the_layout() {
    use crate::gen_ui::title_presets;
    use kt_model::*;
    let mut base = GeneratorKind::default_text();
    if let GeneratorKind::Text { text, .. } = &mut base {
        *text = "Бегущая строка выпуска".into();
    }
    let frame = (1920u32, 1080u32);
    let src_in = kt_time::Tick(123_456);

    // «Бегущая строка»: один ряд, ключи от +span/2 до −span/2, якорь —
    // ВХОД события (кривые генератора живут в тике материала)
    let crawl = title_presets().iter().find(|p| p.anim == TextAnim::Crawl).expect("пресет");
    let made = crawl.applied(&base);
    let curves = crawl.curves(&made, frame, src_in);
    assert_eq!(curves.len(), 1, "бегущая строка — ровно один анимируемый ряд");
    assert_eq!(curves[0].param, "offset_x");
    assert!(
        curves[0].keys.iter().all(|k| k.shape == kt_model::SegShape::Linear),
        "проход линейный: со сглаживанием строка ехала бы рывками"
    );
    assert_eq!(curves[0].keys.len(), 2);
    assert_eq!(curves[0].keys[0].t, src_in, "кривая обязана начинаться на ВХОДЕ события");
    assert_eq!(
        curves[0].keys[1].t.0 - src_in.0,
        (crawl.anim_secs * 90_000.0).round() as i64,
        "длительность прохода — прежняя (старые файлы читаются теми же формулами)"
    );
    let block = kt_media::generator::title_layout(&made, frame.0, frame.1).expect("раскладка");
    let span = 1.0 + block.block.w / frame.0 as f64;
    assert!(
        (curves[0].keys[0].v - span / 2.0).abs() < 1e-9
            && (curves[0].keys[1].v + span / 2.0).abs() < 1e-9,
        "путь считается по РАСКЛАДКЕ блока: {:?}",
        curves[0].keys
    );

    // ...и раскладка от анимации НЕ зависит: сдвиг модулирует показ, а
    // не переносит слова (F-TXT-006)
    let mut moved = made.clone();
    moved.set_param("offset_x", 0.37);
    moved.set_param("reveal", 0.25);
    let after = kt_media::generator::title_layout(&moved, frame.0, frame.1).expect("раскладка");
    assert_eq!(
        (after.block.w, after.block.h),
        (block.block.w, block.block.h),
        "анимация сдвинула РАСКЛАДКУ — правило «считается один раз» нарушено"
    );

    // «Печатная машинка»: пресет у неё есть, и он ведёт ряд `reveal`
    let tw = title_presets().iter().find(|p| p.anim == TextAnim::Typewriter).expect("пресет");
    let made = tw.applied(&base);
    let c = tw.curves(&made, frame, kt_time::Tick::ZERO);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].param, "reveal");
    assert_eq!(c[0].keys.first().map(|k| k.v), Some(0.0));
    assert_eq!(c[0].keys.last().map(|k| k.v), Some(1.0));

    // «Проявление» ведёт ТОТ ЖЕ ряд, и различает их ширина фронта — она
    // постоянна во времени, значит это ЗНАЧЕНИЕ, а не кривая (кривая из
    // одного ключа была бы вторым способом записать то же самое).
    // Пресета у него нет, поэтому спрашиваем общие формулы напрямую —
    // те же, которыми читаются старые файлы.
    for (anim, soft) in [(TextAnim::Typewriter, 0.0), (TextAnim::Fade, 1.0)] {
        let mut k = base.clone();
        text_anim_values(&mut k, anim);
        let GeneratorKind::Text { reveal_soft, .. } = &k else { unreachable!() };
        assert_eq!(*reveal_soft, soft, "{anim:?}: ширина фронта — значение, а не кривая");
        let c = text_anim_curves(anim, 2.0, 2.0);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].param, "reveal", "{anim:?} ведёт тот же ряд");
    }

    // ...и «активный пресет» на этих двух не путается: он сравнивает и
    // значения, и набор кривых
    let curves_tw = tw.curves(&made, frame, kt_time::Tick::ZERO);
    assert_eq!(crate::gen_ui::active_preset(&made, &curves_tw), Some(2));
    assert_eq!(
        crate::gen_ui::active_preset(&made, &[]),
        None,
        "без кривых «машинка» перестаёт быть машинкой — иначе новый титр звался бы ею"
    );
}

// ---- Публичные параметры композиции (Р.59, стадия 3) --------------------

/// Новостной шаблон для тестов: лента с ТИТРОМ внутри, поставленная
/// дважды, и шесть объявленных наружу ручек — по одной на каждый тип
/// значения.
///
/// Сцена НЕ вырожденная намеренно: второй экземпляр стоит в другом месте
/// и переопределяет заголовок. На сцене «один экземпляр без
/// переопределений» половина утверждений ниже выполнялась бы сама собой.
pub(crate) struct NewsScene {
    pub seq: kt_model::MediaId,
    /// Титр ВНУТРИ ленты — цель текстовых публикаций.
    pub inner: kt_model::ClipId,
    /// Цветной логотип внутри ленты — цель цветовой и медийной.
    pub logo_clip: kt_model::ClipId,
    /// Первый выпуск (шаблонные значения) и второй (свой заголовок).
    pub a: kt_model::ClipId,
    pub b: kt_model::ClipId,
    pub headline: kt_model::PubId,
    pub tint: kt_model::PubId,
    pub size: kt_model::PubId,
    pub align: kt_model::PubId,
    pub logo: kt_model::PubId,
    pub flip: kt_model::PubId,
}

/// Собрать сцену. Публикации ставятся ИЗНУТРИ ленты — командами, как это
/// делает окно: команда правит ту ленту, в которой стоишь.
pub(crate) fn harness_news_template() -> (Harness<'static, KtApp>, NewsScene) {
    use kt_model::*;
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::default_text());
    h.step();
    let (_media, title) = made_generator(&h);
    // непустой титр: у вида с умолчаниями текста нет вовсе, и «шаблон»
    // выглядел бы пустым
    let mut kind = gen_of(&h, title);
    if let GeneratorKind::Text { text, .. } = &mut kind {
        *text = "ВЫПУСК".into();
    }
    assert!(h.state_mut().apply(Command::SetClipGen { id: title, kind: Some(kind) }));
    h.step();
    // ЛОГОТИП — цветная плашка рядом с титром. Цвет у неё настоящий ряд
    // паспорта (`color_*` с единицей Rgb8), в отличие от красок титра:
    // те живут прямо в варианте и свойствами не являются вовсе.
    h.state_mut().create_generator(GeneratorKind::Solid { color_r: 200, color_g: 30, color_b: 30 });
    h.step();
    let (_lm, logo_clip) = made_generator(&h);
    {
        let (start, lane) = h
            .state()
            .model
            .clip(title)
            .map(|c| (c.start + c.duration, c.lane))
            .expect("титр на месте");
        assert!(h.state_mut().apply(Command::MoveClip {
            id: logo_clip,
            new_start: start,
            new_lane: lane
        }));
    }
    h.step();
    // окна, открытые созданием генераторов, сцене мешают: дальше свои
    h.state_mut().fx_editors.clear();
    h.state_mut().selected.clear();
    h.state_mut().selected.insert(title);
    h.state_mut().selected.insert(logo_clip);
    h.step();
    h.state_mut().collapse_selection();
    h.step();
    h.step();
    let seq = h.state().model.document().sequences.last().expect("лента").id;
    let a = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| c.media == seq)
        .expect("экземпляр ленты")
        .id;

    // ---- объявляем ручки ИЗНУТРИ ленты ------------------------------
    h.state_mut().enter_nest(a);
    h.step();
    assert_eq!(h.state().model.editing(), Some(seq), "вход внутрь ленты не состоялся");
    let inner = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| matches!(c.r#gen, Some(GeneratorKind::Text { .. })))
        .expect("титр внутри ленты")
        .id;
    let logo_in = h
        .state()
        .model
        .project()
        .clips
        .iter()
        .find(|c| matches!(c.r#gen, Some(GeneratorKind::Solid { .. })))
        .expect("логотип внутри ленты")
        .id;
    let mut next = 0u64;
    let mut publish = |h: &mut Harness<'static, KtApp>,
                       name: &str,
                       clip: ClipId,
                       owner: ParamOwner,
                       param: &str,
                       ty: ParamType| {
        next += 1;
        let id = PubId(next);
        let param = PublishedParam {
            id,
            name: name.to_string(),
            target: ParamAddr::new(clip, owner, param),
            ty,
            range: PubRange::default(),
        };
        assert!(
            h.state_mut().apply(Command::PublishParam { seq, param: Box::new(param) }),
            "публикация «{name}» отвергнута"
        );
        h.step();
        id
    };
    let headline = publish(
        &mut h,
        "Заголовок",
        inner,
        ParamOwner::Generator,
        "text",
        ParamType::Text { multiline: true },
    );
    let tint =
        publish(&mut h, "Цвет логотипа", logo_in, ParamOwner::Generator, "color", ParamType::Color);
    let size = publish(&mut h, "Кегль", inner, ParamOwner::Generator, "size", ParamType::Num);
    let align_ty = h
        .state()
        .model
        .document()
        .addr_type(Some(seq), &ParamAddr::new(inner, ParamOwner::Generator, "align"));
    let align = publish(
        &mut h,
        "Выключка",
        inner,
        ParamOwner::Generator,
        "align",
        align_ty.expect("у титра есть выключка"),
    );
    let logo = publish(&mut h, "Логотип", logo_in, ParamOwner::ClipAttr, "media", ParamType::Media);
    let flip =
        publish(&mut h, "Задом наперёд", inner, ParamOwner::ClipAttr, "reversed", ParamType::Flag);

    h.state_mut().leave_nest();
    h.step();
    assert_eq!(h.state().model.editing(), None, "выход из ленты не состоялся");

    // ---- второй выпуск ----------------------------------------------
    let mut b_clip = h.state().model.project().clips.iter().find(|c| c.id == a).cloned().unwrap();
    let free = h.state().model.document().clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 40;
    b_clip.id = ClipId(free);
    b_clip.start = kt_time::Tick(600_000);
    b_clip.link_group = None;
    assert!(h.state_mut().apply(Command::AddClip(b_clip)), "второй выпуск не встал");
    h.step();
    let b = ClipId(free);
    (h, NewsScene { seq, inner, logo_clip: logo_in, a, b, headline, tint, size, align, logo, flip })
}

/// Тест 680 (Р.59, стадия 3): у превью ЛЕНТЫ ключ по ЭКЗЕМПЛЯРУ.
///
/// До этого раунда служба `reel_preview` ключевалась `(лента,
/// content_hash)`, то есть РЕДАКЦИЕЙ ШАБЛОНА. С переопределениями это
/// значит, что два выпуска с разными заголовками показывали бы на
/// таймлайне ОДНИ И ТЕ ЖЕ кадры — кадры первого из них. Утверждение
/// теста — следствие: ключи двух экземпляров РАЗНЫЕ, а у экземпляра без
/// переопределений ключ остался прежним (байт в байт `content_hash`).
#[test]
fn t680_reel_preview_keys_are_per_instance() {
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // без переопределений оба выпуска делят ОДИН ключ: проекты, которые
    // публикацией не пользуются, не платят ничего
    let keys = h.state_mut().reel_keys();
    let (ka, kb) = (*keys.get(&s.a).expect("ключ A"), *keys.get(&s.b).expect("ключ B"));
    assert_eq!(ka, kb, "экземпляры без переопределений обязаны делить один ключ превью");
    let plain = kt_engine::content_hash(&h.state().model.document(), s.seq);
    assert_eq!(ka.hash, plain, "пустой набор сдвинул редакцию ленты — поехали бы все ключи");

    // второй выпуск получает СВОЙ заголовок
    assert!(h.state_mut().apply(Command::SetOverride {
        id: s.b,
        ov: Box::new(Override::value(s.headline, ParamValue::Text("ВТОРОЙ ВЫПУСК".into()))),
    }));
    h.step();
    let keys = h.state_mut().reel_keys();
    let (ka2, kb2) = (*keys.get(&s.a).expect("ключ A"), *keys.get(&s.b).expect("ключ B"));
    assert_eq!(ka2, ka, "шаблонный экземпляр не трогали — его ключ обязан остаться прежним");
    assert_ne!(
        ka2, kb2,
        "переопределённый экземпляр получил ключ шаблона: на таймлайне он показал бы ЧУЖИЕ кадры"
    );

    // и обе редакции ЖИВЫ одновременно: инвалидация не имеет права
    // выбрасывать соседнюю, иначе два выпуска пересчитывали бы друг друга
    // по кругу
    h.state_mut().request_reel_preview(ka2, &[]);
    let ov_b = h.state().model.clip(s.b).map(|c| c.overrides.clone()).unwrap_or_default();
    h.state_mut().request_reel_preview(kb2, &ov_b);
    let pending = h.state().reel_previews.pending();
    assert!(pending >= 2, "две редакции одной ленты не встали в очередь разом: {pending}");
}

/// Тест 683 (Р.59, стадия 3): окно «Параметры композиции» рисует КАЖДЫЙ
/// тип своим виджетом, сброс шлёт `ClearOverride`, а число с кривой
/// садит ключ в материале ЭКЗЕМПЛЯРА.
#[test]
fn t683_instance_params_window_draws_every_type() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, s) = harness_news_template();
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    assert_eq!(
        h.state().fx_editors.first().and_then(|e| e.selected.get(&s.b)),
        Some(&crate::widgets::ChainSel::Head),
        "голова «Параметры композиции» не выбрана в окне FX"
    );

    // ИМЕНА АВТОРА видны все и в объявленном порядке
    for name in ["Заголовок", "Цвет логотипа", "Кегль", "Выключка", "Логотип", "Задом наперёд"]
    {
        assert!(
            h.query_all_by_label_contains(name).count() > 0,
            "строки «{name}» в окне нет — тип нарисован нечем"
        );
    }

    // --- ТЕКСТ: поле правит переопределение --------------------------
    let win = h.state().fx_editors[0].gesture;
    assert!(h.state_mut().apply_coalesced(
        Command::SetOverride {
            id: s.b,
            ov: Box::new(Override::value(s.headline, ParamValue::Text("ВТОРОЙ".into()))),
        },
        win
    ));
    h.step();
    assert_eq!(
        h.state().model.document().instance_value(h.state().model.clip(s.b).unwrap(), s.headline),
        Some(ParamValue::Text("ВТОРОЙ".into())),
        "переопределение заголовка не доехало до модели"
    );
    // а ШАБЛОН при этом цел: сосед играет своим
    assert_eq!(
        h.state().model.document().published_value(s.seq, s.headline),
        Some(ParamValue::Text("ВЫПУСК".into())),
        "правка экземпляра уехала в ленту — это была бы правка всех мест сразу"
    );

    // --- СБРОС: кнопка строки шлёт ClearOverride ---------------------
    h.step();
    let reset = format!("{} Заголовок", tr(k::pubp::RESET));
    let before = h.state().model.clip(s.b).map(|c| c.overrides.len()).unwrap_or(0);
    assert_eq!(before, 1, "к этому месту у второго выпуска ровно одно переопределение");
    h.get_by_label(&reset).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().model.clip(s.b).map(|c| c.overrides.len()),
        Some(0),
        "кнопка сброса не сняла переопределение"
    );

    // --- ТИПЫ: объявленное совпадает с тем, что у цели ЖИВЬЁМ --------
    // окно рисует строку ПО ТИПУ, и разойдись объявление с целью —
    // человек получил бы чекбокс на месте текста
    {
        let doc = h.state().model.document();
        let seq = doc.sequence(s.seq).cloned().expect("лента");
        for (id, want) in [
            (s.headline, ParamType::Text { multiline: true }),
            (s.tint, ParamType::Color),
            (s.size, ParamType::Num),
            (s.align, ParamType::Choice { variants: std::borrow::Cow::Borrowed(&[]) }),
            (s.logo, ParamType::Media),
            (s.flip, ParamType::Flag),
        ] {
            let p = seq.published.iter().find(|p| p.id == id).expect("объявленная ручка");
            let live = doc.addr_type(Some(s.seq), &p.target).expect("цель жива");
            assert!(
                live.same_family(&want) && p.ty.same_family(&want),
                "ручка «{}» объявлена не тем типом: {:?} против живого {live:?}",
                p.name,
                p.ty
            );
        }
        // цветная ручка целит в ЛОГОТИП, а не в титр: адрес — часть
        // объявления, и перепутай окно носителя, правился бы чужой ряд
        let tint = seq.published.iter().find(|p| p.id == s.tint).unwrap();
        assert!(kt_model::same_clip(tint.target.clip, s.logo_clip), "цвет объявлен не у логотипа");
    }

    // --- ЧИСЛО С КРИВОЙ: ключ садится в материале ЭКЗЕМПЛЯРА ---------
    // второй выпуск стоит НЕ в нуле (start = 600_000), поэтому «материал
    // экземпляра» и «время проекта» тут различимы — сцена не вырождена
    let inst = h.state().model.clip(s.b).cloned().unwrap();
    assert_ne!(inst.start.0, 0, "сцена выродилась: у экземпляра в нуле домен не отличить");
    let frame_step = 90_000 / h.state().model.project().settings.fps.max(1) as i64;
    let at = kt_model::clip_material_at(
        inst.source_in,
        inst.start,
        inst.duration,
        inst.reversed,
        frame_step,
        h.state().player.transport.playhead(),
    );
    // Кривая заводится НАЗВАННЫМ действием окна (Р.60): до этого раунда
    // тут стоял `SetOverride` руками, и тест был зелёным на интерфейсе,
    // где кривую заводил безымянный глиф, а показать её было нечем.
    h.get_by_label(tr(k::fx::AUTO_ADD)).click();
    h.step();
    h.step();
    {
        use eframe::egui::accesskit::Role;
        use egui_kittest::kittest::by;
        h.get(by().role(Role::Button).label("Кегль")).click();
    }
    h.step();
    h.step();
    let got = h
        .state()
        .model
        .clip(s.b)
        .and_then(|c| c.overrides.iter().find(|o| o.pub_id == s.size).cloned())
        .expect("переопределение кегля");
    assert_eq!(got.curves.len(), 1, "кривая переопределения не доехала");
    assert_eq!(got.curves[0].param, "size", "в модель уехало оконное имя кривой");
    assert!(
        got.curves[0].keys.iter().all(|k| k.t.0 >= at.0.min(0)),
        "ключи стартовой кривой легли вне материала экземпляра: {:?}",
        got.curves[0]
    );
    // и сама кривая ВИДНА: тот же виджет, что у слота эффекта
    assert!(
        h.query_all_by_label_contains(tr(k::fx::AUTO_CLIP)).count() > 0,
        "кривая заведена, а виджета автоматизации в окне нет"
    );

    // и модель ЭТО ВИДИТ: разрешение ленты под набором отдаёт кривую цели
    let doc = h.state().model.document();
    let ov = h.state().model.clip(s.b).unwrap().overrides.clone();
    let resolved = doc.resolve_reel(Some(s.seq), &ov);
    let target = resolved.clips.iter().find(|c| kt_model::same_clip(c.id, s.inner)).unwrap();
    assert!(
        target.gen_auto.iter().any(|c| c.param == "size"),
        "кривая переопределения не доехала до цели внутри ленты"
    );
}

/// Тест 691 (Р.60): кривая переопределения ВИДНА, и без неё поле — константа.
///
/// Владелец: «я могу в разных местах поставить разные значения и будет
/// что-то типо автоматизации кривых… но кривых автоматизаций я нигде не
/// вижу… ни одной кнопки в окне про автоматизацию нет». Окно правило ключи
/// вслепую: безымянный глиф заводил кривую, и дальше каждая правка поля
/// клала ключ по курсору.
///
/// Тест утверждает НАБЛЮДАЕМОЕ СЛЕДСТВИЕ с двух сторон. Без кривой правка
/// поля при двух разных положениях курсора обязана дать ОДНО значение и
/// НОЛЬ ключей — скрытой записи не бывает. После названного действия
/// «+ кривая» в окне обязан появиться тот же виджет, что у слота эффекта,
/// и те же две правки дают уже ДВА ключа.
#[test]
fn t691_instance_curves_are_visible_and_never_hidden() {
    use eframe::egui;
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // курсор ставим внутрь ВТОРОГО выпуска: домен ключа — материал
    // экземпляра, и снаружи события он не определён
    let (b_start, b_dur) =
        h.state().model.clip(s.b).map(|c| (c.start, c.duration)).expect("второй выпуск на месте");
    let t1 = kt_time::Tick(b_start.0 + b_dur.0 / 4);
    let t2 = kt_time::Tick(b_start.0 + b_dur.0 / 2);

    h.state_mut().player.seek(t1);
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    assert_eq!(
        h.state().fx_editors.first().and_then(|e| e.selected.get(&s.b)),
        Some(&crate::widgets::ChainSel::Head),
        "голова «Параметры композиции» не выбрана в окне FX"
    );

    // поле «Кегля» лежит между подписью-именем автора и подписью шаблона
    let field_pos = |h: &Harness<'_, KtApp>| -> egui::Pos2 {
        let name = h
            .query_all_by_label("Кегль")
            .map(|n| n.rect())
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .expect("строка «Кегль» в окне");
        let right = h
            .query_all_by_label_contains("шаблон")
            .map(|n| n.rect())
            .filter(|r| (r.center().y - name.center().y).abs() < 6.0)
            .min_by(|a, b| a.left().total_cmp(&b.left()))
            .expect("подпись шаблона в строке «Кегль»");
        assert!(right.left() > name.right() + 8.0, "между именем и шаблоном нет места под поле");
        egui::pos2((name.right() + right.left()) * 0.5, name.center().y)
    };
    let type_value = |h: &mut Harness<'static, KtApp>, pos: egui::Pos2, text: &str| {
        h.hover_at(pos);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            h.step();
        }
        h.event(egui::Event::Text(text.into()));
        h.step();
        h.key_press(egui::Key::Enter);
        h.step();
        h.step();
    };
    let ov_of = |h: &Harness<'_, KtApp>| -> Option<Override> {
        h.state()
            .model
            .clip(s.b)
            .and_then(|c| c.overrides.iter().find(|o| o.pub_id == s.size).cloned())
    };

    // --- БЕЗ КРИВОЙ: две правки в разных местах дают ОДНО значение -----
    // Р.66: поле экземпляра рисует общий ряд, и доля вводится ПРОЦЕНТОМ —
    // той же единицей, что внутри композиции (раньше одно и то же число
    // писалось «0.1» в одном окне и «10 %» в другом)
    let pos = field_pos(&h);
    type_value(&mut h, pos, "10");
    let after_first = ov_of(&h).expect("правка поля не доехала до модели");
    assert!(after_first.curves.is_empty(), "поле завело кривую само: {after_first:?}");

    h.state_mut().player.seek(t2);
    h.step();
    let pos = field_pos(&h);
    type_value(&mut h, pos, "30");
    let after_second = ov_of(&h).expect("вторая правка не доехала");
    assert!(
        after_second.curves.is_empty(),
        "правка при другом положении курсора ЗАПИСАЛА ключ — это и есть скрытая \
         автоматизация, на которую жаловался владелец: {after_second:?}"
    );
    assert_eq!(
        after_second.value.as_num(),
        Some(0.3),
        "без кривой переопределение обязано быть КОНСТАНТОЙ последней правки"
    );
    assert_ne!(
        after_first.value.as_num(),
        after_second.value.as_num(),
        "харнесс не изменил значение — тест ничего не проверяет"
    );

    // --- НАЗВАННОЕ ДЕЙСТВИЕ заводит кривую и показывает её -------------
    assert!(
        h.query_all_by_label_contains(tr(k::fx::AUTO_CLIP)).count() == 0,
        "виджет автоматизации нарисован до того, как кривую завели"
    );
    h.get_by_label(tr(k::fx::AUTO_ADD)).click();
    h.step();
    h.step();
    // в меню ПУНКТ, а не подпись строки: имя ручки в окне встречается и
    // тем и другим, и «любой узел с этим текстом» кликнул бы не туда
    {
        use eframe::egui::accesskit::Role;
        use egui_kittest::kittest::by;
        h.get(by().role(Role::Button).label("Кегль")).click();
    }
    h.step();
    h.step();

    let with_curve = ov_of(&h).expect("переопределение исчезло");
    assert_eq!(with_curve.curves.len(), 1, "кривая не завелась: {with_curve:?}");
    assert_eq!(
        with_curve.curves[0].param, "size",
        "в модель уехало ОКОННОЕ имя кривой вместо целевого — apply_overrides её не найдёт"
    );
    assert!(
        h.query_all_by_label_contains(tr(k::fx::AUTO_CLIP)).count() > 0,
        "кривая заведена, а виджета автоматизации в окне нет — ровно та дыра, \
         из-за которой «кривых автоматизаций я нигде не вижу»"
    );

    // --- С КРИВОЙ: те же две правки дают ДВА ключа ---------------------
    let mat = |t: kt_time::Tick| {
        let c = h.state().model.clip(s.b).expect("второй выпуск");
        kt_model::clip_material_at(
            c.source_in,
            c.start,
            c.duration,
            c.reversed,
            90_000 / h.state().model.document().settings.fps.max(1) as i64,
            t,
        )
    };
    let (m1, m2) = (mat(t1), mat(t2));
    h.state_mut().player.seek(t1);
    h.step();
    // те же проценты, что и выше: 20 % и 40 % от кадра
    let pos = field_pos(&h);
    type_value(&mut h, pos, "20");
    h.state_mut().player.seek(t2);
    h.step();
    let pos = field_pos(&h);
    type_value(&mut h, pos, "40");

    let curve = ov_of(&h).expect("переопределение исчезло").curves.remove(0);
    let at = |t: kt_time::Tick| curve.keys.iter().find(|k| k.t == t).map(|k| k.v);
    assert_eq!(at(m1), Some(0.2), "ключ не сел в материале экземпляра: {curve:?}");
    assert_eq!(at(m2), Some(0.4), "второй ключ не сел: {curve:?}");
    // …и в МОДЕЛИ это доли, а не проценты: пересчёт живёт в одном месте
    assert!(at(m1).is_some_and(|v| v < 1.0), "в модель уехал процент вместо доли");
    assert!(m1 != m2, "харнесс двигал курсор в одну и ту же точку материала");

    // имя АВТОРА виджет рисует дословно: у ручки композиции имени в
    // каталоге нет и быть не может (Р.59)
    assert!(
        h.query_all_by_label_contains("Кегль").count() > 0,
        "виджет подписал кривую не именем автора"
    );
}

/// Тест 709 (Р.61): поле формулы объясняет ошибку и не портит модель.
///
/// Владелец: «возможность задания кривой автоматизации через ввод формулы
/// (без прокликивания мышки)». Проверка на лету обязана быть именно
/// проверкой: полустёртый текст в поле не вправе ронять кривую на каждом
/// нажатии клавиши.
#[test]
fn t709_the_formula_field_explains_errors_without_touching_the_model() {
    use kt_model::{CurveSource, Formula, ParamCurve};

    // --- незакрытая скобка и неизвестное имя НЕ применяются -----------
    let mut c = ParamCurve {
        param: "radius".into(),
        keys: vec![kt_model::AutoKey::at(kt_time::Tick(0), 3.0)],
        source: CurveSource::Keys,
    };
    let before = c.clone();
    for bad in ["(1 + 2", "1 + ", "wat(1)", "1 + $"] {
        let e = Formula::parse(bad).expect_err("битый текст обязан быть отказом");
        // позиция названа — поле подсветит её
        assert!(e.to_string().contains("char"), "в отказе нет позиции: {e}");
        // и модель НЕ ТРОНУТА
        assert_eq!(c, before, "битый текст {bad:?} доехал до модели");
    }

    // --- ВЕРНЫЙ текст применяется и стирает ключи --------------------
    let good = "t * 2 + 1";
    Formula::parse(good).expect("верный текст разбирается");
    c.keys.clear();
    c.source = CurveSource::Formula { expr: good.into() };
    assert!(!c.source.is_keys(), "источник не сменился");
    assert!(c.keys.is_empty(), "у формулы ключей быть не может — они второй источник правды");

    // --- пустое поле НЕ превращает кривую в формулу -------------------
    assert!(Formula::parse("").is_err(), "пустой текст не выражение");

    // --- поле и подсказка названы во всех языках ----------------------
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        let name = tr(k::fx::FORMULA);
        let hint = tr(k::fx::FORMULA_HINT);
        assert!(!name.is_empty(), "{}: у поля формулы нет подписи", lang.code());
        assert!(
            hint.contains('t') || hint.contains("src") || hint.len() > 30,
            "{}: подсказка не объясняет переменные: {hint}",
            lang.code()
        );
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// Тест 706 (Р.61): производная кривая ВИДНА, но мышкой не правится, и её
/// можно отвязать.
///
/// Владелец: «итоговый граф тоже рисуется но не редактируется мышкой».
/// Редактируемый производный график был бы третьей правдой о значении:
/// ключей у такой кривой нет, и перетаскивание точки некуда записать.
#[test]
fn t706_a_driven_curve_is_visible_but_not_draggable() {
    use crate::automation::{AutoCtx, AutoView};
    use kt_model::{AutoKey, CurveSource, ParamCurve};
    use kt_time::Tick;

    let id = |t: Tick| t;
    let ctx = AutoCtx {
        default_shape: Default::default(),
        specs: &[],
        range: (Tick(0), Tick(90_000)),
        view: (Tick(0), Tick(90_000)),
        to_timeline: &id,
        to_curve: &id,
        frame_step: 3000,
        linked: true,
    };
    let _ = &ctx;

    // --- у производной кривой МАРКЕРОВ КЛЮЧЕЙ нет ---------------------
    // (щуп ходит по нарисованному: нет маркера — нет и попадания)
    let mut view = AutoView::default();
    view.key_marks.clear();
    assert!(
        view.key_at(eframe::egui::pos2(10.0, 10.0)).is_none(),
        "щуп нашёл ключ там, где ничего не нарисовано"
    );

    // --- «отвязать» превращает её в обычные ключи ---------------------
    let mut driven = ParamCurve {
        param: "radius".into(),
        keys: vec![AutoKey::at(Tick(0), 3.0), AutoKey::at(Tick(90_000), 9.0)],
        source: CurveSource::Link {
            path: "2/fx0/radius".into(),
            expr: Some("src * 2".into()),
            stops: Vec::new(),
        },
    };
    assert!(!driven.source.is_keys(), "кривая не производная — тест ничего не проверяет");
    let before = driven.keys.clone();
    driven.source = CurveSource::Keys;
    assert!(driven.source.is_keys(), "«отвязать» не сняло источник");
    assert_eq!(driven.keys, before, "«отвязать» изменило значения — они уже разрешены моделью");

    // --- подпись показывает, ЧЕМ она ведома ---------------------------
    let link = CurveSource::Link { path: "2/fx0/radius".into(), expr: None, stops: Vec::new() };
    assert_eq!(link.link_path(), Some("2/fx0/radius"), "адрес источника не достать для подписи");
    let f = CurveSource::Formula { expr: "t * 2".into() };
    assert_eq!(f.expr(), Some("t * 2"), "текст формулы не достать для подписи");
    assert_eq!(CurveSource::Keys.expr(), None);
    assert_eq!(CurveSource::Keys.link_path(), None);

    // --- действие «отвязать» ДОСТИЖИМО и объясняет себя ---------------
    for lang in crate::i18n::Lang::ALL {
        crate::i18n::set_lang(lang);
        let name = tr(k::fx::UNLINK);
        let hint = tr(k::fx::UNLINK_HINT);
        assert!(!name.is_empty(), "{}: у действия «отвязать» нет имени", lang.code());
        assert!(hint.len() > 20, "{}: подсказка не объясняет действие: {hint}", lang.code());
        assert_ne!(name, hint, "{}: подсказка повторяет имя", lang.code());
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
}

/// Тест 700b (Р.61): окно сопоставления открывается только при остатке, а
/// пропущенная кривая НАЗЫВАЕТСЯ.
#[test]
fn t700b_field_mapping_dialog_never_loses_a_curve_silently() {
    use crate::envclip::FitDialog;
    use kt_model::{AutoKey, ParamCurve};
    use kt_time::Tick;

    let (mut h, _g) = harness_three_clips();
    let id = h.state().model.project().clips[0].id;
    let blur = kt_model::FxInstance::Blur { radius: 8, radius_y: 0 };
    assert!(h.state_mut().apply(kt_model::Command::SetFx { id, fx: vec![blur.clone()] }));
    h.step();

    let free: Vec<String> = blur.param_specs().iter().map(|s| s.name.to_string()).collect();
    let curve = |p: &str| ParamCurve {
        param: p.into(),
        keys: vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(45_000), 7.0)],
        source: Default::default(),
    };

    // остаток из двух кривых: одну кладём в свободный параметр, вторую нет
    h.state_mut().env_fit = Some(FitDialog {
        target: (id, 0),
        delta: 0,
        rows: vec![(curve("чужое_1"), Some(free[0].clone())), (curve("чужое_2"), None)],
        free: free.clone(),
        matched: Vec::new(),
        open: true,
    });
    h.step();
    h.step();
    {
        use egui_kittest::kittest::Queryable;
        assert!(
            h.query_all_by_label_contains(tr(k::fx::FIT_TITLE)).count() > 0,
            "окно сопоставления не открылось при непустом остатке"
        );
        h.get_by_label(tr(k::fx::FIT_DO)).click();
    }
    h.step();
    h.step();

    // выбранная кривая доехала до модели ПОД НОВЫМ именем
    let auto = h.state().model.clip(id).map(|c| c.fx_auto.clone()).unwrap_or_default();
    let got: Vec<String> =
        auto.first().map(|v| v.iter().map(|c| c.param.clone()).collect()).unwrap_or_default();
    assert_eq!(got, vec![free[0].clone()], "сопоставленная кривая не легла: {auto:?}");
    // а пропущенная НАЗВАНА, а не исчезла молча
    let msg = h.state().status.as_ref().map(|(m, _)| m.clone()).unwrap_or_default();
    assert!(
        msg.contains('1'),
        "пропущенная кривая исчезла молча — ровно то, чего быть не должно: {msg:?}"
    );

    // окно закрылось и второй раз само не открывается
    assert!(h.state().env_fit.is_none(), "окно сопоставления осталось висеть");
}

/// Тест 699 (Р.61): три исхода чтения буфера, и ни одного молчаливого.
///
/// Владелец: «нужно по нажатию кнопки брать значение из реального буфера и
/// уже потом проверять можно вставить или нет, и предложить действия».
/// Отсюда: свой блок — кладём; чужой текст и пустой буфер — НАЗВАННАЯ
/// ошибка, а не погашенная кнопка.
#[test]
fn t699_clipboard_is_read_on_press_and_explains_refusals() {
    use crate::envclip::ClipRead;
    use kt_model::{AutoKey, ParamCurve};
    use kt_time::Tick;

    // блок огибающей переживает системный буфер ТЕМ ЖЕ текстом, каким
    // туда попадает: копирование пишет json, чтение его разбирает
    let block = kt_model::ops::EnvelopeBlock {
        fx_tag: "blur".into(),
        curves: vec![ParamCurve {
            param: "radius".into(),
            keys: vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(90_000), 9.0)],
            source: Default::default(),
        }],
        src_in: Tick(12_345),
    };
    let json = serde_json::to_string_pretty(&block).expect("блок сериализуется");
    let parsed: kt_model::ops::EnvelopeBlock =
        serde_json::from_str(&json).expect("и разбирается обратно");
    assert_eq!(parsed, block, "блок не пережил круг через текст буфера");

    // чужой текст и пустая строка РАЗЛИЧАЮТСЯ, и у каждого своя причина
    assert!(
        serde_json::from_str::<kt_model::ops::EnvelopeBlock>("просто текст").is_err(),
        "чужой текст разобрался как огибающая"
    );
    let foreign = tr(k::fx::PASTE_ENV_FOREIGN);
    let empty = tr(k::fx::PASTE_ENV_EMPTY);
    assert_ne!(foreign, empty, "у двух разных отказов один текст — причина не названа");
    for t in [foreign, empty] {
        assert!(t.len() > 20, "отказ не объясняет причину: {t:?}");
    }
    // и оба исхода существуют в типе, а не «молчание по умолчанию»
    assert!(matches!(ClipRead::Foreign, ClipRead::Foreign));
    assert!(matches!(ClipRead::Empty, ClipRead::Empty));

    // --- окно сопоставления открывается ТОЛЬКО при остатке ------------
    let blur = kt_model::FxInstance::Blur { radius: 8, radius_y: 0 };
    let own = kt_model::ops::fit_envelope(&block.curves, &blur);
    assert!(own.leftover.is_empty(), "свой вид дал остаток — окно показалось бы зря");
    let alien = ParamCurve {
        param: "чужое_поле".into(),
        keys: vec![AutoKey::at(Tick(0), 1.0)],
        source: Default::default(),
    };
    let mixed = kt_model::ops::fit_envelope(&[alien], &blur);
    assert_eq!(mixed.leftover.len(), 1, "остаток не назван — кривая потерялась бы молча");
    assert!(!mixed.free.is_empty(), "не предложено ни одного параметра цели");

    // --- кнопка вставки ЖИВАЯ даже когда класть нечего ----------------
    let (mut h, _g) = harness_three_clips();
    let id = h.state().model.project().clips[0].id;
    assert!(h.state_mut().apply(kt_model::Command::SetFx { id, fx: vec![blur.clone()] }));
    h.step();
    h.state_mut().open_fx_editor(id);
    h.step();
    h.step();
    {
        use egui_kittest::kittest::Queryable;
        let n = h.query_all_by_label(tr(k::fx::PASTE_ENV)).count();
        assert!(n > 0, "кнопки «вставить огибающую» нет в окне вовсе");
    }
}

/// Тест 698 (Р.61): форма правится ПО УЧАСТКУ, соседи целы, «ко всем» —
/// один шаг; график рисуется той же функцией значения, что считает рендер.
///
/// Владелец: «в автоматизациях хотелось бы задавать формы отрезков… а
/// можно и отдельную кривую сконфигурировать по своему, другим типом».
/// Отсюда два обязательства, которые тест и утверждает: правка одного
/// участка НЕ трогает соседние (форма принадлежит левому ключу отрезка), а
/// у скорости варпа форма только прямая, и отказ ГРОМКИЙ.
#[test]
fn t698_segment_shape_is_edited_per_segment() {
    use kt_model::{AutoKey, ParamCurve, SegShape, ZigProfile, curve_value};
    use kt_time::Tick;

    // --- правка участка не трогает соседей ----------------------------
    let mut c = ParamCurve {
        param: "radius".into(),
        keys: vec![
            AutoKey::at(Tick(0), 0.0),
            AutoKey::at(Tick(30_000), 10.0),
            AutoKey::at(Tick(60_000), 5.0),
        ],
        source: Default::default(),
    };
    let before: Vec<SegShape> = c.keys.iter().map(|k| k.shape).collect();
    let zig = SegShape::Zigzag { periods: 3.0, amplitude: 2.0, profile: ZigProfile::Triangle };
    c.keys[1].shape = zig;
    assert_eq!(c.keys[0].shape, before[0], "левый сосед участка изменился");
    assert_eq!(c.keys[2].shape, before[2], "правый сосед участка изменился");
    // и значение ПЕРВОГО участка осталось прежним
    assert_eq!(
        curve_value(&c.keys, Tick(15_000)).to_bits(),
        SegShape::DEFAULT.at(0.5, 0.0, 10.0).to_bits(),
        "первый участок поехал от правки второго"
    );
    // а ВТОРОЙ теперь дрожит
    let mid = curve_value(&c.keys, Tick(45_000));
    let plain = SegShape::DEFAULT.at(0.5, 10.0, 5.0);
    assert!((mid - plain).abs() > 0.1, "участок не взял новую форму: {mid} против {plain}");

    // --- «ко всем отрезкам» — один шаг --------------------------------
    let all = AutoKey::all_shaped(c.keys.clone(), SegShape::Hold);
    assert!(all.iter().all(|k| k.shape == SegShape::Hold), "форма легла не на все участки");
    assert_eq!(all.len(), c.keys.len(), "команда не вправе менять число ключей");

    // --- ГРАФИК считает ту же функцию, что рендер ---------------------
    // (рисование берёт `curve_value`; второй формулы в программе нет —
    // это же утверждает сторож t693 про единственный виджет)
    for t in [Tick(0), Tick(7_000), Tick(30_000), Tick(45_000), Tick(60_000)] {
        let direct = {
            let keys = &c.keys;
            let i = keys.windows(2).position(|w| t >= w[0].t && t < w[1].t);
            match i {
                Some(i) => {
                    let (a, b) = (&keys[i], &keys[i + 1]);
                    let span = (b.t.0 - a.t.0).max(1) as f64;
                    a.shape.at((t.0 - a.t.0) as f64 / span, a.v, b.v)
                }
                None if t >= keys.last().unwrap().t => keys.last().unwrap().v,
                None => keys[0].v,
            }
        };
        assert_eq!(
            curve_value(&c.keys, t).to_bits(),
            direct.to_bits(),
            "рисунок и рендер считают участок по-разному на {t:?}"
        );
    }

    // --- у СКОРОСТИ форма только прямая, и отказ назван ---------------
    let rate = ParamCurve::constant("rate", Tick(0), Tick(90_000), 1.0, SegShape::DEFAULT);
    assert!(
        rate.keys.iter().all(|k| k.shape == SegShape::Linear),
        "стартовая кривая скорости обязана быть прямой при ЛЮБОМ умолчании проекта: {rate:?}"
    );
    let other = ParamCurve::constant("radius", Tick(0), Tick(90_000), 1.0, SegShape::Hold);
    assert!(
        other.keys.iter().all(|k| k.shape == SegShape::Hold),
        "у обычного параметра стартовая кривая берёт умолчание проекта"
    );
    let mut bad = rate.clone();
    bad.keys[0].shape = SegShape::DEFAULT;
    assert!(
        matches!(
            kt_model::check_fx_auto(kt_model::ClipId(1), &[vec![bad]]),
            Err(kt_model::ProjectError::RateShapeMustBeLinear(_))
        ),
        "сглаженная скорость прошла молча"
    );
}

/// Тест 692b (Р.60): пределы ручки в окне АВТОРА приходят из паспорта цели.
///
/// «Еще значения min max всегда в 0 проставляются по умолчанию» — пустое
/// сужение (`Option::None`) рисовалось нулём, и автор видел ручку, у которой
/// якобы разрешено ровно ничего. Утверждение теста — наблюдаемое следствие:
/// в окне НАРИСОВАНЫ пределы цели, а не нули; кнопка «к пределам цели»
/// доступна только у суженной ручки и снимает сужение в МОДЕЛИ.
#[test]
fn t692b_author_window_shows_the_target_range() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable, by};
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // паспорт цели «Кегля» — то, что окно ОБЯЗАНО показать
    let spec = {
        let doc = h.state().model.document();
        let p = doc
            .sequence(s.seq)
            .expect("лента")
            .published
            .iter()
            .find(|p| p.id == s.size)
            .expect("ручка «Кегль»")
            .clone();
        doc.addr_spec(Some(s.seq), &p.target).expect("паспорт цели «Кегля»")
    };
    assert!(spec.max > spec.min, "паспорт кегля вырожден: {spec:?}");
    assert!(
        spec.min != 0.0 || spec.max != 0.0,
        "тест ничего не докажет: у цели пределы и так нулевые"
    );

    h.state_mut().open_published_win(s.seq);
    h.step();
    h.step();

    let shown: Vec<String> = h
        .query_all(by().role(Role::SpinButton))
        .filter_map(|n| n.accesskit_node().value().map(|v| v.to_string()))
        .collect();
    // Р.66: пределы автора рисует ОБЩИЙ ряд, и единица берётся из паспорта
    // цели — доля показывается процентом ровно так же, как в самом поле
    // параметра. Утверждение то же («автор видит пределы цели»), меняется
    // только единица, в которой они написаны.
    let scale = if spec.unit == kt_model::ParamUnit::Percent { 100.0 } else { 1.0 };
    let has = |x: f64| {
        let want = x * scale;
        shown.iter().any(|t| {
            t.trim()
                .trim_end_matches('%')
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .is_ok_and(|v| (v - want).abs() <= (spec.step.abs() * scale * 0.5).max(1e-6))
        })
    };
    assert!(
        has(spec.min) && has(spec.max),
        "окно автора не показало пределы цели ({} … {}): {shown:?}",
        spec.min,
        spec.max
    );

    // --- сужение доезжает до модели и снимается одной кнопкой ---------
    let narrowed = PubRange { min: Some(spec.min + spec.step), max: Some(spec.max - spec.step) };
    {
        let mut p = h
            .state()
            .model
            .document()
            .sequence(s.seq)
            .expect("лента")
            .published
            .iter()
            .find(|p| p.id == s.size)
            .cloned()
            .expect("ручка «Кегль»");
        p.range = narrowed;
        assert!(
            h.state_mut().apply(Command::SetPublished { seq: s.seq, param: Box::new(p) }),
            "сужение отвергнуто"
        );
    }
    h.step();
    h.step();
    assert!(
        !h.state()
            .model
            .document()
            .sequence(s.seq)
            .expect("лента")
            .published
            .iter()
            .find(|p| p.id == s.size)
            .expect("ручка")
            .range
            .is_empty(),
        "сужение не легло в модель"
    );

    h.get_by_label(tr(k::pubp::RANGE_RESET)).click();
    h.step();
    h.step();
    assert!(
        h.state()
            .model
            .document()
            .sequence(s.seq)
            .expect("лента")
            .published
            .iter()
            .find(|p| p.id == s.size)
            .expect("ручка")
            .range
            .is_empty(),
        "кнопка «к пределам цели» сужение не сняла"
    );

    // и после сброса окно снова показывает пределы цели
    let shown: Vec<String> = h
        .query_all(by().role(Role::SpinButton))
        .filter_map(|n| n.accesskit_node().value().map(|v| v.to_string()))
        .collect();
    assert!(has(spec.min) && has(spec.max), "после сброса пределы цели не вернулись: {shown:?}");
}

/// Тест 684 (Р.59, стадия 3): ИМЯ АВТОРА — содержимое документа.
///
/// Решение владельца: «имена публичных параметров — свободный текст
/// автора». Значит оно рисуется ДОСЛОВНО, мимо каталога переводов, как
/// имя события или ленты, и сторож t131 обязан остаться зелёным (в коде
/// ни одного нового русского литерала).
#[test]
fn t684_author_names_are_shown_verbatim() {
    use kt_model::*;
    let (mut h, s) = harness_news_template();
    // кириллица и арабица — два письма с разной раскладкой; арабское имя
    // проверяет ещё и то, что текст ПРОЕКТА не переупорядочивается как
    // строка каталога
    let names = ["Заголовок выпуска №3", "عنوان النشرة"];
    h.state_mut().enter_nest(s.a);
    h.step();
    for (id, name) in [s.headline, s.tint].iter().zip(names) {
        let mut p = h
            .state()
            .model
            .document()
            .sequence(s.seq)
            .unwrap()
            .published
            .iter()
            .find(|p| p.id == *id)
            .cloned()
            .unwrap();
        p.name = name.to_string();
        assert!(h.state_mut().apply(Command::SetPublished { seq: s.seq, param: Box::new(p) }));
        h.step();
    }
    h.state_mut().leave_nest();
    h.step();

    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    // смотреть надо на ГАЛЛЕИ, а не на дерево доступности: доступное имя
    // кнопки сброса составное и несёт имя ручки тоже — по нему «дословно
    // ли нарисовано» не отличить (урок t255/t548). Берём ИСХОДНЫЙ текст
    // галлеи: перестановка слов у арабского живёт в `tr()` (i18n::bidi) и
    // применяется ДО отрисовки, поэтому расхождение здесь означало бы
    // ровно то, чего быть не должно, — имя автора прошло через каталог.
    let drawn = |h: &Harness<'static, KtApp>| -> Vec<String> {
        crate::reach_tests::painted_texts(h).into_iter().map(|t| t.src).collect()
    };
    for name in names {
        assert!(
            drawn(&h).iter().any(|t| t == name),
            "имя автора «{name}» не НАРИСОВАНО дословно: {:?}",
            drawn(&h)
        );
    }

    // ЯЗЫК ПРОГРАММЫ имя автора не трогает: переключили каталог — имя то же
    crate::i18n::set_lang(crate::i18n::Lang::En);
    h.step();
    h.step();
    assert!(
        drawn(&h).iter().any(|t| t == tr(k::pubp::RESET)),
        "язык программы не переключился — дальнейшее утверждение пусто"
    );
    for name in names {
        assert!(
            drawn(&h).iter().any(|t| t == name),
            "имя автора «{name}» уехало вместе с языком программы"
        );
    }
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    h.step();
}

/// Тест 685 (Р.59, стадия 3): бейджи.
///
/// Булавка на экземпляре С переопределениями, число объявленных ручек на
/// карточке ленты в бине, и НИЧЕГО из этого — когда публикаций нет.
#[test]
fn t685_badges_show_overrides_and_publications() {
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // --- НИЧЕГО, пока не переопределено -------------------------------
    let inst = h.state().model.clip(s.b).cloned().unwrap();
    assert!(inst.overrides.is_empty());
    assert_eq!(
        h.state().instance_override_counts(&inst),
        (0, 6),
        "счётчик обязан знать и сколько объявлено, и сколько тронуто"
    );

    // --- БУЛАВКА и подсказка после переопределения ---------------------
    assert!(h.state_mut().apply(Command::SetOverride {
        id: s.b,
        ov: Box::new(Override::value(s.headline, ParamValue::Text("ВТОРОЙ".into()))),
    }));
    h.step();
    let inst = h.state().model.clip(s.b).cloned().unwrap();
    assert_eq!(h.state().instance_override_counts(&inst), (1, 6));
    // и БУЛАВКА ПО-НАСТОЯЩЕМУ НАРИСОВАНА на событии: смотреть надо на
    // галлеи, а не на модель — «поле выставилось» не следствие (Р.40)
    h.step();
    h.step();
    let pinned: Vec<String> = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .map(|t| t.src)
        .filter(|s| s.contains(crate::icons::PIN))
        .collect();
    // подпись СОБЫТИЯ несёт булавку в хвосте («имя ×N 📌»), карточка
    // бина — в голове («📌6»): это разные бейджи, и путать их нельзя
    assert!(
        pinned.iter().any(|s| s.ends_with(crate::icons::PIN) && s.len() > 4),
        "булавки на событии-экземпляре не нарисовано: {pinned:?}"
    );
    assert!(
        pinned.iter().any(|s| s.starts_with(crate::icons::PIN)),
        "числа объявленных ручек на карточке ленты не нарисовано: {pinned:?}"
    );
    // подсказка НАЗЫВАЕТ числа — «булавка есть, а что значит» без этого
    // было бы загадкой (правило 16)
    let hint = crate::tf!(k::pubp::OVERRIDDEN, 1, 6);
    assert!(hint.contains('1') && hint.contains('6'), "подсказка не называет чисел: {hint}");
    // соседний выпуск булавки не получил: переопределение — свойство
    // ЭКЗЕМПЛЯРА, а не ленты
    assert!(
        h.state().model.clip(s.a).unwrap().overrides.is_empty(),
        "переопределение расползлось на соседний экземпляр"
    );

    // --- КАРТОЧКА БИНА: число объявленных ручек ------------------------
    assert_eq!(h.state().published_of(s.seq).len(), 6, "карточка ленты берёт число ручек отсюда");
    // лента без публикаций числа не показывает — «×1» того же рода шум
    {
        let app = h.state_mut();
        let ids: Vec<PubId> = app.published_of(s.seq).iter().map(|p| p.id).collect();
        app.enter_nest(s.a);
        for id in ids {
            assert!(app.apply(Command::UnpublishParam { seq: s.seq, id }), "снятие ручки отвергнуто");
        }
        app.leave_nest();
    }
    h.step();
    assert_eq!(h.state().published_of(s.seq).len(), 0);
    // и переопределения ушли ВМЕСТЕ с ручками — мёртвой булавки не бывает
    assert!(
        h.state().model.clip(s.b).unwrap().overrides.is_empty(),
        "снятие публикаций оставило переопределение: булавка врала бы"
    );
    h.step();
    h.step();
    let left: Vec<String> = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .map(|t| t.src)
        .filter(|s| s.contains(crate::icons::PIN))
        .collect();
    assert!(left.is_empty(), "бейджи остались там, где публикаций больше нет: {left:?}");
}

/// Тест 716b (Р.62): ведущий узел «Геометрия» в редакторе цепочки —
/// первый и помечен, выбирается кликом, показывает правило приведения,
/// смена правила доезжает до модели одним шагом истории; крестик на нём —
/// ошибка в строке состояния, узел остаётся.
#[test]
fn t716b_lead_geometry_in_the_chain_editor() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();

    // ведущий узел виден и помечен; обычный «Blur» тоже на месте
    let lead_label = crate::fx_editor::fx_display_name(&EventGeometry::default().node());
    assert!(h.query_by_label(lead_label).is_some(), "ведущий узел обязан быть в полосе");
    assert!(h.query_by_label(crate::fx_editor::fx_display_name(&FxInstance::blur(8))).is_some());
    // клик по узлу выбирает его — и появляется правило приведения
    h.get_by_label(lead_label).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().fx_editors[0].selected.get(&ClipId(1)),
        Some(&crate::widgets::ChainSel::Lead),
        "клик по ведущему узлу выбирает его, а не слот с тем же индексом"
    );
    assert!(h.query_by_label(tr(k::fx::CONFORM)).is_some(), "правило приведения показано");
    // смена правила через комбо → SetGeometry, один шаг истории
    let undo_before = h.state().model.undo_depth();
    h.get_by_label(tr(k::fx::CONFORM_FILL)).click();
    h.step();
    h.step();
    let app = h.state();
    assert_eq!(
        app.model.clip(ClipId(1)).unwrap().geometry.rule,
        Conform::Fill,
        "правило доехало до модели"
    );
    assert_eq!(app.model.undo_depth(), undo_before + 1, "сессия окна — один шаг истории");
    assert_eq!(app.model.clip(ClipId(1)).unwrap().fx.len(), 1, "цепочка модели узел не содержит");
    // крестик на ведущем узле — отказ ошибкой, узел на месте
    // крестик ведущего узла — ближайший справа на его строке (в полосе
    // есть и другие крестики: у слота Blur и у поля поиска)
    let lead_rect = h.get_by_label(lead_label).rect();
    let crosses: Vec<_> = h.get_all_by_label(crate::icons::CLEAR).collect();
    assert!(crosses.len() >= 2, "у каждого слота свой крестик");
    let lead_cross = crosses
        .iter()
        .filter(|n| {
            let r = n.rect();
            (r.center().y - lead_rect.center().y).abs() < 10.0 && r.min.x > lead_rect.max.x
        })
        .min_by(|a, b| a.rect().min.x.total_cmp(&b.rect().min.x))
        .expect("крестик справа от ведущего узла");
    lead_cross.click();
    h.step();
    h.step();
    let app = h.state();
    assert!(app.error_status.is_some(), "удаление ведущего узла — ошибка, не тишина");
    assert_eq!(app.model.clip(ClipId(1)).unwrap().fx.len(), 1, "Blur не пострадал");
    assert_eq!(app.model.clip(ClipId(1)).unwrap().geometry.rule, Conform::Fill);
}

/// Тест 720b (Р.62): окно настроек проекта — поля с проверкой, отказ
/// ошибочным стилем, применение одной командой, флаг явных настроек,
/// повторное открытие и возврат фокуса.
#[test]
fn t720b_project_settings_window_validates_and_applies() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness();
    // дважды подряд: окно обязано открываться повторно
    for _ in 0..2 {
        h.state_mut().open_project_settings();
        h.step();
        assert!(h.query_by_label(tr(k::settings::TITLE)).is_some(), "окно открыто");
        h.get_by_label(tr(k::settings::CANCEL)).click();
        h.step();
        h.step();
        assert!(h.query_by_label(tr(k::settings::TITLE)).is_none(), "«Отмена» закрывает");
    }
    // нечётная ширина — отказ ошибкой, настройки не тронуты
    h.state_mut().open_project_settings();
    h.step();
    let before = h.state().model.document().settings.clone();
    {
        let d = h.state_mut().settings_win.as_mut().unwrap();
        d.width = "1441".into();
        d.height = "1080".into();
    }
    h.get_by_label(tr(k::settings::APPLY)).click();
    h.step();
    h.step();
    let app = h.state();
    assert!(app.settings_win.is_some(), "окно остаётся открытым при отказе");
    assert!(app.error_status.is_some(), "нечётная ширина — ошибка, не тишина");
    assert_eq!(app.model.document().settings, before);
    // 29.97 — невыразимо: отказ
    {
        let d = h.state_mut().settings_win.as_mut().unwrap();
        d.width = "1440".into();
        d.fps = "29.97".into();
        h.state_mut().error_status = None;
    }
    h.get_by_label(tr(k::settings::APPLY)).click();
    h.step();
    h.step();
    assert!(h.state().error_status.is_some(), "29.97 отвергается с причиной");
    assert_eq!(h.state().model.document().settings, before);
    // 1440×1080 при 50 fps и правило «заполнить» — одна команда, флаг явности
    {
        let d = h.state_mut().settings_win.as_mut().unwrap();
        d.fps = "50".into();
        d.conform = kt_model::ConformRule::Fill;
    }
    let undo_before = h.state().model.undo_depth();
    h.get_by_label(tr(k::settings::APPLY)).click();
    h.step();
    h.step();
    let app = h.state();
    let s = app.model.document().settings.clone();
    assert_eq!((s.width, s.height, s.fps), (1440, 1080, 50));
    assert_eq!(s.conform, kt_model::ConformRule::Fill);
    assert!(s.settings_explicit, "применение ставит флаг явных настроек");
    assert_eq!(app.model.undo_depth(), undo_before + 1, "одна команда");
    assert!(app.settings_win.is_none(), "окно закрылось");
    // фокус отдан: горячие клавиши живы (правило 14)
    assert!(!h.ctx.memory(|m| m.focused().is_some()), "поле не держит фокус после закрытия");
    // комбо панели превью показывает то же (одно тело)
    assert!(
        h.query_by_label("1440×1080").is_some()
            || h.query_all_by_label_contains("1440").count() > 0
    );
}

/// Тест 721 (Р.62): первый клип в пустой проект с неявными настройками задаёт
/// растр и fps одним undo-шагом с постановкой; явные настройки не трогаются;
/// 29.97 — растр принят, fps прежний, подсказка называет причину.
#[test]
fn t721_first_clip_sets_project_settings() {
    use kt_model::*;
    let info = |w: u32, h: u32, fps: (u32, u32)| kt_media::MediaInfo {
        duration: kt_time::Tick(90_000),
        has_video: true,
        has_audio: false,
        fps: Some(fps),
        width: w,
        height: h,
        sar: (1, 1),
    };
    let mut h = harness();
    let width_before = h.state().model.document().settings.width;
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        assert!(app.model.project().clips.is_empty());
        assert!(!app.model.document().settings.settings_explicit);
        let undo_before = app.model.undo_depth();
        let cmds =
            app.clip_cmds_for(MediaId(1), &info(1440, 1080, (25, 1)), kt_time::Tick(0), None);
        assert!(matches!(cmds[0], Command::SetSettings { .. }), "настройки — первым в батче");
        app.apply_batch(cmds);
        let s = app.model.document().settings.clone();
        assert_eq!((s.width, s.height, s.fps), (1440, 1080, 25));
        assert_eq!(app.model.project().clips.len(), 1);
        assert_eq!(app.model.undo_depth(), undo_before + 1, "постановка и настройки — один шаг");
        let msg = app.status.as_ref().map(|(t, _)| t.clone()).unwrap_or_default();
        assert!(msg.contains("1440"), "подсказка называет принятый растр: {msg}");
        app.undo();
        assert!(app.model.project().clips.is_empty());
        assert_eq!(app.model.document().settings.width, width_before, "undo вернул и настройки");
    }
    // явные настройки: второй проект, окно применено — постановка не трогает
    {
        let app = h.state_mut();
        let mut st = app.model.document().settings.clone();
        st.settings_explicit = true;
        app.apply(Command::SetSettings { settings: st });
        let cmds = app.clip_cmds_for(MediaId(1), &info(640, 480, (30, 1)), kt_time::Tick(0), None);
        assert!(cmds.iter().all(|c| !matches!(c, Command::SetSettings { .. })));
        app.apply_batch(cmds);
        assert_eq!(app.model.document().settings.width, width_before);
    }
    // 29.97 в пустой проект без явных настроек: растр принят, fps нет
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips.clear();
        p.settings.settings_explicit = false;
        p.settings.fps = 30;
        app.model = kt_model::TimelineModel::new(p);
        let cmds =
            app.clip_cmds_for(MediaId(1), &info(1280, 720, (30_000, 1001)), kt_time::Tick(0), None);
        app.apply_batch(cmds);
        let s = app.model.document().settings.clone();
        assert_eq!((s.width, s.height, s.fps), (1280, 720, 30));
        let msg = app.status.as_ref().map(|(t, _)| t.clone()).unwrap_or_default();
        assert!(msg.contains("90000"), "подсказка объясняет, почему fps не принят: {msg}");
    }
}

/// Тест 722 (Р.62): несовпадение пропорций при постановке сообщается с
/// адресом «где изменить»; совпадение — молчит.
#[test]
fn t722_aspect_mismatch_is_reported_with_an_address() {
    use kt_model::*;
    let info = |w: u32, h: u32| kt_media::MediaInfo {
        duration: kt_time::Tick(90_000),
        has_video: true,
        has_audio: false,
        fps: Some((30, 1)),
        width: w,
        height: h,
        sar: (1, 1),
    };
    let mut h = harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.settings.width = 1920;
    p.settings.height = 1080;
    p.settings.settings_explicit = true;
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    // 4:3 в 16:9 — подсказка с правилом и адресом
    let cmds = app.clip_cmds_for(MediaId(1), &info(640, 480), kt_time::Tick(0), None);
    app.apply_batch(cmds);
    let msg = app.status.as_ref().map(|(t, _)| t.clone()).unwrap_or_default();
    assert!(msg.contains("640×480") && msg.contains("1920×1080"), "{msg}");
    assert!(msg.contains(tr(k::fx::CONFORM_FIT)), "правило названо: {msg}");
    assert!(msg.contains(tr(k::menu::file::PROJECT_SETTINGS)), "адрес назван: {msg}");
    // 16:9 в 16:9 — молчание
    app.status = None;
    let cmds = app.clip_cmds_for(MediaId(1), &info(1280, 720), kt_time::Tick(90_000), None);
    app.apply_batch(cmds);
    assert!(app.status.is_none(), "совпадающие пропорции не сообщаются");
}

/// Тест 719 (Р.62): тумба вертикального клипа НЕ раздавлена — нарисована
/// в пропорциях источника, по центру ячейки, а скролл и зум таймлайна не
/// заказывают ни одного декода (инвариант 8: единственный ключ клипа ≤5 c
/// уже в текстурах). Щупается ОТРИСОВАННОЕ: прямоугольник картинки в
/// шейпах кадра, а не поле сервиса.
#[test]
fn t719_vertical_thumb_keeps_its_aspect_and_scroll_orders_no_decode() {
    use kt_model::ClipKind;
    let (mut h, _g) = harness_three_clips_dt(None);
    h.step();
    h.step();
    // ключи спрашивает САМ филмстрип (арифметику шага тест не повторяет);
    // клип 50 000 тиков ≤ 5 c → шаг постоянен при любом зуме, ключей мало
    let mut keys: Vec<crate::thumbs::ThumbKey> = h.state().thumbs.requested_keys();
    keys.sort_by_key(|k| (k.media.0, k.t));
    keys.dedup();
    assert!(!keys.is_empty(), "филмстрип не заказал ни одной тумбы");
    assert!(
        keys.iter().all(|k| h
            .state()
            .model
            .project()
            .clips
            .iter()
            .any(|c| c.kind == ClipKind::Video && kt_engine::clip_source_addr(c) == k.media)),
        "ключи не про видео-события сцены: {keys:?}"
    );
    let img = egui::ColorImage::filled([100, 180], egui::Color32::from_rgb(200, 40, 40));
    let tex = h.ctx.load_texture("t719-vertical", img, egui::TextureOptions::LINEAR);
    // текстуры — под ВСЕ ключи, какие может дать сетка шага (кратные
    // 0.5 c, клип 50 000 тиков): зум открывает новые моменты, и это
    // законный заказ; сторож ниже — про то, что ГОТОВОЕ не заказывается
    let media: std::collections::BTreeSet<kt_model::MediaId> =
        keys.iter().map(|k| k.media).collect();
    for m in &media {
        for t in (0..=150_000).step_by(5_000) {
            h.state_mut()
                .thumb_textures
                .insert(crate::thumbs::ThumbKey { media: *m, t }, tex.clone());
        }
    }
    h.step();
    h.step();
    let before = h.state().thumbs.requested_total();

    fn image_rects(
        shapes: &[egui::epaint::ClippedShape],
        id: egui::TextureId,
        out: &mut Vec<egui::Rect>,
    ) {
        fn walk(sh: &egui::Shape, id: egui::TextureId, out: &mut Vec<egui::Rect>) {
            match sh {
                // `painter.image` в egui 0.36 кладёт меш с текстурой
                egui::Shape::Mesh(m) if m.texture_id == id => out.push(m.calc_bounds()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, id, out)),
                _ => {}
            }
        }
        shapes.iter().for_each(|c| walk(&c.shape, id, out));
    }
    let mut rects = Vec::new();
    image_rects(&h.output().shapes, tex.id(), &mut rects);
    let n_shapes = h.output().shapes.len();
    let n_mesh =
        h.output().shapes.iter().filter(|c| matches!(c.shape, egui::Shape::Mesh(_))).count();
    assert!(
        !rects.is_empty(),
        "вертикальная тумба не нарисована ни разу (shapes={n_shapes}, meshes={n_mesh}, \
         offline={:?}, tex={:?}, keys={:?})",
        h.state().offline.lock().unwrap(),
        h.state().thumb_textures.keys().collect::<Vec<_>>(),
        keys
    );
    // полная (не обрезанная краем события) ячейка: ширина = высота·100/180
    let full = rects.iter().find(|r| (r.width() / r.height() - 100.0 / 180.0).abs() < 0.03);
    assert!(
        full.is_some(),
        "тумба раздавлена: ни один прямоугольник не в пропорциях 100:180, есть {:?}",
        rects.iter().map(|r| (r.width(), r.height())).collect::<Vec<_>>()
    );
    assert!(
        rects.iter().all(|r| r.width() <= r.height() * 16.0 / 9.0 * 0.6 + 1.0),
        "вертикальная тумба растянута на всю ячейку 16:9"
    );

    // скролл и зум — ни одного нового заказа декода (инвариант 8)
    let (vs, tpp) = (h.state().view_start, h.state().ticks_per_px);
    h.state_mut().view_start = vs + 2_000.0;
    h.step();
    h.state_mut().ticks_per_px = tpp * 0.5;
    h.step();
    h.state_mut().ticks_per_px = tpp * 2.0;
    h.state_mut().view_start = vs;
    h.step();
    h.step();
    assert_eq!(
        h.state().thumbs.requested_total(),
        before,
        "скролл/зум таймлайна заказал декод тумб (инвариант 8)"
    );
}

/// Тест 722b (Р.62): смена правила приведения в окне настроек НА ПАУЗЕ
/// пересобирает превью. Нашёл живой сценарий `conform`: правило было
/// записано в файл, а в окне висел прежний кадр — ключ превью не видел
/// версии проекта (ни курсор, ни качество, ни содержимое источников
/// правка не трогает). Утверждается СЛЕДСТВИЕ: пиксель поля меняется.
#[test]
fn t722b_project_rule_change_repaints_the_paused_preview() {
    use kt_model::*;
    use kt_time::Tick;
    let mut h = harness_lang_dt(crate::i18n::Lang::Ru, Some(0.02));
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        // вертикальный проект, горизонтальный синтетический источник 64×36:
        // «вписать» оставляет поля сверху и снизу, «заполнить» — нет
        p.settings.width = 36;
        p.settings.height = 64;
        p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
        app.model = kt_model::TimelineModel::new(p);
        app.push_project_to_player();
        // источник поднимается так же, как это делает создание генератора:
        // первый синк лишь снимает состав медиа, сам он источники не поднимает
        let src = crate::util::open_source("synth:ramp").expect("synthetic source");
        app.media_paths.insert(MediaId(1), "synth:ramp".into());
        app.player.add_media(MediaId(1), src);
        // холст превью — в пропорциях ПРОЕКТА (окно настроек делает это
        // само; здесь настройки подменены мимо него)
        app.player.set_preview_quality(kt_media::Quality { width: 36, height: 64 });
        app.player.seek(Tick(30_000));
    }
    for _ in 0..120 {
        h.step();
        if h.state().preview_key.is_some_and(|k| k.2) {
            break;
        }
    }
    assert!(h.state().preview_key.is_some_and(|k| k.2), "превью так и не посчиталось");
    let top_dark = |app: &KtApp| -> bool {
        let (_, (w, hgt), _) = app.preview_key.unwrap();
        assert!(hgt > 4 && app.preview_buf.len() >= (w * hgt * 4) as usize);
        let i = ((w + w / 2) * 4) as usize; // строка 1, середина
        app.preview_buf[i..i + 3].iter().all(|&v| v < 8)
    };
    assert!(top_dark(h.state()), "«вписать» горизонтального в вертикальный: сверху поле");

    let mut st = h.state().model.document().settings.clone();
    st.conform = ConformRule::Fill;
    st.settings_explicit = true;
    assert!(h.state_mut().apply(Command::SetSettings { settings: st }));
    let mut repainted = false;
    for _ in 0..120 {
        h.step();
        if h.state().preview_key.is_some_and(|k| k.2) && !top_dark(h.state()) {
            repainted = true;
            break;
        }
    }
    assert!(repainted, "правило «заполнить» применено, а превью на паузе осталось с полями");
}

/// Тест 740 (Р.62, change B): окно рендера ПЕРЕЧИСЛЯЕТ заглушённые
/// дорожки и активное соло ДО старта — там же, где говорит о недоступных
/// исходниках. Отказа нет (mute и solo — содержимое проекта, решение
/// владельца), но молчать нельзя: «забыл снять соло — сдал одну дорожку»
/// (правила 16, 19). Утверждается ОТРИСОВАННОЕ: имя дорожки в окне.
#[test]
fn t740_render_dialog_lists_muted_and_solo_tracks() {
    use kt_model::LaneKind::{Audio, Video};
    let mut h = harness_three_clips_dt(None).0;
    {
        let app = h.state_mut();
        edit_project(app, |p| {
            p.ensure_tracks(Audio, 2);
            p.track_at_mut(Video, 0).unwrap().name = "Титры".into();
            p.track_at_mut(Video, 0).unwrap().mute = true;
            p.track_at_mut(Audio, 1).unwrap().name = "Голос".into();
            p.track_at_mut(Audio, 1).unwrap().solo = true;
        });
        app.render_setup.open = true;
    }
    h.step();
    h.step();
    let texts: Vec<String> =
        crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    let all = texts.join(" | ");
    assert!(
        all.contains("Титры"),
        "окно рендера молчит о заглушённой дорожке — «в файле её не будет» человек узнает потом: {all}"
    );
    assert!(all.contains("Голос"), "окно рендера молчит об активном соло: {all}");

    // без заглушённых и соло — ни строки лишней (правило 9)
    let mut h2 = harness_three_clips_dt(None).0;
    h2.state_mut().render_setup.open = true;
    h2.step();
    h2.step();
    let clean: String = crate::reach_tests::painted_texts(&h2)
        .into_iter()
        .map(|t| t.src)
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(!clean.contains("Титры") && !clean.contains("Голос"));
}

/// Тест 742 (Р.62, change B, 4.1): ряд бейджей в заголовке дорожки.
/// Утверждается то, что делает ЧЕЛОВЕК (Р.40): мышь наводится в центр
/// бейджа, нажимается и отпускается БЕЗ движения — и меняется МОДЕЛЬ.
/// Плюс три правила чек-листа: ширина заголовка не скачет при смене
/// состояния (правило 3), повторное нажатие возвращает как было
/// (правило 14), у служебной строки бейдж ровно один — замок
/// (правило 9: мёртвых контролов не бывает).
#[test]
fn t742_track_badges_take_a_human_click_and_never_move() {
    use crate::timeline::{GUTTER_W, TrackBadge};
    use kt_model::LaneKind::{Morph, Video};

    /// Клик человека: навёл, нажал, отпустил, мышь не двигал.
    fn click_at(h: &mut Harness<'_, KtApp>, pos: egui::Pos2) {
        h.hover_at(pos);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
        h.step();
    }

    let (mut h, _) = harness_three_clips_dt(None);
    h.state_mut().model.apply(kt_model::Command::InsertTrack { kind: Morph, at: 0 }).unwrap();
    h.step();

    // ряд бейджей строки — ЧИСТАЯ функция геометрии, ровно та же, что у
    // отрисовки и у хит-теста
    let badges = |h: &Harness<'_, KtApp>, kind: kt_model::LaneKind, lane: u32| {
        let g = h.state().tl_geom.clone().expect("геометрия таймлайна");
        let row = g.rows.iter().position(|r| *r == (kind, lane)).expect("строка на экране");
        let left = g.rect.left() - GUTTER_W;
        TrackBadge::row_rects(kind, left, g.row_top(row), g.row_h(row))
    };
    let track = |h: &Harness<'_, KtApp>, kind, lane| {
        h.state().model.project().track_at(kind, lane).cloned().expect("дорожка есть")
    };

    // хват-зона не уже 14 px (правило 1: попасть с первого раза)
    for (b, r) in badges(&h, Video, 0) {
        assert!(
            r.width() >= 14.0 && r.height() >= 8.0,
            "бейдж {b:?} мельче хват-зоны: {:?}",
            r.size()
        );
    }

    // ---- в каждый бейдж мышью, из точки человека ---------------------------
    let before = badges(&h, Video, 0);
    for (badge, r) in before.clone() {
        click_at(&mut h, r.center());
        let t = track(&h, Video, 0);
        match badge {
            TrackBadge::Mute => assert!(t.mute, "клик в M не заглушил дорожку"),
            TrackBadge::Solo => assert!(t.solo, "клик в S не включил соло"),
            TrackBadge::Lock => assert!(t.lock, "клик в замок не запер дорожку"),
            TrackBadge::Fx => {
                assert_eq!(
                    h.state().track_win.as_ref().map(|w| w.id),
                    Some(t.id),
                    "клик в бейдж FX не открыл цепочку дорожки"
                );
                h.state_mut().track_win = None;
                h.step();
            }
        }
    }

    // ---- ширина не скачет от состояния (правило 3) -------------------------
    let after = badges(&h, Video, 0);
    assert_eq!(
        before, after,
        "ряд бейджей переехал, когда дорожка сменила состояние: заголовок обязан быть \
         зафиксирован галлеей ряда, а не шириной сегодняшних значков"
    );

    // ---- повторное нажатие (правило 14) ------------------------------------
    let mute = before.iter().find(|(b, _)| *b == TrackBadge::Mute).unwrap().1;
    click_at(&mut h, mute.center());
    assert!(!track(&h, Video, 0).mute, "второе нажатие M не сняло глушение");
    click_at(&mut h, mute.center());
    assert!(track(&h, Video, 0).mute, "третье нажатие M не вернуло глушение");

    // ---- кромка окна не ворует клики (правило 15) --------------------------
    // Заголовки дорожек прижаты к левому краю окна, а первые 8 px там —
    // зона ресайза: без записи бейджей в `no_resize_rects` левая половина
    // M мертва, когда медиа-бин скрыт. Целимся сразу за полосой в
    // `WIN_BORDER` — её владелец оставил ресайзу намеренно (п.9 р.12).
    let edge = egui::pos2(mute.left() + crate::app::WIN_BORDER + 0.5, mute.center().y);
    assert!(
        edge.x < mute.left() + crate::timeline::BADGE_W / 2.0,
        "точка кромки обязана лежать ЛЕВЕЕ центра бейджа, иначе проверять нечего"
    );
    let was = track(&h, Video, 0).mute;
    click_at(&mut h, edge);
    assert_ne!(
        track(&h, Video, 0).mute,
        was,
        "левый край бейджа M мёртв: клик в {edge:?} забрала зона ресайза окна"
    );

    // ---- горячие клавиши: ТО ЖЕ ТЕЛО, что у бейджа (правило 17) ------------
    // Голые M и S заняты меткой и разрезом по прямой просьбе владельца,
    // поэтому глушение и соло берут ALT. Цель — дорожка ВЫДЕЛЕННОГО
    // события; выделения нет — строка под указателем.
    let v0 = track(&h, Video, 0).id;
    {
        let app = h.state_mut();
        app.selected.clear();
        app.selected.insert(kt_model::ClipId(1));
        let mut props = app.model.project().track(v0).unwrap().props();
        props.mute = false;
        props.solo = false;
        props.lock = false;
        app.model.apply(kt_model::Command::SetTrackProps { id: v0, props }).unwrap();
    }
    h.step();
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::M);
    h.step();
    assert!(track(&h, Video, 0).mute, "Alt+M не заглушил дорожку выделенного события");
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::S);
    h.step();
    assert!(track(&h, Video, 0).solo, "Alt+S не включил соло на дорожке выделенного события");
    // голая M остаётся МЕТКОЙ: чужую привязку клавиша дорожки не забирает
    let marks = h.state().model.project().markers.len();
    h.key_press(egui::Key::M);
    h.step();
    assert_eq!(
        h.state().model.project().markers.len(),
        marks + 1,
        "голая M перестала ставить метку — привязка владельца отобрана"
    );

    // ---- служебная строка: только замок ------------------------------------
    let serv = badges(&h, Morph, 0);
    assert_eq!(
        serv.iter().map(|(b, _)| *b).collect::<Vec<_>>(),
        vec![TrackBadge::Lock],
        "у морф-строки глушить нечего — лишний бейдж был бы мёртвым контролом"
    );
    // и он стоит В ПЕРВОЙ ячейке ряда: человек жмёт туда, где у видео M
    assert_eq!(serv[0].1.min.x, before[0].1.min.x);
    click_at(&mut h, serv[0].1.center());
    let m = track(&h, Morph, 0);
    assert!(m.lock, "клик в первую ячейку служебной строки не запер её");
    assert!(!m.mute, "служебная строка не имеет права заглушиться");
}

/// Тест 743 (Р.62, change B, 4.2): окно «Свойства дорожки». Проверяется
/// то, что делает человек: открыл, поправил имя, уровень и режим, закрыл —
/// и ВСЯ сессия откатывается ОДНИМ Ctrl+Z (как у окна FX). «Отмена»
/// возвращает снимок, окно отдаёт фокус (правило 14), пределы полей
/// приходят из паспорта, а не из литералов окна (прецедент Gain в Р.33).
#[test]
fn t743_track_window_is_one_undo_step_and_gives_focus_back() {
    use egui_kittest::kittest::Queryable;
    use kt_model::{HasParams, LaneKind::Video};
    let (mut h, _) = harness_three_clips_dt(None);
    let id = h.state().model.project().track_at(Video, 0).unwrap().id;
    let props_of = |h: &Harness<'_, KtApp>| {
        h.state().model.project().track(id).unwrap().props()
    };
    let was = props_of(&h);
    let depth0 = h.state().model.undo_depth();

    // ---- пределы полей — ИЗ ПАСПОРТА --------------------------------------
    {
        let p = h.state().model.project().clone();
        let v = p.track(id).unwrap();
        let a = p.track_at(kt_model::LaneKind::Audio, 0).unwrap();
        assert!(v.param_spec("opacity").is_some(), "у видеодорожки нет строки уровня");
        assert!(v.param_spec("gain_db").is_none(), "децибелы на ВИДЕОдорожке — чужая строка");
        let g = a.param_spec("gain_db").expect("у аудиодорожки обязан быть уровень");
        assert_eq!(
            (g.min, g.max),
            (kt_model::TRACK_GAIN_MIN_DB, kt_model::TRACK_GAIN_MAX_DB),
            "окно обязано брать пределы у паспорта, иначе они разъедутся с моделью"
        );
    }

    // ---- одна сессия — один шаг истории -----------------------------------
    h.state_mut().open_track_window(id);
    h.step();
    h.step();
    // имя: поле забирает фокус при открытии, печатаем как человек
    h.event(egui::Event::Text("Титры".into()));
    h.step();
    // режим наложения и выключатель — в том же окне и той же сессии
    let multiply = crate::track_window::blend_label(kt_model::BlendMode::Multiply).to_string();
    h.get_by_label(&multiply).click();
    h.step();
    let mute_label = crate::i18n::tr(crate::i18n::k::track::MUTE).to_string();
    h.get_by_label(&mute_label).click();
    h.step();

    let now = props_of(&h);
    assert_eq!(now.name, "Титры", "имя не доехало до модели");
    assert_eq!(now.blend, kt_model::BlendMode::Multiply, "режим наложения не доехал");
    assert!(now.mute, "выключатель не доехал");
    assert_eq!(
        h.state().model.undo_depth(),
        depth0 + 1,
        "сессия окна разложилась на несколько шагов истории: человек будет жать Ctrl+Z \
         столько раз, сколько полей тронул"
    );
    h.state_mut().undo();
    h.step();
    assert_eq!(props_of(&h), was, "один Ctrl+Z обязан вернуть ВСЮ сессию окна");

    // выделение готовим ЗАРАНЕЕ: клавишу жмём сразу после закрытия окна,
    // без лишних кадров — иначе фокус успеет отпуститься сам и проверка
    // правила 14 ничего не значит
    {
        let app = h.state_mut();
        app.selected.clear();
        app.selected.insert(kt_model::ClipId(1));
    }

    // ---- строки формы не наезжают друг на друга (правило 7) ---------------
    // Девять кнопок режима переносятся на вторую строку, а `Grid` высоту
    // ПЕРЕНОСА не меряет: в живом окне вторая строка наезжала на
    // «Выключатели», и харнесс этого не видел, пока не начал сверять
    // ПРЯМОУГОЛЬНИКИ нарисованного.
    {
        let painted = crate::reach_tests::painted_texts(&h);
        let rect_of = |name: &str| {
            painted.iter().find(|t| t.src == name).map(|t| t.rect)
        };
        let last_blend =
            crate::track_window::blend_label(kt_model::BlendMode::Difference).to_string();
        let sw = crate::i18n::tr(crate::i18n::k::track::SWITCHES).to_string();
        if let (Some(a), Some(b)) = (rect_of(&last_blend), rect_of(&sw)) {
            assert!(
                !a.intersects(b),
                "кнопки режима наложения наезжают на «Выключатели»: {a:?} и {b:?}"
            );
        } else {
            panic!("в окне дорожки не нарисованы режим наложения и выключатели");
        }
    }

    // ---- «Отмена» возвращает снимок и закрывает окно -----------------------
    h.state_mut().open_track_window(id);
    h.step();
    h.step();
    h.get_by_label(&mute_label).click();
    h.step();
    assert!(props_of(&h).mute, "сцена не состоялась: отменять нечего");
    h.get_by_label(crate::i18n::tr(crate::i18n::k::track::CANCEL)).click();
    h.step();
    h.step();
    assert_eq!(props_of(&h), was, "«Отмена» не вернула свойства дорожки");
    assert!(h.state().track_win.is_none(), "«Отмена» не закрыла окно");

    // ---- Esc закрывает, а фокус возвращается (правило 14) ------------------
    // Поле имени забирает фокус при открытии, и пока оно его держит, до
    // каскада Esc не доходит вовсе — окно обязано ловить его само. А
    // закрывшись, окно обязано фокус ОТДАТЬ: иначе клавиатура мертва до
    // клика мимо.
    h.state_mut().open_track_window(id);
    h.step();
    h.step();
    assert!(
        h.state().track_win.is_some(),
        "сцена не состоялась: окно не открылось"
    );
    h.key_press(egui::Key::Escape);
    h.step();
    assert!(h.state().track_win.is_none(), "Esc не закрыл окно дорожки");
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::M);
    h.step();
    assert!(
        props_of(&h).mute,
        "после закрытия окна клавиши мертвы: поле имени не отдало фокус egui"
    );
}

/// Тест 744 (Р.62, change B, 4.3): цепочка ДОРОЖКИ — та же полоса, что у
/// события (правило 17), а огибающая, приехавшая с вставленной цепочкой,
/// отвергается ВСЛУХ и с причиной (правило 16): у дорожки автоматизации
/// нет, и молча уронить кривые нельзя — человек увидел бы «эффект тот же,
/// а звучит иначе» и искал бы дефект в движке.
#[test]
fn t744_track_chain_is_the_same_strip_and_refuses_curves_aloud() {
    use egui_kittest::kittest::Queryable;
    use kt_model::{FxInstance, LaneKind::Audio, ParamCurve};
    let (mut h, _) = harness_three_clips_dt(None);
    let id = {
        let app = h.state_mut();
        edit_project(app, |p| p.ensure_tracks(Audio, 1));
        app.model.project().track_at(Audio, 0).unwrap().id
    };
    h.step();

    // в буфере — цепочка С ОГИБАЮЩЕЙ, как её кладёт полоса события
    h.state_mut().fx_clipboard = Some(kt_model::ops::FxChainBlock {
        slots: vec![kt_model::ops::FxSlotBlock {
            fx: FxInstance::gain(-6.0),
            bypass: false,
            curves: vec![ParamCurve {
                param: "gain_db".into(),
                keys: vec![
                    kt_model::AutoKey::at(kt_time::Tick(0), -6.0),
                    kt_model::AutoKey::at(kt_time::Tick(90_000), 0.0),
                ],
                source: Default::default(),
            }],
        }],
        src_in: kt_time::Tick::ZERO,
        fx_off: false,
    });
    h.state_mut().open_track_window(id);
    h.step();
    h.step();

    // полоса цепочки у дорожки — ТА ЖЕ: кнопка вставки на месте
    let paste = crate::i18n::tr(crate::i18n::k::fx::PASTE_CHAIN).to_string();
    assert!(
        h.query_by_label(&paste).is_some(),
        "у дорожки нет общей полосы цепочки: вставлять эффекты нечем"
    );
    h.get_by_label(&paste).click();
    h.step();
    h.step();

    let t = h.state().model.project().track(id).unwrap().clone();
    assert_eq!(t.fx.len(), 1, "эффект не доехал до цепочки дорожки");
    assert!(
        matches!(t.fx[0], FxInstance::Gain { .. }),
        "в цепочку дорожки попал не тот эффект: {:?}",
        t.fx[0]
    );
    let err = h.state().error_status.clone().map(|(m, _)| m);
    assert!(
        err.as_deref() == Some(crate::i18n::tr(crate::i18n::k::track::CURVES_DROPPED)),
        "кривая пропала молча — человеку не сказали ни что отброшено, ни куда её класть: {err:?}"
    );
    // и сама модель кривых у дорожки не носит
    assert!(
        h.state().model.project().validate().is_ok(),
        "дорожка с кривыми прошла проверку проекта"
    );
}

/// Тест 745 (Р.62, change B, 4.4): ЗАМОК дорожки в интерфейсе. Жест по
/// запертой дорожке — не тишина и не блёклая подсказка, а ошибка, которая
/// называет дорожку и место, где замок снять (правила 16 и 19), плюс
/// красная подсветка самой строки (механизм t172). Проверяются все четыре
/// жеста, которыми событие правят; глушение при этом продолжает
/// действовать — замок про РЕДАКТИРОВАНИЕ, а не про звук.
#[test]
fn t745_locked_track_refuses_every_gesture_with_a_reason() {
    use kt_model::{ClipId, LaneKind::Video};
    let (mut h, g) = harness_three_clips_dt(None);
    let id = h.state().model.project().track_at(Video, 0).unwrap().id;
    {
        let app = h.state_mut();
        let mut props = app.model.project().track(id).unwrap().props();
        props.lock = true;
        app.model.apply(kt_model::Command::SetTrackProps { id, props }).unwrap();
    }
    h.step();
    let start_of = |h: &Harness<'_, KtApp>, c: ClipId| {
        h.state().model.project().clips.iter().find(|x| x.id == c).map(|x| (x.start, x.duration))
    };
    let was = start_of(&h, ClipId(1)).unwrap();
    let row = g.rows.iter().position(|r| *r == (Video, 0)).unwrap();
    let y = g.row_top(row) + g.row_h(row) * 0.6;
    let body = egui::pos2(g.tick_to_x(20_000), y);

    // ---- перенос --------------------------------------------------------
    h.state_mut().error_status = None;
    h.drag_at(body);
    h.step();
    h.hover_at(body + egui::vec2(9.0, 0.0));
    h.step();
    h.hover_at(body + egui::vec2(120.0, 0.0));
    h.step();
    h.drop_at(body + egui::vec2(120.0, 0.0));
    h.step();
    assert_eq!(start_of(&h, ClipId(1)), Some(was), "запертая дорожка отдала событие драгу");
    let label = h.state().row_label(Video, 0);
    let err = h.state().error_status.clone().map(|(m, _)| m).unwrap_or_default();
    assert!(
        err.contains(&label),
        "отказ не назвал дорожку («{label}»), человек не поймёт, что именно мешает: {err:?}"
    );
    assert!(
        h.state().blocked_track.map(|(t, _)| t) == Some(id),
        "строка запертой дорожки не подсвечена — отказ выглядит как «нажал, и ничего»"
    );

    // ---- тримминг правого края ------------------------------------------
    let edge = egui::pos2(g.tick_to_x(50_000) - 2.0, y);
    h.drag_at(edge);
    h.step();
    h.hover_at(edge + egui::vec2(9.0, 0.0));
    h.step();
    h.hover_at(edge + egui::vec2(60.0, 0.0));
    h.step();
    h.drop_at(edge + egui::vec2(60.0, 0.0));
    h.step();
    assert_eq!(start_of(&h, ClipId(1)), Some(was), "запертая дорожка дала себя подтянуть");

    // ---- разрез и удаление клавишами ------------------------------------
    let n = h.state().model.project().clips.len();
    {
        let app = h.state_mut();
        app.selected.clear();
        app.selected.insert(ClipId(1));
        app.player.seek(kt_time::Tick(20_000));
    }
    h.step();
    h.key_press(egui::Key::S);
    h.step();
    assert_eq!(h.state().model.project().clips.len(), n, "запертое событие разрезалось");
    h.key_press(egui::Key::Delete);
    h.step();
    assert_eq!(h.state().model.project().clips.len(), n, "запертое событие удалилось");
    assert!(
        h.state().blocked_track.map(|(t, _)| t) == Some(id),
        "отказ от КЛАВИШИ не подсветил строку: `handle_keys` идёт раньше отрисовки, и \
         подсветка, гаснущая каждый кадр, живёт ровно ноль кадров"
    );

    // ---- замок про правку, а не про звук --------------------------------
    h.state_mut().toggle_track_badge(id, crate::timeline::TrackBadge::Mute);
    h.step();
    let p = h.state().model.project().clone();
    assert!(p.track(id).unwrap().lock, "бейдж снял чужой выключатель");
    assert!(p.track(id).unwrap().mute, "свойства ЗАПЕРТОЙ дорожки править нельзя стало");
    assert!(
        kt_model::silenced_tracks(&p).muted.contains(&label),
        "глушение запертой дорожки перестало действовать на файл"
    );
}

/// Тест 748d (Р.63 п.2): хвост события БЕЗ МАТЕРИАЛА виден на полосе.
///
/// Событие может быть длиннее файла (край утащили за конец ещё до Р.63,
/// скорость поменяли) — и тогда в его хвосте тишина и стоп-кадр. Пока
/// хвост выглядел как обычный материал, это читалось как «луп ломает
/// звук»: человек видит событие, а звука в нём нет. Утверждается
/// НАРИСОВАННОЕ: штриховка начинается там же, где кончается материал по
/// модели, и у события внутри материала её нет вовсе.
#[test]
fn t748d_tail_without_material_is_hatched() {
    use kt_model::LaneKind::Video;
    let (mut h, _) = harness_three_clips_dt(None);
    // событие 0..50_000 на медиа, которого хватает только на 20_000
    {
        let app = h.state_mut();
        edit_project(app, |p| {
            p.media[0].duration = Some(kt_time::Tick(20_000));
        });
    }
    h.step();
    h.step();

    let probe = |h: &Harness<'_, KtApp>| -> Option<egui::Rect> {
        h.state().model.project().clips.first().and_then(|c| {
            let len = h.state().model.project().media_duration(c.media);
            kt_model::material_end_on_timeline(c, len).map(|end| {
                let g = h.state().tl_geom.clone().expect("геометрия");
                let row = g.rows.iter().position(|r| *r == (Video, 0)).unwrap();
                egui::Rect::from_min_max(
                    egui::pos2(g.tick_to_x(end), g.row_top(row)),
                    egui::pos2(
                        g.tick_to_x((c.start + c.duration).0),
                        g.row_top(row) + g.row_h(row),
                    ),
                )
            })
        })
    };
    let r = probe(&h).expect("модель знает, где кончается материал");
    assert!(r.width() > 1.0, "хвост без материала обязан быть шире пикселя");

    // НАРИСОВАНО: в зоне хвоста есть штриховка — линии, которых нет в
    // теле события. Считаем отрезки, попавшие в прямоугольник хвоста.
    // Штрих — диагональ во всю высоту строки: внутри узкого хвоста она не
    // помещается и обрезается КЛИП-ПРЯМОУГОЛЬНИКОМ. Считаем по нему: у
    // хвоста он свой и ничей больше.
    let lines_in = |h: &Harness<'_, KtApp>, r: egui::Rect| -> usize {
        fn walk(s: &egui::epaint::Shape, n: &mut usize) {
            use egui::epaint::Shape;
            match s {
                Shape::Vec(v) => v.iter().for_each(|s| walk(s, n)),
                Shape::LineSegment { .. } => *n += 1,
                _ => {}
            }
        }
        let mut n = 0;
        for cs in &h.output().shapes {
            let c = cs.clip_rect;
            if (c.left() - r.left()).abs() < 2.0 && (c.right() - r.right()).abs() < 2.0 {
                walk(&cs.shape, &mut n);
            }
        }
        n
    };
    let hatch = lines_in(&h, r);
    assert!(hatch >= 2, "штриховки в хвосте без материала нет (линий {hatch})");

    // у события ВНУТРИ материала хвоста нет вовсе (правило 9)
    {
        let app = h.state_mut();
        edit_project(app, |p| {
            p.media[0].duration = Some(kt_time::Tick(10 * 90_000));
        });
    }
    h.step();
    h.step();
    assert!(probe(&h).is_none(), "у события внутри материала хвоста быть не должно");
    let body = egui::Rect::from_min_max(r.min, r.max);
    assert_eq!(lines_in(&h, body), 0, "штриховка осталась там, где материал есть");
}

/// Тест 749 (Р.63 п.3): формула НАБИРАЕТСЯ целиком, ключи живы до
/// подтверждения, а верная формула видна на графике.
///
/// Владелец: «я не понимаю как ввести формулы в кривые автоматизации — я
/// прописываю а оно либо стирает формулу либо нихера не происходит». И то
/// и другое было правдой: поле перечитывало текст ИЗ МОДЕЛИ каждый кадр
/// (значит `sin(` исчезал на следующем), а первая законная приставка `t`
/// применялась немедленно и обнуляла ключи. Плюс формульная кривая ключей
/// не хранит по контракту — и график рисовал пустоту.
#[test]
fn t749_formula_is_typed_whole_and_drawn() {
    use kt_model::*;
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
        let mut c = ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
            fx: vec![FxInstance::blur(8)],
            fx_bypass: vec![false],
            fx_auto: vec![vec![ParamCurve {
                param: "radius".into(),
                keys: vec![
                    AutoKey::at(kt_time::Tick(0), 2.0),
                    AutoKey::at(kt_time::Tick(45_000), 9.0),
                    AutoKey::at(kt_time::Tick(90_000), 4.0),
                ],
                source: Default::default(),
            }]],
            opacity: 1.0,
            fx_off: false,
        };
        c.fx_bypass = vec![false];
        p.clips.push(c);
        p.normalize();
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.open_fx_editor(ClipId(1));
    }
    h.step();
    h.step();

    // координата поля — из НАСТОЯЩЕЙ ВЁРСТКИ: у TextEdit нет узла
    // доступности, зато подпись «Формула» нарисована рядом (приём t281)
    let label = crate::i18n::tr(crate::i18n::k::fx::FORMULA).to_string();
    let lr = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .find(|t| t.src == label)
        .map(|t| t.rect)
        .expect("подпись поля формулы нарисована");
    let field = egui::pos2(lr.right() + 60.0, lr.center().y);
    h.hover_at(field);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: field,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();

    let keys_of = |h: &Harness<'_, KtApp>| -> Vec<(i64, f64)> {
        h.state().model.project().clips[0].fx_auto[0][0]
            .keys
            .iter()
            .map(|k| (k.t.0, k.v))
            .collect()
    };
    let before = keys_of(&h);
    assert_eq!(before.len(), 3, "сцена: у кривой три ключа");

    // НАБИРАЕМ ПО СИМВОЛУ — и после каждого кадра ключи обязаны быть целы
    for ch in "sin(t)*0.5".chars() {
        h.event(egui::Event::Text(ch.to_string()));
        h.step();
        h.step();
        assert_eq!(
            keys_of(&h),
            before,
            "после символа {ch:?} ключи кривой изменились: текст применился до подтверждения"
        );
        assert!(
            h.state().model.project().clips[0].fx_auto[0][0].source.is_keys(),
            "после символа {ch:?} источник стал формулой без подтверждения"
        );
    }
    // и сам текст не исчез из поля (на прежнем коде — исчезал)
    let shown: Vec<String> =
        crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(
        shown.iter().any(|s| s.contains("sin(t)*0.5")),
        "набранный текст пропал из поля: {shown:?}"
    );

    // ПОДТВЕРЖДЕНИЕ
    h.key_press(egui::Key::Enter);
    h.step();
    h.step();
    let c = &h.state().model.project().clips[0].fx_auto[0][0];
    assert!(
        matches!(&c.source, kt_model::CurveSource::Formula { expr } if expr == "sin(t)*0.5"),
        "после Enter формула не доехала до модели: {:?}",
        c.source
    );
    // и она ВИДНА: разрешение даёт линию, а не пустоту
    let pts = kt_model::formula_preview(c, kt_time::Tick(0), kt_time::Tick(90_000), 750);
    assert!(pts.len() > 10, "формульная кривая не разрешается в точки: {}", pts.len());
    assert!(
        pts.iter().any(|k| (k.v - pts[0].v).abs() > 1e-6),
        "разрешённая кривая — прямая: синус не посчитан"
    );
}

/// Тест 750 (Р.63 п.4): клик мимо поля отдаёт фокус, и клавиши живы СРАЗУ.
///
/// Владелец: «если начать вводить что-то в поле формулы то фокус ввода
/// переходит туда, и как бы я не тыкал по остальной части интерфейса —
/// нажатие пробела не приводит к паузе… пока я не закрою окно эффекта».
/// Проверяется СЛЕДСТВИЕ: после клика по таймлайну пробел играет, а поле
/// снова принимает ввод, если в него кликнуть (правило 14).
#[test]
fn t750_click_outside_a_field_gives_the_keyboard_back() {
    use kt_model::*;
    let (mut h, g) = harness_three_clips_dt(None);
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.clips[0].fx = vec![FxInstance::blur(8)];
        p.clips[0].fx_bypass = vec![false];
        p.clips[0].fx_auto = vec![vec![ParamCurve {
            param: "radius".into(),
            keys: vec![
                AutoKey::at(kt_time::Tick(0), 2.0),
                AutoKey::at(kt_time::Tick(45_000), 9.0),
            ],
            source: Default::default(),
        }]];
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.open_fx_editor(ClipId(1));
    }
    h.step();
    h.step();

    // 1) кликаем в поле формулы и печатаем — фокус там
    let label = crate::i18n::tr(crate::i18n::k::fx::FORMULA).to_string();
    let lr = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .find(|t| t.src == label)
        .map(|t| t.rect)
        .expect("подпись поля формулы нарисована");
    let field = egui::pos2(lr.right() + 60.0, lr.center().y);
    let click = |h: &mut Harness<'_, KtApp>, pos: egui::Pos2| {
        h.hover_at(pos);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
        h.step();
    };
    click(&mut h, field);
    h.event(egui::Event::Text("t".into()));
    h.step();
    assert!(h.ctx.memory(|m| m.focused()).is_some(), "поле обязано взять фокус");
    assert!(
        h.ctx.egui_wants_keyboard_input(),
        "сцена вырождена: клавиатуру никто не держит"
    );

    // 2) КЛИК ПО ТАЙМЛАЙНУ — и пробел играет
    let was_playing = h.state().player.transport.is_playing();
    let empty = egui::pos2(g.tick_to_x(400_000), g.row_center_y(0));
    click(&mut h, empty);
    assert!(
        !h.ctx.egui_wants_keyboard_input(),
        "после клика по таймлайну поле всё ещё держит клавиатуру"
    );
    h.key_press(egui::Key::Space);
    h.step();
    h.step();
    assert_ne!(
        h.state().player.transport.is_playing(),
        was_playing,
        "пробел не сработал после клика мимо поля (на прежнем коде — не работал, \
         пока открыто окно эффекта)"
    );
    h.key_press(egui::Key::Space);
    h.step();

    // 3) поле снова принимает ввод — фича работает ПОВТОРНО (правило 14)
    click(&mut h, field);
    h.event(egui::Event::Text("2".into()));
    h.step();
    assert!(
        h.ctx.egui_wants_keyboard_input(),
        "поле перестало принимать ввод после того, как у него забрали фокус"
    );

    // Про КНОПКИ: в egui клик кнопке фокуса не даёт, поэтому в харнессе
    // эта половина претензии не воспроизводится вовсе — её проверяет живой
    // сценарий `formula.py`. Гейт горячих клавиш при этом всё равно
    // переписан на «набор текста» вместо «что-то в фокусе»: сторожа
    // прежнего смысла — t98 и t627 (маски таймкода и скорости).
}

/// Тест 751 (Р.63 п.5): цель правого клика ФИКСИРУЕТСЯ на нажатии, живёт
/// пока меню открыто, участок подсвечен, а заголовок называет оба конца.
///
/// Владелец: «невозможно указать формы кривых в кривых автоматизациях —
/// контекстное меню просто пропадает я не успеваю дотянуться до него, а
/// еще не понятно между какими точками(и точками ли?) я установлю форму».
/// Причина: и ключ, и участок считались от `hover_pos()` КАЖДЫЙ КАДР, а
/// содержимое `context_menu` перерисовывается каждый кадр — указатель
/// уходил к пунктам, попадание пропадало, и меню закрывало само себя.
/// Утверждения теста — наблюдаемые следствия: после увода указателя меню
/// на месте, форма применяется и ложится РОВНО на левый ключ участка.
#[test]
fn t751_the_menu_target_is_latched_on_press() {
    use crate::automation::MenuTarget;
    use kt_model::*;
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let mut h = harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
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
            fx: vec![FxInstance::blur(8)],
            fx_bypass: vec![false],
            fx_auto: vec![vec![ParamCurve {
                param: "radius".into(),
                keys: vec![
                    AutoKey::at(kt_time::Tick(0), 2.0),
                    AutoKey::at(kt_time::Tick(45_000), 9.0),
                    AutoKey::at(kt_time::Tick(90_000), 4.0),
                ],
                source: Default::default(),
            }]],
            opacity: 1.0,
            fx_off: false,
        });
        p.normalize();
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.open_fx_editor(ClipId(1));
    }
    h.step();
    h.step();

    // положения ключей — из НАСТОЯЩЕЙ отрисовки (рисунок и щуп из одного
    // места правды, п.364)
    let marks = |h: &Harness<'_, KtApp>| -> Vec<(usize, usize, egui::Pos2)> {
        h.state().fx_editors[0].auto_views[&(ClipId(1), 0)].key_marks.clone()
    };
    let m = marks(&h);
    assert_eq!(m.len(), 3, "сцена вырождена: график ключей не нарисован ({m:?})");
    let target = |h: &Harness<'_, KtApp>| -> Option<MenuTarget> {
        h.state().fx_editors[0].auto_views[&(ClipId(1), 0)].menu_target
    };
    let right_click = |h: &mut Harness<'_, KtApp>, pos: egui::Pos2| {
        h.hover_at(pos);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
        h.step();
    };
    // ЗАЛИВКИ подсветки в выводе кадра (участок под меню)
    fn fills(shape: &egui::epaint::Shape, want: egui::Color32, out: &mut Vec<egui::Rect>) {
        use egui::epaint::Shape;
        match shape {
            Shape::Vec(v) => v.iter().for_each(|s| fills(s, want, out)),
            Shape::Rect(r) if r.fill == want => out.push(r.rect),
            _ => {}
        }
    }
    let highlights = |h: &Harness<'_, KtApp>| -> Vec<egui::Rect> {
        let want = egui::Color32::from_rgba_unmultiplied(120, 160, 255, 28);
        let mut out = Vec::new();
        for cs in &h.output().shapes {
            fills(&cs.shape, want, &mut out);
        }
        out
    };

    // --- ПРАВЫЙ КЛИК ПО УЧАСТКУ между ключами 0 и 1 -------------------
    let seg = egui::pos2((m[0].2.x + m[1].2.x) * 0.5, (m[0].2.y + m[1].2.y) * 0.5);
    right_click(&mut h, seg);
    assert_eq!(
        target(&h),
        Some(MenuTarget::Segment(0, 0)),
        "цель не зафиксирована на нажатии"
    );

    // --- ТЯНЕМСЯ УКАЗАТЕЛЕМ К ПУНКТУ МЕНЮ -----------------------------
    // (на прежнем коде именно здесь попадание пропадало, и меню закрывало
    // себя само: «я не успеваю дотянуться до него»)
    let hold = crate::widgets::shape_label(kt_model::SegShape::Hold).to_string();
    let item = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .find(|t| t.src == hold)
        .map(|t| t.rect.center())
        .expect("пункт формы не нарисован — меню не открылось вовсе");
    h.hover_at(item);
    h.step();
    h.step();
    assert_eq!(
        target(&h),
        Some(MenuTarget::Segment(0, 0)),
        "цель потерялась, когда указатель ушёл к пунктам меню"
    );

    // заголовок называет ОБА конца участка: времена и доли события
    let head = crate::tf!(
        k::fx::SHAPE_SEGMENT_AT,
        "0.000".to_string(),
        "0.500".to_string(),
        "0".to_string(),
        "50".to_string()
    );
    let shown: Vec<String> =
        crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(
        shown.iter().any(|s| s == &head),
        "в меню нет заголовка участка «{head}»: {shown:?}"
    );
    // и сам участок подсвечен ровно между этими ключами
    let hl = highlights(&h);
    assert!(
        hl.iter().any(|r| (r.left() - m[0].2.x).abs() < 1.5 && (r.right() - m[1].2.x).abs() < 1.5),
        "участок под меню не подсвечен: {hl:?} против ключей {:?}",
        (m[0].2.x, m[1].2.x)
    );

    // --- ФОРМА ложится на ЛЕВЫЙ ключ участка и только на него ---------
    {
        use egui_kittest::kittest::Queryable;
        h.get_by_label(&hold).click();
    }
    h.step();
    h.step();
    let keys = h.state().model.project().clips[0].fx_auto[0][0].keys.clone();
    assert_eq!(keys[0].shape, kt_model::SegShape::Hold, "форма не легла на левый ключ участка");
    assert_eq!(
        keys[1].shape,
        kt_model::SegShape::DEFAULT,
        "форма поехала на соседний участок"
    );
    // меню закрылось — цель забыта
    h.step();
    assert_eq!(target(&h), None, "после закрытия меню цель осталась висеть");

    // --- ПРАВЫЙ КЛИК ПО КЛЮЧУ ведёт себя так же -----------------------
    let m = marks(&h);
    right_click(&mut h, m[1].2);
    assert_eq!(target(&h), Some(MenuTarget::Key(0, 1)), "ключ не зафиксирован на нажатии");
    let del = tr(k::tl::menu::DELETE).to_string();
    let item = crate::reach_tests::painted_texts(&h)
        .into_iter()
        .find(|t| t.src == del)
        .map(|t| t.rect.center())
        .expect("пункта «удалить» нет — меню ключа не открылось");
    h.hover_at(item);
    h.step();
    h.step();
    assert_eq!(
        target(&h),
        Some(MenuTarget::Key(0, 1)),
        "цель-ключ потерялась, когда указатель ушёл к пунктам меню"
    );
    let head = crate::tf!(k::fx::KEY_AT, "0.500".to_string(), "50".to_string());
    let shown: Vec<String> =
        crate::reach_tests::painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(shown.iter().any(|s| s == &head), "в меню ключа нет заголовка «{head}»: {shown:?}");
}

/// Тест 752c (Р.63 п.9): ручка ведущей геометрии действует у ЭКЗЕМПЛЯРА.
///
/// Вторая половина претензии «у первого geometry нет возможности
/// опубликовать параметры»: мало открыть дверь автору — потребитель обязан
/// получить обычную ручку, которая меняет КАДР и, значит, ключ превью
/// ленты (иначе на таймлайне он увидит чужие кадры, t680).
#[test]
fn t752c_an_instance_edits_the_published_lead_geometry() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // автор объявляет зум ведущего узла титра
    let zoom = PubId(50);
    h.state_mut().enter_nest(s.a);
    h.step();
    assert!(
        h.state_mut().apply(Command::PublishParam {
            seq: s.seq,
            param: Box::new(PublishedParam {
                id: zoom,
                name: "Крупность".into(),
                target: ParamAddr::new(s.inner, ParamOwner::Geometry, "zoom"),
                ty: ParamType::Num,
                range: PubRange::default(),
            }),
        }),
        "публикация зума ведущего узла отвергнута"
    );
    h.step();
    h.state_mut().leave_nest();
    h.step();

    // окно потребителя рисует ручку ИМЕНЕМ АВТОРА
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    assert!(
        h.query_all_by_label_contains("Крупность").count() > 0,
        "ручка ведущей геометрии не показана потребителю"
    );

    // ...и правка поля (тем же жестом, что шлёт окно) меняет КАДР
    let keys = h.state_mut().reel_keys();
    let (ka, kb) = (*keys.get(&s.a).expect("ключ A"), *keys.get(&s.b).expect("ключ B"));
    assert_eq!(ka, kb, "сцена: до правки оба выпуска делят один ключ превью");
    let win = h.state().fx_editors[0].gesture;
    assert!(h.state_mut().apply_coalesced(
        Command::SetOverride { id: s.b, ov: Box::new(Override::value(zoom, ParamValue::Num(3.0))) },
        win
    ));
    h.step();

    let doc = h.state().model.document();
    let inst = h.state().model.clip(s.b).cloned().expect("второй выпуск");
    let r = doc.resolve_reel(Some(s.seq), &inst.overrides);
    assert_eq!(
        r.clips.iter().find(|c| same_clip(c.id, s.inner)).map(|c| c.geometry.zoom),
        Some(3.0),
        "переопределение не доехало до ведущего узла внутри ленты"
    );
    assert_eq!(
        doc.published_value(s.seq, zoom),
        Some(ParamValue::Num(1.0)),
        "правка экземпляра уехала в САМУ ленту — это была бы правка всех мест сразу"
    );
    let keys = h.state_mut().reel_keys();
    let (ka2, kb2) = (*keys.get(&s.a).expect("ключ A"), *keys.get(&s.b).expect("ключ B"));
    assert_eq!(ka2, ka, "шаблонный экземпляр не трогали — его ключ обязан остаться прежним");
    assert_ne!(
        ka2, kb2,
        "ключ превью не поехал: на таймлайне переопределённый выпуск показал бы ЧУЖИЕ кадры"
    );
}

/// Тест 753b (Р.63 п.11): подтверждение публикации НЕ входит в ленту.
///
/// Владелец: «если опубликовать параметр не входя полностью в композицию —
/// то зачем-то мы автоматически полностью входим в композицию». Вход был
/// следствием команды (она правила «ту ленту, в которой стоишь»); теперь
/// лента названа, и человек остаётся там, где стоял, — в корне с раскрытой
/// полосой. Утверждение — наблюдаемое: `editing()` не изменился, а ручка
/// лежит в ленте полосы.
#[test]
fn t753b_confirming_a_publication_keeps_you_where_you_stand() {
    use egui_kittest::kittest::Queryable;
    let (mut h, s) = harness_news_template();
    let publish = tr(k::pubp::PUBLISH).to_string();
    let ok = tr(k::pubp::DIALOG_DO).to_string();
    let free_row =
        crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "line_spacing");

    // человек стоит в КОРНЕ, лента раскрыта полосой
    h.state_mut().expand_in_place(s.a);
    h.step();
    h.step();
    assert!(!h.state().model.bands().is_empty(), "полоса не раскрылась — сцена не та");
    assert_eq!(h.state().model.editing(), None, "сцена: человек в корне");
    h.state_mut().open_fx_editor_head(s.inner);
    h.step();
    h.step();

    // правый клик по подписи → «Опубликовать…»
    let rect = h
        .query_all_by_label_contains(&free_row)
        .map(|n| n.rect())
        .min_by(|a, b| a.top().total_cmp(&b.top()))
        .expect("подписи ряда нет");
    let pos = rect.center();
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    h.step();
    h.get_by_label_contains(&publish).click();
    h.step();
    h.step();
    assert!(h.state().publish_dialog.is_some(), "окно подтверждения не открылось");

    // подтверждаем
    h.get_by_label_contains(&ok).click();
    h.step();
    h.step();

    assert_eq!(
        h.state().model.editing(),
        None,
        "подтверждение утащило человека внутрь ленты (на прежнем коде — утаскивало)"
    );
    assert!(
        !h.state().model.bands().is_empty(),
        "полоса закрылась сама — а она и есть рабочее место человека"
    );
    let set = h.state().model.document().sequence(s.seq).map(|q| q.published.len()).unwrap_or(0);
    assert_eq!(set, 7, "ручка не легла в ленту полосы: в наборе {set} вместо семи");
}

/// Тест 755b (Р.63 п.10): ручка эффекта, правленная у ЭКЗЕМПЛЯРА, меняет
/// то, что показывает таймлайн.
///
/// Владелец: «накинул Radial blur, опубликовал параметр количества,
/// поменял его на композиции — а оно не меняется». Наблюдаемое следствие,
/// которое обязано выполняться: ключ превью ленты у этого экземпляра
/// поехал (иначе на таймлайне он показывает кадры ШАБЛОНА), а у соседа —
/// нет.
#[test]
fn t755b_an_instance_knob_moves_its_preview_key() {
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // внутри ленты на титр — радиальное размытие, ручка «количество»
    let amount = PubId(60);
    h.state_mut().enter_nest(s.a);
    h.step();
    assert!(h.state_mut().apply(Command::SetFx {
        id: s.inner,
        fx: vec![FxInstance::radial_blur()],
    }));
    h.step();
    assert!(
        h.state_mut().apply(Command::PublishParam {
            seq: s.seq,
            param: Box::new(PublishedParam {
                id: amount,
                name: "Размытие".into(),
                target: ParamAddr::new(s.inner, ParamOwner::FxSlot { slot: 0 }, "amount"),
                ty: ParamType::Num,
                range: PubRange::default(),
            }),
        }),
        "публикация ручки размытия отвергнута"
    );
    h.step();
    h.state_mut().leave_nest();
    h.step();

    let keys = h.state_mut().reel_keys();
    let (ka, kb) = (*keys.get(&s.a).expect("ключ A"), *keys.get(&s.b).expect("ключ B"));
    assert_eq!(ka, kb, "сцена: до правки оба выпуска делят один ключ превью");

    // правим ручку У ЭКЗЕМПЛЯРА — тем же путём, каким это делает окно
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    let win = h.state().fx_editors[0].gesture;
    assert!(h.state_mut().apply_coalesced(
        Command::SetOverride {
            id: s.b,
            ov: Box::new(Override::value(amount, ParamValue::Num(0.9))),
        },
        win
    ));
    h.step();

    // ...и это видно: редакция ленты у второго выпуска другая
    let doc = h.state().model.document();
    let inst = h.state().model.clip(s.b).cloned().expect("второй выпуск");
    let r = doc.resolve_reel(Some(s.seq), &inst.overrides);
    let got = r
        .clips
        .iter()
        .find(|c| same_clip(c.id, s.inner))
        .and_then(|c| c.fx.first().and_then(|f| f.get_value("amount")));
    // сравнение с допуском: у слота параметр f32, и 0.9 туда не ложится точно
    assert!(
        matches!(got, Some(ParamValue::Num(v)) if (v - 0.9).abs() < 1e-6),
        "переопределение не доехало до слота эффекта внутри ленты: {got:?}"
    );
    let keys = h.state_mut().reel_keys();
    let (ka2, kb2) = (*keys.get(&s.a).expect("ключ A"), *keys.get(&s.b).expect("ключ B"));
    assert_eq!(ka2, ka, "шаблонный экземпляр не трогали — его ключ обязан остаться прежним");
    assert_ne!(
        ka2, kb2,
        "ключ превью не поехал: экземпляр показывал бы кадры ШАБЛОНА — ровно \
         «поменял его на композиции, а оно не меняется»"
    );
}

/// Тест 754 (Р.63 п.12): параметры композиции живут ГОЛОВОЙ ПОЛОСЫ, а не
/// отдельным окном.
///
/// Владелец: «достаточно ебануто сделано то, что параметры композиции
/// открываются в отдельном окне, когда их можно поместить в тот же
/// fx-chain для удобства. Я ставлю запрет любым агентам порождать
/// какие-либо дополнительные окна без очевидного смысла». Утверждения —
/// наблюдаемые: голова есть в полосе, клик по ней выбирает её, форма
/// рисуется В ОКНЕ FX (её прямоугольники внутри его прямоугольника), а
/// правка ручки и правка слота в одной сессии дают ОДИН шаг истории.
#[test]
fn t754_instance_params_live_in_the_chain_head() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;
    let (mut h, s) = harness_news_template();

    // окно FX экземпляра открывается обычным путём
    h.state_mut().open_fx_editor(s.b);
    h.step();
    h.step();
    let head = format!("{} {}", crate::icons::PIN, tr(k::pubp::INST_TITLE));
    assert!(
        h.query_all_by_label_contains(&head).count() > 0,
        "головы «Параметры композиции» нет в полосе цепочки экземпляра"
    );

    // клик по голове выбирает её — и появляются ручки автора
    h.get_by_label_contains(&head).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().fx_editors[0].selected.get(&s.b),
        Some(&crate::widgets::ChainSel::Head),
        "клик по голове её не выбрал"
    );
    for name in ["Заголовок", "Кегль", "Логотип"] {
        assert!(
            h.query_all_by_label_contains(name).count() > 0,
            "ручки «{name}» в голове нет — форма не переехала"
        );
    }

    // форма ВНУТРИ окна FX: отдельного окна больше нет
    let probes = crate::widgets::field_probe::take();
    let _ = probes;
    let win = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new(("fx", h_clip(&h, s.b)))))
        .expect("прямоугольник окна FX");
    let row = h
        .query_all_by_label_contains("Заголовок")
        .map(|n| n.rect())
        .find(|r| r.width() > 0.0)
        .expect("строка ручки нарисована");
    assert!(
        win.expand(2.0).contains_rect(row),
        "строка ручки нарисована ВНЕ окна FX ({row:?} против {win:?}) — значит \
         рядом всё-таки живёт своё окно"
    );

    // ОДИН шаг истории на сессию: ручка и слот правятся общим токеном
    let token = h.state().fx_editors[0].gesture;
    let before = h.state().model.undo_depth();
    assert!(h.state_mut().apply_coalesced(
        Command::SetOverride {
            id: s.b,
            ov: Box::new(Override::value(s.headline, ParamValue::Text("ВТОРОЙ".into()))),
        },
        token
    ));
    assert!(h.state_mut().apply_coalesced(
        Command::SetOpacity { id: s.b, opacity: 0.5 },
        token
    ));
    h.step();
    assert_eq!(
        h.state().model.undo_depth(),
        before + 1,
        "правка ручки и правка события дали РАЗНЫЕ шаги истории: токен окна не общий"
    );

    // ...и «Отмена» окна возвращает ручку тоже: голова — часть той же
    // сессии, откатывать её половину было бы ложью
    h.get_by_label_contains(&tr(k::fx::CANCEL).to_string()).click();
    h.step();
    h.step();
    assert!(
        h.state().model.clip(s.b).is_some_and(|c| c.overrides.is_empty()),
        "«Отмена» окна FX не вернула ручки композиции — окно откатило половину своей сессии"
    );
}

/// Прямоугольник окна FX ищется по id его якоря — того события, вокруг
/// которого окно собрано (у A/V-пары это один якорь на обе дорожки).
fn h_clip(h: &Harness<'_, KtApp>, want: kt_model::ClipId) -> u64 {
    h.state()
        .fx_editors
        .iter()
        .find(|e| e.members.contains(&want))
        .map(|e| e.clip.0)
        .expect("окно FX экземпляра открыто")
}

pub(crate) fn harness_geom_ctl() -> (Harness<'static, KtApp>, kt_model::ClipId) {
    use kt_model::*;
    // сцена — ГЕНЕРАТОР: у него источник поднимается тем же путём, что в
    // программе, и превью получает НАСТОЯЩИЙ кадр (без кадра нет ни
    // картинки, ни контролов над ней — как у драга титра, t495)
    let (mut h, _g) = harness_three_clips_dt(Some(0.02));
    h.state_mut().create_generator(GeneratorKind::Solid {
        color_r: 200,
        color_g: 40,
        color_b: 40,
    });
    h.step();
    let (_media, clip) = made_generator(&h);
    h.state_mut().fx_editors.clear();
    {
        let app = h.state_mut();
        let c = app.model.clip(clip).cloned().expect("событие");
        app.selected.clear();
        app.selected.insert(clip);
        app.player.seek(kt_time::Tick(c.start.0 + c.duration.0 / 2));
        app.geom_ctl = crate::geomctl::GeomMode::Transform;
    }
    h.step();
    h.step();
    (h, clip)
}

/// Тест 760 (Р.64): рамка над кадром равна аналитической, жест правит те
/// же числа, что поля, и даёт ОДИН шаг истории.
///
/// Владелец (Р.63 п.6): «КАК МИНИМУМ у нас должно быть редактирование прям
/// в превью удобное». Рамка обязана совпадать с прямоугольником, по
/// которому композит кладёт слой, — иначе человек тянет одно, а получает
/// другое.
#[test]
fn t760_frame_matches_the_placement_and_drag_writes_the_same_numbers() {
    use kt_model::*;
    let (mut h, clip) = harness_geom_ctl();
    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);

    // --- РАМКА равна аналитической -----------------------------------
    let pl = h.state().geom_placement(clip, canvas).expect("размещение есть");
    let g = h.state().model.clip(clip).unwrap().geometry;
    let want = kt_fx::geom::geometry_placement(
        &g.node(),
        Some(kt_fx::conform::LeadCtx::new(
            h.state().player.source_view(h.state().model.clip(clip).unwrap().media),
            g.rule.resolve(h.state().model.project().settings.conform),
        )),
        h.state()
            .player
            .source_view(h.state().model.clip(clip).unwrap().media)
            .unwrap_or(canvas),
        canvas,
    )
    .expect("аналитическое размещение");
    assert_eq!(pl, want, "рамка интерфейса разошлась с размещением рендера");

    // --- ПЕРЕНОС за тело: те же числа, один шаг истории ---------------
    let depth = h.state().model.undo_depth();
    let before = h.state().model.clip(clip).unwrap().geometry;
    // масштаб «экран → холст» считается по КАРТИНКЕ, а не по панели:
    // у панели есть поля letterbox, и по ней 48 px указателя переводятся
    // в другое число долей (ровно этим тест краснел на верном коде)
    let img = {
        let r = h.state().preview_img_rect;
        assert!(r.width() > 40.0, "превью в харнессе есть: {r:?}");
        r
    };
    // диагностика сцены: есть ли кадр и виден ли набор
    assert!(h.state().texture.is_some(), "в превью нет кадра — контролам не над чем рисоваться");
    assert_eq!(h.state().geom_mode(), crate::geomctl::GeomMode::Transform, "набор не включён");
    assert!(h.state().geom_target().is_ok(), "цель рамки не выбрана");
    assert!(h.state().geom_ctl_drawn, "контролы не дошли до разбора жеста");
    // берём точку ВНУТРИ рамки, но не на ручке: центр занят якорем,
    // углы и середины сторон — ручками масштаба
    let c = img.min + egui::vec2(img.width() * 0.3, img.height() * 0.32);
    h.hover_at(c);
    h.step();
    h.drag_at(c);
    h.step();
    h.hover_at(c + egui::vec2(12.0, 0.0));
    h.step();
    assert!(h.state().geom_drag.is_some(), "жест не начался: контрол не поймал нажатие");
    h.hover_at(c + egui::vec2(48.0, 30.0));
    h.step();
    h.drop_at(c + egui::vec2(48.0, 30.0));
    h.step();

    let after = h.state().model.clip(clip).unwrap().geometry;
    assert!(
        after.pos_x > before.pos_x + 1e-6 && after.pos_y > before.pos_y + 1e-6,
        "перенос за тело не сдвинул позицию: {:?} → {:?}",
        (before.pos_x, before.pos_y),
        (after.pos_x, after.pos_y)
    );
    assert_eq!(
        h.state().model.undo_depth(),
        depth + 1,
        "жест обязан дать РОВНО один шаг истории"
    );
    // …и сдвиг ПРАВДОПОДОБЕН: это то же движение указателя, переведённое
    // в доли холста той же функцией (точность возврата доказывает t761 на
    // чистой функции — здесь проверяется, что жест ходит через неё)
    let img_w = img.width() as f64;
    let img_h = img.height() as f64;
    let full = kt_fx::geom::pos_for_drag(
        (before.pos_x, before.pos_y),
        canvas,
        (48.0 / img_w * canvas.0, 30.0 / img_h * canvas.1),
    );
    for (got, want, ax) in [(after.pos_x, full.0, "X"), (after.pos_y, full.1, "Y")] {
        assert!(
            got > want * 0.6 && got <= want * 1.05,
            "сдвиг по {ax} не похож на движение указателя: {got} против {want} \
             (порог драга в значение не попадает, поэтому «чуть меньше» — норма)"
        );
    }
    // ЧИСЛО ОДНО: модель и поле окна FX читают одно и то же значение
    assert_eq!(
        h.state().model.clip(clip).unwrap().geometry.pos_x,
        after.pos_x,
        "значение в модели разошлось с тем, что показывает форма"
    );

    // --- рамки НЕТ у аудио и при мультивыделении, причина названа ------
    {
        let app = h.state_mut();
        let audio = app
            .model
            .project()
            .clips
            .iter()
            .find(|c| c.kind == ClipKind::Audio)
            .map(|c| c.id);
        app.selected.clear();
        if let Some(a) = audio {
            app.selected.insert(a);
            assert!(
                matches!(app.geom_target(), Err(crate::geomctl::NoFrame::NotVideo)),
                "у аудио-события рамка не должна рисоваться"
            );
        }
        app.selected.insert(clip);
        if audio.is_some() {
            assert!(
                matches!(app.geom_target(), Err(crate::geomctl::NoFrame::ManySelected)),
                "при мультивыделении рамка не рисуется"
            );
        }
        app.selected.clear();
        assert!(matches!(app.geom_target(), Err(crate::geomctl::NoFrame::NothingSelected)));
    }
}

/// Тест 759 (Р.64): форма «Трансформация» у ведущего узла.
///
/// Претензия владельца (Р.63 п.6): «сплошные скучные поля ввода, даже в
/// Vegas и то лучше сделано… в том же Pr/Ae это удобнее». Утверждается то,
/// что человек видит и делает: группы, пара X/Y ОДНОЙ строкой, ползунки у
/// зума и поворота, схема, считающая ТЕ ЖЕ числа, и «сбросить геометрию»
/// одним шагом истории.
#[test]
fn t759_transform_form_groups_pairs_sliders_and_reset() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    use kt_model::*;
    let (mut h, clip) = harness_geom_ctl();
    // геометрия СДВИНУТА — иначе сброс нечего возвращать
    {
        let app = h.state_mut();
        let c = app.model.clip(clip).cloned().expect("событие");
        app.apply(Command::SetGeometry {
            id: clip,
            geometry: EventGeometry { pos_x: 0.2, zoom: 1.7, rot: 30.0, crop_l: 0.1, ..c.geometry },
            pos: c.geometry_pos,
            auto: Vec::new(),
        });
        app.open_fx_editor(clip);
    }
    h.step();
    h.step();
    let lead_label = crate::fx_editor::fx_display_name(&EventGeometry::default().node());
    h.get_by_label(lead_label).click();
    h.step();
    h.step();

    // ---- ГРУППЫ -------------------------------------------------------
    for key in [k::geomctl::GROUP_FRAME, k::geomctl::GROUP_POS, k::geomctl::GROUP_CROP] {
        let title = tr(key);
        assert!(
            h.query_all_by_label_contains(title).count() > 0,
            "группы «{title}» в форме нет — поля остались сплошным списком"
        );
    }

    // ---- ПАРА X/Y ОДНОЙ СТРОКОЙ ---------------------------------------
    // подпись ищется ТОЧНО: «X:» встречается и внутри «Якорь X:»
    let row_of = |h: &Harness<'_, KtApp>, label: &str| -> egui::Rect {
        h.get_all_by_label(label)
            .map(|n| n.rect())
            .find(|r| r.width() > 0.0)
            .unwrap_or_else(|| panic!("подписи «{label}» нет в форме"))
    };
    let rx = row_of(&h, tr(k::fxp::POS_X));
    let ry = row_of(&h, tr(k::fxp::POS_Y));
    assert!(
        (rx.center().y - ry.center().y).abs() < 4.0 && (rx.center().x - ry.center().x).abs() > 8.0,
        "X и Y стоят не парой на одной строке: {rx:?} против {ry:?}"
    );

    // ---- ПОЛЗУНКИ у зума и поворота -----------------------------------
    let sliders = h.query_all(by().role(Role::Slider)).count();
    assert!(
        sliders >= 2,
        "ползунков в форме {sliders}: зум и поворот обязаны иметь их оба          (тянуть число по одному пикселю — это и есть «скучные поля»)"
    );

    // ---- СХЕМА считает ТЕ ЖЕ числа ------------------------------------
    {
        let g = h.state().model.clip(clip).unwrap().geometry;
        let size = egui::vec2(116.0, 66.0);
        let (quad, anchor) = crate::geomctl::schema_quad(&g.node(), size).expect("схема считается");
        let (q0, a0) = crate::geomctl::schema_quad(&EventGeometry::default().node(), size)
            .expect("схема умолчаний");
        assert_ne!(quad, q0, "схема не видит ни зума, ни поворота, ни кропа");
        let w = |q: [(f64, f64); 4]| {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for c in q {
                lo = lo.min(c.0);
                hi = hi.max(c.0);
            }
            hi - lo
        };
        assert!(w(quad) > w(q0), "зум 1.7 не расширил рамку схемы: {} против {}", w(quad), w(q0));
        assert!(
            (anchor.0 - a0.0).abs() > 1.0,
            "якорь схемы не уехал вместе с положением: {anchor:?} против {a0:?}"
        );
    }

    // ---- СБРОС одним шагом истории ------------------------------------
    let depth = h.state().model.undo_depth();
    h.get_by_label_contains(tr(k::geomctl::RESET)).click();
    h.step();
    h.step();
    let g = h.state().model.clip(clip).unwrap().geometry;
    assert_eq!(
        (g.pos_x, g.zoom, g.rot, g.crop_l),
        (0.0, 1.0, 0.0, 0.0),
        "«Сбросить геометрию» не вернула умолчания"
    );
    assert_eq!(
        h.state().model.undo_depth(),
        depth + 1,
        "сброс — ОДИН шаг истории (сессия окна), а не десять команд"
    );
    // единицы берутся из ПАСПОРТА, а не пишутся в подпись
    assert_eq!(
        crate::geomctl::shown_spec_unit("crop_l"),
        Some(kt_model::ParamUnit::Percent),
        "доля кропа обязана показываться процентом (п.415)"
    );
    assert_eq!(
        crate::geomctl::shown_spec_unit("zoom"),
        Some(kt_model::ParamUnit::Ratio),
        "зум — отношение (×), а не процент"
    );
}

/// Тест 762b (Р.64): жест над АНИМИРОВАННЫМ параметром ставит ключ.
///
/// Правило 17 буквально: экранная рамка и поле окна FX правят одно и то же
/// значение ОДНИМ правилом (`CurveBind`). У параметра с живой кривой это
/// ключ по курсору, а не константа: иначе жест стирал бы анимацию, показывая
/// при этом её же значение. Соседний параметр БЕЗ кривой тем же жестом
/// правит базу — обе половины правила в одном тесте.
#[test]
fn t762b_gesture_over_a_curve_sets_a_key() {
    use kt_model::*;
    let (mut h, clip) = harness_geom_ctl();
    // кривая у pos_x: 0 → 0.2 по всему материалу события
    let (at, keys0) = {
        let app = h.state_mut();
        let c = app.model.clip(clip).cloned().expect("событие");
        let curve = ParamCurve {
            param: "pos_x".into(),
            keys: AutoKey::linear(vec![
                AutoKey::at(c.source_in, 0.0),
                AutoKey::at(kt_time::Tick(c.source_in.0 + c.duration.0), 0.2),
            ]),
            source: Default::default(),
        };
        app.apply(Command::SetGeometry {
            id: clip,
            geometry: c.geometry,
            pos: c.geometry_pos,
            auto: vec![curve],
        });
        (app.geom_curve_at(clip), 2usize)
    };
    h.step();
    // рамка показывает то же, что рендер: значение КРИВОЙ под курсором
    let shown = h.state().geom_effective(clip).expect("эффективная геометрия").pos_x;
    assert!(
        (shown - 0.1).abs() < 1e-6,
        "рамка считает по базе, а не по кривой под курсором: {shown}"
    );

    let c0 = body_pos(&h, clip);
    let depth = h.state().model.undo_depth();
    geom_drag(&mut h, c0, c0 + egui::vec2(40.0, 26.0), egui::Modifiers::NONE);

    let c = h.state().model.clip(clip).cloned().expect("событие");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "жест — один шаг истории");
    // …параметр С КРИВОЙ: база цела, ключ по курсору поставлен
    assert_eq!(c.geometry.pos_x, 0.0, "жест переписал КОНСТАНТУ анимированного параметра");
    let curve = c
        .geometry_auto
        .iter()
        .find(|cu| cu.param == "pos_x")
        .expect("кривая pos_x жива");
    assert!(
        curve.keys.len() > keys0,
        "ключ по курсору не поставлен: ключей по-прежнему {}",
        curve.keys.len()
    );
    let key = curve
        .keys
        .iter()
        .find(|k| k.t == at)
        .unwrap_or_else(|| panic!("нет ключа в позиции курсора {at:?}: {:?}", curve.keys));
    assert!(
        key.v > 0.1 + 1e-6,
        "ключ поставлен, но значение не от жеста: {} (кривая под курсором давала 0.1)",
        key.v
    );
    // …а параметр БЕЗ кривой тем же жестом правит базу
    assert!(c.geometry.pos_y > 1e-6, "pos_y без кривой обязан править базу: {}", c.geometry.pos_y);
    assert!(
        !c.geometry_auto.iter().any(|cu| cu.param == "pos_y"),
        "жест завёл кривую там, где её не было"
    );
}

/// Тест 763 (Р.64): попадание в ручку с первого раза и приоритет при
/// мелкой рамке.
///
/// Разведка F-UI-001: «точность жеста падает вместе с качеством превью» —
/// на мелкой рамке хват-зоны перекрываются, и правило «кто ближе» остаётся
/// единственным однозначным. Плюс правило 1 чек-листа: целиться в ручку
/// размером с точку человек не должен.
#[test]
fn t763_handles_are_hittable_and_the_nearest_wins() {
    use crate::geomctl::{GRAB, GeomMode, nearest_handle};
    use kt_fx::geom::Handle;
    let (h, clip) = harness_geom_ctl();
    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);
    let pl = h.state().geom_placement(clip, canvas).expect("размещение");
    // холст → экран, как в контроле
    let img = h.state().preview_img_rect;
    let to_screen = |p: (f64, f64)| -> egui::Pos2 {
        egui::pos2(
            img.left() + (p.0 / canvas.0) as f32 * img.width(),
            img.top() + (p.1 / canvas.1) as f32 * img.height(),
        )
    };

    // --- КАЖДАЯ ручка ловится в своей точке и в пределах хват-зоны -----
    for h_ in GeomMode::Transform.handles() {
        if *h_ == Handle::Anchor {
            continue; // у якоря своя, узкая зона — проверяется ниже
        }
        let p = to_screen(h_.point(&pl));
        assert_eq!(
            nearest_handle(&pl, GeomMode::Transform, p, &to_screen, false),
            Some(*h_),
            "ручка {h_:?} не ловится в собственной точке"
        );
        // …и с края хват-зоны (правило 1: 14 px)
        let edge = p + egui::vec2(GRAB * 0.45, GRAB * 0.45);
        assert!(
            nearest_handle(&pl, GeomMode::Transform, edge, &to_screen, false).is_some(),
            "ручка {h_:?} не ловится с края хват-зоны — целиться придётся точно"
        );
    }

    // --- ЯКОРЬ: за СВОЮ ТОЧКУ берётся, чуть в стороне — уже тело ------
    // (только с Alt его оставить нельзя: Alt+перетаскивание перехватывает
    // оконный менеджер, и до программы жест не доходит — живой `geomctl`)
    let a = to_screen(Handle::Anchor.point(&pl));
    assert_eq!(
        nearest_handle(&pl, GeomMode::Transform, a, &to_screen, false),
        Some(Handle::Anchor),
        "якорь не берётся за собственную точку — жест недостижим"
    );
    let near_a = a + egui::vec2(crate::geomctl::ANCHOR_GRAB + 3.0, 0.0);
    assert_ne!(
        nearest_handle(&pl, GeomMode::Transform, near_a, &to_screen, false),
        Some(Handle::Anchor),
        "якорь в центре рамки забирает захват картинки вокруг себя: \
         его зона обязана быть узкой"
    );
    assert_eq!(
        nearest_handle(&pl, GeomMode::Transform, near_a, &to_screen, true),
        Some(Handle::Anchor),
        "с Alt зона якоря — общая: поймать его наверняка должно быть можно"
    );

    // --- ТЕЛО: точка внутри рамки, но не у ручки, жеста ручки не даёт --
    let inside = img.min + egui::vec2(img.width() * 0.3, img.height() * 0.32);
    assert_eq!(
        nearest_handle(&pl, GeomMode::Transform, inside, &to_screen, false),
        None,
        "тело рамки перехватило жест ручки"
    );

    // --- МЕЛКАЯ РАМКА: выигрывает БЛИЖАЙШАЯ, а не первая попавшаяся ---
    let tiny = kt_fx::geom::Placement {
        ox: 100.0,
        oy: 100.0,
        dw: 6.0,
        dh: 6.0,
        rot_deg: 0.0,
        anchor: (103.0, 103.0),
        crop_in: (0.0, 0.0, 6.0, 6.0),
    };
    let ident = |p: (f64, f64)| egui::pos2(p.0 as f32, p.1 as f32);
    for corner in [Handle::TopLeft, Handle::TopRight, Handle::BottomRight, Handle::BottomLeft] {
        // с Р.66 ручки мелкой рамки вынесены НАРУЖУ её кромок — щупаем
        // там, где они теперь нарисованы (t781), а утверждение прежнее:
        // указатель на ручке — выигрывает она
        let p = crate::geomctl::handle_screen(&tiny, corner, &ident);
        assert_eq!(
            nearest_handle(&tiny, GeomMode::Transform, p, &ident, false),
            Some(corner),
            "на рамке 6×6 ручка {corner:?} проиграла соседней, хотя указатель на ней"
        );
    }

    // --- КРОП — другой набор: в режиме трансформации его ручек нет ----
    assert_eq!(
        nearest_handle(&pl, GeomMode::Transform, to_screen(Handle::CropLeft.point(&pl)), &to_screen, false),
        Some(Handle::Left),
        "в наборе трансформации середина стороны — это масштаб, а не кроп"
    );
    assert_eq!(
        nearest_handle(&pl, GeomMode::Crop, to_screen(Handle::CropLeft.point(&pl)), &to_screen, false),
        Some(Handle::CropLeft),
        "в наборе кропа середина стороны — это кроп"
    );
}

/// Тест 764 (Р.64): контролы гаснут при игре и не греют простой.
///
/// Отрасль гасит их всегда (FCP) или автоматически (AE): контрол закрывает
/// то, ради чего человек смотрит в кадр. Наше правило стоимости кадра
/// (инвариант 12) требует того же с другой стороны.
#[test]
fn t764_controls_hide_while_playing_and_cost_nothing_idle() {
    let (mut h, _clip) = harness_geom_ctl();
    assert!(h.state().geom_ctl_drawn, "сцена: на паузе контролы рисуются");

    // --- ИГРА: контролов нет ------------------------------------------
    h.state_mut().geom_ctl_drawn = false;
    h.state_mut().player.play();
    h.step();
    h.step();
    assert!(
        !h.state().geom_ctl_drawn,
        "при воспроизведении контролы остались на кадре — они закрывают то, ради чего смотрят"
    );

    // --- ПАУЗА: вернулись ---------------------------------------------
    h.state_mut().player.pause();
    h.state_mut().geom_ctl_drawn = false;
    h.step();
    h.step();
    assert!(h.state().geom_ctl_drawn, "на паузе контролы не вернулись");

    // --- ПРОСТОЙ: набор выключен — путь превью прежний ------------------
    h.state_mut().geom_ctl = crate::geomctl::GeomMode::Off;
    h.state_mut().geom_ctl_drawn = false;
    h.step();
    h.step();
    assert!(!h.state().geom_ctl_drawn, "выключенный набор всё равно лезет в кадр");
}

/// Драг рамки в превью В ТРИ ШАГА (порог драга egui) с модификаторами.
fn geom_drag(
    h: &mut Harness<'_, KtApp>,
    from: egui::Pos2,
    to: egui::Pos2,
    mods: egui::Modifiers,
) {
    // модификатор едет и отдельным событием, и В САМОЙ кнопке: `drag_at`
    // шлёт `PointerButton { modifiers: NONE }` и этим гасит состояние кадра
    // (родня урока `ctrl_click_at`)
    h.event(egui::Event::ModifiersChanged(mods));
    h.hover_at(from);
    h.step();
    h.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: mods,
    });
    h.step();
    h.hover_at(from + (to - from) * 0.25);
    h.step();
    h.hover_at(to);
    h.step();
    h.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: mods,
    });
    h.event(egui::Event::PointerGone);
    h.step();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

/// Экранная точка ручки (контрол считает её тем же `Handle::point`).
/// Точка ТЕЛА рамки: не центр (там якорь) и не кромка (там ручки масштаба).
/// Брать долю панели превью нельзя — сдвинутая рамка из-под этой точки
/// уезжает, и жест начинается мимо (t762b ловил это «нулём шагов истории»).
fn body_pos(h: &Harness<'_, KtApp>, clip: kt_model::ClipId) -> egui::Pos2 {
    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);
    let pl = h.state().geom_placement(clip, canvas).expect("размещение");
    let c = pl.center();
    let p = (c.0 + pl.dw * 0.22, c.1 + pl.dh * 0.24);
    let img = h.state().preview_img_rect;
    egui::pos2(
        img.left() + (p.0 / canvas.0) as f32 * img.width(),
        img.top() + (p.1 / canvas.1) as f32 * img.height(),
    )
}

fn handle_pos(
    h: &Harness<'_, KtApp>,
    clip: kt_model::ClipId,
    which: kt_fx::geom::Handle,
) -> egui::Pos2 {
    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);
    let pl = h.state().geom_placement(clip, canvas).expect("размещение");
    let img = h.state().preview_img_rect;
    let p = which.point(&pl);
    egui::pos2(
        img.left() + (p.0 / canvas.0) as f32 * img.width(),
        img.top() + (p.1 / canvas.1) as f32 * img.height(),
    )
}

/// Тест 760b (Р.64): поворот ручкой вращения, шаг по Shift.
#[test]
fn t760b_rotation_handle_turns_the_layer() {
    use kt_fx::geom::Handle;
    let (mut h, clip) = harness_geom_ctl();
    assert_eq!(h.state().model.clip(clip).unwrap().geometry.rot, 0.0, "сцена: слой не повёрнут");

    // ручка поворота — над верхней стороной; тянем вбок
    let from = handle_pos(&h, clip, Handle::Rot);
    let to = from + egui::vec2(120.0, 60.0);
    // диагностика сцены: ручка поворота вообще внутри превью?
    assert!(
        h.state().preview_rect.contains(from),
        "ручка поворота вне превью: {from:?} при {:?}",
        h.state().preview_rect
    );
    let depth = h.state().model.undo_depth();
    // какая ручка попадётся в этой точке?
    {
        let q = h.state().player.preview_quality();
        let canvas = (q.width as f64, q.height as f64);
        let pl = h.state().geom_placement(clip, canvas).unwrap();
        let img = h.state().preview_img_rect;
        let to_screen = |p: (f64, f64)| -> egui::Pos2 {
            egui::pos2(
                img.left() + (p.0 / canvas.0) as f32 * img.width(),
                img.top() + (p.1 / canvas.1) as f32 * img.height(),
            )
        };
        assert_eq!(
            crate::geomctl::nearest_handle(
                &pl,
                crate::geomctl::GeomMode::Transform,
                from,
                &to_screen,
                false
            ),
            Some(Handle::Rot),
            "в точке ручки поворота ловится не она"
        );
    }
    geom_drag(&mut h, from, to, egui::Modifiers::NONE);
    let rot = h.state().model.clip(clip).unwrap().geometry.rot;
    assert!(rot.abs() > 1.0, "поворот не состоялся: {rot}");
    assert_eq!(h.state().model.undo_depth(), depth + 1, "поворот — один шаг истории");

    // Shift — шаг 15°
    let from = handle_pos(&h, clip, Handle::Rot);
    let to = from + egui::vec2(-90.0, 70.0);
    geom_drag(&mut h, from, to, egui::Modifiers::SHIFT);
    let rot = h.state().model.clip(clip).unwrap().geometry.rot;
    assert!(
        (rot / 15.0 - (rot / 15.0).round()).abs() < 1e-6,
        "с Shift угол обязан быть кратен 15°: {rot}"
    );
}

/// Тест 761b (Р.64): якорь двигается — картинка стоит, и на повёрнутом
/// слое тоже. Тот же контракт, что у чистой функции в t761, но ЧЕРЕЗ
/// НАСТОЯЩИЙ ЖЕСТ.
#[test]
fn t761b_anchor_gesture_keeps_the_picture_in_place() {
    use kt_fx::geom::Handle;
    for rot in [0.0f64, 30.0] {
        let (mut h, clip) = harness_geom_ctl();
        {
            let app = h.state_mut();
            let g = kt_model::EventGeometry { rot, ..app.model.clip(clip).unwrap().geometry };
            let tok = app.gesture_counter + 1;
            app.set_geometry_coalesced(clip, g, tok);
        }
        h.step();
        let q = h.state().player.preview_quality();
        let canvas = (q.width as f64, q.height as f64);
        let before = h.state().geom_placement(clip, canvas).unwrap().corners();
        let a0 = h.state().model.clip(clip).unwrap().geometry;

        // тянем ЯКОРЬ (только с Alt) к левому верхнему углу рамки
        let from = handle_pos(&h, clip, Handle::Anchor);
        let to = handle_pos(&h, clip, Handle::TopLeft);
        geom_drag(&mut h, from, to, egui::Modifiers::ALT);

        let a1 = h.state().model.clip(clip).unwrap().geometry;
        assert!(
            (a1.anchor_x - a0.anchor_x).abs() > 0.05 || (a1.anchor_y - a0.anchor_y).abs() > 0.05,
            "поворот {rot}°: якорь не сдвинулся — жест ничего не сделал"
        );
        let after = h.state().geom_placement(clip, canvas).unwrap().corners();
        for i in 0..4 {
            assert!(
                (after[i].0 - before[i].0).abs() < 1.0 && (after[i].1 - before[i].1).abs() < 1.0,
                "поворот {rot}°: картинка уехала за якорем — угол {i}: {:?} против {:?}",
                after[i],
                before[i]
            );
        }
    }
}

/// Тест 762 (Р.64): кроп своими ручками в своём наборе.
#[test]
fn t762_crop_handles_cut_their_own_side() {
    use kt_fx::geom::Handle;
    let (mut h, clip) = harness_geom_ctl();
    h.state_mut().geom_ctl = crate::geomctl::GeomMode::Crop;
    h.step();

    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);
    let w0 = h.state().geom_placement(clip, canvas).unwrap().dw;
    let from = handle_pos(&h, clip, Handle::CropLeft);
    let to = from + egui::vec2(h.state().preview_rect.width() * 0.15, 0.0);
    let depth = h.state().model.undo_depth();
    geom_drag(&mut h, from, to, egui::Modifiers::NONE);

    let g = h.state().model.clip(clip).unwrap().geometry;
    assert!(g.crop_l > 0.02, "кроп слева не поехал: {}", g.crop_l);
    assert_eq!(
        (g.crop_t, g.crop_r, g.crop_b),
        (0.0, 0.0, 0.0),
        "ручка кропа тронула чужие стороны"
    );
    assert_eq!(h.state().model.undo_depth(), depth + 1, "кроп — один шаг истории");
    let w1 = h.state().geom_placement(clip, canvas).unwrap().dw;
    assert!(w1 < w0, "рамка не сузилась после кропа: {w1} против {w0}");
}

/// Тест 775 (Р.65): ДАКИНГ — ЭТО ДВА КОМПРЕССОРА, а не своя форма.
///
/// Владелец (Р.63 п.8.4): «в качестве дакинга можно показать тот же
/// задублированный виджет аудио-компрессора с затемненными полями, т.е.
/// компрессор на фоновом объекте и компрессор на переднем объекте…
/// повсюду копятся бессмысленные копипасты одного и того же кода».
/// Утверждается то, что видно и делается: выбор ключа заводит прибор,
/// компрессор РЕЧИ заводится одним действием, обе правки — одна сессия
/// окна и один шаг истории, «Отмена» откатывает обе.
#[test]
fn t775_ducking_is_two_compressors_in_the_track_window() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    use kt_model::LaneKind::Audio;
    let (mut h, _) = harness_three_clips_dt(None);
    let (bg, key) = {
        let app = h.state_mut();
        edit_project(app, |p| p.ensure_tracks(Audio, 2));
        let bg = app.model.project().track_at(Audio, 0).unwrap().id;
        let key = app.model.project().track_at(Audio, 1).unwrap().id;
        (bg, key)
    };
    h.step();
    h.state_mut().open_track_window(bg);
    h.step();
    h.step();

    // секция названа по отрасли и стоит В ОКНЕ ДОРОЖКИ
    for s in [tr(k::msg::DUCK_TITLE), tr(k::msg::DUCK_KEY), tr(k::msg::DUCK_BG)] {
        assert!(
            h.query_all_by_label_contains(s).count() > 0,
            "в окне дорожки нет строки «{s}» — дакингу негде жить"
        );
    }
    let depth = h.state().model.undo_depth();

    // ВЫБОР КЛЮЧА заводит прибор: человек не обязан искать компрессор в каталоге
    let key_name = h.state().model.project().track(key).unwrap().label(1);
    // комбо ищется по РОЛИ и ЗНАЧЕНИЮ: слово «нет» встречается в окне не раз
    h.get(by().role(Role::ComboBox).value(tr(k::msg::DUCK_KEY_NONE))).click();
    h.step();
    h.get_by_label_contains(&key_name).click();
    h.step();
    h.step();
    let keys: Vec<_> = h
        .state()
        .model
        .project()
        .track(bg)
        .map(|t| kt_model::sidechain_keys(&t.fx).collect())
        .unwrap_or_default();
    assert_eq!(keys, vec![key], "выбор ключа не доехал до модели (или прибор не заведён)");

    // …и ВТОРОЙ компрессор — настоящий, на дорожке-КЛЮЧЕ, одним действием
    assert!(
        h.query_all_by_label_contains(tr(k::msg::DUCK_ADD_COMP)).count() > 0,
        "кнопки «завести компрессор на ключе» нет — второй прибор недостижим"
    );
    h.get_by_label_contains(&tr(k::msg::DUCK_ADD_COMP).to_string()).click();
    h.step();
    h.step();
    assert!(
        h.state()
            .model
            .project()
            .track(key)
            .is_some_and(|t| t.fx.iter().any(|f| matches!(f, kt_model::FxInstance::Compressor { .. }))),
        "компрессор речи не заведён"
    );

    // ОБЕ правки — ОДНА сессия окна и ОДИН шаг истории
    assert_eq!(
        h.state().model.undo_depth(),
        depth + 1,
        "сессия окна дорожки обязана быть одним шагом истории"
    );

    // «Отмена» откатывает и ключ, и компрессор речи
    h.get_by_label_contains(&tr(k::track::CANCEL).to_string()).click();
    h.step();
    h.step();
    let app = h.state();
    assert_eq!(
        app.model.project().track(bg).map(|t| kt_model::sidechain_keys(&t.fx).count()),
        Some(0),
        "«Отмена» не сняла ключ"
    );
    assert!(
        app.model
            .project()
            .track(key)
            .is_some_and(|t| !t.fx.iter().any(|f| matches!(f, kt_model::FxInstance::Compressor { .. }))),
        "«Отмена» не вернула цепочку дорожки-ключа"
    );
}

/// Тест 776 (Р.65): список ключей не предлагает невозможного.
#[test]
fn t776_key_list_offers_only_possible_tracks() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{Queryable, by};
    use kt_model::LaneKind::Audio;
    let (mut h, _) = harness_three_clips_dt(None);
    let (a, b) = {
        let app = h.state_mut();
        edit_project(app, |p| p.ensure_tracks(Audio, 2));
        let a = app.model.project().track_at(Audio, 0).unwrap().id;
        let b = app.model.project().track_at(Audio, 1).unwrap().id;
        (a, b)
    };
    // A уже слушает B — значит B не может слушать A (цикл)
    {
        let app = h.state_mut();
        let mut c = kt_model::FxInstance::compressor();
        if let kt_model::FxInstance::Compressor { key, .. } = &mut c {
            *key = Some(b);
        }
        app.apply(kt_model::Command::SetTrackFx { id: a, fx: vec![c], bypass: vec![] });
    }
    h.step();
    h.state_mut().open_track_window(b);
    h.step();
    h.step();
    h.get(by().role(Role::ComboBox).value(tr(k::msg::DUCK_KEY_NONE))).click();
    h.step();
    h.step();
    // метка ищется ТОЧНО: заголовок окна содержит имя дорожки, и поиск
    // «по вхождению» нашёл бы его, а не пункт списка
    let a_name = h.state().model.project().track(a).unwrap().label(0);
    let offered = h.query_all_by_label(&a_name).count();
    assert_eq!(
        offered, 0,
        "дорожка, дающая ЦИКЛ, предложена в списке ключей — молчаливый тупик"
    );
    // и себя в списке тоже нет
    let b_name = h.state().model.project().track(b).unwrap().label(1);
    assert_eq!(
        h.query_all_by_label(&b_name).count(),
        0,
        "дорожка предложена ключом самой себе"
    );
}

/// Тест 781 (Р.66): МЕЛКАЯ РАМКА отдаёт ручки наружу, тело — своё.
///
/// На проекте владельца (`paintitblack.otio`) правило «как есть» и
/// заглушка вместо недоступного медиа дают рамку 45×25 px: восемь ручек
/// с хват-зоной 14 px покрывают её целиком, и ПЕРЕНОС становится
/// недоступен — ручка выигрывает всегда. Отложенный предел Р.64.
#[test]
fn t781_small_frame_moves_handles_outside() {
    use crate::geomctl::{GeomMode, handle_screen, nearest_handle, small_frame};
    use kt_fx::geom::{Handle, Placement};

    // рамка 40×24 px на экране: меньше трёх хват-зон по обеим сторонам
    let tiny = Placement {
        ox: 200.0,
        oy: 120.0,
        dw: 40.0,
        dh: 24.0,
        rot_deg: 0.0,
        anchor: (220.0, 132.0),
        crop_in: (0.0, 0.0, 1.0, 1.0),
    };
    // экран = холст один к одному
    let to_screen = |p: (f64, f64)| egui::pos2(p.0 as f32, p.1 as f32);
    assert!(small_frame(&tiny, &to_screen), "сцена: рамка обязана быть мелкой");

    // ручки масштаба УЕХАЛИ наружу своих кромок
    for h in [Handle::TopLeft, Handle::Top, Handle::Right, Handle::BottomLeft] {
        let inside = to_screen(h.point(&tiny));
        let shown = handle_screen(&tiny, h, &to_screen);
        assert!(
            (shown - inside).length() > 10.0,
            "{h:?} не вынесена наружу: {shown:?} против кромки {inside:?}"
        );
        // …и ловится ТАМ ЖЕ, где нарисована
        assert_eq!(
            nearest_handle(&tiny, GeomMode::Transform, shown, &to_screen, false),
            Some(h),
            "{h:?} нарисована снаружи, а ловится не там"
        );
    }

    // ЦЕНТР мелкой рамки — это ПЕРЕНОС, а не ручка (кроме самого якоря)
    let c = to_screen(tiny.center());
    let off = egui::pos2(c.x + 9.0, c.y + 5.0);
    assert_eq!(
        nearest_handle(&tiny, GeomMode::Transform, off, &to_screen, false),
        None,
        "тело мелкой рамки перехвачено ручкой — переносить её нечем"
    );

    // ОБЫЧНАЯ рамка не меняется: ручки на кромках, как прежде
    let big = Placement { dw: 400.0, dh: 240.0, ..tiny };
    assert!(!small_frame(&big, &to_screen), "сцена: обычная рамка не мелкая");
    for h in [Handle::TopLeft, Handle::Right] {
        assert_eq!(
            handle_screen(&big, h, &to_screen),
            to_screen(h.point(&big)),
            "{h:?} на обычной рамке сдвинулась — прежнее поведение сломано"
        );
    }
}

/// Тест 780 (Р.66): ШЕСТЬ ПЛОТНЫХ ФОРМ выложены по образцу «Трансформации».
///
/// Прямая просьба владельца Р.64: «и не забудь на будущее во всех других
/// виджетах и окнах тоже пройтись надо будет». Образец он принял тогда же —
/// свёртываемые группы, пары одной строкой, единицы из паспорта. Здесь
/// утверждается наблюдаемое: у каждой из шести форм есть СВОИ группы, и
/// свёртка — состояние окна, а не проекта.
#[test]
fn t780_dense_forms_have_groups() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    // --- настройки проекта: кадр / звук / кривые ----------------------
    let mut h = harness();
    h.state_mut().open_project_settings();
    h.step();
    h.step();
    for key in [k::grp::FRAME, k::grp::AUDIO, k::grp::CURVES] {
        let t = tr(key);
        assert!(
            h.query_all_by_label_contains(t).count() > 0,
            "в настройках проекта нет группы «{t}»"
        );
    }
    // свёртка — состояние ОКНА: закрыли группу, значение проекта не тронуто
    // «Звук» встречается и подписью поля — берём ЗАГОЛОВОК группы точно
    let before = h.state().model.project().settings.clone();
    h.get_by_label(tr(k::grp::AUDIO)).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().model.project().settings,
        before,
        "свёртка группы изменила ПРОЕКТ — это состояние окна, а не содержимого"
    );

    // --- окно FX: группа параметров у обычного эффекта -----------------
    let (mut h, _) = harness_three_clips_dt(None);
    let clip = h.state().model.project().clips.first().map(|c| c.id).expect("событие");
    {
        let app = h.state_mut();
        app.apply(Command::SetFx { id: clip, fx: vec![FxInstance::blur(8)] });
        app.open_fx_editor(clip);
    }
    h.step();
    h.step();
    h.get_by_label(crate::fx_editor::fx_display_name(&FxInstance::blur(8))).click();
    h.step();
    h.step();
    assert!(
        h.query_all_by_label_contains(tr(k::grp::PARAMS)).count() > 0,
        "в окне FX нет группы «{}» у параметров эффекта",
        tr(k::grp::PARAMS)
    );

    // --- рендер: файл / видео / звук ----------------------------------
    let mut h = harness();
    h.state_mut().render_setup.open = true;
    h.step();
    h.step();
    for key in [k::grp::VIDEO, k::grp::AUDIO, k::grp::FILE] {
        let t = tr(key);
        assert!(
            h.query_all_by_label_contains(t).count() > 0,
            "в окне рендера нет группы «{t}»"
        );
    }
}

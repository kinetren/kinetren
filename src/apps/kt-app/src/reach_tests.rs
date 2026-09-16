//! Тесты ДОСТИЖИМОСТИ (Р.38, пп.252-255) — механизм «тупиков не бывает».
//!
//! Повод дословно: «половина тупиковая… Я устал напоминать агентам чтобы
//! они думали об этих мелочах. ОБЯЖИ ИХ ЭТО ПРОВЕРЯТЬ, В ЧЕМ ПРОБЛЕМА??»
//!
//! Проблема в том, что чек-лист в `apps/kt-app/CLAUDE.md` — ПРОЗА, а проза
//! ничего не роняет. Три претензии Р.38 нарушают правила, записанные там
//! дословно (1 «попал мышью с первого раза», 7 «текст помещается»,
//! 17 «парные способы одинаковы»), и это не помогло.
//!
//! Доказательство механизма провала: `RemoveArea`/`RenameArea`/
//! `SetAreaRange`/`SetAreaLanes` реализованы в `kt-model` и покрыты
//! юнит-тестами, а UI их не вызывает НИ РАЗУ — область нельзя ни удалить,
//! ни переименовать. Юнит-тест модели зелен, пока UI до неё не дотянулся:
//! тесты проверяли модель, а не ДОСТИЖИМОСТЬ.
//!
//! Отсюда четыре теста этого файла. Они не заменяют `scripts/live-check.sh`
//! (правило репозитория «харнесс — не доказательство того, что фича
//! работает у пользователя»), они лишь делают ошибку КРАСНОЙ до того, как
//! её найдёт пользователь.

use eframe::egui;
use std::path::{Path, PathBuf};

pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Убрать комментарии: в теле `enum Command` и в `ops.rs` строковых
/// литералов с `//` нет, поэтому полный токенайзер не нужен — тот же
/// уровень строгости, что у сканера шрифтов (`font_tests`).
pub(crate) fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut depth = 0usize; // вложенность /* */
    while i < b.len() {
        if depth > 0 {
            if b[i..].starts_with(b"/*") {
                depth += 1;
                i += 2;
            } else if b[i..].starts_with(b"*/") {
                depth -= 1;
                i += 2;
            } else {
                if b[i] == b'\n' {
                    out.push('\n');
                }
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            depth = 1;
            i += 2;
        } else if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

/// Тело блока, начинающегося с `{` на позиции `open`, по балансу скобок.
fn balanced_body(src: &str, open: usize) -> &str {
    let b = src.as_bytes();
    let mut depth = 0usize;
    for (k, &c) in b.iter().enumerate().skip(open) {
        match c {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[open + 1..k];
                }
            }
            _ => {}
        }
    }
    panic!("несбалансированные скобки при разборе исходника");
}

/// Имена вариантов `Command` — из ИСХОДНИКА модели, а не из списка в
/// тесте: список в тесте стал бы вторым местом правды и разошёлся ровно
/// тогда, когда его перестанут править.
fn command_variants() -> Vec<String> {
    let path = workspace_root().join("libs/kt-model/src/model.rs");
    let src =
        strip_comments(&std::fs::read_to_string(&path).expect("нет libs/kt-model/src/model.rs"));
    let at = src.find("pub enum Command {").expect("в model.rs нет `pub enum Command {`");
    let body = balanced_body(&src, src[at..].find('{').unwrap() + at);

    let b = body.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut item_start = true; // мы в начале элемента перечисления
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'{' | b'(' | b'[' => {
                depth += 1;
                i += 1;
                continue;
            }
            b'}' | b')' | b']' => {
                depth -= 1;
                i += 1;
                continue;
            }
            b',' if depth == 0 => {
                item_start = true;
                i += 1;
                continue;
            }
            _ => {}
        }
        if depth != 0 || c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'#' {
            // атрибут — пропустить целиком, начало элемента не сбрасывать
            while i < b.len() && b[i] != b']' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if item_start && c.is_ascii_uppercase() {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(body[s..i].to_string());
            item_start = false;
            continue;
        }
        item_start = false;
        i += 1;
    }
    out
}

/// `Command::X` в тексте — но именно `X`, а не `Xyz`.
///
/// Границы слова тут не педантизм, а два реальных ложных результата:
/// `Command::SetFx` — префикс `SetFxOff`/`SetFxAuto`/`SetFxBypass`, а
/// `egui::ViewportCommand::Close` содержит подстроку `Command::Close`.
fn constructs(hay: &str, name: &str) -> bool {
    let needle = format!("Command::{name}");
    let b = hay.as_bytes();
    hay.match_indices(&needle).any(|(i, _)| {
        let before = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        let j = i + needle.len();
        let after = j >= b.len() || !(b[j].is_ascii_alphanumeric() || b[j] == b'_');
        before && after
    })
}

/// Элементы `ops.rs` верхнего уровня: (имя функции, тело).
fn ops_items() -> Vec<(String, String)> {
    let path = workspace_root().join("libs/kt-model/src/ops.rs");
    let src =
        strip_comments(&std::fs::read_to_string(&path).expect("нет libs/kt-model/src/ops.rs"));
    let mut out = Vec::new();
    let mut starts: Vec<usize> = Vec::new();
    for (off, line) in src.match_indices('\n') {
        let rest = &src[off + 1..];
        if rest.starts_with("pub fn ") || rest.starts_with("fn ") {
            starts.push(off + 1);
        }
        let _ = line;
    }
    for (k, &s) in starts.iter().enumerate() {
        let e = starts.get(k + 1).copied().unwrap_or(src.len());
        let head = &src[s..];
        let name_at = head.find("fn ").unwrap() + 3;
        let name: String =
            head[name_at..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        out.push((name, src[s..e].to_string()));
    }
    out
}

/// Код, из которого пользователь реально дотягивается до модели:
/// непроверочные исходники kt-app + тела тех функций `kt_model::ops`,
/// которые kt-app зовёт. Второй уровень обязателен: `LinkClips`
/// конструирует только `ops::magnetize_commands`.
fn production_corpus() -> Vec<(String, String)> {
    let mut out = crate::font_tests::sources();
    let app: String = out.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join("\n");
    for (name, body) in ops_items() {
        if app.contains(&format!("ops::{name}")) {
            out.push((format!("kt-model/ops.rs::{name}"), body));
        }
    }
    out
}

/// Команды, которых пользователь НЕ вызывает из GUI, с причиной.
///
/// Список стартует ПУСТЫМ и обязан таким остаться: попала команда сюда —
/// значит её никто не зовёт, и это надо объяснить предложением, а не
/// словом «TODO». Подключил к UI — обязан отсюда убрать (иначе список за
/// год покроет всё и перестанет что-либо значить).
const NOT_FROM_UI: &[(&str, &str)] = &[];

/// Тест 252 (Р.38): осиротевших команд нет.
///
/// Реализованная и покрытая юнит-тестами команда, которую не создаёт ни
/// одна строка приложения, — это фича, недоступная пользователю. Ровно
/// так уехали именованные области Р.35.
#[test]
fn t252_no_orphan_commands() {
    let variants = command_variants();

    // сначала — самопроверка разбора: сломанный парсер дал бы пустой
    // список и «зелёный тест, который ничего не проверяет»
    assert!(
        variants.len() >= 40,
        "парсер вариантов Command сломался: найдено {}, ожидалось не меньше 40",
        variants.len()
    );
    for sentinel in ["AddClip", "SetFxAuto", "RemoveCameraLane"] {
        assert!(
            variants.iter().any(|v| v == sentinel),
            "парсер потерял Command::{sentinel} — дальнейший результат ничего не значит"
        );
    }
    let model = std::fs::read_to_string(workspace_root().join("libs/kt-model/src/model.rs"))
        .expect("нет model.rs");
    for v in &variants {
        assert!(
            constructs(&model, v),
            "Command::{v} разобран из enum, но в model.rs не обрабатывается — \
             значит разбор поймал не то"
        );
    }
    {
        let mut seen = variants.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), variants.len(), "разбор выдал дубликаты вариантов");
    }

    let corpus = production_corpus();

    // идиомы, которые ослепили бы скан, запрещены ТЕМ ЖЕ тестом —
    // иначе механизм отключается одной строкой `use`
    for (file, src) in &corpus {
        assert!(
            !src.contains("Command::*"),
            "{file}: `use …Command::*` прячет конструирование команд от теста 252 — \
             пиши `Command::X` полным путём (то же правило, что у ключей i18n в t125)"
        );
        assert!(
            !src.contains("use kt_model::ops::") && !src.contains("use kt_model::ops;"),
            "{file}: функции ops зовутся полным путём `kt_model::ops::foo` — \
             сокращённый импорт прячет от теста 252 второй уровень скана"
        );
    }

    let text: String = corpus.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join("\n");
    let allowed: Vec<&str> = NOT_FROM_UI.iter().map(|(n, _)| *n).collect();

    // гигиена самого списка исключений
    for (name, reason) in NOT_FROM_UI {
        assert!(
            variants.iter().any(|v| v == name),
            "NOT_FROM_UI упоминает Command::{name}, которой в модели больше нет"
        );
        assert!(
            !constructs(&text, name),
            "Command::{name} теперь вызывается из приложения — убери её из NOT_FROM_UI, \
             иначе список исключений превратится в свалку"
        );
        assert!(
            reason.trim().chars().count() >= 40,
            "причина у Command::{name} короче 40 символов: {reason:?}. \
             Причина — предложение, а не «TODO»"
        );
    }

    let orphans: Vec<&String> = variants
        .iter()
        .filter(|v| !allowed.contains(&v.as_str()) && !constructs(&text, v))
        .collect();
    assert!(
        orphans.is_empty(),
        "осиротевшие команды: реализованы (и обычно покрыты юнит-тестами), но ни одна \
         строка приложения их не создаёт — пользователь физически не может их вызвать. \
         Подключи к UI или удали из модели; юнит-тест на модель НЕ доказательство \
         достижимости, именно так уехали области Р.35: {orphans:?}"
    );
}

/// Проект со ВСЕМИ сущностями таймлайна и ВСЕ экранные точки, откуда
/// человек действует над одной из них.
///
/// Сцена своя на каждую сущность: программа гасит регион и выделение по
/// Esc, а меню иначе не закрыть — общий харнесс молча ломал бы сцену
/// следующей проверки.
///
/// **Точек несколько, и это главная правка Р.40.** До неё дорожки
/// щупались ТОЛЬКО в заголовке строки (`gutter_x`) — полосе шириной 34 px,
/// куда я сам положил пункт «добавить морфер сюда». Тест был зелёным, а
/// человек, жавший ПКМ по самой дорожке, получал меню пустого места:
/// проверено было место, куда я клал, а не место, куда он тычет.
fn scene(
    e: crate::timeline::TlEntity,
) -> (egui_kittest::Harness<'static, crate::app::KtApp>, Vec<(&'static str, egui::Pos2)>) {
    use crate::timeline::TlEntity as E;
    use kt_model::*;
    use kt_time::Tick;

    let mut h = crate::layout_tests::harness();
    {
        let app = h.state_mut();
        let mut p = app.model.project().clone();
        p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
        p.clips.push(ClipEvent {
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(0),
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
        if e == E::Generator {
            // событие-генератор: своё медиа с `gen:`-путём и клип по нему
            p.media.push(MediaRef { id: MediaId(200), path: "gen:solid:c=ff8800".into() });
            let mut genclip = p.clips[0].clone();
            genclip.id = ClipId(4);
            genclip.media = MediaId(200);
            genclip.start = Tick(600_000);
            p.clips.push(genclip);
        }
        if e == E::Nest {
            // ВЛОЖЕНИЕ: лента с одним событием внутри и экземпляр на
            // дорожке — ровно то, по чему человек жмёт правой кнопкой
            let mut seq = Sequence::new(MediaId(100), "Заставка");
            let mut inner = p.clips[0].clone();
            inner.id = ClipId(50);
            inner.start = Tick(0);
            seq.clips.push(inner);
            p.sequences.push(seq);
            let mut inst = p.clips[0].clone();
            inst.id = ClipId(3);
            inst.media = MediaId(100);
            inst.start = Tick(600_000);
            inst.source_in = Tick(0);
            p.clips.push(inst);
        }
        if e == E::Transition {
            // кроссфейд между двумя событиями: окно шире зоны ручек, и
            // правый клик целится в то, что ВИДНО, а не в ±6 px у стыка
            let mut second = p.clips[0].clone();
            second.id = ClipId(2);
            second.start = Tick(50_000);
            p.clips.push(second);
            p.transitions.push(TransitionEvent {
                id: TransitionId(1),
                a: Some(ClipId(1)),
                b: Some(ClipId(2)),
                in_offset: Tick(20_000),
                out_offset: Tick(20_000),
                kind: TransitionKind::Crossfade,
                audio: Default::default(),
                auto: vec![],
                invert: false,
            });
        }
        p.markers.push(Marker::named(Tick(60_000), "метка"));
        p.areas.push(MarkedArea {
            name: "область".into(),
            start: Tick(120_000),
            duration: Tick(50_000),
            lanes: vec![(LaneKind::Video, 0)],
            color: None,
        });
        p.region = Some(kt_model::Region { start: Tick(300_000), end: Tick(350_000) });
        p.validate().unwrap();
        app.model = kt_model::TimelineModel::new(p);
        app.view_start = 0.0;
        app.ticks_per_px = 1000.0;
        // морф-блок ставится НЕ по региону: иначе он накрыл бы границу
        // региона и спор за один пиксель решал бы порядок хит-тестов
        app.player.seek(Tick(200_000));
        app.add_morph_layer();
        assert!(app.apply(Command::InsertCameraLane { at: 0 }));
        app.player.seek(Tick(250_000));
        app.add_camera_at_playhead(0);
        // камера покороче: сцене нужно СВОБОДНОЕ место на её строке —
        // без него «ПКМ по дорожке» щупал бы сам блок камеры
        let mut cam = app.model.project().cameras[0].clone();
        cam.duration = Tick(50_000);
        assert!(app.apply(Command::SetCamera(cam)));
        // окна редакторов, которые размещение открывает само, закрываем:
        // их подписи попали бы в счёт пунктов меню
        app.morph_editors.clear();
    }
    h.step();
    h.step();

    let g = h.state().tl_geom.clone().expect("нет геометрии таймлайна");
    let pr = h.state().model.project().clone();
    assert_eq!(pr.morphers.len(), 1, "сцена: морфер");
    assert_eq!(pr.cameras.len(), 1, "сцена: камера");
    assert!(pr.region.is_some(), "сцена: регион");

    let morph_row = g.row_of((LaneKind::Morph, 0)).expect("морф-строка в раскладке");
    let cam_row = g.row_of((LaneKind::Camera, 0)).expect("камерная строка в раскладке");
    let video_row = g.row_of((LaneKind::Video, 0)).expect("видео-строка в раскладке");
    let gutter_x = g.rect.left() - 30.0;
    let ruler_y = g.rect.top() + 6.0;
    let m = &pr.morphers[0];
    let c = &pr.cameras[0];

    // пустое место НА дорожке — правее всех событий сцены, но в её строке
    // и НА ЭКРАНЕ: точка вне видимой полосы «проверяла» бы пустоту за краем
    let free_t = 500_000;
    let free_x = g.tick_to_x(free_t);
    assert!(
        free_x > g.rect.left() && free_x < g.rect.right(),
        "точка «свободное место» ({free_x}) вне полосы дорожек {:?} — сцена сломана",
        g.rect
    );
    for (a, b) in pr
        .clips
        .iter()
        .map(|c| (c.start.0, (c.start + c.duration).0))
        .chain(pr.morphers.iter().map(|m| (m.start.0, (m.start + m.duration).0)))
        .chain(pr.cameras.iter().map(|c| (c.start.0, (c.start + c.duration).0)))
    {
        assert!(
            free_t < a || free_t >= b,
            "точка «свободное место» попала в событие {a}..{b} — сцена сломана"
        );
    }
    let pos = match e {
        E::Clip => vec![(
            "тело события",
            egui::pos2(g.tick_to_x(25_000), g.lane_y(ClipKind::Video, 0, 0.6)),
        )],
        E::Nest => vec![(
            "тело вложения",
            egui::pos2(g.tick_to_x(625_000), g.lane_y(ClipKind::Video, 0, 0.6)),
        )],
        E::Generator => vec![(
            "тело события-генератора",
            egui::pos2(g.tick_to_x(625_000), g.lane_y(ClipKind::Video, 0, 0.6)),
        )],
        E::Marker => vec![("флажок на линейке", egui::pos2(g.tick_to_x(60_000), ruler_y))],
        // верхний контур области на её дорожке
        E::Area => {
            vec![("контур области", egui::pos2(g.tick_to_x(145_000), g.row_top(video_row) + 1.0))]
        }
        E::Morpher => vec![(
            "тело морф-блока",
            egui::pos2(g.tick_to_x(m.start.0 + m.duration.0 / 2), g.row_center_y(morph_row)),
        )],
        E::Camera => vec![(
            "тело камерного блока",
            egui::pos2(g.tick_to_x(c.start.0 + c.duration.0 / 2), g.row_center_y(cam_row)),
        )],
        E::ClipLane => vec![
            ("заголовок строки", egui::pos2(gutter_x, g.row_center_y(video_row))),
            ("свободное место НА дорожке", egui::pos2(free_x, g.row_center_y(video_row))),
        ],
        E::MorphLane => vec![
            ("заголовок строки", egui::pos2(gutter_x, g.row_center_y(morph_row))),
            ("свободное место НА дорожке", egui::pos2(free_x, g.row_center_y(morph_row))),
        ],
        E::CameraLane => vec![
            ("заголовок строки", egui::pos2(gutter_x, g.row_center_y(cam_row))),
            ("свободное место НА дорожке", egui::pos2(free_x, g.row_center_y(cam_row))),
        ],
        E::Region => vec![("грань региона", egui::pos2(g.tick_to_x(300_000), ruler_y))],
        // окно перехода, подальше от стыка: там ручки, а это про ЖЕСТ
        E::Transition => {
            vec![("окно перехода", egui::pos2(g.tick_to_x(36_000), g.row_center_y(video_row)))]
        }
    };
    (h, pos)
}

/// Правый клик БЕЗ движения указателя: цель меню и его пункты.
fn right_click(h: &mut egui_kittest::Harness<'_, crate::app::KtApp>, pos: egui::Pos2) {
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

/// Тест 253 (Р.38): у каждой сущности таймлайна полный набор глаголов.
///
/// Претензии дословно: «я не понимаю как удалить saved area region, я его
/// сохранил и он теперь висит, не понимаю как его переименовать — пкм
/// ничего не дает», «я не могу никак добавить морфер-событие на морфер
/// трек», «не понимаю как работает камерный трек».
///
/// Тест доказывает ДВЕ вещи, которых юнит-тесты модели не доказывают:
/// сущность существует для хит-теста (её вообще можно ткнуть) и её меню
/// предлагает обязательные операции. Плюс даром — что подсказка над ней не
/// врёт «пустое место».
#[test]
fn t253_every_entity_has_its_verbs() {
    use crate::timeline::TlEntity as E;
    use egui_kittest::kittest::Queryable;

    let mut problems: Vec<String> = Vec::new();
    for e in E::ALL {
        // СВОЙ харнесс на каждое МЕСТО: Esc в этой программе гасит и
        // регион, и выделение, поэтому закрытие меню одной сущности молча
        // ломало сцену следующей (регион исчезал — и «границы региона не
        // существует для хит-теста» было артефактом теста, а не дефектом)
        let places = scene(e).1.len();
        for place_i in 0..places {
            let (mut h, all) = scene(e);
            let (place, pos) = all[place_i];

            // сколько раз каждая нужная подпись видна ДО меню (та же надпись
            // может жить в тулбаре — считаем прирост, а не наличие)
            let want = verbs(e);
            let before: Vec<usize> = want
                .iter()
                .map(|(_, r)| match r {
                    Reach::Menu(l) => h.query_all_by_label_contains(l.as_str()).count(),
                    _ => 0,
                })
                .collect();

            right_click(&mut h, pos);

            let got = h.state().ctx_target.and_then(|t| t.entity());
            if got != Some(e) {
                problems.push(format!(
                    "{e:?} / {place}: правый клик дал цель {got:?} — отсюда сущности не \
                 существует. Нарисовать мало: нужен вариант CtxTarget и хит-тест ИЗ \
                 КАЖДОГО места, откуда человек действует, а не только из того, куда \
                 положили пункт меню"
                ));
                // меню всё равно открылось (пустой целью) — закрыть, иначе
                // следующий правый клик его ЗАКРОЕТ вместо того чтобы открыть
                // своё, и остальные сущности «потеряют» все пункты разом
                continue;
            }

            for ((verb, reach), n0) in want.iter().zip(before) {
                match reach {
                    Reach::Menu(label) => {
                        // «по вхождению»: у дорожек подпись удаления несёт счётчик
                        // («Удалить дорожку (2 событ.)»), и точное сравнение
                        // потребовало бы повторить форматирование в тесте
                        if h.query_all_by_label_contains(label.as_str()).count() <= n0 {
                            problems.push(format!(
                                "{e:?} / {verb:?} / {place}: пункта «{label}» нет в меню. \
                             Создать её можно, а управлять — нет: это тупик"
                            ));
                        }
                    }
                    Reach::Absent(why) | Reach::Elsewhere(why) => {
                        assert!(
                            why.trim().chars().count() >= 25,
                            "{e:?} / {verb:?}: объяснение «{why}» короче 25 символов. \
                         «Нет глагола» и «он в другом месте» принимаются только с \
                         причиной-предложением, иначе таблица станет отговоркой"
                        );
                    }
                }
            }

            // и подсказка над сущностью не врёт «пустое место»
            h.hover_at(pos);
            h.step();
            h.step();
            let hint = h.state().status_line_shown.clone();
            if hint.0 == crate::i18n::tr(crate::i18n::k::tl::hint::EMPTY) {
                problems.push(format!(
                    "{e:?} / {place}: указатель стоит НА сущности, а строка состояния говорит \
                 «пустое место» — подсказка врёт (правило 10 чек-листа)"
                ));
            }
        }
    }

    assert!(problems.is_empty(), "тупики в таймлайне:\n  {}", problems.join("\n  "));
}

/// Тест 254 (Р.38 п.6): клик — это клик.
///
/// Претензия дословно: «в fx-chain виджетах проблематично активировать
/// ввод параметров». Причина не в размере кнопки: `ui.dnd_drag_source`
/// в egui 0.36 регистрирует ПОВЕРХ содержимого прямоугольник с
/// `Sense::drag()` и делает это ПОСЛЕ него, поэтому `hit_test` отдаёт
/// `click: None` — `.clicked()` внутри не наступает НИКОГДА. Пользователь
/// искал «активный пиксель» на полоске frame-rect выше метки.
///
/// Тест жмёт и отпускает В ЦЕНТРЕ элемента БЕЗ движения указателя
/// (`Node::click()` делает именно это) и требует последствия.
#[test]
fn t254_click_is_a_click() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    let mut h = crate::layout_tests::harness();
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
        fx: vec![FxInstance::blur(8), FxInstance::Mirror { horizontal: true, vertical: false }],
        fx_bypass: vec![false, false],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    app.model = kt_model::TimelineModel::new(p);
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();

    // целимся в НЕ выбранный сейчас слот, каким бы ни было умолчание окна
    let sel_of = |h: &egui_kittest::Harness<'_, crate::app::KtApp>| {
        h.state().fx_editors[0].selected.get(&ClipId(1)).copied().unwrap_or(0)
    };
    let was = sel_of(&h);
    let want = 1 - was;
    let names = [
        crate::fx_editor::fx_display_name(&FxInstance::blur(8)),
        crate::fx_editor::fx_display_name(&FxInstance::Mirror {
            horizontal: true,
            vertical: false,
        }),
    ];
    let second = names[want].to_string();
    assert!(h.query_by_label(&second).is_some(), "слот «{second}» обязан быть виден в полосе");

    // ВОТ ОН, клик человека: навёл, нажал, отпустил, не двигая мышь
    h.get_by_label(&second).click();
    h.step();
    h.step();

    assert_eq!(
        sel_of(&h),
        want,
        "клик по слоту «{second}» не выбрал его. Слот завёрнут в ui.dnd_drag_source — \
         в egui 0.36 он кладёт Sense::drag()-прямоугольник ПОСЛЕ и ПОВЕРХ метки, и \
         hit_test отдаёт click:None: клик физически недостижим. Идиома этого \
         репозитория — одна интеракция Sense::click_and_drag() с разбором \
         drag_started()/clicked() (bin_panel.rs, automation.rs, meters.rs, timeline.rs)"
    );

    // клик не должен ещё и переставить слоты: жест «нажал и отпустил на
    // месте» — это выбор, а не перетаскивание
    let fx = h.state().model.project().clips[0].fx.clone();
    assert_eq!(fx.len(), 2, "клик не меняет состав цепочки");
    assert!(matches!(fx[0], FxInstance::Blur { .. }), "и не переставляет слоты");

    // крестик слота — обычная кнопка, но она стоит В ТОЙ ЖЕ полосе
    let crosses = h.get_all_by_label(crate::icons::CLEAR).count();
    assert!(crosses >= 2, "у каждого слота обязан быть крестик, найдено {crosses}");
}

/// Глагол — операция, без которой сущность становится тупиком: создал и
/// живи с этим.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Verb {
    Create,
    Rename,
    Delete,
    /// Открыть СОДЕРЖИМОЕ или свойства: окно FX у события, редактор у
    /// морфера, окно вида у перехода, вход внутрь у вложения (Р.50).
    ///
    /// Заведён потому, что без него «войти в ленту» некуда было записать,
    /// а вход — единственное, ради чего вложение отличается от клипа.
    Open,
}

/// Как глагол ДОСТИЖИМ.
enum Reach {
    /// Пункт контекст-меню сущности с такой подписью.
    Menu(String),
    /// Глагола у сущности нет ПО СУЩЕСТВУ. Причина обязательна и
    /// проверяется на длину: «нет» без объяснения не принимается.
    Absent(&'static str),
    /// Доступен не меню, а жестом или окном — и это доказывает НАЗВАННЫЙ
    /// тест. Ссылка обязательна: без неё «где-то там» превращается в
    /// отговорку, ровно как прозаический чек-лист.
    Elsewhere(&'static str),
}

/// ЕДИНСТВЕННАЯ таблица «сущность → глаголы». `match` исчерпывающий:
/// новая сущность таймлайна не соберётся, пока строку не заполнили.
fn verbs(e: crate::timeline::TlEntity) -> Vec<(Verb, Reach)> {
    use crate::i18n::{k, tr};
    use crate::timeline::TlEntity as E;
    let menu = |s: &str| Reach::Menu(s.to_string());
    match e {
        E::Clip => vec![
            (Verb::Create, Reach::Elsewhere("медиа-бин, t99; вставка, t205")),
            (Verb::Rename, Reach::Absent("у события нет имени — подпись это имя медиа")),
            (Verb::Delete, menu(tr(k::tl::menu::DELETE))),
            (Verb::Open, menu(tr(k::tl::menu::FX))),
        ],
        E::Generator => vec![
            (
                Verb::Create,
                Reach::Elsewhere(
                    "подменю «Генератор» в меню фона бина кладёт и медиа, и клип \
                     под курсор одним шагом истории, t512",
                ),
            ),
            (
                Verb::Rename,
                Reach::Absent(
                    "у генератора нет имени: подпись — это его ВИД, а вид меняется \
                     пикером в окне параметров",
                ),
            ),
            (Verb::Delete, menu(tr(k::tl::menu::DELETE))),
            (Verb::Open, menu(tr(k::tl::menu::GEN_PARAMS))),
        ],
        E::Nest => vec![
            (Verb::Create, menu(tr(k::tl::menu::COLLAPSE))),
            (Verb::Rename, menu(tr(k::tl::menu::NEST_RENAME))),
            (Verb::Delete, menu(tr(k::tl::menu::DELETE))),
            (Verb::Open, menu(tr(k::tl::menu::NEST_ENTER))),
        ],
        E::Marker => vec![
            (
                Verb::Create,
                Reach::Elsewhere("жест M и пункт MARKER_HERE в меню пустого места, t95/t170"),
            ),
            (Verb::Rename, menu(tr(k::tl::menu::RENAME))),
            (Verb::Delete, menu(tr(k::tl::menu::REMOVE_MARKER))),
            (
                Verb::Open,
                Reach::Elsewhere(
                    "двойной клик по ленте метки-диапазона делает по ней регион, t206",
                ),
            ),
        ],
        E::Transition => vec![
            (
                Verb::Create,
                Reach::Elsewhere(
                    "меню события: TRANSITION_NEXT/TRANSITION_PREV с соседом, \
                     FADE_IN/FADE_OUT у свободного края; жест уголка, t173",
                ),
            ),
            (Verb::Rename, Reach::Absent("переход безымянен: он между событиями, его «имя» — вид")),
            (Verb::Delete, menu(tr(k::tl::menu::REMOVE_TRANSITION))),
            (Verb::Open, menu(tr(k::tl::menu::TRANSITION_PROPS))),
        ],
        E::Area => vec![
            (Verb::Create, Reach::Elsewhere("пункт меню пустого места SAVE_AREA")),
            (Verb::Rename, menu(tr(k::tl::menu::RENAME))),
            (Verb::Delete, menu(tr(k::tl::menu::DELETE))),
            (
                Verb::Open,
                Reach::Absent("у области нет содержимого: это имя, время и набор дорожек"),
            ),
        ],
        E::Morpher => vec![
            (
                Verb::Create,
                Reach::Elsewhere(
                    "пункт ADD_MORPHER в меню пустого места; на дорожке — \
                                  глагол Create у самой MorphLane",
                ),
            ),
            (Verb::Rename, Reach::Absent("морфер безымянен: это слой правок, а не объект")),
            (Verb::Delete, menu(tr(k::tl::menu::REMOVE_MORPHER))),
            (Verb::Open, menu(tr(k::tl::menu::MORPH_EDITOR))),
        ],
        E::Camera => vec![
            (Verb::Create, Reach::Elsewhere("пункт ADD_CAMERA_HERE в меню камерной дорожки")),
            (Verb::Rename, Reach::Elsewhere("поле имени в окне камеры, t231b")),
            (Verb::Delete, menu(tr(k::tl::menu::DELETE))),
            (Verb::Open, menu(tr(k::cam::PROPS))),
        ],
        // у дорожки Create — «положить сюда свой элемент»: событие в бин
        // умеет только бин, а морфер и камера ставятся ИЗ МЕНЮ ДОРОЖКИ.
        // Асимметрия этих трёх строк и была претензией п.2 Р.38.
        E::ClipLane => vec![
            (Verb::Create, Reach::Elsewhere("события кладёт медиа-бин переносом, t99")),
            (Verb::Rename, menu(tr(k::tl::lane::RENAME))),
            (Verb::Delete, menu(tr(k::tl::lane::DELETE))),
            (Verb::Open, Reach::Absent("дорожка и есть содержимое — открывать у неё нечего")),
        ],
        E::MorphLane => vec![
            (Verb::Create, menu(tr(k::tl::lane::ADD_MORPHER_HERE))),
            (Verb::Rename, menu(tr(k::tl::lane::RENAME))),
            (Verb::Delete, menu(tr(k::tl::lane::DELETE))),
            (Verb::Open, Reach::Absent("дорожка и есть содержимое — открывать у неё нечего")),
        ],
        E::CameraLane => vec![
            (Verb::Create, menu(tr(k::tl::lane::ADD_CAMERA_HERE))),
            (Verb::Rename, menu(tr(k::tl::lane::RENAME))),
            (Verb::Delete, menu(tr(k::tl::lane::DELETE))),
            (Verb::Open, Reach::Absent("дорожка и есть содержимое — открывать у неё нечего")),
        ],
        E::Region => vec![
            (Verb::Create, Reach::Elsewhere("драг по руллеру с пустого места, t96")),
            (Verb::Rename, Reach::Absent("регион безымянен; именованное — это область")),
            (Verb::Delete, menu(tr(k::tl::menu::REGION_CLEAR))),
            (
                Verb::Open,
                Reach::Absent("регион — это две грани на линейке, содержимого у него нет"),
            ),
        ],
    }
}

/// Нарисованный текст одного кадра: что просили нарисовать, что реально
/// нарисовалось, сколько строк заняло.
///
/// Смотреть надо именно на ГАЛЛЕИ, а не на accesskit: у кнопки метка
/// доступности берётся из исходного текста и многоточия не содержит
/// никогда, поэтому «усечено или перенесено» по ней не отличить.
#[derive(Clone)]
struct PaintedText {
    src: String,
    drawn: String,
    rows: usize,
    elided: bool,
    rect: egui::Rect,
}

fn collect_texts(shape: &egui::epaint::Shape, out: &mut Vec<PaintedText>) {
    use egui::epaint::Shape;
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| collect_texts(s, out)),
        Shape::Text(t) => {
            let g = &t.galley;
            out.push(PaintedText {
                src: g.text().to_string(),
                drawn: g.rows.iter().map(|r| r.row.text()).collect::<Vec<_>>().join(""),
                rows: g.rows.len(),
                elided: g.elided,
                rect: g.rect.translate(t.pos.to_vec2()),
            });
        }
        _ => {}
    }
}

fn painted_texts(h: &egui_kittest::Harness<'_, crate::app::KtApp>) -> Vec<PaintedText> {
    let mut out = Vec::new();
    for cs in &h.output().shapes {
        collect_texts(&cs.shape, &mut out);
    }
    out
}

/// Тест 255 (Р.38 пп.7-8): длинное имя усекается, а не разносит полосу.
///
/// Претензии дословно: «если в кроше Source указать сурс с длинным именем
/// — то весь чейн разлетается по длинне» и «также если накидать много
/// крошек — то последняя надпись упирается в край».
///
/// Полоса цепочки живёт в `horizontal_wrapped`, а он ставит
/// `TextWrapMode::Wrap`: метка переносится ВНУТРИ СЕБЯ, и поскольку в
/// имени файла нет пробелов — по буквам. Это и есть «разлетается».
/// Правило 7 чек-листа требует обратного: меряй и усекай.
/// Полоса цепочки морфера с источником по имени `file`: вертикальный
/// разброс её подписей и сами подписи имени.
fn strip_metrics(file: &str) -> (f32, Vec<PaintedText>) {
    use kt_model::*;

    let mut h = crate::layout_tests::harness();
    let app = h.state_mut();
    let mut p = app.model.project().clone();
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.media.push(MediaRef { id: MediaId(2), path: format!("/tmp/{file}") });
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
    app.add_morph_layer();
    h.step();
    h.step();

    let app = h.state_mut();
    let mut ev = app.model.project().morphers[0].clone();
    ev.patch.media = Some(MediaId(2));
    ev.patch.video_fx = vec![
        FxInstance::blur(8),
        FxInstance::Mirror { horizontal: true, vertical: false },
        FxInstance::edges(64),
    ];
    assert!(app.apply(Command::SetMorpher(ev)));
    h.step();
    h.step();

    let texts = painted_texts(&h);
    let want_src = crate::tf!(crate::i18n::k::mrph::SRC_SLOT, file.to_string());
    let names: Vec<&PaintedText> = texts
        .iter()
        .filter(|t| {
            t.src == want_src
                || t.src == crate::fx_editor::fx_display_name(&FxInstance::blur(8))
                || t.src == crate::fx_editor::fx_display_name(&FxInstance::edges(64))
        })
        .collect();
    assert!(names.len() >= 3, "в полосе обязаны быть голова и слоты эффектов: {}", names.len());
    let top = names.iter().fold(f32::MAX, |a, t| a.min(t.rect.top()));
    let bot = names.iter().fold(f32::MIN, |a, t| a.max(t.rect.bottom()));
    let heads =
        texts.iter().filter(|t| t.src == want_src).map(|t| PaintedText { ..t.clone() }).collect();
    (bot - top, heads)
}

/// Тест 255 (Р.38 пп.7-8): длинное имя усекается, а не разносит полосу.
///
/// Претензии дословно: «если в кроше Source указать сурс с длинным именем
/// — то весь чейн разлетается по длинне» и «также если накидать много
/// крошек — то последняя надпись упирается в край».
///
/// Полоса живёт в `horizontal_wrapped`, а он ставит `TextWrapMode::Wrap`:
/// подпись переносилась ВНУТРИ СЕБЯ, и поскольку в имени файла нет
/// пробелов — по буквам. Правило 7 чек-листа требует обратного: меряй и
/// усекай. Перенос крошки ЦЕЛИКОМ на следующий ряд при этом законен —
/// незаконно растягивать полосу по вертикали именем.
#[test]
fn t255_long_name_is_elided_not_wrapped() {
    let long = format!("{}.mov", "ОченьДлинноеИмяИсходника".repeat(8));
    let (h_short, _) = strip_metrics("clip.mov");
    let (h_long, heads) = strip_metrics(&long);

    assert!(!heads.is_empty(), "подпись с именем источника обязана рисоваться");
    // ОБА места, где рисуется имя (поле источника и голова полосы)
    for t in &heads {
        assert_eq!(
            t.rows, 1,
            "имя источника перенеслось на {} строк(и) — оно обязано УСЕКАТЬСЯ, а не \
             рваться по буквам (правило 7 чек-листа: меряй и решай, а не надейся, что \
             имя короткое)",
            t.rows
        );
        assert!(
            t.elided && t.drawn.ends_with('…'),
            "имя нарисовано целиком, без усечения и многоточия: {:?}",
            t.drawn
        );
    }

    assert!(
        h_long <= h_short + 1.0,
        "полоса цепочки выросла с {h_short:.0} px до {h_long:.0} px только оттого, что у \
         источника длинное имя — «весь чейн разлетается по длинне». Имя обязано быть \
         усечено по ширине, а не растягивать полосу"
    );
}

/// Тест 254б: `dnd_drag_source` запрещён во всём приложении.
///
/// Одной строкой закрывается КЛАСС, а не один виджет: любой будущий
/// `dnd_drag_source` вокруг кликабельного снова убьёт клик молча.
#[test]
fn t254b_no_dnd_drag_source_over_clickables() {
    // без комментариев: объяснение, ПОЧЕМУ так больше не пишут, само
    // содержит это слово — и тест ловил бы собственную документацию
    let bad: Vec<String> = crate::font_tests::sources()
        .into_iter()
        .filter(|(_, s)| strip_comments(s).contains("dnd_drag_source"))
        .map(|(f, _)| f)
        .collect();
    assert!(
        bad.is_empty(),
        "{bad:?}: ui.dnd_drag_source кладёт Sense::drag()-прямоугольник ПОВЕРХ своего \
         содержимого (egui 0.36) и убивает клик по нему — hit_test отдаёт click:None. \
         Идиома репозитория: одна интеракция Sense::click_and_drag(), \
         drag_started() начинает перетаскивание, clicked() выбирает"
    );
}

/// Тест 273 (Р.39): арифметики над `source_in` вне ОДНОГО правила не бывает.
///
/// Повод: комментарий над `clip_slice` обещал «правило кусков ОДНО на весь
/// проект», а линейная формула была выписана руками ещё в шести местах —
/// поэтому починка `Split` не чинила ни тримы, ни punch-out, ни кроп
/// копируемого блока. Тот же приём, что t252 для осиротевших команд:
/// обещание, которое нечем проверить, перестаёт выполняться.
#[test]
fn t273_source_in_arithmetic_lives_in_one_place() {
    let root = workspace_root();
    // где правилу и место: сама функция и её помощники
    const HOME: &str = "libs/kt-model/src/project.rs";
    let mut bad: Vec<String> = Vec::new();
    for rel in ["libs/kt-model/src/model.rs", "libs/kt-model/src/ops.rs", HOME] {
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        for (n, line) in src.lines().enumerate() {
            // присваивание source_in из выражения с ним же — это и есть
            // «своя формула куска»
            let t = line.trim();
            let assigns = t.contains("source_in =") || t.contains("source_in +=");
            if !assigns {
                continue;
            }
            // в доме правила такие строки законны
            if rel == HOME {
                continue;
            }
            bad.push(format!("{rel}:{}: {t}", n + 1));
        }
    }
    assert!(
        bad.is_empty(),
        "материальная точка входа куска считается ВНЕ единственного правила \
         (`clip_slice`/`slice_source_in` в {HOME}). На растянутом клипе своя формула \
         уезжает, и починка одного жеста не чинит соседние — ровно так Split, тримы, \
         punch-out и кроп разъехались до Р.39:\n  {}",
        bad.join("\n  ")
    );
}

/// Тест 426b (Р.50): поиск источника ОТ КЛИПА живёт в одном доме.
///
/// Повод — цена, которую мы сознательно платим за выбранный носитель ссылки.
/// Адрес вложенной ленты лежит в ОДНОМ пространстве с `MediaId` файлов, но
/// `MediaRef`-а у ленты нет: тело живёт в `Project.sequences`. Поэтому код,
/// спрашивающий `project.media` про `clip.media`, для вложения получает
/// `None` — и это не отказ, а ТИХАЯ неправильная работа: watchdog покрасит
/// вложение тинтом офлайна, `IndexService` полезет строить по нему пики
/// ffmpeg'ом, команда откажет с «медиа не найдено».
///
/// Разрешено ровно одно: `Project::source()`. Перебирать `project.media` само
/// по себе законно (бин, диалог путей, сборка карты путей — они и правда про
/// ФАЙЛЫ); запрещено сравнивать элемент этого перебора с полем `media` клипа.
#[test]
fn t426b_media_lookup_from_a_clip_lives_in_one_place() {
    let root = workspace_root();
    // дом правила — сама `Project::source()` и её соседи по файлу
    const HOME: &str = "libs/kt-model/src/project.rs";
    let mut bad: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    for dir in ["libs", "apps"] {
        collect_rs(&root.join(dir), &mut files);
    }
    for rel in files {
        if rel == HOME || rel.contains("/tests/") || rel.ends_with("_tests.rs") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(root.join(&rel)).expect(&rel));
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            if !t.contains("media.iter()") {
                continue;
            }
            // «сравнили с полем `media` какого-то события» — вот это и есть
            // поиск ОТ КЛИПА
            let from_clip = t.contains(".media)")
                || t.contains(".media {")
                || t.contains("== c.media")
                || t.contains("== clip.media");
            if from_clip {
                bad.push(format!("{rel}:{}: {t}", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "источник события ищется в `project.media` мимо единственного дома          (`Project::source()` в {HOME}). У вложенной секвенции `MediaRef`-а нет:          такой поиск вернёт None и превратит вложение в «офлайн-медиа» молча —          ни одного отказа, ни одной ошибки, просто неправильная работа:\n  {}",
        bad.join("\n  ")
    );
}

/// Тест 456b (Р.50, В1): наружу уходит ДОКУМЕНТ, а не рабочий вид.
///
/// Стоя внутри ленты, `model.project()` отдаёт ЕЁ содержимое на месте
/// корня — на этом держится весь интерфейс, который о вложениях не знает
/// вовсе. Но плееру, файлу и экспорту нужен документ: отдать им рабочий вид
/// значит записать на диск проект, у которого содержимое корня и ленты
/// поменялись местами. Человек заметил бы это только при следующем открытии
/// — ровно тот класс тихой потери работы, из-за которого заведён Р.49.
#[test]
fn t456b_only_the_document_leaves_the_model() {
    let root = workspace_root();
    let mut bad: Vec<String> = Vec::new();
    for (file, src) in production_corpus() {
        if !file.contains("kt-app") && !file.ends_with(".rs") {
            continue;
        }
        let _ = &root;
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            // на одной строке: «отдаём наружу» и «рабочий вид»
            let out = t.contains("set_project(")
                || t.contains("save_project(")
                || t.contains("render_raw_video")
                || t.contains("ExportBackends");
            if out && t.contains("model.project()") {
                bad.push(format!("{file}:{}: {t}", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "наружу (в плеер, в файл, в экспорт) уходит РАБОЧИЙ ВИД вместо документа. \
         Стоя внутри ленты он держит её содержимое на месте корня — записать такое \
         значит поменять местами корень и ленту на диске. Нужен `model.document()` \
         (а офлайн-счёту по тому, что под руками — `model.reel_document()`):\n  {}",
        bad.join("\n  ")
    );
}

/// Рекурсивный обход `.rs` с путями относительно корня workspace.
fn collect_rs(dir: &std::path::Path, out: &mut Vec<String>) {
    let root = workspace_root();
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == "vendor") {
                continue;
            }
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs")
            && let Ok(rel) = p.strip_prefix(&root)
        {
            out.push(rel.to_string_lossy().into_owned());
        }
    }
}

// ---- правый клик В ПАНЕЛЯХ (Р.49, волна В) ---------------------------------

/// Глагол панельного объекта — что человек хочет с ним сделать.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PanelVerb {
    /// Правая кнопка вообще отвечает.
    Menu,
    /// Хоть одно ДЕЙСТВИЕ в этом меню, а не только заголовок.
    Act,
}

/// ЕДИНСТВЕННАЯ таблица «панельная сущность → чем отвечает правая кнопка».
/// `match` исчерпывающий: новая панельная сущность не соберётся, пока
/// строку не заполнили.
fn panel_verbs(e: crate::ctx_menu::PanelEntity) -> Vec<(PanelVerb, &'static str)> {
    use crate::ctx_menu::PanelEntity as P;
    match e {
        P::FxSlot => vec![
            (PanelVerb::Menu, "правый клик по слоту цепочки открывает его меню"),
            (PanelVerb::Act, "включить или выключить слот, удалить слот из цепочки"),
        ],
        P::FxHead => vec![
            (PanelVerb::Menu, "правый клик по приколотой голове цепочки открывает меню"),
            (PanelVerb::Act, "включить или выключить источник морфер-слоя"),
        ],
        P::CurveKey => vec![
            (PanelVerb::Menu, "правый клик по ключу кривой автоматизации открывает меню"),
            (
                PanelVerb::Act,
                "удалить этот ключ; сама правая кнопка НЕ удаляет — иначе обещание \
                 «правой кнопкой можно пробовать безопасно» обесценится везде",
            ),
        ],
        P::Bin => vec![
            (PanelVerb::Menu, "правый клик по свободному фону медиа-бина открывает меню"),
            (
                PanelVerb::Act,
                "переключить раскладку: свободная или сетка; создать генератор (Р.52) — \
                 единственный вход для медиа без файла",
            ),
        ],
        P::Preview => vec![
            (PanelVerb::Menu, "правый клик по окну превью открывает его меню"),
            (PanelVerb::Act, "отделить превью в своё окно или вернуть обратно"),
        ],
        P::Meters => vec![
            (PanelVerb::Menu, "правый клик по шкалам метров и по фейдеру открывает меню"),
            (PanelVerb::Act, "приглушить мастер, сбросить фейдер в 0 dB, сбросить пик"),
        ],
    }
}

/// Тест 395 (Р.49 В3/В6): у КАЖДОЙ панельной сущности правая кнопка чем-то
/// отвечает, и это записано в одном месте.
///
/// Реестр панелей отдельный от `TlEntity` намеренно: у целей таймлайна общая
/// сцена и общий род идентификатора (время и дорожка), у панелей — ни того,
/// ни другого. Сваленные в один enum, они дали бы `match`, половина веток
/// которого в своём месте не встречается никогда.
#[test]
fn t395_every_panel_entity_answers_right_click() {
    use crate::ctx_menu::PanelEntity as P;
    assert_eq!(P::ALL.len(), P::N, "литерал ALL разошёлся с N");
    for e in P::ALL {
        let verbs = panel_verbs(e);
        assert!(
            verbs.iter().any(|(v, _)| *v == PanelVerb::Menu),
            "{e:?}: не сказано, отвечает ли правая кнопка"
        );
        for (_, why) in &verbs {
            assert!(
                why.trim().chars().count() >= 20,
                "{e:?}: объяснение «{why}» короче 20 символов — так строка \
                 заполняется «лишь бы собралось»"
            );
        }
    }
    // род цели — исчерпывающий match, каждая цель кому-то принадлежит
    use crate::ctx_menu::PanelTarget as T;
    use kt_model::ClipId;
    let all: Vec<T> = vec![
        T::FxSlot { chain: ClipId(1), slot: 0 },
        T::FxHead { chain: ClipId(1) },
        T::CurveKey { chain: ClipId(1), slot: 0, curve: 0, key: 0 },
        T::BinEmpty,
        T::Preview,
        T::Meters,
    ];
    let mut seen: Vec<crate::ctx_menu::PanelEntity> = all.iter().map(|t| t.entity()).collect();
    seen.sort_by_key(|e| *e as usize);
    seen.dedup();
    assert_eq!(seen.len(), P::N, "не у каждой панельной сущности есть своя цель: {seen:?}");
}

/// Тест 396 (Р.49 В6): объект, который отвечает на НАВЕДЕНИЕ, обязан
/// отвечать и на правую кнопку.
///
/// Скан каталога, а не кода: подсказка объекта — это обещание «я вещь, со
/// мной можно работать», и человек, научившийся правой кнопке на таймлайне,
/// жмёт её везде. Молчание читается как «объекта нет» — ровно тот класс
/// тупиков, что стоил раундов Р.33-Р.40. Родня «catalog keys nobody uses».
#[test]
fn t396_hinted_objects_have_a_right_click_answer() {
    // объектные подсказки: у каждой в паре стоит панельная сущность,
    // которая на правую кнопку отвечает
    const ANSWERED: &[(&str, &str)] = &[
        ("fx.slot_hint", "FxSlot"),
        ("mrph.src_slot_hint", "FxHead"),
        ("bin.hint_free", "Bin"),
        ("bin.hint_grid", "Bin"),
        ("hint.preview_click", "Preview"),
        ("meter.mute", "Meters"),
        ("meter.fader", "Meters"),
        ("meter.peak", "Meters"),
        ("fx.key_at", "CurveKey"),
    ];
    let src = crate::font_tests::sources();
    let named: Vec<&str> = crate::ctx_menu::PanelEntity::ALL
        .iter()
        .map(|e| match e {
            crate::ctx_menu::PanelEntity::FxSlot => "FxSlot",
            crate::ctx_menu::PanelEntity::FxHead => "FxHead",
            crate::ctx_menu::PanelEntity::CurveKey => "CurveKey",
            crate::ctx_menu::PanelEntity::Bin => "Bin",
            crate::ctx_menu::PanelEntity::Preview => "Preview",
            crate::ctx_menu::PanelEntity::Meters => "Meters",
        })
        .collect();
    for (key, entity) in ANSWERED {
        assert!(
            named.contains(entity),
            "{key}: назван род «{entity}», которого нет в реестре панелей"
        );
        // ключ подсказки обязан и правда существовать в коде: иначе
        // таблица сторожит выдуманное
        let path = {
            let mut parts: Vec<String> = key.split('.').map(str::to_string).collect();
            if let Some(last) = parts.last_mut() {
                *last = last.to_uppercase();
            }
            parts.join("::")
        };
        assert!(
            src.iter().any(|(_, s)| s.contains(&path)),
            "{key}: такой подсказки в коде нет — таблица сторожит выдуманное"
        );
    }
    // и обратное: каждая панельная сущность закрыта хотя бы одной подсказкой
    for e in named {
        assert!(
            ANSWERED.iter().any(|(_, x)| *x == e),
            "{e}: сущность в реестре есть, а подсказки, которую она закрывает, \
             в таблице нет — либо объект нем на наведение, либо строку забыли"
        );
    }
}

/// Тест 508 (Р.53, п.414): у КАЖДОГО параметра есть подпись из каталога.
///
/// `param_label` на неизвестном имени возвращает СЫРОЕ английское имя
/// поля — и делает это молча, на всех пяти языках. Так в окнах Р.52
/// оказались `line_spacing`, `plate_pad_x`, `angle_deg`, а кегль титра
/// подписался «Зерно:» (чужим ключом эффекта Noise). Ни t100, ни t125
/// этого не ловят: ключа просто не существует.
#[test]
fn t508_every_param_has_a_label() {
    use kt_model::HasParams;
    let mut bad: Vec<String> = Vec::new();
    let mut check = |who: &str, f: &dyn HasParams, covered: &[&str]| {
        let scope = f.params_scope();
        for spec in f.param_specs() {
            // компоненты цвета подписи не имеют и не должны: они закрыты
            // ПИПЕТКОЙ (правило 13 — три ползунка 0..255 на один цвет это
            // трата места). Но закрыты обязаны быть ВСЕ: незакрытый
            // Rgb8-параметр вывалится в сетку сырым именем.
            if spec.unit == kt_model::ParamUnit::Rgb8 {
                assert!(
                    covered.contains(&spec.name),
                    "{who}.{}: компонента цвета не попала ни в одну пипетку — \
                     пользователь увидит сырое имя поля",
                    spec.name
                );
                continue;
            }
            let label = crate::widgets::param_label_scoped(scope, spec.name);
            // подпись, равная имени поля, — это и есть «ключа нет»
            if label == spec.name {
                bad.push(format!("{who}.{}", spec.name));
            }
        }
        for spec in f.flag_specs() {
            if crate::widgets::flag_label(spec.name) == spec.name {
                bad.push(format!("{who}.{} (флаг)", spec.name));
            }
        }
    };
    for k in kt_model::GeneratorKind::all() {
        let covered: Vec<&str> = crate::generator_editor::color_groups(&k)
            .iter()
            .flat_map(|(_, names)| names.iter().copied())
            .collect();
        check(k.tag(), &k, &covered);
    }
    // у переходов цвет рисует своя пипетка (окно перехода, Р.33 С4)
    const TRANS_COLOR: &[&str] = &["color_r", "color_g", "color_b", "tint_r", "tint_g", "tint_b"];
    for k in kt_model::TransitionKind::ALL {
        check(k.tag(), k, TRANS_COLOR);
    }
    check("camera", &kt_model::CameraParams::default(), &[]);
    assert!(
        bad.is_empty(),
        "параметры без подписи в каталоге — пользователь видит сырое имя поля:\n  {}",
        bad.join("\n  ")
    );
}

/// Тест 509 (Р.53, п.414): у каждого ФЛАГА есть контрол.
///
/// Флаг, объявленный в паспорте, но не нарисованный ни одним окном, —
/// это ровно та «осиротевшая фича», которую сторожит t252 для команд:
/// тон 1 кГц у SMPTE существовал в формате с Р.52, а включить его было
/// нечем. Сканируем корпус kt-app на вызов общего виджета `flags_row`.
#[test]
fn t509_every_flag_has_a_control() {
    let root = workspace_root();
    let mut files: Vec<String> = Vec::new();
    collect_rs(&root.join("apps/kt-app"), &mut files);
    let mut draws_flags = false;
    for rel in &files {
        if rel.ends_with("_tests.rs") || rel.contains("/tests/") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        if src.contains("widgets::flags_row(") {
            draws_flags = true;
        }
    }
    let has_flags = kt_model::GeneratorKind::all().iter().any(|k| {
        use kt_model::HasParams;
        !k.flag_specs().is_empty()
    });
    assert!(
        !has_flags || draws_flags,
        "виды объявляют флаги, но ни одно окно не зовёт widgets::flags_row — \
         включить их пользователю нечем"
    );
}

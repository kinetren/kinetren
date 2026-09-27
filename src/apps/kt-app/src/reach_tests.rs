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
    for sentinel in ["AddClip", "SetFxAuto", "RemoveTrack"] {
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
            // событие-генератор: медиа несёт ТОЛЬКО ВИД, а ЗНАЧЕНИЯ живут
            // на событии (Р.59, стадия 2) — иначе `validate` отвергает
            // сцену, и это правильно: без значений генератору нечего
            // рисовать
            p.media.push(MediaRef::new(MediaId(200), "gen:solid"));
            let mut genclip = p.clips[0].clone();
            genclip.id = ClipId(4);
            genclip.media = MediaId(200);
            genclip.start = Tick(600_000);
            genclip.r#gen = Some(GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
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
        assert!(app.apply(Command::InsertTrack { kind: kt_model::LaneKind::Camera, at: 0 }));
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
        h.state().fx_editors[0].selected.get(&ClipId(1)).and_then(|s| s.fx()).unwrap_or(0)
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

/// Тест 547 (Р.54, п.431): галерея пикера доходит до МОДЕЛИ.
///
/// Утверждается следствие, которое видит человек: он открыл пикер, нажал
/// на ячейку «Плотно» у Blur — и на событии стоит Blur ИМЕННО с её
/// параметрами, а не шаблонный. Щупается оттуда, откуда действует человек:
/// клик по ячейке, найденной по её подписи.
///
/// Плюс закрепляется новое поведение попапа: у меню egui дефолт
/// `CloseOnClick` — попап закрывается ЛЮБЫМ кликом внутри, и галерея с
/// кликабельными миниатюрами была бы мертворождённой. Тест жмёт в строку
/// поиска и требует, чтобы попап ОСТАЛСЯ открыт.
#[test]
fn t547_gallery_click_reaches_the_model() {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let mut h = crate::layout_tests::harness();
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
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();

    // 1) открыть пикер — та же кнопка «+ эффект…», что и до Р.54
    let add = crate::i18n::tr(crate::i18n::k::fxreg::ADD).to_string();
    assert!(h.query_by_label(&add).is_some(), "кнопки пикера нет");
    h.get_by_label(&add).click();
    h.step();
    h.step();

    // 2) галерея видна: имя эффекта и подписи его вариантов
    assert!(h.query_by_label("Blur").is_some(), "строка эффекта не нарисовалась");
    let heavy = {
        let e = crate::fx_registry::registry()
            .iter()
            .find(|e| e.name == "Blur")
            .expect("Blur в реестре");
        let v = &e.variants[1];
        // доступное имя ячейки — составное: подпись «Плотно» есть у
        // шести видов, и различить их можно только с именем эффекта
        (format!("{} {}", e.name, crate::i18n::tr(v.name)), v.fx.clone())
    };
    // до поиска ячейка Blur лежит НИЖЕ СГИБА прокрутки, и её там
    // сознательно нет: невидимая ячейка не рисуется и не считается
    // (иначе одно открытие галереи заказывало полторы сотни миниатюр —
    // нашёл живой сценарий fxpicker)
    assert!(
        h.query_by_label(&heavy.0).is_none(),
        "ячейка «{}» нарисована ниже сгиба: гард видимости не работает",
        heavy.0
    );

    // 3) НАБИРАЕМ в поиске, как человек: галерея из 27 строк не влезает
    // в прокрутку целиком, и нужная строка может быть НИЖЕ сгиба — ровно
    // поэтому у пикера и есть поиск. Строка поиска получает фокус сама
    // при открытии меню (это и закрепляется здесь).
    h.event(egui::Event::Text("blur".into()));
    h.step();
    h.step();
    assert!(
        h.query_by_label(&heavy.0).is_some(),
        "после поиска ячейка «{}» обязана быть видна: ввод ВНУТРИ попапа \
         его не закрывает (у меню egui дефолт CloseOnClick, галерее нужен \
         MenuConfig::close_behavior(CloseOnClickOutside))",
        heavy.0
    );

    // 4) клик по ячейке — и в модели ИМЕННО эти параметры
    h.get_by_label(&heavy.0).click();
    h.step();
    h.step();
    let fx = h.state().model.project().clips[0].fx.clone();
    assert_eq!(fx.len(), 1, "клик по ячейке варианта не добавил эффект");
    assert_eq!(
        format!("{:?}", fx[0]),
        format!("{:?}", heavy.1),
        "добавился не вариант, а что-то другое (шаблон?)"
    );
    // попап закрылся ВЫБОРОМ (явный ui.close), а не остался висеть
    assert!(h.query_by_label(&heavy.0).is_none(), "после выбора попап остался открыт");
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
                    "подменю «Генератор» — в ГЛАВНОМ меню и в меню фона бина, одно \
                     тело на оба входа; кладёт и медиа, и клип под курсор одним \
                     шагом истории, t512/t560",
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
            (Verb::Open, menu(tr(k::track::MENU_PROPS))),
        ],
        E::MorphLane => vec![
            (Verb::Create, menu(tr(k::tl::lane::ADD_MORPHER_HERE))),
            (Verb::Rename, menu(tr(k::tl::lane::RENAME))),
            (Verb::Delete, menu(tr(k::tl::lane::DELETE))),
            (Verb::Open, menu(tr(k::track::MENU_PROPS))),
        ],
        E::CameraLane => vec![
            (Verb::Create, menu(tr(k::tl::lane::ADD_CAMERA_HERE))),
            (Verb::Rename, menu(tr(k::tl::lane::RENAME))),
            (Verb::Delete, menu(tr(k::tl::lane::DELETE))),
            (Verb::Open, menu(tr(k::track::MENU_PROPS))),
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
pub(crate) struct PaintedText {
    pub src: String,
    pub drawn: String,
    pub rows: usize,
    pub elided: bool,
    pub rect: egui::Rect,
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

/// Нарисованные ЛИНИИ (Р.67): ломаные и отрезки из списка форм кадра —
/// то, что видит человек на графике; утверждения про рисунок кривой
/// делаются по ним, а не по модели (урок t698).
pub(crate) fn painted_lines(h: &egui_kittest::Harness<'_, crate::app::KtApp>) -> Vec<Vec<egui::Pos2>> {
    fn walk(shape: &egui::epaint::Shape, out: &mut Vec<Vec<egui::Pos2>>) {
        use egui::epaint::Shape;
        match shape {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::LineSegment { points, .. } => out.push(points.to_vec()),
            Shape::Path(p) => out.push(p.points.clone()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in &h.output().shapes {
        walk(&cs.shape, &mut out);
    }
    out
}

pub(crate) fn painted_texts(h: &egui_kittest::Harness<'_, crate::app::KtApp>) -> Vec<PaintedText> {
    let mut out = Vec::new();
    for cs in &h.output().shapes {
        collect_texts(&cs.shape, &mut out);
    }
    out
}

/// Галерея пикера на клипе заданного вида: открыть окно FX, раскрыть
/// пикер и (по желанию) отфильтровать поиском — галерея из 27 строк в
/// прокрутку целиком не влезает, и нужная строка может быть НИЖЕ сгиба.
fn picker_harness(
    kind: kt_model::ClipKind,
    query: Option<&str>,
) -> egui_kittest::Harness<'static, crate::app::KtApp> {
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let mut h = crate::layout_tests::harness();
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
        kind,
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
    app.open_fx_editor(ClipId(1));
    h.step();
    h.step();
    let add = crate::i18n::tr(crate::i18n::k::fxreg::ADD).to_string();
    h.get_by_label(&add).click();
    h.step();
    h.step();
    if let Some(q) = query {
        h.event(egui::Event::Text(q.into()));
        h.step();
        h.step();
    }
    h
}

/// Тест 551 (Р.54, п.433): ОСЬ СИЛЫ у ячейки.
///
/// Утверждается то, что делает человек: нажал на ячейке, повёл вбок,
/// отпустил — и на событии стоит НЕ названный вариант, а то, что он
/// видел под курсором. И обратное, не менее важное: нажал и отпустил не
/// двигаясь — стоит ровно названный вариант (правило t254, иначе «Плотно»
/// перестало бы быть достижимым).
#[test]
fn t551_strength_axis_lands_between_variants() {
    use egui_kittest::kittest::Queryable;

    // язык ЯВНО и ДО чтения имён: `tr` живёт в TLS потока, а харнесс
    // ставит язык только внутри себя — подпись посчиталась бы английской,
    // а искалась бы русская
    crate::i18n::set_lang(crate::i18n::Lang::Ru);
    let e =
        crate::fx_registry::registry().iter().find(|e| e.name == "Blur").expect("Blur в реестре");
    let soft = e.variants[0].fx.get_param("radius").unwrap();
    let heavy = e.variants[1].fx.get_param("radius").unwrap();
    let label_heavy = format!("{} {}", e.name, crate::i18n::tr(e.variants[1].name));

    // --- (а) клик без движения даёт ИМЕННО вариант -------------------
    let mut h = picker_harness(kt_model::ClipKind::Video, Some("blur"));
    h.get_by_label(&label_heavy).click();
    h.step();
    h.step();
    let fx = h.state().model.project().clips[0].fx.clone();
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0].get_param("radius"), Some(heavy), "клик перестал давать сам вариант");

    // --- (б) проводка ВЛЕВО даёт смесь с соседом слева ---------------
    // соседняя точка ряда слева от «Плотно» — «Мягко»; ведём на половину
    // ячейки, то есть примерно посередине между ними
    let mut h = picker_harness(kt_model::ClipKind::Video, Some("blur"));
    let cell = h.get_by_label(&label_heavy).rect();
    let c = cell.center();
    h.drag_at(c);
    h.step();
    // порог драга egui: сначала маленький сдвиг, потом до цели (та же
    // трёхшаговая идиома, что у всех драг-тестов репозитория)
    h.hover_at(egui::pos2(c.x - 8.0, c.y));
    h.step();
    let target = egui::pos2(c.x - crate::fx_registry::CELL_DRAW_W / 2.0, c.y);
    h.hover_at(target);
    h.step();
    h.drop_at(target);
    h.step();
    h.step();

    let fx = h.state().model.project().clips[0].fx.clone();
    assert_eq!(fx.len(), 1, "проводка не добавила эффект");
    let got = fx[0].get_param("radius").expect("radius");
    assert!(
        got > soft && got < heavy,
        "ось не сработала: radius {got} обязан лежать МЕЖДУ {soft} и {heavy}"
    );
    // ось квантуется, а не «сколько пикселей — столько значений»
    let (j, tt) = crate::fx_registry::axis_at(1, -crate::fx_registry::CELL_DRAW_W / 2.0, 4);
    assert_eq!((j, tt), (1, 0.5), "середина между соседями считается не так");
}

/// Чистая арифметика оси: края упираются, центр ячейки — сам вариант.
#[test]
fn t551b_axis_math_clamps_and_snaps() {
    use crate::fx_registry::{CELL_DRAW_W, axis_at};
    // ряд: [шаблон, v0, v1, v2] — четыре точки, ячейка i стоит в i + 1
    let n = 4;
    // центр своей ячейки = сама точка (доля 0 у неё же)
    assert_eq!(axis_at(0, 0.0, n), (1, 0.0));
    assert_eq!(axis_at(1, 0.0, n), (2, 0.0));
    // мелкое дрожание руки не сдвигает значение (квант оси)
    assert_eq!(axis_at(1, 3.0, n), (2, 0.0));
    assert_eq!(axis_at(1, -3.0, n), (2, 0.0));
    // ячейка влево — предыдущая точка ряда
    assert_eq!(axis_at(1, -CELL_DRAW_W, n), (1, 0.0));
    // левее ШАБЛОНА не уедешь: ряд упирается
    assert_eq!(axis_at(0, -10.0 * CELL_DRAW_W, n), (0, 0.0));
    // и правее последней точки тоже
    assert_eq!(axis_at(2, 10.0 * CELL_DRAW_W, n), (n - 2, 1.0));
    // половина ячейки — ровно половина доли
    assert_eq!(axis_at(0, CELL_DRAW_W / 2.0, n), (1, 0.5));
}

/// Тест 548 (Р.54, п.431): вёрстка галереи.
///
/// Четыре утверждения, и все — про то, что ВИДНО: подпись ячейки усечена
/// (а не перенесена и не вылезла), заголовок категории печатается ОДИН
/// раз, ширина галереи не зависит от набранного в поиске, а в аудио-пикере
/// ячеек нет вовсе.
#[test]
fn t548_gallery_layout_is_stable_and_elided() {
    use egui_kittest::kittest::Queryable;

    // ---- (а) подпись ячейки усекается, а не разносит её ---------------
    let h = picker_harness(kt_model::ClipKind::Video, Some("aberration"));
    let texts = painted_texts(&h);
    let cells: Vec<&PaintedText> = texts
        .iter()
        .filter(|t| {
            crate::fx_registry::registry()
                .iter()
                .flat_map(|e| e.variants.iter())
                .any(|v| crate::i18n::tr(v.name) == t.src)
        })
        .collect();
    assert!(!cells.is_empty(), "подписей ячеек не нарисовано вовсе");
    for c in &cells {
        assert_eq!(c.rows, 1, "подпись «{}» перенеслась на {} строк", c.src, c.rows);
        assert!(
            c.rect.width() <= crate::fx_registry::CELL_DRAW_W + 1.0,
            "подпись «{}» шире ячейки: {} px",
            c.src,
            c.rect.width()
        );
    }

    // ---- (б) заголовок категории — ОДИН раз --------------------------
    // Реестр отсортирован по смыслу, и категории в нём повторяются:
    // «Стилизация» печаталась пять раз, «Размытие» — три. Группировка на
    // отрисовке обязана это схлопнуть.
    let h2 = picker_harness(kt_model::ClipKind::Video, None);
    for cat in [
        crate::i18n::k::fxreg::cat::STYLE,
        crate::i18n::k::fxreg::cat::BLUR,
        crate::i18n::k::fxreg::cat::THREE_D,
        crate::i18n::k::fxreg::cat::GEOMETRY,
    ] {
        let name = crate::i18n::tr(cat);
        let n = painted_texts(&h2).iter().filter(|t| t.src == name).count();
        assert!(n <= 1, "заголовок категории «{name}» напечатан {n} раз");
    }

    // ---- (в) содержимое галереи не вылезает и не скачет --------------
    // «Ничего не скачет» (правило 3) и «текст помещается» (правило 7):
    // строка ячеек обязана ЗАВОРАЧИВАТЬСЯ внутри объявленной ширины, а
    // набор в поиске не имеет права двигать галерею.
    let content_box = |h: &egui_kittest::Harness<'_, crate::app::KtApp>| -> egui::Rect {
        let mut r = egui::Rect::NOTHING;
        for e in crate::fx_registry::registry().iter().filter(|e| e.video) {
            if let Some(n) = h.query_by_label(e.name) {
                r = r.union(n.rect());
            }
            for v in &e.variants {
                let l = format!("{} {}", e.name, crate::i18n::tr(v.name));
                if let Some(n) = h.query_by_label(&l) {
                    r = r.union(n.rect());
                }
            }
        }
        r
    };
    let wide = content_box(&h2);
    let narrow = content_box(&h);
    assert!(
        wide.width() <= crate::fx_registry::GALLERY_W,
        "содержимое галереи шире объявленной ширины: {} px против {}",
        wide.width(),
        crate::fx_registry::GALLERY_W
    );
    assert!(
        (wide.left() - narrow.left()).abs() <= 1.0,
        "галерея съехала от запроса: левый край {} против {}",
        wide.left(),
        narrow.left()
    );

    // ---- (г) в аудио-пикере картинок нет ----------------------------
    let ha = picker_harness(kt_model::ClipKind::Audio, None);
    assert!(ha.query_by_label("Gain").is_some(), "аудио-список не нарисовался");
    for l in crate::fx_registry::registry()
        .iter()
        .flat_map(|e| e.variants.iter().map(|v| format!("{} {}", e.name, crate::i18n::tr(v.name))))
    {
        assert!(
            ha.query_by_label(&l).is_none(),
            "в аудио-пикере нашлась ячейка «{l}»: миниатюру звука не нарисовать"
        );
    }
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), format!("/tmp/{file}")));
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

/// Тест 554 (Р.55, С1): что играет плеер — решается в ОДНОМ месте.
///
/// t456b сторожит, чтобы наружу не уходил рабочий вид. Этого мало: до
/// Р.55 восемь мест писали `set_project(model.document())` каждое от себя,
/// и «внутри ленты играет лента» держалось бы ровно до первой правки —
/// `apply_batch` тут же вернул бы в плеер корень, и дефект воскрес бы
/// незаметно, потому что видимого следа у него нет (звук просто не тот).
///
/// Поэтому прямые формы запрещены совсем: источник того, что играет, —
/// `KtApp::player_project()`, и он один.
#[test]
fn t554_only_one_place_decides_what_plays() {
    let mut bad: Vec<String> = Vec::new();
    for (file, src) in production_corpus() {
        if !file.contains("kt-app") {
            continue;
        }
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            let direct = t.contains("set_project(self.model.document())")
                || t.contains("set_project(self.model.reel_document())")
                || t.contains("set_project(self.model.project())");
            if direct {
                bad.push(format!("{file}:{}: {t}", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "плеер получает проект мимо `player_project()`. Стоя внутри ленты, играть \
         обязана ЛЕНТА, и решаться это обязано в одном месте — иначе первая же \
         правка вернёт в плеер корень, а услышит это только пользователь:\n  {}",
        bad.join("\n  ")
    );
    // самопроверка скана: сама точка решения обязана существовать и
    // быть ровно одна (иначе тест зелен на пустом множестве)
    let src = std::fs::read_to_string(workspace_root().join("apps/kt-app/src/app.rs"))
        .expect("нет app.rs");
    assert_eq!(
        src.matches("fn player_project(").count(),
        1,
        "точка решения «что играет» либо исчезла, либо размножилась — \
         запрет выше перестал что-либо значить"
    );
}

/// Тест 560 (Р.55, С6): генератор создаётся НЕ ТОЛЬКО из бина.
///
/// **Претензия владельца дословно:** «я не знаю как создать генератор
/// медиа, у нас код есть а как создавать в ui не понятно.. наверное еще
/// куча вещей недоступна из ui».
///
/// И правда: вход был ровно один — правый клик по ПУСТОМУ фону бина.
/// Ни главного меню, ни клавиши, ни меню таймлайна. t253 это не ловил:
/// он спрашивает, ДОСТИЖИМ ли глагол, а не сколько входов и насколько они
/// заметны.
///
/// Тест утверждает две вещи сразу: пункт есть в главном меню И список
/// видов у обоих входов ОДИН (правило 17) — второго списка не завелось.
#[test]
fn t560_generator_is_reachable_from_the_main_menu() {
    let root = workspace_root();
    let toolbar =
        std::fs::read_to_string(root.join("apps/kt-app/src/toolbar.rs")).expect("нет toolbar.rs");
    assert!(
        toolbar.contains("gen_ui::generator_menu(ui)"),
        "в главном меню нет пункта «Создать генератор…» — единственным входом \
         остался правый клик по пустому фону бина, а его человек не нашёл"
    );
    let bin = std::fs::read_to_string(root.join("apps/kt-app/src/bin_panel.rs"))
        .expect("нет bin_panel.rs");
    assert!(
        bin.contains("gen_ui::generator_menu(ui)"),
        "меню бина строит свой список видов — разойдись они, новый вид появился бы \
         в одном входе и не появился в другом (правило 17)"
    );
    // МЕНЮ строит список видов ровно в одном файле. Своего перебора у
    // входов быть не должно: новый вид попал бы в одно меню и не попал в
    // другое (`GeneratorKind::all()` законно живёт ещё в окне генератора —
    // там это пикер вида и замер ширины, а не меню).
    for (name, src) in [("toolbar.rs", &toolbar), ("bin_panel.rs", &bin)] {
        assert!(
            !src.contains("GeneratorKind::all()"),
            "{name} перебирает виды генератора сам — список видов обязан быть \
             один на все входы (правило 17)"
        );
    }
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
                    covered.contains(&&*spec.name),
                    "{who}.{}: компонента цвета не попала ни в одну пипетку — \
                     пользователь увидит сырое имя поля",
                    spec.name
                );
                continue;
            }
            let label = crate::widgets::param_label_scoped(scope, &spec.name);
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
        let mut covered: Vec<&str> = crate::gen_ui::color_groups(&k)
            .iter()
            .flat_map(|(_, names)| names.iter().copied())
            .collect();
        // четвёртая компонента (альфа фигур, Р.53 п.419) закрыта той же
        // пипеткой — окно передаёт её в `color_row` отдельным параметром
        covered.extend(crate::gen_ui::color_alpha(&k));
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

/// Тест 602 (Р.56, п.452): рендер ленты вне плеера живёт в ОДНОМ месте.
///
/// Офлайн-путь имеет право на свои источники ровно при трёх условиях
/// (инвариант 5): он ими ни с кем не делится, темпа не обещает, задача
/// конечна и отменяема. Проверить это у ОДНОГО места можно чтением; у
/// пяти — уже нет. Поэтому `VideoRenderer`/`AudioRenderer` в kt-app
/// заводит либо рендер-диалог (экспорт), либо служба превью ленты, и
/// больше никто.
#[test]
fn t602_reel_rendering_lives_in_one_place() {
    let mut bad: Vec<String> = Vec::new();
    for (file, src) in production_corpus() {
        if !file.contains("kt-app")
            || file.contains("render_dialog.rs")
            || file.contains("reel_preview.rs")
        {
            continue;
        }
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("//") {
                continue;
            }
            let renders = t.contains("VideoRenderer::new")
                || t.contains("AudioRenderer::new")
                || t.contains("render_raw_video_range")
                || t.contains("render_raw_audio_range");
            if renders {
                bad.push(format!("{file}:{}: {t}", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "офлайн-рендер завёлся мимо службы превью ленты. Свои источники, \
         свой поток и своя отмена — это форма, за которой надо следить в ОДНОМ \
         месте, иначе следующий такой путь отнимет диск у воспроизведения:\n  {}",
        bad.join("\n  ")
    );
    // самопроверка скана: само место обязано существовать и быть одним
    let src = std::fs::read_to_string(workspace_root().join("apps/kt-app/src/reel_preview.rs"))
        .expect("нет reel_preview.rs");
    assert_eq!(
        src.matches("pub fn render_reel_preview(").count(),
        1,
        "тело офлайн-рендера ленты либо исчезло, либо размножилось — \
         запрет выше перестал что-либо значить"
    );
}

/// Тест 637c (Р.58, п.462): ПОЛ ВРЕМЕНИ у жестов — одно правило на файл.
///
/// В `timeline.rs` ПОЗИЦИЯ ВО ВРЕМЕНИ голым `.max(0)` не клампится: у
/// жестов над СОБЫТИЯМИ пол — `self.time_floor()` (внутри ленты ноль не
/// стена), у курсора/региона/меток/областей — `CURSOR_FLOOR` по имени
/// (ноль всегда, решение владельца). Литерал `0` в этом месте — это
/// второе правило, записанное числом, и ровно так до Р.58 внутри ленты
/// «нельзя было закинуть событие за её пределы». Нулём законно клампятся
/// только НЕ-позиции: индекс строки, ДЛИНА (фейд, диапазон метки) и
/// смещение в материале (`source_in`).
#[test]
fn t637c_gesture_time_floor_is_never_a_bare_zero() {
    let root = workspace_root();
    let rel = "apps/kt-app/src/timeline.rs";
    let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
    // что нулём клампить можно: строки, длины, материал
    const NOT_A_POSITION: [&str; 5] =
        ["row_at_y", "len.max(0)", "- start).max(0)", "- m.t.0).max(0)", "src_in"];
    let bad: Vec<String> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains(".max(0)") && !NOT_A_POSITION.iter().any(|p| l.contains(p)))
        .map(|(n, l)| format!("{rel}:{}: {}", n + 1, l.trim()))
        .collect();
    assert!(
        bad.is_empty(),
        "a bare `.max(0)` over a time position: use `self.time_floor()` for events or \
         `CURSOR_FLOOR` for the cursor/region/markers:\n  {}",
        bad.join("\n  ")
    );
    // и оба правила в файле действительно есть
    assert!(src.contains("time_floor()"), "events lost their time floor");
    assert!(src.contains("CURSOR_FLOOR"), "the cursor floor lost its name");
}

/// Тест 669 (Р.59, стадия 2): картинки генератора берутся по АДРЕСУ
/// СОБЫТИЯ, а вид без картинки называет себя.
///
/// Значения переехали на событие, и у медиа их больше нет: спроси
/// филмстрип `c.media` — и он покажет умолчания вида, то есть чёрный
/// прямоугольник вместо набранного титра. Правило одно на программу —
/// `kt_engine::clip_source_addr`, и оно же держит инвариант 4 («заказ ==
/// чтение»).
///
/// Вторая половина — правило 16: у карточки бина вид с умолчаниями
/// законно даёт ПУСТОЙ кадр (у титра умолчание — пустая строка), и
/// молчать об этом нельзя: карточка пишет имя вида прямо в тумбе.
#[test]
fn t669_generator_thumbs_follow_the_event_address() {
    use kt_model::GeneratorKind;

    // 1) предикат «кадра не будет» честен
    assert!(crate::gen_ui::blank_default(&GeneratorKind::default_text()));
    assert!(crate::gen_ui::blank_default(&GeneratorKind::Transparent));
    assert!(!crate::gen_ui::blank_default(&GeneratorKind::Solid {
        color_r: 255,
        color_g: 136,
        color_b: 0
    }));

    // 2) филмстрип и полоска прогретости спрашивают АДРЕС СОБЫТИЯ
    let root = workspace_root();
    for (rel, what) in [
        ("apps/kt-app/src/timeline.rs", "филмстрип"),
        ("apps/kt-app/src/app.rs", "полоска прогретости"),
        ("apps/kt-app/src/fx_editor.rs", "кадр-основа галереи"),
    ] {
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        assert!(
            src.contains("clip_source_addr"),
            "{what} ({rel}) спрашивает медиа события, а не его ИСТОЧНИК: \
             у генератора значения живут на событии, и медиа знает только вид"
        );
    }

    // 3) источник тумб события поднят по этому адресу и знает ЗНАЧЕНИЯ
    let (mut h, _g) = crate::layout_tests::harness_three_clips_dt(Some(0.02));
    let made = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    h.state_mut().create_generator(made.clone());
    h.step();
    let (media, clip) = crate::layout_tests::made_generator(&h);
    let addr = kt_engine::gen_source_addr(clip);
    assert_ne!(addr, media, "адрес источника события обязан отличаться от адреса медиа");
    let _ = h.state().thumbs.duration_or_probe(addr);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some((v, a)) = h.state().thumbs.kinds(addr) {
            assert!(v && !a, "источник события генератора — видео без аудио");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "тумб-источник события так и не поднялся: филмстрип генератора останется пустым"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
        h.step();
    }
    assert_eq!(
        h.state().gen_states.get(&clip).map(|s| s.kind.clone()),
        Some(made),
        "снимок источника обязан нести ЗНАЧЕНИЯ события"
    );

    // 4) карточка пустого вида пишет ИМЯ ВИДА прямо в тумбе
    h.state_mut().create_generator(GeneratorKind::default_text());
    for _ in 0..4 {
        h.step();
    }
    let text_media = h.state().model.project().media.last().unwrap().id;
    let card = h.state().bin_card_rect(text_media).expect("карточка титра не нарисована");
    let want = crate::gen_ui::kind_name(&GeneratorKind::default_text());
    let inside =
        painted_texts(&h).into_iter().any(|t| t.src == want && card.contains(t.rect.center()));
    assert!(
        inside,
        "карточка вида без кадра осталась ЧЁРНЫМ прямоугольником — \
         невозможность показать картинку обязана быть видна (правило 16)"
    );
}

/// Тест 682 (Р.59, стадия 3): дверь публикации ОДНА и она достижима.
///
/// Утверждаются три следствия, каждое — из точки, откуда действует
/// человек: (1) правый клик по подписи параметра ВНУТРИ ленты предлагает
/// «Опубликовать…», а по уже объявленной — «Снять публикацию»; и так
/// и при полном входе, и в раскрытой полосе; (2) вне ленты этого пункта
/// НЕТ ВОВСЕ (правило 9), а строка состояния объясняет почему (правило
/// 16); (3) меню экземпляра достаёт до ОБОИХ окон — потребительского и
/// авторского.
#[test]
fn t682_publishing_is_reachable_from_inside_a_reel_only() {
    use crate::i18n::{k, tr};
    use crate::layout_tests::harness_news_template;
    use egui_kittest::kittest::Queryable;
    use kt_model::*;

    // подпись параметра — ВЕРХНЯЯ из одноимённых: то же имя стоит ещё и
    // кнопкой выбора кривой ниже (урок t281)
    let label_top =
        |h: &mut egui_kittest::Harness<'_, crate::app::KtApp>, label: &str| -> Option<egui::Rect> {
            h.query_all_by_label_contains(label)
                .map(|n| n.rect())
                .min_by(|a, b| a.top().total_cmp(&b.top()))
        };
    // ЯЗЫК ставит харнесс, поэтому подписи спрашиваются ПОСЛЕ его
    // создания: собранная раньше строка пришла бы на языке по умолчанию,
    // и тест искал бы в русском окне английское слово.
    let labels = || {
        (
            tr(k::pubp::PUBLISH).to_string(),
            tr(k::pubp::UNPUBLISH).to_string(),
            // объявленный ряд и НЕобъявленный: у первого дверь ведёт
            // наружу, у второго — внутрь
            crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "size"),
            crate::widgets::param_label_scoped(kt_model::ParamScope::Generator, "line_spacing"),
        )
    };

    // ---- 1а) ПОЛНЫЙ ВХОД ---------------------------------------------
    {
        let (mut h, s) = harness_news_template();
        let (publish, unpublish, published_row, free_row) = labels();
        h.state_mut().enter_nest(s.a);
        h.step();
        h.state_mut().open_fx_editor_head(s.inner);
        h.step();
        h.step();
        let rect = label_top(&mut h, &free_row).expect("подписи необъявленного ряда нет");
        let before = h.query_all_by_label_contains(&publish).count();
        right_click(&mut h, rect.center());
        assert!(
            h.query_all_by_label_contains(&publish).count() > before,
            "внутри ленты правый клик по подписи не предложил публикацию"
        );
        let _ = (&unpublish, &published_row);
    }

    // ---- 1в) уже объявленный ряд предлагает СНЯТИЕ, той же дверью ----
    // Свой харнесс: закрывать меню Esc'ом нельзя — он каскадом гасит и
    // само окно, и следующая проверка щупала бы пустой экран.
    {
        let (mut h, s) = harness_news_template();
        let (_pub, unpublish, published_row, _free) = labels();
        h.state_mut().enter_nest(s.a);
        h.step();
        h.state_mut().open_fx_editor_head(s.inner);
        h.step();
        h.step();
        let rect = label_top(&mut h, &published_row).expect("подписи объявленного ряда нет");
        let before = h.query_all_by_label_contains(&unpublish).count();
        right_click(&mut h, rect.center());
        assert!(
            h.query_all_by_label_contains(&unpublish).count() > before,
            "у объявленной ручки нет обратного действия — она была бы ловушкой"
        );
    }

    // ---- 1б) РАСКРЫТАЯ ПОЛОСА ----------------------------------------
    {
        let (mut h, s) = harness_news_template();
        let (publish, _un, _pub_row, free_row) = labels();
        h.state_mut().expand_in_place(s.a);
        h.step();
        h.step();
        assert!(!h.state().model.bands().is_empty(), "полоса не раскрылась — сцена не та");
        h.state_mut().open_fx_editor_head(s.inner);
        h.step();
        h.step();
        let rect = label_top(&mut h, &free_row).expect("подписи в полосе нет");
        let before = h.query_all_by_label_contains(&publish).count();
        right_click(&mut h, rect.center());
        assert!(
            h.query_all_by_label_contains(&publish).count() > before,
            "в раскрытой полосе публикация недостижима, хотя содержимое то же самое"
        );
    }

    // ---- 2) ВНЕ ЛЕНТЫ пункта нет -------------------------------------
    {
        let (mut h, _s) = harness_news_template();
        let (publish, _un, _pr, _fr) = labels();
        let root = {
            let doc = h.state().model.document();
            h.state()
                .model
                .project()
                .clips
                .iter()
                .find(|c| c.r#gen.is_none() && doc.sequence(c.media).is_none())
                .map(|c| c.id)
                .expect("корневое событие из файла")
        };
        assert!(
            h.state_mut().apply(Command::SetFx {
                id: root,
                fx: vec![FxInstance::Gain { db: 0.0, pan: 0.0 }],
            })
        );
        h.step();
        h.state_mut().open_fx_editor(root);
        h.step();
        h.step();
        let db = crate::widgets::param_label("db");
        let rect = label_top(&mut h, &db).expect("подписи усиления в окне нет");
        // СНАЧАЛА подсказка (меню бы её накрыло): молчанием отсутствие
        // пункта не остаётся — строка состояния называет причину
        h.hover_at(rect.center());
        h.step();
        h.step();
        let hint = h.state().hover_hint.clone().unwrap_or_default();
        assert!(
            hint.contains(tr(k::pubp::OUTSIDE_REEL)),
            "подсказка не объяснила, где живёт публикация: {hint:?}"
        );
        let before = h.query_all_by_label_contains(&publish).count();
        right_click(&mut h, rect.center());
        assert_eq!(
            h.query_all_by_label_contains(&publish).count(),
            before,
            "вне ленты пункт публикации появился — это мёртвый контрол (правило 9)"
        );
    }

    // ---- 3) меню экземпляра достаёт до ОБОИХ окон --------------------
    {
        let (mut h, s) = harness_news_template();
        h.step();
        let g = h.state().tl_geom.clone().expect("геометрия таймлайна");
        let c = h.state().model.clip(s.b).cloned().expect("второй выпуск");
        let pos = egui::pos2(
            g.tick_to_x((c.start + kt_time::Tick(c.duration.0 / 2)).0),
            g.lane_y(kt_model::ClipKind::Video, c.lane, 0.5),
        );
        right_click(&mut h, pos);
        let labels: Vec<String> = h
            .query_all_by_label_contains("")
            .filter_map(|n| {
                use egui_kittest::kittest::NodeT;
                n.accesskit_node().label().map(|l| l.to_string())
            })
            .collect();
        for want in [tr(k::pubp::INST_OPEN), tr(k::pubp::LIST_OPEN)] {
            assert!(
                labels.iter().any(|l| l.contains(want)),
                "меню экземпляра не ведёт к «{want}»: {labels:?}"
            );
        }
    }
}

/// Тест 693 (Р.60): кривые ЛЮБОГО носителя показывает ОДИН виджет.
///
/// Решение Р.34 — «один механизм анимации на всё». Окно параметров
/// композиции его обошло: завело собственную запись ключей (поле писало
/// ключ по курсору) и не завело ни графика, ни таблицы. Владелец: «кривых
/// автоматизаций я нигде не вижу… ни одной кнопки в окне про автоматизацию
/// нет».
///
/// Сторож двусторонний. Сверху — по КОДУ: примитивы записи в кривую
/// (`bound_apply`/`bound_value`) живут только там, где живёт сам виджет;
/// снизу — по СЛЕДСТВИЮ: у каждого окна, правящего значение по времени,
/// есть вызов виджета, а у его контролов — подсказка в строке состояния
/// (правило 10: всплывающих у нас нет).
#[test]
fn t693_every_carrier_draws_its_curves_with_the_one_widget() {
    let root = workspace_root();

    // --- 1. примитивы кривой не зовутся мимо виджета ------------------
    const HOME: [&str; 2] = ["apps/kt-app/src/widgets.rs", "apps/kt-app/src/automation.rs"];
    let mut files: Vec<String> = Vec::new();
    for dir in ["libs", "apps"] {
        collect_rs(&root.join(dir), &mut files);
    }
    let mut bad: Vec<String> = Vec::new();
    for rel in &files {
        if HOME.contains(&rel.as_str()) || rel.contains("/tests/") || rel.ends_with("_tests.rs") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            // `CurveBind` — законная дверь: она и есть «поле, связанное с
            // кривой», и живёт в том же доме, что виджет
            if (t.contains("bound_apply(") || t.contains("bound_value("))
                && !t.contains("bind.")
                && !t.contains("fn bound_")
            {
                bad.push(format!("{rel}:{}: {t}", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "запись в кривую мимо единственного виджета ({}): так и появился \
         невидимый редактор ключей в окне параметров композиции:\n  {}",
        HOME.join(", "),
        bad.join("\n  ")
    );

    // --- 2. у каждого носителя есть вызов виджета ---------------------
    for (rel, who) in [
        ("apps/kt-app/src/fx_editor.rs", "слот эффекта"),
        ("apps/kt-app/src/gen_ui.rs", "генератор на событии"),
        ("apps/kt-app/src/camera_editor.rs", "камера"),
        ("apps/kt-app/src/pubparam.rs", "публичный параметр композиции"),
    ] {
        let src = std::fs::read_to_string(root.join(rel)).expect(rel);
        assert!(
            src.contains("automation::draw("),
            "{who} ({rel}) правит значение по времени, но виджета автоматизации не зовёт — \
             значит, у него завёлся свой редактор ключей"
        );
    }

    // --- 3. следствие: контрол назван и объясняет себя ----------------
    use crate::i18n::{k, tr};
    use crate::layout_tests::harness_news_template;
    let (mut h, s) = harness_news_template();
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    let add = {
        use egui_kittest::kittest::Queryable;
        h.query_all_by_label(tr(k::fx::AUTO_ADD))
            .map(|n| n.rect())
            .next()
            .expect("в окне параметров композиции нет НАЗВАННОГО действия «завести кривую»")
    };
    h.hover_at(add.center());
    h.step();
    h.step();
    let hint = h.state().hover_hint.clone();
    assert_eq!(
        hint.as_deref(),
        Some(tr(k::pubp::AUTO_ADD_HINT)),
        "у действия «завести кривую» нет подсказки в строке состояния (правило 10): {hint:?}"
    );
}

/// Тест 699b (Р.61): состояния буфера обмена внутри программы НЕ БЫВАЕТ.
///
/// Владелец опознал дефект по косвенному признаку: «судя по тому что кнопка
/// может быть "disabled" — в коде есть какое-то состояние буфера обмена —
/// что плохо». Он был прав: копирование писало и в системный буфер, и в
/// зеркало `env_clipboard`, а доступность кнопки решалась ПО ЗЕРКАЛУ —
/// скопированное во втором окне программы вставить было нельзя.
///
/// Сторож двусторонний: по КОДУ — зеркала нет и кнопка вставки не гасится;
/// по СЛЕДСТВИЮ — разбор буфера живёт в одном месте и зовётся из фона.
#[test]
fn t699b_no_clipboard_state_lives_inside_the_program() {
    let root = workspace_root();
    let mut files: Vec<String> = Vec::new();
    collect_rs(&root.join("apps"), &mut files);

    // --- 1. зеркала буфера огибающих не осталось ----------------------
    let mut bad: Vec<String> = Vec::new();
    for rel in &files {
        if rel.ends_with("_tests.rs") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        for (n, line) in src.lines().enumerate() {
            // ищем ПОЛЕ, а не любое вхождение строки: `poll_env_clipboard`
            // — это как раз чтение системного буфера, оно законно
            if line.contains(".env_clipboard") || line.contains("env_clipboard:") {
                bad.push(format!("{rel}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "зеркало буфера огибающих вернулось — кнопка снова будет гаснуть на \
         содержимом, скопированном в другом окне:\n  {}",
        bad.join("\n  ")
    );

    // --- 2. кнопка вставки не гасится ---------------------------------
    for rel in &files {
        if rel.ends_with("_tests.rs") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        for (n, line) in src.lines().enumerate() {
            let t = line.trim();
            assert!(
                !(t.contains("add_enabled") && t.contains("PASTE_ENV")),
                "{rel}:{}: кнопка вставки огибающей снова гаснет — содержимое \
                 буфера обязано читаться ПО НАЖАТИЮ, а не решать доступность",
                n + 1
            );
        }
    }

    // --- 3. системный буфер трогает РОВНО одно место ------------------
    const HOME: &str = "apps/kt-app/src/envclip.rs";
    let mut readers: Vec<String> = Vec::new();
    for rel in &files {
        let src = strip_comments(&std::fs::read_to_string(root.join(rel)).expect(rel));
        if src.contains("arboard::") && rel != HOME {
            readers.push(rel.clone());
        }
    }
    assert!(
        readers.is_empty(),
        "системный буфер читается мимо единственного дома ({HOME}): {readers:?}"
    );
    let home = std::fs::read_to_string(root.join(HOME)).expect(HOME);
    assert!(
        home.contains("BgJob::spawn"),
        "чтение буфера перестало быть фоновой задачей: на X11 буфером владеет \
         окно, и синхронный запрос к самим себе повиснет в цикле событий"
    );
}

/// Тест 720 (Р.62): окно настроек проекта достижимо из меню «Файл» — из
/// точки, откуда действует человек, а не вызовом функции.
#[test]
fn t720_project_settings_reachable_from_the_file_menu() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    let mut h = crate::layout_tests::harness();
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();
    h.step();
    let labels = crate::layout_tests::menu_labels(&mut h);
    assert!(
        h.query_by_label(tr(k::menu::file::PROJECT_SETTINGS)).is_some(),
        "пункт «Настройки проекта…» обязан быть в меню «Файл»; видно: {labels:?}"
    );
    h.get_by_label(tr(k::menu::file::PROJECT_SETTINGS)).click();
    h.step();
    h.step();
    assert!(h.state().settings_win.is_some(), "пункт меню открывает окно");
    assert!(h.query_by_label(tr(k::settings::TITLE)).is_some());
    assert!(h.query_by_label(tr(k::settings::APPLY)).is_some());
}

/// Тест 741 (Р.62, change B, 4.5): у ДОРОЖКИ полный набор глаголов, и
/// парные пути одного действия — одно тело.
///
/// Два утверждения, которых не делают ни t253 (он про сущности и их
/// меню), ни t742 (он про бейджи):
///
/// 1. Глушение, соло и цепочка есть ровно там, где им есть что делать.
///    У служебной строки их нет ни бейджем, ни пунктом меню: мёртвый
///    контрол — нарушение правила 9, и «в меню-то он есть» тут не ответ.
/// 2. Бейдж, пункт меню и горячая клавиша дают ОДИН И ТОТ ЖЕ результат и
///    один шаг истории. Разойдись они — «заглушил из меню» и «заглушил
///    бейджем» вели бы себя по-разному на одной дорожке (правило 17).
#[test]
fn t741_track_has_every_verb_and_paired_paths_are_one_body() {
    use crate::timeline::{GUTTER_W, TrackBadge};
    use egui_kittest::kittest::Queryable;
    use kt_model::LaneKind::{Audio, Camera, Morph, Video};

    let scene = || {
        let (mut h, _) = crate::layout_tests::harness_three_clips_dt(None);
        {
            let app = h.state_mut();
            app.model.apply(kt_model::Command::InsertTrack { kind: Morph, at: 0 }).unwrap();
            app.model.apply(kt_model::Command::InsertTrack { kind: Camera, at: 0 }).unwrap();
        }
        h.step();
        let g = h.state().tl_geom.clone().expect("геометрия таймлайна");
        (h, g)
    };

    // подписи берём ПОСЛЕ создания харнесса: язык каталога живёт в TLS
    // UI-потока, и посчитанные раньше строки пришли бы на другом языке
    let mut problems: Vec<String> = Vec::new();
    for kind in [Video, Audio, Morph, Camera] {
        let (mut h, g) = scene();
        let switches = [
            crate::i18n::tr(crate::i18n::k::track::MENU_MUTE),
            crate::i18n::tr(crate::i18n::k::track::MENU_SOLO),
            crate::i18n::tr(crate::i18n::k::track::MENU_FX),
        ];
        let always = [
            crate::i18n::tr(crate::i18n::k::track::MENU_PROPS),
            crate::i18n::tr(crate::i18n::k::track::MENU_LOCK),
        ];
        let row = g.rows.iter().position(|r| *r == (kind, 0)).expect("строка на экране");
        // жмём ПО ДОРОЖКЕ, а не в заголовок: человек тычет туда, где
        // видит строку (урок Р.40). Место — СВОБОДНОЕ: попади клик в
        // событие, откроется его меню, и проверялась бы не дорожка
        let pos = egui::pos2(g.tick_to_x(400_000), g.row_center_y(row));
        let count = |h: &egui_kittest::Harness<'_, crate::app::KtApp>, l: &str| {
            h.query_all_by_label_contains(l).count()
        };
        let before: Vec<usize> =
            always.iter().chain(switches.iter()).map(|l| count(&h, l)).collect();
        right_click(&mut h, pos);
        let after: Vec<usize> =
            always.iter().chain(switches.iter()).map(|l| count(&h, l)).collect();
        for (i, l) in always.iter().enumerate() {
            if after[i] <= before[i] {
                problems.push(format!(
                    "{kind:?}: пункта «{l}» нет в меню строки — свойствами дорожки \
                     управлять нечем"
                ));
            }
        }
        for (i, l) in switches.iter().enumerate() {
            let j = always.len() + i;
            let shown = after[j] > before[j];
            if kind.has_switches() && !shown {
                problems.push(format!("{kind:?}: пункта «{l}» нет, а глушить тут есть что"));
            }
            if !kind.has_switches() && shown {
                problems.push(format!(
                    "{kind:?}: пункт «{l}» на служебной строке — он ничего не делает, \
                     а мёртвых контролов не бывает (правило 9)"
                ));
            }
        }
        // и бейджи говорят ровно то же самое
        let badges = crate::timeline::row_badges(kind);
        let want = if kind.has_switches() { 4 } else { 1 };
        if badges.len() != want {
            problems.push(format!(
                "{kind:?}: бейджей {} вместо {want} — ряд и меню разошлись",
                badges.len()
            ));
        }
    }
    assert!(problems.is_empty(), "глаголы дорожки:\n  {}", problems.join("\n  "));

    // ---- 2. три пути — один результат и один шаг истории --------------------
    let mute_by = |what: &str| {
        let (mut h, g) = scene();
        let id = h.state().model.project().track_at(Video, 0).unwrap().id;
        let row = g.rows.iter().position(|r| *r == (Video, 0)).unwrap();
        let depth0 = h.state().model.undo_depth();
        match what {
            "badge" => {
                let r = TrackBadge::row_rects(
                    Video,
                    g.rect.left() - GUTTER_W,
                    g.row_top(row),
                    g.row_h(row),
                )
                .into_iter()
                .find(|(b, _)| *b == TrackBadge::Mute)
                .unwrap()
                .1;
                let pos = r.center();
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
            "menu" => {
                right_click(&mut h, egui::pos2(g.tick_to_x(400_000), g.row_center_y(row)));
                h.get_by_label(crate::i18n::tr(crate::i18n::k::track::MENU_MUTE)).click();
                h.step();
                h.step();
            }
            "key" => {
                {
                    let app = h.state_mut();
                    app.selected.clear();
                    app.selected.insert(kt_model::ClipId(1));
                }
                h.step();
                h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::M);
                h.step();
            }
            _ => unreachable!(),
        }
        let t = h.state().model.project().track(id).unwrap().clone();
        (t, h.state().model.undo_depth() - depth0)
    };
    let (badge, steps_b) = mute_by("badge");
    let (menu, steps_m) = mute_by("menu");
    let (key, steps_k) = mute_by("key");
    assert!(badge.mute, "бейдж не заглушил дорожку");
    assert_eq!(badge, menu, "бейдж и пункт меню дали РАЗНУЮ дорожку — тела разошлись");
    assert_eq!(badge, key, "бейдж и горячая клавиша дали РАЗНУЮ дорожку — тела разошлись");
    assert_eq!(
        (steps_b, steps_m, steps_k),
        (1, 1, 1),
        "глушение обязано быть одним шагом истории на любом пути"
    );
}

/// Тест 752b (Р.63 п.9): дверь публикации есть и у ВЕДУЩЕГО УЗЛА.
///
/// Владелец: «по какой причине у первого geometry нет возможности
/// опубликовать параметры а у второго есть? ведь этот первый geometry идет
/// как fx, разве нет? или у нас дубли кода?». Дублей нет — у ведущего узла
/// не было АДРЕСА публикации, и дверь была закрыта одним `!lead_sel`.
/// Утверждение — из точки человека: выбрал узел, правый клик по подписи
/// зума, пункт «Опубликовать…» есть (на прежнем коде его не было).
#[test]
fn t752b_publishing_is_reachable_at_the_lead_geometry() {
    use crate::i18n::{k, tr};
    use crate::layout_tests::harness_news_template;
    use egui_kittest::kittest::Queryable;

    let (mut h, s) = harness_news_template();
    let publish = tr(k::pubp::PUBLISH).to_string();
    let zoom = crate::widgets::param_label("zoom");
    h.state_mut().enter_nest(s.a);
    h.step();
    h.state_mut().open_fx_editor(s.inner);
    h.step();
    h.step();

    // выбираем ВЕДУЩИЙ узел — его подпись стоит в полосе цепочки
    let lead = crate::fx_editor::fx_display_name(&kt_model::EventGeometry::default().node());
    h.get_by_label(lead).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().fx_editors[0].selected.get(&s.inner),
        Some(&crate::widgets::ChainSel::Lead),
        "ведущий узел не выбран — тест щупал бы обычный слот"
    );

    let rect = h
        .query_all_by_label_contains(&zoom)
        .map(|n| n.rect())
        .min_by(|a, b| a.top().total_cmp(&b.top()))
        .expect("подписи зума у ведущего узла нет");
    let before = h.query_all_by_label_contains(&publish).count();
    right_click(&mut h, rect.center());
    assert!(
        h.query_all_by_label_contains(&publish).count() > before,
        "у ведущего узла дверь публикации закрыта, хотя свойства у него те же, \
         что у слота: «или у нас дубли кода?»"
    );
}

/// РЕЕСТР ОКОН программы (Р.63 п.12).
///
/// Владелец: «Я ставлю запрет любым агентам порождать какие-либо
/// дополнительные окна без очевидного смысла (по типу окна при закрытии
/// программы). Мне надоело что агенты срут этими окнами и не думают».
/// Запрет держится не обещанием, а списком: у каждого окна здесь стоит
/// причина, по которой оно не может быть частью уже существующего места.
/// Новое окно, не внесённое сюда, валит t757 — и внести его значит
/// написать эту причину словами.
const WINDOWS: &[(&str, &str)] = &[
    ("fx", "цепочка эффектов события: полоса, параметры, автоматизация, голова источника и параметров композиции"),
    ("morph", "морфер-слой: свои цели, окно и патч — отдельная сущность монтажа"),
    ("transition", "переход на стыке: вид, кривые, инверсия"),
    ("camera", "камера-событие: своё, потому что живёт на камерной дорожке"),
    ("track_window", "дорожка-объект: имя, уровень, выключатели, цепочка и дакинг"),
    ("env_fit", "сопоставление полей при вставке огибающей — вопрос, на который надо ответить до вставки"),
    ("project_settings", "настройки проекта: размер кадра, частота, правило приведения"),
    ("kt_render_settings", "параметры экспорта: заполняются до запуска и проверяются разом"),
    ("kt_render_progress", "ход экспорта: он идёт минутами и его можно отменить"),
    ("kt_analysis", "ход фоновых анализов — то же самое для громкости, синхрона и дакинга"),
    ("kt_notices", "журнал сообщений: строка состояния показывает последнее, журнал — все"),
    ("kt_search", "поиск по монтажу: список находок живёт, пока по нему ходят"),
    ("kt_diag", "диагностика и счётчики: окно разработчика, живёт рядом с работой"),
    ("seq_name", "имя новой ленты: спрашивается один раз при сворачивании выделения"),
    ("paste_names", "приставка к именам вставляемого — спрашивается до вставки"),
    ("preset_save", "имя сохраняемого пресета — спрашивается один раз при сохранении"),
    ("pub_name", "имя публикуемой ручки — спрашивается один раз"),
    ("pub_unpublish", "подтверждение снятия ручки: переопределения умрут во всех местах"),
    ("published_win", "авторский список публичных параметров ленты"),
    ("kt_recovery", "восстановление несохранённой работы при старте"),
    ("kt_ask", "вопрос при закрытии — то самое «очевидно осмысленное» окно из слов владельца"),
];

/// Тест 757 (Р.63 п.12): КАЖДОЕ окно программы — в реестре, и наоборот.
///
/// Сканер ищет `egui::Window::new`/`Modal::new` по исходникам kt-app и
/// достаёт id окна (он обязателен с t129). Окно без записи — новое окно,
/// заведённое молча; запись без окна — мусор в реестре.
#[test]
fn t757_every_window_is_in_the_registry() {
    let root = workspace_root();
    let dir = root.join("apps/kt-app/src");
    let mut found: Vec<(String, String)> = Vec::new();
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .expect("исходники kt-app")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        // сами тесты окон не заводят
        if name.ends_with("_tests.rs") {
            continue;
        }
        let src = strip_comments(&std::fs::read_to_string(&path).expect("файл"));
        let lines: Vec<&str> = src.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            if !line.contains("egui::Window::new") && !line.contains("Modal::new") {
                continue;
            }
            // id — на этой же строке (модалка) или на ближайших следующих
            let mut id = None;
            for probe in lines.iter().skip(n).take(4) {
                if let Some(p) = probe.find("egui::Id::new(") {
                    let rest = &probe[p + "egui::Id::new(".len()..];
                    let rest = rest.trim_start_matches('(');
                    if let Some(q1) = rest.find('"') {
                        let tail = &rest[q1 + 1..];
                        if let Some(q2) = tail.find('"') {
                            id = Some(tail[..q2].to_string());
                            break;
                        }
                    }
                }
            }
            match id {
                Some(id) => found.push((id, format!("{name}:{}", n + 1))),
                None => panic!(
                    "{name}:{}: у окна нет `egui::Id::new(\"…\")` — его нечем ни отличить, \
                     ни внести в реестр (правило t129)",
                    n + 1
                ),
            }
        }
    }
    assert!(!found.is_empty(), "сканер не нашёл ни одного окна — он сломан");

    for (id, at) in &found {
        assert!(
            WINDOWS.iter().any(|(k, _)| k == id),
            "окно «{id}» ({at}) не внесено в реестр WINDOWS. Владелец запретил \
             заводить окна без очевидного смысла — внести окно значит написать \
             словами, почему оно не может быть частью уже существующего места"
        );
    }
    for (id, why) in WINDOWS {
        assert!(
            found.iter().any(|(f, _)| f == id),
            "в реестре есть запись «{id}» ({why}), а окна с таким id в программе нет"
        );
        assert!(
            why.chars().count() >= 25,
            "у окна «{id}» причина не написана словами: {why:?}"
        );
    }
    // окон, которые владелец назвал лишними, в программе больше нет
    for gone in ["duck", "inst_params"] {
        assert!(
            !found.iter().any(|(f, _)| f == gone),
            "окно «{gone}» вернулось: оно было снесено по прямому требованию владельца (Р.63)"
        );
    }
}

/// Тест 766 (Р.64): экранные контролы ДОСТИЖИМЫ и объясняют себя.
///
/// Жест, которого не найти, не существует (правило 1 чек-листа), а ручка,
/// молчащая в строке состояния, — это «угадай, что я сделаю» (правило 10).
/// Здесь щупается ровно то, откуда действует человек: правый клик по кадру,
/// пункт меню, наведение на ручку. Отдельным утверждением — что контролы
/// живут в ОДНОМ месте (`draw_preview`), поэтому в отделённом окне они те
/// же самые: своей копии рисования превью в программе нет.
#[test]
fn t766_on_screen_controls_are_reachable_and_speak() {
    use egui_kittest::kittest::Queryable;
    let (mut h, clip) = crate::layout_tests::harness_geom_ctl();
    // сцена начинается с ВЫКЛЮЧЕННЫХ контролов — их и надо найти
    h.state_mut().geom_ctl = crate::geomctl::GeomMode::Off;
    h.step();

    // ---- переключатель: правый клик по кадру → первый пункт ------------
    let pos = h.state().preview_rect.center();
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
    let label = crate::tf!(
        crate::i18n::k::geomctl::TOGGLE,
        crate::geomctl::mode_name(crate::geomctl::GeomMode::Off.next())
    );
    h.get_by_label_contains(&label).click();
    h.step();
    h.step();
    assert_eq!(
        h.state().geom_ctl,
        crate::geomctl::GeomMode::Transform,
        "пункт меню превью не включил контролы: жест остался недостижимым"
    );

    // ---- ручка называет СЕБЯ в строке состояния ------------------------
    let q = h.state().player.preview_quality();
    let canvas = (q.width as f64, q.height as f64);
    let pl = h.state().geom_placement(clip, canvas).expect("размещение");
    let img = h.state().preview_img_rect;
    let to_screen = |p: (f64, f64)| -> egui::Pos2 {
        egui::pos2(
            img.left() + (p.0 / canvas.0) as f32 * img.width(),
            img.top() + (p.1 / canvas.1) as f32 * img.height(),
        )
    };
    for (handle, key) in [
        (kt_fx::geom::Handle::Rot, crate::i18n::k::geomctl::HINT_ROT),
        (kt_fx::geom::Handle::TopLeft, crate::i18n::k::geomctl::HINT_SCALE),
    ] {
        h.hover_at(to_screen(handle.point(&pl)));
        h.step();
        h.step();
        let shown = h.state().status_line_shown.0.clone();
        assert_eq!(
            shown,
            crate::i18n::tr(key),
            "над ручкой {handle:?} строка состояния говорит «{shown}» — \
             подсказка обязана называть ЖЕСТ, который тут будет"
        );
    }

    // ---- контролы рисуются В ОДНОМ МЕСТЕ -------------------------------
    // (значит отделённое окно получает их тем же кодом: `draw_preview`
    // зовут обе ветки — панель и вьюпорт)
    let src = std::fs::read_to_string("apps/kt-app/src/app.rs")
        .or_else(|_| std::fs::read_to_string("src/app.rs"))
        .expect("исходник app.rs");
    let calls = src.matches("self.geometry_controls(").count();
    assert_eq!(
        calls, 1,
        "контролы геометрии зовутся из {calls} мест — вторая копия разойдётся \
         с первой молча (правило 17)"
    );
    assert!(
        src.matches("self.draw_preview(ui);").count() >= 2,
        "превью рисуется одним вызовом — отделённое окно перестало быть тем же кодом"
    );
}

/// Тест 777 (Р.65): САЙДЧЕЙН ДОСТИЖИМ и объясняет отказ.
///
/// Правило Р.40: щупать оттуда, откуда действует человек. К дакингу ведут
/// два пути — окно дорожки (секция) и её же полоса цепочки (компрессор со
/// своим полем ключа); оба обязаны вести в ОДНО тело (правило 17), а
/// невозможный выбор — называться словами, а не отсутствовать молча.
#[test]
fn t777_sidechain_is_reachable_and_names_its_refusals() {
    use egui_kittest::kittest::Queryable;
    use kt_model::LaneKind::Audio;
    let (mut h, _) = crate::layout_tests::harness_three_clips_dt(None);
    let (bg, key) = {
        let app = h.state_mut();
        crate::layout_tests::edit_project(app, |p| p.ensure_tracks(Audio, 2));
        let bg = app.model.project().track_at(Audio, 0).unwrap().id;
        let key = app.model.project().track_at(Audio, 1).unwrap().id;
        (bg, key)
    };
    h.step();

    // ПУТЬ 1: окно дорожки — секция дакинга с выбором ключа
    h.state_mut().open_track_window(bg);
    h.step();
    h.step();
    assert!(
        h.query_all_by_label_contains(crate::i18n::tr(crate::i18n::k::msg::DUCK_KEY)).count() > 0,
        "в окне дорожки нет выбора ключа — сайдчейн недостижим оттуда, где человек стоит"
    );

    // ПУТЬ 2: бейдж FX дорожки ведёт в ТО ЖЕ окно (одно тело, правило 17)
    h.state_mut().track_win = None;
    h.step();
    h.state_mut().open_track_fx(bg);
    h.step();
    assert_eq!(
        h.state().track_win.as_ref().map(|w| w.id),
        Some(bg),
        "бейдж FX ведёт не в окно дорожки — путей два, а тело должно быть одно"
    );

    // ОТКАЗ НАЗВАН: цикл модель отвергает с причиной, а не молча
    {
        let app = h.state_mut();
        let mut c = kt_model::FxInstance::compressor();
        if let kt_model::FxInstance::Compressor { key: k, .. } = &mut c {
            *k = Some(key);
        }
        app.apply(kt_model::Command::SetTrackFx { id: bg, fx: vec![c.clone()], bypass: vec![] });
        // обратный ключ — цикл
        let mut back = kt_model::FxInstance::compressor();
        if let kt_model::FxInstance::Compressor { key: k, .. } = &mut back {
            *k = Some(bg);
        }
        let e = app
            .model
            .apply(kt_model::Command::SetTrackFx { id: key, fx: vec![back], bypass: vec![] })
            .unwrap_err();
        assert!(
            matches!(e, kt_model::CommandError::SidechainCycle(_)),
            "цикл отвергнут без названной причины: {e:?}"
        );
        assert!(
            !format!("{e}").is_empty(),
            "у отказа нет текста — человеку нечего показать в строке состояния"
        );
    }
}

/// Как открыть окно реестра в харнессе — или почему это невозможно (Р.66).
enum OpenWin {
    /// Функция готовит сцену и открывает окно.
    By(fn(&mut egui_kittest::Harness<'static, crate::app::KtApp>)),
    /// Окно в харнессе не открыть; причина пишется словами, как в t253.
    Cant(&'static str),
}

/// Чем окно закрывается: своей кнопкой действия или крестиком заголовка.
/// Второе — законно (журнал, поиск, диагностика), но объявляется ЯВНО и с
/// причиной: иначе «кнопок не нашлось» тихо сойдёт за норму.
enum Closes {
    Action,
    /// Окно КОРОТКОЕ ПО ПОСТРОЕНИЮ (вопрос из заголовка, строки и ряда
    /// кнопок): прокрутке там нечего скрывать. Причина пишется словами, а
    /// «короткое» ПРОВЕРЯЕТСЯ числом строк — иначе исключение станет
    /// дешевле проверки.
    ActionShort { why: &'static str },
    Titlebar { title: fn() -> String, why: &'static str },
}

/// Сколько строк окно имеет право показывать, чтобы считаться коротким.
const SHORT_WINDOW_ROWS: usize = 25;

/// Кнопки ДЕЙСТВИЯ окна: по любой из них тест жмёт. Набор общий — окна
/// программы закрываются и отменяются одними и теми же словами.
fn action_labels() -> Vec<String> {
    use crate::i18n::{k, tr};
    vec![
        tr(k::track::CLOSE).to_string(),
        tr(k::track::CANCEL).to_string(),
        tr(k::fx::CANCEL).to_string(),
        tr(k::render::CLOSE).to_string(),
        tr(k::render::CANCEL).to_string(),
        tr(k::settings::CANCEL).to_string(),
        tr(k::ask::RECOVERY_RESTORE).to_string(),
        tr(k::ask::RECOVERY_LATER).to_string(),
    ]
}

/// Таблица окон: id из `WINDOWS` → чем его открыть.
fn window_openers() -> Vec<(&'static str, OpenWin, Closes)> {
    use kt_model::*;
    vec![
        ("fx", OpenWin::By(|h| {
            // сцена — генератор: событие поднимается тем же путём, что в
            // программе, и не требует файла на диске
            h.state_mut().create_generator(GeneratorKind::Solid {
                color_r: 200,
                color_g: 40,
                color_b: 40,
            });
            h.step();
            let id = first_clip(h);
            h.state_mut().open_fx_editor(id);
        }), Closes::Action),
        ("track_window", OpenWin::By(|h| {
            let app = h.state_mut();
            crate::layout_tests::edit_project(app, |p| p.ensure_tracks(LaneKind::Audio, 1));
            let id = app.model.project().track_at(LaneKind::Audio, 0).unwrap().id;
            app.open_track_window(id);
        }), Closes::Action),
        ("project_settings", OpenWin::By(|h| h.state_mut().open_project_settings()), Closes::Action),
        ("kt_notices", OpenWin::By(|h| h.state_mut().notices_open = true), Closes::Titlebar { title: || crate::i18n::tr(crate::i18n::k::win::NOTICES_TITLE).to_string(), why: "журнал сообщений закрывается крестиком заголовка: своего действия у списка нет" }),
        ("kt_search", OpenWin::By(|h| h.state_mut().search_open = true), Closes::Titlebar { title: || crate::i18n::tr(crate::i18n::k::win::SEARCH_TITLE).to_string(), why: "поиск закрывается крестиком заголовка: список находок живёт, пока по нему ходят" }),
        ("kt_diag", OpenWin::By(|h| h.state_mut().perf.diag_open = true), Closes::Titlebar { title: || crate::i18n::tr(crate::i18n::k::win::DIAG_TITLE).to_string(), why: "диагностика закрывается крестиком заголовка: это окно разработчика без действий" }),
        ("kt_recovery", OpenWin::By(|h| h.state_mut().recovery_open = true), Closes::Action),
        ("kt_ask", OpenWin::By(|h| {
            h.state_mut().ask = Some(crate::app::Ask::DropOtio);
        }), Closes::ActionShort {
            why: "вопрос при закрытии: заголовок, одна строка и ряд кнопок — содержимому расти \
                  неоткуда, и прокрутке нечего скрывать",
        }),
        ("morph", OpenWin::Cant(
            "морфер-слой нужно сперва поставить на морф-дорожку: окно открывается по его id, и \
             сцена для него строится отдельным тестом слоя",
        ), Closes::Action),
        ("transition", OpenWin::Cant(
            "окно перехода открывается по id существующего перехода, а переход появляется только \
             из наезда двух событий — своя сцена, она есть в тестах переходов",
        ), Closes::Action),
        ("camera", OpenWin::Cant(
            "камера-событие живёт на камерной дорожке и открывается по своему id — сцена с \
             камерой строится в тестах камеры",
        ), Closes::Action),
        ("env_fit", OpenWin::Cant(
            "окно сопоставления полей появляется только при вставке огибающей, которая не легла \
             молча: его сцена — содержимое буфера обмена, она есть в тестах вставки",
        ), Closes::Action),
        ("kt_render_settings", OpenWin::Cant(
            "окно параметров экспорта заводится вместе с его сессией рендера; сессия требует \
             настоящего кодека, которого в харнессе нет",
        ), Closes::Action),
        ("kt_render_progress", OpenWin::Cant(
            "окно хода экспорта живёт только при ЗАПУЩЕННОЙ задаче кодирования — в харнессе её \
             не завести без настоящего кодека",
        ), Closes::Action),
        ("kt_analysis", OpenWin::Cant(
            "окно хода анализа живёт, пока жива фоновая задача; в харнессе она не запускается без \
             настоящего файла материала",
        ), Closes::Action),
        ("seq_name", OpenWin::Cant(
            "имя новой ленты спрашивается ровно в момент сворачивания выделения — своя сцена, \
             она есть в тестах композиций",
        ), Closes::Action),
        ("paste_names", OpenWin::Cant(
            "приставка к именам спрашивается при вставке ИМЕНОВАННОГО из буфера — сцена буфера \
             строится в тестах копипаста",
        ), Closes::Action),
        ("preset_save", OpenWin::Cant(
            "имя пресета спрашивается при сохранении цепочки из полосы — сцена есть в тестах \
             библиотеки пресетов",
        ), Closes::Action),
        ("pub_name", OpenWin::Cant(
            "имя ручки спрашивается при публикации изнутри ленты — сцена есть в тестах публичных \
             параметров",
        ), Closes::Action),
        ("pub_unpublish", OpenWin::Cant(
            "подтверждение снятия ручки появляется у существующей ручки — сцена есть в тестах \
             публичных параметров",
        ), Closes::Action),
        ("published_win", OpenWin::Cant(
            "авторский список ручек открывается изнутри ленты по её адресу — сцена есть в тестах \
             публичных параметров",
        ), Closes::Action),
    ]
}

fn first_clip(h: &egui_kittest::Harness<'static, crate::app::KtApp>) -> kt_model::ClipId {
    h.state().model.project().clips.first().map(|c| c.id).expect("в сцене есть событие")
}

/// Тест 779 (Р.66): КОНТРАКТ ФОРМЫ — по всем окнам реестра.
///
/// Прямая просьба владельца Р.64: «и не забудь на будущее во всех других
/// виджетах и окнах тоже пройтись надо будет». Проза этого не удержала:
/// к Р.66 прокрутка тела была ровно у ОДНОГО окна из двадцати одного, и
/// появилась она только потому, что t775 уронил окно со спрятанными за
/// кромкой кнопками. Здесь два утверждения, оба машинные:
///
/// 1. ни одного своего `DragValue` вне общего виджета — иначе у числа
///    заводятся вторые пределы и вторая единица (дефект Р.33: Gain был
///    −24…24 в одном окне и −48…24 в другом);
/// 2. у каждого окна реестра, которое можно открыть, кнопка действия
///    ВИДНА и НАЖИМАЕТСЯ в низком окне — то есть тело прокручивается, а
///    кнопки живут вне прокрутки.
#[test]
fn t779_window_form_contract() {
    // --- 1. ни одного числа мимо общего виджета ------------------------
    let root = workspace_root();
    let dir = root.join("apps/kt-app/src");
    let mut bad: Vec<String> = Vec::new();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .expect("исходники kt-app")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    files.sort();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        // widgets.rs — ДОМ ряда: там он и определён; сам сторож ищет это
        // имя строкой и считал бы себя
        if name == "widgets.rs" || name == "reach_tests.rs" {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("исходник читается");
        // тестовый модуль не считается: текст ассертов человек не видит
        let code = src.split("\n#[cfg(test)]").next().unwrap_or(&src);
        for (i, line) in code.lines().enumerate() {
            if line.contains("DragValue::new") {
                bad.push(format!("{name}:{}", i + 1));
            }
        }
    }
    let mut problems: Vec<String> = Vec::new();
    if !bad.is_empty() {
        problems.push(format!(
            "своё числовое поле мимо общего ряда виджетов (widgets::param_row / plain_row): {}. \
             У такого числа свои пределы и своя единица — это второй источник правды о них \
             (правило «один виджет — не копия»)",
            bad.join(", ")
        ));
    }

    // --- 2. кнопки действия достижимы в НИЗКОМ окне --------------------
    let openers = window_openers();
    let known: std::collections::HashSet<&str> = WINDOWS.iter().map(|(id, _)| *id).collect();
    for (id, _, _) in &openers {
        assert!(known.contains(id), "в таблице окон есть «{id}», которого нет в реестре WINDOWS");
    }
    for (id, _) in WINDOWS {
        assert!(
            openers.iter().any(|(o, _, _)| o == id),
            "окно «{id}» есть в реестре, но в таблице t779 его нет: \
             контракт формы обязан быть проверен у КАЖДОГО окна либо иметь причину"
        );
    }
    for (id, how, closes) in openers {
        let open = match how {
            OpenWin::Cant(why) => {
                assert!(
                    why.chars().count() >= 25,
                    "окно «{id}»: причина «{why}» короче 25 символов — так отговорка становится \
                     дешевле проверки"
                );
                continue;
            }
            OpenWin::By(f) => f,
        };
        // НИЗКОЕ окно: содержимое заведомо выше экрана
        let mut h = crate::layout_tests::harness_sized(1100.0, 420.0);
        open(&mut h);
        h.step();
        h.step();
        let screen = h.ctx.content_rect();
        match closes {
            Closes::Titlebar { title, why } => {
                assert!(
                    why.chars().count() >= 25,
                    "окно «{id}»: причина «{why}» короче 25 символов"
                );
                // заголовок нарисован и виден — значит крестик достижим
                let t = title();
                use egui_kittest::kittest::Queryable;
                let seen = h
                    .query_all_by_label_contains(&t)
                    .map(|n| n.rect())
                    .any(|r| screen.contains_rect(r));
                if !seen {
                    problems.push(format!(
                        "«{id}»: заголовок «{t}» не виден на экране — крестик, которым окно \
                         закрывается, недостижим"
                    ));
                }
            }
            Closes::ActionShort { why } => {
                assert!(why.chars().count() >= 25, "окно «{id}»: причина «{why}» короче 25 символов");
                // «короткое» — это измеримо: столько строк, сколько влезает
                // без прокрутки на любом экране
                use egui_kittest::kittest::{NodeT, Queryable};
                let rows = h
                    .query_all_by_label_contains("")
                    .filter_map(|n| n.accesskit_node().label().map(|l| l.to_string()))
                    .count();
                let mut bare = crate::layout_tests::harness_sized(1100.0, 420.0);
                bare.step();
                let base = bare
                    .query_all_by_label_contains("")
                    .filter_map(|n| n.accesskit_node().label().map(|l| l.to_string()))
                    .count();
                let own = rows.saturating_sub(base);
                if own > SHORT_WINDOW_ROWS {
                    problems.push(format!(
                        "«{id}» объявлено коротким, но показывает {own} своих строк (предел \
                         {SHORT_WINDOW_ROWS}) — значит прокрутка ему нужна"
                    ));
                }
                let labels = action_labels();
                let found = labels
                    .iter()
                    .flat_map(|l| h.query_all_by_label(l).map(|n| n.rect()).collect::<Vec<_>>())
                    .collect::<Vec<_>>();
                if !found.iter().any(|r| screen.contains_rect(*r)) {
                    problems.push(format!("«{id}»: кнопки действия недостижимы"));
                }
            }
            Closes::Action => {
                // ТЕЛО ОКНА ЖИВЁТ В ПРОКРУТКЕ. Проверяется по состоянию
                // самой прокрутки в памяти egui: окно зажимается экраном и
                // рисует клипованные узлы с ОБРЕЗАННЫМИ прямоугольниками —
                // ни «узел за кромкой», ни «спрятанные строки» в дереве
                // доступности не видны, то есть следствие «часть формы
                // недостижима» приборами kittest не наблюдаемо. Поэтому
                // здесь честная структурная проверка с названной причиной:
                // открытие окна обязано завести хотя бы одно состояние
                // ScrollArea (а что она не вокруг пустоты — держит живой
                // сценарий `forms.py`, который крутит колесо в настоящем
                // окне).
                let scrolls = |h: &egui_kittest::Harness<'static, crate::app::KtApp>| {
                    h.ctx.memory(|m| m.data.count::<egui::scroll_area::State>())
                };
                let mut bare = crate::layout_tests::harness_sized(1100.0, 420.0);
                bare.step();
                let before = scrolls(&bare);
                let after = scrolls(&h);
                if after <= before {
                    problems.push(format!(
                        "«{id}»: тело окна не в прокрутке (состояний ScrollArea {after} против \
                         {before} без окна) — содержимому, которое не влезло, некуда деться"
                    ));
                }
                let labels = action_labels();
                let found: Vec<(String, egui::Rect)> = labels
                    .iter()
                    .flat_map(|l| {
                        use egui_kittest::kittest::Queryable;
                        h.query_all_by_label(l)
                            .map(move |n| (l.clone(), n.rect()))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                if found.is_empty() {
                    problems.push(format!(
                        "«{id}»: в окне не найдено ни одной кнопки действия, а крестик заголовка \
                         как способ закрытия не объявлен"
                    ));
                    continue;
                }
                if !found.iter().any(|(_, r)| screen.contains_rect(*r)) {
                    problems.push(format!(
                        "«{id}»: кнопки действия за кромкой экрана {:?} — окно переросло экран и \
                         спрятало свои действия (тело обязано прокручиваться, кнопки — жить вне \
                         прокрутки)",
                        found.iter().map(|(l, r)| format!("{l} {r:?}")).collect::<Vec<_>>()
                    ));
                }
            }
        }
    }
    assert!(problems.is_empty(), "контракт формы нарушен:\n  {}", problems.join("\n  "));
}

/// Тест 783 (Р.67): проект СТАРОЙ версии открывается через свою дверь —
/// «Файл → Импортировать проект старой версии…», — а обычное «Открыть»
/// отвергает его с версией файла и именем этой двери (правило 19: диалог
/// говорит, где изменить решение). Импорт чинит `targets: null` у
/// морферов v5 (форма `untitled2_v26.09.26.otio`), открывает документ с
/// прежним путём и звёздочкой. Ридер при этом не трогается — на прежнем
/// коде «Открыть» отвечало «metadata.kt is unreadable», а двери не было.
#[test]
fn t783_older_project_goes_through_import_and_open_names_the_door() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    use crate::app::KtApp;
    use kt_model::{
        ClipEvent, ClipId, ClipKind, MediaId, MediaRef, MorphTargets, Project, ProjectSettings,
    };
    use kt_time::Tick;

    // --- фикстура: v5 с null-целями у морфера ----------------------------
    let dir = std::env::temp_dir().join(format!("kt-t783-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("old_v5.otio");
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
    p.media.push(MediaRef::new(MediaId(1), "synth:Ramp"));
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
    let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
    old["metadata"]["kt"]["v"] = serde_json::json!(5);
    old["metadata"]["kt"]["morph_lanes"] = serde_json::json!(1);
    old["metadata"]["kt"]["morphers"] = serde_json::json!([{
        "id": 1, "lane": 0, "start": 0, "duration": 45000,
        "targets": null,
        "patch": { "media": 1, "source_in_delta": 4500 },
    }]);
    std::fs::write(&path, serde_json::to_string_pretty(&old).unwrap()).unwrap();

    // --- «Открыть» отвергает с версией и именем двери ---------------------
    let err = match KtApp::open(&path) {
        Ok(_) => panic!("v5 с null-целями обычное «Открыть» читать не должно"),
        Err(e) => e,
    };
    assert!(err.contains("5"), "ошибка без версии файла: {err}");
    assert!(
        err.contains(tr(k::menu::file::IMPORT_OLDER)),
        "ошибка не называет пункт импорта: {err}"
    );
    assert!(!err.contains("unreadable"), "прежний текст «unreadable» — не адрес: {err}");

    // --- дверь есть в меню «Файл» -----------------------------------------
    let mut h = crate::layout_tests::harness();
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();
    h.step();
    let labels = crate::layout_tests::menu_labels(&mut h);
    assert!(
        h.query_by_label(tr(k::menu::file::IMPORT_OLDER)).is_some(),
        "пункт импорта старой версии обязан быть в меню «Файл»; видно: {labels:?}"
    );
    h.get_by_label(tr(k::menu::file::TITLE)).click();
    h.step();

    // --- импорт по пути: то же тело, что у пункта (без системного диалога)
    h.state_mut().import_older_path(&path);
    h.step();
    h.step();
    let app = h.state();
    assert_eq!(app.project_path.as_deref(), Some(path.as_path()), "путь прежний");
    assert!(app.dirty_content, "после импорта документ помечен правками (звёздочка)");
    let doc = app.model.project();
    assert_eq!(doc.clips.len(), 1, "событие дошло");
    assert_eq!(doc.morphers.len(), 1, "морфер дошёл");
    assert_eq!(doc.morphers[0].targets, MorphTargets::Below, "null стал «всё, что ниже»");
    let said: Vec<String> = app.notices.iter().map(|n| n.text.clone()).collect();
    assert!(
        said.iter().any(|t| t.contains("null → below")),
        "починка не названа в уведомлениях: {said:?}"
    );
    assert!(
        said.iter().any(|t| t.contains(&crate::tf!(k::msg::IMPORT_OLDER_DONE, 5))),
        "импорт не сообщил о версии: {said:?}"
    );

    // --- сохранение пишет текущую форму, и «Открыть» уже читает сам -------
    let out = kt_otio::save_project(doc, &Default::default(), path.parent());
    let out_path = dir.join("saved.otio");
    std::fs::write(&out_path, &out).unwrap();
    let reopened = KtApp::open(&out_path).expect("после импорта и записи файл читается сам");
    assert_eq!(reopened.model.project().morphers[0].targets, MorphTargets::Below);
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 788 (Р.67): дверь связи «Ведётся…» в блоке кривой — из окна FX:
/// «это событие» → свойство → источник-связь с адресом, имя кривой несёт
/// адрес, «отвязать» возвращает ключи. На прежнем коде двери не было: модель
/// и тесты связи существовали, а назначить её человеку было нечем.
#[test]
fn t788_link_door_assigns_a_source() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    let mut h = crate::layout_tests::harness_with_fx_window_two_slots();
    crate::layout_tests::reveal_curve_block(&mut h);
    h.get_by_label(tr(k::fx::DRIVE_BY)).click();
    h.step();
    h.step();
    // подменю открывается кликом (наведение в харнессе его не раскрывает);
    // у кнопки подменю egui дописывает стрелку — ищем по вхождению
    h.query_all_by_label_contains(tr(k::fx::DRIVE_THIS)).last().expect("подменю «Это событие»").click();
    h.step();
    h.step();
    // свойство второго слота — по подписи «Эффект: параметр»
    let wanted = crate::widgets::param_label("radius_y");
    h.query_all_by_label_contains(&wanted).last().expect("свойство соседнего слота в списке").click();
    h.step();
    h.step();
    let c = h.state().model.project().clips[0].fx_auto[0][0].clone();
    match &c.source {
        kt_model::CurveSource::Link { path, .. } => {
            assert_eq!(path, "self/fx1/radius_y", "адрес связи не тот");
        }
        other => panic!("связь не назначена: {other:?}"),
    }
    let texts: Vec<String> = painted_texts(&h).into_iter().map(|t| t.src).collect();
    assert!(texts.iter().any(|t| t.contains("<- self/fx1/radius_y")), "имя кривой не несёт адрес: {texts:?}");
    // «отвязать» — ключи
    h.get_by_label(tr(k::fx::UNLINK)).click();
    h.step();
    h.step();
    assert!(h.state().model.project().clips[0].fx_auto[0][0].source.is_keys(), "«отвязать» не вернул ключи");
}

/// Тест 789b (Р.67): справка по формуле и само поле достижимы из окна FX
/// (слот), из формы генератора и из головы экземпляра ленты — блок один.
#[test]
fn t789b_formula_help_reachable_from_every_carrier() {
    use crate::i18n::{k, tr};
    use egui_kittest::kittest::Queryable;
    // слот
    let h = crate::layout_tests::harness_with_fx_window();
    assert!(h.query_by_label(tr(k::fx::HELP)).is_some(), "справки нет у слота");
    assert!(h.query_by_label(tr(k::fx::DRIVE_BY)).is_some(), "двери связи нет у слота");
    // экземпляр ленты
    let (mut h, s) = crate::layout_tests::harness_news_template();
    h.state_mut().open_fx_editor_head(s.b);
    h.step();
    h.step();
    h.get_by_label(tr(k::fx::AUTO_ADD)).click();
    h.step();
    h.step();
    h.query_all_by_label_contains("Кегль").last().expect("пункт «Кегль»").click();
    h.step();
    h.step();
    assert!(h.query_by_label(tr(k::fx::HELP)).is_some(), "справки нет у экземпляра");
    assert!(h.query_by_label(tr(k::fx::DRIVE_BY)).is_some(), "двери связи нет у экземпляра");
}

/// Тест 799 (Р.67): сторож «ПУТИ ПОЛЬЗОВАТЕЛЯ». У каждого UI-сценария с
/// тестом kt-app от t783 в спеке стоит строка `ПУТЬ` — что человек делает
/// и что видит; у сценария без жеста — причина словами, не короче сорока
/// символов. Проверяется ЗАПУСКОМ скрипта: он же стоит в `finish.sh`.
///
/// Владелец: «СКОЛЬКО РАЗ МНЕ ПОВТОРЯТЬ ЧТОБЫ ВСЕ АГЕНТЫ ДУМАЛИ О
/// ПОЛЬЗОВАТЕЛЕ В ПЕРВУЮ ОЧЕРЕДЬ? … ЭТО КООРДИНАЛЬНО НАДО МЕНЯТЬ!».
#[test]
fn t799_ui_scenarios_describe_the_user_path() {
    // CARGO_MANIFEST_DIR = src/apps/kt-app; корень дерева — на три выше
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let script = root.join("src/scripts/specs-user-path.py");
    assert!(script.exists(), "сторожа нет на месте: {}", script.display());

    let run = |dir: &std::path::Path| -> (bool, String) {
        let out = std::process::Command::new("python3")
            .arg(&script)
            .arg("--check")
            .current_dir(dir)
            .output()
            .expect("python3 для сторожа спек");
        (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
    };

    // 1) на СЕГОДНЯШНИХ спеках — зелёный
    let (ok, said) = run(&root);
    assert!(ok, "сторож красен на текущих спеках: {said}");

    // 2) у НОВОГО сценария удаляем строку ПУТЬ — обязан покраснеть с
    //    адресом файла и именем сценария
    let specs: Vec<std::path::PathBuf> = ["ui-timeline", "ui-reach"]
        .iter()
        .flat_map(|cap| {
            let mut v = vec![root.join("openspec/specs").join(cap).join("spec.md")];
            if let Ok(rd) = std::fs::read_dir(root.join("openspec/changes")) {
                for e in rd.flatten() {
                    v.push(e.path().join("specs").join(cap).join("spec.md"));
                }
            }
            v
        })
        .filter(|p| p.exists())
        .collect();
    let victim = specs
        .iter()
        .find(|p| std::fs::read_to_string(p).unwrap_or_default().contains("- **ПУТЬ**"))
        .cloned()
        .expect("ни в одной спеке нет строки ПУТЬ — правило не действует вовсе");
    let orig = std::fs::read_to_string(&victim).unwrap();
    let broken = orig.replacen("- **ПУТЬ**", "- **БЫЛО**", 1);
    std::fs::write(&victim, &broken).unwrap();
    let (ok_after, said_after) = run(&root);
    std::fs::write(&victim, &orig).unwrap();
    assert!(!ok_after, "сторож НЕ заметил сценария без пути пользователя");
    assert!(
        said_after.contains("сценарий") && said_after.contains("spec.md"),
        "сторож молчит об адресе: {said_after}"
    );

    // 3) сам сторож стоит в finish.sh — иначе он зелёный и забытый
    let finish = std::fs::read_to_string(root.join("src/scripts/finish.sh")).expect("finish.sh");
    assert!(
        finish.contains("specs-user-path.py"),
        "сторож не зовётся из finish.sh: правило снова держится только прозой"
    );
    assert!(
        finish.contains("шаги живых сценариев"),
        "finish.sh не печатает список шагов ПУТИ — отчёт снова без картинок"
    );
}

/// t800 (Р.68): КАЖДАЯ стадия `finish.sh` называет свою цену, а бюджет —
/// пять минут — печатается и помечается при перерасходе.
///
/// Повод дословно (владелец): «тесты в finish.sh или что-то типо того -
/// очень долго выполняются, нужно что-то придумать». Замер показал круг
/// проверки в 21 минуту, и ни одна стадия не называла своей цены: говорить
/// о том, ЧТО ускорять, было нечем. Стадию можно было бы тихо вынести из
/// бюджета, напечатав заголовок мимо обёртки — это тот же класс, что
/// «красное чужим именем» (Р.55) и «вердикт у tail» (Р.58), поэтому
/// заголовок стадии печатает только обёртка, а сторож это проверяет.
#[test]
fn t800_finish_stages_name_their_price() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let script = root.join("src/scripts/finish-budget.py");
    let finish_path = root.join("src/scripts/finish.sh");
    assert!(script.exists(), "нет учёта цены: {}", script.display());

    let run = |args: &[&str]| -> (bool, String) {
        let out = std::process::Command::new("python3")
            .arg(&script)
            .args(args)
            .current_dir(&root)
            .output()
            .expect("python3 для учёта цены");
        let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
        said.push_str(&String::from_utf8_lossy(&out.stderr));
        (out.status.success(), said)
    };

    // 1) самопроверка учёта: перерасход назван, бюджет назван, красная
    //    стадия названа красной, сломанный учёт — красное
    let (ok, said) = run(&["--selftest"]);
    assert!(ok, "самопроверка учёта цены красная: {said}");

    // 2) все стадии скрипта идут через обёртку с замером
    let (ok, said) = run(&["--check-script", "src/scripts/finish.sh"]);
    assert!(ok, "стадия печатает заголовок мимо замера: {said}");

    // 3) заголовок мимо обёртки ловится на НАСТОЯЩЕМ скрипте, а не только
    //    на синтетике: без этой проверки сторож мог бы смотреть не туда
    let finish = std::fs::read_to_string(&finish_path).expect("finish.sh");
    let broken = format!("{finish}\necho \"== стадия мимо бюджета ==\"\n");
    let tmp = root.join("src/target/t800-finish-probe.sh");
    std::fs::create_dir_all(tmp.parent().unwrap()).ok();
    std::fs::write(&tmp, &broken).unwrap();
    let (ok_after, said_after) = run(&["--check-script", "src/target/t800-finish-probe.sh"]);
    std::fs::remove_file(&tmp).ok();
    assert!(!ok_after, "сторож НЕ заметил стадии вне замера");
    assert!(
        said_after.contains("мимо обёртки") && said_after.contains("стадия мимо бюджета"),
        "сторож молчит об адресе стадии: {said_after}"
    );

    // 4) скрипт действительно зовёт учёт и держит стадии, названные спекой:
    //    прогон мастерской — стадия ЭТОГО скрипта, отдельная стадия kt-gpu
    //    снята как дубль (Р.68), SIMD остаётся своим окружением
    for (need, why) in [
        ("finish-budget.py --report", "скрипт не печатает таблицу цены стадий"),
        ("тесты мастерской", "прогон мастерской не стал стадией скрипта"),
        ("SIMD-конформанс", "стадия SIMD пропала"),
        ("живые сценарии", "стадия живых сценариев пропала"),
    ] {
        assert!(finish.contains(need), "{why} (нет «{need}»)");
    }
    assert!(
        !finish.contains("cargo test -p kt-gpu"),
        "стадия kt-gpu осталась дублем прогона мастерской (Р.68)"
    );
}

/// Живая проверка есть на машине? Без Xvfb и собранной программы сценарии
/// сторожей гонять негде — тест скипается, НАЗЫВАЯ причину (вакуумный skip
/// без причины — это «зелено, потому что ничего не проверяли», урок т.194).
fn live_check_available(root: &std::path::Path) -> Option<String> {
    if std::process::Command::new("sh")
        .args(["-c", "command -v Xvfb"])
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        return Some("нет Xvfb (sudo apt install xvfb openbox)".into());
    }
    if !root.join("target/debug/kt-app").exists() {
        return Some("нет target/debug/kt-app (cargo build -p kt-app)".into());
    }
    None
}

fn run_live_check(
    root: &std::path::Path,
    workers: &str,
    display: &str,
    names: &[&str],
) -> (bool, String) {
    let out = std::process::Command::new("./scripts/live-check.sh")
        .args(names)
        .env("KT_LIVE_WORKERS", workers)
        // свои номера экранов у КАЖДОГО теста: тесты идут параллельно, и
        // два прогона на одних экранах открывали бы окна друг другу
        .env("KT_LIVE_DISPLAY", display)
        .current_dir(root)
        .output()
        .expect("scripts/live-check.sh");
    let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
    said.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), said)
}

/// t801 (Р.68): красный в параллели перегоняется ОДИН в одиночку, и только
/// второе падение — вердикт.
///
/// Правило «упавший под нагрузкой перезапускается отдельно» записано с Р.59
/// (§468: второй прогон рядом покрасил зелёные сценарии), но исполняла его
/// память агента. Владелец разрешил параллель вместе с механизмом —
/// значит механизм обязан быть проверяемым, а перегон — ВИДИМЫМ в отчёте:
/// молчаливый перегон превратил бы флак в невидимку.
#[test]
fn t801_red_scenario_is_rerun_alone() {
    let root = workspace_root();
    if let Some(why) = live_check_available(&root) {
        eprintln!("t801 пропущен: {why}");
        return;
    }
    // подставной «красный с первого раза» помнит прогон файлом — снимаем
    let mark = root.join("target/live-out/_flaky/already-ran");
    std::fs::remove_file(&mark).ok();

    let (ok, said) = run_live_check(&root, "2", ":80", &["_flaky", "_red"]);
    assert!(!ok, "прогон с всегда-красным сценарием обязан быть красным:\n{said}");
    assert!(
        said.contains("ПРОШЁЛ СО ВТОРОГО РАЗА") && said.contains("_flaky"),
        "перегон в одиночку не назван в отчёте:\n{said}"
    );
    assert!(
        said.contains("КРАСНЫЕ ДВАЖДЫ") && said.contains("_red"),
        "дважды упавший сценарий не назван красным:\n{said}"
    );
    let verdict = std::fs::read_to_string(root.join("target/live-out/verdicts/_flaky"))
        .unwrap_or_default();
    assert_eq!(verdict, "ok", "перегнанный сценарий остался красным");

    // одиночный прогон НИЧЕГО не перегоняет: там покраснение — вердикт
    std::fs::remove_file(&mark).ok();
    let (ok1, said1) = run_live_check(&root, "1", ":80", &["_flaky"]);
    assert!(!ok1, "одиночный прогон обязан отдать красное как есть:\n{said1}");
    assert!(
        said1.contains("прогон и так одиночный"),
        "одиночный прогон молча перегнал сценарий:\n{said1}"
    );
}

/// t802 (Р.68): работники не видят друг друга — свой экран и своё хранилище.
///
/// Это и есть закрытая причина запрета «в два потока»: два kt-app на одном
/// экране — прицел в чужое окно (`probe.window()` ищет по WM_CLASS), а общее
/// хранилище смешивает рабочее место, пресеты и замки.
#[test]
fn t802_workers_are_isolated() {
    let root = workspace_root();
    if let Some(why) = live_check_available(&root) {
        eprintln!("t802 пропущен: {why}");
        return;
    }
    let (ok, said) = run_live_check(&root, "3", ":84", &["_env1", "_env2", "_env3"]);
    assert!(ok, "подставные сценарии обязаны быть зелёными:\n{said}");
    let mut displays = std::collections::BTreeSet::new();
    let mut stores = std::collections::BTreeSet::new();
    for n in ["_env1", "_env2", "_env3"] {
        let p = root.join("target/live-out").join(n).join("env.json");
        let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let get = |key: &str| -> String {
            text.split(&format!("\"{key}\": \""))
                .nth(1)
                .and_then(|s| s.split('"').next())
                .unwrap_or_default()
                .to_string()
        };
        displays.insert(get("display"));
        stores.insert(get("store"));
    }
    assert_eq!(displays.len(), 3, "работники делят экран: {displays:?}");
    assert_eq!(stores.len(), 3, "работники делят хранилище: {stores:?}");
    assert!(
        said.contains("работников 3"),
        "прогон не сказал, сколькими работниками он шёл:\n{said}"
    );
}

/// t803 (Р.68): живые сценарии ждут ПРОГРАММУ, а не часы.
///
/// Замер раунда: 96 % времени живого прогона — безусловный сон (`nestparam`:
/// 130.5 с из 135.9 в 591 вызове). Правило прозой тут держалось бы ровно до
/// следующего сценария, поэтому его держит сторож: сырых пауз в сценариях не
/// бывает, в каркасе они живут в `pause`/`_poll`, и у каждой `pause` причина
/// словами не короче сорока символов (мера — та же, что у сторожа ПУТЬ).
#[test]
fn t803_live_scenarios_wait_for_the_program() {
    let root = workspace_root();
    let guard = root.join("scripts/live-sleeps.py");
    assert!(guard.exists(), "сторожа нет на месте: {}", guard.display());
    let run = || -> (bool, String) {
        let out = std::process::Command::new("python3")
            .arg(&guard)
            .arg("--check")
            .current_dir(&root)
            .output()
            .expect("python3 для сторожа пауз");
        let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
        said.push_str(&String::from_utf8_lossy(&out.stderr));
        (out.status.success(), said)
    };

    let (ok, said) = run();
    assert!(ok, "сторож красен на сегодняшних сценариях: {said}");

    // сырая пауза в СЦЕНАРИИ — красное с адресом
    let probe = root.join("scripts/live/_t803_probe.py");
    std::fs::write(&probe, "import time\n\n\ndef main(out):\n    time.sleep(0.5)\n").unwrap();
    let (ok_raw, said_raw) = run();
    std::fs::remove_file(&probe).ok();
    assert!(!ok_raw, "сторож не заметил сырой паузы в сценарии");
    assert!(
        said_raw.contains("_t803_probe.py") && said_raw.contains("сырой `time.sleep`"),
        "сторож молчит об адресе паузы: {said_raw}"
    );

    // пауза без внятной причины — тоже красное
    std::fs::write(
        &probe,
        "from kt_live import pause\n\n\ndef main(out):\n    pause(0.1, \"так надо\")\n",
    )
    .unwrap();
    let (ok_short, said_short) = run();
    std::fs::remove_file(&probe).ok();
    assert!(!ok_short, "сторож принял отговорку вместо причины паузы");
    assert!(
        said_short.contains("причина паузы короче"),
        "сторож не назвал короткую причину: {said_short}"
    );
}

/// t805 (Р.68): канал стенда мёртв без переменной окружения, кадры считаются
/// по отрисовке, а «чисто» — это ноль открытых окон.
///
/// Стенд открывает проекты подряд в одном процессе, и доказательство чистоты
/// обязано быть наблюдаемым ДО первого жеста: иначе утечка состояния между
/// сценариями даёт «зелёное, потому что смотрели не туда».
#[test]
fn t805_bench_channel_is_dead_without_env() {
    // 1) в продукте канала нет: переменной нет — команд нет
    assert!(
        !crate::bench::enabled(),
        "канал стенда включён без KT_LIVE_CTL — в продукте он обязан быть мёртв"
    );
    let dir = std::env::temp_dir().join("kt-t805");
    std::fs::create_dir_all(&dir).ok();
    let ctl = dir.join("ctl");
    std::fs::write(&ctl, "1 open /tmp/нет-такого.otio\n").unwrap();
    assert!(crate::bench::poll().is_none(), "выключенный канал прочитал команду");
    std::fs::remove_file(&ctl).ok();

    // 2) номер кадра растёт ОТ ОТРИСОВКИ
    let mut h = crate::layout_tests::harness();
    let before = crate::bench::frames();
    h.step();
    h.step();
    let after = crate::bench::frames();
    assert!(
        after >= before + 2,
        "счётчик кадров не считает отрисовку: {before} → {after}"
    );

    // 3) «чисто» — это ноль окон; открытое окно видно, закрытое — нет
    assert_eq!(
        crate::bench::open_windows(&h.ctx),
        0,
        "свежая программа отдала открытые окна"
    );
    h.state_mut().open_project_settings();
    h.step();
    h.step();
    assert!(
        crate::bench::open_windows(&h.ctx) >= 1,
        "открытое окно настроек не видно счётчику окон"
    );
}

/// t804 (Р.68): отчёт живого покрытия — что человек НЕ трогал мышью ни разу.
///
/// Решение владельца: «прокликивать сложные моменты или те которые не
/// прокликали ни разу». Чтобы «ни разу» не было вкусовщиной, его считает
/// машина: программа публикует прямоугольники именованных виджетов, сценарии
/// их кликают, отчёт называет разницу. Красным отчёт делают только ЛОЖНЫЕ
/// вещи: сценарий целится в несуществующее место, или спека ссылается на
/// живой сценарий, которого нет, — то есть «переезд» проверки в headless
/// выкинул её вместо замены адреса.
#[test]
fn t804_live_coverage_names_untouched_places() {
    let root = workspace_root();
    let guard = root.join("scripts/live-coverage.py");
    assert!(guard.exists(), "сторожа нет на месте: {}", guard.display());
    let run = |extra: &[&str]| -> (bool, String) {
        let out = std::process::Command::new("python3")
            .arg(&guard)
            .args(extra)
            .current_dir(&root)
            .output()
            .expect("python3 для отчёта покрытия");
        let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
        said.push_str(&String::from_utf8_lossy(&out.stderr));
        (out.status.success(), said)
    };

    // 1) сегодняшнее дерево: отчёт называет число нетронутых мест
    let (ok, said) = run(&["--check"]);
    assert!(ok, "сторож покрытия красен на текущем дереве: {said}");
    assert!(
        said.contains("мышью ни разу не трогали") || said.contains("мышью НИ РАЗУ не трогали"),
        "отчёт не назвал нетронутые места: {said}"
    );

    // 2) сценарий целится в место, которого программа не публикует
    let probe = root.join("scripts/live/_t804_probe.py");
    std::fs::write(
        &probe,
        "def main(out):\n    app.click_probe(\"нетакогощупа.вовсе\", \"место\")\n",
    )
    .unwrap();
    let (ok_phantom, said_phantom) = run(&["--check"]);
    std::fs::remove_file(&probe).ok();
    assert!(!ok_phantom, "сторож не заметил прицела в несуществующее место");
    assert!(
        said_phantom.contains("нетакогощупа"),
        "сторож не назвал несуществующее место: {said_phantom}"
    );

    // 3) живой сценарий, названный спекой, исчез с диска — красное:
    //    переезд проверки обязан МЕНЯТЬ адрес, а не выкидывать сценарий
    let victim = root.join("scripts/live/deadend.py");
    let saved = std::fs::read_to_string(&victim).expect("живой сценарий deadend.py");
    std::fs::remove_file(&victim).unwrap();
    let (ok_gone, said_gone) = run(&["--check"]);
    std::fs::write(&victim, saved).unwrap();
    assert!(!ok_gone, "сторож не заметил исчезнувшего сценария спеки");
    assert!(
        said_gone.contains("deadend.py"),
        "сторож не назвал исчезнувший сценарий: {said_gone}"
    );
}

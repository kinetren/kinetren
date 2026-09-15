//! Тесты 423-426 (docs/porting-notes.md): вложенные секвенции — модель и гарды.
//!
//! Вложение — кусок монтажа, свёрнутый в ОДНО событие. Ссылка живёт внутри
//! проекта (решение владельца Р.50), поэтому документ перестаёт быть плоским
//! списком и становится деревом. Всё, что здесь проверяется, — цена этого
//! перехода: дерево обязано оставаться деревом (цикла нет), конечным
//! (глубина ограничена), однозначным (`ClipId` уникален по документу) и
//! замкнутым по лентам (ни одна ссылка модели не пересекает границу).

use kt_model::*;
use kt_time::Tick;

const LEN: Tick = Tick(90_000);

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    }
}

fn clip(id: u64, media: u64, start: Tick, dur: Tick) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start,
        duration: dur,
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn base() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef { id: MediaId(1), path: "synth:sine:440".into() });
    p
}

/// Лента с одним событием внутри.
fn seq(id: u64, name: &str, inner: Vec<ClipEvent>) -> Sequence {
    let mut s = Sequence::new(MediaId(id), name);
    s.clips = inner;
    s
}

// ---- 423. цикл ---------------------------------------------------------------

#[test]
fn t423_cycle_is_refused_and_names_the_path() {
    // лента, положенная сама в себя
    let mut p = base();
    p.sequences.push(seq(100, "Заставка", vec![clip(10, 100, Tick(0), LEN)]));
    let err = p.validate().expect_err("лента внутри самой себя обязана быть отвергнута");
    let text = err.to_string();
    assert!(
        matches!(err, ProjectError::SequenceCycle(..)),
        "самовложение опознано не как цикл: {err}"
    );
    assert!(
        text.matches("Заставка").count() >= 2,
        "текст ошибки обязан называть ИМЕНА и показывать путь, а не адреса: {text}"
    );

    // цикл из ДВУХ лент, не поставленных в монтаж НИГДЕ. Обход идёт со всех
    // вершин: из корня такая пара недостижима, но файл с ней сохранился бы и
    // не открылся.
    let mut p = base();
    p.sequences.push(seq(100, "Заставка", vec![clip(10, 101, Tick(0), LEN)]));
    p.sequences.push(seq(101, "Титры", vec![clip(11, 100, Tick(0), LEN)]));
    assert!(p.clips.is_empty(), "пара не используется в монтаже — в этом весь смысл случая");
    let err = p.validate().expect_err("цикл A-B обязан быть отвергнут даже вне монтажа");
    let text = err.to_string();
    assert!(matches!(err, ProjectError::SequenceCycle(..)), "цикл A-B не опознан: {err}");
    assert!(
        text.contains("Заставка") && text.contains("Титры"),
        "в пути цикла обязаны быть обе ленты: {text}"
    );

    // ...а ромб (одна лента использована двумя разными) — не цикл
    let mut p = base();
    p.sequences.push(seq(100, "Кусок", vec![clip(10, 1, Tick(0), LEN)]));
    p.sequences.push(seq(101, "Слева", vec![clip(11, 100, Tick(0), LEN)]));
    p.sequences.push(seq(102, "Справа", vec![clip(12, 100, Tick(0), LEN)]));
    p.clips.push(clip(1, 101, Tick(0), LEN));
    p.clips.push(clip(2, 102, LEN, LEN));
    p.validate().expect("ромб — не цикл: одна лента законно стоит в двух местах");
    assert_eq!(p.sequence_uses(MediaId(100)), 2, "«сколько мест изменится вместе» считается неверно");
}

// ---- 424. глубина ------------------------------------------------------------

#[test]
fn t424_depth_is_capped_and_the_cap_names_the_sequence() {
    // цепочка из MAX_NEST_DEPTH лент законна
    let chain = |levels: u64| {
        let mut p = base();
        for i in 0..levels {
            let inner = if i == 0 { 1 } else { 100 + i - 1 };
            p.sequences.push(seq(100 + i, &format!("У{i}"), vec![clip(10 + i, inner, Tick(0), LEN)]));
        }
        p.clips.push(clip(1, 100 + levels - 1, Tick(0), LEN));
        p
    };
    chain(MAX_NEST_DEPTH as u64).validate().expect("предельная глубина обязана быть законной");

    let err = chain(MAX_NEST_DEPTH as u64 + 1)
        .validate()
        .expect_err("на уровень глубже предела — отказ");
    assert!(matches!(err, ProjectError::SequenceTooDeep(..)), "глубина не опознана: {err}");
    assert!(
        err.to_string().contains(&format!("У{}", MAX_NEST_DEPTH)),
        "отказ обязан назвать ленту, на которой дерево стало слишком глубоким: {err}"
    );

    // глубина считается и у ленты, НЕ поставленной никуда: ругаться надо
    // там, где её собрали, а не через неделю на драге
    let mut p = chain(MAX_NEST_DEPTH as u64 + 1);
    p.clips.clear();
    assert!(
        matches!(p.validate(), Err(ProjectError::SequenceTooDeep(..))),
        "слишком глубокая лента в бине пропущена — при первой же постановке \
         пользователь получил бы отказ на ровном месте"
    );
}

// ---- 425. одно пространство адресов, замкнутость лент -------------------------

#[test]
fn t425_document_wide_ids_and_closed_reels() {
    // клип СМОТРИТ на ленту — это законно и не «неизвестное медиа»
    let mut p = base();
    p.sequences.push(seq(100, "Кусок", vec![clip(10, 1, Tick(0), LEN)]));
    p.clips.push(clip(1, 100, Tick(0), LEN));
    p.validate().expect("событие-вложение обязано быть законным");
    assert!(p.is_nest(&p.clips[0]), "вложение не опознано");
    assert!(matches!(p.source(MediaId(100)), Some(Source::Seq(_))), "лента не находится по адресу");
    assert!(matches!(p.source(MediaId(1)), Some(Source::File(_))), "файл не находится по адресу");
    assert!(p.source(MediaId(7)).is_none(), "несуществующий адрес обязан быть None");

    // `ClipId` уникален по ВСЕМУ документу: `Retained` в kt-otio — глобальная
    // карта по id, и совпавшие id молча обменялись бы материалом
    let mut p = base();
    p.sequences.push(seq(100, "Кусок", vec![clip(10, 1, Tick(0), LEN)]));
    p.clips.push(clip(10, 100, Tick(0), LEN));
    assert!(
        matches!(p.validate(), Err(ProjectError::DuplicateClipId(ClipId(10)))),
        "одинаковый id события в корне и в ленте пропущен"
    );

    // адрес ленты и адрес файла — ОДНО пространство
    let mut p = base();
    p.sequences.push(seq(1, "Столкновение", vec![clip(10, 1, Tick(0), LEN)]));
    assert!(
        matches!(p.validate(), Err(ProjectError::DuplicateMediaId(MediaId(1)))),
        "лента с адресом существующего файла пропущена"
    );

    // имя ленты непустое: карточка без подписи в бине неотличима от соседней
    let mut p = base();
    p.sequences.push(seq(100, "  ", vec![clip(10, 1, Tick(0), LEN)]));
    assert!(matches!(p.validate(), Err(ProjectError::SequenceUnnamed(..))), "лента без имени прошла");

    // ни одна ссылка модели не пересекает границу ленты: цель морфер-слоя
    // в чужой ленте вернула бы None из joint(), и слой молча перестал бы
    // действовать
    let mut p = base();
    p.morph_lanes = 1;
    let mut s = seq(100, "Кусок", vec![clip(10, 1, Tick(0), LEN)]);
    s.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: LEN,
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: Default::default(),
        enabled: true,
        timed: true,
        strength: 1.0,
        fade_in: Tick(0),
        fade_out: Tick(0),
    });
    s.morph_lanes = 1;
    p.sequences.push(s);
    p.clips.push(clip(1, 100, Tick(0), LEN));
    assert!(
        matches!(p.validate(), Err(ProjectError::MorphUnknownTarget(..))),
        "морфер-слой дотянулся до события ЧУЖОЙ ленты — переход/слой через \
         границу молча перестаёт действовать, а не ломается заметно"
    );

    // старший бит адреса отдан лентам в движке: файл там же — общий ключ кэша
    let mut p = Project::new(settings());
    p.media.push(MediaRef { id: MediaId(NEST_TAG | 5), path: "synth:ramp".into() });
    assert!(
        matches!(p.validate(), Err(ProjectError::MediaIdTooLarge(..))),
        "адрес медиа со взведённым старшим битом пропущен"
    );
}

// ---- 426. лента не отстаёт от корня -------------------------------------------

/// Риск, который компилятор сам не закрывает: `Sequence` — частичный дубль
/// `Project`, и новое поле содержимого, заведённое в корне, в ленте забыть
/// НЕЧЕМ поймать. Деструктурирование обеих структур исчерпывающее: новое
/// поле ломает сборку теста, и в этот момент принимается решение — оно
/// содержимое ленты (завести в `Sequence`) или свойство документа (внести
/// сюда в список исключений с причиной).
#[test]
fn t426_sequence_does_not_lag_behind_the_root() {
    let p = Project::new(settings());
    let Project {
        // --- содержимое ленты: обязано быть и в Sequence ---
        clips: _,
        transitions: _,
        morphers: _,
        cameras: _,
        video_lanes: _,
        audio_lanes: _,
        morph_lanes: _,
        camera_lanes: _,
        lane_order: _,
        overlap_layers: _,
        // --- свойства ДОКУМЕНТА: своих у ленты нет по решению владельца
        // «вложение наследует настройки родителя» (Р.50) ---
        format_version: _,
        settings: _,
        media: _,
        sequences: _,
        // --- удобство и разметка: живут только в корне ---
        markers: _,
        areas: _,
        region: _,
        loop_enabled: _,
        timeline_end: _,
        trim_stretches_auto: _,
        ui: _,
        // --- чужое сырьё OTIO: внутрь ленты не попадает (А5 кладёт чужие
        // Stack настоящими лентами, а не плейсхолдерами) ---
        foreign: _,
    } = p;

    let s = Sequence::new(MediaId(100), "Кусок");
    let Sequence {
        id: _,
        name: _,
        clips: _,
        transitions: _,
        morphers: _,
        cameras: _,
        video_lanes: _,
        audio_lanes: _,
        morph_lanes: _,
        camera_lanes: _,
        lane_order: _,
        overlap_layers: _,
    } = s;

    // умолчания ленты совпадают с умолчаниями корня: пустая лента и пустой
    // проект обязаны вести себя одинаково, иначе «свернул кусок» меняет
    // картинку само по себе
    let s = Sequence::new(MediaId(100), "Кусок");
    let p = Project::new(settings());
    assert_eq!((s.video_lanes, s.audio_lanes), (p.video_lanes, p.audio_lanes));
    assert_eq!((s.morph_lanes, s.camera_lanes), (p.morph_lanes, p.camera_lanes));
    assert_eq!(s.overlap_layers, p.overlap_layers);
    assert_eq!(s.end(), Tick(0), "пустая лента обязана кончаться в нуле");
}

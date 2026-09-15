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
    assert_eq!(
        p.sequence_uses(MediaId(100)),
        2,
        "«сколько мест изменится вместе» считается неверно"
    );
}

// ---- 424. глубина ------------------------------------------------------------

#[test]
fn t424_depth_is_capped_and_the_cap_names_the_sequence() {
    // цепочка из MAX_NEST_DEPTH лент законна
    let chain = |levels: u64| {
        let mut p = base();
        for i in 0..levels {
            let inner = if i == 0 { 1 } else { 100 + i - 1 };
            p.sequences.push(seq(
                100 + i,
                &format!("У{i}"),
                vec![clip(10 + i, inner, Tick(0), LEN)],
            ));
        }
        p.clips.push(clip(1, 100 + levels - 1, Tick(0), LEN));
        p
    };
    chain(MAX_NEST_DEPTH as u64).validate().expect("предельная глубина обязана быть законной");

    let err =
        chain(MAX_NEST_DEPTH as u64 + 1).validate().expect_err("на уровень глубже предела — отказ");
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
    assert!(
        matches!(p.validate(), Err(ProjectError::SequenceUnnamed(..))),
        "лента без имени прошла"
    );

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

// ---- 427-430. команды и жесты (Р.50, п.380) -----------------------------------

/// Три события подряд на одной дорожке, кроссфейд между вторым и третьим,
/// и одно «снаружи» слева, с которым второе склеено переходом.
fn montage() -> Project {
    let mut p = base();
    p.media.push(MediaRef { id: MediaId(2), path: "synth:ramp".into() });
    for (i, start) in (0..4).map(|i| (i, Tick(i * LEN.0))) {
        // второй файл стоит РОВНО на том, что уедет внутрь ленты: так
        // проверяется, что медиа держит всё дерево, а не только корень
        let mut c = clip(10 + i as u64, if (1..=2).contains(&i) { 2 } else { 1 }, start, LEN);
        c.source_in = Tick(1_500);
        p.clips.push(c);
    }
    p
}

/// Снимок «как это выглядит и звучит»: id не сравниваем — при разворачивании
/// события законно получают свежие адреса, а тождественной обязана остаться
/// КАРТИНКА.
fn shape(p: &Project) -> Vec<(ClipKind, u32, i64, i64, i64, u64, bool)> {
    let mut v: Vec<_> = p
        .clips
        .iter()
        .map(|c| (c.kind, c.lane, c.start.0, c.duration.0, c.source_in.0, c.media.0, c.reversed))
        .collect();
    v.sort_by_key(|x| (x.0 as u8, x.1, x.2));
    v
}

/// Переходы как ГЕОМЕТРИЯ: положение стыка и обе ручки.
fn joints(p: &Project) -> Vec<(i64, i64, i64)> {
    let mut v: Vec<_> = p
        .transitions
        .iter()
        .filter_map(|t| t.joint(p).map(|j| (j.0, t.in_offset.0, t.out_offset.0)))
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn t427_collapse_then_expand_is_identity_in_one_undo_step() {
    let mut p = montage();
    // кроссфейд ВНУТРИ будущей ленты: он обязан уехать внутрь целиком и
    // вернуться на тот же стык — иначе «свернул, развернул» теряет работу
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(11)),
        b: Some(ClipId(12)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: Vec::new(),
    });
    p.validate().expect("фикстура обязана быть законной");
    let before_shape = shape(&p);
    let before_joints = joints(&p);
    let mut m = TimelineModel::new(p);

    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Кусок", n.ids());
    m.apply_batch(cmds).expect("сворачивание обязано быть законным");
    let seq_id = rep.seq.expect("лента не заведена");

    // корень: два события остались, третьим стал экземпляр
    let p = m.project();
    assert_eq!(p.clips.len(), 3, "после сворачивания в корне не то число событий");
    let inst = p.clips.iter().find(|c| c.media == seq_id).expect("экземпляра нет");
    assert_eq!((inst.start.0, inst.duration.0), (LEN.0, 2 * LEN.0), "окно экземпляра не то");
    assert_eq!(p.sequence(seq_id).map(|s| s.clips.len()), Some(2), "в ленте не два события");
    assert_eq!(
        p.sequence(seq_id).map(|s| s.transitions.len()),
        Some(1),
        "кроссфейд между свёрнутыми событиями не уехал внутрь ленты"
    );
    assert!(p.transitions.is_empty(), "кроссфейд остался и в корне — он теперь в двух местах");
    // ClipId СОХРАНЕНЫ: те же события переехали, а не родились заново —
    // на этом держатся переходы, link-группы, цели морферов и FX-состояние
    let inner: Vec<u64> = p.sequence(seq_id).unwrap().clips.iter().map(|c| c.id.0).collect();
    assert_eq!(inner, vec![11, 12], "id свёрнутых событий изменились: {inner:?}");

    // ...и ОДИН шаг истории
    m.undo().expect("сворачивание обязано отменяться одним шагом");
    assert_eq!(shape(m.project()), before_shape, "undo не вернул монтаж");
    assert!(m.project().sequences.is_empty(), "undo оставил ленту");

    // развернули обратно — тождество
    m.redo().expect("redo");
    let (cmds, rep) = ops::expand_instance(
        m.project(),
        m.project().clips.iter().find(|c| c.media == seq_id).unwrap().id,
        n.ids(),
    );
    assert!(!rep.chain_dropped && !rep.level_dropped, "снимать было нечего: {rep:?}");
    m.apply_batch(cmds).expect("разворачивание обязано быть законным");
    assert_eq!(
        shape(m.project()),
        before_shape,
        "свернул и развернул — монтаж обязан быть ТЕМ ЖЕ: вложение с пустой \
         цепочкой семантически тождественно своему содержимому (настройки \
         наследуются, промежуточного растра нет)"
    );
    assert_eq!(joints(m.project()), before_joints, "переходы не вернулись на свои стыки");
}

#[test]
fn t428_expand_drops_the_instance_chain_and_says_so() {
    let mut m = TimelineModel::new(montage());
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Кусок", n.ids());
    m.apply_batch(cmds).unwrap();
    let seq_id = rep.seq.unwrap();
    let inst = m.project().clips.iter().find(|c| c.media == seq_id).unwrap().id;

    // ЦЕПОЧКА на вложении законна с волны Б: она применяется к СОБРАННОЙ
    // картинке — ради этого вложение и заводилось
    m.apply(Command::SetFx { id: inst, fx: vec![FxInstance::Gain { db: 3.0, pan: 0.0 }] })
        .expect("цепочка на вложении законна");
    m.apply(Command::SetOpacity { id: inst, opacity: 0.5 }).expect("уровень вложения законен");
    // ...а реверс и варп времени экземпляра — ещё нет: они требуют
    // смешивания ДВУХ суб-композитов и ресемплера над суб-миксом. Такого
    // состояния просто не бывает (правило 16), а не «считается не то»
    assert!(
        matches!(
            m.apply(Command::SetReversed { id: inst, reversed: true }),
            Err(CommandError::InvalidLayout(ProjectError::NestInstanceNotChainable(_)))
        ),
        "реверс вложения прошёл молча"
    );

    let (cmds, rep) = ops::expand_instance(m.project(), inst, n.ids());
    assert!(
        rep.chain_dropped && rep.level_dropped,
        "разворачивание промолчало о снятой цепочке и уровне: {rep:?}. Снаружи они \
         применялись к СОБРАННОЙ картинке, внутри такого места нет — молча \
         раскидать их по кускам значит посчитать другое"
    );
    m.apply_batch(cmds).unwrap();
    assert!(
        m.project().clips.iter().all(|c| c.fx.is_empty()),
        "цепочка экземпляра всё-таки расползлась по кускам"
    );
}

#[test]
fn t429_a_used_sequence_refuses_to_leave() {
    let mut m = TimelineModel::new(montage());
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Кусок", n.ids());
    m.apply_batch(cmds).unwrap();
    let seq_id = rep.seq.unwrap();

    assert_eq!(
        m.apply(Command::RemoveSequence { id: seq_id }),
        Err(CommandError::SequenceInUse(seq_id, 1)),
        "занятая лента удалилась молча — события повисли бы на несуществующем адресе"
    );
    assert!(
        ops::remove_unused_sequences(m.project()).is_empty(),
        "занятая лента попала в уборку неиспользованных"
    );

    // поставили её во ВТОРОЕ место и отвязали одно из них
    let inst = m.project().clips.iter().find(|c| c.media == seq_id).unwrap().clone();
    let mut twin = inst.clone();
    twin.id = ClipId(n.clip);
    n.clip += 1;
    twin.link_group = None;
    twin.start = Tick(10 * LEN.0);
    m.apply(Command::AddClip(twin.clone())).expect("вторая постановка ленты законна");
    assert_eq!(m.project().sequence_uses(seq_id), 2, "второе место не посчиталось");

    let cmds = ops::make_independent(m.project(), twin.id, "Кусок 2", n.ids());
    m.apply_batch(cmds).expect("отвязывание обязано быть законным");
    assert_eq!(m.project().sequences.len(), 2, "своя лента не появилась");
    assert_eq!(m.project().sequence_uses(seq_id), 1, "первая лента не освободилась");
    // содержимое то же, а адреса событий — СВЕЖИЕ (иначе карты kt-otio
    // молча обменяли бы материал между лентами)
    let a: Vec<u64> = m.project().sequences[0].clips.iter().map(|c| c.id.0).collect();
    let b: Vec<u64> = m.project().sequences[1].clips.iter().map(|c| c.id.0).collect();
    assert_eq!(a.len(), b.len(), "копия ленты потеряла события");
    assert!(a.iter().all(|x| !b.contains(x)), "адреса событий копии совпали с оригиналом");

    // освободили — и уборка её видит
    m.apply(Command::RemoveClip { id: twin.id }).unwrap();
    let cmds = ops::remove_unused_sequences(m.project());
    assert_eq!(cmds.len(), 1, "освободившаяся лента не попала в уборку");
    m.apply_batch(cmds).expect("уборка обязана проходить");
}

#[test]
fn t430_outer_transition_is_rehung_and_media_is_held_by_the_whole_tree() {
    let mut m = TimelineModel::new(montage());
    // кроссфейд между СНАРУЖИ (10) и тем, что уедет внутрь (11)
    m.apply(Command::AddTransition(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(10)),
        b: Some(ClipId(11)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: Vec::new(),
    }))
    .expect("кроссфейд на стыке законен");

    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Кусок", n.ids());
    assert_eq!(rep.rehung, 1, "переход с одним концом снаружи не перевешен: {rep:?}");
    m.apply_batch(cmds).unwrap();

    let p = m.project();
    let inst = p.clips.iter().find(|c| c.media == rep.seq.unwrap()).unwrap();
    let t = p.transitions.iter().find(|t| t.id == TransitionId(1)).expect(
        "кроссфейд со свёрнутым куском умер вместе с исчезнувшим концом — \
         пользователь потерял работу молча",
    );
    assert_eq!((t.a, t.b), (Some(ClipId(10)), Some(inst.id)), "переход перевешен не туда");
    assert_eq!(t.joint(p), Some(inst.start), "стык перевешенного перехода уехал");

    // медиа держит ВСЁ ДЕРЕВО: файл, оставшийся только внутри ленты, не
    // удаляется — иначе события повисли бы, и всплыло бы это через неделю
    assert!(
        p.clips.iter().all(|c| c.media != MediaId(2)),
        "в корне не должно остаться событий на этом файле — в этом весь случай"
    );
    assert_eq!(
        m.apply(Command::RemoveMedia { id: MediaId(2) }),
        Err(CommandError::MediaInUse(MediaId(2))),
        "файл, стоящий только ВНУТРИ ленты, удалился молча"
    );
}

// ---- 453-455 (Р.50, В1): контекст редактирования -----------------------------

/// Проект: корневое событие и лента с двумя событиями внутри.
fn with_reel() -> (TimelineModel, MediaId) {
    let mut p = base();
    p.media.push(MediaRef { id: MediaId(2), path: "synth:ramp".into() });
    let mut s = Sequence::new(MediaId(100), "Кусок");
    s.clips.push(clip(20, 1, Tick(0), LEN));
    s.clips.push(clip(21, 2, LEN, LEN));
    p.sequences.push(s);
    p.clips.push(clip(1, 1, Tick(0), LEN));
    let mut inst = clip(10, 100, LEN, LEN + LEN);
    inst.source_in = Tick(0);
    p.clips.push(inst);
    p.validate().expect("фикстура законна");
    (TimelineModel::new(p), MediaId(100))
}

#[test]
fn t453_commands_edit_the_reel_you_are_standing_in() {
    let (mut m, seq) = with_reel();
    let root_before: Vec<i64> = m.document().clips.iter().map(|c| c.start.0).collect();

    m.set_editing(Some(seq));
    assert_eq!(m.editing(), Some(seq));
    // РАБОЧИЙ ВИД: под руками содержимое ленты, и весь интерфейс работает
    // с ним, не зная о вложениях вовсе
    assert_eq!(m.project().clips.len(), 2, "внутри ленты обязаны быть ЕЁ события");
    assert_eq!(m.document().clips.len(), 2, "документ обязан остаться документом");

    // правка ВНУТРИ: корень не шелохнулся, лента изменилась
    m.apply(Command::MoveClip { id: ClipId(20), new_start: Tick(LEN.0 / 2), new_lane: 0 })
        .expect("перенос внутри ленты законен");
    assert_eq!(
        m.document().clips.iter().map(|c| c.start.0).collect::<Vec<_>>(),
        root_before,
        "правка ВНУТРИ ленты уехала в корень — человек смотрит в одно место, а \
         меняется другое"
    );
    assert_eq!(
        m.document().sequence(seq).unwrap().clips[0].start.0,
        LEN.0 / 2,
        "правка не доехала до ленты"
    );

    // события КОРНЯ изнутри не видны и не адресуются
    assert!(
        matches!(
            m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(0), new_lane: 0 }),
            Err(CommandError::ClipNotFound(ClipId(1)))
        ),
        "изнутри ленты удалось адресовать событие КОРНЯ"
    );

    // вышли — и корень снова под руками
    m.set_editing(None);
    m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(LEN.0 * 9), new_lane: 0 })
        .expect("снаружи корень снова правится");
    assert_eq!(
        m.document().sequence(seq).unwrap().clips.len(),
        2,
        "лента пострадала от правки корня"
    );
}

#[test]
fn t454_undo_takes_you_back_to_where_the_edit_was() {
    let (mut m, seq) = with_reel();
    // правка СНАРУЖИ
    m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(LEN.0 * 9), new_lane: 0 }).unwrap();
    // правка ВНУТРИ
    m.set_editing(Some(seq));
    m.apply(Command::MoveClip { id: ClipId(20), new_start: Tick(LEN.0 / 2), new_lane: 0 }).unwrap();
    // вышли наружу и жмём Ctrl+Z
    m.set_editing(None);
    m.undo().unwrap();
    assert_eq!(
        m.editing(),
        Some(seq),
        "Ctrl+Z отменил правку ВНУТРИ ленты, а показал корень — для человека это \
         «нажал, и ничего не произошло»"
    );
    assert_eq!(
        m.document().sequence(seq).unwrap().clips[0].start.0,
        0,
        "правка внутри не отменена"
    );

    // следующий шаг — правка снаружи, и контекст обязан уехать наружу
    m.undo().unwrap();
    assert_eq!(m.editing(), None, "Ctrl+Z до корневой правки оставил нас внутри ленты");
    assert_eq!(m.document().clips[0].start.0, 0, "корневая правка не отменена");

    // redo ведёт обратно тем же путём
    m.redo().unwrap();
    assert_eq!(m.editing(), None, "redo корневой правки перебросил внутрь");
    m.redo().unwrap();
    assert_eq!(m.editing(), Some(seq), "redo правки внутри не вернул в ленту");
}

#[test]
fn t455_editing_context_survives_and_yields() {
    let (mut m, seq) = with_reel();
    m.set_editing(Some(seq));

    // контекст — рабочее место, а не работа: он живёт в МОДЕЛИ, а не в
    // проекте, поэтому в файл ему попасть неоткуда
    let dump = format!("{:?}", m.project());
    assert!(!dump.contains("editing"), "контекст редактирования пролез в проект");

    // лента исчезла под руками (undo сворачивания) — выходим в корень, а
    // не остаёмся в никуда
    m.set_editing(None);
    let cmds = ops::remove_unused_sequences(m.project());
    assert!(cmds.is_empty(), "лента используется — убирать нечего");
    m.apply(Command::RemoveClip { id: ClipId(10) }).unwrap();
    let cmds = ops::remove_unused_sequences(m.project());
    m.apply_batch(cmds).unwrap();
    m.set_editing(Some(seq));
    assert_eq!(m.editing(), None, "вход в исчезнувшую ленту оставил нас в никуда");
    assert_eq!(
        m.project().clips.len(),
        m.document().clips.len(),
        "лента без адреса подменила корень"
    );
}

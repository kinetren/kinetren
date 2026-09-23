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

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    }
}

fn clip(id: u64, media: u64, start: Tick, dur: Tick) -> ClipEvent {
    ClipEvent {
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
        // публичные параметры (Р.59, стадия 3) — СОДЕРЖИМОЕ ленты: у
        // корня поле есть только потому, что вход внутрь сделан обменом
        published: _,
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
        published: _,
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
        invert: false,
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
    // ...И РЕВЕРС С РАСТЯЖЕНИЕМ ТОЖЕ (Р.55, С4). До этого раунда они
    // отвергались: суб-микс читался строго по порядку, а обоим нужно
    // читать иначе. Теперь у суб-микса есть продюсер с пре-роллом, и
    // реверс с варпом действуют на СЭМПЛЫ собранного микса — ровно так
    // же, как на сэмплы файла. Это и была претензия владельца: «на
    // композицию не работает реверсы», «slowedandreverb не срабатывает
    // на прекомпозициях».
    m.apply(Command::SetReversed { id: inst, reversed: true })
        .expect("реверс вложения обязан проходить");
    let mut fx = m.project().clips.iter().find(|c| c.id == inst).unwrap().fx.clone();
    fx.push(FxInstance::slowed_reverb(0.82, 0.1, 85.0));
    m.apply(Command::SetFx { id: inst, fx })
        .expect("растягивающая цепочка на вложении обязана проходить");
    m.project().validate().expect("документ с реверснутым и замедленным вложением законен");

    let (cmds, rep) = ops::expand_instance(m.project(), inst, n.ids());
    assert!(
        rep.chain_dropped && rep.level_dropped,
        "разворачивание промолчало о снятой цепочке и уровне: {rep:?}. Снаружи они \
         применялись к СОБРАННОЙ картинке, внутри такого места нет — молча \
         раскидать их по кускам значит посчитать другое"
    );
    // РЕВЕРС и СКОРОСТЬ названы ОТДЕЛЬНО: «реверс собранного куска ≠
    // реверс каждого куска» — это реверс-аналог закона Г (t438), и свалить
    // его в общую кучу с уровнем значит не сказать главного
    assert!(rep.reverse_dropped, "снятый реверс не назван: {rep:?}");
    assert!(rep.warp_dropped, "снятая скорость не названа: {rep:?}");
    m.apply_batch(cmds).unwrap();
    assert!(
        m.project().clips.iter().all(|c| c.fx.is_empty()),
        "цепочка экземпляра всё-таки расползлась по кускам"
    );
    assert!(
        m.project().clips.iter().all(|c| !c.reversed),
        "реверс экземпляра расползся по кускам — это ДРУГОЙ монтаж"
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
        invert: false,
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

// ---- Р.55, С3: сворачивание ничего не теряет ---------------------------------

/// Тест 557: морфер-слой, уехавший в ленту, накрывает ТО ЖЕ, что накрывал
/// снаружи.
///
/// **Претензия владельца дословно:** «При создании композицию если был
/// морфер в регионе то он почему-то не попадает туда, наверное и камеры
/// тоже не попадают и генераторы, проверяй».
///
/// Слой попадал внутрь и там УМИРАЛ, и это хуже, чем не попасть: у ленты
/// не было `lane_order`, а в дефолтном порядке (видео → аудио → морферы →
/// камеры) морф-строки идут ПОСЛЕДНИМИ. Режим «всё, что ниже» требует
/// цели НИЖЕ слоя — под ним не оказывалось ничего.
///
/// Тест утверждает СЛЕДСТВИЕ: набор накрытых событий, а не наличие поля.
#[test]
fn t557_a_layer_inside_covers_what_it_covered_outside() {
    let mut p = base();
    p.clips.push(clip(11, 1, Tick::ZERO, LEN));
    p.clips.push(clip(12, 1, LEN, LEN));
    p.morph_lanes = 1;
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick::ZERO,
        duration: Tick(2 * LEN.0),
        targets: MorphTargets::Below,
        patch: MorphPatch {
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            ..Default::default()
        },
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    // строка морфера СВЕРХУ — иначе «всё, что ниже» не накрывает ничего и
    // снаружи, и сравнивать было бы не с чем
    p.lane_order = vec![(LaneKind::Morph, 0), (LaneKind::Video, 0)];
    p.validate().expect("фикстура законна");

    // что слой накрывал СНАРУЖИ: оба события получили зеркало
    let mirrored = |q: &Project| -> usize {
        q.effective_clips()
            .iter()
            .filter(|c| c.fx.iter().any(|f| matches!(f, FxInstance::Mirror { .. })))
            .count()
    };
    assert_eq!(mirrored(&p), 2, "сцена выродилась: слой не накрывает ничего и снаружи");

    let mut m = TimelineModel::new(p);
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Кусок", n.ids());
    m.apply_batch(cmds).expect("сворачивание обязано быть законным");
    let seq_id = rep.seq.expect("лента не заведена");

    assert_eq!(rep.moved_layers, 1, "слой обязан уехать внутрь и быть НАЗВАН");
    let s = m.project().sequence(seq_id).expect("лента");
    assert_eq!(s.morphers.len(), 1, "слоя нет внутри ленты");
    assert!(
        m.project().morphers.is_empty(),
        "слой остался И в корне: одно и то же наложилось бы дважды"
    );
    // ПОРЯДОК СТРОК уехал вместе с ним
    let rows = s.rows();
    let mine = rows.iter().position(|&k| k == (LaneKind::Morph, 0)).expect("строка слоя");
    let his = rows.iter().position(|&k| k == (LaneKind::Video, 0)).expect("строка видео");
    assert!(mine < his, "внутри ленты слой оказался НИЖЕ цели: {rows:?}");

    // СЛЕДСТВИЕ: внутри ленты слой накрывает те же два события
    let inside = m.project().effective_reel(Some(seq_id));
    let n_mirrored = inside
        .iter()
        .filter(|c| c.fx.iter().any(|f| matches!(f, FxInstance::Mirror { .. })))
        .count();
    assert_eq!(n_mirrored, 2, "внутри ленты слой не накрыл ничего — он уехал и умер");
}

/// Тест 558: камеры едут в ленту вместе с монтажом.
///
/// `Sequence` держит `cameras` и `camera_lanes` с самого Р.50, но
/// сворачивание их не заполняло — поле было, а класть в него было некому.
/// Событие с `Pan3d`, ездившее по камере, внутри свёрнутой ленты теряло
/// её МОЛЧА: `resolve_cam3d` при пустом списке отдаёт камеру по умолчанию.
#[test]
fn t558_cameras_travel_into_the_sequence() {
    let mut p = base();
    p.clips.push(clip(11, 1, Tick::ZERO, LEN));
    p.clips.push(clip(12, 1, LEN, LEN));
    p.camera_lanes = 1;
    p.cameras.push(CameraEvent {
        ease: true,
        id: CameraId(7),
        lane: 0,
        start: Tick::ZERO,
        duration: Tick(2 * LEN.0),
        name: "Проезд".into(),
        cam: Default::default(),
        keyframes: Vec::new(),
        auto: Vec::new(),
    });
    p.validate().expect("фикстура законна");

    let mut m = TimelineModel::new(p);
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Кусок", n.ids());
    m.apply_batch(cmds).expect("сворачивание обязано быть законным");
    let seq_id = rep.seq.expect("лента");

    assert_eq!(rep.moved_cameras, 1, "камера обязана уехать внутрь и быть НАЗВАНА");
    let s = m.project().sequence(seq_id).expect("лента");
    assert_eq!(s.cameras.len(), 1, "камеры нет внутри ленты");
    assert_eq!(s.cameras[0].id, CameraId(7), "камера родилась заново вместо переезда");
    assert_eq!(s.cameras[0].start, Tick::ZERO, "камера не ребейзнута в ось ленты");
    assert!(s.camera_lanes >= 1, "камерная строка внутри ленты не заведена — камеры не видно");
    assert!(
        s.rows().contains(&(LaneKind::Camera, 0)),
        "камерной строки нет в порядке строк ленты: {:?}",
        s.rows()
    );
    assert!(m.project().cameras.is_empty(), "камера осталась И в корне");
    m.project().validate().expect("документ после сворачивания обязан быть законным");
}

/// Тест 559: свернуть под слоем ПО ОБЛАСТИ можно, и слой остаётся снаружи.
///
/// Ссылочные режимы внутри ленты запрещены (`MorphRefInsideSequence`):
/// область и метка — навигация ПО ПРОЕКТУ, они живут в корне. А
/// `block_extras` клал такой слой ВНУТРЬ — и `validate` отвергал весь
/// батч. Свернуть выделение под слоем «по области» было нельзя ВООБЩЕ, с
/// английской ошибкой в статусбаре.
#[test]
fn t559_collapsing_under_an_area_layer_is_possible() {
    let mut p = base();
    p.clips.push(clip(11, 1, Tick::ZERO, LEN));
    p.clips.push(clip(12, 1, LEN, LEN));
    p.morph_lanes = 1;
    p.areas.push(MarkedArea {
        name: "Кусок".into(),
        start: Tick::ZERO,
        duration: Tick(2 * LEN.0),
        lanes: Vec::new(),
        color: Default::default(),
    });
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick::ZERO,
        duration: Tick(2 * LEN.0),
        targets: MorphTargets::Areas { names: vec!["Кусок".into()] },
        patch: MorphPatch {
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            ..Default::default()
        },
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    p.validate().expect("фикстура законна");

    let mut m = TimelineModel::new(p);
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) =
        ops::collapse_into_sequence(m.project(), &[ClipId(11), ClipId(12)], "Кусок", n.ids());
    m.apply_batch(cmds).expect("свернуть под ссылочным слоем обязано быть можно");
    let seq_id = rep.seq.expect("лента");

    assert_eq!(rep.kept_ref_layers, 1, "оставленный снаружи слой обязан быть НАЗВАН");
    assert_eq!(m.project().morphers.len(), 1, "ссылочный слой обязан остаться в корне");
    assert!(
        m.project().sequence(seq_id).is_some_and(|s| s.morphers.is_empty()),
        "ссылочный слой уехал внутрь — документ перестал быть законным"
    );
    m.project().validate().expect("документ после сворачивания обязан быть законным");
}

// ---- Р.55, С5: раскрытие ленты на месте --------------------------------------

/// Сцена для раскрытия: корень с двумя событиями и переходом, лента с
/// двумя событиями, своим переходом, морфер-слоем и камерой, и СВОИМ
/// порядком строк — то есть всё, что раскрытие обязано вернуть на место.
fn expandable() -> Project {
    let mut p = base();
    p.clips.push(clip(1, 1, Tick::ZERO, LEN));
    p.clips.push(clip(2, 1, Tick(3 * LEN.0), LEN));
    p.video_lanes = 1;
    p.audio_lanes = 1;

    let mut s = Sequence::new(MediaId(100), "Кусок");
    s.clips = vec![clip(11, 1, Tick::ZERO, LEN), clip(12, 1, LEN, LEN)];
    s.clips[1].lane = 1;
    s.video_lanes = 2;
    s.audio_lanes = 1;
    s.morph_lanes = 1;
    s.camera_lanes = 1;
    s.morphers.push(MorpherEvent {
        id: MorphId(5),
        lane: 0,
        start: Tick(LEN.0 / 2),
        duration: LEN,
        targets: MorphTargets::Below,
        patch: MorphPatch {
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            ..Default::default()
        },
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    s.cameras.push(CameraEvent {
        ease: true,
        id: CameraId(7),
        lane: 0,
        start: Tick::ZERO,
        duration: Tick(2 * LEN.0),
        name: "Проезд".into(),
        cam: Default::default(),
        keyframes: Vec::new(),
        auto: Vec::new(),
    });
    // СВОЙ порядок строк: морф-строка сверху, иначе слой не накрывает ничего
    s.lane_order = vec![
        (LaneKind::Morph, 0),
        (LaneKind::Video, 0),
        (LaneKind::Video, 1),
        (LaneKind::Audio, 0),
        (LaneKind::Camera, 0),
    ];
    p.sequences.push(s);

    let mut inst = clip(20, 100, Tick(LEN.0), Tick(2 * LEN.0));
    inst.source_in = Tick::ZERO;
    p.clips.push(inst);
    p.validate().expect("сцена для раскрытия законна");
    p
}

/// Тест 566 (Р.55, С5): РАСКРЫТЬ И СОБРАТЬ — тождество.
///
/// Фундамент всей стадии. Раскрытие на месте делается тем же приёмом, что
/// контекст правки в Р.50 (п.392): перекладыванием СОДЕРЖИМОГО, а не
/// параметром у шестидесяти рук, — только со смещением по времени и по
/// номеру дорожки. Весь интерфейс продолжает работать с корнем и о полосе
/// не знает; цена этого — вот это тождество, и оно обязано быть ПОБАЙТОВЫМ.
///
/// Красный здесь означает, что дизайн неверен, а не что тест придирчив.
#[test]
fn t566_expanding_and_collapsing_a_band_is_identity() {
    let before = expandable();
    let b = Band::of(&before, ClipId(20)).expect("полоса для экземпляра");
    assert_eq!(b.dt, Tick(LEN.0), "сдвиг полосы посчитан не так");

    let mut p = before.clone();
    p.merge_reel(&b);
    // события ленты ФИЗИЧЕСКИ в корне, со смещением
    assert!(p.sequence(MediaId(100)).is_some_and(|s| s.clips.is_empty()), "тело ленты не опустело");
    let inner = p.clips.iter().find(|c| c.id == ClipId(11)).expect("события ленты нет в корне");
    assert_eq!(inner.start, Tick(LEN.0), "время полосы не смещено в ось корня");
    assert_eq!(inner.lane, 1, "дорожка полосы не ушла в хвост пространства");
    assert!(p.clips.iter().any(|c| c.id == ClipId(20)), "экземпляр обязан остаться в корне");
    // и строки полосы стоят НЕРАЗРЫВНЫМ блоком под строкой экземпляра
    let rows = p.rows();
    let at = rows.iter().position(|&k| k == (LaneKind::Video, 0)).expect("строка экземпляра");
    // блок стоит В СВОЁМ ПОРЯДКЕ (морф-строка сверху — так его завёл
    // человек внутри ленты), только номера сдвинуты в хвост пространства
    let want: Vec<(LaneKind, u32)> = before
        .sequence(MediaId(100))
        .unwrap()
        .rows()
        .into_iter()
        .map(|(k, l)| (k, l + b.base.get(k)))
        .collect();
    for (j, r) in want.iter().enumerate() {
        assert_eq!(rows.get(at + 1 + j), Some(r), "строки полосы не встали блоком: {rows:?}");
    }

    p.unmerge_reel(&b).expect("сборка обратно обязана пройти");
    assert_eq!(p, before, "раскрыть и собрать — НЕ тождество, и весь приём неверен");
}

/// Тест 567 (Р.55, С5): через границу полосы не уедешь.
///
/// Это и есть ответ на риск, который п.392 называет словами «пропусти
/// одну, и правка молча уедет в корень». Адресация правок не нужна: уехать
/// физически нечем, а попытку ловит сборка — и батч отвергается ЦЕЛИКОМ,
/// а не «доправляется».
#[test]
fn t567_an_event_cannot_change_reels_inside_a_band() {
    let p0 = expandable();
    let b = Band::of(&p0, ClipId(20)).expect("полоса");

    // событие ПОЛОСЫ утащили на корневую дорожку
    let mut p = p0.clone();
    p.merge_reel(&b);
    if let Some(c) = p.clips.iter_mut().find(|c| c.id == ClipId(11)) {
        c.lane = 0;
    }
    let mut q = p.clone();
    assert!(
        q.unmerge_reel(&b).is_ok(),
        "уехавшее в корень событие обязано просто остаться в корне — там оно законно"
    );
    assert!(
        q.sequence(MediaId(100)).is_some_and(|s| s.clips.len() == 1),
        "из ленты обязано уехать ровно одно событие"
    );

    // а вот ЭКЗЕМПЛЯР внутрь собственной ленты — никогда
    let mut p = p0.clone();
    p.merge_reel(&b);
    if let Some(c) = p.clips.iter_mut().find(|c| c.id == ClipId(20)) {
        c.lane = 1;
    }
    assert!(
        matches!(p.unmerge_reel(&b), Err(ProjectError::BandEscape(ClipId(20)))),
        "экземпляр уехал внутрь собственной ленты — она стала бы содержать саму себя"
    );

    // корневая строка ПОСРЕДИ блока раскрытия — не ошибка: блок ставится
    // заново при каждой пересборке вида, а относительный порядок и
    // корневых строк, и строк ленты при делении сохраняется
    let mut p = p0.clone();
    p.merge_reel(&b);
    let mut rows = p.rows();
    let first_band = rows.iter().position(|&(k, l)| b.owns(k, l)).expect("строка полосы");
    let root_row = rows.iter().rposition(|&(k, l)| !b.owns(k, l)).expect("корневая строка");
    let moved = rows.remove(root_row);
    rows.insert(first_band + 1, moved);
    p.lane_order = rows;
    p.unmerge_reel(&b).expect("разрыв блока ронять батч не имеет права");
    assert_eq!(
        p.sequence(MediaId(100)).map(|s| s.rows()),
        p0.sequence(MediaId(100)).map(|s| s.rows()),
        "порядок строк ЛЕНТЫ поехал от чужой строки посреди блока"
    );
}

/// Тест 568 (Р.55, С5): вставка и удаление дорожки НЕ РВУТ полосу.
///
/// Единственное место конструкции, где правильность держится на человеке,
/// а не на построении, — `Band::after_batch`. Ошибка там не падает ни
/// одним ассертом сама: полоса «съезжает», и сборка начинает отвергать
/// батчи, а при симметричной ошибке — молча кладёт событие не в ту ленту.
///
/// Утверждение одно и оно сильное: после ЛЮБОЙ лейн-команды тождество
/// t566 обязано остаться верным.
#[test]
fn t568_inserting_and_removing_lanes_does_not_break_a_band() {
    // корневая дорожка НИЖЕ начала полосы — полоса едет вниз целиком
    for (name, cmd) in [
        ("видео-дорожка в корне", Command::InsertLane { kind: ClipKind::Video, at: 0 }),
        ("аудио-дорожка в корне", Command::InsertLane { kind: ClipKind::Audio, at: 0 }),
        ("морф-строка в корне", Command::InsertMorphLane { at: 0 }),
        ("камерная строка в корне", Command::InsertCameraLane { at: 0 }),
    ] {
        let mut m = TimelineModel::new(expandable());
        assert!(m.expand_band(ClipId(20)), "полоса не раскрылась");
        let before_doc = m.document();
        m.apply(cmd).unwrap_or_else(|e| panic!("{name}: команда не прошла: {e:?}"));
        // СЛЕДСТВИЕ: содержимое ленты на месте и не перемешалось с корнем
        let doc = m.document();
        let s0 = before_doc.sequence(MediaId(100)).unwrap();
        let s1 = doc.sequence(MediaId(100)).unwrap();
        assert_eq!(
            s1.clips.iter().map(|c| c.id).collect::<Vec<_>>(),
            s0.clips.iter().map(|c| c.id).collect::<Vec<_>>(),
            "{name}: содержимое ленты поехало"
        );
        assert_eq!(s1.clips[0].start, s0.clips[0].start, "{name}: время в ленте поехало");
        assert_eq!(s1.clips[0].lane, s0.clips[0].lane, "{name}: дорожка в ленте поехала");
        assert!(doc.clips.iter().any(|c| c.id == ClipId(20)), "{name}: экземпляр пропал из корня");
        // и свернуть обратно всё ещё можно
        m.collapse_all_bands();
        assert_eq!(
            m.project().sequence(MediaId(100)).map(|s| s.clips.len()),
            Some(2),
            "{name}: после свёртки лента не собралась"
        );
    }
}

/// Тест 569 (Р.55, С5): правка В ПОЛОСЕ доезжает до ЛЕНТЫ, а правка в
/// корне — до корня.
///
/// Это ответ на «как адресуются правки»: никак. Событие полосы физически
/// лежит в корневом векторе рабочего вида, команда приходит на него
/// обычным путём, а сборка вычитает смещение. Проверяется СЛЕДСТВИЕ — что
/// лежит в `document()`, а не какие функции вызвались.
#[test]
fn t569_an_edit_inside_a_band_lands_in_the_reel() {
    let mut m = TimelineModel::new(expandable());
    assert!(m.expand_band(ClipId(20)));
    // событие ПОЛОСЫ видно в рабочем виде — со смещением
    let inner = m.project().clips.iter().find(|c| c.id == ClipId(11)).expect("события полосы нет");
    let was = inner.start;
    assert_eq!(was, Tick(LEN.0), "полоса не смещена в ось корня");

    m.apply(Command::MoveClip {
        id: ClipId(11),
        new_start: was + Tick(1_000),
        new_lane: inner.lane,
    })
    .expect("правка в полосе обязана проходить");

    // СЛЕДСТВИЕ: сдвинулось событие ЛЕНТЫ, в её собственной оси
    let doc = m.document();
    let s = doc.sequence(MediaId(100)).expect("лента");
    let moved = s.clips.iter().find(|c| c.id == ClipId(11)).expect("события нет в ленте");
    assert_eq!(moved.start, Tick(1_000), "правка не доехала в ленту, в её ось");
    assert!(
        !doc.clips.iter().any(|c| c.id == ClipId(11)),
        "событие ленты осталось в КОРНЕ документа — на диск уехал бы другой монтаж"
    );

    // а правка КОРНЕВОГО события — в корень
    m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(5_000), new_lane: 0 })
        .expect("правка в корне обязана проходить");
    let doc = m.document();
    assert_eq!(
        doc.clips.iter().find(|c| c.id == ClipId(1)).map(|c| c.start),
        Some(Tick(5_000)),
        "правка корневого события не доехала до корня"
    );

    // Ctrl+Z снимает ЕЁ одним шагом и оставляет полосу раскрытой
    m.undo().expect("undo");
    assert_eq!(m.bands().len(), 1, "undo свернул раскрытие — правка пропала бы из виду");
}

// ---- Р.55, С6: вставка ЧУЖОГО проекта композицией ---------------------------

/// Чужой проект: два события, переход между ними, своя вложенная лента и
/// СВОИ области с метками — то, что приехать НЕ должно.
fn foreign() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef { id: MediaId(1), path: "synth:sine:440".into() });
    // ОДИН из путей совпадает с тем, что уже стоит у нас: он обязан быть
    // УЗНАН, а не приехать вторым экземпляром
    p.media.push(MediaRef { id: MediaId(2), path: "foreign:only".into() });
    let mut inner = Sequence::new(MediaId(50), "Вложенная чужая");
    inner.clips = vec![clip(90, 1, Tick::ZERO, LEN)];
    p.sequences.push(inner);
    p.clips.push(clip(1, 1, Tick::ZERO, LEN));
    p.clips.push(clip(2, 2, LEN, LEN));
    p.clips.push(clip(3, 50, Tick(2 * LEN.0), LEN));
    p.areas.push(MarkedArea {
        name: "Чужая область".into(),
        start: Tick::ZERO,
        duration: LEN,
        lanes: Vec::new(),
        color: Default::default(),
    });
    p.markers.push(Marker {
        t: Tick(LEN.0),
        duration: Tick::ZERO,
        name: "чужая метка".into(),
        color: Default::default(),
    });
    p.validate().expect("чужой проект законен");
    p
}

/// Тест 574 (Р.55, С6): чужой проект вставляется КОМПОЗИЦИЕЙ.
///
/// **Претензия владельца дословно:** «я переношу файл проекта на таймлайн и
/// открывается новый таймлайн, а я хочу чтобы оно дало мне выбор — открыть
/// как проект или вставить как композицию».
///
/// Проверяется СЛЕДСТВИЕ: документ после вставки законен, адреса не
/// столкнулись, вложенные ленты чужого проекта приехали вместе с ним, а
/// медиа, уже стоящее у нас, УЗНАНО и не задвоилось.
#[test]
fn t574_a_foreign_project_is_inserted_as_a_composition() {
    let mut dst = base();
    dst.clips.push(clip(1, 1, Tick::ZERO, LEN));
    dst.validate().expect("свой проект законен");
    let mine_media = dst.media.len();

    let mut m = TimelineModel::new(dst);
    let doc = m.document();
    let mut n = ops::NestCounters::of(&doc);
    let (cmds, rep) =
        ops::import_as_sequences(m.project(), &foreign(), "Чужой", Tick(5 * LEN.0), 0, n.ids());
    assert!(!cmds.is_empty(), "вставка не дала команд");
    m.apply_batch(cmds).expect("вставка чужого проекта обязана быть законной");
    m.document().validate().expect("документ после вставки обязан быть законным");

    let seq = rep.seq.expect("лента-корень чужого проекта");
    assert_eq!(rep.inner_seqs, 1, "вложенная лента чужого проекта не приехала: {rep:?}");
    // МЕДИА: совпавший путь узнан, чужой заведён
    assert_eq!(rep.media_reused, 1, "уже стоящее медиа не узнано и приедет вторым: {rep:?}");
    assert_eq!(rep.media_added, 1, "чужое медиа не заведено: {rep:?}");
    assert_eq!(
        m.document().media.len(),
        mine_media + 1,
        "медиа задвоилось: у одного файла был бы свой воркер, своя строка кэша и свои тумбы"
    );

    // ЭКЗЕМПЛЯР встал туда, куда бросили
    let p = m.document();
    let inst = p.clips.iter().find(|c| c.media == seq).expect("экземпляра нет");
    assert_eq!(inst.start, Tick(5 * LEN.0), "экземпляр встал не в точку дропа");
    assert_eq!(inst.source_in, Tick::ZERO);
    // содержимое на месте
    let s = p.sequence(seq).expect("лента");
    assert_eq!(s.clips.len(), 3, "содержимое чужого корня приехало не целиком");

    // ОБЛАСТИ И МЕТКИ чужого проекта НЕ приехали: это навигация по ЕГО
    // документу, и подменять ими свои нельзя
    assert!(p.areas.is_empty(), "чужая область подменила бы навигацию по своему проекту");
    assert!(p.markers.is_empty(), "чужая метка подменила бы навигацию по своему проекту");
    assert_eq!(rep.dropped_extras, 2, "оставленное снаружи обязано быть НАЗВАНО: {rep:?}");
}

/// Тест 575 (Р.55, С6): адреса чужого проекта не сталкиваются со своими.
///
/// `ClipId` уникален по ВСЕМУ документу (t425). Приедь чужой проект со
/// своими номерами — карты `Retained` в kt-otio молча обменяли бы материал,
/// стартовый таймкод и чужие эффекты между лентами (тот же класс, что
/// п.379).
#[test]
fn t575_imported_addresses_never_collide() {
    let mut dst = base();
    // НАРОЧНО те же номера, что у чужого проекта
    dst.clips.push(clip(1, 1, Tick::ZERO, LEN));
    dst.clips.push(clip(2, 1, LEN, LEN));
    dst.clips[1].lane = 1;
    dst.video_lanes = 2;
    dst.validate().unwrap();

    let mut m = TimelineModel::new(dst);
    let doc = m.document();
    let mut n = ops::NestCounters::of(&doc);
    let (cmds, _) =
        ops::import_as_sequences(m.project(), &foreign(), "Чужой", Tick(9 * LEN.0), 0, n.ids());
    m.apply_batch(cmds).expect("вставка обязана быть законной");

    // каждый адрес встречается РОВНО ОДИН раз по всему документу
    let p = m.document();
    let mut ids: Vec<u64> = p
        .clips
        .iter()
        .chain(p.sequences.iter().flat_map(|s| s.clips.iter()))
        .map(|c| c.id.0)
        .collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total, "адреса событий столкнулись после вставки чужого проекта");
    let mut mids: Vec<u64> =
        p.media.iter().map(|m| m.id.0).chain(p.sequences.iter().map(|s| s.id.0)).collect();
    let mt = mids.len();
    mids.sort_unstable();
    mids.dedup();
    assert_eq!(mids.len(), mt, "адреса медиа и лент столкнулись после вставки");
    p.validate().expect("документ после вставки законен");
}

// ---- Р.56, п.449: порядок строк и раскрытая полоса ---------------------------

/// Тест 591 (Р.56, п.449). Бенч раскрытия ВЫВЕДЕН из-под инварианта
/// «номер дорожки = ранг строки», а команды, которые этот инвариант
/// применяют, при открытой полосе отвергаются целиком.
///
/// Полоса намеренно занимает ХВОСТ числового пространства дорожек, а её
/// видимое место задаёт только `lane_order` — то есть в рабочем виде
/// номера и строки расходятся ПО ПОСТРОЕНИЮ. Перенумеруй мы бенч, и
/// `unmerge_reel` разложил бы события не по тем лентам, причём молча.
#[test]
fn t591_the_bench_is_exempt_and_row_commands_are_refused() {
    let mut m = TimelineModel::new(expandable());
    assert!(m.expand_band(ClipId(20)), "полоса не раскрылась");

    // рабочий вид НЕ канонический — и это законно
    assert!(
        !rows_are_canonical(&m.project().rows()),
        "сцена выродилась: в бенче строки и так по возрастанию, тест ничего не утверждает"
    );
    // а канон — канонический, и валидация его принимает
    assert!(rows_are_canonical(&m.document().rows()));
    m.document().validate().expect("канон обязан оставаться валидным при открытой полосе");

    // обычная правка в полосе проходит
    let inner = m.project().clips.iter().find(|c| c.id == ClipId(11)).unwrap().clone();
    m.apply(Command::MoveClip {
        id: ClipId(11),
        new_start: inner.start + Tick(1_000),
        new_lane: inner.lane,
    })
    .expect("правка в полосе обязана проходить");

    // а команды строк — нет, и отказ ИМЕННОЙ
    let rows = m.project().rows();
    assert!(
        matches!(m.apply(Command::SetLaneOrder { order: rows }), Err(CommandError::BandsOpen)),
        "порядок строк при открытой полосе обязан быть отвергнут"
    );
    assert!(
        matches!(m.apply(Command::SetLanes { video: 2, audio: 1 }), Err(CommandError::BandsOpen)),
        "число дорожек при открытой полосе обязано быть отвергнуто"
    );
    assert_eq!(m.bands().len(), 1, "отказ не имеет права свернуть раскрытие сам");
}

/// Тест 592 (Р.56, п.449). Вставка дорожки В ХВОСТ полосы достаётся
/// ПОЛОСЕ: за ней ничего не лежит по построению, а такая вставка — это
/// «ленте нужна ещё одна дорожка» (вставка событий в раскрытии).
///
/// До Р.56 хвостовая дорожка не доставалась никому: `base + len` не
/// попадало ни в одну ветку, и событие на ней молча уезжало в корень.
#[test]
fn t592_a_lane_at_the_band_tail_belongs_to_the_band() {
    let mut m = TimelineModel::new(expandable());
    assert!(m.expand_band(ClipId(20)));
    let base = m.bands()[0].base.video;
    let len = m.bands()[0].len.video;

    m.apply(Command::InsertLane { kind: ClipKind::Video, at: base + len })
        .expect("вставка в хвост полосы обязана проходить");
    assert_eq!(m.bands()[0].len.video, len + 1, "хвостовая дорожка не досталась полосе");

    // СЛЕДСТВИЕ: событие на новой дорожке уезжает В ЛЕНТУ, а не в корень
    let mut c = clip(31, 1, Tick(LEN.0 / 2), LEN);
    c.lane = base + len;
    m.apply(Command::AddClip(c)).expect("событие на новой дорожке");
    let doc = m.document();
    let s = doc.sequence(MediaId(100)).expect("лента");
    assert!(s.clips.iter().any(|c| c.id == ClipId(31)), "событие не доехало в ленту");
    assert!(!doc.clips.iter().any(|c| c.id == ClipId(31)), "событие осталось в корне");
}

/// Тест 598 (Р.56, п.452): лента КОРНЕМ — документ, который можно считать
/// офлайн.
///
/// Службе превью ленты нужен проект, где лента стоит на месте корня: код
/// композиции один на всю программу (инвариант 7), и другого входа у него
/// нет. Утверждается СЛЕДСТВИЕ: содержимое то же, документ законен, а
/// самой ленты в списке больше нет — иначе она ссылалась бы на себя.
#[test]
fn t598_reel_as_root_is_a_valid_document() {
    let p = expandable();
    let seq = MediaId(100);
    let inner = p.sequence(seq).unwrap().clips.iter().map(|c| c.id).collect::<Vec<_>>();
    let n_morph = p.sequence(seq).unwrap().morphers.len();

    let root = p.reel_as_root(seq).expect("лента есть");
    assert_eq!(root.clips.iter().map(|c| c.id).collect::<Vec<_>>(), inner, "содержимое не то");
    assert_eq!(root.morphers.len(), n_morph, "слой ленты не поехал корнем");
    assert!(!root.sequences.iter().any(|s| s.id == seq), "лента осталась ссылкой на себя");
    root.validate().expect("лента корнем обязана быть законным документом");

    // исходный проект не тронут — служба считает по КОПИИ
    assert_eq!(p.sequence(seq).unwrap().clips.len(), inner.len());
    assert!(p.reel_as_root(MediaId(999)).is_none(), "несуществующая лента");
}

// ---- Р.58: полоса ВНУТРИ полного входа -------------------------------------

/// Тест 635 (Р.58, п.460): раскрытие на месте работает внутри полного входа.
///
/// Претензия владельца дословно: «открывая композицию не могу раскрыть в ней
/// вложенную композицию». Модель этого никогда не запрещала: после
/// `swap_reel` канон — это текущая лента корнем, `Band::of` считает базы от
/// НЕЁ, а не от документа. Сторожа стояли в приложении с неверным
/// комментарием «полоса считается от корня». Тест прибивает три вещи: базы
/// полосы — от дорожек ленты, в которой стоим; раскрыть и свернуть —
/// тождество рабочего вида; undo-снимок несёт и контекст, и полосу.
#[test]
fn t635_a_band_inside_an_entered_reel_counts_from_that_reel() {
    let mut p = base();
    p.video_lanes = 1;
    p.audio_lanes = 1;
    // Y — лента-внучка
    let mut y = Sequence::new(MediaId(101), "Внучка");
    y.clips = vec![clip(31, 1, Tick::ZERO, LEN), clip(32, 1, LEN, LEN)];
    y.video_lanes = 1;
    y.audio_lanes = 1;
    // X — лента с экземпляром Y на ВТОРОЙ дорожке: базы обязаны выйти
    // равными числу дорожек X, а не корня
    let mut x = Sequence::new(MediaId(100), "Дочка");
    x.clips.push(clip(21, 1, Tick::ZERO, LEN));
    let mut yi = clip(22, 101, LEN, Tick(2 * LEN.0));
    yi.lane = 1;
    x.clips.push(yi);
    x.video_lanes = 2;
    x.audio_lanes = 2;
    p.sequences.push(y);
    p.sequences.push(x);
    p.clips.push(clip(1, 1, Tick::ZERO, LEN));
    p.clips.push(clip(10, 100, LEN, Tick(3 * LEN.0)));
    p.validate().expect("fixture is legal");

    let mut m = TimelineModel::new(p);
    m.set_editing(Some(MediaId(100)));
    let canon_before = m.document();
    let view_before = m.project().clone();

    assert!(m.expand_band(ClipId(22)), "a band inside an entered reel must open");
    let b = &m.bands()[0];
    assert_eq!(
        (b.base.video, b.base.audio),
        (2, 2),
        "band bases must count the lanes of the reel we stand in, not the root"
    );
    assert_eq!(b.dt, LEN, "band time offset is the instance start minus source_in");
    let inner = m.project().clips.iter().find(|c| c.id == ClipId(31)).expect("band content");
    assert_eq!((inner.lane, inner.start), (2, LEN), "band content sits after X's lanes, shifted");
    assert_eq!(m.document(), canon_before, "expanding changed the DOCUMENT");

    // правка в полосе доезжает до Y — не до X и не до корня
    m.apply(Command::MoveClip { id: ClipId(31), new_start: LEN + Tick(1_000), new_lane: 2 })
        .expect("an edit inside the band must pass");
    let doc = m.document();
    assert_eq!(doc.sequence(MediaId(101)).unwrap().clips[0].start, Tick(1_000), "edit missed Y");
    assert!(
        !doc.sequence(MediaId(100)).unwrap().clips.iter().any(|c| c.id == ClipId(31))
            && !doc.clips.iter().any(|c| c.id == ClipId(31)),
        "the event of Y leaked into X or into the root"
    );

    // undo-снимок несёт КОНТЕКСТ и ПОЛОСУ: выйти в корень (полосы чистит
    // модель — это её инвариант, восстановление — дело приложения), Ctrl+Z
    // — и мы снова внутри X с раскрытой Y
    m.set_editing(None);
    assert!(m.bands().is_empty(), "set_editing must clear bands (model invariant)");
    m.undo().expect("undo");
    assert_eq!(m.editing(), Some(MediaId(100)), "undo did not bring the context back");
    assert_eq!(
        m.bands().iter().map(|b| b.inst).collect::<Vec<_>>(),
        vec![ClipId(22)],
        "undo did not bring the band back"
    );
    assert_eq!(m.document(), canon_before, "undo did not revert the edit");

    // свернуть — ПОБАЙТОВОЕ тождество рабочего вида (как t566 в корне)
    assert!(m.collapse_band(ClipId(22)));
    assert_eq!(m.project(), &view_before, "expand + collapse inside a reel is not identity");
    assert_eq!(m.document(), canon_before);
}

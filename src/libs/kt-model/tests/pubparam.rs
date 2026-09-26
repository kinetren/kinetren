//! Тесты 671-673 и 690 (docs/porting-notes.md): публичные параметры
//! композиции и переопределения у экземпляра (Р.59, стадия 3).
//!
//! Владелец: «кто-то создал проект-шаблон для новостного выпуска… очевидно
//! не будут же люди каждый раз лезть в композицию и менять там текст
//! логотипы». Отсюда две половины: объявленный автором набор ручек у ЛЕНТЫ
//! и значения этих ручек у ЭКЗЕМПЛЯРА.

use kt_model::*;
use kt_time::Tick;

const FPS: u32 = 30;
const FRAME: i64 = 90_000 / FPS as i64;
const LEN: Tick = Tick(FRAME * 30);

const SEQ: MediaId = MediaId(100);
const TITLE: ClipId = ClipId(1);
const PLATE: ClipId = ClipId(2);
const INST_A: ClipId = ClipId(10);
const INST_B: ClipId = ClipId(11);
const HEADLINE: PubId = PubId(1);
const BLUR: PubId = PubId(2);
const LOGO: PubId = PubId(3);

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: FPS,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
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
        source_in: Tick::ZERO,
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: Vec::new(),
        opacity: 1.0,
        fx_off: false,
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
    }
}

fn title_kind(text: &str) -> GeneratorKind {
    let mut k = GeneratorKind::default_for_tag("text").expect("вид «текст» есть в реестре");
    k.set_value("text", &ParamValue::Text(text.into()));
    k
}

/// Шаблон новостного выпуска: титр-заголовок и подложка с размытием.
fn template() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&title_kind(""))));
    p.media.push(MediaRef::new(MediaId(2), "synth:2"));
    p.media.push(MediaRef::new(MediaId(3), "synth:3"));

    let mut s = Sequence::new(SEQ, "Выпуск");
    let mut title = clip(TITLE.0, 1, Tick::ZERO, LEN);
    title.r#gen = Some(title_kind("Заголовок по умолчанию"));
    s.clips.push(title);
    let mut plate = clip(PLATE.0, 2, Tick::ZERO, LEN);
    plate.lane = 1;
    plate.fx = vec![FxInstance::Blur { radius: 8, radius_y: 0 }];
    s.clips.push(plate);
    s.ensure_tracks(LaneKind::Video, 2);
    s.published = vec![
        PublishedParam {
            id: HEADLINE,
            name: "Заголовок новости".into(),
            target: ParamAddr::new(TITLE, ParamOwner::Generator, "text"),
            ty: ParamType::Text { multiline: false },
            range: PubRange::default(),
        },
        PublishedParam {
            id: BLUR,
            name: "Размытие подложки".into(),
            target: ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "radius"),
            ty: ParamType::Num,
            range: PubRange { min: Some(2.0), max: Some(16.0) },
        },
        PublishedParam {
            id: LOGO,
            name: "Логотип".into(),
            target: ParamAddr::new(PLATE, ParamOwner::ClipAttr, "media"),
            ty: ParamType::Media,
            range: PubRange::default(),
        },
    ];
    p.sequences.push(s);

    let mut a = clip(INST_A.0, SEQ.0, Tick::ZERO, LEN);
    a.source_in = Tick::ZERO;
    p.clips.push(a);
    let mut b = clip(INST_B.0, SEQ.0, LEN, LEN);
    b.source_in = Tick::ZERO;
    p.clips.push(b);
    p.validate().expect("шаблон обязан быть законным");
    p
}

fn text_of(p: &Project, inst: &[Override]) -> String {
    let r = p.resolve_reel(Some(SEQ), inst);
    let c = r.clips.iter().find(|c| same_clip(c.id, TITLE)).expect("титр на месте");
    match c.r#gen.as_ref().and_then(|g| g.get_value("text")) {
        Some(ParamValue::Text(s)) => s,
        other => panic!("у титра не текст: {other:?}"),
    }
}

// ---- 671. команды, чтение шаблона и действующее значение ---------------------

/// Тест 671 (Р.59, стадия 3): шесть команд и два вопроса, на которые
/// обязана отвечать модель, — «что стоит в шаблоне» и «что играет здесь».
#[test]
fn t671_publishing_and_overriding_go_through_commands() {
    let mut p = template();
    // начинаем с чистой ленты: публикуем КОМАНДОЙ, изнутри ленты
    p.sequence_mut(SEQ).unwrap().published.clear();
    let mut m = TimelineModel::new(p);
    m.set_editing(Some(SEQ));

    let head = PublishedParam {
        id: HEADLINE,
        name: "Заголовок новости".into(),
        target: ParamAddr::new(TITLE, ParamOwner::Generator, "text"),
        ty: ParamType::Text { multiline: false },
        range: PubRange::default(),
    };
    let blur = PublishedParam {
        id: BLUR,
        name: "Размытие подложки".into(),
        target: ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "radius"),
        ty: ParamType::Num,
        range: PubRange { min: Some(2.0), max: Some(16.0) },
    };
    m.apply_batch(vec![
        Command::PublishParam { seq: SEQ, param: Box::new(head.clone()) },
        Command::PublishParam { seq: SEQ, param: Box::new(blur.clone()) },
    ])
    .expect("публикация двух живых целей законна");

    // дубль номера — отказ, а не молчаливая перезапись
    assert!(
        m.apply_batch(vec![Command::PublishParam { seq: SEQ, param: Box::new(head.clone()) }]).is_err(),
        "два объявления с одним номером обязаны быть отказом: номер — это АДРЕС"
    );
    // цель, которой нет, — тоже отказ
    let ghost = PublishedParam {
        target: ParamAddr::new(TITLE, ParamOwner::FxSlot { slot: 7 }, "radius"),
        id: PubId(9),
        ..head.clone()
    };
    assert!(
        m.apply_batch(vec![Command::PublishParam { seq: SEQ, param: Box::new(ghost) }]).is_err(),
        "публикация на несуществующий слот обязана быть отказом: ручка, которая ни \
         на что не действует, — это минус, который разведка назвала прямо"
    );

    // порядок — часть авторского интерфейса
    m.apply_batch(vec![Command::ReorderPublished { seq: SEQ, order: vec![BLUR, HEADLINE] }])
        .expect("перестановка законна");
    assert_eq!(
        m.project().published.iter().map(|x| x.id).collect::<Vec<_>>(),
        vec![BLUR, HEADLINE],
        "порядок ручек не применился"
    );
    assert!(
        m.apply_batch(vec![Command::ReorderPublished { seq: SEQ, order: vec![BLUR] }]).is_err(),
        "порядок обязан быть ПЕРЕСТАНОВКОЙ: неполный список молча потерял бы ручку"
    );

    // переименование и сужение предела
    let renamed =
        PublishedParam { name: "Заголовок выпуска".into(), ..head.clone() };
    m.apply_batch(vec![Command::SetPublished { seq: SEQ, param: Box::new(renamed) }])
        .expect("переименование законно");
    assert_eq!(
        m.project().published.iter().find(|x| x.id == HEADLINE).map(|x| x.name.as_str()),
        Some("Заголовок выпуска"),
        "имя автора не доехало"
    );

    // выходим наружу — дальше правим ЭКЗЕМПЛЯРЫ
    m.set_editing(None);
    let p = m.project().clone();
    assert_eq!(
        p.published_value(SEQ, HEADLINE),
        Some(ParamValue::Text("Заголовок по умолчанию".into())),
        "значение ШАБЛОНА читается у цели живьём — хранить его вторым экземпляром нельзя"
    );

    let ov = Override::value(HEADLINE, ParamValue::Text("Второй выпуск".into()));
    m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(ov) }])
        .expect("переопределение опубликованного законно");
    let p = m.project().clone();
    let a = p.clips.iter().find(|c| c.id == INST_A).unwrap();
    let b = p.clips.iter().find(|c| c.id == INST_B).unwrap();
    assert_eq!(
        p.instance_value(a, HEADLINE),
        Some(ParamValue::Text("Заголовок по умолчанию".into())),
        "экземпляр без переопределения обязан играть ШАБЛОН"
    );
    assert_eq!(
        p.instance_value(b, HEADLINE),
        Some(ParamValue::Text("Второй выпуск".into())),
        "переопределение — ЕДИНСТВЕННОЕ законное расхождение экземпляра с лентой"
    );
    assert_eq!(text_of(&p, &b.overrides), "Второй выпуск", "значение не доехало до содержимого");
    assert_eq!(text_of(&p, &a.overrides), "Заголовок по умолчанию");

    // переопределить НЕопубликованное нельзя (решение владельца)
    let stray = Override::value(PubId(77), ParamValue::Text("нет такой ручки".into()));
    assert!(
        m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(stray) }]).is_err(),
        "переопределяется ТОЛЬКО опубликованное: иначе шаблон теряет интерфейс"
    );
    // и не того типа — тоже нельзя
    let wrong = Override::value(HEADLINE, ParamValue::Num(1.0));
    assert!(
        m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(wrong) }]).is_err(),
        "тип значения обязан совпадать с объявленным"
    );

    // сужение предела — часть ОБЪЯВЛЕНИЯ, а не украшение окна
    let hard = Override::value(BLUR, ParamValue::Num(40.0));
    m.apply_batch(vec![Command::SetOverride { id: INST_A, ov: Box::new(hard) }])
        .expect("число в поле законно, зажимать его — дело разрешения");
    let p = m.project().clone();
    let a = p.clips.iter().find(|c| c.id == INST_A).unwrap();
    let got = p.resolve_reel(Some(SEQ), &a.overrides);
    let plate = got.clips.iter().find(|c| same_clip(c.id, PLATE)).unwrap();
    let radius = plate.fx[0].get_param("radius").unwrap();
    assert!(
        (radius - 16.0).abs() < 1e-9,
        "предел автора обязан зажимать значение экземпляра: получено {radius}"
    );

    // сброс строки
    m.apply_batch(vec![Command::ClearOverride { id: INST_B, pub_id: HEADLINE }])
        .expect("сброс законен");
    let p = m.project().clone();
    let b = p.clips.iter().find(|c| c.id == INST_B).unwrap();
    assert!(b.overrides.is_empty(), "сброс не убрал переопределение");
    assert_eq!(text_of(&p, &b.overrides), "Заголовок по умолчанию", "сброс не вернул шаблон");
}

// ---- 672. судьба цели ---------------------------------------------------------

/// Тест 672 (Р.59, стадия 3): опубликованный адрес приклеен к `ClipId`.
///
/// Переживает трим и ЛЕВУЮ половину разреза (кусок №0 держит id — то же
/// правило, что у морфер-резки). Цель исчезла — исчезает и публикация, и
/// все её переопределения ВО ВСЕХ экземплярах, ОДНИМ шагом истории.
#[test]
fn t672_a_published_target_lives_and_dies_with_its_clip() {
    let mut p = template();
    p.clips.iter_mut().find(|c| c.id == INST_B).unwrap().overrides =
        vec![Override::value(HEADLINE, ParamValue::Text("Второй выпуск".into()))];
    p.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides =
        vec![Override::value(BLUR, ParamValue::Num(4.0))];
    p.validate().expect("шаблон с переопределениями законен");
    let mut m = TimelineModel::new(p);

    // --- трим цели: публикация цела --------------------------------------
    m.set_editing(Some(SEQ));
    m.apply_batch(vec![Command::TrimEnd { id: TITLE, new_end: Tick(LEN.0 - FRAME * 5) }])
        .expect("трим законен");
    assert!(
        m.project().published.iter().any(|x| x.id == HEADLINE),
        "трим цели не имеет права убивать публикацию: адрес — ClipId, а не окно"
    );

    // --- разрез: кусок №0 держит id, значит и ручку -----------------------
    m.apply_batch(vec![Command::Split {
        id: TITLE,
        at: Tick(FRAME * 10),
        new_id: ClipId(50),
        new_group: None,
    }])
    .expect("разрез законен");
    assert!(
        m.project().published.iter().any(|x| x.id == HEADLINE),
        "разрез обязан оставить ручку на ЛЕВОЙ половине — она сохранила id \
         (то же правило, что у морфер-резки)"
    );
    m.set_editing(None);
    let b = m.project().clips.iter().find(|c| c.id == INST_B).unwrap();
    assert_eq!(b.overrides.len(), 1, "переопределение пережило правку внутри ленты");

    // --- цель исчезла: ручка и ВСЕ её переопределения уходят одним шагом --
    m.set_editing(Some(SEQ));
    m.apply_batch(vec![Command::RemoveClip { id: TITLE }]).expect("удаление законно");
    m.set_editing(None);
    let p = m.project().clone();
    assert!(
        !p.sequence(SEQ).unwrap().published.iter().any(|x| x.id == HEADLINE),
        "цель исчезла — обязана исчезнуть и публикация: ручка, которая ни на что \
         не действует, и есть тот минус, о котором говорит разведка"
    );
    assert!(
        p.clips.iter().find(|c| c.id == INST_B).unwrap().overrides.is_empty(),
        "переопределение снятой ручки осталось у экземпляра"
    );
    assert_eq!(
        p.clips.iter().find(|c| c.id == INST_A).unwrap().overrides.len(),
        1,
        "чужая ручка (размытие) пострадала за компанию"
    );
    p.validate().expect("после согласования проект обязан быть законным");

    // ОДИН шаг истории: undo возвращает и цель, и ручку, и переопределение
    m.undo().expect("шаг истории есть — откат обязан пройти");
    // undo переносит туда, где правка была (t454), — выходим обратно наружу
    m.set_editing(None);
    let p = m.project().clone();
    assert!(
        p.sequence(SEQ).unwrap().published.iter().any(|x| x.id == HEADLINE),
        "undo не вернул публикацию — значит согласование шло ОТДЕЛЬНЫМ шагом"
    );
    assert_eq!(
        p.clips.iter().find(|c| c.id == INST_B).unwrap().overrides.len(),
        1,
        "undo не вернул переопределение"
    );

    // ...и РАСКРЫТИЕ ПОЛОСЫ публикаций не сносит. Пока полоса открыта,
    // содержимое ленты физически лежит в корне, а её публикации остаются
    // в `sequences`: посчитай мы судьбу цели в этот момент — каждая
    // выглядела бы пропавшей, и раскрытие тихо съело бы весь набор.
    m.undo().expect("шаг истории есть — откат обязан пройти");
    m.set_editing(None);
    assert!(m.expand_band(INST_A), "полоса раскрытия обязана открыться");
    m.apply_batch(vec![Command::SetOpacity { id: INST_B, opacity: 0.5 }])
        .expect("правка при открытой полосе законна");
    assert!(
        m.document().sequence(SEQ).unwrap().published.iter().any(|x| x.id == HEADLINE),
        "раскрытие полосы снесло публикацию"
    );
    m.collapse_all_bands();

    // снятие ручки руками действует так же — по всему дереву сразу
    m.set_editing(Some(SEQ));
    m.apply_batch(vec![Command::UnpublishParam { seq: SEQ, id: HEADLINE }]).expect("снятие законно");
    m.set_editing(None);
    assert!(
        m.project().clips.iter().find(|c| c.id == INST_B).unwrap().overrides.is_empty(),
        "снятая ручка обязана унести свои переопределения из ВСЕХ экземпляров"
    );
}

/// Тест 672b: что валидация обязана отвергать явно.
#[test]
fn t672b_validation_refuses_dangling_and_mistyped_overrides() {
    let mut p = template();
    p.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides =
        vec![Override::value(PubId(77), ParamValue::Num(1.0))];
    assert!(
        matches!(p.validate(), Err(ProjectError::OverrideDangling(INST_A, PubId(77)))),
        "переопределение на неопубликованное обязано валить загрузку: иначе \
         экземпляр молча играет шаблон и объяснить это нечем"
    );

    let mut p = template();
    p.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides =
        vec![Override::value(HEADLINE, ParamValue::Num(1.0))];
    assert!(matches!(p.validate(), Err(ProjectError::OverrideTypeMismatch(INST_A, HEADLINE))));

    // переопределение на ОБЫЧНОМ событии
    let mut p = template();
    let mut stray = clip(60, 3, Tick(LEN.0 * 3), LEN);
    stray.overrides = vec![Override::value(HEADLINE, ParamValue::Text("нет".into()))];
    p.clips.push(stray);
    assert!(matches!(p.validate(), Err(ProjectError::OverrideOnPlainClip(ClipId(60)))));

    // кривая на СТУПЕНЧАТОМ типе
    let mut p = template();
    p.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides = vec![Override {
        pub_id: HEADLINE,
        value: ParamValue::Text("раз".into()),
        curves: vec![ParamCurve {
            param: "text".into(),
            keys: vec![AutoKey::at(Tick::ZERO, 0.0)],
            source: CurveSource::Keys,
        }],
    }];
    assert!(
        matches!(p.validate(), Err(ProjectError::OverrideCurveNotAllowed(INST_A, HEADLINE))),
        "между «Заголовком» и другим заголовком нет половины — кривой там взяться неоткуда"
    );

    // объявление на цель ДРУГОГО типа
    let mut p = template();
    p.sequence_mut(SEQ).unwrap().published[0].ty = ParamType::Num;
    assert!(matches!(p.validate(), Err(ProjectError::PublishedTypeMismatch(Some(SEQ), HEADLINE))));

    // объявление без имени
    let mut p = template();
    p.sequence_mut(SEQ).unwrap().published[0].name = "  ".into();
    assert!(matches!(p.validate(), Err(ProjectError::PublishedUnnamed(Some(SEQ), HEADLINE))));
}

// ---- 673. пустой набор — прежний путь -----------------------------------------

/// Тест 673 (Р.59, стадия 3): `resolve_reel(seq, &[])` — ПРЕЖНИЙ путь.
///
/// Требование ноля цены («если проект простой без выкрутасов — то и лагать
/// он не должен вовсе») здесь выражается буквально: без переопределений
/// содержимое ленты обязано получаться тем же кодом, без клона ленты и без
/// клона её переходов и камер.
#[test]
fn t673_an_empty_override_set_takes_the_old_path() {
    let p = template();

    // 1) байтовое равенство с прежней дверью выдачи клипов
    assert_eq!(
        p.resolve_reel(Some(SEQ), &[]).clips,
        p.effective_reel(Some(SEQ)),
        "пустой набор обязан давать РОВНО то же, что старая дверь"
    );

    // 2) ни одного клона: переходы и камеры ЗАИМСТВУЮТСЯ
    let r = p.resolve_reel(Some(SEQ), &[]);
    assert!(
        matches!(r.transitions, std::borrow::Cow::Borrowed(_))
            && matches!(r.cameras, std::borrow::Cow::Borrowed(_)),
        "пустой набор клонирует содержимое ленты — это и есть цена, которой быть не должно"
    );

    // 3) и набор, СОСТОЯЩИЙ ИЗ ЧУЖИХ ручек, тоже идёт прежним путём: он
    //    ничего не меняет, а значит и клонировать нечего
    let alien = [Override::value(PubId(999), ParamValue::Num(1.0))];
    let r = p.resolve_reel(Some(SEQ), &alien);
    assert!(
        matches!(r.transitions, std::borrow::Cow::Borrowed(_)),
        "набор без единой живой ручки обязан идти прежним путём"
    );

    // 4) публикация САМА ПО СЕБЕ содержимое не двигает: лента с
    //    объявленным набором и лента без него дают одни и те же клипы
    let mut bare = template();
    bare.sequence_mut(SEQ).unwrap().published.clear();
    assert_eq!(
        bare.effective_reel(Some(SEQ)),
        p.effective_reel(Some(SEQ)),
        "объявление ручки изменило содержимое ленты — значит публикация не бесплатна"
    );
}

// ---- 690. полоса показывает шаблон, раскрытие впекает -------------------------

/// Тест 690 (Р.59, стадия 3): полоса раскрытия — ССЫЛКА, разворачивание —
/// КОПИЯ.
///
/// Полоса показывает ШАБЛОН и обязана собираться обратно побайтово вместе с
/// переопределениями (они на ЭКЗЕМПЛЯРЕ, а он в полосу не уезжает).
/// `expand_instance` — действие необратимое: ленты под кусками больше нет,
/// жить ручке негде, поэтому значения ВПЕКАЮТСЯ и число называется вслух.
#[test]
fn t690_a_band_shows_the_template_and_expanding_bakes_the_overrides() {
    let mut p = template();
    // одна лента — один экземпляр: полоса раскрытия считается от него
    p.clips.retain(|c| c.id != INST_B);
    p.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides =
        vec![Override::value(HEADLINE, ParamValue::Text("Второй выпуск".into()))];
    p.ensure_tracks(LaneKind::Video, 1);
    p.validate().expect("сцена законна");

    // --- полоса: раскрыть и собрать — ПОБАЙТОВОЕ тождество ---------------
    let before = p.clone();
    let b = Band::of(&before, INST_A).expect("полоса для экземпляра");
    let mut q = before.clone();
    q.merge_reel(&b);
    let inner = q.clips.iter().find(|c| c.id == TITLE).expect("содержимое ленты в полосе");
    assert_eq!(
        inner.r#gen.as_ref().and_then(|g| g.get_value("text")),
        Some(ParamValue::Text("Заголовок по умолчанию".into())),
        "полоса раскрытия обязана показывать ШАБЛОН: она ссылка, а не копия, и \
         правка в ней правит ЛЕНТУ — то есть все экземпляры разом"
    );
    q.unmerge_reel(&b).expect("сборка обратно обязана пройти");
    assert_eq!(q, before, "раскрыть и собрать перестало быть тождеством");

    // --- разворачивание: значения ВПЕКАЮТСЯ ------------------------------
    let mut m = TimelineModel::new(p);
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) = ops::expand_instance(m.project(), INST_A, n.ids());
    assert_eq!(rep.overrides_baked, 1, "разворачивание обязано сказать, сколько впекло");
    m.apply_batch(cmds).expect("разворачивание законно");
    let p = m.project();
    let titles: Vec<&ClipEvent> = p.clips.iter().filter(|c| c.r#gen.is_some()).collect();
    assert_eq!(titles.len(), 1, "титр не развернулся");
    assert_eq!(
        titles[0].r#gen.as_ref().and_then(|g| g.get_value("text")),
        Some(ParamValue::Text("Второй выпуск".into())),
        "«что видел, то и получил»: развёрнутый титр обязан нести значение \
         ЭКЗЕМПЛЯРА — ссылки на ленту у него больше нет, и ручке жить негде"
    );
    assert!(
        p.sequence(SEQ).unwrap().clips.iter().any(|c| c
            .r#gen
            .as_ref()
            .and_then(|g| { g.get_value("text") })
            == Some(ParamValue::Text("Заголовок по умолчанию".into()))),
        "впекание не имеет права трогать САМУ ЛЕНТУ"
    );
}

// ---- 692, 694 (Р.60). паспорт цели — единственный источник пределов --------

/// Тест 694 (Р.60): паспорт свойства ПО АДРЕСУ есть у каждого владельца.
///
/// До Р.60 модель отвечала на «какой тип у этой цели» (`addr_type`) и «какое
/// у неё значение» (`addr_value`), но не на «какие у неё пределы» — и окно
/// автора рисовало пустое сужение нулём. Паспорт обязан приходить оттуда же,
/// откуда тип, и совпадать с ним по типу: разойдутся — у цвета окажется
/// паспорт компоненты, а тип группы.
#[test]
fn t694_addr_spec_answers_for_every_owner() {
    let mut p = template();

    // слот эффекта: пределы и умолчание — паспорта самого эффекта
    let fx = ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "radius");
    let s = p.addr_spec(Some(SEQ), &fx).expect("паспорт слота");
    assert_eq!(Some(s.ty.clone()), p.addr_type(Some(SEQ), &fx), "тип разошёлся с addr_type");
    let native = FxInstance::Blur { radius: 8, radius_y: 0 }
        .param_spec("radius")
        .expect("у Blur есть radius")
        .clone();
    assert_eq!((s.min, s.max, s.default), (native.min, native.max, native.default));
    assert!(s.max > 0.0, "паспорт слота пришёл пустым: {s:?}");

    // генератор на событии
    let gen_addr = ParamAddr::new(TITLE, ParamOwner::Generator, "text");
    let s = p.addr_spec(Some(SEQ), &gen_addr).expect("паспорт генератора");
    assert_eq!(Some(s.ty), p.addr_type(Some(SEQ), &gen_addr), "тип генератора разошёлся");

    // атрибут события: пределы те же, которыми его клампит модель
    let att = ParamAddr::new(PLATE, ParamOwner::ClipAttr, "opacity");
    let s = p.addr_spec(Some(SEQ), &att).expect("паспорт атрибута");
    assert_eq!(Some(s.ty.clone()), p.addr_type(Some(SEQ), &att));
    assert_eq!((s.min, s.max, s.default), (0.0, 1.0, 1.0), "пределы уровня не те");
    let media_att = ParamAddr::new(PLATE, ParamOwner::ClipAttr, "media");
    assert_eq!(
        p.addr_spec(Some(SEQ), &media_att).map(|s| s.ty),
        p.addr_type(Some(SEQ), &media_att),
        "у ссылки на медиа паспорт и тип разошлись"
    );

    // переход и камера — свои номера, `clip` в адресе не участвует
    {
        let s = p.sequences.iter_mut().find(|s| s.id == SEQ).expect("лента");
        s.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(TITLE),
            b: Some(PLATE),
            in_offset: Tick(FRAME),
            out_offset: Tick(FRAME),
            kind: TransitionKind::default(),
            audio: Default::default(),
            auto: Vec::new(),
            invert: false,
        });
    }
    let tr_name = {
        let s = p.sequences.iter().find(|s| s.id == SEQ).expect("лента");
        s.transitions[0].kind.param_specs().first().map(|x| x.name.to_string())
    };
    if let Some(name) = tr_name {
        let addr = ParamAddr::new(ClipId(0), ParamOwner::Transition { id: TransitionId(1) }, &name);
        let s = p.addr_spec(Some(SEQ), &addr).expect("паспорт перехода");
        assert_eq!(Some(s.ty), p.addr_type(Some(SEQ), &addr), "тип перехода разошёлся");
    }

    // цель исчезла — паспорта нет, ровно как типа
    let gone = ParamAddr::new(ClipId(999), ParamOwner::FxSlot { slot: 0 }, "radius");
    assert!(p.addr_spec(Some(SEQ), &gone).is_none(), "паспорт у мёртвой цели");
    assert!(p.addr_type(Some(SEQ), &gone).is_none());
    let no_such = ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "нет_такого");
    assert!(p.addr_spec(Some(SEQ), &no_such).is_none(), "паспорт у несуществующего имени");

    // вложенная публикация: наружу едет паспорт ЦЕЛИ, суженный внутренней
    // ручкой — потребитель внешней ленты не получает больше, чем разрешил
    // автор внутренней
    let mut outer = Sequence::new(MediaId(200), "Обёртка");
    let mut inst = clip(50, SEQ.0, Tick::ZERO, LEN);
    inst.media = SEQ;
    outer.clips.push(inst);
    p.media.push(MediaRef::new(MediaId(200), "seq:200"));
    p.sequences.push(outer);
    let nested = ParamAddr::new(ClipId(50), ParamOwner::Nested { pub_id: BLUR }, "radius");
    let s = p.addr_spec(Some(MediaId(200)), &nested).expect("паспорт вложенной ручки");
    assert_eq!(
        (s.min, s.max),
        (2.0, 16.0),
        "сужение внутренней ручки не наложилось на паспорт цели: {s:?}"
    );
    assert!(
        s.default >= s.min && s.default <= s.max,
        "умолчание уехало за суженные пределы: {s:?}"
    );
}

/// Тест 692 (Р.60): сужение хранится ТОЛЬКО как сужение.
///
/// «Еще значения min max всегда в 0 проставляются по умолчанию» — пустое
/// `Option` рисовалось нулём. Обратная половина правила: предел, равный
/// пределу цели, обязан лечь в файл как отсутствие, иначе первая же
/// публикация запишет туда копию паспорта, и та разойдётся с целью при
/// смене версии эффекта (тот же довод, что против хранимого значения
/// шаблона, Р.59).
#[test]
fn t692_a_published_range_stores_narrowing_only() {
    let p = template();
    let addr = ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "radius");
    let spec = p.addr_spec(Some(SEQ), &addr).expect("паспорт цели");

    // только что опубликованная ручка сужения не несёт
    let fresh = PubRange::default();
    assert!(fresh.is_empty(), "свежая публикация уже что-то сузила");
    assert_eq!(
        (fresh.min.unwrap_or(spec.min), fresh.max.unwrap_or(spec.max)),
        (spec.min, spec.max),
        "окно автора показало бы не пределы цели"
    );

    // пределы цели, выставленные руками, — это ОТСУТСТВИЕ сужения
    let same = PubRange::narrowed(&spec, spec.min, spec.max);
    assert!(same.is_empty(), "пределы цели записались копией: {same:?}");

    // сузили только сверху — снизу по-прежнему ничего
    let hi = PubRange::narrowed(&spec, spec.min, spec.max / 2.0);
    assert_eq!(hi.min, None, "снизу появилось сужение из ниоткуда");
    assert_eq!(hi.max, Some(spec.max / 2.0));

    // попытка РАСШИРИТЬ пределы цели сужением не становится
    let wide = PubRange::narrowed(&spec, spec.min - 100.0, spec.max + 100.0);
    assert!(wide.is_empty(), "расширение записалось как сужение: {wide:?}");

    // возврат к пределу цели снимает сужение
    let back = PubRange::narrowed(&spec, spec.min, spec.max);
    assert!(back.is_empty(), "возврат к пределу цели сужение не снял");

    // кламп в обе стороны — поверх пределов цели
    let narrow = PubRange { min: Some(4.0), max: Some(6.0) };
    assert_eq!(narrow.clamp(1.0), 4.0);
    assert_eq!(narrow.clamp(99.0), 6.0);

    // и сериализация: пустое сужение полей не рождает
    let json = serde_json::to_string(&PublishedParam {
        id: BLUR,
        name: "Размытие".into(),
        target: addr,
        ty: ParamType::Num,
        range: PubRange::default(),
    })
    .expect("сериализация");
    assert!(!json.contains("min"), "пустое сужение уехало в файл: {json}");
    assert!(!json.contains("max"), "пустое сужение уехало в файл: {json}");
}

// ---- 752. ведущая геометрия — такой же носитель ручки -------------------------

const ZOOM: PubId = PubId(4);

/// Тест 752 (Р.63 п.9): ВЕДУЩАЯ ГЕОМЕТРИЯ публикуется как любой слот.
///
/// Владелец: «по какой причине у первого geometry нет возможности
/// опубликовать параметры а у второго есть? ведь этот первый geometry идет
/// как fx, разве нет? или у нас дубли кода?». Не дубли — ДЫРА В АДРЕСАЦИИ:
/// ведущий узел живёт своим полем события, а `ParamOwner` о нём не знал.
/// Паспорт, `set_value` и кривые у него ровно те же, что у слота, поэтому
/// тест утверждает то же самое, что t671/t672 утверждают про слот: значение
/// шаблона читается живьём, переопределение действует числом и кривой,
/// адрес переживает трим и левую половину разреза, смерть цели уносит ручку,
/// а у аудио-события такого адреса не существует.
#[test]
fn t752_the_lead_geometry_is_a_publishable_owner() {
    let mut p = template();
    {
        let s = p.sequence_mut(SEQ).expect("лента");
        s.published.push(PublishedParam {
            id: ZOOM,
            name: "Крупность подложки".into(),
            target: ParamAddr::new(PLATE, ParamOwner::Geometry, "zoom"),
            ty: ParamType::Num,
            range: PubRange::default(),
        });
    }
    p.validate().expect("ручка на ведущей геометрии законна");

    // --- значение ШАБЛОНА читается живьём, как у слота --------------------
    assert_eq!(
        p.published_value(SEQ, ZOOM),
        Some(ParamValue::Num(1.0)),
        "значение шаблона обязано читаться У ЦЕЛИ, а не храниться в объявлении"
    );
    let spec = p
        .addr_spec(Some(SEQ), &ParamAddr::new(PLATE, ParamOwner::Geometry, "zoom"))
        .expect("паспорт цели");
    assert!(spec.min > 0.0 && spec.max >= 32.0, "паспорт пришёл не от узла геометрии: {spec:?}");

    // --- переопределение ЧИСЛОМ меняет кадр экземпляра --------------------
    let zoom_of = |p: &Project, ov: &[Override]| -> f64 {
        let r = p.resolve_reel(Some(SEQ), ov);
        r.clips.iter().find(|c| same_clip(c.id, PLATE)).expect("подложка").geometry.zoom
    };
    assert_eq!(zoom_of(&p, &[]), 1.0, "без переопределения — значение шаблона");
    assert_eq!(
        zoom_of(&p, &[Override::value(ZOOM, ParamValue::Num(2.5))]),
        2.5,
        "переопределение зума не доехало до ведущего узла"
    );
    assert_eq!(
        p.sequence(SEQ).unwrap().clips.iter().find(|c| c.id == PLATE).unwrap().geometry.zoom,
        1.0,
        "переопределение экземпляра переписало САМУ ленту"
    );

    // --- кривая ложится в `geometry_auto` и переводится в материал цели ---
    let curve = ParamCurve {
        param: "zoom".into(),
        keys: vec![AutoKey::at(Tick::ZERO, 1.0), AutoKey::at(Tick(FRAME * 10), 3.0)],
        source: CurveSource::Keys,
    };
    let ov = vec![Override { pub_id: ZOOM, value: ParamValue::Num(2.5), curves: vec![curve] }];
    let r = p.resolve_reel(Some(SEQ), &ov);
    let plate = r.clips.iter().find(|c| same_clip(c.id, PLATE)).expect("подложка");
    assert_eq!(plate.geometry_auto.len(), 1, "кривая не доехала до дома ведущего узла");
    assert_eq!(plate.geometry_auto[0].param, "zoom");
    assert_eq!(
        plate.geometry_auto[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, FRAME * 10],
        "ключи не переведены в материал цели тем же кодом, что у слота"
    );
    // и она ЗАКОННА: носитель кривой объявлен
    let mut with = p.clone();
    with.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides = ov.clone();
    with.validate().expect("кривая на ведущей геометрии — законный носитель");

    // --- ПРАВИЛО приведения — такое же свойство (выбор) -------------------
    {
        let s = p.sequence_mut(SEQ).expect("лента");
        s.published.push(PublishedParam {
            id: PubId(5),
            name: "Правило кадра".into(),
            target: ParamAddr::new(PLATE, ParamOwner::Geometry, "rule"),
            ty: ParamType::Choice { variants: std::borrow::Cow::Borrowed(CONFORM_KEYS) },
            range: PubRange::default(),
        });
    }
    p.validate().expect("ручка на правиле законна");
    let r = p.resolve_reel(Some(SEQ), &[Override::value(
        PubId(5),
        ParamValue::Choice("fill".into()),
    )]);
    assert_eq!(
        r.clips.iter().find(|c| same_clip(c.id, PLATE)).unwrap().geometry.rule,
        Conform::Fill,
        "правило приведения не переопределилось"
    );

    // --- У АУДИО-СОБЫТИЯ такого адреса НЕ СУЩЕСТВУЕТ ----------------------
    let mut bad = template();
    {
        let s = bad.sequence_mut(SEQ).expect("лента");
        let mut a = clip(70, 3, Tick::ZERO, LEN);
        a.kind = ClipKind::Audio;
        a.lane = 0;
        s.clips.push(a);
        s.ensure_tracks(LaneKind::Audio, 1);
        s.published.push(PublishedParam {
            id: ZOOM,
            name: "Зум у звука".into(),
            target: ParamAddr::new(ClipId(70), ParamOwner::Geometry, "zoom"),
            ty: ParamType::Num,
            range: PubRange::default(),
        });
    }
    assert_eq!(
        bad.addr_type(Some(SEQ), &ParamAddr::new(ClipId(70), ParamOwner::Geometry, "zoom")),
        None,
        "у аудио-события ведущего узла нет — адрес обязан быть НЕсуществующим"
    );
    assert!(
        matches!(bad.validate(), Err(ProjectError::PublishedTargetMissing(Some(SEQ), ZOOM))),
        "публикация геометрии у звука прошла валидацию молча"
    );

    // --- СУДЬБА ЦЕЛИ: та же, что у слота ----------------------------------
    let mut m = TimelineModel::new(p.clone());
    m.set_editing(Some(SEQ));
    m.apply_batch(vec![Command::TrimEnd { id: PLATE, new_end: Tick(LEN.0 - FRAME * 5) }])
        .expect("трим законен");
    assert!(
        m.project().published.iter().any(|x| x.id == ZOOM),
        "трим цели убил ручку геометрии: адрес — ClipId, а не окно"
    );
    m.apply_batch(vec![Command::Split {
        id: PLATE,
        at: Tick(FRAME * 10),
        new_id: ClipId(51),
        new_group: None,
    }])
    .expect("разрез законен");
    assert!(
        m.project().published.iter().any(|x| x.id == ZOOM),
        "разрез обязан оставить ручку на ЛЕВОЙ половине"
    );
    m.apply_batch(vec![Command::RemoveClip { id: PLATE }]).expect("удаление законно");
    m.set_editing(None);
    assert!(
        !m.project().sequence(SEQ).unwrap().published.iter().any(|x| x.id == ZOOM),
        "цель исчезла, а ручка геометрии осталась висеть"
    );
    m.project().validate().expect("после согласования проект законен");
}

// ---- 753. команда называет ленту, а не контекст --------------------------------

const SEQ2: MediaId = MediaId(200);
const FROM_BAND: PubId = PubId(10);
const FROM_INSIDE: PubId = PubId(11);
const FROM_OTHER: PubId = PubId(12);
const ON_TRANS: PubId = PubId(13);

/// Тест 753 (Р.63 п.11): публикация адресует ЛЕНТУ, а не то место, где
/// стоит человек.
///
/// Владелец: «если опубликовать параметр не входя полностью в композицию —
/// то зачем-то мы автоматически полностью входим в композицию». Вход был
/// не капризом интерфейса, а следствием команды: она правила «ту ленту, в
/// которой стоишь», и из раскрытой полосы ручка легла бы в КОРЕНЬ. Теперь
/// лента названа в самой команде — и три места подачи дают один результат,
/// `editing()` не двигается, а несуществующая лента получает отказ.
#[test]
fn t753_publishing_names_the_reel_not_the_context() {
    let mut p = template();
    // соседняя лента — чтобы подать команду ИЗ ДРУГОЙ ленты
    let mut s2 = Sequence::new(SEQ2, "Соседняя");
    s2.clips.push(clip(30, 3, Tick::ZERO, LEN));
    s2.ensure_tracks(LaneKind::Video, 1);
    p.sequences.push(s2);
    let mut inst2 = clip(20, SEQ2.0, Tick(LEN.0 * 2), LEN);
    inst2.lane = 1;
    p.clips.push(inst2);
    p.ensure_tracks(LaneKind::Video, 2);
    // и переход ВНУТРИ первой ленты: его ручка адресуется той же `seq`
    {
        let s = p.sequence_mut(SEQ).expect("лента");
        let mut b = clip(3, 2, LEN, LEN);
        b.lane = 1;
        s.clips.push(b);
        s.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(PLATE),
            b: Some(ClipId(3)),
            in_offset: Tick(FRAME * 2),
            out_offset: Tick(FRAME * 2),
            // у кроссфейда своих чисел нет вовсе — публиковать нечего;
            // берём вид с параметрами
            kind: TransitionKind::Wipe { angle: 0.0, softness: 0.15, dir: Default::default() },
            audio: Default::default(),
            auto: Vec::new(),
            invert: false,
        });
    }
    p.validate().expect("сцена законна");
    let mut m = TimelineModel::new(p);

    let decl = |id: PubId, name: &str, target: ParamAddr| PublishedParam {
        id,
        name: name.to_string(),
        target,
        ty: ParamType::Num,
        range: PubRange::default(),
    };
    let zoom = |c: ClipId| ParamAddr::new(c, ParamOwner::Geometry, "zoom");

    // --- 1) ИЗ КОРНЯ, лента раскрыта полосой ------------------------------
    assert!(m.expand_band(INST_A), "полоса раскрытия обязана открыться");
    assert_eq!(m.editing(), None, "сцена: человек стоит в КОРНЕ");
    m.apply_batch(vec![Command::PublishParam {
        seq: SEQ,
        param: Box::new(decl(FROM_BAND, "Из полосы", zoom(PLATE))),
    }])
    .expect("публикация из полосы законна");
    assert_eq!(
        m.editing(),
        None,
        "подтверждение публикации ВОШЛО в ленту — ровно то, что владелец назвал \
         «зачем-то мы автоматически полностью входим в композицию»"
    );
    assert!(
        m.document().sequence(SEQ).unwrap().published.iter().any(|x| x.id == FROM_BAND),
        "ручка легла не в названную ленту"
    );
    // переход ленты адресуется ТОЙ ЖЕ `seq` из той же полосы (закрыт
    // упор «Отложено» Р.59: раньше до него было не дотянуться)
    m.apply_batch(vec![Command::PublishParam {
        seq: SEQ,
        param: Box::new(PublishedParam {
            id: ON_TRANS,
            name: "Мягкость стыка".into(),
            target: ParamAddr::new(
                ClipId(0),
                ParamOwner::Transition { id: TransitionId(1) },
                "softness",
            ),
            ty: m
                .document()
                .addr_type(
                    Some(SEQ),
                    &ParamAddr::new(
                        ClipId(0),
                        ParamOwner::Transition { id: TransitionId(1) },
                        "softness",
                    ),
                )
                .expect("у кроссфейда есть мягкость"),
            range: PubRange::default(),
        }),
    }])
    .expect("публикация параметра перехода из полосы законна");
    assert!(
        m.document().sequence(SEQ).unwrap().published.iter().any(|x| x.id == ON_TRANS),
        "ручка перехода легла не в названную ленту"
    );
    m.collapse_all_bands();

    // --- 2) ИЗНУТРИ САМОЙ ленты -------------------------------------------
    m.set_editing(Some(SEQ));
    m.apply_batch(vec![Command::PublishParam {
        seq: SEQ,
        param: Box::new(decl(FROM_INSIDE, "Изнутри", zoom(TITLE))),
    }])
    .expect("публикация изнутри законна");
    assert!(
        m.project().published.iter().any(|x| x.id == FROM_INSIDE),
        "стоя ВНУТРИ ленты, ручка не легла в неё же"
    );
    m.set_editing(None);

    // --- 3) ИЗ ДРУГОЙ ленты ------------------------------------------------
    m.set_editing(Some(SEQ2));
    m.apply_batch(vec![Command::PublishParam {
        seq: SEQ,
        param: Box::new(decl(FROM_OTHER, "Из соседней", zoom(ClipId(3)))),
    }])
    .expect("публикация в НАЗВАННУЮ ленту из другой законна");
    assert_eq!(m.editing(), Some(SEQ2), "команда утащила человека из его ленты");
    m.set_editing(None);

    // все три ручки — в ОДНОЙ ленте, и она названная
    let set = m.document().sequence(SEQ).unwrap().published.clone();
    for id in [FROM_BAND, FROM_INSIDE, FROM_OTHER, ON_TRANS] {
        assert!(set.iter().any(|x| x.id == id), "ручка {id:?} не дошла до ленты: {set:?}");
    }
    assert!(
        m.document().sequence(SEQ2).unwrap().published.is_empty(),
        "ручка уехала в соседнюю ленту — именно там она и умерла бы тем же батчем"
    );
    assert!(m.document().published.is_empty(), "ручка легла в КОРЕНЬ: он никому не экземпляр");

    // --- 4) НЕСУЩЕСТВУЮЩАЯ лента — отказ с адресом ------------------------
    let ghost = MediaId(777);
    assert_eq!(
        m.apply_batch(vec![Command::PublishParam {
            seq: ghost,
            param: Box::new(decl(PubId(14), "В никуда", zoom(TITLE))),
        }]),
        Err(CommandError::SequenceNotFound(ghost)),
        "публикация в несуществующую ленту обязана быть ОТКАЗОМ с адресом"
    );
    assert_eq!(
        m.apply_batch(vec![Command::UnpublishParam { seq: ghost, id: FROM_BAND }]),
        Err(CommandError::SequenceNotFound(ghost))
    );

    // --- 5) снятие и правка объявления адресуются так же -------------------
    m.apply_batch(vec![Command::SetPublished {
        seq: SEQ,
        param: Box::new(decl(FROM_BAND, "Переименована", zoom(PLATE))),
    }])
    .expect("правка объявления из корня законна");
    assert_eq!(
        m.document()
            .sequence(SEQ)
            .unwrap()
            .published
            .iter()
            .find(|x| x.id == FROM_BAND)
            .map(|x| x.name.clone()),
        Some("Переименована".to_string()),
        "правка объявления не дошла до названной ленты"
    );
    m.apply_batch(vec![Command::UnpublishParam { seq: SEQ, id: FROM_BAND }])
        .expect("снятие из корня законно");
    assert!(
        !m.document().sequence(SEQ).unwrap().published.iter().any(|x| x.id == FROM_BAND),
        "снятие не дошло до названной ленты"
    );
    m.document().validate().expect("документ обязан остаться законным");
}

// ---- 792. кривые переопределения: формы, слой, формула — в домене ленты -------

/// Тест 792 (Р.67): переопределение экземпляра принимает те же формы и
/// источники, что кривая слота, и разрешает их В ДОМЕНЕ ЛЕНТЫ (ось
/// экземпляра) до перевода во время цели. Структура — проект владельца:
/// два экземпляра одной ленты друг над другом, кривая у верхнего. На
/// прежнем коде формула отвергалась `CurveSourceUnsupported`, а окно молча
/// стирало поле.
#[test]
fn t792_override_curves_take_shapes_and_sources_in_the_reel_domain() {
    let mut p = template();
    // второй экземпляр — НАД первым, в то же время (лента V4 над V1)
    let b = p.clips.iter_mut().find(|c| c.id == INST_B).unwrap();
    b.start = Tick::ZERO;
    b.lane = 3;
    p.ensure_tracks(LaneKind::Video, 4);
    p.validate().expect("два экземпляра друг над другом законны");
    let radius_at = |p: &Project, ov: &[Override], t: i64| -> f64 {
        let r = p.resolve_reel(Some(SEQ), ov);
        let plate = r.clips.iter().find(|c| same_clip(c.id, PLATE)).unwrap();
        match plate.fx_auto.first().and_then(|s| s.iter().find(|c| c.param == "radius")) {
            Some(c) => curve_value(&c.keys, Tick(t)),
            // без кривой — значение слота (шаблон либо постоянное переопределение)
            None => plate.fx[0].get_param("radius").expect("у размытия есть радиус"),
        }
    };
    let sec = 90_000;
    // --- горка ключами 0→16→0 (пределы ручки 2..16) ------------------------
    let hill = |shape: SegShape| Override {
        pub_id: BLUR,
        value: ParamValue::Num(8.0),
        curves: vec![ParamCurve {
            param: "radius".into(),
            keys: vec![
                AutoKey::shaped(Tick(0), 2.0, shape),
                AutoKey::shaped(Tick(sec / 2), 16.0, shape),
                AutoKey::shaped(Tick(sec), 2.0, shape),
            ],
            source: CurveSource::Keys,
        }],
    };
    let mut m = TimelineModel::new(p.clone());
    m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(hill(SegShape::Linear)) }])
        .expect("горка ключами законна");
    let p1 = m.project().clone();
    let ov = &p1.clips.iter().find(|c| c.id == INST_B).unwrap().overrides;
    assert!((radius_at(&p1, ov, sec / 2) - 16.0).abs() < 1e-9, "вершина горки");
    assert!((radius_at(&p1, ov, 0) - 2.0).abs() < 1e-9 && (radius_at(&p1, ov, sec) - 2.0).abs() < 1e-9, "края");
    // нижний экземпляр (без набора) играет шаблон
    let a = p1.clips.iter().find(|c| c.id == INST_A).unwrap();
    assert!((radius_at(&p1, &a.overrides, sec / 2) - 8.0).abs() < 1e-9, "нижний — шаблон");

    // --- зигзаг на участке ------------------------------------------------
    let zig = SegShape::Zigzag { periods: 4.0, amplitude: 2.0, profile: ZigProfile::Triangle };
    m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(hill(zig)) }])
        .expect("форма отрезка у переопределения законна");
    let p2 = m.project().clone();
    let ov = &p2.clips.iter().find(|c| c.id == INST_B).unwrap().overrides;
    let plain = 2.0 + (16.0 - 2.0) * 0.25;
    assert!((radius_at(&p2, ov, sec / 8) - plain).abs() > 0.1, "зигзаг не звучит у цели");

    // --- слой k * 0.5 над горкой ------------------------------------------
    let mut layered = hill(SegShape::Linear);
    layered.curves[0].source = CurveSource::Formula { expr: "k * 0.5".into() };
    m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(layered) }])
        .expect("слой над ключами законен (на прежнем коде — CurveSourceUnsupported)");
    let p3 = m.project().clone();
    let ov = &p3.clips.iter().find(|c| c.id == INST_B).unwrap().overrides;
    assert!((radius_at(&p3, ov, sec / 2) - 8.0).abs() < 1e-6, "k*0.5 на вершине: {}", radius_at(&p3, ov, sec / 2));
    // ключи горки в МОДЕЛИ целы
    assert_eq!(ov[0].curves[0].keys.len(), 3, "слой не стёр ключи");

    // --- формула-источник t/10 по оси экземпляра ---------------------------
    let formula = Override {
        pub_id: BLUR,
        value: ParamValue::Num(8.0),
        curves: vec![ParamCurve { param: "radius".into(), keys: vec![], source: CurveSource::Formula { expr: "2 + t * 4".into() } }],
    };
    m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(formula) }])
        .expect("формула у переопределения законна");
    let p4 = m.project().clone();
    let ov = &p4.clips.iter().find(|c| c.id == INST_B).unwrap().overrides;
    // 0.5 с от начала экземпляра → 2 + 0.5·4 = 4
    assert!((radius_at(&p4, ov, sec / 2) - 4.0).abs() < 1e-6, "t считается по оси экземпляра: {}", radius_at(&p4, ov, sec / 2));

    // --- связь наружу ленты — отказ с причиной ------------------------------
    let outside = Override {
        pub_id: BLUR,
        value: ParamValue::Num(8.0),
        curves: vec![ParamCurve {
            param: "radius".into(),
            keys: vec![],
            source: CurveSource::Link { path: format!("{}/fx0/radius", INST_A.0), expr: None, stops: vec![] },
        }],
    };
    let e = m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(outside) }]);
    assert!(e.is_err(), "связь через границу ленты обязана быть отказом");
    assert!(format!("{e:?}").contains("outside the reel"), "причина не названа: {e:?}");
}

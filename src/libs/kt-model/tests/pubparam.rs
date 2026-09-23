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
    p.media.push(MediaRef { id: MediaId(1), path: gen_kind_path(&title_kind("")) });
    p.media.push(MediaRef { id: MediaId(2), path: "synth:2".into() });
    p.media.push(MediaRef { id: MediaId(3), path: "synth:3".into() });

    let mut s = Sequence::new(SEQ, "Выпуск");
    let mut title = clip(TITLE.0, 1, Tick::ZERO, LEN);
    title.r#gen = Some(title_kind("Заголовок по умолчанию"));
    s.clips.push(title);
    let mut plate = clip(PLATE.0, 2, Tick::ZERO, LEN);
    plate.lane = 1;
    plate.fx = vec![FxInstance::Blur { radius: 8, radius_y: 0 }];
    s.clips.push(plate);
    s.video_lanes = 2;
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
        Command::PublishParam { param: Box::new(head.clone()) },
        Command::PublishParam { param: Box::new(blur.clone()) },
    ])
    .expect("публикация двух живых целей законна");

    // дубль номера — отказ, а не молчаливая перезапись
    assert!(
        m.apply_batch(vec![Command::PublishParam { param: Box::new(head.clone()) }]).is_err(),
        "два объявления с одним номером обязаны быть отказом: номер — это АДРЕС"
    );
    // цель, которой нет, — тоже отказ
    let ghost = PublishedParam {
        target: ParamAddr::new(TITLE, ParamOwner::FxSlot { slot: 7 }, "radius"),
        id: PubId(9),
        ..head.clone()
    };
    assert!(
        m.apply_batch(vec![Command::PublishParam { param: Box::new(ghost) }]).is_err(),
        "публикация на несуществующий слот обязана быть отказом: ручка, которая ни \
         на что не действует, — это минус, который разведка назвала прямо"
    );

    // порядок — часть авторского интерфейса
    m.apply_batch(vec![Command::ReorderPublished { order: vec![BLUR, HEADLINE] }])
        .expect("перестановка законна");
    assert_eq!(
        m.project().published.iter().map(|x| x.id).collect::<Vec<_>>(),
        vec![BLUR, HEADLINE],
        "порядок ручек не применился"
    );
    assert!(
        m.apply_batch(vec![Command::ReorderPublished { order: vec![BLUR] }]).is_err(),
        "порядок обязан быть ПЕРЕСТАНОВКОЙ: неполный список молча потерял бы ручку"
    );

    // переименование и сужение предела
    let renamed =
        PublishedParam { name: "Заголовок выпуска".into(), ..head.clone() };
    m.apply_batch(vec![Command::SetPublished { param: Box::new(renamed) }])
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
    m.apply_batch(vec![Command::UnpublishParam { id: HEADLINE }]).expect("снятие законно");
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
    p.video_lanes = 1;
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
    p.media.push(MediaRef { id: MediaId(200), path: "seq:200".into() });
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

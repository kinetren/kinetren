//! Тесты словаря генераторов (Р.52, п.402): разбор/сборка `gen:`-пути,
//! экранирование строковых значений, полнота спек параметров.

use kt_model::{
    GeneratorKind, HasParams, escape_gen_value, format_gen_path, is_pseudo_path, parse_gen_path,
    unescape_gen_value,
};

/// t469: `format(parse(s))` канонизирует, разбор регистронезависимый
/// (регресс сноски п.211: `synth:Ramp` молча считался отсутствующим
/// файлом), мусор — честный `None`, умолчания не пишутся.
#[test]
fn t469_gen_path_parse_format_canonical() {
    // Канонический roundtrip всех видов реестра с умолчаниями.
    for k in GeneratorKind::all() {
        let path = format_gen_path(&k);
        assert!(path.starts_with("gen:"), "{path}");
        assert_eq!(parse_gen_path(&path), Some(k.clone()), "{path}");
        assert_eq!(format_gen_path(&parse_gen_path(&path).unwrap()), path);
    }

    // титр с НЕумолчаниями: строка с экранированием, стиль, цвета и
    // порядок слоёв переживают путь лослесс (текст титра — С2)
    let mut title = GeneratorKind::default_text();
    if let GeneratorKind::Text {
        text, bold, size, align, anchor, fill_a, stroke_w, order, ..
    } = &mut title
    {
        *text = "Привет: a=b %25\nвторая строка".into();
        *bold = true;
        *size = 0.12;
        *align = kt_model::TextAlign::Left;
        *anchor = kt_model::TextAnchor::BottomCenter;
        *fill_a = 200;
        *stroke_w = 0.05;
        *order = [
            kt_model::TextLayer::Shadow,
            kt_model::TextLayer::Plate,
            kt_model::TextLayer::Fill,
            kt_model::TextLayer::Stroke,
        ];
    }
    let path = format_gen_path(&title);
    assert!(!path.contains('\n'), "путь остаётся одной строкой: {path:?}");
    assert_eq!(parse_gen_path(&path), Some(title.clone()), "{path}");
    assert_eq!(format_gen_path(&parse_gen_path(&path).unwrap()), path, "канон стабилен");

    // Умолчания не сериализуются: чёрный solid — голый тег.
    assert_eq!(
        format_gen_path(&GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        "gen:solid"
    );

    // Не-умолчание пишется одним цветовым токеном, нижним регистром.
    let orange = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    assert_eq!(format_gen_path(&orange), "gen:solid:c=ff8800");
    assert_eq!(parse_gen_path("gen:solid:c=ff8800"), Some(orange.clone()));

    // Регистр схемы, тега, ключа и hex-значения безразличен.
    assert_eq!(parse_gen_path("Gen:SOLID:C=FF8800"), Some(orange));
    // ...а канонизация возвращает нижний регистр.
    assert_eq!(
        format_gen_path(&parse_gen_path("GEN:Solid:c=FF8800").unwrap()),
        "gen:solid:c=ff8800"
    );

    // Мусор — None, а не догадка: незнакомый тег, незнакомый ключ,
    // битое значение, токен без '=', пустой тег, чужая схема.
    for bad in [
        "gen:unknown",
        "gen:solid:zz=1",
        "gen:solid:c=xyzxyz",
        "gen:solid:c=ff88",
        "gen:solid:ff8800",
        "gen:",
        "synth:ramp",
        "clip.mp4",
        "",
    ] {
        assert_eq!(parse_gen_path(bad), None, "{bad:?}");
    }

    // Единый предикат псевдо-путей: обе схемы, любой регистр, но не файлы.
    assert!(is_pseudo_path("gen:solid"));
    assert!(is_pseudo_path("GEN:solid"));
    assert!(is_pseudo_path("synth:ramp"));
    assert!(is_pseudo_path("Synth:Ramp"));
    assert!(!is_pseudo_path("media/clip.mp4"));
    assert!(!is_pseudo_path("generator.mp4"));
}

/// t469b: экранирование строкового значения токена лослесс для любой
/// строки — на нём будет стоять текст титра (С2). Служебные символы пути
/// и управляющие уходят в %XX, путь остаётся одной строкой.
#[test]
fn t469b_gen_value_escaping_roundtrip() {
    let cases = [
        "",
        "Привет, мир!",
        "a:b=c%d",
        "первая\nвторая\r\nтретья",
        "табы\tи нули\u{1}",
        "百分号 %25 уже экранирован",
        "العربية and 中文 🎬",
    ];
    for s in cases {
        let esc = escape_gen_value(s);
        assert!(!esc.contains(':') && !esc.contains('=') && !esc.contains('\n'), "{esc:?}");
        assert_eq!(unescape_gen_value(&esc).as_deref(), Some(s), "{esc:?}");
    }
    // Битые последовательности — None.
    for bad in ["%", "%f", "%zz", "обрыв %2"] {
        assert_eq!(unescape_gen_value(bad), None, "{bad:?}");
    }
}

/// t470: спеки полны (родня t223 у переходов): у каждого вида реестра
/// есть спеки, умолчания лежат в [min, max] и совпадают со значениями
/// самого реестрового экземпляра, get/set ходят по каждому имени.
#[test]
fn t470_generator_param_specs_are_complete() {
    // Виды БЕЗ чисел объявляются здесь поимённо (Р.53, п.420/422):
    // «спеки пусты» — законное состояние ровно для них, и новый вид,
    // потерявший спеки по недосмотру, по-прежнему валит тест.
    const NO_PARAMS: &[&str] = &["transparent", "table"];
    for k in GeneratorKind::all() {
        let specs = k.param_specs();
        assert_eq!(
            specs.is_empty(),
            NO_PARAMS.contains(&k.tag()),
            "{}: спеки и объявление «вид без чисел» разошлись",
            k.tag()
        );
        let mut probe = k.clone();
        for s in specs {
            assert!(
                s.min <= s.default && s.default <= s.max,
                "{}.{}: умолчание вне диапазона",
                k.tag(),
                s.name
            );
            let v = k
                .get_param(&s.name)
                .unwrap_or_else(|| panic!("{}.{}: get_param не знает своё имя", k.tag(), s.name));
            assert_eq!(v, s.default, "{}.{}: реестр не на умолчании", k.tag(), s.name);
            assert!(probe.set_param(&s.name, s.max), "{}.{}: set_param", k.tag(), s.name);
            assert_eq!(probe.get_param(&s.name), Some(s.max));
        }
        assert_eq!(k.get_param("no_such_param"), None);
        assert!(!probe.set_param("no_such_param", 1.0));
        let _ = &probe;
    }
}

/// Тест 475 (Р.52): команда смены пути медиа — правка параметров
/// генератора. check (медиа существует), мутация видна всем клипам этого
/// медиа, undo/redo работают, живой драг окна схлопывается в ОДИН шаг.
#[test]
fn t475_set_media_path_command() {
    use kt_model::{
        ClipEvent, ClipId, ClipKind, Command, CommandError, MediaId, Project, ProjectSettings,
        TimelineModel,
    };
    use kt_time::Tick;

    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(kt_model::MediaRef { id: MediaId(1), path: "gen:solid".into() });
    p.clips.push(ClipEvent {
        r#gen: Some(kt_model::GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        gen_auto: Vec::new(),
        overrides: Vec::new(),
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
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    let mut m = TimelineModel::new(p);

    // несуществующее медиа — честный отказ
    assert!(matches!(
        m.apply(Command::SetMediaPath { id: MediaId(9), path: "gen:solid".into() }),
        Err(CommandError::MediaNotFound(MediaId(9)))
    ));

    // Р.59, стадия 2: путь медиа несёт ТОЛЬКО ВИД, поэтому полный путь
    // приводится к канону, а значения достаются событию. Прежнее ожидание
    // («путь хранит параметры») снято решением владельца «параметры
    // генератора — на событие».
    m.apply(Command::SetMediaPath { id: MediaId(1), path: "gen:solid:c=ff8800".into() }).unwrap();
    assert_eq!(m.project().media[0].path, "gen:solid", "в пути остаётся вид");
    assert!(m.project().is_gen(&m.project().clips[0]));

    // смена ВИДА переодевает события этого медиа: иначе первая же правка
    // упёрлась бы в «вид значений не тот, что в пути»
    m.apply(Command::SetMediaPath { id: MediaId(1), path: "gen:gradient".into() }).unwrap();
    assert_eq!(m.project().media[0].path, "gen:gradient");
    assert!(
        matches!(m.project().clips[0].r#gen, Some(kt_model::GeneratorKind::Gradient { .. })),
        "событие обязано переодеться вместе с медиа"
    );

    // undo возвращает прежний вид ВМЕСТЕ со значениями события
    m.undo().unwrap();
    assert_eq!(m.project().media[0].path, "gen:solid");
    assert!(matches!(m.project().clips[0].r#gen, Some(kt_model::GeneratorKind::Solid { .. })));
    m.redo().unwrap();
    assert_eq!(m.project().media[0].path, "gen:gradient");

    // сессия окна: три правки одним жестом = ОДИН undo-шаг
    let before_depth = m.undo_depth();
    for c in ["gen:checker", "gen:corners", "gen:transparent"] {
        m.apply_coalesced(Command::SetMediaPath { id: MediaId(1), path: c.into() }, 42).unwrap();
    }
    assert_eq!(m.project().media[0].path, "gen:transparent");
    assert_eq!(m.undo_depth(), before_depth + 1, "жест схлопнулся в один шаг");
    m.undo().unwrap();
    assert_eq!(m.project().media[0].path, "gen:gradient", "undo жеста — до сессии");
}

/// Тест 510 (Р.53, п.414): флаги видов — полный паспорт. Каждое
/// bool-поле вида ОБЯЗАНО быть в `flag_specs`: иначе оно существует в
/// формате, а включить его нечем (так с Р.52 жили тон 1 кГц у SMPTE и
/// цветной режим шума). Полнота проверяется по serde-представлению —
/// оно видит ВСЕ поля, а не те, что вспомнил автор.
#[test]
fn t510_generator_flags_are_complete_and_roundtrip() {
    use kt_model::GeneratorKind;
    for k in GeneratorKind::all() {
        let v = serde_json::to_value(&k).expect("вид сериализуется");
        let obj = v.as_object().expect("вид — объект");
        let bools: Vec<&str> =
            obj.iter().filter(|(_, v)| v.is_boolean()).map(|(k, _)| k.as_str()).collect();
        let declared: Vec<&str> = k.flag_specs().iter().map(|s| s.name).collect();
        for b in &bools {
            assert!(
                declared.contains(b),
                "{}: поле {b} есть в формате, но не объявлено флагом — включить его нечем",
                k.tag()
            );
        }
        for d in &declared {
            assert!(bools.contains(d), "{}: флаг {d} не существует в формате", k.tag());
        }
        // чтение и запись ходят по каждому имени, путь переживает круг
        let mut probe = k.clone();
        for spec in k.flag_specs() {
            let was = probe.get_flag(spec.name).expect("флаг читается");
            assert!(probe.set_flag(spec.name, !was), "флаг {} не пишется", spec.name);
            assert_eq!(probe.get_flag(spec.name), Some(!was));
        }
        let path = kt_model::format_gen_path(&probe);
        assert_eq!(kt_model::parse_gen_path(&path), Some(probe), "{path}: флаги не пережили путь");
    }

    // и главное следствие: «тон включён» и «цветной шум» выразимы
    let tone = GeneratorKind::SmpteBars { tone: true, tone_db: -20.0 };
    assert_eq!(tone.get_flag("tone"), Some(true));
    let colour = GeneratorKind::NoiseField { amount: 1.0, mono: false };
    assert_eq!(colour.get_flag("mono"), Some(false));
}

/// Тест 524 (Р.53, пп.419-422): новые виды ездят по пути туда и обратно,
/// а канон остаётся каноном.
///
/// Правило разбора одно на все виды: `format(parse(s)) == format(s)`,
/// умолчания не пишутся, битое значение — `None`, а не тихий кламп.
#[test]
fn t524_new_kinds_round_trip_through_the_path() {
    use kt_model::{TestTable, format_gen_path, parse_gen_path};
    // каждый вид реестра: умолчания дают ГОЛЫЙ путь без токенов
    for k in GeneratorKind::all() {
        let p = format_gen_path(&k);
        assert_eq!(parse_gen_path(&p).as_ref(), Some(&k), "{p}: путь не вернул свой вид");
        assert_eq!(format_gen_path(&parse_gen_path(&p).unwrap()), p, "{p}: канон не канон");
    }
    // разновидность таблицы — ОДИН тег на пять видов
    for t in TestTable::ALL {
        let k = GeneratorKind::TestPattern { table: t };
        let p = format_gen_path(&k);
        assert!(p.starts_with("gen:table"), "{p}");
        assert_eq!(parse_gen_path(&p), Some(k), "{p}");
    }
    assert_eq!(format_gen_path(&GeneratorKind::Transparent), "gen:transparent");
    assert_eq!(parse_gen_path("gen:transparent"), Some(GeneratorKind::Transparent));
    assert_eq!(parse_gen_path("gen:transparent:c=ff0000"), None, "у подложки параметров нет");

    // значения вне диапазона — ошибка, а не молчаливый кламп
    assert_eq!(parse_gen_path("gen:rect:cx=1.5"), None);
    assert_eq!(parse_gen_path("gen:rect:w=3"), None, "размер больше двух долей оси");
    assert!(parse_gen_path("gen:rect:w=1.8").is_some(), "фигура вправе вылезти за кадр");
    assert_eq!(parse_gen_path("gen:line:th=nan"), None);
    assert_eq!(parse_gen_path("gen:table:tb=nope"), None);
    assert_eq!(parse_gen_path("gen:leader:n=0"), None);

    // цвет фигуры — С АЛЬФОЙ (восемь hex-цифр)
    let path = "gen:rect:c=ff8800c0";
    let Some(GeneratorKind::Rect { color_r, color_a, .. }) = parse_gen_path(path) else {
        panic!("{path} не разобрался");
    };
    assert_eq!((color_r, color_a), (0xff, 0xc0));
    assert_eq!(parse_gen_path("gen:rect:c=ff8800"), None, "шесть цифр — это путь без альфы");
}

/// Тест 525 (Р.53, п.419): «вид = вариант enum тогда, когда у него СВОИ
/// числа». Сторож против объединения фигур в один вид с мёртвыми
/// контролами (правило UI 9) и против пяти копий одного кода у таблиц.
#[test]
fn t525_kinds_differ_by_their_numbers() {
    use std::collections::BTreeSet;
    let names = |k: &GeneratorKind| -> BTreeSet<String> {
        k.param_specs().iter().map(|s| s.name.to_string()).collect()
    };
    let rect = GeneratorKind::default_for_tag("rect").unwrap();
    let ell = GeneratorKind::default_for_tag("ellipse").unwrap();
    let line = GeneratorKind::default_for_tag("line").unwrap();
    assert_ne!(names(&rect), names(&ell), "у прямоугольника и эллипса одинаковые числа");
    assert_ne!(names(&ell), names(&line));
    assert!(names(&rect).contains("radius"), "скругление есть только у прямоугольника");
    assert!(!names(&ell).contains("radius"), "у эллипса скругление было бы мёртвым контролом");

    // у таблиц чисел нет вовсе — поэтому вид ОДИН, а разновидность поле
    for t in kt_model::TestTable::ALL {
        assert!(GeneratorKind::TestPattern { table: t }.param_specs().is_empty());
    }
}

// ---- Р.59, стадия 2: параметры генератора на СОБЫТИИ ----------------------

/// Тест 656: модель несёт значения генератора на событии, а путь медиа —
/// ТОЛЬКО ВИД; байты файлов без генераторов при этом не двигаются.
///
/// Три утверждения, и каждое держит свой кусок раунда: «в пути остаётся
/// вид» (иначе два титра с разным текстом снова стали бы двумя медиа),
/// «сорт варианта из пути не уходит» (имя вида есть функция значения —
/// правило п.419), «пустые поля не сериализуются» (иначе прежние проекты
/// перестали бы быть байт-стабильными).
#[test]
fn t656_generator_values_live_on_the_event() {
    use kt_model::{ClipEvent, ClipId, ClipKind, GeneratorKind, MediaId, TestTable, gen_kind_path};
    use kt_time::Tick;

    // путь несёт вид; значения из него вычищены
    let mut orange = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    assert_eq!(gen_kind_path(&orange), "gen:solid");
    orange = GeneratorKind::default_text();
    if let GeneratorKind::Text { text, size, .. } = &mut orange {
        *text = "Заголовок".into();
        *size = 0.3;
    }
    assert_eq!(gen_kind_path(&orange), "gen:text", "текст титра в пути медиа не живёт");

    // СОРТ варианта — часть адреса: пять таблиц это пять строк меню, и
    // имя вида у них функция значения
    assert_eq!(gen_kind_path(&GeneratorKind::TestPattern { table: TestTable::Ebu75 }), "gen:table");
    assert_eq!(
        gen_kind_path(&GeneratorKind::TestPattern { table: TestTable::Grayscale }),
        "gen:table:tb=grey"
    );
    // и путь вида всегда разбирается обратно в ТОТ ЖЕ вид
    for k in GeneratorKind::all() {
        let back = kt_model::parse_gen_path(&gen_kind_path(&k)).expect("путь вида разбирается");
        assert_eq!(back.tag(), k.tag(), "вид не пережил круг через путь");
        assert_eq!(gen_kind_path(&back), gen_kind_path(&k), "путь вида не канонический");
    }

    // пустые поля события не сериализуются: файл без генератора байт-в-байт
    let plain = ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
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
    };
    let json = serde_json::to_string(&plain).unwrap();
    assert!(!json.contains("\"gen\""), "поле gen попало в файл без генератора: {json}");
    assert!(!json.contains("gen_auto"), "поле gen_auto попало в файл без генератора");

    // а с генератором — переживает круг
    let mut g = plain.clone();
    g.r#gen = Some(GeneratorKind::Solid { color_r: 1, color_g: 2, color_b: 3 });
    g.gen_auto = kt_model::text_anim_curves(kt_model::TextAnim::Typewriter, 1.0, 2.0);
    let back: ClipEvent = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(back, g, "значения и кривые генератора не пережили круг через serde");

    // адресное пространство: кусок морфер-слоя тоже помещается под GEN_TAG
    let slice = kt_model::slice_id(ClipId(kt_model::MAX_REAL_CLIP_ID - 1), u16::MAX as u32);
    assert!(slice.0 < kt_model::GEN_TAG, "адрес куска залез в диапазон генератора");
    let addr = kt_model::GEN_TAG + slice.0;
    assert_eq!(addr, kt_model::GEN_TAG | slice.0, "тег генератора обязан не пересекаться с id");
    assert!(addr < kt_model::NEST_TAG, "адрес генератора залез в диапазон ленты");
}

/// Тест 657: валидация значений генератора — БЕЗ ЛЕГАСИ.
///
/// Строгость здесь бесплатна: значения появляются и исчезают ровно
/// вместе с медиа (`AddClip`/`SetClipMedia`/`SetMediaPath` держат это
/// сами), поэтому любое расхождение — это испорченный файл, а не
/// состояние, в котором кто-то мог оказаться честно.
#[test]
fn t657_generator_values_are_validated_against_the_media() {
    use kt_model::{
        AutoKey, ClipEvent, ClipId, ClipKind, GeneratorKind, MediaId, MediaRef, ParamCurve,
        Project, ProjectError, ProjectSettings, TestTable,
    };
    use kt_time::Tick;

    let mk = || {
        let mut p = Project::new(ProjectSettings {
            fps: 30,
            width: 64,
            height: 36,
            sample_rate: 48000,
            pitch_engine: Default::default(),
        });
        p.media.push(MediaRef { id: MediaId(1), path: "gen:solid".into() });
        p.media.push(MediaRef { id: MediaId(2), path: "movie.mp4".into() });
        p.clips.push(ClipEvent {
            r#gen: Some(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
            gen_auto: Vec::new(),
            overrides: Vec::new(),
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
        p
    };
    mk().validate().expect("согласованное состояние законно");

    // на генераторе, но без значений
    let mut p = mk();
    p.clips[0].r#gen = None;
    assert!(matches!(p.validate(), Err(ProjectError::GenMissing(ClipId(1)))));

    // значения на ФАЙЛЕ
    let mut p = mk();
    p.clips[0].media = MediaId(2);
    assert!(matches!(p.validate(), Err(ProjectError::GenOnFile(ClipId(1)))));

    // вид значений не тот, что в пути медиа
    let mut p = mk();
    p.clips[0].r#gen = Some(GeneratorKind::TestPattern { table: TestTable::Ebu75 });
    assert!(matches!(p.validate(), Err(ProjectError::GenKindMismatch(ClipId(1)))));
    // и СОРТ — тоже часть вида
    let mut p = mk();
    p.media[0].path = "gen:table".into();
    p.clips[0].r#gen = Some(GeneratorKind::TestPattern { table: TestTable::Grayscale });
    assert!(matches!(p.validate(), Err(ProjectError::GenKindMismatch(ClipId(1)))));

    // кривая-сирота ОТВЕРГАЕТСЯ: вид у события ровно один, взяться сироте
    // неоткуда, а молча не действующая кривая выглядит как «анимация
    // пропала» (у `fx_auto` правило другое — там слот мог сменить вид)
    let curve = |name: &str| ParamCurve {
        param: name.into(),
        ease: false,
        keys: vec![AutoKey { t: Tick(0), v: 0.0 }, AutoKey { t: Tick(90_000), v: 255.0 }],
        source: Default::default(),
    };
    let mut p = mk();
    p.clips[0].gen_auto = vec![curve("color_r")];
    p.validate().expect("кривая на существующий параметр законна");
    let mut p = mk();
    p.clips[0].gen_auto = vec![curve("nope")];
    assert!(matches!(p.validate(), Err(ProjectError::GenAutoOrphan(ClipId(1), _))));

    // структурная проверка: ключи по возрастанию, значения конечны
    let mut p = mk();
    let mut bad = curve("color_r");
    bad.keys.reverse();
    p.clips[0].gen_auto = vec![bad];
    assert!(matches!(p.validate(), Err(ProjectError::BadFxAuto(ClipId(1)))));

    // источник кривой, которого мы ещё не умеем считать, — громкий отказ
    let mut p = mk();
    let mut formula = curve("color_r");
    formula.source = kt_model::CurveSource::Formula { expr: "t".into() };
    p.clips[0].gen_auto = vec![formula];
    assert!(matches!(p.validate(), Err(ProjectError::CurveSourceUnsupported(ClipId(1)))));
}

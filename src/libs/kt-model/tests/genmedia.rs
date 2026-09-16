//! Тесты словаря генераторов (Р.52, п.402): разбор/сборка `gen:`-пути,
//! экранирование строковых значений, полнота спек параметров.

use kt_model::{
    escape_gen_value, format_gen_path, is_pseudo_path, parse_gen_path, unescape_gen_value,
    GeneratorKind, HasParams,
};

/// t469: `format(parse(s))` канонизирует, разбор регистронезависимый
/// (регресс сноски п.211: `synth:Ramp` молча считался отсутствующим
/// файлом), мусор — честный `None`, умолчания не пишутся.
#[test]
fn t469_gen_path_parse_format_canonical() {
    // Канонический roundtrip всех видов реестра с умолчаниями.
    for k in GeneratorKind::ALL {
        let path = format_gen_path(k);
        assert!(path.starts_with("gen:"), "{path}");
        assert_eq!(parse_gen_path(&path).as_ref(), Some(k), "{path}");
        assert_eq!(format_gen_path(&parse_gen_path(&path).unwrap()), path);
    }

    // Умолчания не сериализуются: чёрный solid — голый тег.
    assert_eq!(
        format_gen_path(&GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        "gen:solid"
    );

    // Не-умолчание пишется одним цветовым токеном, нижним регистром.
    let orange = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    assert_eq!(format_gen_path(&orange), "gen:solid:c=ff8800");
    assert_eq!(parse_gen_path("gen:solid:c=ff8800"), Some(orange));

    // Регистр схемы, тега, ключа и hex-значения безразличен.
    assert_eq!(parse_gen_path("Gen:SOLID:C=FF8800"), Some(orange));
    // ...а канонизация возвращает нижний регистр.
    assert_eq!(format_gen_path(&parse_gen_path("GEN:Solid:c=FF8800").unwrap()), "gen:solid:c=ff8800");

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
    for k in GeneratorKind::ALL {
        let specs = k.param_specs();
        assert!(!specs.is_empty(), "{}: пустые спеки", k.tag());
        let mut probe = *k;
        for s in specs {
            assert!(
                s.min <= s.default && s.default <= s.max,
                "{}.{}: умолчание вне диапазона",
                k.tag(),
                s.name
            );
            let v = k.get_param(s.name).unwrap_or_else(|| {
                panic!("{}.{}: get_param не знает своё имя", k.tag(), s.name)
            });
            assert_eq!(v, s.default, "{}.{}: реестр не на умолчании", k.tag(), s.name);
            assert!(probe.set_param(s.name, s.max), "{}.{}: set_param", k.tag(), s.name);
            assert_eq!(probe.get_param(s.name), Some(s.max));
        }
        assert_eq!(k.get_param("no_such_param"), None);
        assert!(!probe.set_param("no_such_param", 1.0));
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

    // правка видна: путь сменился, клип ссылается на то же медиа
    m.apply(Command::SetMediaPath { id: MediaId(1), path: "gen:solid:c=ff8800".into() })
        .unwrap();
    assert_eq!(m.project().media[0].path, "gen:solid:c=ff8800");
    assert!(m.project().is_gen(&m.project().clips[0]));

    // undo возвращает прежний путь
    m.undo().unwrap();
    assert_eq!(m.project().media[0].path, "gen:solid");
    m.redo().unwrap();
    assert_eq!(m.project().media[0].path, "gen:solid:c=ff8800");

    // сессия окна: три правки одним жестом = ОДИН undo-шаг
    let before_depth = m.undo_depth();
    for c in ["gen:solid:c=110000", "gen:solid:c=220000", "gen:solid:c=330000"] {
        m.apply_coalesced(Command::SetMediaPath { id: MediaId(1), path: c.into() }, 42).unwrap();
    }
    assert_eq!(m.project().media[0].path, "gen:solid:c=330000");
    assert_eq!(m.undo_depth(), before_depth + 1, "жест схлопнулся в один шаг");
    m.undo().unwrap();
    assert_eq!(m.project().media[0].path, "gen:solid:c=ff8800", "undo жеста — до сессии");
}

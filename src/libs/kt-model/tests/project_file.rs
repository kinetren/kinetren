//! Тесты файла проекта — СВОЕГО формата (Р.70): круг поле-в-поле и
//! байт-в-байт, размер ∝ модели, версия, индексируемость в git.
//!
//! Половина t810, которой нужен `.otio` для сравнения («на порядки меньше
//! того же проекта в формате обмена»), живёт в `kt-otio/tests/convert.rs`:
//! этот крейт о формате обмена не знает и знать не должен.

use kt_model::project_file::{
    EXTENSION, FileKind, MARKER, PROJECT_FILE_VERSION, ProjectFileError, from_text,
    is_project_file, to_text,
};
use kt_model::*;
use kt_time::Tick;

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn clip(id: u64, media: u64, start: i64, dur: i64) -> ClipEvent {
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
        start: Tick(start),
        duration: Tick(dur),
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

/// Богатый проект: лента с кроссфейдом, морфером и камерой, два её
/// экземпляра, корневой переход, метка, регион, настройки — всё, что
/// обязано пережить круг.
fn rich_project() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "media/a.mp4"));
    p.media.push(MediaRef::new(MediaId(2), "media/b.mp4"));

    let mut s = Sequence::new(MediaId(100), "Заставка");
    s.clips.push(clip(11, 1, 0, 45_000));
    s.clips.push(clip(12, 2, 45_000, 45_000));
    s.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(11)),
        b: Some(ClipId(12)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    s.morphers.push(MorpherEvent {
        id: MorphId(3),
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        targets: MorphTargets::Below,
        patch: Default::default(),
        enabled: true,
        timed: true,
        strength: 1.0,
        fade_in: Tick(0),
        fade_out: Tick(0),
    });
    s.ensure_tracks(LaneKind::Morph, 1);
    p.sequences.push(s);

    // два экземпляра ленты и обычное событие между ними
    p.clips.push(clip(1, 100, 0, 90_000));
    p.clips.push(clip(2, 1, 90_000, 30_000));
    p.clips.push(clip(3, 100, 120_000, 90_000));
    p.transitions.push(TransitionEvent {
        id: TransitionId(2),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick(3_000),
        out_offset: Tick(3_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.markers.push(Marker {
        t: Tick(60_000),
        duration: Tick(0),
        name: "сцена".into(),
        color: Some("blue".into()),
    });
    p.region = Some(Region { start: Tick(0), end: Tick(100_000) });
    p.loop_enabled = true;
    // КАНОНИЧЕСКАЯ модель, как после открытия: `ensure_tracks` даёт дорожкам
    // ленты заглушку `TrackId(0)`, а чтение раздаёт настоящие id. Сравнивать
    // круг надо с тем, что программа пишет на самом деле, а она пишет
    // нормализованное.
    p.normalize();
    p.validate().expect("сцена законна");
    p
}

/// Чужой passthrough: намеренно НЕ по алфавиту, с вложенностью.
fn foreign_passthrough() -> serde_json::Value {
    serde_json::json!({
        "zeta": {"b": 1, "a": [3, {"y": 1, "x": 2}]},
        "alpha": "чужой неймспейс",
        "markers": [{"name": "их метка", "color": "RED"}],
    })
}

// ---- t811. круг поле-в-поле и байт-в-байт ----------------------------------

#[test]
fn t811_round_trip_is_field_exact_and_byte_stable() {
    let p = rich_project();
    let text = to_text(&p, Some(&foreign_passthrough()));

    let back = from_text(&text).expect("свой файл читается");
    assert_eq!(back.project, p, "модель после круга обязана совпасть поле-в-поле");
    assert!(back.otio.is_some(), "чужой passthrough потерян на круге");

    let text2 = to_text(&back.project, back.otio.as_ref());
    assert_eq!(text2, text, "второй круг обязан быть байт-в-байт");

    // признак и версия стоят ПЕРВОЙ строкой: незнакомую версию отвергают, не
    // читая остального
    let first = text.lines().nth(1).unwrap_or_default();
    assert!(
        first.contains(&format!("\"{MARKER}\": {PROJECT_FILE_VERSION}")),
        "версия обязана быть первым полем файла, а стоит: {first}"
    );
    assert!(text.ends_with('\n'), "файл обязан кончаться переводом строки");
    assert_eq!(EXTENSION, "kt");
}

// ---- t810 (половина без .otio). размер ∝ модели, а не числу экземпляров ----

#[test]
fn t810_a_second_instance_adds_a_reference_not_a_body() {
    let mut p = rich_project();
    // убираем второй экземпляр: считаем прирост от его возвращения
    p.clips.retain(|c| c.id != ClipId(3));
    p.validate().unwrap();
    let one = to_text(&p, None);

    p.clips.push(clip(3, 100, 120_000, 90_000));
    p.validate().unwrap();
    let two = to_text(&p, None);

    p.clips.push(clip(4, 100, 210_000, 90_000));
    p.validate().unwrap();
    let three = to_text(&p, None);

    // тело ленты (два события + переход + морфер) весит заведомо больше
    // одной ссылки на неё; прирост на экземпляр — ссылка
    let body = {
        let mut only_body = Project::new(settings());
        only_body.media = p.media.clone();
        only_body.sequences = p.sequences.clone();
        to_text(&only_body, None).len() - to_text(&Project::new(settings()), None).len()
    };
    let step_2 = two.len() - one.len();
    let step_3 = three.len() - two.len();
    assert!(
        step_2 < body / 2 && step_3 < body / 2,
        "экземпляр прибавляет к файлу ссылку, а не тело: прирост {step_2} и {step_3} байт \
         при теле ленты {body} байт"
    );
    assert!(
        step_3.abs_diff(step_2) < 64,
        "каждый следующий экземпляр стоит столько же, сколько предыдущий: {step_2} против {step_3}"
    );
}

// ---- t812. версия: отказ вперёд, чужой файл — дверь импорта ----------------

#[test]
fn t812_version_refuses_forward_and_names_the_import_door() {
    let p = rich_project();
    let text = to_text(&p, None);

    // версия БОЛЬШЕ нынешней — отказ с номером, частичного чтения нет
    let newer = text.replacen(
        &format!("\"{MARKER}\": {PROJECT_FILE_VERSION}"),
        &format!("\"{MARKER}\": {}", PROJECT_FILE_VERSION + 7),
        1,
    );
    assert_ne!(newer, text, "сцена: версия в файле найдена");
    match from_text(&newer) {
        Err(ProjectFileError::TooNew { version }) => {
            assert_eq!(version, PROJECT_FILE_VERSION + 7)
        }
        other => panic!("файл будущей версии обязан отвергаться с номером, а не {other:?}"),
    }
    let said = from_text(&newer).unwrap_err().to_string();
    assert!(said.contains(&(PROJECT_FILE_VERSION + 7).to_string()), "ошибка не назвала версию: {said}");

    // файл обмена — не проект; ошибка называет дверь импорта
    let otio = r#"{"OTIO_SCHEMA": "Timeline.1", "name": "x", "tracks": {}}"#;
    match from_text(otio) {
        Err(ProjectFileError::NotOurs { looks_like: Some(FileKind::Otio) }) => {}
        other => panic!("файл обмена обязан опознаваться как OTIO, а не {other:?}"),
    }
    let said = from_text(otio).unwrap_err().to_string();
    assert!(said.contains("Import older Kinetren project"), "ошибка не назвала дверь: {said}");
    assert!(!is_project_file(otio) && is_project_file(&text));

    // легаси-проект — тоже дверь импорта, своим именем
    let legacy = r#"{"format_version": 1, "settings": {}}"#;
    match from_text(legacy) {
        Err(ProjectFileError::NotOurs { looks_like: Some(FileKind::LegacyKtproj) }) => {}
        other => panic!("легаси обязан опознаваться как .ktproj, а не {other:?}"),
    }

    // просто чужой JSON — не наш, и это сказано без догадок
    match from_text(r#"{"hello": 1}"#) {
        Err(ProjectFileError::NotOurs { looks_like: None }) => {}
        other => panic!("чужой JSON: {other:?}"),
    }

    // битая модель внутри НАШЕГО конверта — ошибка модели, не «не наш файл»
    let broken = text.replacen("\"fps\": 30", "\"fps\": 7", 1);
    assert_ne!(broken, text);
    assert!(
        matches!(from_text(&broken), Err(ProjectFileError::Model(_))),
        "битая модель обязана давать ошибку модели"
    );
}

// ---- t822. индексируем в git ----------------------------------------------

#[test]
fn t822_the_file_is_indexable_in_git() {
    // полсотни событий
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "media/a.mp4"));
    for i in 0..50u64 {
        p.clips.push(clip(1000 + i, 1, i as i64 * 45_000, 45_000));
    }
    p.validate().unwrap();
    let a = to_text(&p, None);

    // построчный: строк не меньше, чем событий (компактный дал бы одну)
    let lines_a: Vec<&str> = a.lines().collect();
    assert!(
        lines_a.len() >= 50,
        "файл обязан быть построчным: {} строк на 50 событий",
        lines_a.len()
    );

    // правка ОДНОГО события меняет единицы строк, а не файл
    let last = p.clips.len() - 1;
    p.clips[last].start = p.clips[last].start + Tick(1_000);
    let b = to_text(&p, None);
    let lines_b: Vec<&str> = b.lines().collect();
    assert_eq!(lines_a.len(), lines_b.len(), "сдвиг события не меняет число строк");
    let changed = lines_a.iter().zip(&lines_b).filter(|(x, y)| x != y).count();
    assert!(changed <= 2, "правка одного события изменила {changed} строк, а обязана единицы");

    // детерминизм: та же модель — те же байты, и с чужим passthrough тоже
    assert_eq!(to_text(&p, None), to_text(&p.clone(), None));
    let fp = foreign_passthrough();
    let with = to_text(&p, Some(&fp));
    assert_eq!(with, to_text(&p.clone(), Some(&fp.clone())));

    // passthrough записан КАК ЕСТЬ: порядок чужих ключей — обещание файлу
    // обмена (экспорт возвращает их байт-в-байт, t154), и сортировать его
    // здесь нельзя. Детерминизм — да, перестановка — нет.
    let z = with.find("\"zeta\"").expect("passthrough в файле");
    let al = with.find("\"alpha\"").expect("passthrough в файле");
    let m = with.find("\"markers\"").expect("passthrough в файле");
    assert!(z < al && al < m, "порядок чужих ключей обязан сохраниться: zeta, alpha, markers");
    let y_pos = with.find("\"y\": 1").expect("y");
    let x_pos = with.find("\"x\": 2").expect("x");
    assert!(y_pos < x_pos, "и вложенный порядок тоже: y перед x, как в исходнике");
}

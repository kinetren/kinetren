//! Тест 188 (Р.29): камеры-события и камерные дорожки в модели.
//! Правило активной камеры детерминировано (рендер зависит от него),
//! команды undo-able, validate ловит битые события.

use kt_model::*;
use kt_time::Tick;

fn cam(id: u64, lane: u32, start: i64, dur: i64) -> CameraEvent {
    CameraEvent {
        id: CameraId(id),
        lane,
        start: Tick(start),
        duration: Tick(dur),
        name: String::new(),
        cam: CameraParams::default(),
        keyframes: vec![],
        auto: vec![],
        ease: true,
    }
}

fn project() -> Project {
    Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    })
}

#[test]
fn t188_active_camera_rule() {
    let mut p = project();
    p.camera_lanes = 2;
    // три камеры: перекрытие по времени на разных дорожках и на одной
    p.cameras = vec![
        cam(1, 1, 0, 200_000),      // нижняя дорожка, рано
        cam(2, 0, 50_000, 100_000), // верхняя дорожка — приоритет
        cam(3, 1, 100_000, 80_000), // та же нижняя дорожка, позже
    ];
    p.validate().unwrap();
    // вне всех событий — None (движок возьмёт дефолтную «50 мм»)
    assert!(p.active_camera_at(Tick(300_000)).is_none());
    // накрыта только камера 1
    assert_eq!(p.active_camera_at(Tick(10_000)).unwrap().id, CameraId(1));
    // меньшая lane важнее (как z-order видео)
    assert_eq!(p.active_camera_at(Tick(120_000)).unwrap().id, CameraId(2));
    // при равной lane — больший start («сверху тот, кто позже», Vegas)
    assert_eq!(p.active_camera_at(Tick(160_000)).unwrap().id, CameraId(3));
    // границы: start включён, конец исключён
    assert_eq!(p.active_camera_at(Tick(50_000)).unwrap().id, CameraId(2));
    assert_eq!(p.active_camera_at(Tick(150_000)).unwrap().id, CameraId(3));
    assert_eq!(p.active_camera_at(Tick(180_000)).unwrap().id, CameraId(1));
}

#[test]
fn t188_camera_commands_undo() {
    let mut m = TimelineModel::new(project());
    assert!(m.apply(Command::InsertCameraLane { at: 0 }).is_ok());
    assert!(m.apply(Command::AddCamera(cam(1, 0, 0, 90_000))).is_ok());
    assert_eq!(m.project().camera_lanes, 1);
    assert_eq!(m.project().cameras.len(), 1);
    // дубль id — отказ
    assert!(m.apply(Command::AddCamera(cam(1, 0, 10, 100))).is_err());
    // замена параметров
    let mut c2 = cam(1, 0, 0, 90_000);
    c2.cam.fov_deg = 60.0;
    c2.name = "wide".into();
    assert!(m.apply(Command::SetCamera(c2.clone())).is_ok());
    assert_eq!(m.project().cameras[0].cam.fov_deg, 60.0);
    // битые события не проходят
    let mut bad = cam(9, 0, 0, 0);
    assert!(m.apply(Command::AddCamera(bad.clone())).is_err(), "нулевая длительность");
    bad.duration = Tick(100);
    bad.cam.fov_deg = 200.0;
    assert!(m.apply(Command::AddCamera(bad.clone())).is_err(), "fov вне (0,180)");
    bad.cam.fov_deg = 50.0;
    bad.keyframes = vec![
        CameraKeyframe { t: 0.9, cam: CameraParams::default() },
        CameraKeyframe { t: 0.1, cam: CameraParams::default() },
    ];
    assert!(m.apply(Command::AddCamera(bad)).is_err(), "кейфреймы не отсортированы");
    // удаление дорожки уносит события; undo возвращает всё
    assert!(m.apply(Command::RemoveCameraLane { at: 0 }).is_ok());
    assert_eq!(m.project().camera_lanes, 0);
    assert!(m.project().cameras.is_empty());
    m.undo().unwrap();
    assert_eq!(m.project().cameras.len(), 1);
    assert_eq!(m.project().cameras[0].name, "wide");
    m.undo().unwrap();
    assert_eq!(m.project().cameras[0].name, "");
}

#[test]
fn t188_project_without_cameras_is_byte_stable() {
    // сериализация проекта без камер не обрастает новыми полями
    let p = project();
    let json = serde_json::to_string(&p).unwrap();
    assert!(!json.contains("camera"), "пустые камерные поля не пишутся: {json}");
    // а с камерами — round-trip честный
    let mut p2 = project();
    p2.camera_lanes = 1;
    p2.cameras.push(cam(1, 0, 0, 90_000));
    let json2 = serde_json::to_string(&p2).unwrap();
    let back: Project = serde_json::from_str(&json2).unwrap();
    assert_eq!(back, p2);
}

/// Тест 231 (Р.34 К3): камера анимируется ОБЩИМ механизмом. Кейфреймы
/// прежнего домена (доля события) конвертируются в кривые (тики от начала
/// события), кривые считаются `params_at`, перенос камеры везёт анимацию
/// с собой, а битая кривая валит валидацию, а не молчит.
#[test]
fn t231_camera_curves_replace_keyframes() {
    let mut p = project();
    p.camera_lanes = 1;
    let mut c = cam(1, 0, 90_000, 90_000);
    c.ease = false;
    c.keyframes = vec![
        CameraKeyframe { t: 0.0, cam: CameraParams { pos_x: 0.0, ..Default::default() } },
        CameraKeyframe {
            t: 1.0,
            cam: CameraParams { pos_x: 100.0, fov_deg: 39.6, ..Default::default() },
        },
    ];
    let mut legacy = c.clone();
    assert!(kt_model::migrate_legacy_camera(&mut legacy), "миграция обязана сработать");
    assert!(legacy.keyframes.is_empty(), "легаси-кейфреймы съедены");
    // ровные параметры кривых не заводят: семь прямых линий в редакторе — мусор
    assert_eq!(legacy.auto.len(), 1, "кривая только у изменившегося: {:?}", legacy.auto);
    let px = &legacy.auto[0];
    assert_eq!(px.param, "pos_x");
    assert_eq!(
        px.keys.iter().map(|k| (k.t.0, k.v)).collect::<Vec<_>>(),
        vec![(0, 0.0), (90_000, 100.0)],
        "доля события переведена в тики ОТ НАЧАЛА"
    );

    // значения совпадают со старым механизмом в тех же точках
    for (t, want) in [(90_000i64, 0.0), (135_000, 50.0), (180_000, 100.0)] {
        let old = c.params_at(Tick(t)).pos_x;
        let new = legacy.params_at(Tick(t)).pos_x;
        assert!((old - want).abs() < 1e-9, "старый механизм в {t}: {old}");
        assert!((new - want).abs() < 1e-9, "новый механизм в {t}: {new}");
    }

    // ПЕРЕНОС камеры везёт анимацию с собой (домен — от начала события)
    let mut moved = legacy.clone();
    moved.start = Tick(270_000);
    assert!(
        (moved.params_at(Tick(315_000)).pos_x - 50.0).abs() < 1e-9,
        "середина анимации переехала вместе с камерой"
    );

    // кривая на несуществующий параметр — отказ валидации, а не тишина
    p.cameras = vec![legacy.clone()];
    p.validate().expect("камера с кривыми валидна");
    p.cameras[0].auto[0].param = "no_such".into();
    assert!(p.validate().is_err(), "кривая-сирота обязана валить загрузку");
    p.cameras[0].auto[0].param = "pos_x".into();
    p.cameras[0].auto[0].keys[0].t = Tick(-5);
    assert!(p.validate().is_err(), "ключ до начала события — отказ");

    // кривые НОВОГО домена главнее легаси-кейфреймов
    let mut both = c.clone();
    both.auto = vec![ParamCurve {
        param: "pos_x".into(),
        ease: false,
        keys: vec![AutoKey { t: Tick(0), v: 7.0 }],
    }];
    assert!(!kt_model::migrate_legacy_camera(&mut both), "миграция не трогает готовые кривые");
    assert!((both.params_at(Tick(135_000)).pos_x - 7.0).abs() < 1e-9);
}

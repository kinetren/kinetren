//! Тест 545 (docs/porting-notes.md, п.429, Р.54): правило разрешения
//! 3D-камеры — ОДНО на программу.
//!
//! До Р.54 оно жило инлайном в приватной `layer_src`, и второй потребитель
//! (миниатюры пикера) физически не мог его позвать — только переписать.
//! Переписанное правило разошлось бы молча: вариант `Pan 3D` в пикере
//! выглядел бы иначе, чем тот же вариант после клика.

use kt_engine::resolve_cam3d;
use kt_model::{CameraEvent, CameraId, CameraParams, FxInstance};
use kt_time::Tick;

fn cam(id: u64, lane: u32, start: i64, dur: i64, rot_y: f64) -> CameraEvent {
    CameraEvent {
        ease: true,
        id: CameraId(id),
        lane,
        start: Tick(start),
        duration: Tick(dur),
        name: String::new(),
        cam: CameraParams { rot_y, ..CameraParams::default() },
        keyframes: vec![],
        auto: vec![],
    }
}

fn pan3d(camera: Option<CameraId>) -> FxInstance {
    FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale: 1.0,
        camera,
    }
}

#[test]
fn t545_camera_resolution_has_one_rule() {
    let cams = [cam(1, 0, 0, 90_000, 11.0), cam(2, 0, 90_000, 90_000, 22.0)];

    // 1) нет Pan3d в цепочке — нет и камеры: Pan3d пропускается, и
    // остальным эффектам камера не нужна
    assert!(
        resolve_cam3d(&[FxInstance::blur(8)], &cams, Tick(0), 1920, 1920).is_none(),
        "a chain without Pan3d asked for a camera"
    );
    assert!(resolve_cam3d(&[], &cams, Tick(0), 1920, 1920).is_none());

    // 2) ПРИБИТАЯ по id побеждает время: событие №1 стоит в [0, 1 c),
    // но прибитая камера действует и во втором секунде
    let pinned = resolve_cam3d(&[pan3d(Some(CameraId(1)))], &cams, Tick(120_000), 1920, 1920)
        .expect("pinned camera");
    assert_eq!(pinned.cam.rot_y, 11.0, "pinned camera lost to the active one");

    // 3) не прибита — активная ПО ВРЕМЕНИ
    let active =
        resolve_cam3d(&[pan3d(None)], &cams, Tick(120_000), 1920, 1920).expect("active camera");
    assert_eq!(active.cam.rot_y, 22.0);
    let first = resolve_cam3d(&[pan3d(None)], &cams, Tick(0), 1920, 1920).expect("active camera");
    assert_eq!(first.cam.rot_y, 11.0);

    // 4) прибита по НЕСУЩЕСТВУЮЩЕМУ id — падаем на активную, а не на
    // дефолт: камеру могли удалить, а слой пережил её
    let gone = resolve_cam3d(&[pan3d(Some(CameraId(99)))], &cams, Tick(0), 1920, 1920)
        .expect("fallback camera");
    assert_eq!(gone.cam.rot_y, 11.0);

    // 5) камер нет вовсе — дефолтная «50 мм», то есть тождество для Pan3d
    // с дефолтами (на этом стоит t298 и голдены)
    let def = resolve_cam3d(&[pan3d(None)], &[], Tick(0), 1920, 1920).expect("default camera");
    assert_eq!(def.cam, CameraParams::default());

    // 6) world_scale = размер ЦЕЛИ / размер ПРОЕКТА: мир камеры заморожен
    // форматом в мировых пикселях проекта (Р.29), поэтому масштаб цели
    // входит в него, а не в саму камеру. Миниатюра 320 px в проекте
    // 1920 px обязана получить 1/6, иначе перспектива в пикере будет не
    // та, что после клика
    let small = resolve_cam3d(&[pan3d(None)], &cams, Tick(0), 320, 1920).expect("camera");
    assert!((small.world_scale - 320.0 / 1920.0).abs() < 1e-12, "{}", small.world_scale);
    let full = resolve_cam3d(&[pan3d(None)], &cams, Tick(0), 1920, 1920).expect("camera");
    assert_eq!(full.world_scale, 1.0);
    // нулевой размер проекта не делит на ноль (кламп .max(1))
    let zero = resolve_cam3d(&[pan3d(None)], &cams, Tick(0), 320, 0).expect("camera");
    assert_eq!(zero.world_scale, 320.0);
}

//! Файл проекта СВОЕГО формата против файла обмена (Р.70) — та половина
//! тестов формата, которой нужен `.otio` для сравнения и атомарный писатель.
//! Сам формат (круг, версия, git) проверяется в
//! `kt-model/tests/project_file.rs`: модель о формате обмена не знает.
use std::cell::RefCell;
use std::io;
use std::path::Path;

use kt_model::project_file::{from_text, to_text};
use kt_model::*;
use kt_otio::{AtomicFs, Retained, save_project, write_atomic_with};
use kt_time::Tick;

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

/// Та самая сцена замера: лента L1 из `inner` клипов, L2 из `uses2`
/// экземпляров L1, корень из `uses1` экземпляров L2.
fn nested_scene(inner: u64, uses2: u64, uses1: u64) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "media/m1.mp4"));
    let mut next = 1000u64;
    let mut s1 = Sequence::new(MediaId(100), "L1");
    for i in 0..inner {
        next += 1;
        s1.clips.push(clip(next, 1, i as i64 * 45_000, 45_000));
    }
    let l1 = inner as i64 * 45_000;
    let mut s2 = Sequence::new(MediaId(101), "L2");
    for i in 0..uses2 {
        next += 1;
        s2.clips.push(clip(next, 100, i as i64 * l1, l1));
    }
    let l2 = uses2 as i64 * l1;
    for i in 0..uses1 {
        next += 1;
        p.clips.push(clip(next, 101, i as i64 * l2, l2));
    }
    p.sequences.push(s1);
    p.sequences.push(s2);
    p.normalize();
    p.validate().expect("сцена законна");
    p
}

// ---- t810 (вторая половина). на порядки меньше файла обмена ---------------

#[test]
fn t810_project_file_is_orders_of_magnitude_smaller_than_the_interchange_file() {
    // замер 02.10.2026: .otio 1.97 МБ против 3.9 КБ компактного JSON; теперь
    // файл построчный (≈2× компактного), и порог стоит с запасом
    let p = nested_scene(10, 8, 8);
    let otio = save_project(&p, &Retained::default(), None);
    let ours = to_text(&p, None);
    let ratio = otio.len() / ours.len().max(1);
    assert!(
        ratio >= 100,
        "файл проекта обязан быть на порядки меньше файла обмена: {} байт против {} ({}×)",
        ours.len(),
        otio.len(),
        ratio
    );
    // и это не ценой содержимого: круг цел
    assert_eq!(from_text(&ours).expect("читается").project, p);

    // плоский проект без повторов — тоже меньше, но без порядков: здесь
    // выигрыш только от отсутствия чужой обёртки
    let flat = nested_scene(10, 1, 1);
    let r_flat = save_project(&flat, &Retained::default(), None).len() / to_text(&flat, None).len();
    assert!(r_flat >= 4, "и плоский проект меньше файла обмена: {r_flat}×");
    eprintln!("t810: вложенная сцена {ratio}×, плоская {r_flat}×");
}

// ---- t813. запись честна к отключению питания ----------------------------

/// Подставная ФС: падает на названном шаге. `write_and_sync` и `rename`
/// настоящие, пока не велено упасть, — так проверяется ИСХОД на диске, а не
/// порядок вызовов (его держит t351).
struct FailAt {
    step: &'static str,
    log: RefCell<Vec<String>>,
}

impl AtomicFs for FailAt {
    fn write_and_sync(&self, tmp: &Path, bytes: &[u8]) -> io::Result<()> {
        self.log.borrow_mut().push("write".into());
        if self.step == "write" {
            // «питание пропало посреди записи»: на диске остался огрызок
            std::fs::write(tmp, &bytes[..bytes.len() / 2])?;
            return Err(io::Error::other("power loss during write"));
        }
        std::fs::write(tmp, bytes)
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.log.borrow_mut().push("rename".into());
        if self.step == "rename" {
            return Err(io::Error::other("power loss before rename"));
        }
        std::fs::rename(from, to)
    }
    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        self.log.borrow_mut().push("sync_dir".into());
        Ok(())
    }
}

#[test]
fn t813_project_write_leaves_old_or_new_never_half() {
    let dir = std::env::temp_dir().join(format!("kt-t813-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("film.kt");

    let old = to_text(&nested_scene(3, 1, 1), None);
    let new = to_text(&nested_scene(10, 2, 2), None);
    assert_ne!(old, new);
    std::fs::write(&path, &old).unwrap();

    for step in ["write", "rename"] {
        let fs = FailAt { step, log: RefCell::new(vec![]) };
        let r = write_atomic_with(&fs, &path, &new);
        assert!(r.is_err(), "обрыв на шаге «{step}» обязан быть ОТКАЗОМ, а не успехом");
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, old, "после обрыва на «{step}» на диске обязан лежать ПРЕЖНИЙ проект целиком");
        assert_eq!(from_text(&on_disk).unwrap().project, nested_scene(3, 1, 1));
    }

    // без обрыва — новый целиком, и читается
    let fs = FailAt { step: "", log: RefCell::new(vec![]) };
    write_atomic_with(&fs, &path, &new).expect("запись без обрыва проходит");
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert_eq!(on_disk, new, "после записи на диске обязан лежать НОВЫЙ проект целиком");
    assert_eq!(from_text(&on_disk).unwrap().project, nested_scene(10, 2, 2));
    assert_eq!(fs.log.borrow().as_slice(), ["write", "rename", "sync_dir"]);

    let _ = std::fs::remove_dir_all(&dir);
}

// ---- t816/t817. чужой passthrough переживает файл проекта -----------------

/// Чужой файл обмена с незнакомым полем на Timeline, чужим неймспейсом в
/// metadata, чужим полем у клипа и целым незнакомым узлом — ровно то, что
/// обязано вернуться при экспорте.
fn foreign_document() -> String {
    serde_json::json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": "f",
        "vendor_field": {"deep": [1, 2, {"x": true}]},
        "metadata": {"resolve": {"colorVersion": 3}, "fcp_xml": "keep"},
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "children": [{
                "OTIO_SCHEMA": "Track.1",
                "kind": "Video",
                "children": [
                    {
                        "OTIO_SCHEMA": "Clip.2",
                        "name": "known",
                        "custom_clip_field": 42,
                        "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                            "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 90000.0},
                            "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 900.0, "rate": 90000.0}},
                        "media_references": {"DEFAULT_MEDIA": {
                            "OTIO_SCHEMA": "ExternalReference.1", "target_url": "a.mov",
                            "available_range": null}},
                        "active_media_reference_key": "DEFAULT_MEDIA"
                    },
                    {
                        "OTIO_SCHEMA": "FancyClip.7",
                        "zeta": 1,
                        "alpha": 2,
                        "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                            "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 24.0},
                            "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 24.0, "rate": 24.0}}
                    }
                ]
            }]
        }
    })
    .to_string()
}

#[test]
fn t816_retained_survives_json_round_trip() {
    let bundle = kt_otio::load_project(&foreign_document(), None).expect("чужой файл читается");
    let direct = save_project(&bundle.project, &bundle.retained, None);

    let json = bundle.retained.to_json();
    assert_eq!(json, bundle.retained.to_json(), "to_json обязан быть детерминирован");
    let back = Retained::from_json(&json).expect("passthrough читается обратно");
    assert_eq!(back.to_json(), json, "круг Retained → JSON → Retained обязан быть тождеством");

    // настоящая мера — ЭКСПОРТ: с восстановленным passthrough файл тот же
    let via_json = save_project(&bundle.project, &back, None);
    assert_eq!(via_json, direct, "экспорт через восстановленный passthrough обязан быть байт-в-байт");
    for needle in ["vendor_field", "colorVersion", "custom_clip_field", "FancyClip.7", "\"zeta\""] {
        assert!(via_json.contains(needle), "чужое «{needle}» потеряно на круге");
    }
}

#[test]
fn t817_import_project_export_keeps_foreign_fields() {
    // импорт чужого .otio → файл проекта → чтение → экспорт
    let bundle = kt_otio::load_project(&foreign_document(), None).expect("импорт");
    let text = to_text(&bundle.project, Some(&bundle.retained.to_json()));
    assert!(text.contains("\"otio\""), "passthrough обязан лежать в файле проекта отдельным полем");

    let file = from_text(&text).expect("файл проекта читается");
    let retained = Retained::from_json(file.otio.as_ref().expect("поле passthrough на месте"))
        .expect("passthrough из файла проекта читается");
    let exported = save_project(&file.project, &retained, None);
    assert_eq!(
        exported,
        save_project(&bundle.project, &bundle.retained, None),
        "экспорт после круга через файл проекта обязан совпасть с прямым экспортом"
    );
    // и чужой порядок ключей пережил всё: zeta раньше alpha, как в исходнике
    let z = exported.find("\"zeta\"").unwrap();
    let a = exported.find("\"alpha\"").unwrap();
    assert!(z < a, "порядок чужих ключей обязан сохраниться");

    // проект без passthrough поля не несёт — файл не растёт впустую
    let plain = to_text(&bundle.project, None);
    assert!(!plain.contains("\"otio\""), "пустой passthrough не пишется");
}

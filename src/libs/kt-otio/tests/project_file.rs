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

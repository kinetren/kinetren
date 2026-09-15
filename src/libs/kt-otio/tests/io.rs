//! Тесты 351-353 (Р.49, п.349): атомарная запись честна к отключению
//! питания.
//!
//! `write + rename` защищает от падения процесса, но не от потери
//! питания: переименование может лечь в журнал раньше данных, и на месте
//! проекта окажется файл нулевой длины. Порядок операций и есть всё, ради
//! чего функция написана, — поэтому он проверяется наблюдением, а не
//! фолт-инжекцией ядра (на ext4 её результат прячет `auto_da_alloc`).

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

use kt_otio::{AtomicFs, write_atomic, write_atomic_with};

/// Записывает порядок обращений к файловой системе.
#[derive(Default)]
struct Recorder {
    log: RefCell<Vec<String>>,
    dir_sync_fails: bool,
}

impl AtomicFs for Recorder {
    fn write_and_sync(&self, tmp: &Path, bytes: &[u8]) -> io::Result<()> {
        self.log.borrow_mut().push(format!("write+sync {} ({} bytes)", name(tmp), bytes.len()));
        std::fs::write(tmp, bytes)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.log.borrow_mut().push(format!("rename {} -> {}", name(from), name(to)));
        std::fs::rename(from, to)
    }

    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        self.log.borrow_mut().push("sync dir".into());
        if self.dir_sync_fails {
            // ровно то, что отдают exFAT и сетевые шары
            return Err(io::Error::new(io::ErrorKind::Unsupported, "no directory sync here"));
        }
        Ok(())
    }
}

fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kt-io-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Тест 351: данные на диске ДО переименования, каталог — после.
#[test]
fn t351_atomic_write_syncs_file_then_renames_then_syncs_dir() {
    let dir = tmp_dir("order");
    let path = dir.join("a.b.otio");
    let rec = Recorder::default();
    let w = write_atomic_with(&rec, &path, "hello").expect("write");
    assert!(w.dir_synced, "на обычной ФС каталог обязан сброситься");

    let log = rec.log.borrow().clone();
    // имя временного файла несёт номер процесса (п.352) — в ожиданиях он
    // подставляется, а не захардкожен
    let pid = std::process::id();
    assert_eq!(
        log,
        vec![
            // tmp дописан к ПОЛНОМУ имени — «a.b.otio» не теряет «.b» (t150)
            format!("write+sync a.b.otio.{pid}.tmp (5 bytes)"),
            format!("rename a.b.otio.{pid}.tmp -> a.b.otio"),
            "sync dir".to_string(),
        ],
        "порядок обязан быть: записать и сбросить ФАЙЛ → переименовать → сбросить КАТАЛОГ"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
    assert!(!kt_otio::atomic_tmp_path(&path).exists(), "временный файл не остаётся");

    // и то же настоящей файловой системой — обёртка не разошлась с трейтом
    let path2 = dir.join("real.otio");
    write_atomic(&path2, "{}").expect("real write");
    assert_eq!(std::fs::read_to_string(&path2).unwrap(), "{}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 353: отказ синка КАТАЛОГА — не отказ записи.
///
/// На exFAT и сетевых шарах каталог так не сбросить, и превращать это в
/// ошибку значило бы врать пользователю, что сохранения не было: файл на
/// месте и цел. Вызывающий узнаёт о деградации из `dir_synced` и говорит
/// об этом один раз за сессию.
#[test]
fn t353_dir_sync_failure_is_not_a_write_failure() {
    let dir = tmp_dir("nodirsync");
    let path = dir.join("p.otio");
    let rec = Recorder { dir_sync_fails: true, ..Default::default() };
    let w = write_atomic_with(&rec, &path, "payload").expect("запись обязана считаться успешной");
    assert!(!w.dir_synced, "но о деградации обязана сказать");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "payload", "файл на месте и цел");
    let _ = std::fs::remove_dir_all(&dir);
}

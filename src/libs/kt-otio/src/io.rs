//! Чтение/запись .otio-документа.
//!
//! Запись детерминирована: `OTIO_SCHEMA` первым ключом, известные поля в
//! порядке объявления структур, passthrough — в порядке исходного документа;
//! отступ 4 пробела (стиль эталонной библиотеки), завершающий перевод строки
//! (домашний стиль). Идемпотентность `write(parse(write(x))) == write(x)`
//! закреплена тестом 144.

use serde::Serialize;
use serde_json::Value;

use crate::schema::{decode_timeline, encode_timeline};
use crate::{Error, Timeline};

pub fn parse_document(text: &str) -> Result<Timeline, Error> {
    let v: Value = serde_json::from_str(text)?;
    decode_timeline(v)
}

pub fn write_document(t: &Timeline) -> String {
    let v = encode_timeline(t);
    let mut out = Vec::with_capacity(16 * 1024);
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, fmt);
    v.serialize(&mut ser).expect("Value serialization is infallible");
    out.push(b'\n');
    String::from_utf8(out).expect("serde_json emits valid UTF-8")
}

// ---- атомарная запись файла проекта (Р.49, п.349) -----------------------
//
// Один дом на всю программу: сюда ходят и GUI (Ctrl+S, фоновое
// восстановление, выход), и kt-cli. Раньше атомарная запись жила в
// kt-app, а kt-cli писал голым `fs::write` — обрыв на середине оставлял
// обрубок прямо на месте проекта.
//
// Порядок операций не косметика: `write + rename` БЕЗ fsync защищает от
// падения процесса, но не от потери питания — переименование может лечь
// в журнал раньше самих данных, и на месте проекта окажется файл нулевой
// длины. Поэтому: записать → **сбросить файл на диск** → переименовать →
// **сбросить каталог**.

use std::io;
use std::path::{Path, PathBuf};

/// Чем кончилась запись. `dir_synced == false` — данные и переименование
/// на диске, но каталог сбросить не удалось (exFAT, сетевая шара,
/// не-unix): при потере питания может потеряться САМО переименование.
/// Это НЕ ошибка записи — файл на месте и цел.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Written {
    pub dir_synced: bool,
}

/// Точки, которыми атомарная запись трогает файловую систему. Вынесены в
/// трейт ради ОДНОГО теста — наблюдения ПОРЯДКА: фолт-инжекцией ядра
/// порядок не проверить (на ext4 его прячет `auto_da_alloc`), а порядок
/// и есть всё, ради чего эта функция написана.
pub trait AtomicFs {
    /// Создать временный файл, записать байты и сбросить ИХ на диск.
    fn write_and_sync(&self, tmp: &Path, bytes: &[u8]) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    /// Сбросить запись каталога (чтобы переименование пережило отключение).
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;
}

/// Настоящая файловая система.
pub struct RealFs;

impl AtomicFs for RealFs {
    fn write_and_sync(&self, tmp: &Path, bytes: &[u8]) -> io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(tmp)?;
        f.write_all(bytes)?;
        // тот же дескриптор, в который писали: данные гарантированно наши
        f.sync_all()
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    #[cfg(unix)]
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        std::fs::File::open(dir)?.sync_all()
    }

    #[cfg(not(unix))]
    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        // на Windows каталог так не открыть — честная деградация, о
        // которой вызывающий сообщает один раз за сессию
        Err(io::Error::new(io::ErrorKind::Unsupported, "directory sync is unix-only"))
    }
}

/// Записать текст в файл атомарно и настолько долговечно, насколько
/// позволяет файловая система. Каталог создаётся при необходимости.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<Written> {
    write_atomic_with(&RealFs, path, text)
}

pub fn write_atomic_with(fs: &dyn AtomicFs, path: &Path, text: &str) -> io::Result<Written> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
    if let Some(dir) = dir {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = atomic_tmp_path(path);
    fs.write_and_sync(&tmp, text.as_bytes())?;
    fs.rename(&tmp, path)?;
    // Отказ синка каталога — НЕ отказ записи: файл уже на месте и цел.
    // На exFAT и сетевых шарах это законный `Unsupported`, и превращать
    // его в ошибку значило бы врать пользователю, что сохранения не было.
    let dir_synced = match dir {
        Some(d) => fs.sync_dir(d).is_ok(),
        None => false,
    };
    Ok(Written { dir_synced })
}

/// Имя временного файла ДОПИСЫВАЕТСЯ к полному имени (тест 150): старый
/// `with_extension("ktproj.tmp")` для «a.b.otio» съедал часть имени.
pub fn atomic_tmp_path(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!("{name}.tmp"))
}

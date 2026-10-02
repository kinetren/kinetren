//! Замок на ПРОЕКТ (Р.49, п.352).
//!
//! Задача — не запретить вторую копию программы, а не дать двум копиям молча
//! писать в один файл проекта, затирая работу друг друга.
//!
//! **Почему не единственный экземпляр программы.** Глобальный замок убил бы
//! живые сценарии (`scripts/live-check.sh` запускает kt-app в Xvfb
//! параллельно) и законный сценарий «открыл второй раз, чтобы посмотреть
//! другой кусок монтажа». Замок именной: на конкретный проект.
//!
//! **Почему `flock`, а не файл с pid и `O_EXCL`.** Ядро снимает `flock` при
//! ЛЮБОЙ смерти процесса — падении, SIGKILL, `terminate()` из живого
//! каркаса. Замок, который ядро само не снимает, после первого же
//! принудительного гашения оставляет вечную блокировку: класс, уже стоивший
//! раунда (п.232) — семь зелёных сценариев стали бы красными.
//!
//! **Почему замок на СОСЕДНЕМ файле, а не на самом проекте.** Сохранение
//! атомарно: `rename` подменяет inode. Замок, взятый на файл проекта, после
//! первого же Ctrl+S остался бы висеть на осиротевшем inode, а следующая
//! копия открыла бы НОВЫЙ и спокойно взяла свой — защита исчезала ровно в
//! тот момент, ради которого была нужна.
//!
//! **Почему файл замка лежит в каталоге данных, а не рядом с проектом.**
//! Каталог пользователя не засоряется, замок берётся даже на проекте в
//! read-only каталоге (на сетевой шаре, на смонтированном образе), а
//! брошенные нулевые файлы остаются в нашем каталоге, где никому не мешают.
//! Имя — хэш абсолютного пути; сам путь и pid пишутся ВНУТРЬ файла, чтобы
//! сообщение могло назвать чужой процесс.

use std::path::{Path, PathBuf};

/// Чем кончилась попытка занять проект.
pub enum LockState {
    /// Проект наш: пишем в него свободно.
    Owned(ProjectLock),
    /// Занят другой копией. `pid` — её номер процесса, если файл замка
    /// удалось прочитать (нет — работаем без номера, это не повод молчать).
    Busy { pid: Option<u32> },
    /// Замок взять не удалось совсем (нет каталога данных, нет прав).
    /// Не повод не работать: ведём себя как владелец, но без гарантии.
    Unavailable,
}

pub struct ProjectLock {
    _file: std::fs::File,
}

impl Drop for ProjectLock {
    fn drop(&mut self) {
        // Файл НЕ удаляем: между нашим `unlink` и `open` соседа успел бы
        // втиснуться третий, и два процесса держали бы замки на разных
        // inode одного имени. Нулевой файл в своём каталоге данных — цена
        // корректности, и ядро освободит сам замок в любом случае.
    }
}

/// Каталог для файлов замков.
fn locks_dir() -> PathBuf {
    crate::journal::data_dir().join("locks")
}

/// Имя файла замка: хэш абсолютного пути проекта. Абсолютный, а не
/// канонический: `canonicalize` требует существующего файла, а замок нужен
/// и на «Сохранить как…» в ещё не созданный.
pub fn lock_file_for(project: &Path) -> PathBuf {
    let abs = std::path::absolute(project).unwrap_or_else(|_| project.to_path_buf());
    locks_dir().join(format!("{:016x}.lock", fnv1a(abs.to_string_lossy().as_bytes())))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Попытаться занять проект. Не блокирует ни на миллисекунду: занято —
/// значит занято, и программа работает дальше, только честно об этом
/// говорит.
pub fn acquire(project: &Path) -> LockState {
    let path = lock_file_for(project);
    if let Some(d) = path.parent()
        && std::fs::create_dir_all(d).is_err()
    {
        return LockState::Unavailable;
    }
    // truncate(false): файл замка переживает смену держателя, и обнулять
    // чужую запись до взятия замка нельзя
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
    {
        Ok(f) => f,
        Err(_) => return LockState::Unavailable,
    };
    if try_lock(&file) {
        // Содержимое — для сообщения соседу и для разбора руками. Пишем
        // ПОСЛЕ взятия замка: иначе затирали бы чужую запись.
        let _ = std::io::Write::write_all(
            &mut (&file),
            format!("{}\n{}\n", std::process::id(), project.display()).as_bytes(),
        );
        LockState::Owned(ProjectLock { _file: file })
    } else {
        let pid = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.lines().next().and_then(|l| l.trim().parse().ok()));
        LockState::Busy { pid }
    }
}

#[cfg(unix)]
fn try_lock(file: &std::fs::File) -> bool {
    use std::os::fd::AsRawFd;
    // SAFETY: передаётся живой дескриптор открытого файла; flock не
    // трогает память и возвращает только код ошибки.
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

#[cfg(not(unix))]
fn try_lock(_file: &std::fs::File) -> bool {
    // Без flock честнее считать проект свободным, чем изображать защиту.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t362: замок именной и живучий. Вторая попытка занять ТОТ ЖЕ проект
    /// отказывает и называет процесс; другой проект берётся свободно;
    /// атомарная перезапись самого проекта замок НЕ ломает (потому он и
    /// взят на соседнем файле, а не на inode проекта).
    #[test]
    fn t362_project_lock_is_per_project_and_survives_atomic_save() {
        let dir = std::env::temp_dir().join(format!("kt-t362-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.kt");
        let b = dir.join("b.kt");
        std::fs::write(&a, "{}").unwrap();

        let first = match acquire(&a) {
            LockState::Owned(l) => l,
            _ => panic!("свободный проект обязан заниматься"),
        };
        // тот же процесс, тот же проект — flock у нас рекурсивен по
        // дескриптору, поэтому берём НОВЫЙ дескриптор, как это сделала бы
        // вторая копия программы
        match acquire(&a) {
            LockState::Busy { pid } => {
                assert_eq!(pid, Some(std::process::id()), "замок не назвал держателя")
            }
            _ => panic!("занятый проект отдался второй копии"),
        }
        // соседний проект не задет
        assert!(matches!(acquire(&b), LockState::Owned(_)), "замок оказался глобальным");

        // сохранение проекта атомарно подменяет inode — замок обязан устоять
        kt_otio::write_atomic(&a, "{\"x\":1}").unwrap();
        assert!(matches!(acquire(&a), LockState::Busy { .. }), "Ctrl+S сбросил замок");

        drop(first);
        assert!(matches!(acquire(&a), LockState::Owned(_)), "замок не отпущен");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Разные пути — разные файлы замков, один путь — один файл (в том
    /// числе записанный по-разному).
    #[test]
    fn t362b_lock_file_is_keyed_by_path() {
        let a = lock_file_for(Path::new("/w/film.kt"));
        let b = lock_file_for(Path::new("/w/film.mp4.kt"));
        assert_ne!(a, b);
        assert_eq!(a, lock_file_for(Path::new("/w/./film.kt")));
        assert!(a.to_string_lossy().ends_with(".lock"));
    }
}

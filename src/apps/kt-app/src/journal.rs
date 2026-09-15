//! Журнал программы — единственный канал, когда показать некому.
//!
//! Зачем он вообще (Р.49, п.350). Всё, что программа хочет сказать, идёт в
//! колокольчик. Но у `on_exit` окна УЖЕ НЕТ: егуи закрыт, кадров больше не
//! будет, а `.desktop` у нас с `Terminal=false` — вывод в stderr уходит в
//! никуда (в лучшем случае в journald, куда монтажёр не пойдёт). Именно там
//! живёт финальная запись проекта: отказ на ней — худший из возможных, и до
//! Р.49 он не оставлял ни следа. То же у панического хука (п.355): показать
//! отчёт некому, окно уже мертво.
//!
//! Поэтому журнал НЕ дублирует колокольчик: в него пишется то, что
//! пользователь иначе не увидит никогда, плюс предупреждения — чтобы у
//! жалобы «оно что-то сказало и пропало» был адрес.
//!
//! Формат — строка на запись: `ГГГГ-ММ-ДД ЧЧ:ММ:СС уровень текст`. Ротация
//! по 1 МиБ в `kt-app.log.1` (один шаг, без веера номеров): журнал нужен для
//! разбора последней сессии, а не для истории.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Порог ротации. Мегабайт — это примерно десять тысяч строк: больше, чем
/// программа наговорит за сутки работы, и меньше, чем заметно на диске.
pub const ROTATE_BYTES: u64 = 1024 * 1024;

/// Каталог данных программы (`$XDG_DATA_HOME/kinotrain`). Общий с
/// автосейвом — отдельный дом журналу заводить незачем.
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("kinotrain")
}

pub fn path() -> PathBuf {
    data_dir().join("kt-app.log")
}

/// Дописать строку. Ошибки записи журнала ГЛОТАЮТСЯ намеренно: журнал —
/// вспомогательный канал, и падать/шуметь из-за него, когда пользователь
/// как раз теряет проект, было бы издевательством.
pub fn log(level: &str, text: &str) {
    write_to(&path(), level, text);
}

pub(crate) fn write_to(path: &Path, level: &str, text: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    rotate(path);
    let line = format!("{} {:<5} {}\n", stamp(), level, text.replace('\n', " | "));
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

fn rotate(path: &Path) {
    let big = std::fs::metadata(path).map(|m| m.len() >= ROTATE_BYTES).unwrap_or(false);
    if big {
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
}

/// Дата и время без внешних зависимостей: программа не тянет chrono ради
/// одной строки. Алгоритм — civil_from_days Говарда Хиннанта.
fn stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kt-jr-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("kt-app.log")
    }

    /// t354: журнал дописывает, а не перетирает, и ротируется по порогу,
    /// не теряя старое. Отказ записи журнала не должен быть заметен.
    #[test]
    fn t354_journal_appends_and_rotates() {
        let p = tmp("rot");
        write_to(&p, "WARN", "first line");
        write_to(&p, "INFO", "second line");
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("first line") && body.contains("second line"), "{body}");
        assert_eq!(body.lines().count(), 2, "дописывание перетёрло журнал");
        assert!(body.starts_with("20"), "нет даты в начале строки: {body}");

        // перевалили порог — прошлое уезжает в .log.1, новое начинает с нуля
        std::fs::write(&p, "x".repeat(ROTATE_BYTES as usize + 1)).unwrap();
        write_to(&p, "WARN", "after rotate");
        let fresh = std::fs::read_to_string(&p).unwrap();
        assert_eq!(fresh.lines().count(), 1, "ротация не обнулила журнал");
        assert!(fresh.contains("after rotate"));
        let old = std::fs::read_to_string(p.with_extension("log.1")).unwrap();
        assert_eq!(old.len(), ROTATE_BYTES as usize + 1, "старое потеряно при ротации");

        // недоступный путь — молча мимо: журнал не смеет ронять программу
        write_to(Path::new("/proc/self/no-such-dir/kt.log"), "WARN", "ignored");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    /// Дата собирается своими руками — сторож на известные значения.
    #[test]
    fn t354b_civil_from_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1)); // високосный
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(20_713), (2026, 9, 17));
    }
}

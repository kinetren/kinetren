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

/// Name of the per-user data directory under `$XDG_DATA_HOME`.
pub const DATA_DIR_NAME: &str = "kinetren";
/// The directory of the program's former name (before 2026-10-21). It is
/// moved into place once (`data_dir_in`); nothing reads it afterwards.
pub const FORMER_DATA_DIR_NAME: &str = "kinotrain";

/// Каталог данных программы (`$XDG_DATA_HOME/kinetren`). Общий с
/// автосейвом — отдельный дом журналу заводить незачем. The single resolver:
/// recovery, journal, locks, presets, workspace and eframe's own storage
/// (its app id is the same name) all live here.
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    data_dir_in(&base)
}

/// `base/kinetren`, after moving the former `base/kinotrain` into place if
/// only that one exists (kinetren-rename). A rename is atomic on one
/// filesystem; if it fails, the content is copied into a staging directory
/// that is then renamed, and the former directory is left as it was. When
/// both exist the new one wins and nothing is merged: a merge could bring
/// back deleted presets or stale locks.
pub fn data_dir_in(base: &Path) -> PathBuf {
    let new = base.join(DATA_DIR_NAME);
    let former = base.join(FORMER_DATA_DIR_NAME);
    if !new.exists() && former.is_dir() && std::fs::rename(&former, &new).is_err() {
        let staging = base.join(format!("{DATA_DIR_NAME}.moving"));
        let _ = std::fs::remove_dir_all(&staging);
        if copy_tree(&former, &staging).is_ok() {
            let _ = std::fs::rename(&staging, &new);
        } else {
            let _ = std::fs::remove_dir_all(&staging);
        }
    }
    new
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dst = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &dst)?;
        } else {
            std::fs::copy(e.path(), dst)?;
        }
    }
    Ok(())
}

pub fn path() -> PathBuf {
    data_dir().join("kt-app.log")
}

/// Отчёт о последнем падении. Один файл: он перезаписывается каждым новым
/// падением, а прочитанный уезжает в `.seen` — так предупреждение появляется
/// ровно один раз на аварию, и при этом последний отчёт всегда на месте.
pub fn crash_path() -> PathBuf {
    data_dir().join("kt-app-crash.txt")
}

pub fn crash_seen_path() -> PathBuf {
    data_dir().join("kt-app-crash.seen.txt")
}

/// «Чёрный ящик» для панического хука. Хранится ОТДЕЛЬНО от приложения:
/// хук не имеет права трогать модель (инвариант 2 — состояние FX принадлежит
/// рендер-потоку; инвариант 6 — в колбэке ни локов, ни аллокаций), а
/// кольцо событий самодостаточно и читается из любого потока.
static FLIGHT: std::sync::OnceLock<
    std::sync::Mutex<Option<std::sync::Arc<kt_engine::flight::FlightRecorder>>>,
> = std::sync::OnceLock::new();

fn flight_slot()
-> &'static std::sync::Mutex<Option<std::sync::Arc<kt_engine::flight::FlightRecorder>>> {
    FLIGHT.get_or_init(|| std::sync::Mutex::new(None))
}

pub fn set_flight(f: std::sync::Arc<kt_engine::flight::FlightRecorder>) {
    if let Ok(mut slot) = flight_slot().lock() {
        *slot = Some(f);
    }
}

/// Поставить панический хук. Разделение труда с `Drop for KtApp` (п.354)
/// прямое: **хук — диагностика, `Drop` — данные**. Хук не сохраняет проект
/// (лезть в модель из произвольного паникующего потока нельзя) и не мешает
/// раскрутке: прежний хук зовётся следом, `Drop` доедет своим ходом.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let out = crash_report(&info.to_string());
        let p = crash_path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(&p, &out);
        log("CRASH", &format!("panic report written to {}", p.display()));
        prev(info);
    }));
}

/// Тело отчёта. Вынесено из хука, чтобы его можно было проверить, не
/// подменяя глобальный хук в тесте (подмена задела бы соседние тесты).
pub(crate) fn crash_report(panic_text: &str) -> String {
    let dump = flight_slot().lock().ok().and_then(|s| s.as_ref().map(|f| f.dump()));
    crash_report_with(panic_text, dump.as_deref())
}

pub(crate) fn crash_report_with(panic_text: &str, flight: Option<&str>) -> String {
    let mut out = String::with_capacity(8 * 1024);
    out.push_str(&format!("kinetren {}\n", kt_version::display()));
    out.push_str(&format!("time: {}\n", stamp()));
    let t = std::thread::current();
    out.push_str(&format!("thread: {}\n", t.name().unwrap_or("<unnamed>")));
    out.push_str(&format!("panic: {panic_text}\n\n"));
    out.push_str(&format!("{}\n", std::backtrace::Backtrace::force_capture()));
    // дамп «чёрного ящика»: что программа делала последние секунды
    if let Some(f) = flight {
        out.push_str("\n--- flight recorder ---\n");
        out.push_str(f);
    }
    out
}

/// Если прошлая сессия оставила отчёт — вернуть его путь ОДИН раз и
/// отложить сам отчёт в сторону.
pub fn take_crash_report() -> Option<PathBuf> {
    let p = crash_path();
    if !p.exists() {
        return None;
    }
    let seen = crash_seen_path();
    match std::fs::rename(&p, &seen) {
        Ok(()) => Some(seen),
        // переименовать не вышло — сказать всё равно надо, отчёт на месте
        Err(_) => Some(p),
    }
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
    stamp_of(std::time::SystemTime::now())
}

/// Та же метка для произвольного момента — ею диалог восстановления
/// показывает время каждого кандидата (п.353).
pub fn stamp_of(t: std::time::SystemTime) -> String {
    let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
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

    /// t375: отчёт о падении называет версию, поток и место, несёт бэктрейс
    /// и дамп «чёрного ящика» — иначе жалоба «программа закрылась» остаётся
    /// без единой зацепки. Симметричный сторож на отравление кольца — t373
    /// в kt-engine: хук читает ящик ровно тогда, когда кто-то уже паниковал.
    #[test]
    fn t375_crash_report_names_version_thread_and_flight() {
        use kt_engine::flight::{Ev, FlightRecorder};
        let f = FlightRecorder::new();
        f.push(Ev::Play, 48_000, 0);
        // дамп передаётся явно: слот «чёрного ящика» глобальный, а тесты
        // идут параллельно — соседний харнесс успевает положить туда свой
        let r =
            crash_report_with("assertion failed: у пользователя всё сломалось", Some(&f.dump()));
        assert!(r.contains(kt_version::display()), "нет версии: жалоба без неё бесполезна");
        assert!(r.contains("thread:"), "нет потока");
        assert!(r.contains("assertion failed"), "нет самого сообщения паники");
        assert!(r.contains("crash_report_with"), "в бэктрейсе нет имён — отчёт из голых адресов");
        assert!(r.contains("--- flight recorder ---"), "нет дампа чёрного ящика");
        assert!(r.to_lowercase().contains("play"), "чёрный ящик пуст: {r}");
    }

    /// t374: отчёт прошлой сессии отдаётся ровно ОДИН раз и не теряется.
    #[test]
    fn t374_crash_report_is_reported_once() {
        let dir = std::env::temp_dir().join(format!("kt-t374-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let crash = dir.join("kt-app-crash.txt");
        let seen = dir.join("kt-app-crash.seen.txt");
        std::fs::write(&crash, "report").unwrap();

        // та же логика, что в `take_crash_report`, на явных путях: каталог
        // данных один на процесс, а тесты идут параллельно
        let take = |c: &Path, s: &Path| -> Option<PathBuf> {
            if !c.exists() {
                return None;
            }
            match std::fs::rename(c, s) {
                Ok(()) => Some(s.to_path_buf()),
                Err(_) => Some(c.to_path_buf()),
            }
        };
        assert_eq!(take(&crash, &seen), Some(seen.clone()), "об аварии не сказано");
        assert_eq!(std::fs::read_to_string(&seen).unwrap(), "report", "отчёт потерян");
        assert_eq!(take(&crash, &seen), None, "об одной аварии сказано дважды");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Дата собирается своими руками — сторож на известные значения.
    #[test]
    fn t354b_civil_from_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1)); // високосный
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(20_713), (2026, 9, 17));
    }

    /// t830 (kinetren-rename): the first resolve moves the former directory
    /// with everything in it; the next resolve changes nothing; with both
    /// present the new one wins and the former one is not touched.
    #[test]
    fn t830_former_data_dir_is_moved_once() {
        let base = std::env::temp_dir().join(format!("kt-t830-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let former = base.join(FORMER_DATA_DIR_NAME);
        std::fs::create_dir_all(former.join("locks")).unwrap();
        std::fs::write(former.join("fx-presets.json"), "[1]").unwrap();
        std::fs::write(former.join("locks/0123.lock"), "4242\n").unwrap();

        let dir = data_dir_in(&base);
        assert_eq!(dir, base.join(DATA_DIR_NAME));
        assert_eq!(std::fs::read_to_string(dir.join("fx-presets.json")).unwrap(), "[1]");
        assert_eq!(std::fs::read_to_string(dir.join("locks/0123.lock")).unwrap(), "4242\n");
        assert!(!former.exists(), "the former directory is moved, not duplicated");

        // a second resolve changes nothing
        std::fs::write(dir.join("workspace.json"), "{}").unwrap();
        assert_eq!(data_dir_in(&base), dir);
        assert!(dir.join("workspace.json").exists() && !former.exists());

        // both present: the new one wins, the former one is untouched
        std::fs::create_dir_all(&former).unwrap();
        std::fs::write(former.join("fx-presets.json"), "[2]").unwrap();
        assert_eq!(data_dir_in(&base), dir);
        assert_eq!(std::fs::read_to_string(dir.join("fx-presets.json")).unwrap(), "[1]");
        assert_eq!(std::fs::read_to_string(former.join("fx-presets.json")).unwrap(), "[2]");
        let _ = std::fs::remove_dir_all(&base);
    }
}

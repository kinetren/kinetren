//! Версия сборки (п.6 р.12, префикс — п.3 р.13): `ktYY.MM.DD-<hash8>` по
//! дате КОММИТА. Префикс везде одинаковый: короткая ktYY.MM — это просто
//! обрезанная полная, и пользователю не надо держать в голове два формата.
//!
//! Версия в манифесте остаётся 0.1.0 — semver не примет «v26.08.27-…».
//! Отображаемая версия считается здесь, тремя способами по приоритету:
//!   1. переменные окружения KT_VERSION_FULL/KT_VERSION_SHORT — так их
//!      прокидывает release-build.sh: внутри контейнера сборки нет ни
//!      git, ни каталога .git (монтируется только src/);
//!   2. git в рабочем дереве;
//!   3. фолбэк «v00.00.00-nogit» — чтобы сборка не падала нигде.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn main() {
    // пересборка при смене коммита и при подмене версии снаружи
    println!("cargo:rerun-if-env-changed=KT_VERSION_FULL");
    println!("cargo:rerun-if-env-changed=KT_VERSION_SHORT");
    for p in ["../../../.git/HEAD", "../../../.git/refs/heads"] {
        if std::path::Path::new(p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }

    let full = std::env::var("KT_VERSION_FULL").ok().or_else(|| {
        let date = git(&["log", "-1", "--format=%cd", "--date=format:%y.%m.%d"])?;
        let hash = git(&["rev-parse", "--short=8", "HEAD"])?;
        Some(format!("kt{date}-{hash}"))
    });
    let full = full.unwrap_or_else(|| "kt00.00.00-nogit".to_string());
    // короткая: ktYY.MM из тех же цифр
    let short = std::env::var("KT_VERSION_SHORT").ok().unwrap_or_else(|| {
        let digits: String = full.trim_start_matches("kt").chars().take(5).collect();
        format!("kt{digits}")
    });

    println!("cargo:rustc-env=KT_VERSION_FULL={full}");
    println!("cargo:rustc-env=KT_VERSION_SHORT={short}");
}

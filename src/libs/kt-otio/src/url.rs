//! Пути медиа в target_url (решение Р.24 п.7): пишем ОТНОСИТЕЛЬНО файла
//! проекта (POSIX-слэши), фолбэк — абсолютный file://-URL с percent-encoding
//! (так пишут Resolve/Hiero). Читаем все три вида: относительный путь,
//! абсолютный путь, file://-URL. `synth:<pattern>` — verbatim (тестовые
//! источники). Никакого url-крейта — руками, минимум.

use std::path::{Component, Path, PathBuf};

fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/')
}

/// Абсолютный путь → file://-URL (percent-encoding всех байт вне unreserved).
pub fn encode_file_url(path: &Path) -> String {
    let mut out = String::from("file://");
    for &b in path.to_string_lossy().as_bytes() {
        if is_unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// file://-URL → путь (None, если это не file-URL).
pub fn decode_file_url(url: &str) -> Option<PathBuf> {
    let rest = url
        .strip_prefix("file://localhost/")
        .map(|r| format!("/{r}"))
        .or_else(|| url.strip_prefix("file://").map(str::to_owned))?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

/// Чисто лексическая релятивизация `target` от каталога `base_dir`
/// (оба абсолютные). None — если относительный путь не строится.
pub fn relativize(base_dir: &Path, target: &Path) -> Option<String> {
    if !base_dir.is_absolute() || !target.is_absolute() {
        return None;
    }
    let base: Vec<Component> = base_dir.components().collect();
    let tgt: Vec<Component> = target.components().collect();
    let common = base.iter().zip(tgt.iter()).take_while(|(a, b)| a == b).count();
    if common <= 1 {
        // общего только корень "/" — путь через весь диск не пишем,
        // это случай для file://-фолбэка
        return None;
    }
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), base.len() - common)
        .chain(tgt[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()))
        .collect();
    if parts.is_empty() {
        parts.push(".".into());
    }
    Some(parts.join("/"))
}

/// target_url из файла → путь для открытия. `base_dir` — каталог проекта.
/// `synth:` возвращается как есть (нефайловый источник).
pub fn resolve_media_path(base_dir: &Path, target_url: &str) -> PathBuf {
    if target_url.starts_with("synth:") {
        return PathBuf::from(target_url);
    }
    if let Some(p) = decode_file_url(target_url) {
        return p;
    }
    let p = Path::new(target_url);
    if p.is_absolute() { p.to_path_buf() } else { base_dir.join(p) }
}

/// Путь медиа → target_url для записи (решение Р.24 п.7).
pub fn media_target_url(base_dir: &Path, media_path: &str) -> String {
    if media_path.starts_with("synth:") {
        return media_path.to_string();
    }
    let p = Path::new(media_path);
    if !p.is_absolute() {
        // уже относительный — нормализуем разделители и оставляем
        return media_path.replace('\\', "/");
    }
    match relativize(base_dir, p) {
        Some(rel) => rel,
        None => encode_file_url(p),
    }
}

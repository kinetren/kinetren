//! Media paths in target_url (decision of R.24 item 7): we write RELATIVE to the project
//! file (POSIX slashes), fallback — an absolute file://-URL with percent-encoding
//! (as Resolve/Hiero write it). We read all three kinds: a relative path,
//! an absolute path, a file://-URL. `synth:<pattern>` — verbatim (test
//! sources). No url crate — by hand, minimally.

use std::path::{Component, Path, PathBuf};

fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/')
}

/// Absolute path → file://-URL (percent-encoding of all bytes outside the unreserved set).
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

/// file://-URL → path (None if this is not a file-URL).
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

/// A purely lexical relativization of `target` from the directory `base_dir`
/// (both absolute). None — if the relative path cannot be built.
pub fn relativize(base_dir: &Path, target: &Path) -> Option<String> {
    if !base_dir.is_absolute() || !target.is_absolute() {
        return None;
    }
    let base: Vec<Component> = base_dir.components().collect();
    let tgt: Vec<Component> = target.components().collect();
    let common = base.iter().zip(tgt.iter()).take_while(|(a, b)| a == b).count();
    if common <= 1 {
        // only the root “/” in common — we do not write a path across the whole disk,
        // this is a case for the file://-fallback
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

/// target_url from the file → a path to open. `base_dir` — the project directory.
/// Pseudo-paths (`synth:`, `gen:`) are returned as-is (non-file
/// sources).
pub fn resolve_media_path(base_dir: &Path, target_url: &str) -> PathBuf {
    if kt_model::is_pseudo_path(target_url) {
        return PathBuf::from(target_url);
    }
    if let Some(p) = decode_file_url(target_url) {
        return p;
    }
    let p = Path::new(target_url);
    if p.is_absolute() { p.to_path_buf() } else { base_dir.join(p) }
}

/// Media path → target_url for writing (decision of R.24 item 7).
pub fn media_target_url(base_dir: &Path, media_path: &str) -> String {
    if kt_model::is_pseudo_path(media_path) {
        return media_path.to_string();
    }
    let p = Path::new(media_path);
    if !p.is_absolute() {
        // already relative — we normalize the separators and leave it
        return media_path.replace('\\', "/");
    }
    match relativize(base_dir, p) {
        Some(rel) => rel,
        None => encode_file_url(p),
    }
}

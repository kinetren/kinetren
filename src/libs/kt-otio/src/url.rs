//! Media paths in target_url (decision of R.24 item 7): we write RELATIVE to the project
//! file (POSIX slashes), fallback — an absolute file://-URL with percent-encoding
//! (as Resolve/Hiero write it). We read all three kinds: a relative path,
//! an absolute path, a file://-URL. `synth:<pattern>` — verbatim (test
//! sources). No url crate — by hand, minimally.
//!
//! The rules depend on the OS (windows-build, design D7): a drive letter or a UNC share
//! on Windows, case-insensitive components there, the verbatim prefix `\\?\`. They work on
//! STRINGS with an explicit [`PathFlavor`], so the Windows rules are tested on Linux and
//! the other way round (t854–t856); the `Path` functions are the native wrappers.
//!
//! "Absolute" on READING is the union of both flavors: a path absolute on either OS is
//! never joined to the project directory. On its own OS it opens; on the other one it
//! stays an offline placeholder and is written back exactly as it was.

use std::path::{Path, PathBuf};

/// Whose path rules apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathFlavor {
    Posix,
    Windows,
}

impl PathFlavor {
    pub const NATIVE: PathFlavor = if cfg!(windows) { PathFlavor::Windows } else { PathFlavor::Posix };
}

fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/')
}

fn pct_encode(out: &mut String, s: &str) {
    for &b in s.as_bytes() {
        if is_unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
}

fn pct_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
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
    String::from_utf8(out).ok()
}

/// `C:` at the start (a drive letter and a colon).
fn has_drive(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// Remove the verbatim prefix: `\\?\C:\x` → `C:\x`, `\\?\UNC\h\s\x` → `\\h\s\x`.
fn strip_verbatim(s: &str) -> std::borrow::Cow<'_, str> {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}").into()
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.into()
    } else {
        s.into()
    }
}

/// Is `s` absolute by the rules of `f`?
pub fn is_absolute_in(s: &str, f: PathFlavor) -> bool {
    match f {
        PathFlavor::Posix => s.starts_with('/'),
        PathFlavor::Windows => {
            let b = s.as_bytes();
            (has_drive(s) && b.len() >= 3 && matches!(b[2], b'\\' | b'/')) || s.starts_with(r"\\")
        }
    }
}

/// Absolute on either OS — never to be joined to the project directory.
pub fn is_absolute_anywhere(s: &str) -> bool {
    is_absolute_in(s, PathFlavor::Posix) || is_absolute_in(s, PathFlavor::Windows)
}

/// Absolute path (of flavor `f`) → file://-URL.
/// Posix `/a b` → `file:///a%20b`; Windows `C:\a b` → `file:///C:/a%20b`,
/// `\\server\share\x` → `file://server/share/x`.
pub fn encode_file_url_in(path: &str, f: PathFlavor) -> String {
    let mut out = String::from("file://");
    match f {
        PathFlavor::Posix => pct_encode(&mut out, path),
        PathFlavor::Windows => {
            let p = strip_verbatim(path).replace('\\', "/");
            if let Some(unc) = p.strip_prefix("//") {
                pct_encode(&mut out, unc);
            } else if has_drive(&p) {
                out.push('/');
                out.push_str(&p[..2]);
                pct_encode(&mut out, &p[2..]);
            } else {
                pct_encode(&mut out, &p);
            }
        }
    }
    out
}

/// file://-URL → path (None if this is not a file-URL). A drive letter or a host gives a
/// Windows path (`C:\x`, `\\host\share\x`) whoever reads it; a plain `/path` stays a Posix
/// path even for the Windows reader (a URL written on Linux).
pub fn decode_file_url_in(url: &str, f: PathFlavor) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost/").map(|r| format!("/{r}")).unwrap_or_else(|| rest.to_string());
    let p = pct_decode(&rest)?;
    if let Some(after) = p.strip_prefix('/') {
        if has_drive(after) && (after.len() == 2 || after.as_bytes()[2] == b'/') {
            return Some(after.replace('/', "\\"));
        }
        let _ = f;
        return Some(p);
    }
    // `file://host/share/x` — a UNC path
    Some(format!(r"\\{}", p.replace('/', "\\")))
}

/// Components of an absolute path of flavor `f`: the root as one component (`""` for
/// Posix `/`, `C:` for a drive, `\\host\share` for UNC), then the names.
fn components(s: &str, f: PathFlavor) -> Vec<String> {
    match f {
        PathFlavor::Posix => {
            let mut v = vec![String::new()];
            v.extend(s.split('/').filter(|c| !c.is_empty() && *c != ".").map(str::to_string));
            v
        }
        PathFlavor::Windows => {
            let p = strip_verbatim(s).replace('/', "\\");
            let (root, rest) = if let Some(unc) = p.strip_prefix(r"\\") {
                let mut it = unc.splitn(3, '\\');
                let host = it.next().unwrap_or("");
                let share = it.next().unwrap_or("");
                (format!(r"\\{host}\{share}"), it.next().unwrap_or("").to_string())
            } else {
                (p[..2].to_string(), p[2..].to_string())
            };
            let mut v = vec![root];
            v.extend(rest.split('\\').filter(|c| !c.is_empty() && *c != ".").map(str::to_string));
            v
        }
    }
}

/// A purely lexical relativization of `target` from the directory `base_dir` (both
/// absolute in flavor `f`). None — if the relative path cannot be built, including the
/// case where only the root (Windows: the drive or the share) is shared: a path across the
/// whole disk is the case for the file://-fallback.
pub fn relativize_in(base_dir: &str, target: &str, f: PathFlavor) -> Option<String> {
    if !is_absolute_in(base_dir, f) || !is_absolute_in(target, f) {
        return None;
    }
    let base = components(base_dir, f);
    let tgt = components(target, f);
    let same = |a: &String, b: &String| match f {
        PathFlavor::Posix => a == b,
        PathFlavor::Windows => a.to_lowercase() == b.to_lowercase(),
    };
    let common = base.iter().zip(tgt.iter()).take_while(|(a, b)| same(a, b)).count();
    if common <= 1 {
        return None;
    }
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), base.len() - common)
        .chain(tgt[common..].iter().cloned())
        .collect();
    if parts.is_empty() {
        parts.push(".".into());
    }
    Some(parts.join("/"))
}

/// target_url from the file → a path to open (flavor `f` is the reader's).
pub fn resolve_media_path_in(base_dir: &str, target_url: &str, f: PathFlavor) -> String {
    if kt_model::is_pseudo_path(target_url) {
        return target_url.to_string();
    }
    if let Some(p) = decode_file_url_in(target_url, f) {
        return p;
    }
    if is_absolute_anywhere(target_url) {
        // absolute here, or absolute on the OTHER OS: never joined to the project
        return target_url.to_string();
    }
    match f {
        PathFlavor::Posix => format!("{}/{}", base_dir.trim_end_matches('/'), target_url),
        PathFlavor::Windows => {
            format!("{}\\{}", base_dir.trim_end_matches(['\\', '/']), target_url.replace('/', "\\"))
        }
    }
}

/// Media path → target_url for writing (flavor `f` is the writer's).
pub fn media_target_url_in(base_dir: &str, media_path: &str, f: PathFlavor) -> String {
    if kt_model::is_pseudo_path(media_path) {
        return media_path.to_string();
    }
    if !is_absolute_in(media_path, f) {
        if is_absolute_anywhere(media_path) {
            // a path of the other OS: written back exactly as it was read
            return media_path.to_string();
        }
        // already relative — we normalize the separators and leave it
        return media_path.replace('\\', "/");
    }
    match relativize_in(base_dir, media_path, f) {
        Some(rel) => rel,
        None => encode_file_url_in(media_path, f),
    }
}

// ---- native wrappers --------------------------------------------------------------

/// Absolute path → file://-URL (native rules).
pub fn encode_file_url(path: &Path) -> String {
    encode_file_url_in(&path.to_string_lossy(), PathFlavor::NATIVE)
}

/// file://-URL → path (None if this is not a file-URL).
pub fn decode_file_url(url: &str) -> Option<PathBuf> {
    decode_file_url_in(url, PathFlavor::NATIVE).map(PathBuf::from)
}

/// Relativization with the native rules.
pub fn relativize(base_dir: &Path, target: &Path) -> Option<String> {
    relativize_in(&base_dir.to_string_lossy(), &target.to_string_lossy(), PathFlavor::NATIVE)
}

/// target_url from the file → a path to open. `base_dir` — the project directory.
/// Pseudo-paths (`synth:`, `gen:`) are returned as-is (non-file sources).
pub fn resolve_media_path(base_dir: &Path, target_url: &str) -> PathBuf {
    PathBuf::from(resolve_media_path_in(&base_dir.to_string_lossy(), target_url, PathFlavor::NATIVE))
}

/// Media path → target_url for writing (decision of R.24 item 7).
pub fn media_target_url(base_dir: &Path, media_path: &str) -> String {
    media_target_url_in(&base_dir.to_string_lossy(), media_path, PathFlavor::NATIVE)
}

//! Parsing of translation catalogs `i18n/*.toml`.
//!
//! Format — a STRICT subset of TOML: a comment `#`, a section header
//! `[a.b]`, a pair `key = "value"`. This is enough for a catalog, and in return the file stays
//! valid TOML (translators get editors and `taplo` to work), and
//! the parser does not pull in either `toml` or `serde` as build dependencies — the same
//! trick as in `libs/kt-version/build.rs`.
//!
//! The module is included TWICE: as a module of the crate and via `#[path]` in
//! `build.rs`. Therefore it must not depend on anything but std.

/// One catalog entry: the full key (`section.path.key`), the value, and the line
/// number in the file — the number is needed so that an error points to the place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: String,
    pub line: usize,
}

fn bare_key_ok(s: &str) -> bool {
    !s.is_empty()
        && s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Parse the contents of a catalog. An error is a human-readable string with the line
/// number; the caller appends the file name to it.
pub fn parse(src: &str) -> Result<Vec<Entry>, String> {
    let mut out: Vec<Entry> = Vec::new();
    let mut section = String::new();

    for (i, raw) in src.lines().enumerate() {
        let line = i + 1;
        let s = raw.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }

        if let Some(rest) = s.strip_prefix('[') {
            let close = rest
                .find(']')
                .ok_or_else(|| format!("line {line}: section header is not closed with ']'"))?;
            let name = rest[..close].trim();
            let tail = rest[close + 1..].trim();
            if !tail.is_empty() && !tail.starts_with('#') {
                return Err(format!("line {line}: trailing text after section header"));
            }
            if name.is_empty() {
                return Err(format!("line {line}: empty section name"));
            }
            for seg in name.split('.') {
                if !bare_key_ok(seg.trim()) {
                    return Err(format!(
                        "line {line}: bad section segment {seg:?} (want [a-z][a-z0-9_]*)"
                    ));
                }
            }
            section = name.split('.').map(str::trim).collect::<Vec<_>>().join(".");
            continue;
        }

        let eq = s.find('=').ok_or_else(|| format!("line {line}: expected `key = \"value\"`"))?;
        let key = s[..eq].trim();
        if !bare_key_ok(key) {
            return Err(format!("line {line}: bad key {key:?} (want [a-z][a-z0-9_]*)"));
        }
        let (value, consumed) = parse_string(s[eq + 1..].trim_start(), line)?;
        let tail = s[eq + 1..].trim_start()[consumed..].trim();
        if !tail.is_empty() && !tail.starts_with('#') {
            return Err(format!("line {line}: trailing text after value"));
        }

        let full = if section.is_empty() { key.to_string() } else { format!("{section}.{key}") };
        if let Some(prev) = out.iter().find(|e| e.key == full) {
            return Err(format!(
                "line {line}: duplicate key {full:?} (first defined on line {})",
                prev.line
            ));
        }
        out.push(Entry { key: full, value, line });
    }

    Ok(out)
}

/// A TOML basic string on one line. Returns the value and the number of consumed
/// bytes together with the quotes.
fn parse_string(s: &str, line: usize) -> Result<(String, usize), String> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'"') {
        return Err(format!("line {line}: value must be a \"quoted string\""));
    }
    let mut out = String::new();
    let mut it = s.char_indices();
    it.next();
    while let Some((i, c)) = it.next() {
        match c {
            '"' => return Ok((out, i + 1)),
            '\\' => {
                let (_, e) = it
                    .next()
                    .ok_or_else(|| format!("line {line}: string ends with a lone backslash"))?;
                match e {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'u' => {
                        let mut hex = String::new();
                        for _ in 0..4 {
                            let (_, h) = it
                                .next()
                                .ok_or_else(|| format!("line {line}: \\u needs four hex digits"))?;
                            hex.push(h);
                        }
                        let cp = u32::from_str_radix(&hex, 16)
                            .map_err(|_| format!("line {line}: bad \\u{hex}"))?;
                        out.push(
                            char::from_u32(cp)
                                .ok_or_else(|| format!("line {line}: \\u{hex} is not a char"))?,
                        );
                    }
                    other => {
                        return Err(format!("line {line}: unsupported escape \\{other}"));
                    }
                }
            }
            _ => out.push(c),
        }
    }
    Err(format!("line {line}: unterminated string"))
}

/// Indices of the placeholders `{0}`, `{1}`, … in the value. `{{` and `}}` are literals.
/// Used both by the build (cross-checking sets between languages) and by test 125.
pub fn placeholders(s: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '{' {
            if it.peek() == Some(&'{') {
                it.next();
                continue;
            }
            let mut idx = 0usize;
            let mut any = false;
            while let Some(d) = it.peek().and_then(|c| c.to_digit(10)) {
                idx = idx * 10 + d as usize;
                any = true;
                it.next();
            }
            if any && it.peek() == Some(&'}') {
                it.next();
                if !out.contains(&idx) {
                    out.push(idx);
                }
            }
        } else if c == '}' && it.peek() == Some(&'}') {
            it.next();
        }
    }
    out.sort_unstable();
    out
}

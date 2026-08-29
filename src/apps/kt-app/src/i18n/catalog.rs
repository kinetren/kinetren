//! Разбор каталогов перевода `i18n/*.toml`.
//!
//! Формат — СТРОГОЕ подмножество TOML: комментарий `#`, заголовок секции
//! `[a.b]`, пара `key = "value"`. Этого хватает для каталога, а взамен файл
//! остаётся валидным TOML (переводчику работают редакторы и `taplo`), и при
//! этом парсер не тянет ни `toml`, ни `serde` в build-зависимости — тот же
//! приём, что в `libs/kt-version/build.rs`.
//!
//! Модуль подключается ДВАЖДЫ: как модуль крейта и через `#[path]` в
//! `build.rs`. Поэтому он не должен зависеть ни от чего, кроме std.

/// Одна запись каталога: полный ключ (`section.path.key`), значение, номер
/// строки в файле — номер нужен, чтобы ошибка указывала на место.
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

/// Разобрать содержимое каталога. Ошибка — человекочитаемая строка с номером
/// строки; вызывающий добавляет к ней имя файла.
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

        let eq = s
            .find('=')
            .ok_or_else(|| format!("line {line}: expected `key = \"value\"`"))?;
        let key = s[..eq].trim();
        if !bare_key_ok(key) {
            return Err(format!(
                "line {line}: bad key {key:?} (want [a-z][a-z0-9_]*)"
            ));
        }
        let (value, consumed) = parse_string(s[eq + 1..].trim_start(), line)?;
        let tail = s[eq + 1..].trim_start()[consumed..].trim();
        if !tail.is_empty() && !tail.starts_with('#') {
            return Err(format!("line {line}: trailing text after value"));
        }

        let full = if section.is_empty() {
            key.to_string()
        } else {
            format!("{section}.{key}")
        };
        if let Some(prev) = out.iter().find(|e| e.key == full) {
            return Err(format!(
                "line {line}: duplicate key {full:?} (first defined on line {})",
                prev.line
            ));
        }
        out.push(Entry {
            key: full,
            value,
            line,
        });
    }

    Ok(out)
}

/// Basic-строка TOML в одну строку. Возвращает значение и число съеденных
/// байт вместе с кавычками.
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
                            let (_, h) = it.next().ok_or_else(|| {
                                format!("line {line}: \\u needs four hex digits")
                            })?;
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

/// Индексы плейсхолдеров `{0}`, `{1}`, … в значении. `{{` и `}}` — литералы.
/// Используется и сборкой (сверка наборов между языками), и тестом 125.
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

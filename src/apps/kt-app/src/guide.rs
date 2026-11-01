//! Help → Guide (welcome-window-and-tour D9, capability `ui-reach`).
//!
//! The owner (2026-10-27): "We have no guideline inside the program explaining what can be done
//! and where." The guide already exists — `docs/guide/*.md`, checked by `readme-check.py` — so
//! the pages are compiled into the binary FROM THOSE FILES (one source with the repository; no
//! second copy). A guard compares the list below with the directory: a new guide file must be
//! added here, or the test names it.
//!
//! The renderer is our own small one over the subset the guide uses — headings, paragraphs,
//! one-level-nested lists, bold, italic, inline code, `<kbd>` key caps, tables, links between
//! guide pages, fenced code blocks; an image becomes its alt text as a caption. Anything else
//! fails `parse` with the file and the line (t989), so a guide edit the window cannot show
//! breaks the build instead of rendering wrongly. No new dependency.
//!
//! The index "I want to … → where" is the `[guide.index]` catalog section (translated ×5);
//! each entry points to `<file>#<heading-slug>` by the GitHub slug rule, so the same anchor
//! works on the repository page. Search is a case-insensitive substring over the translated
//! entries and the English headings — no stemming (F-HLP-023: a few dozen topics).

use crate::i18n::{k, tr};
use eframe::egui;

/// The guide pages, in the order the window lists them. Guarded against `docs/guide/` (t989).
pub const PAGES: &[(&str, &str)] = &[
    ("getting-started.md", include_str!("../../../../docs/guide/getting-started.md")),
    ("timeline.md", include_str!("../../../../docs/guide/timeline.md")),
    ("effects-and-automation.md", include_str!("../../../../docs/guide/effects-and-automation.md")),
    ("morpher.md", include_str!("../../../../docs/guide/morpher.md")),
    (
        "compositions-and-generators.md",
        include_str!("../../../../docs/guide/compositions-and-generators.md"),
    ),
    ("export.md", include_str!("../../../../docs/guide/export.md")),
    ("shortcuts.md", include_str!("../../../../docs/guide/shortcuts.md")),
    ("whats-new.md", include_str!("../../../../docs/guide/whats-new.md")),
];

/// A run of inline text.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    Kbd(String),
    /// A link: its text and target (`file.md#anchor`, `#anchor`, or an outside path).
    Link(String, String),
}

/// A block of a page.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading { level: u8, text: String, slug: String },
    Para(Vec<Inline>),
    /// An item and its nesting depth (0 or 1); `ordered` carries the number.
    Item { depth: u8, number: Option<u32>, text: Vec<Inline> },
    /// The header row, then the body rows.
    Table(Vec<Vec<Vec<Inline>>>),
    Code(String),
    /// An image, shown as its caption.
    Caption(String),
}

/// The GitHub heading slug: lower case, punctuation dropped (but `-` and `_`), spaces → `-`.
pub fn slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// Parse a page. `Err` names the file, the line and what is outside the subset.
pub fn parse(file: &str, text: &str) -> Result<Vec<Block>, String> {
    let mut out: Vec<Block> = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut table: Vec<Vec<Vec<Inline>>> = Vec::new();
    let mut code: Option<String> = None;
    let mut item: Option<(u8, Option<u32>, Vec<String>)> = None;
    let err = |n: usize, what: &str| format!("{file}:{n}: {what}");

    fn flush_para(out: &mut Vec<Block>, para: &mut Vec<String>, n: usize, file: &str) -> Result<(), String> {
        if !para.is_empty() {
            let text = para.join(" ");
            para.clear();
            out.push(Block::Para(inlines(&text).map_err(|e| format!("{file}:{n}: {e}"))?));
        }
        Ok(())
    }
    fn flush_item(
        out: &mut Vec<Block>,
        item: &mut Option<(u8, Option<u32>, Vec<String>)>,
        n: usize,
        file: &str,
    ) -> Result<(), String> {
        if let Some((depth, number, lines)) = item.take() {
            let text = inlines(&lines.join(" ")).map_err(|e| format!("{file}:{n}: {e}"))?;
            out.push(Block::Item { depth, number, text });
        }
        Ok(())
    }

    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        if let Some(c) = code.as_mut() {
            if raw.trim_start().starts_with("```") {
                out.push(Block::Code(std::mem::take(c)));
                code = None;
            } else {
                if !c.is_empty() {
                    c.push('\n');
                }
                c.push_str(raw);
            }
            continue;
        }
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        // the subset is checked where the line IS, not where its paragraph ends
        check_html(trimmed).map_err(|e| err(n, &e))?;
        let is_table = trimmed.starts_with('|');
        if !is_table && !table.is_empty() {
            out.push(Block::Table(std::mem::take(&mut table)));
        }
        if trimmed.is_empty() {
            flush_para(&mut out, &mut para, n, file)?;
            flush_item(&mut out, &mut item, n, file)?;
            continue;
        }
        if trimmed.starts_with("```") {
            flush_para(&mut out, &mut para, n, file)?;
            flush_item(&mut out, &mut item, n, file)?;
            code = Some(String::new());
            continue;
        }
        if trimmed.starts_with('>') {
            return Err(err(n, "a `>` quote is outside the guide subset"));
        }
        if line.starts_with('#') {
            flush_para(&mut out, &mut para, n, file)?;
            flush_item(&mut out, &mut item, n, file)?;
            let level = line.chars().take_while(|c| *c == '#').count();
            let text = line[level..].trim().trim_end_matches('#').trim().to_string();
            if level > 6 || text.is_empty() {
                return Err(err(n, "a malformed heading"));
            }
            check_html(&text).map_err(|e| err(n, &e))?;
            out.push(Block::Heading { level: level as u8, slug: slug(&text), text });
            continue;
        }
        if is_table {
            flush_para(&mut out, &mut para, n, file)?;
            flush_item(&mut out, &mut item, n, file)?;
            let cells: Vec<&str> = split_cells(trimmed);
            if cells.iter().all(|c| {
                let c = c.trim();
                !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':')
            }) {
                continue; // the separator row
            }
            let mut row = Vec::new();
            for c in cells {
                row.push(inlines(&c.trim().replace("\\|", "|")).map_err(|e| err(n, &e))?);
            }
            table.push(row);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("![") {
            let Some(end) = rest.find("](") else { return Err(err(n, "a malformed image")) };
            flush_para(&mut out, &mut para, n, file)?;
            flush_item(&mut out, &mut item, n, file)?;
            out.push(Block::Caption(rest[..end].to_string()));
            continue;
        }
        let indent = line.len() - trimmed.len();
        let bullet = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* "));
        let numbered = trimmed
            .split_once(". ")
            .filter(|(d, _)| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
        if bullet.is_some() || numbered.is_some() {
            flush_para(&mut out, &mut para, n, file)?;
            flush_item(&mut out, &mut item, n, file)?;
            let depth = if indent >= 2 { 1 } else { 0 };
            if indent >= 6 {
                return Err(err(n, "a list nested two levels deep is outside the guide subset"));
            }
            let (number, body) = match (bullet, numbered) {
                (Some(b), _) => (None, b),
                (None, Some((d, b))) => (d.parse().ok(), b),
                _ => unreachable!(),
            };
            item = Some((depth, number, vec![body.to_string()]));
            continue;
        }
        if let Some((_, _, lines)) = item.as_mut()
            && indent > 0
        {
            lines.push(trimmed.to_string());
            continue;
        }
        flush_item(&mut out, &mut item, n, file)?;
        para.push(trimmed.to_string());
    }
    let n = text.lines().count();
    if code.is_some() {
        return Err(err(n, "an unclosed code block"));
    }
    flush_para(&mut out, &mut para, n, file)?;
    flush_item(&mut out, &mut item, n, file)?;
    if !table.is_empty() {
        out.push(Block::Table(table));
    }
    Ok(out)
}

fn split_cells(row: &str) -> Vec<&str> {
    let body = row.trim().trim_start_matches('|').trim_end_matches('|');
    // a cell may hold `\|`; split on a bare `|` only
    let mut cells = Vec::new();
    let bytes = body.as_bytes();
    let mut start = 0;
    for i in 0..bytes.len() {
        if bytes[i] == b'|' && (i == 0 || bytes[i - 1] != b'\\') {
            cells.push(&body[start..i]);
            start = i + 1;
        }
    }
    cells.push(&body[start..]);
    cells
}

/// HTML other than a `<kbd>…</kbd>` pair, outside inline code.
fn check_html(text: &str) -> Result<(), String> {
    let mut rest = String::new();
    let mut in_code = false;
    for c in text.chars() {
        if c == '`' {
            in_code = !in_code;
        } else if !in_code {
            rest.push(c);
        }
    }
    let rest = rest.replace("<kbd>", "").replace("</kbd>", "");
    let b = rest.as_bytes();
    for i in 0..b.len().saturating_sub(1) {
        if b[i] == b'<' && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'/' || b[i + 1] == b'!') {
            return Err("HTML other than a <kbd>…</kbd> pair is outside the guide subset".into());
        }
    }
    Ok(())
}

/// Split inline text into runs.
pub fn inlines(text: &str) -> Result<Vec<Inline>, String> {
    check_html(text)?;
    let mut out: Vec<Inline> = Vec::new();
    let mut plain = String::new();
    let mut s = text;
    let push_plain = |out: &mut Vec<Inline>, plain: &mut String| {
        if !plain.is_empty() {
            out.push(Inline::Text(std::mem::take(plain)));
        }
    };
    while !s.is_empty() {
        if let Some(r) = s.strip_prefix("<kbd>")
            && let Some(e) = r.find("</kbd>")
        {
            push_plain(&mut out, &mut plain);
            out.push(Inline::Kbd(r[..e].to_string()));
            s = &r[e + "</kbd>".len()..];
            continue;
        }
        if let Some(r) = s.strip_prefix("**")
            && let Some(e) = r.find("**")
        {
            push_plain(&mut out, &mut plain);
            out.push(Inline::Bold(r[..e].to_string()));
            s = &r[e + 2..];
            continue;
        }
        if let Some(r) = s.strip_prefix('`')
            && let Some(e) = r.find('`')
        {
            push_plain(&mut out, &mut plain);
            out.push(Inline::Code(r[..e].to_string()));
            s = &r[e + 1..];
            continue;
        }
        if let Some(r) = s.strip_prefix('*')
            && !r.starts_with(' ')
            && let Some(e) = r.find('*')
        {
            push_plain(&mut out, &mut plain);
            out.push(Inline::Italic(r[..e].to_string()));
            s = &r[e + 1..];
            continue;
        }
        if let Some(r) = s.strip_prefix('[')
            && let Some(e) = r.find("](")
            && let Some(close) = r[e + 2..].find(')')
        {
            push_plain(&mut out, &mut plain);
            out.push(Inline::Link(r[..e].to_string(), r[e + 2..e + 2 + close].to_string()));
            s = &r[e + 2 + close + 1..];
            continue;
        }
        let c = s.chars().next().unwrap();
        plain.push(c);
        s = &s[c.len_utf8()..];
    }
    push_plain(&mut out, &mut plain);
    Ok(out)
}

/// The plain text of runs (search and accessibility).
pub fn plain(runs: &[Inline]) -> String {
    runs.iter()
        .map(|r| match r {
            Inline::Text(t) | Inline::Bold(t) | Inline::Italic(t) | Inline::Code(t) | Inline::Kbd(t) => t.as_str(),
            Inline::Link(t, _) => t.as_str(),
        })
        .collect()
}

/// The plain text of blocks, one line each, list items with a bullet (a lesson's checked step).
pub fn plain_blocks(blocks: &[Block]) -> String {
    let mut out = String::new();
    for b in blocks {
        let line = match b {
            Block::Para(runs) => plain(runs),
            Block::Item { text, .. } => format!("\u{2022} {}", plain(text)),
            Block::Heading { text, .. } | Block::Code(text) | Block::Caption(text) => text.clone(),
            Block::Table(_) => continue,
        };
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&line);
    }
    out
}

/// A parsed page.
pub struct Page {
    pub file: &'static str,
    pub title: String,
    pub blocks: Vec<Block>,
}

/// All pages, parsed once per process (they are compiled in; a parse error is a build-time
/// guard failure, t989 — here such a page shows its error as text).
pub fn pages() -> &'static [Page] {
    static PAGES_PARSED: std::sync::OnceLock<Vec<Page>> = std::sync::OnceLock::new();
    PAGES_PARSED.get_or_init(|| {
        PAGES
            .iter()
            .map(|(file, text)| {
                // the guide writes menu paths with arrows that no font of ours has (U+2192, U+2190);
                // Phosphor does. By code point: a literal here would be scanned as shown text (t100)
                let (right, left) = (char::from_u32(0x2192).unwrap_or(' '), char::from_u32(0x2190).unwrap_or(' '));
                let text = text.replace(right, crate::icons::ARROW_RIGHT).replace(left, crate::icons::ARROW_LEFT);
                let blocks = parse(file, &text).unwrap_or_else(|e| vec![Block::Code(e)]);
                let title = blocks
                    .iter()
                    .find_map(|b| match b {
                        Block::Heading { level: 1, text, .. } => Some(text.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| file.to_string());
                Page { file, title, blocks }
            })
            .collect()
    })
}

/// The What's new notes for a version label (`ktYY.MM`): the section of that version, or the
/// newest section with its own label when the running one has none (D10).
pub fn whats_new(version: &str) -> (String, &'static [Block]) {
    let page = pages().iter().find(|p| p.file == "whats-new.md").expect("whats-new.md is compiled in");
    let sections: Vec<(usize, &str)> = page
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| match b {
            Block::Heading { level: 2, text, .. } => Some((i, text.as_str())),
            _ => None,
        })
        .collect();
    let pick = sections.iter().position(|(_, t)| *t == version).unwrap_or(0);
    let Some(&(start, label)) = sections.get(pick) else { return (version.to_string(), &[]) };
    let end = sections.get(pick + 1).map(|(i, _)| *i).unwrap_or(page.blocks.len());
    (label.to_string(), &page.blocks[start + 1..end])
}

/// The What's new PAGE for the welcome window's pane (welcome-window-and-settings D2): the
/// version's section as `whats_new` picks it, followed by every older section under its own `##`
/// heading — the pane scrolls on its own, so it can hold the whole history.
pub fn whats_new_from(version: &str) -> (String, &'static [Block]) {
    let page = pages().iter().find(|p| p.file == "whats-new.md").expect("whats-new.md is compiled in");
    let (label, section) = whats_new(version);
    if section.is_empty() {
        return (label, section);
    }
    // the section is a sub-slice of the page's blocks: its offset is where it starts
    let start = (section.as_ptr() as usize - page.blocks.as_ptr() as usize) / std::mem::size_of::<Block>();
    (label, &page.blocks[start..])
}

/// One entry of the index: the "I want to…" text, the "where" text, the page and anchor.
pub struct Entry {
    pub want: &'static str,
    pub how: &'static str,
    pub file: &'static str,
    pub anchor: &'static str,
}

/// The index (catalog section `[guide.index]`): pairs of keys and the heading they open.
pub fn index() -> Vec<Entry> {
    // the keys are written IN FULL: t125 finds a key's use by its whole path
    let e = |want, how, file, anchor| Entry { want: tr(want), how: tr(how), file, anchor };
    vec![
        e(k::guide::index::LOOP, k::guide::index::LOOP_HOW, "timeline.md", "markers-and-the-region"),
        e(k::guide::index::CUT, k::guide::index::CUT_HOW, "shortcuts.md", "editing"),
        e(k::guide::index::TITLE, k::guide::index::TITLE_HOW, "compositions-and-generators.md", "generators"),
        e(k::guide::index::EFFECT, k::guide::index::EFFECT_HOW, "effects-and-automation.md", "the-chain"),
        e(k::guide::index::CURVE, k::guide::index::CURVE_HOW, "effects-and-automation.md", "automation-curves"),
        e(k::guide::index::TRANSITION, k::guide::index::TRANSITION_HOW, "timeline.md", "transitions"),
        e(k::guide::index::MORPHER, k::guide::index::MORPHER_HOW, "morpher.md", "the-morpher"),
        e(k::guide::index::REEL, k::guide::index::REEL_HOW, "compositions-and-generators.md", "reels-nested-compositions"),
        e(k::guide::index::RENDER, k::guide::index::RENDER_HOW, "export.md", "render-to-mp4"),
        e(k::guide::index::RECOVER, k::guide::index::RECOVER_HOW, "getting-started.md", "a-first-edit"),
        e(k::guide::index::KEYS, k::guide::index::KEYS_HOW, "shortcuts.md", "keyboard-and-mouse"),
        e(k::guide::index::ZOOM, k::guide::index::ZOOM_HOW, "timeline.md", "navigation-and-zoom"),
    ]
}

/// What the search shows: index entries and page headings whose text holds the query.
pub fn search(query: &str) -> (Vec<usize>, Vec<(usize, usize)>) {
    let q = query.trim().to_lowercase();
    let idx = index();
    let entries = (0..idx.len())
        .filter(|i| {
            q.is_empty()
                || idx[*i].want.to_lowercase().contains(&q)
                || idx[*i].how.to_lowercase().contains(&q)
        })
        .collect();
    // a heading is found by its own text or by the text of its section (the guide says "L loops
    // it" under "Markers and the region"; nobody types the heading's words)
    let mut heads = Vec::new();
    if !q.is_empty() {
        for (pi, p) in pages().iter().enumerate() {
            let mut cur: Option<usize> = None;
            for (bi, b) in p.blocks.iter().enumerate() {
                match b {
                    Block::Heading { level, .. } if *level >= 2 => cur = Some(bi),
                    Block::Heading { .. } => cur = None,
                    _ => {}
                }
                let Some(h) = cur else { continue };
                if heads.last() == Some(&(pi, h)) {
                    continue;
                }
                if block_text(b).to_lowercase().contains(&q) {
                    heads.push((pi, h));
                }
            }
        }
    }
    (entries, heads)
}

/// The plain text of a block (search).
fn block_text(b: &Block) -> String {
    match b {
        Block::Heading { text, .. } | Block::Code(text) | Block::Caption(text) => text.clone(),
        Block::Para(r) | Block::Item { text: r, .. } => plain(r),
        Block::Table(rows) => rows.iter().flatten().map(|c| plain(c)).collect::<Vec<_>>().join(" "),
    }
}

/// The Guide window's state (lives in `KtApp::start`, survives a project open).
#[derive(Default)]
pub struct GuideUi {
    pub open: bool,
    pub query: String,
    /// The page shown and the block to scroll to once.
    pub page: usize,
    pub scroll_to: Option<usize>,
    pub focus: bool,
}

impl GuideUi {
    pub fn open_at(&mut self, file: &str, anchor: &str) {
        self.open = true;
        if let Some(pi) = pages().iter().position(|p| p.file == file) {
            self.page = pi;
            self.scroll_to = pages()[pi]
                .blocks
                .iter()
                .position(|b| matches!(b, Block::Heading { slug, .. } if slug == anchor));
        }
    }
}

/// Draw runs as one wrapped line of a layout job.
fn runs_job(ui: &egui::Ui, runs: &[Inline], size: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let color = ui.visuals().text_color();
    let strong = ui.visuals().strong_text_color();
    let body = egui::FontId::proportional(size);
    let mono = egui::FontId::monospace(size - 1.0);
    for r in runs {
        let (text, fmt) = match r {
            Inline::Text(t) => (t.as_str(), egui::TextFormat::simple(body.clone(), color)),
            Inline::Bold(t) => (t.as_str(), egui::TextFormat::simple(body.clone(), strong)),
            Inline::Italic(t) => {
                (t.as_str(), egui::TextFormat { italics: true, ..egui::TextFormat::simple(body.clone(), color) })
            }
            Inline::Code(t) => (
                t.as_str(),
                egui::TextFormat {
                    background: crate::theme::HOVER,
                    ..egui::TextFormat::simple(mono.clone(), color)
                },
            ),
            Inline::Kbd(t) => (
                t.as_str(),
                egui::TextFormat {
                    background: crate::theme::SEAM,
                    ..egui::TextFormat::simple(mono.clone(), strong)
                },
            ),
            Inline::Link(t, _) => (
                t.as_str(),
                egui::TextFormat {
                    underline: egui::Stroke::new(1.0, crate::theme::ACCENT),
                    ..egui::TextFormat::simple(body.clone(), crate::theme::ACCENT)
                },
            ),
        };
        job.append(text, 0.0, fmt);
    }
    job.wrap.max_width = ui.available_width();
    job
}

/// The cell padding of a guide table.
const CELL_PAD: egui::Vec2 = egui::vec2(8.0, 4.0);

/// The widest run of text in `runs` that cannot wrap: words split at spaces, while key caps and
/// code are whole and glue to their neighbours ("<kbd>Ctrl</kbd>+<kbd>S</kbd>" is one word).
fn min_word_width(ui: &egui::Ui, runs: &[Inline], size: f32) -> f32 {
    let body = egui::FontId::proportional(size);
    let mono = egui::FontId::monospace(size - 1.0);
    let measure =
        |t: &str, f: &egui::FontId| ui.painter().layout_no_wrap(t.to_string(), f.clone(), egui::Color32::WHITE).size().x;
    let (mut best, mut cur) = (0.0f32, 0.0f32);
    for r in runs {
        match r {
            Inline::Kbd(t) | Inline::Code(t) => cur += measure(t, &mono),
            Inline::Text(t) | Inline::Bold(t) | Inline::Italic(t) | Inline::Link(t, _) => {
                for (i, part) in t.split(' ').enumerate() {
                    if i > 0 {
                        best = best.max(cur);
                        cur = 0.0;
                    }
                    if !part.is_empty() {
                        cur += measure(part, &body);
                    }
                }
            }
        }
    }
    best.max(cur)
}

/// The column widths of a table in `avail` points (design D4): a column is never narrower than its
/// longest word; when the natural widths fit, the rest is shared in proportion to them, otherwise
/// each column gets its minimum plus a share of the rest in proportion to how much it would still
/// grow — the long-text columns take the room, a keys column keeps its caps unwrapped.
pub(crate) fn column_widths(min: &[f32], nat: &[f32], avail: f32) -> Vec<f32> {
    let sum_nat: f32 = nat.iter().sum();
    if sum_nat <= avail {
        let extra = avail - sum_nat;
        return nat.iter().map(|n| n + extra * n / sum_nat.max(1.0)).collect();
    }
    // water-filling: a column that needs less than an even share of the rest gets its whole
    // natural width (a keys column stays unwrapped), the others share what is left in
    // proportion to how much they would still grow
    let mut w = min.to_vec();
    let mut rest = (avail - min.iter().sum::<f32>()).max(0.0);
    let mut open: Vec<usize> = (0..w.len()).filter(|&c| nat[c] > min[c]).collect();
    while !open.is_empty() && rest > 0.0 {
        let share = rest / open.len() as f32;
        let (small, big): (Vec<usize>, Vec<usize>) = open.iter().partition(|&&c| nat[c] - w[c] <= share);
        if small.is_empty() {
            let need: f32 = big.iter().map(|&c| nat[c] - w[c]).sum();
            for &c in &big {
                w[c] += rest * (nat[c] - w[c]) / need;
            }
            break;
        }
        for &c in &small {
            rest -= nat[c] - w[c];
            w[c] = nat[c];
        }
        open = big;
    }
    w
}

/// A guide table as a GRID TABLE (kkt-26.11.01 item 12: "all the tables are crooked, their first
/// column is narrow and it looks cheap"): a header row in its own shade, rules between the columns
/// and the rows, row stripes, and column widths from the content. Returns the table's response
/// and the (row, column) of a clicked cell.
fn table(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, rows: &[Vec<Vec<Inline>>]) -> (egui::Response, Option<(usize, usize)>) {
    let id = ui.id().with(id);
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
    let size = 13.0;
    let header = |ui: &egui::Ui, cell: &[Inline]| {
        let mut job = egui::text::LayoutJob::simple_singleline(
            plain(cell),
            egui::FontId::proportional(size),
            ui.visuals().strong_text_color(),
        );
        job.wrap.max_width = f32::INFINITY;
        job
    };
    let job_of = |ui: &egui::Ui, ri: usize, cell: &[Inline]| if ri == 0 { header(ui, cell) } else { runs_job(ui, cell, size) };
    let mut min = vec![0.0f32; ncols];
    let mut nat = vec![0.0f32; ncols];
    for (ri, row) in rows.iter().enumerate() {
        for (ci, cell) in row.iter().enumerate() {
            let mut job = job_of(ui, ri, cell);
            job.wrap.max_width = f32::INFINITY;
            let w = ui.painter().layout_job(job).size().x;
            nat[ci] = nat[ci].max(w + 2.0 * CELL_PAD.x);
            min[ci] = min[ci].max(min_word_width(ui, cell, size) + 2.0 * CELL_PAD.x);
        }
    }
    for c in 0..ncols {
        nat[c] = nat[c].max(min[c]);
    }
    // whole points: a sum a hair over the room would widen the window by it every frame (t918)
    let widths: Vec<f32> = column_widths(&min, &nat, ui.available_width()).iter().map(|w| w.floor()).collect();
    let total: f32 = widths.iter().sum();
    let rule = egui::Stroke::new(1.0, crate::theme::OUTLINE);
    let mut hit = None;
    let mut whole = egui::Rect::NOTHING;
    // rows touch: the rules are the separation, not the layout's spacing
    let gap = std::mem::replace(&mut ui.spacing_mut().item_spacing.y, 0.0);
    for (ri, row) in rows.iter().enumerate() {
        let galleys: Vec<std::sync::Arc<egui::Galley>> = (0..ncols)
            .map(|ci| {
                let mut job = row.get(ci).map(|c| job_of(ui, ri, c)).unwrap_or_default();
                job.wrap.max_width = (widths[ci] - 2.0 * CELL_PAD.x).max(1.0);
                ui.painter().layout_job(job)
            })
            .collect();
        let h = galleys.iter().map(|g| g.size().y).fold(0.0, f32::max) + 2.0 * CELL_PAD.y;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(total, h), egui::Sense::hover());
        whole = whole.union(rect);
        let fill = if ri == 0 {
            Some(crate::theme::HOVER)
        } else if ri % 2 == 0 {
            Some(ui.visuals().faint_bg_color)
        } else {
            None
        };
        if let Some(f) = fill {
            ui.painter().rect_filled(rect, 0.0, f);
        }
        let mut x = rect.left();
        for (ci, g) in galleys.into_iter().enumerate() {
            let cell = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(widths[ci], h));
            let text = ui.visuals().text_color();
            ui.painter().galley(cell.min + CELL_PAD, g, text);
            // each cell is a node for screen readers (and the harness): the text is painted, not
            // a label; the closure runs only while accessibility is on (no per-frame allocation)
            let r = ui.interact(cell, id.with((ri, ci)), egui::Sense::click());
            let label = row.get(ci).map(|c| c.as_slice()).unwrap_or(&[]);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, plain(label)));
            if r.clicked() {
                hit = Some((ri, ci));
            }
            if ci > 0 {
                ui.painter().vline(x, rect.y_range(), rule);
            }
            x += widths[ci];
        }
        ui.painter().hline(rect.x_range(), rect.bottom(), rule);
    }
    ui.spacing_mut().item_spacing.y = gap;
    ui.painter().rect_stroke(whole, 0.0, rule, egui::StrokeKind::Inside);
    #[cfg(test)]
    table_probe::note(id, TableProbe { min: min.clone(), nat: nat.clone(), widths: widths.clone(), rect: whole, room: ui.max_rect().width() });
    (ui.interact(whole, id.with("table"), egui::Sense::hover()), hit)
}

/// Render blocks. A link to a guide page comes back as `(file, anchor)`.
pub fn render(ui: &mut egui::Ui, blocks: &[Block], scroll_to: Option<usize>) -> Option<(String, String)> {
    let mut go: Option<(String, String)> = None;
    let link_of = |runs: &[Inline]| -> Option<(String, String)> {
        runs.iter().find_map(|r| match r {
            Inline::Link(_, t) if !t.contains("://") => {
                let (f, a) = t.split_once('#').unwrap_or((t.as_str(), ""));
                Some((f.rsplit('/').next().unwrap_or(f).to_string(), a.to_string()))
            }
            _ => None,
        })
    };
    for (bi, b) in blocks.iter().enumerate() {
        let r = match b {
            Block::Heading { level, text, .. } => {
                ui.add_space(if *level <= 2 { 8.0 } else { 4.0 });
                let size = match level {
                    1 => 20.0,
                    2 => 16.0,
                    _ => 14.0,
                };
                ui.label(egui::RichText::new(text).size(size).strong())
            }
            Block::Para(runs) => {
                let r = ui.add(egui::Label::new(runs_job(ui, runs, 13.0)).wrap().sense(egui::Sense::click()));
                if r.clicked() && go.is_none() {
                    go = link_of(runs);
                }
                r
            }
            Block::Item { depth, number, text } => {
                let bullet = match number {
                    Some(n) => format!("{n}."),
                    None => "•".to_string(),
                };
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0 + 14.0 * *depth as f32);
                    ui.label(bullet);
                    let r = ui.add(egui::Label::new(runs_job(ui, text, 13.0)).wrap().sense(egui::Sense::click()));
                    if r.clicked() && go.is_none() {
                        go = link_of(text);
                    }
                })
                .response
            }
            Block::Table(rows) => {
                let (r, hit) = table(ui, ("guide_table", bi), rows);
                if let Some((ri, ci)) = hit
                    && go.is_none()
                {
                    go = link_of(&rows[ri][ci]);
                }
                r
            }
            Block::Code(t) => egui::Frame::new()
                .fill(crate::theme::EXTREME)
                .inner_margin(6.0)
                .show(ui, |ui| ui.label(egui::RichText::new(t).monospace()))
                .response,
            Block::Caption(t) => ui.label(egui::RichText::new(format!("[{t}]")).weak().italics()),
        };
        if scroll_to == Some(bi) {
            r.scroll_to_me(Some(egui::Align::TOP));
        }
    }
    go
}

/// The index side's width: a share of the Guide window between these two.
const INDEX_MIN_W: f32 = 240.0;
const INDEX_MAX_W: f32 = 420.0;

/// One "I want to…" entry as a CARD (kkt-26.11.01 item 12: the entries "visually run together,
/// though they are separate sections"): the "what" on the first line, the "where" under it in a
/// weaker style, a frame, a gap below; the whole card is the click target (checklist rule 1).
fn card(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, what: &str, how: Option<&str>) -> egui::Response {
    let id = ui.id().with(id);
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.hovered());
    let inner = egui::Frame::new()
        .fill(if hovered { crate::theme::HOVER } else { ui.visuals().faint_bg_color })
        .stroke(egui::Stroke::new(1.0, crate::theme::OUTLINE))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(egui::RichText::new(what).strong()).wrap());
            if let Some(how) = how {
                ui.add(
                    egui::Label::new(egui::RichText::new(format!("{} {how}", crate::icons::ARROW_RIGHT)).weak())
                        .wrap(),
                );
            }
        });
    ui.add_space(2.0);
    let r = ui.interact(inner.response.rect, id, egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, what));
    r
}

impl crate::app::KtApp {
    /// The Guide window (`kt_guide`): the search field, the "I want to…" index and the page list
    /// on the left, the page on the right. A tool window: it lives beside the work.
    pub(crate) fn draw_guide(&mut self, ctx: &egui::Context) {
        if !self.start.guide.open {
            return;
        }
        let g = &mut self.start.guide;
        let mut go: Option<(String, String)> = None;
        let win = crate::window_shell::Shell::new(
            egui::Id::new("kt_guide"),
            tr(k::guide::TITLE),
            crate::window_shell::Kind::Tool,
        )
        .size([820.0, 560.0], [520.0, 320.0])
        .probe("guide.window")
        .show(ctx, |ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut g.query)
                    .hint_text(tr(k::guide::SEARCH))
                    .desired_width(f32::INFINITY),
            );
            crate::widgets::field_probe::note("guide.search", r.rect);
            if std::mem::take(&mut g.focus) {
                r.request_focus();
            }
            ui.add_space(4.0);
            let (entries, heads) = search(&g.query);
            let idx = index();
            // the two sides SHARE the window (kkt-26.11.01 item 12): the index a proportion with a
            // floor and a ceiling, the page the rest; both lists reach the window's bottom
            let gap = ui.spacing().item_spacing.x;
            let left_w = (ui.available_width() * 0.3).clamp(INDEX_MIN_W, INDEX_MAX_W).floor();
            let page_w = (ui.available_width() - left_w - 2.0 * gap - 6.0).max(200.0).floor();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(left_w);
                    crate::widgets::window_fill(ui, |ui| {
                        ui.label(egui::RichText::new(tr(k::guide::WANT)).strong());
                        for i in &entries {
                            let e = &idx[*i];
                            if card(ui, ("guide_want", *i), e.want, Some(e.how)).clicked() {
                                go = Some((e.file.to_string(), e.anchor.to_string()));
                            }
                        }
                        for (pi, bi) in &heads {
                            let p = &pages()[*pi];
                            if let Block::Heading { text, slug, .. } = &p.blocks[*bi]
                                && card(ui, ("guide_head", *pi, *bi), text, None).clicked()
                            {
                                go = Some((p.file.to_string(), slug.clone()));
                            }
                        }
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(tr(k::guide::PAGES)).strong());
                        ui.weak(tr(k::guide::ENGLISH));
                        for (pi, p) in pages().iter().enumerate() {
                            let r = ui.selectable_label(g.page == pi, p.title.as_str());
                            crate::widgets::field_probe::note(&format!("guide.page.{pi}"), r.rect);
                            if r.clicked() {
                                g.page = pi;
                                g.scroll_to = Some(0);
                            }
                        }
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(page_w);
                    let page = &pages()[g.page.min(pages().len() - 1)];
                    let scroll = g.scroll_to.take();
                    ui.push_id(("guide_page", g.page), |ui| {
                        crate::widgets::window_fill(ui, |ui| {
                            if let Some(l) = render(ui, &page.blocks, scroll) {
                                go = Some(l);
                            }
                        });
                    });
                });
            });
        });
        if win.closed() {
            self.start.guide.open = false;
        }
        if let Some((file, anchor)) = go {
            let file = if file.is_empty() { pages()[self.start.guide.page].file.to_string() } else { file };
            self.start.guide.open_at(&file, &anchor);
        }
    }
}

/// What a drawn table measured and decided (t1128): the harness cannot see painted text widths.
#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct TableProbe {
    pub min: Vec<f32>,
    pub nat: Vec<f32>,
    pub widths: Vec<f32>,
    pub rect: egui::Rect,
    pub room: f32,
}

#[cfg(test)]
pub(crate) mod table_probe {
    use super::TableProbe;
    use eframe::egui;
    use std::cell::RefCell;
    thread_local! {
        static T: RefCell<Vec<(egui::Id, TableProbe)>> = const { RefCell::new(Vec::new()) };
    }
    pub fn note(id: egui::Id, t: TableProbe) {
        T.with(|v| {
            let mut v = v.borrow_mut();
            v.retain(|(i, _)| *i != id);
            v.push((id, t));
        });
    }
    pub fn clear() {
        T.with(|v| v.borrow_mut().clear());
    }
    pub fn all() -> Vec<TableProbe> {
        T.with(|v| v.borrow().iter().map(|(_, t)| t.clone()).collect())
    }
}

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
                egui::Grid::new(("guide_table", bi)).striped(true).num_columns(2).show(ui, |ui| {
                    for (ri, row) in rows.iter().enumerate() {
                        for cell in row {
                            let job = runs_job(ui, cell, 13.0);
                            if ri == 0 {
                                ui.label(egui::RichText::new(plain(cell)).strong());
                            } else {
                                ui.add(egui::Label::new(job).wrap());
                            }
                        }
                        ui.end_row();
                    }
                })
                .response
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
            // the visible part of the window, not the room the content asks for: the lists
            // scroll inside the window instead of running past its bottom edge
            let h = (ui.ctx().content_rect().height() * 0.55).clamp(160.0, ui.available_height().max(160.0));
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(260.0);
                    egui::ScrollArea::vertical().id_salt("guide_left").max_height(h).show(ui, |ui| {
                        ui.label(egui::RichText::new(tr(k::guide::WANT)).strong());
                        for i in &entries {
                            let e = &idx[*i];
                            let r = ui.add(
                                egui::Button::new(format!("{} {} {}", e.want, crate::icons::ARROW_RIGHT, e.how))
                                    .frame(false)
                                    .wrap(),
                            );
                            if r.clicked() {
                                go = Some((e.file.to_string(), e.anchor.to_string()));
                            }
                        }
                        for (pi, bi) in &heads {
                            let p = &pages()[*pi];
                            if let Block::Heading { text, slug, .. } = &p.blocks[*bi]
                                && ui.add(egui::Button::new(text.as_str()).frame(false)).clicked()
                            {
                                go = Some((p.file.to_string(), slug.clone()));
                            }
                        }
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(tr(k::guide::PAGES)).strong());
                        ui.weak(tr(k::guide::ENGLISH));
                        for (pi, p) in pages().iter().enumerate() {
                            if ui.selectable_label(g.page == pi, p.title.as_str()).clicked() {
                                g.page = pi;
                                g.scroll_to = Some(0);
                            }
                        }
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    let page = &pages()[g.page.min(pages().len() - 1)];
                    let scroll = g.scroll_to.take();
                    egui::ScrollArea::vertical()
                        .id_salt(("guide_page", g.page))
                        .max_height(h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(l) = render(ui, &page.blocks, scroll) {
                                go = Some(l);
                            }
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

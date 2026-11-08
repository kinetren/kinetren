//! The symbolizer: a raw report → the report a person reads. Runs in a healthy process. Each address
//! is placed in its mapping, turned into the object file's own address through its load segments,
//! and named from the file's line tables (DWARF), else from its symbol tables, else left as
//! `module+offset`. A system library's separate debug file (`/usr/lib/debug/.build-id/…`) is used
//! when it is installed.

use object::{Object, ObjectSegment, ObjectSymbol};
use std::collections::HashMap;

type Slice = gimli::EndianSlice<'static, gimli::RunTimeEndian>;

/// One executable mapping of the crashed process.
#[derive(Debug, Clone)]
struct Map {
    start: usize,
    end: usize,
    offset: usize,
    path: String,
}

/// What the raw report says.
#[derive(Debug, Default)]
pub(crate) struct Raw {
    header: Vec<String>,
    signal: i32,
    code: i32,
    address: usize,
    pid: i64,
    thread: String,
    tid: i64,
    pc: usize,
    stack: Vec<usize>,
    scan: Vec<usize>,
    threads: Vec<(i64, String)>,
    maps: Vec<Map>,
}

fn hex(s: &str) -> Option<usize> {
    usize::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
}

pub(crate) fn parse(text: &str) -> Option<Raw> {
    let mut r = Raw::default();
    let mut section = "";
    let mut seen = false;
    for line in text.lines() {
        if let Some(s) = line.strip_prefix("--- ").and_then(|s| s.strip_suffix(" ---")) {
            section = match s {
                "stack" | "scan" | "threads" | "maps" | "end" => s,
                _ => "",
            };
            continue;
        }
        match section {
            "stack" => r.stack.extend(hex(line)),
            "scan" => r.scan.extend(hex(line)),
            "threads" => {
                if let Some((t, n)) = line.split_once(' ') {
                    r.threads.push((t.parse().unwrap_or(0), n.to_string()));
                }
            }
            "maps" => {
                let f: Vec<&str> = line.split_whitespace().collect();
                if f.len() >= 6
                    && let Some((a, b)) = f[0].split_once('-')
                {
                    r.maps.push(Map {
                        start: hex(a).unwrap_or(0),
                        end: hex(b).unwrap_or(0),
                        offset: hex(f[2]).unwrap_or(0),
                        path: f[5..].join(" "),
                    });
                }
            }
            "end" => {}
            _ => {
                if line == "raw crash report" {
                    seen = true;
                } else if let Some(v) = line.strip_prefix("signal: ") {
                    r.signal = v.trim().parse().unwrap_or(0);
                } else if let Some(v) = line.strip_prefix("code: ") {
                    r.code = v.trim().parse().unwrap_or(0);
                } else if let Some(v) = line.strip_prefix("address: ") {
                    r.address = hex(v).unwrap_or(0);
                } else if let Some(v) = line.strip_prefix("pid: ") {
                    r.pid = v.trim().parse().unwrap_or(0);
                } else if let Some(v) = line.strip_prefix("thread: ") {
                    r.thread = v.to_string();
                } else if let Some(v) = line.strip_prefix("tid: ") {
                    r.tid = v.trim().parse().unwrap_or(0);
                } else if let Some(v) = line.strip_prefix("pc: ") {
                    r.pc = hex(v).unwrap_or(0);
                } else if !seen && !line.starts_with("unix: ") && !line.starts_with("sp: ") {
                    r.header.push(line.to_string());
                }
            }
        }
    }
    seen.then_some(r)
}

/// A frame: where in which module, and what the module's tables say.
#[derive(Debug, Clone, Default)]
pub(crate) struct Named {
    pub module: String,
    pub offset: usize,
    /// Inlined calls first, the outer function last: `function (file:line)`.
    pub names: Vec<String>,
}

impl Named {
    fn place(&self) -> String {
        format!("{}+{:#x}", self.module, self.offset)
    }
}

struct Module {
    /// The object's own address for a file offset: (file offset, size, address) per load segment.
    segments: Vec<(u64, u64, u64)>,
    dwarf: Option<addr2line::Context<Slice>>,
    /// Sorted (address, size, name) of the function symbols, static and dynamic.
    symbols: Vec<(u64, u64, String)>,
}

fn leak_map(path: &std::path::Path) -> Option<&'static [u8]> {
    let f = std::fs::File::open(path).ok()?;
    // SAFETY: the file is read, never written by us; a change under us gives wrong names, no fault
    // the symbolizer cannot survive (the binary of a crashed run is not rebuilt while it reads)
    let m = unsafe { memmap2::Mmap::map(&f) }.ok()?;
    Some(&Box::leak(Box::new(m))[..])
}

fn dwarf_of(file: &object::File<'static>) -> Option<addr2line::Context<Slice>> {
    file.section_by_name(".debug_info")?;
    let endian = if file.is_little_endian() { gimli::RunTimeEndian::Little } else { gimli::RunTimeEndian::Big };
    let dwarf = gimli::Dwarf::load(|id: gimli::SectionId| -> Result<Slice, gimli::Error> {
        let data = file
            .section_by_name(id.name())
            .and_then(|s| object::ObjectSection::data(&s).ok())
            .unwrap_or(&[]);
        Ok(gimli::EndianSlice::new(data, endian))
    })
    .ok()?;
    addr2line::Context::from_dwarf(dwarf).ok()
}

fn open_module(path: &str) -> Option<Module> {
    let data = leak_map(std::path::Path::new(path))?;
    let file = object::File::parse(data).ok()?;
    let segments = file
        .segments()
        .map(|s| {
            let (off, size) = s.file_range();
            (off, size, s.address())
        })
        .collect();
    let mut dwarf = dwarf_of(&file);
    if dwarf.is_none()
        && let Ok(Some(id)) = file.build_id()
        && id.len() > 1
    {
        // the distribution's separate debug file, when it is installed
        let hexid: String = id.iter().map(|b| format!("{b:02x}")).collect();
        let debug = format!("/usr/lib/debug/.build-id/{}/{}.debug", &hexid[..2], &hexid[2..]);
        if let Some(d) = leak_map(std::path::Path::new(&debug))
            && let Ok(f) = object::File::parse(d)
        {
            dwarf = dwarf_of(&f);
        }
    }
    let mut symbols: Vec<(u64, u64, String)> = file
        .symbols()
        .chain(file.dynamic_symbols())
        .filter(|s| s.kind() == object::SymbolKind::Text && s.address() != 0)
        .filter_map(|s| Some((s.address(), s.size(), s.name().ok()?.to_string())))
        .collect();
    symbols.sort_by_key(|s| s.0);
    symbols.dedup_by_key(|s| s.0);
    Some(Module { segments, dwarf, symbols })
}

fn demangle(name: &str) -> String {
    addr2line::demangle_auto(std::borrow::Cow::Borrowed(name), None).into_owned()
}

/// Names addresses of one report, each module opened once.
pub(crate) struct Namer<'a> {
    maps: &'a [Map],
    modules: HashMap<String, Option<Module>>,
}

impl<'a> Namer<'a> {
    fn new(maps: &'a [Map]) -> Self {
        Namer { maps, modules: HashMap::new() }
    }

    /// `ret`: a return address — the call is the instruction before it.
    pub(crate) fn name(&mut self, addr: usize, ret: bool) -> Named {
        let Some(m) = self.maps.iter().find(|m| addr >= m.start && addr < m.end) else {
            return Named { module: "?".into(), offset: addr, names: Vec::new() };
        };
        let file_off = (addr - m.start + m.offset) as u64;
        let module = m.path.rsplit('/').next().unwrap_or(&m.path).to_string();
        let mut out = Named { module, offset: file_off as usize, names: Vec::new() };
        let Some(md) = self.modules.entry(m.path.clone()).or_insert_with(|| open_module(&m.path)).as_ref() else {
            return out;
        };
        let Some(svma) = md.segments.iter().find(|s| file_off >= s.0 && file_off < s.0 + s.1).map(|s| file_off - s.0 + s.2)
        else {
            return out;
        };
        let probe = if ret { svma.saturating_sub(1) } else { svma };
        if let Some(ctx) = &md.dwarf
            && let Ok(mut frames) = ctx.find_frames(probe).skip_all_loads()
        {
            while let Ok(Some(f)) = frames.next() {
                let func = f
                    .function
                    .as_ref()
                    .and_then(|n| n.demangle().ok().map(|c| c.into_owned()))
                    .unwrap_or_else(|| "?".into());
                let place =
                    f.location.as_ref().and_then(|l| Some(format!("{}:{}", short_path(l.file?), l.line.unwrap_or(0))));
                out.names.push(match place {
                    Some(p) => format!("{} ({p})", strip_hash(&func)),
                    None => strip_hash(&func),
                });
            }
        }
        if out.names.is_empty() {
            let i = md.symbols.partition_point(|s| s.0 <= probe);
            if i > 0 {
                let (a, size, name) = &md.symbols[i - 1];
                if *size == 0 || probe < a + size {
                    out.names.push(format!("{}+{:#x}", strip_hash(&demangle(name)), probe - a));
                }
            }
        }
        out
    }
}

/// `kt_gpu::x::h1a2b3c4d5e6f7a8b` → `kt_gpu::x`: the legacy mangling's hash is noise to a reader.
fn strip_hash(s: &str) -> String {
    match s.rfind("::h") {
        Some(i) if s.len() - i == 19 && s[i + 3..].bytes().all(|c| c.is_ascii_hexdigit()) => s[..i].to_string(),
        _ => s.to_string(),
    }
}

/// A source path as a reader of this tree writes it: `libs/kt-gpu/src/x.rs`, `library/std/…` for
/// the standard library, `wgpu-hal-30.0.0/src/…` for a crate of the registry.
fn short_path(p: &str) -> &str {
    for mark in ["/src/libs/", "/src/apps/", "/src/plugins/"] {
        if let Some(i) = p.rfind(mark) {
            return &p[i + 5..];
        }
    }
    if p.starts_with("/rustc/")
        && let Some(i) = p.find("/library/")
    {
        return &p[i + 1..];
    }
    if let Some(i) = p.find("/registry/src/")
        && let Some(j) = p[i + 14..].find('/')
    {
        return &p[i + 14 + j + 1..];
    }
    p
}

/// Our own code, as a reader looks for it first.
fn ours(name: &str) -> bool {
    const OURS: [&str; 7] = ["kt_", "fx_", "transitions_", "generators_", "kinetren", "kt_cli", "<kt_"];
    OURS.iter().any(|p| name.starts_with(p))
}

fn code_words(r: &Raw) -> &'static str {
    match (r.signal, r.code) {
        (11, 1) => "address not mapped",
        (11, 2) => "access to a protected page",
        (7, 1) => "misaligned address",
        (7, 2) => "address beyond the object",
        (_, 0) | (_, -6) => "sent by the process itself",
        _ => "",
    }
}

pub(crate) fn symbolize(raw: &str) -> String {
    let Some(r) = parse(raw) else { return raw.to_string() };
    let mut namer = Namer::new(&r.maps);
    let mut out = String::new();
    for h in &r.header {
        out.push_str(h);
        out.push('\n');
    }
    out.push_str(&format!("signal: {}", crate::signal_name(r.signal)));
    let words = code_words(&r);
    out.push_str(&format!(", code {}{}", r.code, if words.is_empty() { String::new() } else { format!(" ({words})") }));
    if [4, 7, 8, 11].contains(&r.signal) {
        out.push_str(&format!(", address {:#x}", r.address));
    }
    out.push('\n');
    out.push_str(&format!("thread: {} (tid {}, pid {})\n", r.thread, r.tid, r.pid));
    out.push_str("\nstack of the faulting thread:\n");
    for (i, &a) in r.stack.iter().enumerate().filter(|(_, a)| **a != 0) {
        push_frame(&mut out, i, a, namer.name(a, i > 0));
    }
    if !r.scan.is_empty() {
        out.push_str("\nthe unwinder stopped; code addresses found on the stack (callers, some stale):\n");
        for (i, &a) in r.scan.iter().enumerate() {
            push_frame(&mut out, i, a, namer.name(a, true));
        }
    }
    out.push_str(&format!("\nthreads ({}):\n", r.threads.len()));
    for (tid, name) in &r.threads {
        let mark = if *tid == r.tid { "  ← faulted" } else { "" };
        out.push_str(&format!("  {tid:>8} {name}{mark}\n"));
    }
    out
}

fn push_frame(out: &mut String, i: usize, addr: usize, n: Named) {
    if n.names.is_empty() {
        out.push_str(&format!("  {i:>3}  {addr:#018x}  {}\n", n.place()));
        return;
    }
    for (j, name) in n.names.iter().enumerate() {
        if j == 0 {
            out.push_str(&format!("  {i:>3}  {addr:#018x}  {name}   [{}]\n", n.place()));
        } else {
            out.push_str(&format!("       {:18}  inlined into {name}\n", ""));
        }
    }
}

pub(crate) fn summary(raw: &str) -> Option<String> {
    let r = parse(raw)?;
    let mut namer = Namer::new(&r.maps);
    let sig = crate::signal_name(r.signal);
    let sig = sig.split(' ').next().unwrap_or(sig);
    let named: Vec<Named> = r.stack.iter().enumerate().map(|(i, &a)| namer.name(a, i > 0)).collect();
    let scanned: Vec<Named> = r.scan.iter().map(|&a| namer.name(a, true)).collect();
    let top = named.first().map(|n| n.names.last().cloned().unwrap_or_else(|| n.place()));
    let own = named.iter().chain(&scanned).flat_map(|n| n.names.iter()).find(|s| ours(s)).cloned();
    let mut s = format!("{sig} in thread \"{}\"", r.thread);
    match (top, own) {
        (Some(t), Some(o)) if t == o => s.push_str(&format!(" at {t}")),
        (Some(t), Some(o)) => s.push_str(&format!(" at {t}, called from {o}")),
        (Some(t), None) => s.push_str(&format!(" at {t}")),
        (None, _) => {}
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t2083_raw_report_parses_and_hashes_are_dropped() {
        let raw = "kinetren v1\nraw crash report\nsignal: 11\ncode: 1\naddress: 0x0000000000000000\npid: 7\nunix: 1\n\
                   thread: audio-render\ntid: 9\npc: 0x0000000000001000\nsp: 0x0\n--- stack ---\n0x0000000000001000\n\
                   0x0000000000002000\n--- threads ---\n7 kinetren\n9 audio-render\n--- maps ---\n\
                   00001000-00003000 r-xp 00000000 00:00 0 /nonexistent/kinetren\n--- end ---\n";
        let r = parse(raw).expect("a raw report");
        assert_eq!((r.signal, r.code, r.tid, r.pid), (11, 1, 9, 7));
        assert_eq!(r.thread, "audio-render");
        assert_eq!(r.stack, vec![0x1000, 0x2000]);
        assert_eq!(r.threads.len(), 2);
        assert_eq!(r.maps[0].path, "/nonexistent/kinetren");
        assert_eq!(r.header, vec!["kinetren v1".to_string()]);
        let text = symbolize(raw);
        assert!(text.contains("SIGSEGV (segmentation fault), code 1 (address not mapped), address 0x0"), "{text}");
        assert!(text.contains("thread: audio-render (tid 9, pid 7)"), "{text}");
        assert!(text.contains("kinetren+0x1000") && text.contains("← faulted"), "{text}");
        assert_eq!(summary(raw).as_deref(), Some("SIGSEGV in thread \"audio-render\" at kinetren+0x0"));
        assert_eq!(strip_hash("kt_gpu::engine::run::h0123456789abcdef"), "kt_gpu::engine::run");
        assert_eq!(strip_hash("kt_gpu::engine::run"), "kt_gpu::engine::run");
        assert!(parse("plain text").is_none() && symbolize("plain text") == "plain text");
    }
}

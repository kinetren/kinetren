//! The native-crash report (segv-crashes; capability `durability`).
//!
//! The owner, kkt-26.11.08 H5: "краш скудный" — a SIGSEGV left a report that named the signal and
//! nothing else. A process that dies by a native signal cannot be asked anything afterwards, so the
//! report is written AT the signal, by a handler that does only what is safe there:
//!
//! * [`install`] (once, at start) puts a handler on SIGSEGV, SIGBUS, SIGILL, SIGFPE and SIGABRT;
//! * the handler writes a RAW report: the signal, its code and address, the faulting thread's name
//!   and id, its program counter, its stack (the unwinder's frames and, when the unwinder stops early,
//!   the code addresses found on the stack), every thread's name, and the executable mappings —
//!   with `open`/`write`/`read` and fixed static buffers, no allocation, no lock of ours;
//! * [`symbolize`] turns the raw report into the one a person reads (functions, files and lines from
//!   the binaries' own line tables), in a HEALTHY process: the editor's supervisor, the next start,
//!   or a child the handler starts (kt-cli).
//!
//! After writing, the handler restores the default action and re-raises: the process still dies by
//! its own signal, so the supervisor's verdict, the exit status and a core dump stay what they were.

use std::path::Path;

#[cfg(target_os = "linux")]
mod handler;
#[cfg(target_os = "linux")]
mod symbols;

/// Where the handler writes and what it does after.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// The raw report's path; it is created at the signal (its folder must exist).
    pub raw: std::path::PathBuf,
    /// The first line(s): the program and its full version.
    pub header: String,
    /// A command the handler starts after writing (`argv[0]` is the executable's path; the raw
    /// report's path is appended) and waits for, bounded: kt-cli prints the symbolized report
    /// itself, there is no supervisor to do it. `None`: someone else symbolizes.
    pub symbolizer: Option<Vec<std::ffi::OsString>>,
}

/// Put the handler on the fatal signals. `false` where there is none (not Linux) or a second call.
pub fn install(cfg: &Config) -> bool {
    #[cfg(target_os = "linux")]
    {
        handler::install(cfg)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cfg;
        false
    }
}

/// The report a person reads, from a raw one. Names what it can: a frame with no line table keeps
/// its module and offset, so the report stays exact for whoever has the same build.
pub fn symbolize(raw: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        symbols::symbolize(raw)
    }
    #[cfg(not(target_os = "linux"))]
    {
        raw.to_string()
    }
}

/// One line for a window, a journal or a terminal: the signal, the thread and where — the first
/// frame of our own code and the top frame when it is elsewhere. `None` for text that is no raw
/// report.
pub fn summary(raw: &str) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        symbols::summary(raw)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = raw;
        None
    }
}

/// Symbolize the raw report at `raw` into `out` (the human report, overwritten) and return its
/// summary line. The raw file stays: it is the exact record, for another symbolizer.
pub fn symbolize_file(raw: &Path, out: &Path) -> std::io::Result<Option<String>> {
    let text = std::fs::read_to_string(raw)?;
    std::fs::write(out, symbolize(&text))?;
    Ok(summary(&text))
}

/// The signal's name in a report, e.g. `SIGSEGV (segmentation fault)`.
pub fn signal_name(sig: i32) -> &'static str {
    match sig {
        4 => "SIGILL (illegal instruction)",
        5 => "SIGTRAP",
        6 => "SIGABRT (abort)",
        7 => "SIGBUS (bus error)",
        8 => "SIGFPE (arithmetic error)",
        11 => "SIGSEGV (segmentation fault)",
        _ => "a fatal signal",
    }
}

/// Test helpers: crash a child process on purpose, never the test process itself.
#[doc(hidden)]
pub mod testing {
    /// Fault by a write through a null pointer, from a function that keeps its name in the stack.
    #[inline(never)]
    pub fn kt_crash_probe_segv() {
        let p = std::hint::black_box(std::ptr::null_mut::<u32>());
        // SAFETY: none — the fault is the point; only a child process started for it calls this
        unsafe { p.write_volatile(7) };
    }

    /// Abort, as a panic inside a panic does.
    #[inline(never)]
    pub fn kt_crash_probe_abort() {
        std::process::abort();
    }
}

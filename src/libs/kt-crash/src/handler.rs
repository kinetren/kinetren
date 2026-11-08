//! The signal handler. Everything here runs inside a dying process, on the faulting thread: only
//! system calls (`open`, `read`, `write`, `getdents64`, `process_vm_readv`, `prctl`, `clone`,
//! `execve`, `wait4`, `tgkill`), fixed static buffers, and the unwinder of libgcc (primed at
//! install, so its one-time set-up does not happen here). No allocation, no lock of ours, no
//! formatting machinery.
//!
//! Bounded: an alarm guards the whole handler. If the unwinder or the symbolizer child hangs, the
//! alarm ends the process by the original signal; what was written by then stays written, and the
//! signal and thread lines come first.

use std::cell::UnsafeCell;
use std::ffi::{CString, c_int, c_void};
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

const SIGNALS: [c_int; 5] = [libc::SIGSEGV, libc::SIGBUS, libc::SIGILL, libc::SIGFPE, libc::SIGABRT];
/// Seconds the handler may take, writing and symbolizing included.
const WATCHDOG_S: u32 = 20;
const MAX_FRAMES: usize = 128;
const MAX_SCAN: usize = 64;
/// How far above the stack pointer the scan reads.
const SCAN_BYTES: usize = 128 * 1024;
const MAPS_BYTES: usize = 1 << 20;
const MAX_RANGES: usize = 4096;

struct Static<T>(UnsafeCell<T>);
// SAFETY: written once by `install` before any handler can run; the handler's buffers are used by
// one thread at a time (the first to enter, `OWNER`)
unsafe impl<T> Sync for Static<T> {}
impl<T> Static<T> {
    const fn new(v: T) -> Self {
        Static(UnsafeCell::new(v))
    }
    #[allow(clippy::mut_from_ref)]
    unsafe fn get(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
}

struct Setup {
    raw: Option<CString>,
    header: Vec<u8>,
    /// The symbolizer's argv, NUL-terminated pointer array into `exec_strings`.
    exec: Vec<*const libc::c_char>,
    exec_strings: Vec<CString>,
}

static INSTALLED: AtomicBool = AtomicBool::new(false);
/// The thread inside the handler (0: none).
static OWNER: AtomicI32 = AtomicI32::new(0);
/// The signal being reported, for the watchdog.
static SIGNAL: AtomicI32 = AtomicI32::new(0);
static SETUP: Static<Setup> =
    Static::new(Setup { raw: None, header: Vec::new(), exec: Vec::new(), exec_strings: Vec::new() });
static MAPS: Static<[u8; MAPS_BYTES]> = Static::new([0; MAPS_BYTES]);
static RANGES: Static<[(usize, usize); MAX_RANGES]> = Static::new([(0, 0); MAX_RANGES]);
static FRAMES: Static<[usize; MAX_FRAMES]> = Static::new([0; MAX_FRAMES]);
static SCAN: Static<[u8; 4096]> = Static::new([0; 4096]);
static DENTS: Static<[u8; 8192]> = Static::new([0; 8192]);

pub fn install(cfg: &crate::Config) -> bool {
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return false;
    }
    let Ok(raw) = CString::new(cfg.raw.as_os_str().as_bytes()) else { return false };
    // SAFETY: no handler is installed yet, nothing else reads SETUP
    let setup = unsafe { SETUP.get() };
    setup.raw = Some(raw);
    setup.header = cfg.header.as_bytes().to_vec();
    if !setup.header.ends_with(b"\n") {
        setup.header.push(b'\n');
    }
    if let Some(argv) = &cfg.symbolizer {
        setup.exec_strings = argv
            .iter()
            .map(|a| a.as_bytes())
            .chain(std::iter::once(cfg.raw.as_os_str().as_bytes()))
            .filter_map(|a| CString::new(a).ok())
            .collect();
        setup.exec = setup.exec_strings.iter().map(|s| s.as_ptr()).chain(std::iter::once(std::ptr::null())).collect();
    }
    // the unwinder's first walk registers the loaded objects' tables: do it now, not at the signal
    let mut n = 0usize;
    unsafe { _Unwind_Backtrace(count_frame, (&raw mut n).cast()) };
    for sig in SIGNALS {
        // SAFETY: a plain sigaction with a handler of the right signature; no SA_ONSTACK: Rust's
        // per-thread alternate stack is a few KiB, too small for the unwinder, and a thread from a
        // C library has none (a stack overflow then ends without this report, see design.md)
        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = on_signal as *const () as usize;
            sa.sa_flags = libc::SA_SIGINFO;
            libc::sigemptyset(&mut sa.sa_mask);
            libc::sigaction(sig, &sa, std::ptr::null_mut());
        }
    }
    true
}

unsafe extern "C" {
    fn _Unwind_Backtrace(trace: extern "C" fn(*mut c_void, *mut c_void) -> c_int, arg: *mut c_void) -> c_int;
    fn _Unwind_GetIP(ctx: *mut c_void) -> usize;
}

const URC_NO_REASON: c_int = 0;
const URC_END_OF_STACK: c_int = 5;

extern "C" fn count_frame(_ctx: *mut c_void, arg: *mut c_void) -> c_int {
    // SAFETY: `arg` is the `usize` the caller passed
    unsafe { *(arg as *mut usize) += 1 };
    URC_NO_REASON
}

struct Walk {
    n: usize,
}

extern "C" fn take_frame(ctx: *mut c_void, arg: *mut c_void) -> c_int {
    // SAFETY: `arg` is the handler's `Walk`; FRAMES belongs to the handler's owner
    unsafe {
        let w = &mut *(arg as *mut Walk);
        if w.n >= MAX_FRAMES {
            return URC_END_OF_STACK;
        }
        FRAMES.get()[w.n] = _Unwind_GetIP(ctx);
        w.n += 1;
    }
    URC_NO_REASON
}

/// A writer over a file descriptor: bytes, hex and decimal numbers, no allocation.
struct Out(c_int);

impl Out {
    fn bytes(&self, b: &[u8]) {
        let mut b = b;
        while !b.is_empty() {
            // SAFETY: a write of a live slice
            let n = unsafe { libc::write(self.0, b.as_ptr().cast(), b.len()) };
            if n <= 0 {
                return;
            }
            b = &b[n as usize..];
        }
    }
    fn hex(&self, v: usize) {
        let mut buf = [0u8; 18];
        buf[0] = b'0';
        buf[1] = b'x';
        for i in 0..16 {
            let d = (v >> ((15 - i) * 4)) & 0xf;
            buf[2 + i] = b"0123456789abcdef"[d];
        }
        self.bytes(&buf);
    }
    fn dec(&self, v: i64) {
        let mut buf = [0u8; 21];
        let mut i = buf.len();
        let neg = v < 0;
        let mut u = v.unsigned_abs();
        loop {
            i -= 1;
            buf[i] = b'0' + (u % 10) as u8;
            u /= 10;
            if u == 0 {
                break;
            }
        }
        if neg {
            i -= 1;
            buf[i] = b'-';
        }
        self.bytes(&buf[i..]);
    }
}

fn gettid() -> i32 {
    // SAFETY: a system call with no arguments
    unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

/// Read a whole small file into `buf`; the length read.
fn read_file(path: &[u8], buf: &mut [u8]) -> usize {
    // SAFETY: `path` ends with a NUL, `buf` is live
    unsafe {
        let fd = libc::open(path.as_ptr().cast(), libc::O_RDONLY | libc::O_CLOEXEC);
        if fd < 0 {
            return 0;
        }
        let mut n = 0;
        while n < buf.len() {
            let r = libc::read(fd, buf[n..].as_mut_ptr().cast(), buf.len() - n);
            if r <= 0 {
                break;
            }
            n += r as usize;
        }
        libc::close(fd);
        n
    }
}

fn parse_hex(s: &[u8]) -> usize {
    s.iter().fold(0usize, |a, &c| {
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return a,
        };
        (a << 4) | d as usize
    })
}

/// The executable mappings of `/proc/self/maps` (already in MAPS): their ranges into RANGES, and
/// each such line written out when `out` is given. Returns the number of ranges.
fn exec_lines(maps: &[u8], out: Option<&Out>) -> usize {
    // SAFETY: the handler's owner
    let ranges = unsafe { RANGES.get() };
    let mut n = 0;
    for line in maps.split(|&c| c == b'\n') {
        let mut fields = line.split(|&c| c == b' ').filter(|f| !f.is_empty());
        let (Some(range), Some(perms)) = (fields.next(), fields.next()) else { continue };
        if perms.get(2) != Some(&b'x') {
            continue;
        }
        let mut ends = range.split(|&c| c == b'-');
        let (Some(a), Some(b)) = (ends.next(), ends.next()) else { continue };
        if n < MAX_RANGES {
            ranges[n] = (parse_hex(a), parse_hex(b));
            n += 1;
        }
        if let Some(o) = out {
            o.bytes(line);
            o.bytes(b"\n");
        }
    }
    n
}

fn in_code(addr: usize, n: usize) -> bool {
    // SAFETY: the handler's owner
    let ranges = unsafe { RANGES.get() };
    ranges[..n].iter().any(|&(a, b)| addr >= a && addr < b)
}

/// The words above the stack pointer that point into code: return addresses, some stale. Read with
/// `process_vm_readv`, which answers EFAULT at the stack's end instead of faulting.
fn scan_stack(out: &Out, sp: usize, ranges: usize) {
    // SAFETY: the handler's owner
    let buf = unsafe { SCAN.get() };
    let mut found = 0;
    let mut at = sp & !7;
    let end = sp.saturating_add(SCAN_BYTES);
    while at < end && found < MAX_SCAN {
        let local = libc::iovec { iov_base: buf.as_mut_ptr().cast(), iov_len: buf.len() };
        let remote = libc::iovec { iov_base: at as *mut c_void, iov_len: buf.len() };
        // SAFETY: reads our own memory into our own buffer; a hole is an error, not a fault
        let n = unsafe { libc::process_vm_readv(libc::getpid(), &local, 1, &remote, 1, 0) };
        if n <= 0 {
            break;
        }
        for w in buf[..n as usize].chunks_exact(8) {
            let v = usize::from_ne_bytes([w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]]);
            if in_code(v, ranges) {
                out.hex(v);
                out.bytes(b"\n");
                found += 1;
                if found >= MAX_SCAN {
                    break;
                }
            }
        }
        at += n as usize;
    }
}

/// `/proc/self/task/<tid>/comm` for each thread: "tid name" lines.
fn thread_names(out: &Out) {
    // SAFETY: the handler's owner
    let dents = unsafe { DENTS.get() };
    let fd = unsafe { libc::open(c"/proc/self/task".as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
    if fd < 0 {
        return;
    }
    loop {
        // SAFETY: getdents64 into our buffer
        let n = unsafe { libc::syscall(libc::SYS_getdents64, fd, dents.as_mut_ptr(), dents.len()) };
        if n <= 0 {
            break;
        }
        let mut off = 0usize;
        while off < n as usize {
            // linux_dirent64: d_ino u64, d_off i64, d_reclen u16, d_type u8, d_name
            let reclen = u16::from_ne_bytes([dents[off + 16], dents[off + 17]]) as usize;
            let name = &dents[off + 19..off + reclen];
            let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
            if !name.is_empty() && name[0].is_ascii_digit() {
                let mut path = [0u8; 64];
                let pre = b"/proc/self/task/";
                let suf = b"/comm\0";
                if pre.len() + name.len() + suf.len() <= path.len() {
                    path[..pre.len()].copy_from_slice(pre);
                    path[pre.len()..pre.len() + name.len()].copy_from_slice(name);
                    path[pre.len() + name.len()..pre.len() + name.len() + suf.len()].copy_from_slice(suf);
                    let mut comm = [0u8; 32];
                    let len = read_file(&path, &mut comm);
                    out.bytes(name);
                    out.bytes(b" ");
                    out.bytes(comm[..len].strip_suffix(b"\n").unwrap_or(&comm[..len]));
                    out.bytes(b"\n");
                }
            }
            off += reclen;
        }
    }
    // SAFETY: our descriptor
    unsafe { libc::close(fd) };
}

#[cfg(target_arch = "x86_64")]
fn pc_sp(uc: *mut c_void) -> (usize, usize) {
    // SAFETY: the kernel's ucontext for this signal
    unsafe {
        let uc = &*(uc as *const libc::ucontext_t);
        (uc.uc_mcontext.gregs[libc::REG_RIP as usize] as usize, uc.uc_mcontext.gregs[libc::REG_RSP as usize] as usize)
    }
}

#[cfg(target_arch = "aarch64")]
fn pc_sp(uc: *mut c_void) -> (usize, usize) {
    // SAFETY: the kernel's ucontext for this signal
    unsafe {
        let uc = &*(uc as *const libc::ucontext_t);
        (uc.uc_mcontext.pc as usize, uc.uc_mcontext.sp as usize)
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn pc_sp(_uc: *mut c_void) -> (usize, usize) {
    (0, 0)
}

/// Die by `sig`: the default action, then the signal again to this thread. It is blocked while the
/// handler runs, so it lands as the handler returns.
fn die_by(sig: c_int) {
    // SAFETY: plain system calls
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = libc::SIG_DFL;
        libc::sigaction(sig, &sa, std::ptr::null_mut());
        libc::syscall(libc::SYS_tgkill, libc::getpid(), gettid(), sig);
    }
}

extern "C" fn on_alarm(_sig: c_int, _info: *mut libc::siginfo_t, _uc: *mut c_void) {
    let sig = SIGNAL.load(Ordering::Acquire);
    // SAFETY: plain system calls; the faulting thread blocks `sig`, another thread takes it
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = libc::SIG_DFL;
        libc::sigaction(sig, &sa, std::ptr::null_mut());
        libc::kill(libc::getpid(), sig);
        let ts = libc::timespec { tv_sec: 1, tv_nsec: 0 };
        libc::nanosleep(&ts, std::ptr::null_mut());
        libc::_exit(128 + sig);
    }
}

extern "C" fn on_signal(sig: c_int, info: *mut libc::siginfo_t, uc: *mut c_void) {
    let tid = gettid();
    if let Err(owner) = OWNER.compare_exchange(0, tid, Ordering::AcqRel, Ordering::Acquire) {
        if owner != tid {
            // another thread is writing the report and will end the process: wait for it
            loop {
                let ts = libc::timespec { tv_sec: 1, tv_nsec: 0 };
                // SAFETY: a sleep
                unsafe { libc::nanosleep(&ts, std::ptr::null_mut()) };
            }
        }
        // a fault inside the handler itself: give up the report
        die_by(sig);
        return;
    }
    SIGNAL.store(sig, Ordering::Release);
    // SAFETY: plain system calls
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = on_alarm as *const () as usize;
        sa.sa_flags = libc::SA_SIGINFO;
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(libc::SIGALRM, &sa, std::ptr::null_mut());
        libc::alarm(WATCHDOG_S);
    }
    report(sig, info, uc, tid);
    die_by(sig);
}

fn report(sig: c_int, info: *mut libc::siginfo_t, uc: *mut c_void, tid: i32) {
    // SAFETY: written once by `install`, read-only now
    let setup = unsafe { SETUP.get() };
    let Some(raw) = &setup.raw else { return };
    // SAFETY: a NUL-terminated path
    let fd = unsafe {
        libc::open(raw.as_ptr(), libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_CLOEXEC, 0o644)
    };
    if fd < 0 {
        return;
    }
    let o = Out(fd);
    o.bytes(&setup.header);
    o.bytes(b"raw crash report\nsignal: ");
    o.dec(sig as i64);
    o.bytes(b"\n");
    // SAFETY: the kernel's siginfo for this signal
    let (code, addr) = unsafe { ((*info).si_code, (*info).si_addr() as usize) };
    o.bytes(b"code: ");
    o.dec(code as i64);
    o.bytes(b"\naddress: ");
    o.hex(addr);
    o.bytes(b"\npid: ");
    // SAFETY: plain system calls
    o.dec(unsafe { libc::getpid() } as i64);
    let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
    unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts) };
    o.bytes(b"\nunix: ");
    o.dec(ts.tv_sec);
    let mut name = [0u8; 17];
    // SAFETY: PR_GET_NAME writes at most 16 bytes
    unsafe { libc::prctl(libc::PR_GET_NAME, name.as_mut_ptr()) };
    let len = name.iter().position(|&c| c == 0).unwrap_or(16);
    o.bytes(b"\nthread: ");
    o.bytes(&name[..len]);
    o.bytes(b"\ntid: ");
    o.dec(tid as i64);
    let (pc, sp) = pc_sp(uc);
    o.bytes(b"\npc: ");
    o.hex(pc);
    o.bytes(b"\nsp: ");
    o.hex(sp);
    o.bytes(b"\n");

    // the mappings first: the scan needs the code ranges
    // SAFETY: the handler's owner
    let maps = unsafe { MAPS.get() };
    let maps_len = read_file(b"/proc/self/maps\0", maps);
    let ranges = exec_lines(&maps[..maps_len], None);

    // the unwinder walks from here, through the kernel's signal frame, into the faulting code; the
    // frames before the faulting pc are this handler's
    let mut walk = Walk { n: 0 };
    unsafe { _Unwind_Backtrace(take_frame, (&raw mut walk).cast()) };
    // SAFETY: the handler's owner
    let frames = unsafe { &FRAMES.get()[..walk.n] };
    let from = frames.iter().position(|&f| f == pc);
    o.bytes(b"--- stack ---\n");
    o.hex(pc);
    o.bytes(b"\n");
    let after = match from {
        Some(i) => &frames[i + 1..],
        None => &[][..],
    };
    for &f in after {
        o.hex(f);
        o.bytes(b"\n");
    }
    // a fault in code without unwind tables (a JIT, a stripped driver) stops the unwinder at once:
    // the stack's code addresses still say who called
    if after.len() < 3 && sp != 0 {
        o.bytes(b"--- scan ---\n");
        scan_stack(&o, sp, ranges);
    }
    o.bytes(b"--- threads ---\n");
    thread_names(&o);
    o.bytes(b"--- maps ---\n");
    exec_lines(&maps[..maps_len], Some(&o));
    o.bytes(b"--- end ---\n");
    // SAFETY: our descriptor
    unsafe { libc::close(fd) };

    if !setup.exec.is_empty() {
        symbolize_in_child(setup);
    }
}

/// Start the symbolizer as a fresh process and wait for it. A raw `clone` with fork semantics: the
/// libc `fork` runs atfork handlers and takes locks, which a crashed thread may hold.
fn symbolize_in_child(setup: &Setup) {
    // SAFETY: the child calls only execve and _exit; the parent waits
    unsafe {
        let pid = libc::syscall(libc::SYS_clone, libc::SIGCHLD as libc::c_ulong, 0usize, 0usize, 0usize, 0usize);
        if pid == 0 {
            unsafe extern "C" {
                static environ: *const *const libc::c_char;
            }
            libc::execve(setup.exec[0], setup.exec.as_ptr(), environ);
            libc::_exit(127);
        }
        if pid > 0 {
            let mut status = 0;
            libc::wait4(pid as libc::pid_t, &mut status, 0, std::ptr::null_mut());
        }
    }
}

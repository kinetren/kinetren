//! Blocking calls on the threads that must not wait (one-scheduler, `docs/engine-audit.md` M3).
//!
//! The UI thread and the audio render thread mark themselves ([`mark`]); a call that can stand for
//! as long as the disk or a build likes — a file open, an fsync, a thread join, a renderer build, a
//! sidecar open — opens a [`wait`] guard. On an unmarked thread the guard costs a thread-local read.
//! On a marked one it is counted (process-wide per role, and per thread for a test) and recorded
//! in the trace with its length (`Ev::Block`). The rule is that those threads post jobs and read
//! answers; t2236 reads the count over the person's gestures and requires zero.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::trace::{self, Ev};

/// The threads that must not wait.
pub mod role {
    pub const NONE: u8 = 0;
    pub const UI: u8 = 1;
    pub const RENDER: u8 = 2;
}

/// What the call waited for (`Block.aux`).
pub mod kind {
    /// A media file or the project opened (FFmpeg probe, a packet index).
    pub const FILE_OPEN: u32 = 1;
    /// An fsync of a written file.
    pub const FSYNC: u32 = 2;
    /// A thread joined (a decoder's owner).
    pub const JOIN: u32 = 3;
    /// A renderer or an engine built.
    pub const BUILD: u32 = 4;
    /// A sidecar database opened.
    pub const SIDECAR: u32 = 5;
    /// A project file written.
    pub const WRITE: u32 = 6;
    pub const COUNT: usize = 7;

    pub const fn name(k: u32) -> &'static str {
        match k {
            FILE_OPEN => "file_open",
            FSYNC => "fsync",
            JOIN => "join",
            BUILD => "build",
            SIDECAR => "sidecar",
            WRITE => "write",
            _ => "other",
        }
    }
}

thread_local! {
    static ROLE: Cell<u8> = const { Cell::new(role::NONE) };
    static HERE: Cell<[u64; kind::COUNT]> = const { Cell::new([0; kind::COUNT]) };
}

static TOTAL: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

/// This thread is one that must not wait.
pub fn mark(r: u8) {
    ROLE.with(|c| c.set(r));
}

/// This thread's role.
pub fn role_here() -> u8 {
    ROLE.with(|c| c.get())
}

/// A blocking call is about to run: hold the guard across it.
#[must_use]
pub fn wait(k: u32) -> Wait {
    let r = role_here();
    if r == role::NONE {
        return Wait(None);
    }
    TOTAL[r as usize].fetch_add(1, Ordering::Relaxed);
    HERE.with(|h| {
        let mut v = h.get();
        v[(k as usize).min(kind::COUNT - 1)] += 1;
        h.set(v);
    });
    Wait(Some((k, r, Instant::now())))
}

/// A blocking call's guard; records its length when dropped.
pub struct Wait(Option<(u32, u8, Instant)>);

impl Drop for Wait {
    fn drop(&mut self) {
        if let Some((k, r, t0)) = self.0 {
            let ns = t0.elapsed().as_nanos() as u64;
            trace::ev(Ev::Block).aux(k).a(r as u64).lasted(ns).emit();
        }
    }
}

/// Blocking calls made on THIS thread so far, by [`kind`].
pub fn here() -> [u64; kind::COUNT] {
    HERE.with(|h| h.get())
}

/// Their sum.
pub fn here_total() -> u64 {
    here().iter().sum()
}

/// Blocking calls made on threads of this role, process-wide.
pub fn total(r: u8) -> u64 {
    TOTAL[(r as usize).min(2)].load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t2235: an unmarked thread counts nothing; a marked one counts each call by kind, on its
    /// own thread only.
    #[test]
    fn t2235_blocking_calls_are_counted_on_marked_threads_only() {
        {
            let _w = wait(kind::FILE_OPEN);
        }
        assert_eq!(here_total(), 0, "an unmarked thread counted a call");
        mark(role::UI);
        let before = total(role::UI);
        {
            let _a = wait(kind::FSYNC);
            let _b = wait(kind::JOIN);
        }
        let h = here();
        assert_eq!((h[kind::FSYNC as usize], h[kind::JOIN as usize]), (1, 1));
        assert!(total(role::UI) >= before + 2);
        let other = std::thread::spawn(|| {
            let _w = wait(kind::BUILD);
            here_total()
        })
        .join()
        .unwrap();
        assert_eq!(other, 0, "the role leaked to another thread");
        mark(role::NONE);
    }
}

//! The process ends before the native libraries' destructors (found-defects 784/785). t3200.

use std::path::{Path, PathBuf};

const CHILD: &str = "KT_T3200_CHILD";

fn dir() -> PathBuf {
    PathBuf::from(std::env::var(CHILD).unwrap())
}

/// A library's destructor: registered by `exit`'s rules as a library loaded before the end is.
extern "C" fn library_destructor() {
    let _ = std::fs::write(dir().join("library"), b"ran");
}

fn last_step() {
    let _ = std::fs::write(dir().join("step"), b"ran");
}

unsafe extern "C" {
    fn atexit(cb: extern "C" fn()) -> std::ffi::c_int;
}

/// The child: a library loaded, the end registered after it (or not, the control), a thread still
/// working in it, then `exit(7)`.
fn child(armed: bool) -> ! {
    // SAFETY: a plain C function registered once
    unsafe { atexit(library_destructor) };
    kt_time::shutdown::before_exit(last_step);
    if armed {
        kt_time::shutdown::end_before_libraries();
        assert!(kt_time::shutdown::end_registrations() >= 1);
    }
    // a thread a bounded close left behind, still at work when the process ends
    std::thread::spawn(|| {
        loop {
            std::hint::black_box(0u64);
        }
    });
    std::process::exit(7)
}

fn run(armed: bool, dir: &Path) -> std::process::ExitStatus {
    std::process::Command::new(std::env::current_exe().unwrap())
        .args(["t3200_the_process_ends_before_the_libraries_destructors", "--exact", "--nocapture"])
        .env(CHILD, dir)
        .env("KT_T3200_ARMED", if armed { "1" } else { "0" })
        .status()
        .unwrap()
}

/// Test t3200 (found-defects 784/785): a test process died natively after its test passed —
/// `exit` ran the destructors of libraries loaded before it (libLLVM under llvmpipe, a driver's C++
/// statics) while threads a bounded close left behind still drew in them. Once
/// `end_before_libraries` is registered after a library loads, `exit` ends the process before that
/// library's destructors: the program's last steps run, the status `exit` was given is kept, the
/// library's destructor never runs, with a thread still at work. The control without the
/// registration shows the destructor would run.
#[test]
fn t3200_the_process_ends_before_the_libraries_destructors() {
    if std::env::var(CHILD).is_ok() {
        child(std::env::var("KT_T3200_ARMED").as_deref() == Ok("1"));
    }
    if !cfg!(target_os = "linux") {
        return; // glibc's `on_exit`; elsewhere the end is the C `exit` as before
    }
    for armed in [true, false] {
        let d = std::env::temp_dir().join(format!("kt-t3200-{}-{armed}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let status = run(armed, &d);
        assert_eq!(status.code(), Some(7), "the status exit was given is kept (armed {armed})");
        let library = d.join("library").exists();
        let step = d.join("step").exists();
        let _ = std::fs::remove_dir_all(&d);
        if armed {
            assert!(step, "the program's last step ran");
            assert!(!library, "the library's destructor ran: the end did not stand before it");
        } else {
            assert!(library, "control: without the end the library's destructor runs");
            assert!(!step, "control: no end, no last step");
        }
    }
}

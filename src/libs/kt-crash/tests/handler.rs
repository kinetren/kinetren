//! The handler in a real crash. Every crash happens in a CHILD process — this test binary started
//! again for one test — never in the test process.
#![cfg(target_os = "linux")]

use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CHILD: &str = "KT_CRASH_CHILD";
const RAW: &str = "KT_CRASH_RAW";

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kt-crash-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Run `child_body` in a child with `mode`; its raw report goes to `raw`. Returns the signal it died by.
fn crash_child(mode: &str, raw: &Path) -> Option<i32> {
    let st = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_body", "--nocapture", "--test-threads=1"])
        .env(CHILD, mode)
        .env(RAW, raw)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    st.signal()
}

/// The child's body: does something only when a test below started it.
#[test]
fn child_body() {
    let Ok(mode) = std::env::var(CHILD) else { return };
    let raw = PathBuf::from(std::env::var_os(RAW).unwrap());
    assert!(kt_crash::install(&kt_crash::Config {
        raw,
        header: "kt-crash test child".into(),
        symbolizer: None,
    }));
    // a few named threads that live through the crash
    let _keep: Vec<_> = (0..3)
        .map(|i| {
            std::thread::Builder::new()
                .name(format!("kt-bystander-{i}"))
                .spawn(|| std::thread::sleep(std::time::Duration::from_secs(60)))
                .unwrap()
        })
        .collect();
    let probe = std::thread::Builder::new().name("kt-crash-probe".into());
    let h = probe
        .spawn(move || match mode.as_str() {
            "segv" => kt_crash::testing::kt_crash_probe_segv(),
            "abort" => kt_crash::testing::kt_crash_probe_abort(),
            _ => {}
        })
        .unwrap();
    let _ = h.join();
}

/// Test 2080 (segv-crashes): a SIGSEGV in a named thread dies by SIGSEGV and leaves a report naming
/// the thread, the fault, the faulting function with its file and line, and the other threads.
#[test]
fn t2080_segv_report_names_thread_stack_and_threads() {
    let dir = scratch("t2080");
    let raw = dir.join("crash.raw");
    assert_eq!(crash_child("segv", &raw), Some(11), "the child must still die by its own signal");
    let text = std::fs::read_to_string(&raw).expect("the handler wrote no report");
    assert!(text.starts_with("kt-crash test child\nraw crash report\nsignal: 11\n"), "{text}");
    assert!(text.contains("\nthread: kt-crash-probe\n"), "{text}");
    assert!(text.contains("\naddress: 0x0000000000000000\n"), "{text}");
    let report = kt_crash::symbolize(&text);
    assert!(report.contains("SIGSEGV (segmentation fault), code 1 (address not mapped), address 0x0"), "{report}");
    assert!(report.contains("thread: kt-crash-probe"), "{report}");
    // the faulting frame is the probe, named with its file and line from the line tables
    let frame0: String = report
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("0  "))
        .take_while(|l| !l.trim_start().starts_with("1  "))
        .collect();
    assert!(frame0.contains("kt_crash::testing::kt_crash_probe_segv (libs/kt-crash/src/lib.rs:"), "{report}");
    // its caller, this test's closure, is further down the unwound stack
    assert!(report.contains("handler::child_body"), "{report}");
    for name in ["kt-bystander-0", "kt-bystander-2", "kt-crash-probe  ← faulted"] {
        assert!(report.contains(name), "the threads miss {name}: {report}");
    }
    let s = kt_crash::summary(&text).unwrap();
    assert!(s.starts_with("SIGSEGV in thread \"kt-crash-probe\" at kt_crash::testing::kt_crash_probe_segv"), "{s}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 2081 (segv-crashes): an abort (what a panic inside a panic is) dies by SIGABRT and names the
/// thread and the code that aborted, through the C library's frames.
#[test]
fn t2081_abort_report_names_the_caller() {
    let dir = scratch("t2081");
    let raw = dir.join("crash.raw");
    assert_eq!(crash_child("abort", &raw), Some(6));
    let text = std::fs::read_to_string(&raw).expect("the handler wrote no report");
    let report = kt_crash::symbolize(&text);
    assert!(report.contains("SIGABRT (abort)") && report.contains("sent by the process itself"), "{report}");
    assert!(report.contains("thread: kt-crash-probe"), "{report}");
    assert!(report.contains("kt_crash::testing::kt_crash_probe_abort"), "{report}");
    let s = kt_crash::summary(&text).unwrap();
    assert!(s.contains("called from kt_crash::testing::kt_crash_probe_abort") || s.contains("at kt_crash::testing::kt_crash_probe_abort"), "{s}");
    let _ = std::fs::remove_dir_all(&dir);
}

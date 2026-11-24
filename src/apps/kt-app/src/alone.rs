//! One process, many tests (one-process-tests). `cargo test` runs the crate's tests as threads of one
//! process; nextest gives each its own. A process has one scheduler, one set of work counters, one
//! compute host, one poster service and one list of cache notices, and the program is their one user,
//! so a test that reads, drains or installs them meets every other test's. Such a test begins with
//! [`own_process!`]: outside nextest it runs again in a child process, alone, and the thread here only
//! brings its verdict.

/// The first line of a test that needs a process of its own.
macro_rules! own_process {
    () => {
        fn here() {}
        if crate::alone::ran_in_a_child(std::any::type_name_of_val(&here)) {
            return;
        }
    };
}
pub(crate) use own_process;

const CHILD: &str = "KT_TEST_CHILD";

/// `true` — the test named by `here` (`<crate>::<test>::here`) ran in a child process and passed; `false`
/// — this process is its own (nextest's, or the child's): run the body.
pub fn ran_in_a_child(here: &str) -> bool {
    if std::env::var_os(CHILD).is_some() {
        end_with_the_parent();
        return false;
    }
    if std::env::var_os("NEXTEST").is_some() {
        return false;
    }
    let name = here.strip_suffix("::here").unwrap_or(here);
    let name = name.split_once("::").map_or(name, |(_, n)| n);
    let out = std::env::temp_dir().join(format!("kt-own-process-{}-{}.log", std::process::id(), name.replace("::", "-")));
    let log = std::fs::File::create(&out).expect("the child's log");
    let mut child = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args(["--exact", name, "--nocapture"])
        .env(CHILD, "1")
        .stdout(log.try_clone().expect("the child's log"))
        .stderr(log)
        .spawn()
        .expect("the child runs");
    // a hang of the child is the child's test's own deadline; past it and as much again it is killed
    let deadline = kt_time::test_deadline() + kt_time::TEST_WAIT;
    let status = loop {
        match child.try_wait().expect("the child") {
            Some(s) => break Some(s),
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => std::thread::sleep(std::time::Duration::from_millis(20)),
        }
    };
    let said = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    assert!(status.is_some_and(|s| s.success()), "{name} in a process of its own: {status:?}\n{said}");
    assert!(said.contains("1 passed"), "{name}: the child ran another number of tests\n{said}");
    true
}

/// A run that is killed leaves no child behind.
fn end_with_the_parent() {
    #[cfg(unix)]
    std::thread::spawn(|| {
        let parent = std::os::unix::process::parent_id();
        while std::os::unix::process::parent_id() == parent {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        std::process::exit(3);
    });
}

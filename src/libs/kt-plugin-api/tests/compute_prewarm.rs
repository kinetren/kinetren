//! Test t2124 (pipeline-prewarm D6): `compute::prewarm` returns at once while the device is still
//! opening (it stands until the test lets it go), then prepares each shader on its own thread once: a
//! second prewarm of the same shaders, and of a shader asked before beside a new one, asks only the
//! new one. Its own test binary: the device is one per process.

use kt_plugin_api::compute::{self, ComputeHost, Job, Shader};
use std::sync::{Arc, Condvar, Mutex};

static GATE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());
static PREPARED: (Mutex<Vec<String>>, Condvar) = (Mutex::new(Vec::new()), Condvar::new());

struct Host;

impl ComputeHost for Host {
    fn run(&self, _job: &Job<'_>) -> Result<Vec<Vec<u8>>, String> {
        Ok(Vec::new())
    }
    fn prepare(&self, s: &Shader) -> Result<(), String> {
        PREPARED.0.lock().unwrap().push(s.text.to_string());
        PREPARED.1.notify_all();
        Ok(())
    }
}

fn slow_open() -> Result<Arc<dyn ComputeHost>, String> {
    let mut open = GATE.0.lock().unwrap();
    while !*open {
        open = GATE.1.wait(open).unwrap();
    }
    Ok(Arc::new(Host))
}

fn shader(text: &'static str) -> Shader {
    Shader { text: text.into(), reads: 1, writes: 1 }
}

fn wait_prepared(n: usize) -> Vec<String> {
    let deadline = kt_time::test_deadline();
    let mut got = PREPARED.0.lock().unwrap();
    while got.len() < n {
        assert!(std::time::Instant::now() < deadline, "prepared {:?}", *got);
        got = PREPARED.1.wait_timeout(got, std::time::Duration::from_millis(50)).unwrap().0;
    }
    got.clone()
}

#[test]
fn t2124_prewarm_returns_at_once_and_prepares_each_shader_once() {
    compute::install(slow_open);
    // the device is still opening: the call returns without waiting for it
    compute::prewarm(vec![shader("a"), shader("b")]);
    compute::prewarm(vec![shader("a"), shader("b")]);
    assert!(PREPARED.0.lock().unwrap().is_empty());
    *GATE.0.lock().unwrap() = true;
    GATE.1.notify_all();
    let mut got = wait_prepared(2);
    got.sort();
    assert_eq!(got, ["a", "b"]);
    compute::prewarm(vec![shader("b"), shader("c")]);
    // the thread of this call prepares its list in order: a "b" asked again would come before "c"
    let got = wait_prepared(3);
    assert_eq!(&got[2], "c", "a shader asked before was prepared again: {got:?}");
}

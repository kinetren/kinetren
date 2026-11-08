//! Test t1847 (ui-never-waits): the interface asks whether the compute device works without opening
//! it on its own thread. Opening a device takes from a fraction of a second to seconds; the effects
//! window asks every frame a Datamosh slot is shown. The open below stands until the test lets it
//! go, so a question that waited for it could never return.
//!
//! Its own test binary: the device is one per process.

use kt_plugin_api::compute::{self, ComputeHost, Job};
use std::sync::{Arc, Condvar, Mutex};

static GATE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

struct Host;

impl ComputeHost for Host {
    fn run(&self, _job: &Job<'_>) -> Result<Vec<Vec<u8>>, String> {
        Ok(Vec::new())
    }
}

fn slow_open() -> Result<Arc<dyn ComputeHost>, String> {
    let mut open = GATE.0.lock().unwrap();
    while !*open {
        open = GATE.1.wait(open).unwrap();
    }
    Ok(Arc::new(Host))
}

#[test]
fn t1847_asking_about_the_compute_device_never_opens_it_here() {
    compute::install(slow_open);
    for _ in 0..3 {
        assert_eq!(compute::unavailable_now(), None, "nothing is known before the device opens");
    }
    *GATE.0.lock().unwrap() = true;
    GATE.1.notify_all();
    // host() waits for the open the question started on its thread
    assert!(compute::host().is_some(), "the device never opened");
    assert_eq!(compute::unavailable_now(), None, "the opened device works");
}

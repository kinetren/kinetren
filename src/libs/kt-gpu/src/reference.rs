//! The frames the tests hold the GPU to (compute-kernels-render, D5): the last output of the deleted
//! CPU composite, one file per test in `references/`, frames in the order the test asks for them.
//! A test thread reads its own file by its name, so a test runs under the harness's threads.

use std::cell::RefCell;
use std::io::Write as _;
use std::path::PathBuf;

fn file() -> PathBuf {
    let name = std::thread::current().name().unwrap_or("").rsplit("::").next().unwrap_or("").to_string();
    assert!(!name.is_empty() && name != "main", "a reference frame belongs to a test thread, got {name:?}");
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("references").join(format!("{name}.bin"))
}

/// Append a frame to this test's file.
pub fn keep(rgba: &[u8], complete: bool) {
    let path = file();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
    f.write_all(&[complete as u8]).unwrap();
    f.write_all(&(rgba.len() as u32).to_le_bytes()).unwrap();
    f.write_all(rgba).unwrap();
}

thread_local! {
    static AT: RefCell<(Vec<u8>, usize)> = const { RefCell::new((Vec::new(), 0)) };
}

/// This test's next reference frame and whether the composite was complete.
pub fn take() -> (Vec<u8>, bool) {
    AT.with(|at| {
        let (bytes, pos) = &mut *at.borrow_mut();
        if bytes.is_empty() {
            *bytes = std::fs::read(file()).expect("the test's reference file");
        }
        assert!(*pos < bytes.len(), "the test asked for more frames than its reference file holds");
        let complete = bytes[*pos] != 0;
        let len = u32::from_le_bytes(bytes[*pos + 1..*pos + 5].try_into().unwrap()) as usize;
        let rgba = bytes[*pos + 5..*pos + 5 + len].to_vec();
        *pos += 5 + len;
        (rgba, complete)
    })
}

/// `take` into a buffer, giving the completeness.
pub fn next(out: &mut Vec<u8>) -> bool {
    let (rgba, complete) = take();
    *out = rgba;
    complete
}

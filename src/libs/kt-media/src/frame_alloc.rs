//! Frame-sized buffers go back to the system when freed (export-throughput D11).
//!
//! glibc serves a large allocation from mmap only below its DYNAMIC threshold, which rises (up to
//! 32 MB) every time such a block is freed; from then on decoded frames (8–33 MB) come from the
//! per-thread arenas and, freed on another thread, stay there. The pipelined export — decode,
//! compose and convert on different threads — peaked at 16.2 GB on the showcase, of which 9 GB
//! were freed frames held by arenas; a fixed 1 MiB threshold gave 7.2 GB, the same frames.
//! Small allocations (the UI's, the audio's) are untouched.

/// The threshold above which an allocation is its own mapping.
pub const MMAP_THRESHOLD: i32 = 1 << 20;

/// Fix the threshold for the process; called first thing by the binaries (kt-app, kt-cli).
/// Returns whether the allocator took it (always `false` off glibc, where there is nothing to do).
pub fn return_large_frames_to_os() -> bool {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        unsafe extern "C" {
            fn mallopt(param: core::ffi::c_int, value: core::ffi::c_int) -> core::ffi::c_int;
        }
        // M_MMAP_THRESHOLD; setting it also switches the dynamic adjustment off
        const M_MMAP_THRESHOLD: core::ffi::c_int = -3;
        // SAFETY: mallopt only changes a malloc tunable; any value is accepted or refused
        unsafe { mallopt(M_MMAP_THRESHOLD, MMAP_THRESHOLD) == 1 }
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    /// t1458: the allocator takes the threshold on glibc, and both binaries set it before anything
    /// else (a source guard: the call is the first statement of their entry points).
    #[test]
    fn t1458_frame_buffers_return_to_the_system() {
        let took = super::return_large_frames_to_os();
        assert_eq!(took, cfg!(all(target_os = "linux", target_env = "gnu")));
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps");
        for (file, entry) in [("kt-cli/src/main.rs", "fn main() {"), ("kt-app/src/main.rs", "fn main() -> eframe::Result {")] {
            let src = std::fs::read_to_string(root.join(file)).unwrap();
            let body = src.split_once(entry).unwrap_or_else(|| panic!("{file}: no `{entry}`")).1;
            let first = body.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("//")).unwrap();
            assert_eq!(first, "kt_media::frame_alloc::return_large_frames_to_os();", "{file}: the first statement");
        }
    }
}

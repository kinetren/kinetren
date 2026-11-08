//! Test 2088 (segv-crashes, the live `render` scenario): on the GL backend the export opens its
//! engines while another device of the process is alive — what a render from the editor does under
//! `WGPU_BACKEND=gl` (the live scenarios' setting, and a machine without Vulkan). wgpu's GL adapters
//! of ONE instance share that instance's one EGL context; when every device came from the process's
//! one instance, the offline engine and the export's engines made the same context current on
//! several threads, and every render died in `GpuFxEngine::build` ("Could not lock adapter
//! context"), then aborted dropping the queue during the unwind. A GL device gets an instance of its
//! own; Vulkan and the rest keep the shared one (t2087).
//!
//! Alone in its file: the backend is read from the environment when the first instance is made.

#[test]
fn t2088_gl_export_engines_open_beside_a_live_device() {
    // SAFETY: the only test of this binary, before any thread or instance exists.
    unsafe {
        std::env::set_var("WGPU_BACKEND", "gl");
        std::env::set_var("KT_VIDEO_ENGINES", "4");
    }
    let Some(alive) = kt_gpu::offline_engine() else {
        eprintln!("skipped: no GL adapter");
        return;
    };
    use kt_engine::VideoGpuExec as _;
    assert!(!alive.info().name.is_empty());
    for round in 0..2 {
        let engines = kt_gpu::export_engines(320, 180).unwrap_or_else(|e| panic!("round {round}: {e}"));
        assert_eq!(engines.len(), 4, "round {round}: KT_VIDEO_ENGINES=4 opens four engines");
    }
    // the device that was alive all along still opens nothing new and still answers
    assert!(kt_gpu::offline_engine().is_some());
}

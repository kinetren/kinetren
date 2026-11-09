//! t2129 (pipeline-prewarm D10; capability `video-plan`): a process that exits while the warm worker
//! and the compute host's preparation are compiling on a software rasterizer exits cleanly. `exit`
//! runs libLLVM's destructors under llvmpipe's threads; two kt-app tests died by SIGSEGV after "test
//! result: ok" in a loaded full run (row 172), because their process ended while a warm-up compiled.
//! The child below is this binary run again: it starts the warm-up of every effect and every compute
//! shader on an empty Mesa cache (each compile takes up to seconds), waits until the work is on the
//! device and calls `exit` at once. Without an adapter: skip with a printout.

use std::process::Command;

const CHILD: &str = "KT_T2129_CHILD";

fn child() -> ! {
    kt_plugins_builtin::install();
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let engine = kt_gpu::GpuContext::new_headless().and_then(kt_gpu::GpuFxEngine::new).expect("engine");
    let reg = kt_plugin_api::installed().unwrap();
    let ids: Vec<String> = reg.plugins().map(|p| p.manifest().id.to_string()).collect();
    use kt_engine::VideoGpuExec as _;
    engine.prewarm(&ids);
    kt_plugin_api::compute::prewarm(reg.plugins().flat_map(|p| p.compute_shaders()).collect());
    let deadline = kt_time::test_deadline();
    while kt_gpu::background_gpu_work() < 2 {
        assert!(std::time::Instant::now() < deadline, "the warm-up never reached the device");
        std::thread::yield_now();
    }
    eprintln!("t2129 child: {} background GPU works in flight, exiting", kt_gpu::background_gpu_work());
    std::process::exit(0)
}

#[test]
fn t2129_exit_while_a_warm_up_compiles_is_clean() {
    if std::env::var_os(CHILD).is_some() {
        child();
    }
    if kt_gpu::GpuContext::new_headless().is_err() {
        eprintln!("skipped: no wgpu adapter");
        return;
    }
    let cache = std::env::temp_dir().join(format!("kt-t2129-mesa-{}", std::process::id()));
    for run in 0..3 {
        let _ = std::fs::remove_dir_all(&cache);
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "t2129_exit_while_a_warm_up_compiles_is_clean", "--nocapture"])
            .env(CHILD, "1")
            .env("MESA_SHADER_CACHE_DIR", &cache)
            .output()
            .expect("the child runs");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "run {run}: the child ended by {:?}\n{err}", out.status);
        assert!(err.contains("t2129 child:"), "run {run}: the child never reached its exit\n{err}");
    }
    let _ = std::fs::remove_dir_all(&cache);
}

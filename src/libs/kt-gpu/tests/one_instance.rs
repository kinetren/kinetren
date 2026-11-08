//! Test 2087 (segv-crashes, found-defects 131): devices opened at once from several threads — what
//! `export_engines` does on a software adapter — share the process's one instance and do not race the
//! Vulkan loader. Before, each opened its own instance at the same moment and the process died in
//! `vkEnumerateInstanceExtensionProperties` (every `kt-cli export` without DISPLAY on llvmpipe).

#[test]
fn t2087_devices_opened_at_once_share_one_instance() {
    if kt_gpu::GpuContext::new_headless().is_err() {
        eprintln!("skipped: no adapter");
        return;
    }
    for _ in 0..3 {
        let opened: Vec<_> = std::thread::scope(|s| {
            let open: Vec<_> = (0..4).map(|_| s.spawn(kt_gpu::GpuContext::new_headless_for_export)).collect();
            open.into_iter().map(|h| h.join().expect("an opening thread")).collect()
        });
        assert!(opened.iter().all(|c| c.is_ok()), "a device failed to open");
    }
    let _ = kt_gpu::adapter_facts();
    let _ = kt_gpu::adapter_facts();
    assert_eq!(kt_gpu::instances_created(), 1, "every opening and listing must share the one instance");
}

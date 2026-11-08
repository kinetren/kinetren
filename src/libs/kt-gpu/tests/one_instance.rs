//! Test 2087 (segv-crashes, found-defects 131): devices opened at once from several threads — what
//! `export_engines` does on a software adapter — share the process's one instance and do not race the
//! Vulkan loader. Before, each opened its own instance at the same moment and the process died in
//! `vkEnumerateInstanceExtensionProperties` (every `kt-cli export` without DISPLAY on llvmpipe), and
//! t985 died the same way when a generator's first shader frame opened the compute host on a media
//! worker while the poster's offline engine opened (row 132; 33 of 60 runs on the old code).

#[test]
fn t2087_devices_opened_at_once_share_one_instance() {
    if kt_gpu::GpuContext::new_headless().is_err() {
        eprintln!("skipped: no adapter");
        return;
    }
    for _ in 0..3 {
        let opened: Vec<_> = std::thread::scope(|s| {
            let open: Vec<_> = (0..4).map(|_| s.spawn(kt_gpu::GpuContext::new_headless_for_export)).collect();
            // the compute host and the adapter list at the same moment (t985's path)
            let compute = s.spawn(kt_gpu::compute::open_device);
            let facts = s.spawn(kt_gpu::adapter_facts);
            let _ = facts.join().expect("the listing thread");
            let _ = compute.join().expect("the compute thread");
            open.into_iter().map(|h| h.join().expect("an opening thread")).collect()
        });
        assert!(opened.iter().all(|c| c.is_ok()), "a device failed to open");
    }
    // one for the headless devices and the list, one for the compute host's primary backends
    assert!(kt_gpu::instances_created() <= 2, "every opening must share its backend set's instance: {}", kt_gpu::instances_created());
}

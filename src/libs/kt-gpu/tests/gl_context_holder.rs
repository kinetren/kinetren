//! gl-context-holder (kkt-26.11.15 N1 class, found-defects 651): on GL every call into a device takes
//! its adapter's ONE context lock, and egui's paint panics after six seconds without it. The preview
//! therefore composes on a GL device of its own, never on the window's. These hold the premise: two
//! GL devices of the process have two locks. Skipped without a GL adapter.

/// t2741: while one GL device's context lock is held (a call of ours stalled by the system), another
/// GL device of the process creates, writes and submits, and its waits finish — on the old shared
/// device the second caller would have waited for the lock, and the UI's paint panicked at six seconds.
#[test]
fn t2741_a_stalled_gl_device_holds_no_other_device() {
    // SAFETY: the only test of this binary, before any instance exists
    unsafe { std::env::set_var("WGPU_BACKEND", "gl") };
    let (Ok(window), Ok(preview)) = (kt_gpu::GpuContext::new_headless(), kt_gpu::GpuContext::new_headless()) else {
        eprintln!("skipped: no GL adapter");
        return;
    };
    if preview.adapter_info.backend != wgpu::Backend::Gl {
        eprintln!("skipped: not a GL adapter");
        return;
    }
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let (held_tx, held_rx) = std::sync::mpsc::channel::<()>();
    let device = preview.device.clone();
    let holder = std::thread::spawn(move || {
        // held until the other device's work is done: if that work needed this lock, the test would
        // end in wgpu-hal's six-second panic instead
        kt_gpu::holds::hold_gl_context(&device, || {
            held_tx.send(()).unwrap();
            let _ = done_rx.recv();
        })
    });
    held_rx.recv().unwrap();
    let buf = window.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("t2741"),
        size: 4096,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    window.queue.write_buffer(&buf, 0, &[7u8; 4096]);
    let sub = window.queue.submit([window.device.create_command_encoder(&Default::default()).finish()]);
    window.device.poll(wgpu::PollType::Wait { submission_index: Some(sub), timeout: None }).expect("the window's device served");
    done_tx.send(()).unwrap();
    assert!(holder.join().unwrap(), "the preview's device is a GL device: its lock was held");
}

/// t2742: the window's device is the UI thread's alone exactly on GL.
#[test]
fn t2742_only_a_gl_window_device_is_left_to_the_ui() {
    let info = |backend| wgpu::AdapterInfo {
        name: "x".into(),
        vendor: 0,
        device: 0,
        device_type: wgpu::DeviceType::Cpu,
        device_pci_bus_id: String::new(),
        driver: String::new(),
        driver_info: String::new(),
        backend,
        subgroup_min_size: 4,
        subgroup_max_size: 64,
        transient_saves_memory: None,
        limit_bucket: None,
    };
    assert!(kt_gpu::display_is_ui_only(&info(wgpu::Backend::Gl)));
    for b in [wgpu::Backend::Vulkan, wgpu::Backend::Dx12, wgpu::Backend::Metal] {
        assert!(!kt_gpu::display_is_ui_only(&info(b)), "{b:?}");
    }
}

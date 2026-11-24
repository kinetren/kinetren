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

/// t2745 (D4): on one GL engine a call of ours stalled under the context lock for longer than
/// wgpu-hal's six seconds makes a frame on another thread WAIT, not panic: our calls into a GL device
/// queue at its gate first. Without the gate the frame's thread panicked "Could not lock adapter
/// context" and the preview's thread died with it (gl_stall's first runs).
#[test]
fn t2745_a_frame_waits_out_a_stalled_call_on_its_gl_device() {
    // SAFETY: before any instance of this test's process (the binary's tests run as processes)
    unsafe { std::env::set_var("WGPU_BACKEND", "gl") };
    kt_plugins_builtin::install();
    let Some(base) = kt_gpu::preview_engine_off_display() else {
        eprintln!("skipped: no GL adapter");
        return;
    };
    use kt_engine::VideoGpuExec as _;
    if base.info().name.is_empty() {
        return;
    }
    let engine = base.clone();
    let (held_tx, held_rx) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        engine.hold_gl_context(|| {
            held_tx.send(()).unwrap();
            // past wgpu-hal's CONTEXT_LOCK_TIMEOUT_SECS (6): the stall IS the check
            std::thread::sleep(std::time::Duration::from_secs(7));
        })
    });
    held_rx.recv().unwrap();
    let mut out = Vec::new();
    // a frame with a layer: an upload, passes and a readback on the device. It waits at the gate for
    // the holder, then composes
    let t0 = std::time::Instant::now();
    assert!(base.execute_into(&layer_plan(16, 8), &mut out).is_ok(), "the frame composed after the stall");
    assert_eq!(out.len(), 16 * 8 * 4);
    println!("t2745: the frame waited {:?}", t0.elapsed());
    if !holder.join().unwrap() {
        eprintln!("skipped: the preview's device is not GL");
    }
    // the same frame with nothing held: the same bytes
    let mut again = Vec::new();
    assert!(base.execute_into(&layer_plan(16, 8), &mut again).is_ok());
    assert_eq!(out, again, "the frame that waited is the frame");
}

fn layer_plan(w: u32, h: u32) -> kt_engine::FramePlan<'static> {
    let fr = std::sync::Arc::new(kt_media::VideoFrame {
        pts: kt_time::Tick(0),
        width: w,
        height: h,
        rgba: vec![200u8; (w * h * 4) as usize].into_boxed_slice(),
        has_alpha: false,
    });
    let src = kt_engine::LayerSrc {
        source: kt_cache::SourceId(1),
        cache_idx: 0,
        content: 1,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(fr)),
        fx: kt_engine::LayerChain::Owned(Vec::new()),
        fx_hash: 1,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
        fx_in: &[],
        reads: Vec::new(),
    };
    kt_engine::FramePlan {
        t: kt_time::Tick(90_000),
        out_w: w,
        out_h: h,
        layers: vec![kt_engine::PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }],
    }
}

/// t2746 (D5): the preview off the window's device keeps what it composed, as the display path does:
/// the same moment asked again is read back from the kept composite with nothing composed (a replay),
/// and its bytes are the frame's. Before, the bytes path had only the fixed 64 MB readback memo and a
/// covered loop composed again on GL (live cached_range red under the new route).
#[test]
fn t2746_a_kept_moment_is_read_back_not_composed() {
    // SAFETY: before any instance of this test's process
    unsafe { std::env::set_var("WGPU_BACKEND", "gl") };
    kt_plugins_builtin::install();
    let Some(engine) = kt_gpu::preview_engine_off_display() else {
        eprintln!("skipped: no GL adapter");
        return;
    };
    use kt_engine::VideoGpuExec as _;
    let (mut first, mut again) = (Vec::new(), Vec::new());
    assert!(engine.execute_into(&layer_plan(32, 16), &mut first).unwrap());
    assert!(engine.last_stages().stored, "the complete composite was kept");
    assert!(!engine.last_stages().replayed);
    assert!(engine.holds(engine.present_key(&layer_plan(32, 16)).unwrap()), "the coverage bars see it kept");
    assert!(engine.execute_into(&layer_plan(32, 16), &mut again).unwrap());
    assert!(engine.last_stages().replayed, "the second ask is a replay");
    assert_eq!(first, again, "the replay's bytes are the frame's");
}

/// t2747 (gl-record-preview): the frames the preview composes on its own device off the window's are
/// counted as the preview's in the GPU record. They take the bytes path, which timed them as the
/// offline device's, and on GL every record of a playback read "N frames (offline N)" (live
/// gpu_record red on stage 16).
#[test]
fn t2747_the_preview_off_the_display_counts_its_frames_as_the_preview() {
    // SAFETY: before any instance of this test's process
    unsafe { std::env::set_var("WGPU_BACKEND", "gl") };
    kt_plugins_builtin::install();
    let Some(engine) = kt_gpu::preview_engine_off_display() else {
        eprintln!("skipped: no GL adapter");
        return;
    };
    use kt_engine::VideoGpuExec as _;
    let mut out = Vec::new();
    let _ = kt_gpu::pass_time::take();
    // two moments never shown before: both composed, neither replayed
    for (w, h) in [(40, 20), (48, 24)] {
        assert!(engine.execute_into(&layer_plan(w, h), &mut out).unwrap());
        assert!(!engine.last_stages().replayed);
    }
    let s = kt_gpu::pass_time::take().expect("two frames composed");
    let line = s.line();
    eprintln!("t2747 GPU   {line}");
    // the other tests of this binary compose on the same preview engine only
    assert!(s.preview >= 2, "the preview's frames are the preview's: {line}");
    assert_eq!((s.offline, s.export), (0, 0), "{line}");
    assert!(line.contains("preview"), "{line}");
}

/// t2748 (gl-record-preview): a new project's preview engine is made, and the last one released, while
/// a call of ours holds the preview device's GL context past wgpu-hal's six seconds: both wait at the
/// gate instead of panicking. A sibling's frame state created its textures past the gate, and the
/// thread that opened a project died "Could not lock adapter context" (t2746 beside t2745's hold under
/// load).
#[test]
fn t2748_a_new_preview_engine_waits_out_a_stalled_call() {
    // SAFETY: before any instance of this test's process
    unsafe { std::env::set_var("WGPU_BACKEND", "gl") };
    kt_plugins_builtin::install();
    let Some(first) = kt_gpu::preview_engine_off_display() else {
        eprintln!("skipped: no GL adapter");
        return;
    };
    let (held_tx, held_rx) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        first.hold_gl_context(|| {
            held_tx.send(()).unwrap();
            // past wgpu-hal's CONTEXT_LOCK_TIMEOUT_SECS (6): the stall IS the check
            std::thread::sleep(std::time::Duration::from_secs(7));
        })
    });
    held_rx.recv().unwrap();
    // the next project's engine: a sibling made, the last one released, both while the lock is held
    let next = kt_gpu::preview_engine_off_display().expect("the preview device stands");
    if !holder.join().unwrap() {
        eprintln!("skipped: the preview's device is not GL");
        return;
    }
    use kt_engine::VideoGpuExec as _;
    let mut out = Vec::new();
    assert!(next.execute_into(&layer_plan(20, 10), &mut out).unwrap(), "the new engine composes");
}

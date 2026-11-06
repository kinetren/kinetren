//! t1502, t1503 (compute-kernels), t1570, t1571 (compute-kernels-render), capability `plugins`: the
//! compute door on the host's device.

use kt_plugin_api::compute::{Buf, ComputeHost, Init, Job, Step};

const DOUBLE: &str = "@group(0) @binding(0) var<uniform> u: vec4<u32>;\n\
    @group(0) @binding(1) var<storage, read> a: array<u32>;\n\
    @group(0) @binding(2) var<storage, read_write> b: array<u32>;\n\
    @compute @workgroup_size(4) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
    if id.x < u.x { b[id.x] = a[id.x] * 2u; }\n}\n";

fn double(host: &dyn ComputeHost, shader: &str) -> Result<Vec<u32>, String> {
    let words: Vec<u8> = (0u32..9).flat_map(|v| v.to_le_bytes()).collect();
    let uniform: Vec<u8> = [9u32, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let step = Step { shader, uniform: &uniform, read: &[Buf(0)], write: &[Buf(1)], groups: [3, 1, 1] };
    let out = host.run(&Job {
        buffers: &[Init::Bytes(&words), Init::Zero(36)],
        steps: &[step],
        reads: &[Buf(1)],
    })?;
    Ok(out[0].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

fn host() -> Option<std::sync::Arc<dyn ComputeHost>> {
    match kt_gpu::compute::open() {
        Ok(h) => Some(h),
        Err(e) if std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1") => {
            eprintln!("skipped: no compute device ({e})");
            None
        }
        Err(e) => panic!("KT_REQUIRE_GPU=1, but no compute device: {e}"),
    }
}

#[test]
fn t1502_a_job_runs_and_reads_back() {
    let Some(host) = host() else { return };
    let want: Vec<u32> = (0..9).map(|v| v * 2).collect();
    assert_eq!(double(&*host, DOUBLE).unwrap(), want);
    assert_eq!(double(&*host, DOUBLE).unwrap(), want, "the same text again");
}

#[test]
fn t1503_a_refused_shader_is_an_error_not_a_panic() {
    let Some(host) = host() else { return };
    let err = double(&*host, "this is not a shader").unwrap_err();
    assert!(err.contains("refused"), "{err}");
    assert_eq!(double(&*host, DOUBLE).unwrap()[8], 16, "the host survives");
}

#[test]
fn t1570_a_zero_buffer_is_zero_after_a_job_used_it() {
    let Some(host) = host() else { return };
    assert_eq!(double(&*host, DOUBLE).unwrap()[8], 16);
    let got = host.run(&Job { buffers: &[Init::Zero(36)], steps: &[], reads: &[Buf(0)] }).unwrap();
    assert_eq!(got[0], vec![0u8; 36], "the buffer of the last job came back unclean");
}

#[test]
fn t1571_the_pool_reuses_and_stays_bounded() {
    let Ok(device) = kt_gpu::compute::open_device() else { return };
    double(&*device, DOUBLE).unwrap();
    let one = device.pooled_bytes();
    assert!(one > 0, "a finished job left nothing to reuse");
    double(&*device, DOUBLE).unwrap();
    assert_eq!(device.pooled_bytes(), one, "the same job grew the pool");
    // six live buffers of 200 MiB against a bound of 1 GiB: the last does not fit
    let big = vec![0u8; 200 << 20];
    for _ in 0..3 {
        let buffers = [Init::Bytes(&big); 6];
        device.run(&Job { buffers: &buffers, steps: &[], reads: &[] }).unwrap();
        assert!(device.pooled_bytes() <= 1 << 30);
    }
}

/// t1642 (gpu-render-hardening, D1): the devices of an export follow the memory and the frame, stop
/// at the ceiling, and never reach zero.
#[test]
fn t1642_the_device_count_follows_the_memory() {
    use kt_gpu::memory::engines_for;
    let gb = |n: u64| Some(n << 30);
    assert_eq!(engines_for(32, gb(64), 1920, 1080), 4, "plenty: the ceiling");
    assert_eq!(engines_for(8, gb(64), 1920, 1080), 2, "an eighth of the cores, at least two");
    let two = engines_for(32, gb(7), 1920, 1080);
    assert!((1..=3).contains(&two), "7 GB free does not take the ceiling: {two}");
    assert_eq!(engines_for(32, gb(4), 1920, 1080), 1, "little memory: one device");
    assert_eq!(engines_for(32, gb(1), 3840, 2160), 1, "never none");
    assert!(engines_for(32, gb(12), 3840, 2160) < engines_for(32, gb(12), 1920, 1080), "a bigger frame costs more");
    assert_eq!(engines_for(32, None, 1920, 1080), 2, "unknown memory: the small machine's count");
    assert_eq!(engines_for(2, None, 1920, 1080), 2);
    use kt_gpu::memory::decode_threads;
    assert_eq!(decode_threads(gb(64), 1920, 1080), 4, "a render does not need auto's sixteen");
    assert_eq!(decode_threads(None, 1920, 1080), 4);
    assert_eq!(decode_threads(gb(3), 1920, 1080), 2, "little memory: two");
}

/// t1643: the memory the process reads is never more than the system has, and it follows a limit.
#[test]
fn t1643_the_available_memory_is_read() {
    if cfg!(any(target_os = "linux", windows)) {
        assert!(kt_gpu::memory::available().is_some_and(|m| m > 0), "the memory was not read");
    }
}

/// t1650 (gpu-render-hardening, D4): every engine shader compiles on D3D12 through FXC, and no frame
/// runs. The Windows runner has only WARP, which dies running our passes, but its compiler is the
/// compiler of every D3D12 machine, and FXC refuses what naga and Vulkan pass. Elsewhere: no D3D12,
/// a skip.
#[test]
fn t1650_every_shader_compiles_on_d3d12() {
    kt_plugins_builtin::install();
    let Some(ctx) = kt_gpu::GpuContext::new_d3d12() else {
        eprintln!("skipped: no D3D12 adapter");
        return;
    };
    let name = ctx.adapter_info.name.clone();
    kt_gpu::GpuFxEngine::compile_check(ctx).unwrap_or_else(|e| panic!("{name}: {e}"));
}

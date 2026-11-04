//! t1502, t1503 (compute-kernels, capability `plugins`): the compute door on the host's device.

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

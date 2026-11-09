//! t2122 (pipeline-prewarm; capabilities `plugins`, `video-plan`): the shaders Datamosh names in
//! `compute_shaders` are every shader its jobs run. Once the host has prepared them (what the player's
//! prewarm does when a project reaches the effect), the first intra frame, the first predicted frame
//! and the first decode create no pipeline and dispatch none cold. Its own test binary: the counters
//! are process-wide.

use fx_video_codec::codec::{self, Cfg, Planes};
use kt_plugin_api::compute;

#[test]
fn t2122_a_prepared_datamosh_compiles_nothing_in_its_first_frame() {
    compute::install(kt_gpu::compute::open);
    let Some(host) = compute::host() else {
        eprintln!("skipped: no compute device");
        return;
    };
    let mosh = fx_video_codec::plugins().into_iter().find(|p| &*p.manifest().id == "datamosh").expect("datamosh");
    let shaders = mosh.compute_shaders();
    assert!(!shaders.is_empty(), "Datamosh names no shader");
    for s in &shaders {
        host.prepare(s).expect("prepared");
    }
    let (cold, made) = (kt_gpu::cold_draws(), kt_gpu::pipelines_in_frame());
    let (w, h) = (320usize, 176usize);
    let mut src = Planes::grey(w, h);
    for (i, v) in src.y.iter_mut().enumerate() {
        *v = ((i * 5 + (i / w) * 7) % 251) as u8;
    }
    let cfg = Cfg { bs: 16, search: 8, qp: 26.0 };
    let intra = codec::encode(&src, None, &cfg).expect("intra");
    let next = codec::encode(&src, Some(&intra.recon), &cfg).expect("predicted");
    codec::decode(&intra.recon, &next.pkt.mv, 16, &next.pkt.res, 256).expect("decoded");
    assert_eq!(kt_gpu::pipelines_in_frame() - made, 0, "a job compiled a shader Datamosh does not name");
    assert_eq!(kt_gpu::cold_draws() - cold, 0, "a job dispatched a pipeline the preparation did not run");
}

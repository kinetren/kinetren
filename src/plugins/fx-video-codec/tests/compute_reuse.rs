//! t2024 (gpu-pass-cost; capability `video-fx`): after warm-up a Datamosh frame — the encoder's job
//! and the decoder's — creates no buffer or pipeline on the compute host: the pool hands back the
//! same sizes every frame. Its own test binary: the counter is process-wide.

use fx_video_codec::codec::{self, Cfg, Planes};
use kt_plugin_api::compute;

#[test]
fn t2024_a_datamosh_frame_creates_nothing_after_warm_up() {
    compute::install(kt_gpu::compute::open);
    if compute::host().is_none() {
        eprintln!("skipped: no compute device");
        return;
    }
    let (w, h) = (640usize, 352usize);
    let mut src = Planes::grey(w, h);
    for (i, v) in src.y.iter_mut().enumerate() {
        *v = ((i * 7 + (i / w) * 3) % 251) as u8;
    }
    let cfg = Cfg { bs: 16, search: 16, qp: 26.0 };
    let mut prev = codec::encode(&src, None, &cfg).unwrap();
    let frame = |prev: &codec::Clean| {
        let n = codec::encode(&src, Some(&prev.recon), &cfg).unwrap();
        codec::decode(&prev.recon, &n.pkt.mv, 16, &n.pkt.res, 256).unwrap();
        n
    };
    for _ in 0..2 {
        prev = frame(&prev);
    }
    let before = kt_gpu::resources_created();
    for _ in 0..3 {
        prev = frame(&prev);
    }
    assert_eq!(kt_gpu::resources_created() - before, 0, "buffers or pipelines created in three warm frames");
}

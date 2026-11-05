//! What the first frame of a compute kernel waits for (compute-kernels-render, D1): run by hand in
//! a fresh process, several times, with the machine's load in mind.

use std::time::Instant;

use fx_video_codec::codec::{self, Cfg, Planes};
use kt_plugin_api::compute;

#[test]
#[ignore]
fn first_frame_cost() {
    compute::install(kt_gpu::compute::open);
    let t = Instant::now();
    assert!(compute::host().is_some());
    eprintln!("open   {:>8.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let (w, h) = (1920usize, 1080usize);
    let mut src = Planes::grey(w, h);
    for (i, v) in src.y.iter_mut().enumerate() {
        *v = ((i * 7 + (i / w) * 3) % 251) as u8;
    }
    let cfg = Cfg { bs: 16, search: 16, qp: 26.0 };
    let t = Instant::now();
    let a = codec::encode(&src, None, &cfg).unwrap();
    eprintln!("intra  {:>8.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let b = codec::encode(&src, Some(&a.recon), &cfg).unwrap();
    eprintln!("first P{:>8.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let _ = codec::encode(&src, Some(&b.recon), &cfg).unwrap();
    eprintln!("next P {:>8.1} ms", t.elapsed().as_secs_f64() * 1e3);
}

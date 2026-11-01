//! The cost of camera_isp per 1080p frame on the CPU (design D9). Not a regression test: run it on a
//! release build when the kernel changes —
//! `cargo test --release -p fx-video-camera --test cost -- --ignored --nocapture`.

use fx_video_camera::CameraIsp;
use kt_plugin_api::barrier::{BarrierCtx, BarrierKernel, Frame, RasterHost, Scratch};
use kt_plugin_api::{ParamValues, Plugin};

struct NoRaster;
impl RasterHost for NoRaster {
    fn resample(&self, _: &Frame, _: u32, _: u32, _: &mut dyn Scratch) -> Frame {
        unreachable!()
    }
    fn sample(&self, _: &Frame, _: f64, _: f64) -> [f64; 4] {
        unreachable!()
    }
    fn premultiply(&self, _: &mut [u8]) {}
    fn unpremultiply(&self, _: &mut [u8]) {}
}

struct Pool;
impl Scratch for Pool {
    fn take(&mut self, len: usize) -> Vec<u8> {
        vec![0; len]
    }
    fn put(&mut self, _: Vec<u8>) {}
}

#[test]
#[ignore]
fn camera_isp_cost_per_1080p_frame() {
    let cam = CameraIsp::new();
    let m = cam.manifest();
    let (w, h) = (1920u32, 1080u32);
    let rgba: Vec<u8> =
        (0..w * h).flat_map(|i| [(i % w) as u8, (i / w) as u8, (i / 7) as u8, 255]).collect();
    let mut forms = vec![("defaults", ParamValues::defaults(&m.passport).0)];
    for v in m.ui.variants {
        let mut vals = ParamValues::defaults(&m.passport).0;
        for (n, x) in v.sets {
            vals[m.passport.iter().position(|s| s.name == *n).unwrap()] = *x;
        }
        forms.push((v.name_key, vals));
    }
    for (what, vals) in &forms {
        for simd in [false, true] {
            if simd && !std::is_x86_feature_detected!("avx2") {
                continue;
            }
            let cx = BarrierCtx { res_scale: 1, burst_index: 0, simd, raster: &NoRaster };
            let run = || {
                cam.frame_values(
                    vals,
                    Frame { w, h, rgba: rgba.clone(), has_alpha: false },
                    &cx,
                    &mut Pool,
                )
            };
            run();
            let t = std::time::Instant::now();
            for _ in 0..3 {
                run();
            }
            println!(
                "{what} simd={simd}: {:.0} ms/frame",
                t.elapsed().as_secs_f64() * 1000.0 / 3.0
            );
        }
    }
}

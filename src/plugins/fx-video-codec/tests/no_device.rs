//! t1505 (compute-kernels): with no compute device the stage passes the source frame through. This
//! binary never installs a host.

use fx_video_codec::Datamosh;
use kt_plugin_api::Plugin;
use kt_plugin_api::stream::{Rgba, Side, StreamInput, StreamLayout, StreamMemo};

struct One(Vec<u8>);

impl StreamLayout for One {
    fn size(&self) -> (u32, u32) {
        (4, 2)
    }
    fn first(&self) -> i64 {
        0
    }
    fn cut(&self) -> i64 {
        0
    }
    fn end(&self) -> i64 {
        8
    }
    fn second_start(&self, _k: i64) -> Option<i64> {
        None
    }
    fn ident(&self, _side: Side, k: i64) -> u64 {
        k as u64 + 77
    }
}

impl StreamInput for One {
    fn with_frame(&self, _side: Side, _k: i64, f: &mut dyn FnMut(Rgba<'_>)) -> bool {
        f(Rgba { w: 4, h: 2, rgba: &self.0 });
        true
    }
}

#[test]
fn t1505_no_device_leaves_the_frame_and_says_why() {
    assert!(kt_plugin_api::compute::host().is_none());
    let plugin = Datamosh::new();
    let kernel = plugin.stream().expect("a stream kernel");
    let input = One((0..32).collect());
    let mut out = Vec::new();
    let got = kernel.render(&fx_video_codec::defaults(), &input, 3, &StreamMemo::new(1 << 20), &mut out).expect("a frame");
    assert_eq!(out, input.0, "the source frame, untouched");
    assert_eq!((got.w, got.h, got.key), (4, 2, 80));
    assert!(kt_plugin_api::compute::unavailable().is_some(), "the panel has a reason to show");
}

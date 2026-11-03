//! Color Grade written ONCE (change `compute-backends`, F-EXT-004): the prototype of one kernel
//! text for the CPU and the GPU. The plugin does NOT use it — `ColorGrade` keeps `grade` and
//! `color_grade.wgsl`; t1500 holds this text's CPU form byte for byte to the shipped kernel and
//! t1501 compiles its WGSL on the GPU against the CPU reference.
//!
//! The order of operations is the CPU kernel's (`grade`), not the hand-written WGSL's: the WGSL
//! mirror added `128 + br·255` as one term, the CPU adds 128 and then `br·255`. One text has one order.

kt_plugin_api::shared_kernel! {
    pub mod color_grade {
        fn cg_hue(s: vec3f, hc: f32, hs: f32) -> vec3f {
            let kk = 1.0 / sqrt(3.0);
            let dv = kk * (s.x + s.y + s.z);
            let cx = kk * (s.y - s.z);
            let cy = kk * (s.z - s.x);
            let cz = kk * (s.x - s.y);
            return vec3f(
                s.x * hc + cx * hs + kk * dv * (1.0 - hc),
                s.y * hc + cy * hs + kk * dv * (1.0 - hc),
                s.z * hc + cz * hs + kk * dv * (1.0 - hc),
            );
        }
        fn tap_color_grade(v: vec4f, a: vec4f, b: vec4f, c: vec2f) -> vec4f {
            let br = a.y;
            let con = a.z;
            let sat = a.w;
            let l = (77.0 * v.x + 150.0 * v.y + 29.0 * v.z) / 256.0;
            let s = vec3f(l + (v.x - l) * sat, l + (v.y - l) * sat, l + (v.z - l) * sat);
            let h = select(s, cg_hue(s, b.x, b.y), b.x != 1.0 || b.y != 0.0);
            return vec4f(
                (h.x - 128.0) * (1.0 + con) + 128.0 + br * 255.0,
                (h.y - 128.0) * (1.0 + con) + 128.0 + br * 255.0,
                (h.z - 128.0) * (1.0 + con) + 128.0 + br * 255.0,
                v.w,
            );
        }
    }
}

/// The shipped kernel's cooked numbers (brightness, contrast, saturation, hue cos, hue sin) as the
/// text's `a`/`b` arguments — the packing of `gpu_params` into `P.f0`/`P.f1`.
pub fn tap_args(k: &[f64; 8]) -> ([f64; 4], [f64; 4]) {
    ([0.0, k[0], k[1], k[2]], [k[3], k[4], 0.0, 0.0])
}

/// The shipped kernel, for the prototype's comparisons: `grade` through the barrier loop.
pub fn shipped_frame(k: &[f64; 8], rgba: &mut [u8]) {
    kt_plugin_api::frame_with(rgba, |v| crate::grade(v, k[0], k[1], k[2], k[3], k[4]));
}

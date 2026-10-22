// Common header of all kt-gpu passes (prepended to the start of each .wgsl).
//
// All textures are Rgba8Uint: textureLoad returns RAW bytes 0..255 without
// unorm normalization (u8/255.0 in f32 loses precision and breaks the mirrors of
// the integer CPU arithmetic). There is no filtering or sampler: bilinear is
// computed by hand, as in bilinear_into (kt-fx/src/video.rs).
//
// One uniform for all passes (64 bytes, generic slots): one bind group
// layout for the whole engine; unused textures are bound to a 1×1 stub.

struct Params {
    u0: vec4<u32>,
    u1: vec4<u32>,
    f0: vec4<f32>,
    f1: vec4<f32>,
    // MVP matrix of the 3D passes (R.29, pan3d); the other passes do not read it.
    m: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var T0: texture_2d<u32>;
@group(0) @binding(2) var T1: texture_2d<u32>;
@group(0) @binding(3) var T2: texture_2d<u32>;

// Fullscreen triangle: (-1,-1) (-1,3) (3,-1) covers the whole viewport.
@vertex
fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(vi / 2u)) * 4.0 - 1.0;
    let y = f32(i32(vi & 1u)) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

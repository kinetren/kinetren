// Sharpen (R.47): a mirror of the tail of kt_fx::video::sharpen: src +
// (src − mip base)·amount, the source’s alpha. T0 = original, T1 = base
// (down/up resamples). f0.x = amount.
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let c = textureLoad(T0, p, 0);
    let b = textureLoad(T1, p, 0);
    let a = P.f0.x;
    // No write through a dynamic vector index: FXC (wgpu's D3D12 compiler on Windows) rejects
    // it when it cannot unroll the loop (windows-build). Same per-channel arithmetic.
    let s = vec3<f32>(c.rgb);
    let o = s + (s - vec3<f32>(b.rgb)) * a;
    let rgb = vec3<u32>(clamp(round(o), vec3<f32>(0.0), vec3<f32>(255.0)));
    return vec4<u32>(rgb, c.a);
}

// blocks: the order of cells is the hash of the cell
fn trans_blocks(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let cols = max(p0.x, 1.0);
    let rows = max(p0.y, 1.0);
    let i = u32(floor(uv.x * cols));
    let j = u32(floor(uv.y * rows));
    let s = hash01(t_hash(i * 1973u) ^ t_hash(j * 9277u));
    return mix(samp(0u, uv), samp(1u, uv), front_mask(s, k, p0.z));
}

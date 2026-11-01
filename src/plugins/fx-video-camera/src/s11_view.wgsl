// PV the view of a stage (cpu.rs view_row): the texel of the stage shown, as a picture — the scene,
// a mosaic in its filter colours (W grey, IR magenta), luma, or chroma at mid-grey.
// u0 = (flags, format, adc_max | black << 16, 0); u1 = the table of the stage shown;
// f0 = (Kr, Kb, 1/Kg, 2(1−Kr)); f1 = (2(1−Kb), 0, 0, 0). T0 = the stage, T1 = the source.

fn tint(code: u32, v: f32) -> vec3<u32> {
    let e = byte(oetf(v, 1u));
    switch code {
        case 0u: { return vec3<u32>(e, 0u, 0u); }
        case 1u: { return vec3<u32>(0u, e, 0u); }
        case 2u: { return vec3<u32>(0u, 0u, e); }
        case 3u: { return vec3<u32>(e, e, e); }
        default: { return vec3<u32>(e, 0u, e); }
    }
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    FL = P.u0.x;
    TAB = P.u1;
    let p = vec2<i32>(floor(pos.xy));
    let k = KK();
    let sx = p.x / k;
    let sy = p.y / k;
    let f = P.u0.y;
    let code = cfa_code(sx, sy);
    var rgb = vec3<u32>(0u);
    if (f == 0u) {
        let s = unpack_scene(at_frame(p.x, p.y));
        rgb = vec3<u32>(byte(oetf(s.x, 1u)), byte(oetf(s.y, 1u)), byte(oetf(s.z, 1u)));
    } else if (f == 1u || f == 3u) {
        rgb = tint(code, f32(v16(at(sx, sy))) * 6.1035156e-5);
    } else if (f == 2u) {
        let adc_max = P.u0.z & 0xFFFFu;
        let black = P.u0.z >> 16u;
        let dn = f32(v16(at(sx, sy)));
        rgb = tint(code, (dn - f32(black)) / f32(adc_max - black));
    } else if (f == 4u) {
        let yy = at(sx, sy).r;
        rgb = vec3<u32>(yy, yy, yy);
    } else {
        let t = at(sx, sy);
        let u = f32(t.g) * 0.003921569 - 0.5;
        let v = f32(t.b) * 0.003921569 - 0.5;
        let r = 0.5 + P.f0.w * v;
        let b = 0.5 + P.f1.x * u;
        let g = (0.5 - P.f0.x * r - P.f0.y * b) * P.f0.z;
        rgb = vec3<u32>(byte(r), byte(g), byte(b));
    }
    return vec4<u32>(rgb.x, rgb.y, rgb.z, src_alpha(p));
}

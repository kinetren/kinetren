// The export's RGB → YUV conversion on the composing device (export-host-copy D2, D3).
//
// A MIRROR of `kt_engine::yuv::convert`, byte for byte: the same Q15 integers (the host computes them
// once, `YuvLayout::coeffs`), the same left-sited chroma filter, the same edge repeat, the same packing.
// Integer arithmetic only: no float touches a sample, so every device writes the reference's bytes.
//
// The target is a word texture (Rgba8Uint, one 32-bit word of the packed planes per texel, row after
// row of `tw` texels): a fragment writes bytes 4k … 4k+3 of the frame, k = y·tw + x; past the frame's
// bytes it writes zeros. T0 is the composite (Rgba8Uint, RGB raw bytes).
//
// Uniform: u0 = (w, h, layout code, tw); u1 = (Y weights of R, G, B as i32 bits, Y offset Q15 as i32
// bits); f0 = (Cb weights of R, G, B, chroma offset Q15) and f1 = (Cr weights, the largest sample) as
// float VALUES — every one of them is an integer below 2^24 in magnitude, exact in f32.
// Layout codes: 0 yuv420p, 1 nv12, 2 yuv420p10le, 3 p010le, 4 yuv422p10le.

fn ten_bit(pix: u32) -> bool {
    return pix >= 2u;
}

fn semi_planar(pix: u32) -> bool {
    return pix == 1u || pix == 3u;
}

fn half_rows(pix: u32) -> bool {
    return pix != 4u;
}

fn rgb(x: i32, y: i32) -> vec3<i32> {
    let w = i32(P.u0.x);
    let h = i32(P.u0.y);
    let c = textureLoad(T0, vec2<i32>(clamp(x, 0, w - 1), clamp(y, 0, h - 1)), 0);
    return vec3<i32>(i32(c.x), i32(c.y), i32(c.z));
}

fn top() -> i32 {
    return i32(P.f1.w);
}

fn luma(x: i32, y: i32) -> i32 {
    let p = rgb(x, y);
    let s = bitcast<i32>(P.u1.x) * p.x + bitcast<i32>(P.u1.y) * p.y + bitcast<i32>(P.u1.z) * p.z;
    return clamp((s + bitcast<i32>(P.u1.w) + 16384) >> 15u, 0, top());
}

// a chroma's Q15 value at one pixel: `which` 0 — Cb, 1 — Cr
fn chroma_at(which: u32, x: i32, y: i32) -> i32 {
    let p = rgb(x, y);
    var k = vec3<i32>(i32(P.f0.x), i32(P.f0.y), i32(P.f0.z));
    if (which == 1u) {
        k = vec3<i32>(i32(P.f1.x), i32(P.f1.y), i32(P.f1.z));
    }
    return k.x * p.x + k.y * p.y + k.z * p.z + i32(P.f0.w);
}

// the filtered sample: [1 2 1] across at the even column, [1 1] down for 4:2:0
fn chroma(which: u32, cx: i32, cy: i32, pix: u32) -> i32 {
    let x0 = 2 * cx;
    let xl = max(x0 - 1, 0);
    var sum = 0;
    var bits = 17u;
    if (half_rows(pix)) {
        bits = 18u;
        for (var r = 0; r < 2; r = r + 1) {
            let ry = 2 * cy + r;
            sum = sum + chroma_at(which, xl, ry) + 2 * chroma_at(which, x0, ry) + chroma_at(which, x0 + 1, ry);
        }
    } else {
        sum = chroma_at(which, xl, cy) + 2 * chroma_at(which, x0, cy) + chroma_at(which, x0 + 1, cy);
    }
    return clamp((sum + (1 << (bits - 1u))) >> bits, 0, top());
}

// byte `i` of the packed planes
fn packed_byte(i: u32) -> u32 {
    let w = P.u0.x;
    let h = P.u0.y;
    let pix = P.u0.z;
    var s = 1u;
    if (ten_bit(pix)) {
        s = 2u;
    }
    let cw = (w + 1u) / 2u;
    var ch = h;
    if (half_rows(pix)) {
        ch = (h + 1u) / 2u;
    }
    let y_bytes = w * h * s;
    let c_bytes = cw * ch * s;
    var total = y_bytes + 2u * c_bytes;
    if (i >= total) {
        return 0u;
    }
    var v = 0;
    var j = 0u;
    if (i < y_bytes) {
        let si = i / s;
        v = luma(i32(si % w), i32(si / w));
        j = i % s;
    } else if (semi_planar(pix)) {
        let k = i - y_bytes;
        let si = k / s;
        let pair = si / 2u;
        v = chroma(si % 2u, i32(pair % cw), i32(pair / cw), pix);
        j = k % s;
    } else {
        let k = i - y_bytes;
        let which = k / c_bytes;
        let si = (k % c_bytes) / s;
        v = chroma(which, i32(si % cw), i32(si / cw), pix);
        j = k % s;
    }
    var u = u32(v);
    if (pix == 3u) {
        u = u << 6u;
    }
    if (j == 1u) {
        return (u >> 8u) & 255u;
    }
    return u & 255u;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<u32>(pos.xy);
    let k = p.y * P.u0.w + p.x;
    let b = 4u * k;
    return vec4<u32>(packed_byte(b), packed_byte(b + 1u), packed_byte(b + 2u), packed_byte(b + 3u));
}

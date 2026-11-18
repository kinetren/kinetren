// The carrier decoder of one plane (mosh-sharp D1): the codec's decisions at its size executed on a
// picture of the stream's input size. A carrier pixel takes the codec block covering it (by the visible
// sizes), predicts from the reference by that block's applied vector scaled to the carrier, and where the
// block is coded in this plane adds `gain ×` the source's own difference: the frame minus the previous
// frame moved by the packet's own vector, scaled alike. Integer only — `codec::carry_planes`, the CPU
// reference, to the byte. Planes are bytes, four to a word; each invocation writes one word.
struct U { pw: u32, ph: u32, cw: u32, ch: u32, sw: u32, sh: u32, bs: u32, mw: u32, mh: u32, shift: u32, gain: i32, bit: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> rf: array<u32>;
@group(0) @binding(2) var<storage, read> cur: array<u32>;
@group(0) @binding(3) var<storage, read> prev: array<u32>;
@group(0) @binding(4) var<storage, read> mv: array<vec2<i32>>;
@group(0) @binding(5) var<storage, read> own: array<vec2<i32>>;
@group(0) @binding(6) var<storage, read> flags: array<u32>;
@group(0) @binding(7) var<storage, read_write> pic: array<u32>;

fn rf_at(i: i32) -> i32 { return i32((rf[u32(i) >> 2u] >> ((u32(i) & 3u) * 8u)) & 255u); }
fn cur_at(i: i32) -> i32 { return i32((cur[u32(i) >> 2u] >> ((u32(i) & 3u) * 8u)) & 255u); }
fn prev_at(i: i32) -> i32 { return i32((prev[u32(i) >> 2u] >> ((u32(i) & 3u) * 8u)) & 255u); }

// a codec pel in carrier pels, rounded half away from zero
fn sc(v: i32, num: u32, den: u32) -> i32 {
    let a = v * i32(num);
    let d = i32(den);
    if a >= 0 { return (2 * a + d) / (2 * d); }
    return -((-2 * a + d) / (2 * d));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= u.pw / 4u || id.y >= u.ph { return; }
    let w = i32(u.pw);
    let h = i32(u.ph);
    let y = i32(id.y);
    let ly = id.y << u.shift;
    let by = min(ly * u.ch / (u.sh * u.bs), u.mh - 1u);
    var word = 0u;
    for (var j = 0u; j < 4u; j++) {
        let x = i32(id.x * 4u + j);
        let lx = u32(x) << u.shift;
        let bx = min(lx * u.cw / (u.sw * u.bs), u.mw - 1u);
        let b = by * u.mw + bx;
        let v = mv[b];
        let vx = sc(v.x, u.sw, u.cw) >> u.shift;
        let vy = sc(v.y, u.sh, u.ch) >> u.shift;
        var out = rf_at(clamp(y + vy, 0, h - 1) * w + clamp(x + vx, 0, w - 1));
        if (flags[b] & u.bit) != 0u {
            let o = own[b];
            let ox = sc(o.x, u.sw, u.cw) >> u.shift;
            let oy = sc(o.y, u.sh, u.ch) >> u.shift;
            let r = cur_at(y * w + x) - prev_at(clamp(y + oy, 0, h - 1) * w + clamp(x + ox, 0, w - 1));
            out += (r * u.gain) >> 8u;
        }
        word |= u32(clamp(out, 0, 255)) << (j * 8u);
    }
    pic[(id.y * u.pw + id.x * 4u) >> 2u] = word;
}

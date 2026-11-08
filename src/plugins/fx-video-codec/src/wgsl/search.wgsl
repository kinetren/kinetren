// The motion search (compute-kernels D5): candidates (0,0) and the vectors a previous sweep found
// around the block, a diamond of step 2 to convergence, a ±1 refinement. Ties: the shorter vector.
struct U { w: u32, h: u32, bs: u32, r: i32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> cur: array<u32>;
@group(0) @binding(2) var<storage, read> rf: array<u32>;
@group(0) @binding(3) var<storage, read> seed: array<vec2<i32>>;
@group(0) @binding(4) var<storage, read_write> mv: array<vec2<i32>>;

// a plane's sample: four to a word, as the plane's bytes are (gpu-pass-cost)
fn cur_at(i: i32) -> u32 { return (cur[u32(i) >> 2u] >> ((u32(i) & 3u) * 8u)) & 255u; }

// a plane's sample: four to a word, as the plane's bytes are (gpu-pass-cost)
fn rf_at(i: i32) -> u32 { return (rf[u32(i) >> 2u] >> ((u32(i) & 3u) * 8u)) & 255u; }

fn sad(bx: i32, by: i32, v: vec2<i32>, limit: u32) -> u32 {
    var s = 0u;
    let w = i32(u.w);
    let h = i32(u.h);
    for (var y = 0; y < i32(u.bs); y++) {
        let sy = clamp(by + y + v.y, 0, h - 1);
        for (var x = 0; x < i32(u.bs); x++) {
            let sx = clamp(bx + x + v.x, 0, w - 1);
            let a = i32(cur_at((by + y) * w + bx + x));
            let b = i32(rf_at(sy * w + sx));
            s += u32(abs(a - b));
        }
        if s >= limit { return s; }
    }
    return s;
}

fn better(c: u32, v: vec2<i32>, bc: u32, b: vec2<i32>) -> bool {
    if c != bc { return c < bc; }
    let l = abs(v.x) + abs(v.y);
    let bl = abs(b.x) + abs(b.y);
    if l != bl { return l < bl; }
    if v.y != b.y { return v.y < b.y; }
    return v.x < b.x;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let bw = u.w / u.bs;
    let bh = u.h / u.bs;
    if id.x >= bw || id.y >= bh { return; }
    let bx = i32(id.x * u.bs);
    let by = i32(id.y * u.bs);
    let at = id.y * bw + id.x;
    var best = vec2<i32>(0, 0);
    var bc = sad(bx, by, best, 0xffffffffu);
    let n = array<vec2<i32>, 5>(vec2<i32>(0, 0), vec2<i32>(-1, 0), vec2<i32>(1, 0), vec2<i32>(0, -1), vec2<i32>(0, 1));
    for (var i = 0; i < 5; i++) {
        let bi = vec2<i32>(i32(id.x), i32(id.y)) + n[i];
        if bi.x < 0 || bi.y < 0 || bi.x >= i32(bw) || bi.y >= i32(bh) { continue; }
        let c = clamp(seed[u32(bi.y) * bw + u32(bi.x)], vec2<i32>(-u.r), vec2<i32>(u.r));
        let cc = sad(bx, by, c, bc + 1u);
        if better(cc, c, bc, best) { best = c; bc = cc; }
    }
    let d2 = array<vec2<i32>, 4>(vec2<i32>(2, 0), vec2<i32>(-2, 0), vec2<i32>(0, 2), vec2<i32>(0, -2));
    for (var k = 0; k < 16; k++) {
        var moved = false;
        let base = best;
        for (var i = 0; i < 4; i++) {
            let c = clamp(base + d2[i], vec2<i32>(-u.r), vec2<i32>(u.r));
            let cc = sad(bx, by, c, bc + 1u);
            if better(cc, c, bc, best) { best = c; bc = cc; moved = true; }
        }
        if !moved { break; }
    }
    let d1 = array<vec2<i32>, 8>(vec2<i32>(1, 0), vec2<i32>(-1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1),
                                 vec2<i32>(1, 1), vec2<i32>(-1, -1), vec2<i32>(1, -1), vec2<i32>(-1, 1));
    let base = best;
    for (var i = 0; i < 8; i++) {
        let c = clamp(base + d1[i], vec2<i32>(-u.r), vec2<i32>(u.r));
        let cc = sad(bx, by, c, bc + 1u);
        if better(cc, c, bc, best) { best = c; bc = cc; }
    }
    mv[at] = best;
}

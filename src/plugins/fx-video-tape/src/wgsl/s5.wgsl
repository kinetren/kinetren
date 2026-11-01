// vhs_tape stage 5 (`sim::s5`): read — the own track and the neighbour through the row's kernels,
// the dropout, the noise and the switching transient. T0 = the magnetisation, T1 = the line table.
// u0 = (W, H, seed); f0 = (burst σ, burst half-width).

fn lt(c: i32, y: i32) -> f32 { return ld1f(c, y); }

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    let cr = i32(lt(0, y));
    let adj = min(i32(lt(1, y)), i32(P.u0.y) - 1);
    var own = 0.0;
    var xt = 0.0;
    for (var k = 0; k < KN; k = k + 1) {
        let j = n + KR - k;
        own = own + lt(16 + k, y) * ld0f(j, cr);
        xt = xt + lt(16 + KN + k, y) * ld0f(j, adj);
    }
    let sig = lt(2, y) * own + lt(3, y) * xt;
    let nf = f32(n);
    let ds = lt(5, y);
    var env = 1.0;
    if (ds >= 0.0) {
        let inside = clamp(min(nf - ds, ds + lt(6, y) - nf) / 3.0 + 0.5, 0.0, 1.0);
        env = 1.0 - 0.98 * inside;
    }
    var noise = lt(4, y) * gauss(hash3(P.u0.z, u32(y), u32(n)));
    let b = lt(8, y);
    if (b >= 0.0 && abs(nf - b) < P.f0.y) {
        noise = noise + P.f0.x * gauss(hash3(P.u0.z ^ 0xB0B0u, u32(y), u32(n)));
    }
    return putf(env * sig + noise);
}

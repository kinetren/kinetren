// vhs_tape, the line table: a mirror of `deck::line_row` — texel (x, y) is column x of row y:
// the scalars `lt_scalar` (x < 16), then the own kernel's taps and the neighbour kernel's taps.
// T0 is the parameter texture (`deck::words`).

fn cw(i: i32) -> u32 { return ld0(i, 0); }
fn cf(i: i32) -> f32 { return ld0f(i, 0); }

// word indices (deck::w)
const W_H: i32 = 1;
const W_W: i32 = 2;
const W_FLAGS: i32 = 3;
const W_FRAME: i32 = 4;
const W_V_SWITCH: i32 = 6;
const W_PITCH: i32 = 8;
const W_HEAD_W: i32 = 9;
const W_TRACKING: i32 = 10;
const W_SKEW: i32 = 11;
const W_CROSSING: i32 = 12;
const W_AZ: i32 = 13;
const W_VAR_TAPE: i32 = 14;
const W_VAR_AMP: i32 = 15;
const W_P_DROP: i32 = 17;
const W_DROP_LEN: i32 = 18;
const W_JITTER: i32 = 19;
const W_SWITCH_POINT: i32 = 20;
const W_SWITCH_SKEW: i32 = 21;
const W_PH_DRUM: i32 = 22;
const W_PH_FLUT: i32 = 23;
const W_DRUM_LINE: i32 = 24;
const W_FLUT_LINE: i32 = 25;
const W_DRUM_FIELD: i32 = 26;
const W_FLUT_FIELD: i32 = 27;
const W_FCU_N: i32 = 28;
const W_A_C: i32 = 29;
const W_ROT_A: i32 = 30;
const W_ROT_B: i32 = 31;
const W_HEADS: i32 = 32;
const F_TBC: u32 = 1u;
const F_STILL4: u32 = 8u;

struct Geo {
    content: u32,
    head: u32,
    switched: bool,
    yf: u32,
    field: u32,
    c0: f32,
    c1: f32,
    c1w: f32,
    c2: f32,
};

fn ov(lo: f32, hi: f32, lo_h: f32, hi_h: f32) -> f32 {
    return max(min(hi, hi_h) - max(lo, lo_h), 0.0);
}

fn row_geo(y: u32) -> Geo {
    var g: Geo;
    let hf = cw(W_H) / 2u;
    g.field = y & 1u;
    g.yf = y >> 1u;
    let still4 = (cw(W_FLAGS) & F_STILL4) != 0u;
    let sp = cf(W_SWITCH_POINT);
    g.switched = (f32(g.yf) + 0.5) > (f32(hf) - sp);
    g.content = y;
    if (still4 && g.field == 1u) { g.content = y - 1u; }
    if (still4) {
        g.head = 0u;
    } else if (g.switched) {
        g.head = 1u - g.field;
    } else {
        g.head = g.field;
    }
    var u = (f32(g.yf) + 0.5) / f32(hf);
    if (g.switched) { u = u - 1.0; }
    let p = cf(W_PITCH);
    let o = cf(W_TRACKING) + cf(W_SKEW) * (u - 0.5) + cf(W_CROSSING) * u;
    let hw = cf(W_HEAD_W);
    let lo = o - 0.5 * hw;
    let hi = o + 0.5 * hw;
    g.c0 = ov(-0.5 * p, 0.5 * p, lo, hi);
    let c1a = ov(0.5 * p, 1.5 * p, lo, hi);
    let c1b = ov(-1.5 * p, -0.5 * p, lo, hi);
    g.c2 = ov(1.5 * p, 2.5 * p, lo, hi) + ov(-2.5 * p, -1.5 * p, lo, hi);
    g.c1 = c1a + c1b;
    g.c1w = max(c1a, c1b);
    return g;
}

// spacing, gap, gain of drum d, head h
fn hd(d: u32, h: u32) -> vec3<f32> {
    let i = W_HEADS + i32(2u * d + h) * 3;
    return vec3<f32>(cf(i), cf(i + 1), cf(i + 2));
}

// (a, b) of the own kernel (xy) and of the neighbour's (zw)
fn kernel_ab(g: Geo) -> vec4<f32> {
    let p = hd(1u, g.head);
    let r = hd(0u, g.head);
    let x = hd(0u, 1u - g.head);
    return vec4<f32>(
        max(r.x + p.x, 0.001), max(p.y, 0.05),
        max(x.x + p.x, 0.001), max(p.y + g.c1w * cf(W_AZ), 0.05));
}

fn lor_j(t: f32, a: f32) -> f32 {
    return (t * atan2_rad(t, a) - 0.5 * a * log(a * a + t * t)) * (1.0 / 3.1415927);
}

fn tap(k: i32, a: f32, b: f32) -> f32 {
    let t = f32(k);
    let hb = 0.5 * b;
    return ((lor_j(t + 0.5 + hb, a) - lor_j(t - 0.5 + hb, a)) - (lor_j(t + 0.5 - hb, a) - lor_j(t - 0.5 - hb, a))) / b;
}

fn lt_scalar(y: u32, x: i32) -> f32 {
    let g = row_geo(y);
    let pgain = hd(1u, g.head).z;
    let rgain = hd(0u, g.head).z;
    let xgain = hd(0u, 1u - g.head).z;
    let frame = cw(W_FRAME);
    switch x {
        case 0: { return f32(g.content); }
        case 1: { return f32(g.content ^ 1u); }
        case 2: { return pgain * rgain * (g.c0 + g.c2); }
        case 3: { return pgain * xgain * g.c1; }
        case 4: { return sqrt(cf(W_VAR_TAPE) * (g.c0 + g.c1 + g.c2) * (pgain * pgain) + cf(W_VAR_AMP)); }
        case 5, 6: {
            let h = hash3(frame, y, 0x0D0Du);
            if (unit(h) < cf(W_P_DROP)) {
                if (x == 5) { return unit(hsh(h ^ 1u)) * f32(cw(W_W)); }
                return cf(W_DROP_LEN) * (0.5 + unit(hsh(h ^ 2u)));
            }
            return -1.0;
        }
        case 7: {
            var e = 0.0;
            if ((cw(W_FLAGS) & F_TBC) == 0u) {
                let fl = f32(g.field);
                let yf = f32(g.yf);
                let drum = cf(W_PH_DRUM) + cf(W_DRUM_FIELD) * fl + cf(W_DRUM_LINE) * yf;
                let flut = cf(W_PH_FLUT) + cf(W_FLUT_FIELD) * fl + cf(W_FLUT_LINE) * yf;
                let rnd = gauss(hash3(frame, y, 0x7BEu));
                e = cf(W_JITTER) * (0.6 * sin_cyc(drum) + 0.5 * sin_cyc(flut) + 0.6 * rnd);
                if (g.switched) { e = e + cf(W_SWITCH_SKEW); }
            }
            return e;
        }
        case 8: {
            let hf = f32(cw(W_H) / 2u);
            let at = hf - cf(W_SWITCH_POINT);
            if (f32(g.yf) == floor(at)) { return (at - floor(at)) * f32(cw(W_W)); }
            return -1.0;
        }
        case 9: {
            let ab = kernel_ab(g);
            let fcu = cf(W_FCU_N);
            var t = 0.0;
            for (var k = 0; k < KN; k = k + 1) {
                t = t + tap(k - KR, ab.x, ab.y) * cos_cyc(fcu * f32(k - KR));
            }
            let gc = max(pgain * rgain * (g.c0 + g.c2) * t, 1.0 / 6.0);
            return 1.0 / (gc * cf(W_A_C));
        }
        case 10: {
            if (g.head == 0u) { return cf(W_ROT_A); }
            return cf(W_ROT_B);
        }
        case 11: {
            if (cw(W_V_SWITCH) != 0u && ((g.content >> 1u) & 1u) == 1u) { return -1.0; }
            return 1.0;
        }
        case 12: { return f32(g.content >> 1u); }
        default: { return 0.0; }
    }
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let x = i32(floor(pos.x));
    let y = u32(floor(pos.y));
    if (x < 16) {
        return putf(lt_scalar(y, x));
    }
    let g = row_geo(y);
    let ab = kernel_ab(g);
    let k = x - 16;
    if (k < KN) {
        return putf(tap(k - KR, ab.x, ab.y));
    }
    return putf(tap(k - KN - KR, ab.z, ab.w));
}

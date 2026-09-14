// Слитый гео-проход (Р.44, конкатенация трансформов): зеркало
// kt_fx::geom (map_pos + sample_at + ss_taps). Программа примитивов
// приезжает юниформой 1024 Б (свой bind group layout — общая Params на
// 128 Б мала); коэффициенты ПЕРЕМНОЖЕНЫ на CPU в f64, здесь —
// однократное округление f32. Правишь семантику — правь kt-fx/src/geom.rs
// ТЕМ ЖЕ КОММИТОМ (t181).
//
// Примитивы (kind по 4 бита в h.y|h.z, данные — vec4-слоты подряд):
//   0 Affine: (ax, bx, ay, by)                          — 1 слот
//   1 Clamp:  (x0, x1, y0, y1), выключенная ось ±1e30   — 1 слот
//   2 Fold:   (gw, gh, cw, ch) + (in_w, in_h, 0, 0)     — 2 слота;
//     вне [0,gw)×[0,gh) — чёрный непрозрачный, внутри — тайл-локальная
//     фаза `(r / cell) * in − 0.5` (дословно старое ядро event_grid).
//   3 Proj:   три строки обратной гомографии Pan3d      — 3 слота;
//     вход — центр-space; t.z ≤ 0 (за камерой) и uv вне [0,1]² — прозрачно.
//
// Tap-таблица (Р.45, п.312): поглощённые поточечные опы (Greenscreen/
// Noise) — слоты 46+2i (meta|params, params2), счётчик в h.w битах 24..31;
// применяются к значению сабтапа в порядке цепочки, координата стадии
// записывается обратным проходом (зеркало kt_fx::geom::map_pos_rec).
//
// Суперсемплинг (п.309): h.w = mode | nx<<8 | ny<<16. mode 0 — тапы
// посчитаны на CPU (аффинная программа, якобиан константен); mode 1 —
// по-пиксельно конечными разностями, ФОРМУЛА тапов та же, что в kt-fx
// (`ss_taps`: квант 1/32, включение строго при len > 1 + 1/64, потолок 4).

struct GeomParams {
    // x: число примитивов, y|z: kinds (4 бита на примитив, до 16),
    // w: суперсемплинг (mode | nx<<8 | ny<<16)
    h: vec4<u32>,
    // src_w, src_h (входной кадр серии), out_w, out_h
    d: vec4<u32>,
    p: array<vec4<f32>, 62>,
};

@group(0) @binding(0) var<uniform> G: GeomParams;
@group(0) @binding(1) var T0: texture_2d<u32>;

// Fullscreen-треугольник — как в common.wgsl.
@vertex
fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(vi / 2u)) * 4.0 - 1.0;
    let y = f32(i32(vi & 1u)) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// Обратная цепочка примитивов: (x, y, dead), dead: 0 — жив,
// 1 — чёрный непрозрачный (вне сетки Fold), 2 — прозрачный (Proj).
fn geo_map(px: f32, py: f32) -> vec3<f32> {
    var x = px;
    var y = py;
    var dead = 0.0;
    var cur = 0u;
    let n = G.h.x;
    for (var i = 0u; i < n; i = i + 1u) {
        let kind = (select(G.h.z, G.h.y, i < 8u) >> ((i % 8u) * 4u)) & 0xfu;
        if (kind == 0u) { // Affine
            let a = G.p[cur];
            cur = cur + 1u;
            if (dead == 0.0) {
                x = a.x * x + a.y;
                y = a.z * y + a.w;
            }
        } else if (kind == 1u) { // Clamp («плато» Pan при zoom<1/Stretch)
            let c = G.p[cur];
            cur = cur + 1u;
            if (dead == 0.0) {
                x = clamp(x, c.x, c.y);
                y = clamp(y, c.z, c.w);
            }
        } else if (kind == 2u) { // Fold (EventGrid)
            let g = G.p[cur];
            let s = G.p[cur + 1u];
            cur = cur + 2u;
            if (dead == 0.0) {
                if (x < 0.0 || y < 0.0 || x >= g.x || y >= g.y) {
                    dead = 1.0;
                } else {
                    let kx = floor(x / g.z);
                    x = (x - kx * g.z) / g.z * s.x - 0.5;
                    let ky = floor(y / g.w);
                    y = (y - ky * g.w) / g.w * s.y - 0.5;
                }
            }
        } else if (kind == 3u) { // Proj (Pan3d): вход — центр-space, зеркало kt_fx::geom
            let r0 = G.p[cur];
            let r1 = G.p[cur + 1u];
            let r2 = G.p[cur + 2u];
            cur = cur + 3u;
            if (dead == 0.0) {
                let tx = r0.x * x + r0.y * y + r0.z;
                let ty = r1.x * x + r1.y * y + r1.z;
                let tz = r2.x * x + r2.y * y + r2.z;
                if (tz <= 0.0) { // за камерой (клиппинг растеризатора)
                    dead = 2.0;
                } else {
                    let qx = tx / tz;
                    let qy = ty / tz;
                    if (qx < 0.0 || qx > 1.0 || qy < 0.0 || qy > 1.0) {
                        dead = 2.0; // вне квада — прозрачно
                    } else {
                        x = qx;
                        y = qy;
                    }
                }
            }
        }
    }
    return vec3<f32>(x, y, dead);
}

// Финальная билинейка БЕЗ округления — дословно как resample.wgsl.
fn bilin(mx: f32, my: f32) -> vec4<f32> {
    let sw = i32(G.d.x);
    let sh = i32(G.d.y);
    let xf = floor(mx);
    let yf = floor(my);
    let fx = mx - xf;
    let fy = my - yf;
    let xi0 = i32(xf);
    let yi0 = i32(yf);
    var acc = vec4<f32>(0.0);
    for (var dy = 0; dy < 2; dy = dy + 1) {
        let wy = select(fy, 1.0 - fy, dy == 0);
        let yi = clamp(yi0 + dy, 0, sh - 1);
        for (var dx = 0; dx < 2; dx = dx + 1) {
            let wx = select(fx, 1.0 - fx, dx == 0);
            let xi = clamp(xi0 + dx, 0, sw - 1);
            acc = acc + vec4<f32>(textureLoad(T0, vec2<i32>(xi, yi), 0)) * (wy * wx);
        }
    }
    return acc;
}

// Сабтап без округления: короткие замыкания либо билинейка.
fn geo_tap(m: vec3<f32>) -> vec4<f32> {
    if (m.z == 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 255.0);
    }
    if (m.z == 2.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    return bilin(m.x, m.y);
}

// lowbias32 — копия noise.wgsl (зеркало kt_fx::video::noise_hash).
fn nh(v: u32) -> u32 {
    var x = v;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

// Офсет зерна общей формулы noise (целочисленное деление — как i64 в Rust).
fn noff(gx: u32, gy: u32, ch: u32, seed: u32, k: i32) -> f32 {
    let h = nh(gx ^ nh(gy ^ nh(ch ^ seed)));
    let n = i32((h >> 16u) & 0x7fffu) - 16384;
    return f32(n * k / 32767);
}

// meta i-го tap-опа: kind | after_prim<<2 | color<<7 | special<<8.
fn tap_meta(i: u32) -> u32 {
    return bitcast<u32>(G.p[46u + i * 2u].x);
}

// Сабтап С ПОГЛОЩЁННЫМИ поточечными опами (Р.45, п.312): обратный проход
// записывает координаты tap-стадий, после билинейки опы применяются к
// значению в порядке цепочки. Зеркало kt_fx::geom::{map_pos_rec,sample_at}.
fn geo_sample(px: f32, py: f32) -> vec4<f32> {
    let tap_count = (G.h.w >> 24u) & 0xffu;
    var rec: array<vec2<f32>, 8>;
    var x = px;
    var y = py;
    var dead = 0.0;
    var cur = 0u;
    // tap'ы лежат с УБЫВАЮЩИМИ after_prim — курсор идёт с конца списка
    var tcur = tap_count;
    loop {
        if (tcur == 0u) { break; }
        if (((tap_meta(tcur - 1u) >> 4u) & 0x1fu) != 0u) { break; }
        rec[tcur - 1u] = vec2<f32>(x, y);
        tcur = tcur - 1u;
    }
    var applied = 0u;
    let n = G.h.x;
    for (var i = 0u; i < n; i = i + 1u) {
        let kind = (select(G.h.z, G.h.y, i < 8u) >> ((i % 8u) * 4u)) & 0xfu;
        if (kind == 0u) {
            let a = G.p[cur];
            cur = cur + 1u;
            if (dead == 0.0) {
                x = a.x * x + a.y;
                y = a.z * y + a.w;
            }
        } else if (kind == 1u) {
            let c = G.p[cur];
            cur = cur + 1u;
            if (dead == 0.0) {
                x = clamp(x, c.x, c.y);
                y = clamp(y, c.z, c.w);
            }
        } else if (kind == 2u) {
            let g = G.p[cur];
            let sdim = G.p[cur + 1u];
            cur = cur + 2u;
            if (dead == 0.0) {
                if (x < 0.0 || y < 0.0 || x >= g.x || y >= g.y) {
                    dead = 1.0;
                } else {
                    let kx = floor(x / g.z);
                    x = (x - kx * g.z) / g.z * sdim.x - 0.5;
                    let ky = floor(y / g.w);
                    y = (y - ky * g.w) / g.w * sdim.y - 0.5;
                }
            }
        } else if (kind == 3u) {
            let r0 = G.p[cur];
            let r1 = G.p[cur + 1u];
            let r2 = G.p[cur + 2u];
            cur = cur + 3u;
            if (dead == 0.0) {
                let tx = r0.x * x + r0.y * y + r0.z;
                let ty = r1.x * x + r1.y * y + r1.z;
                let tz = r2.x * x + r2.y * y + r2.z;
                if (tz <= 0.0) {
                    dead = 2.0;
                } else {
                    let qx = tx / tz;
                    let qy = ty / tz;
                    if (qx < 0.0 || qx > 1.0 || qy < 0.0 || qy > 1.0) {
                        dead = 2.0;
                    } else {
                        x = qx;
                        y = qy;
                    }
                }
            }
        }
        if (dead == 0.0) {
            applied = i + 1u;
            loop {
                if (tcur == 0u) { break; }
                if (((tap_meta(tcur - 1u) >> 4u) & 0x1fu) != applied) { break; }
                rec[tcur - 1u] = vec2<f32>(x, y);
                tcur = tcur - 1u;
            }
        }
    }
    var v: vec4<f32>;
    if (dead == 1.0) {
        v = vec4<f32>(0.0, 0.0, 0.0, 255.0);
    } else if (dead == 2.0) {
        v = vec4<f32>(0.0);
    } else {
        v = bilin(x, y);
    }
    // опы в порядке цепочки; при коротком замыкании на приме d живы опы
    // с after_prim <= d
    for (var t = 0u; t < tap_count; t = t + 1u) {
        let tm = tap_meta(t);
        if (((tm >> 4u) & 0x1fu) > applied) { continue; }
        let a = G.p[46u + t * 2u];
        let b = G.p[47u + t * 2u];
        let tap_kind = tm & 0xfu;
        if (tap_kind == 0u) { // Greenscreen (континуум формулы ядра)
            let key = a.yzw;
            let tol = b.x;
            let hard = b.y;
            let spill = b.z;
            let kc = u32(b.w);
            let dd = distance(v.xyz, key);
            if (dd <= hard) {
                v.w = 0.0;
            } else if (dd < tol) {
                v.w = v.w * (dd - hard) / max(tol - hard, 1e-9);
            }
            if (spill > 0.0 && v.w > 0.0) {
                var vc = 0.0;
                var o1 = 0.0;
                var o2 = 0.0;
                if (kc == 0u) { vc = v.x; o1 = v.y; o2 = v.z; }
                else if (kc == 1u) { vc = v.y; o1 = v.z; o2 = v.x; }
                else { vc = v.z; o1 = v.x; o2 = v.y; }
                let other = max(o1, o2);
                if (vc > other) {
                    vc = vc - (vc - other) * spill;
                    if (kc == 0u) { v.x = vc; }
                    else if (kc == 1u) { v.y = vc; }
                    else { v.z = vc; }
                }
            }
        } else if (tap_kind == 2u) { // ColorGrade: зеркало tap_grade
            let br = a.y;
            let con = a.z;
            let sat = a.w;
            let hc = b.x;
            let hs = b.y;
            let l = (77.0 * v.x + 150.0 * v.y + 29.0 * v.z) / 256.0;
            v = vec4<f32>(vec3<f32>(l) + (v.xyz - vec3<f32>(l)) * sat, v.w);
            if (hc != 1.0 || hs != 0.0) {
                let kk = 1.0 / sqrt(3.0);
                let dotv = kk * (v.x + v.y + v.z);
                let cx = kk * (v.y - v.z);
                let cy = kk * (v.z - v.x);
                let cz = kk * (v.x - v.y);
                v = vec4<f32>(
                    v.x * hc + cx * hs + kk * dotv * (1.0 - hc),
                    v.y * hc + cy * hs + kk * dotv * (1.0 - hc),
                    v.z * hc + cz * hs + kk * dotv * (1.0 - hc),
                    v.w,
                );
            }
            v = vec4<f32>((v.xyz - vec3<f32>(128.0)) * (1.0 + con) + vec3<f32>(128.0 + br * 255.0), v.w);
        } else if (tap_kind == 3u) { // Duotone: зеркало tap_duo
            let sh3 = a.yzw;
            let hi3 = b.xyz;
            let am = b.w;
            let l = clamp((77.0 * v.x + 150.0 * v.y + 29.0 * v.z) / 256.0 / 255.0, 0.0, 1.0);
            let outc = sh3 + (hi3 - sh3) * l;
            v = vec4<f32>(v.xyz + (outc - v.xyz) * am, v.w);
        } else if (tap_kind == 4u) { // Posterize: зеркало tap_poster
            let n = a.y;
            let idx3 = clamp(floor(v.xyz * n / 256.0), vec3<f32>(0.0), vec3<f32>(n - 1.0));
            v = vec4<f32>(idx3 * 255.0 / (n - 1.0), v.w);
        } else if (tap_kind == 5u) { // Vignette: зеркало tap_vig
            let st = a.y;
            let ra = a.z;
            let so = a.w;
            let sw = b.x;
            let sh1 = b.y;
            let c = rec[t];
            let ddx = (c.x + 0.5) / max(sw, 1.0) - 0.5;
            let ddy = (c.y + 0.5) / max(sh1, 1.0) - 0.5;
            let r = sqrt((2.0 * ddx) * (2.0 * ddx) + (2.0 * ddy) * (2.0 * ddy)) / sqrt(2.0);
            let tt = clamp((r - ra) / max(so, 1e-6), 0.0, 1.0);
            let ww = tt * tt * (3.0 - 2.0 * tt);
            v = vec4<f32>(v.xyz * (1.0 - st * ww), v.w);
        } else if (tap_kind == 1u) { // Noise: зерно прибито к сетке своей стадии
            let seed = bitcast<u32>(a.y);
            let k = i32(bitcast<u32>(a.z));
            let cell = u32(a.w);
            let sw = i32(b.x);
            let sh = i32(b.y);
            let c = rec[t];
            let xi = u32(clamp(i32(floor(c.x)), 0, sw - 1));
            let yi = u32(clamp(i32(floor(c.y)), 0, sh - 1));
            if (((tm >> 10u) & 1u) == 1u) { // особая формула size==1 && !color
                let hsh = nh(xi ^ nh(yi ^ seed));
                let nn = i32((hsh >> 16u) & 0x7fffu) - 16384;
                let off = f32(nn * k / 32767);
                v = vec4<f32>(v.xyz + vec3<f32>(off), v.w);
            } else {
                let gx = xi / cell;
                let gy = yi / cell;
                if (((tm >> 9u) & 1u) == 1u) { // цветное зерно
                    v = vec4<f32>(
                        v.x + noff(gx, gy, 1u, seed, k),
                        v.y + noff(gx, gy, 2u, seed, k),
                        v.z + noff(gx, gy, 3u, seed, k),
                        v.w,
                    );
                } else {
                    v = vec4<f32>(v.xyz + vec3<f32>(noff(gx, gy, 0u, seed, k)), v.w);
                }
            }
        } else {
            // неизвестный вид тапа: значение не трогается (замок Кв1)
        }
    }
    return v;
}

// Число сабтапов на ось — ФОРМУЛА kt_fx::geom::ss_taps один в один.
fn ss_taps(len: f32) -> u32 {
    let q = floor(len * 32.0 + 0.5) / 32.0;
    return clamp(u32(ceil(q)), 1u, 4u);
}

// Длина образа единичного шага оси: мёртвая прямая проба заменяется
// ОБРАТНОЙ (зеркало kt_fx::geom::probe_taps — иначе кромка тождественной
// карты получала бы лишние тапы); обе мертвы/мёртв центр — один тап.
fn probe_axis(c0: vec3<f32>, ox: f32, oy: f32, dx: f32, dy: f32) -> f32 {
    if (c0.z != 0.0) {
        return 1.0;
    }
    let fwd = geo_map(ox + dx, oy + dy);
    if (fwd.z == 0.0) {
        return length(c0.xy - fwd.xy);
    }
    let back = geo_map(ox - dx, oy - dy);
    if (back.z == 0.0) {
        return length(c0.xy - back.xy);
    }
    return 1.0;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let o = floor(pos.xy);
    let mode = G.h.w & 0xffu;
    var nx = (G.h.w >> 8u) & 0xffu;
    var ny = (G.h.w >> 16u) & 0xffu;
    if (mode == 1u) { // Proj-серия: якобиан по-пиксельно
        let c0 = geo_map(o.x, o.y);
        nx = ss_taps(probe_axis(c0, o.x, o.y, 1.0, 0.0));
        ny = ss_taps(probe_axis(c0, o.x, o.y, 0.0, 1.0));
    }
    let tap_count = (G.h.w >> 24u) & 0xffu;
    var acc = vec4<f32>(0.0);
    for (var j = 0u; j < ny; j = j + 1u) {
        let sy = o.y - 0.5 + (f32(j) + 0.5) / f32(ny);
        for (var i = 0u; i < nx; i = i + 1u) {
            let sx = o.x - 0.5 + (f32(i) + 0.5) / f32(nx);
            if (tap_count == 0u) {
                // путь Р.44 — без поглощённых опов, байт-в-байт прежний
                acc = acc + geo_tap(geo_map(sx, sy));
            } else {
                acc = acc + geo_sample(sx, sy);
            }
        }
    }
    acc = acc / f32(nx * ny);
    return vec4<u32>(clamp(round(acc), vec4<f32>(0.0), vec4<f32>(255.0)));
}

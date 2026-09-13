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
        } else { // Proj (Pan3d): вход — центр-space, зеркало kt_fx::geom
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

// Сабтап БЕЗ округления — финальная билинейка дословно как resample.wgsl.
fn geo_tap(m: vec3<f32>) -> vec4<f32> {
    if (m.z == 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 255.0);
    }
    if (m.z == 2.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let sw = i32(G.d.x);
    let sh = i32(G.d.y);
    let xf = floor(m.x);
    let yf = floor(m.y);
    let fx = m.x - xf;
    let fy = m.y - yf;
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
    var acc = vec4<f32>(0.0);
    for (var j = 0u; j < ny; j = j + 1u) {
        let sy = o.y - 0.5 + (f32(j) + 0.5) / f32(ny);
        for (var i = 0u; i < nx; i = i + 1u) {
            let sx = o.x - 0.5 + (f32(i) + 0.5) / f32(nx);
            acc = acc + geo_tap(geo_map(sx, sy));
        }
    }
    acc = acc / f32(nx * ny);
    return vec4<u32>(clamp(round(acc), vec4<f32>(0.0), vec4<f32>(255.0)));
}

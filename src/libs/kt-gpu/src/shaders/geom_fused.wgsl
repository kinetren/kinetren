// Слитый гео-проход (Р.44, конкатенация трансформов): зеркало
// kt_fx::geom (map_pos + sample_bilinear). Программа примитивов приезжает
// юниформой 1024 Б (свой bind group layout — общая Params на 128 Б мала);
// коэффициенты ПЕРЕМНОЖЕНЫ на CPU в f64, здесь — однократное округление
// f32. Правишь семантику — правь kt-fx/src/geom.rs ТЕМ ЖЕ КОММИТОМ (t181).
//
// Примитивы (kind по 4 бита в h.y|h.z, данные — vec4-слоты подряд):
//   0 Affine: (ax, bx, ay, by)                          — 1 слот
//   1 Clamp:  (x0, x1, y0, y1), выключенная ось ±1e30   — 1 слот
//   2 Fold:   (gw, gh, cw, ch) + (in_w, in_h, 0, 0)     — 2 слота;
//     вне [0,gw)×[0,gh) — чёрный непрозрачный, внутри — тайл-локальная
//     фаза `(r / cell) * in − 0.5` (дословно старое ядро event_grid).

struct GeomParams {
    // x: число примитивов, y|z: kinds (4 бита на примитив, до 16), w: флаги
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

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let o = floor(pos.xy);
    var x = o.x;
    var y = o.y;
    var dead = 0u; // 1 = чёрный непрозрачный (вне сетки Fold)
    var cur = 0u;  // курсор vec4-слотов; зависит только от юниформы и i
    let n = G.h.x;
    for (var i = 0u; i < n; i = i + 1u) {
        let kind = (select(G.h.z, G.h.y, i < 8u) >> ((i % 8u) * 4u)) & 0xfu;
        if (kind == 0u) { // Affine
            let a = G.p[cur];
            cur = cur + 1u;
            if (dead == 0u) {
                x = a.x * x + a.y;
                y = a.z * y + a.w;
            }
        } else if (kind == 1u) { // Clamp («плато» Pan при zoom<1/Stretch)
            let c = G.p[cur];
            cur = cur + 1u;
            if (dead == 0u) {
                x = clamp(x, c.x, c.y);
                y = clamp(y, c.z, c.w);
            }
        } else { // Fold (EventGrid)
            let g = G.p[cur];
            let s = G.p[cur + 1u];
            cur = cur + 2u;
            if (dead == 0u) {
                if (x < 0.0 || y < 0.0 || x >= g.x || y >= g.y) {
                    dead = 1u;
                } else {
                    let kx = floor(x / g.z);
                    x = (x - kx * g.z) / g.z * s.x - 0.5;
                    let ky = floor(y / g.w);
                    y = (y - ky * g.w) / g.w * s.y - 0.5;
                }
            }
        }
    }
    if (dead == 1u) {
        return vec4<u32>(0u, 0u, 0u, 255u);
    }
    // финальный тап — дословно resample.wgsl (зеркало bilinear_into)
    let sw = i32(G.d.x);
    let sh = i32(G.d.y);
    let xf = floor(x);
    let yf = floor(y);
    let fx = x - xf;
    let fy = y - yf;
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
    return vec4<u32>(clamp(round(acc), vec4<f32>(0.0), vec4<f32>(255.0)));
}

//! The compute probe of F-EXT-004 (`--ignored`, a measurement, not a check): the Datamosh encoder —
//! motion estimation, prediction and the quantised DCT residual — as plain WGSL compute shaders, in
//! a closed loop over a moving 1080p picture, against `fx_video_codec::codec::encode` on the CPU.
//! The search is the same diamond, made data-parallel: two sweeps, the second taking the first's
//! neighbouring vectors, instead of the CPU's left/top order.
//! `cargo test --release -p kt-gpu --test compute_mosh -- --ignored --nocapture`.

use fx_video_codec::codec::{self, Cfg, Planes};
use kt_gpu::GpuContext;

const ME: &str = r#"
struct U { w: u32, h: u32, bs: u32, r: i32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> cur: array<u32>;
@group(0) @binding(2) var<storage, read> rf: array<u32>;
@group(0) @binding(3) var<storage, read> seed: array<vec2<i32>>;
@group(0) @binding(4) var<storage, read_write> mv: array<vec2<i32>>;

fn sad(bx: i32, by: i32, v: vec2<i32>, limit: u32) -> u32 {
    var s = 0u;
    let w = i32(u.w);
    let h = i32(u.h);
    for (var y = 0; y < i32(u.bs); y++) {
        let sy = clamp(by + y + v.y, 0, h - 1);
        for (var x = 0; x < i32(u.bs); x++) {
            let sx = clamp(bx + x + v.x, 0, w - 1);
            let a = i32(cur[(by + y) * w + bx + x]);
            let b = i32(rf[sy * w + sx]);
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
"#;

const DCT: &str = r#"
struct U { w: u32, h: u32, bs: u32, shift: u32, q: f32, mw: u32, intra: u32, pad: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> src: array<u32>;
@group(0) @binding(2) var<storage, read> rf: array<u32>;
@group(0) @binding(3) var<storage, read> mv: array<vec2<i32>>;
@group(0) @binding(4) var<storage, read_write> recon: array<u32>;

fn round_away(v: f32) -> f32 { return sign(v) * floor(abs(v) + 0.5); }

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let bw = u.w / 8u;
    let bh = u.h / 8u;
    if id.x >= bw || id.y >= bh { return; }
"#;

fn main_dct() -> String {
    // the basis as a private table: the same numbers `codec::basis` holds
    let mut c = String::from("var<private> C: array<f32, 64> = array<f32, 64>(");
    for u in 0..8 {
        let k = if u == 0 { 0.353_553_390_593_273_73f64 } else { 0.5 };
        for x in 0..8 {
            c += &format!("{:.9},", k * (((2 * x + 1) * u) as f64 * std::f64::consts::PI / 16.0).cos());
        }
    }
    c += ");\n";
    let body = r#"
    let ox = i32(id.x * 8u);
    let oy = i32(id.y * 8u);
    let w = i32(u.w);
    let h = i32(u.h);
    var pred: array<f32, 64>;
    var b: array<f32, 64>;
    for (var y = 0; y < 8; y++) {
        for (var x = 0; x < 8; x++) {
            let px = ox + x;
            let py = oy + y;
            var p = 128.0;
            if u.intra == 0u {
                let pbs = i32(u.bs >> u.shift);
                let at = u32(py / pbs) * u.mw + u32(px / pbs);
                let v = mv[at] >> vec2<u32>(u.shift);
                let sx = clamp(px + v.x, 0, w - 1);
                let sy = clamp(py + v.y, 0, h - 1);
                p = f32(rf[sy * w + sx]);
            }
            pred[y * 8 + x] = p;
            b[y * 8 + x] = f32(src[py * w + px]) - p;
        }
    }
    var t: array<f32, 64>;
    var f: array<f32, 64>;
    for (var y = 0; y < 8; y++) { for (var c0 = 0; c0 < 8; c0++) {
        var s = 0.0;
        for (var x = 0; x < 8; x++) { s += C[c0 * 8 + x] * b[y * 8 + x]; }
        t[y * 8 + c0] = s;
    } }
    for (var c0 = 0; c0 < 8; c0++) { for (var v = 0; v < 8; v++) {
        var s = 0.0;
        for (var y = 0; y < 8; y++) { s += C[v * 8 + y] * t[y * 8 + c0]; }
        f[v * 8 + c0] = round_away(s / u.q) * u.q;
    } }
    for (var c0 = 0; c0 < 8; c0++) { for (var y = 0; y < 8; y++) {
        var s = 0.0;
        for (var v = 0; v < 8; v++) { s += C[v * 8 + y] * f[v * 8 + c0]; }
        t[y * 8 + c0] = s;
    } }
    for (var y = 0; y < 8; y++) { for (var x = 0; x < 8; x++) {
        var s = 0.0;
        for (var c0 = 0; c0 < 8; c0++) { s += C[c0 * 8 + x] * t[y * 8 + c0]; }
        let r = clamp(round_away(s), -512.0, 512.0);
        recon[(oy + y) * w + ox + x] = u32(clamp(pred[y * 8 + x] + r, 0.0, 255.0));
    } }
}
"#;
    format!("{c}{DCT}{body}")
}

fn hash(x: i32, y: i32) -> f64 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0xC2B2_AE3D);
    ((h ^ (h >> 13)) & 0xffff) as f64 / 65535.0
}

/// Smooth noise of three octaves: a picture with texture, so the search has work.
fn noise(x: f64, y: f64) -> f64 {
    let mut s = 0.0;
    for (cell, amp) in [(64.0, 0.6), (17.0, 0.3), (5.0, 0.1)] {
        let (fx, fy) = (x / cell, y / cell);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (tx, ty) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
        let at = |dx, dy| hash(ix as i32 + dx, iy as i32 + dy);
        let top = at(0, 0) + (at(1, 0) - at(0, 0)) * tx;
        let bot = at(0, 1) + (at(1, 1) - at(0, 1)) * tx;
        s += amp * (top + (bot - top) * ty);
    }
    s
}

/// Frame `k`: a background panning one way, a large box of other texture moving the other way.
fn picture(w: u32, h: u32, k: i32) -> Vec<u8> {
    let mut out = vec![255u8; (w * h * 4) as usize];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let in_box = x > 400 + 5 * k && x < 1300 + 5 * k && y > 200 && y < 800;
            let (px, py) = if in_box { (x - 5 * k + 5000, y + 3 * k) } else { (x + 3 * k, y - k) };
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            for c in 0..3 {
                out[i + c] = (noise(px as f64 + 900.0 * c as f64, py as f64) * 255.0) as u8;
            }
        }
    }
    out
}

fn psnr(a: &Planes, b: &Planes) -> f64 {
    let mse: f64 = a.y.iter().zip(&b.y).map(|(p, q)| (*p as f64 - *q as f64).powi(2)).sum::<f64>() / a.y.len() as f64;
    10.0 * (255.0 * 255.0 / mse.max(1e-9)).log10()
}

struct Plane {
    w: u32,
    h: u32,
    shift: u32,
    src: wgpu::Buffer,
    rf: wgpu::Buffer,
    recon: wgpu::Buffer,
    staging: wgpu::Buffer,
    uni: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

fn u32s(v: &[u8]) -> Vec<u8> {
    v.iter().flat_map(|b| (*b as u32).to_le_bytes()).collect()
}

fn layout(d: &wgpu::Device) -> wgpu::BindGroupLayout {
    let e = |b, ty| wgpu::BindGroupLayoutEntry {
        binding: b,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    };
    let ro = wgpu::BufferBindingType::Storage { read_only: true };
    let rw = wgpu::BufferBindingType::Storage { read_only: false };
    d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[e(0, wgpu::BufferBindingType::Uniform), e(1, ro), e(2, ro), e(3, ro), e(4, rw)],
    })
}

fn pipeline(d: &wgpu::Device, l: &wgpu::BindGroupLayout, text: &str) -> wgpu::ComputePipeline {
    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(text.into()),
    });
    let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(l)],
        immediate_size: 0,
    });
    d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pl),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

#[test]
#[ignore]
fn bench_mosh_encoder_compute() {
    let ctx = GpuContext::new_headless().expect("adapter");
    let i = &ctx.adapter_info;
    println!("adapter {} / {:?} / {:?}", i.name, i.backend, i.device_type);
    let (d, q) = (&ctx.device, &ctx.queue);
    let (w, h, n) = (1920u32, 1080u32, 12);
    let cfg = Cfg { bs: 16, search: 16, qp: 24.0 };
    let (pw, ph) = (codec::pad16(w), codec::pad16(h));
    let planes: Vec<Planes> = (0..n).map(|k| Planes::from_rgba(&picture(w, h, k), w, h, pw, ph)).collect();

    // the CPU, as shipped: one thread
    let mut cpu_ms = Vec::new();
    let mut prev: Option<codec::Clean> = None;
    let mut cpu_recon = Vec::new();
    for p in &planes {
        let t = std::time::Instant::now();
        let c = codec::encode(p, prev.as_ref().map(|c| &c.recon), &cfg);
        cpu_ms.push(t.elapsed().as_secs_f64() * 1e3);
        cpu_recon.push(c.recon.clone());
        prev = Some(c);
    }

    let l = layout(d);
    let me = pipeline(d, &l, ME);
    let dct = pipeline(d, &l, &main_dct());
    let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
    let buf = |size: u64, usage| d.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage, mapped_at_creation: false });
    let (bw, bh) = (pw / cfg.bs, ph / cfg.bs);
    let mv_size = (bw * bh * 8) as u64;
    let (mv0, mv1) = (buf(mv_size, storage), buf(mv_size, storage));
    let zero = buf(mv_size, storage);
    let me_uni = buf(16, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
    let words: Vec<u8> = [pw as u32, ph as u32, cfg.bs as u32, cfg.search as u32].iter().flat_map(|v| v.to_le_bytes()).collect();
    q.write_buffer(&me_uni, 0, &words);

    let qstep = cfg.qstep() as f32;
    let plane = |pw: u32, ph: u32, shift: u32| {
        let size = (pw * ph * 4) as u64;
        let (src, rf, recon) = (buf(size, storage), buf(size, storage), buf(size, storage));
        let staging = buf(size, wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
        let uni = buf(32, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &l,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uni.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: src.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: rf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: mv1.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: recon.as_entire_binding() },
            ],
        });
        Plane { w: pw, h: ph, shift, src, rf, recon, staging, uni, bind }
    };
    let ps = [plane(pw as u32, ph as u32, 0), plane(pw as u32 / 2, ph as u32 / 2, 1), plane(pw as u32 / 2, ph as u32 / 2, 1)];
    let me_bind = |seed: &wgpu::Buffer, out: &wgpu::Buffer| {
        d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &l,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: me_uni.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: ps[0].src.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: ps[0].rf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: seed.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: out.as_entire_binding() },
            ],
        })
    };
    let (sweep1, sweep2) = (me_bind(&zero, &mv0), me_bind(&mv0, &mv1));

    let mut gpu_ms = Vec::new();
    let mut gpu_psnr = Vec::new();
    for (k, p) in planes.iter().enumerate() {
        let t = std::time::Instant::now();
        let intra = k == 0;
        for (pl, data) in ps.iter().zip([&p.y, &p.cb, &p.cr]) {
            q.write_buffer(&pl.src, 0, &u32s(data));
            let mut u = Vec::new();
            for v in [pl.w, pl.h, cfg.bs as u32, pl.shift] {
                u.extend(v.to_le_bytes());
            }
            u.extend(qstep.to_le_bytes());
            for v in [bw as u32, intra as u32, 0] {
                u.extend(v.to_le_bytes());
            }
            q.write_buffer(&pl.uni, 0, &u);
        }
        let mut enc = d.create_command_encoder(&Default::default());
        {
            let mut cp = enc.begin_compute_pass(&Default::default());
            if !intra {
                cp.set_pipeline(&me);
                for b in [&sweep1, &sweep2] {
                    cp.set_bind_group(0, b, &[]);
                    cp.dispatch_workgroups((bw as u32).div_ceil(8), (bh as u32).div_ceil(8), 1);
                }
            }
            cp.set_pipeline(&dct);
            for pl in &ps {
                cp.set_bind_group(0, &pl.bind, &[]);
                cp.dispatch_workgroups((pl.w / 8).div_ceil(8), (pl.h / 8).div_ceil(8), 1);
            }
        }
        for pl in &ps {
            let size = (pl.w * pl.h * 4) as u64;
            enc.copy_buffer_to_buffer(&pl.recon, 0, &pl.rf, 0, size);
            enc.copy_buffer_to_buffer(&pl.recon, 0, &pl.staging, 0, size);
        }
        q.submit([enc.finish()]);
        let mut out = Vec::new();
        for pl in &ps {
            let s = pl.staging.slice(..);
            s.map_async(wgpu::MapMode::Read, |r| r.unwrap());
            d.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let bytes: Vec<u8> = s.get_mapped_range().unwrap().chunks_exact(4).map(|c| c[0]).collect();
            pl.staging.unmap();
            out.push(bytes);
        }
        gpu_ms.push(t.elapsed().as_secs_f64() * 1e3);
        let recon = Planes { w: p.w, h: p.h, y: out[0].clone(), cb: out[1].clone(), cr: out[2].clone() };
        gpu_psnr.push(psnr(&recon, p));
    }
    let avg = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!("CPU encode, one thread: P-frames {:.1} ms, I-frame {:.1} ms", avg(&cpu_ms[1..]), cpu_ms[0]);
    println!("compute with readback: P-frames {:.1} ms, I-frame {:.1} ms", avg(&gpu_ms[1..]), gpu_ms[0]);
    let cpu_q: Vec<f64> = cpu_recon.iter().zip(&planes).map(|(a, b)| psnr(a, b)).collect();
    println!("luma PSNR over P-frames: CPU {:.2} dB, compute {:.2} dB", avg(&cpu_q[1..]), avg(&gpu_psnr[1..]));
}

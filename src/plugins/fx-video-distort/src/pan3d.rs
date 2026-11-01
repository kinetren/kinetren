//! Pan3d's FORWARD homography (R.29, R.44), moved verbatim from kt-fx `geom.rs` (plugin-api-stage1,
//! 4.1c-4): the world/camera semantics are frozen by the file format, the identity at the defaults
//! is byte for byte (t298), and t187 runs it on both executors every time.

use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::geo::Camera3d;

// ---- Pan3d: homography (R.44) ----------------------------------------------
// The layer quad is planar (z = 0) and z_clip ≡ 0, therefore the mapping
// (q.x, q.y) → screen is a pure 3×3 homography. The math VERBATIM repeats the
// former pan3d_mvp (glam f32) + the rasterizer viewport, but is computed in f64;
// the world/camera semantics are frozen by the file format (kt-gpu/CLAUDE.md):
// world — project px × world_scale, X right, Y down, Z into depth, rotations
// Rz·Rx·Ry, camera “50 mm” at auto-distance −(H/2)·cot(fov/2).

type V3 = [f64; 3];
type M3 = [[f64; 3]; 3];

fn m3_mul(a: &M3, b: &M3) -> M3 {
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    r
}

fn cross(a: &V3, b: &V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: &V3, b: &V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn rot_z(a: f64) -> M3 {
    let (s, c) = a.fsin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

fn rot_x(a: f64) -> M3 {
    let (s, c) = a.fsin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

fn rot_y(a: f64) -> M3 {
    let (s, c) = a.fsin_cos();
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}

/// FORWARD homography of Pan3d: (q.x, q.y, 1) → (c.x·w, c.y·w, w), where q is the uv
/// of the quad 0..1, c is the CENTER-space of the output pixels.
pub(crate) fn pan3d_forward(
    c3: &Camera3d,
    out: (u32, u32),
    pos: [f64; 3],
    rot: [f64; 3],
    scale: f64,
    anchor: [f64; 2],
    quad: (f64, f64),
) -> M3 {
    let s = c3.world_scale;
    let cam = c3;
    let (ow, oh) = (out.0 as f64, out.1 as f64);
    let sc = scale.max(1e-6);

    // model: world(q) = pos·s + R·(sc·lx, sc·ly, 0), lx = (q.x − ax)·qw
    let r = m3_mul(
        &m3_mul(&rot_z(rot[2].to_radians()), &rot_x(rot[0].to_radians())),
        &rot_y(rot[1].to_radians()),
    );
    let col = |j: usize| -> V3 { [r[0][j], r[1][j], r[2][j]] };
    let (c0, c1) = (col(0), col(1));
    let k1 = sc * quad.0;
    let k2 = sc * quad.1;
    let e1: V3 = [c0[0] * k1, c0[1] * k1, c0[2] * k1];
    let e2: V3 = [c1[0] * k2, c1[1] * k2, c1[2] * k2];
    let e0: V3 = [
        pos[0] * s - anchor[0] * e1[0] - anchor[1] * e2[0],
        pos[1] * s - anchor[0] * e1[1] - anchor[1] * e2[1],
        pos[2] * s - anchor[0] * e1[2] - anchor[1] * e2[2],
    ];

    // camera: auto-distance “50 mm” — the layer at z=0 renders 1:1
    let fov = cam.fov_deg.clamp(1.0, 179.0).to_radians();
    let f = 1.0 / (fov * 0.5).ftan();
    let auto_z = -(oh * 0.5) * f;
    let eye: V3 =
        [cam.pos_x * s, cam.pos_y * s, if cam.pos_z == 0.0 { auto_z } else { cam.pos_z * s }];
    // view: at the target (two-node) or along +Z; screen up in the world is −Y
    let dir: V3 = match cam.target {
        Some((tx, ty, tz)) => {
            let d = [tx * s - eye[0], ty * s - eye[1], tz * s - eye[2]];
            let len2 = dot(&d, &d);
            if len2 < 1e-9 {
                [0.0, 0.0, 1.0]
            } else {
                let l = len2.sqrt();
                [d[0] / l, d[1] / l, d[2] / l]
            }
        }
        None => [0.0, 0.0, 1.0],
    };
    let up: V3 = [0.0, -1.0, 0.0];
    let rt = cross(&dir, &up);
    let rl2 = dot(&rt, &rt);
    let right: V3 = if rl2 > 1e-24 {
        let l = rl2.sqrt();
        [rt[0] / l, rt[1] / l, rt[2] / l]
    } else {
        [1.0, 0.0, 0.0]
    };
    let cdown = cross(&dir, &right);
    // basis rows: right / cdown / dir; the orientation rotation is AFTER the target
    let basis: M3 = [right, cdown, dir];
    let orient = m3_mul(
        &m3_mul(&rot_z(-cam.rot_z.to_radians()), &rot_x(-cam.rot_x.to_radians())),
        &rot_y(-cam.rot_y.to_radians()),
    );
    let view = m3_mul(&orient, &basis);
    // directions: v = view·e; point: v0 = view·(e0 − eye)
    let mv = |e: &V3| -> V3 {
        [
            view[0][0] * e[0] + view[0][1] * e[1] + view[0][2] * e[2],
            view[1][0] * e[0] + view[1][1] * e[1] + view[1][2] * e[2],
            view[2][0] * e[0] + view[2][1] * e[1] + view[2][2] * e[2],
        ]
    };
    let v1 = mv(&e1);
    let v2 = mv(&e2);
    let d0: V3 = [e0[0] - eye[0], e0[1] - eye[1], e0[2] - eye[2]];
    let v0 = mv(&d0);

    // projection: perspective x = f/aspect·v.x, y = −f·v.y, w = v.z;
    // ortho — affine (w of points = 1, of directions = 0)
    let aspect = ow / oh;
    let cl = |v: &V3, point: bool| -> V3 {
        if cam.ortho {
            [2.0 / ow * v[0], -2.0 / oh * v[1], if point { 1.0 } else { 0.0 }]
        } else {
            [f / aspect * v[0], -f * v[1], v[2]]
        }
    };
    let p1 = cl(&v1, false);
    let p2 = cl(&v2, false);
    let p0 = cl(&v0, true);

    // screen: c.x·w = ow/2·(clip.x + w); c.y·w = oh/2·(w − clip.y)
    [
        [ow * 0.5 * (p1[0] + p1[2]), ow * 0.5 * (p2[0] + p2[2]), ow * 0.5 * (p0[0] + p0[2])],
        [oh * 0.5 * (p1[2] - p1[1]), oh * 0.5 * (p2[2] - p2[1]), oh * 0.5 * (p0[2] - p0[1])],
        [p1[2], p2[2], p0[2]],
    ]
}

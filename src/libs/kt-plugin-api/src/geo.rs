//! The geometry helpers a geometry plugin and the host share (plugin-api-stage1, D11.2): ONE f64
//! text of each, so a plugin's homography and the core's are inverted by the same arithmetic.

/// A 3×3 matrix, rows first.
pub type M3 = [[f64; 3]; 3];

/// The degenerate inverse homography: every point is behind the camera, the layer is transparent.
pub const TRANSPARENT: M3 = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]];

/// Inversion of the 3×3 via adj/det. None — degeneracy (quad edge-on to the camera).
/// The sign of t.z of the inverse image under adj/det is correct: t = G⁻¹·(c,1) = q̃/w,
/// i.e. sign(t.z) = sign(w) — the “behind the camera” test comes out by itself.
pub fn invert3(g: &M3) -> Option<M3> {
    let adj = [
        [
            g[1][1] * g[2][2] - g[1][2] * g[2][1],
            g[0][2] * g[2][1] - g[0][1] * g[2][2],
            g[0][1] * g[1][2] - g[0][2] * g[1][1],
        ],
        [
            g[1][2] * g[2][0] - g[1][0] * g[2][2],
            g[0][0] * g[2][2] - g[0][2] * g[2][0],
            g[0][2] * g[1][0] - g[0][0] * g[1][2],
        ],
        [
            g[1][0] * g[2][1] - g[1][1] * g[2][0],
            g[0][1] * g[2][0] - g[0][0] * g[2][1],
            g[0][0] * g[1][1] - g[0][1] * g[1][0],
        ],
    ];
    let det = g[0][0] * adj[0][0] + g[0][1] * adj[1][0] + g[0][2] * adj[2][0];
    // the determinant scale of our matrices is ~ (ow·oh·f)² — the threshold is relative
    let scale = g.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs()));
    if det.abs() <= scale * scale * scale * 1e-12 {
        return None;
    }
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = adj[i][j] / det;
        }
    }
    Some(m)
}

/// The layer's resolved 3D camera (R.29), already interpolated at the frame's time: the fields of
/// the model's camera, frozen by the file format, plus the world scale (quality px / project px —
/// the preview is smaller than the project, and the Pan3d/camera coordinates are project px).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Camera3d {
    pub pos_x: f64,
    pub pos_y: f64,
    /// 0 = “auto” (default distance 50 mm).
    pub pos_z: f64,
    /// Point of interest (two-node camera, as in AE). None — one-node: looks along +Z.
    pub target: Option<(f64, f64, f64)>,
    /// Extra rotation AFTER target (AE orientation), degrees, ZXY.
    pub rot_x: f64,
    pub rot_y: f64,
    pub rot_z: f64,
    /// Vertical FOV in degrees.
    pub fov_deg: f64,
    pub ortho: bool,
    pub world_scale: f64,
}

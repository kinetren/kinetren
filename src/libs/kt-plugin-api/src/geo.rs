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

//! Test 761 (R.64): placement of the leading “Geometry” node and the inverse
//! recomputation of the on-screen gesture.
//!
//! The owner (R.63 item 6): “AT MINIMUM we must have convenient editing right
//! in the preview”. The on-screen frame must be computed by THE SAME function that
//! builds the render's geo program, and the gesture must return values giving exactly
//! the requested shift. A separate check for the ANCHOR: it moves, the picture
//! stays — and on a rotated layer too, where After Effects has this broken
//! (quotes in `docs/features/ui/viewer/on-screen-controls.md`).

use kt_fx::geom::{
    Handle, Placement, anchor_move, crop_for_handle, geometry_placement, pos_for_drag,
    rot_for_point, zoom_for_handle,
};
use kt_fx::conform::{ConformRule, LeadCtx};
use kt_model::{EventGeometry, FxInstance};

const CANVAS: (f64, f64) = (1920.0, 1080.0);
/// The source view is vertical: “fit” gets margins, and the frame stops
/// matching the canvas (otherwise the test is green on nothing).
const VIEW: (f64, f64) = (1080.0, 1920.0);

fn node(g: EventGeometry) -> FxInstance {
    g.node()
}

fn lead() -> Option<LeadCtx> {
    Some(LeadCtx::new(Some(VIEW), ConformRule::Fit))
}

fn place(g: &EventGeometry) -> Placement {
    geometry_placement(&node(*g), lead(), VIEW, CANVAS).expect("geometry node")
}

fn near(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} against {b} (tolerance {tol})");
}

fn corners_near(a: [(f64, f64); 4], b: [(f64, f64); 4], tol: f64, what: &str) {
    for i in 0..4 {
        assert!(
            (a[i].0 - b[i].0).abs() <= tol && (a[i].1 - b[i].1).abs() <= tol,
            "{what}: corner {i} moved — {:?} against {:?}",
            a[i],
            b[i]
        );
    }
}

/// Test 761 (R.64): placement and the inverse recomputations.
#[test]
fn t761_placement_and_inverse_gestures() {
    // --- the FRAME equals the analytical one ------------------------------------
    let g0 = EventGeometry::default();
    let p0 = place(&g0);
    // “fit” of a vertical kind into 16:9 — the frame by height, margins at the sides
    near(p0.dh, 1080.0, 1e-9, "height of frame “fit”");
    near(p0.dw, 1080.0 * VIEW.0 / VIEW.1, 1e-9, "width of frame “fit”");
    near(p0.ox + p0.dw / 2.0, CANVAS.0 / 2.0, 1e-9, "frame at the canvas centre");
    assert_eq!(p0.anchor, (p0.ox + p0.dw / 2.0, p0.oy + p0.dh / 2.0), "the default anchor is the centre");

    // --- SHIFT: a shift in canvas px gives exactly itself --------------------
    let want = (137.0, -42.0);
    let pos = pos_for_drag((0.0, 0.0), CANVAS, want);
    let moved = place(&EventGeometry { pos_x: pos.0, pos_y: pos.1, ..g0 });
    near(moved.ox - p0.ox, want.0, 1e-9, "translate along X");
    near(moved.oy - p0.oy, want.1, 1e-9, "translate along Y");

    // --- SCALE: the opposite corner stayed in place ----------------
    for h in [Handle::TopLeft, Handle::BottomRight, Handle::Right, Handle::Top] {
        let from = h.point(&p0);
        let fixed_before = {
            // the same point that `opposite` returns — we take it via the gesture below
            let cs = p0.corners();
            match h {
                Handle::TopLeft => cs[2],
                Handle::BottomRight => cs[0],
                Handle::Right => ((cs[3].0 + cs[0].0) / 2.0, (cs[3].1 + cs[0].1) / 2.0),
                Handle::Top => ((cs[2].0 + cs[3].0) / 2.0, (cs[2].1 + cs[3].1) / 2.0),
                _ => unreachable!(),
            }
        };
        // we drag the handle one and a half times farther from the fixed point
        let target = (
            fixed_before.0 + (from.0 - fixed_before.0) * 1.5,
            fixed_before.1 + (from.1 - fixed_before.1) * 1.5,
        );
        let (z, pos) = zoom_for_handle(&p0, h, target, g0.zoom, (g0.pos_x, g0.pos_y), CANVAS)
            .expect("scale handle");
        near(z, 1.5, 1e-9, "zoom after dragging by 1.5");
        let after = place(&EventGeometry { zoom: z, pos_x: pos.0, pos_y: pos.1, ..g0 });
        let fixed_after = {
            let cs = after.corners();
            match h {
                Handle::TopLeft => cs[2],
                Handle::BottomRight => cs[0],
                Handle::Right => ((cs[3].0 + cs[0].0) / 2.0, (cs[3].1 + cs[0].1) / 2.0),
                Handle::Top => ((cs[2].0 + cs[3].0) / 2.0, (cs[2].1 + cs[3].1) / 2.0),
                _ => unreachable!(),
            }
        };
        near(fixed_after.0, fixed_before.0, 1e-6, "the fixed point moved along X");
        near(fixed_after.1, fixed_before.1, 1e-6, "the fixed point moved along Y");
        // and the handle itself arrived where it was dragged
        let now = h.point(&after);
        near(now.0, target.0, 1e-6, "the handle did not reach along X");
        near(now.1, target.1, 1e-6, "the handle did not reach along Y");
    }

    // --- ROTATION: angle from the anchor, step with Shift --------------------------
    let a = p0.anchor;
    // a point strictly to the right of the anchor is a +90° rotation (zero is “up”)
    let deg = rot_for_point(&p0, (a.0 + 100.0, a.1), None);
    near(deg, 90.0, 1e-9, "the angle by the point on the right");
    let snapped = rot_for_point(&p0, (a.0 + 100.0, a.1 + 7.0), Some(15.0));
    near(snapped, 90.0, 1e-9, "a 15° step rounded 94° to 90°");

    // --- ANCHOR: values change, PICTURE STAYS ---------------------
    for rot in [0.0, 30.0, -137.0] {
        let g = EventGeometry { rot, ..g0 };
        let pl = place(&g);
        let before = pl.corners();
        // drag the anchor to the top-left corner of the frame (in its own system)
        let target = {
            let cs = before;
            cs[0]
        };
        let (na, npos) = anchor_move(&pl, target, (g.pos_x, g.pos_y), CANVAS);
        assert!(
            (na.0 - g.anchor_x).abs() > 0.1 || (na.1 - g.anchor_y).abs() > 0.1,
            "the anchor did not change — the gesture did nothing: {na:?}"
        );
        let after = place(&EventGeometry {
            anchor_x: na.0,
            anchor_y: na.1,
            pos_x: npos.0,
            pos_y: npos.1,
            ..g
        });
        corners_near(after.corners(), before, 1e-6, &format!("rotation {rot}°: the picture moved away"));
        // …and the anchor is NOW at the point it was dragged to
        near(after.anchor.0, target.0, 1e-6, "the anchor is not where it was dragged (X)");
        near(after.anchor.1, target.1, 1e-6, "the anchor is not where it was dragged (Y)");
    }

    // --- CROP: the handle cuts its own side, the frame follows the rule ---------
    let pl = place(&g0);
    let left = Handle::CropLeft.point(&pl);
    let target = (left.0 + pl.dw * 0.25, left.1);
    let crop = crop_for_handle(&pl, Handle::CropLeft, target, [0.0; 4]).expect("crop from the left");
    near(crop[0], 0.25, 1e-9, "crop from the left");
    assert_eq!(&crop[1..], &[0.0, 0.0, 0.0], "the crop touched the other sides");
    let after = place(&EventGeometry { crop_l: crop[0], ..g0 });
    assert!(after.dw < pl.dw, "the frame did not narrow after the crop: {} against {}", after.dw, pl.dw);
    // mutual limit: the view cannot be eaten away entirely
    assert!(
        crop_for_handle(&pl, Handle::CropLeft, (left.0 + pl.dw * 0.99, left.1), [0.0, 0.0, 0.9, 0.0])
            .is_none(),
        "the crop ate the whole view — the limit did not trigger"
    );
}

/// Test 782 (R.66): the OUTWARD NORMAL of a handle is the direction in which it protrudes.
///
/// A small frame (the “as is” rule with a small source, or a stub
/// instead of unavailable media) is fully covered by grab zones: on the owner’s
/// `paintitblack.otio` it came out 45×25 px, and there was nothing to grab the body by.
/// Handles protrude outward along these normals, so the normals must
/// be perpendicular to their own edges and rotate TOGETHER with the frame.
#[test]
fn t782_handle_outward_normals_follow_the_frame() {
    let g0 = EventGeometry::default();
    let pl = place(&g0);

    // without rotation: the sides give the axes, the corners give the diagonals
    let unit = |h: Handle| pl_outward(&pl, h);
    near(unit(Handle::Top).1, -1.0, 1e-9, "top faces up");
    near(unit(Handle::Bottom).1, 1.0, 1e-9, "bottom faces down");
    near(unit(Handle::Left).0, -1.0, 1e-9, "left faces left");
    near(unit(Handle::Right).0, 1.0, 1e-9, "right faces right");
    for h in [Handle::TopLeft, Handle::TopRight, Handle::BottomRight, Handle::BottomLeft] {
        let (x, y) = unit(h);
        near((x * x + y * y).sqrt(), 1.0, 1e-9, "the corner normal is not unit length");
    }

    // ROTATION: the normals move together with the frame
    for rot in [30.0, -137.0] {
        let p = place(&EventGeometry { rot, ..g0 });
        let top = pl_outward(&p, Handle::Top);
        let want = (rot.to_radians().sin(), -rot.to_radians().cos());
        near(top.0, want.0, 1e-6, &format!("rotation {rot}°: the top normal along X"));
        near(top.1, want.1, 1e-6, &format!("rotation {rot}°: the top normal along Y"));
        // and it is still perpendicular to the top edge
        let cs = p.corners();
        let edge = (cs[1].0 - cs[0].0, cs[1].1 - cs[0].1);
        let dot = edge.0 * top.0 + edge.1 * top.1;
        near(dot / (edge.0 * edge.0 + edge.1 * edge.1).sqrt(), 0.0, 1e-6, "the normal is not perpendicular to the edge");
    }

    // handles that do not move from their place have no normal
    for h in [Handle::Rot, Handle::Anchor, Handle::CropLeft, Handle::CropTop] {
        assert!(h.outward(&pl).is_none(), "{h:?} does not move — it must have no normal");
    }
}

fn pl_outward(pl: &Placement, h: Handle) -> (f64, f64) {
    h.outward(pl).unwrap_or_else(|| panic!("{h:?} must have a normal"))
}

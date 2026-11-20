//! gen-shapes (`v26.10` G5, the shapes and plates group): polygon, star, arrow, border frame,
//! lower-third plate, progress bar. Test t3709; the contract — `openspec/specs/generators/spec.md`.

use kt_media::generator::GeneratorSource;
use kt_media::{MediaSource, Quality};
use kt_model::{GeneratorKind, HasParams};
use kt_time::Tick;

const W: u32 = 160;
const H: u32 = 90;

fn frame(k: &GeneratorKind, t: i64) -> Vec<u8> {
    let mut src = GeneratorSource::new(k.clone());
    src.read_video_frame(Tick(t), Some(Quality { width: W, height: H })).expect("a frame").rgba.to_vec()
}

fn a(f: &[u8], x: u32, y: u32) -> u8 {
    f[((y * W + x) * 4 + 3) as usize]
}

/// Test 3709: each shape is drawn where its numbers say over a transparent frame; the star's valleys
/// are empty where the polygon is full; the plate's reveal hides its right part; the progress bar fills
/// with the material time.
#[test]
fn t3709_the_shapes_and_plates_stand_where_their_rows_say() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let k = |tag: &str| GeneratorKind::default_for_tag(tag).expect(tag);
    for tag in ["polygon", "star", "arrow", "frame", "plate", "progress"] {
        // the progress bar's fill is empty at 0: it is read half way through its fill time
        let f = frame(&k(tag), if tag == "progress" { 5 * 90_000 } else { 0 });
        assert!(f.chunks(4).any(|p| p[3] > 200), "{tag}: draws nothing");
        assert!(f.chunks(4).any(|p| p[3] == 0), "{tag}: is not over a transparent frame");
    }
    // a hexagon is full at its centre; a 5-point star with a deep valley is empty halfway to a valley
    let poly = frame(&k("polygon"), 0);
    assert!(a(&poly, W / 2, H / 2) > 200, "the polygon's centre");
    let mut star = k("star");
    star.set_param("inner", 0.2);
    let s = frame(&star, 0);
    assert!(a(&s, W / 2, H / 2) > 200, "the star's centre");
    // a valley of the 5-point star (tip up) lies at 36° from the top; halfway out it is empty
    let r = 0.25 * H as f64 * 0.7;
    let (vx, vy) = ((W as f64 / 2.0 + r * 36f64.to_radians().sin()) as u32, (H as f64 / 2.0 - r * 36f64.to_radians().cos()) as u32);
    assert_eq!(a(&s, vx, vy), 0, "the star's valley is filled");
    assert!(a(&poly, vx, vy) > 200, "the polygon is not full there");
    // the plate revealed to half: its right end is not drawn
    let mut half = k("plate");
    half.set_param("reveal", 0.5);
    let full = frame(&k("plate"), 0);
    let hf = frame(&half, 0);
    let y = (0.81 * H as f64) as u32;
    let right = (0.5 * W as f64) as u32;
    assert!(a(&full, right, y) > 100 && a(&hf, right, y) < 20, "the reveal does not hide the plate's right part");
    // the progress bar at a quarter of its fill time: filled up to a quarter
    let pr = k("progress");
    let q = frame(&pr, (10 * 90_000) / 4);
    let yb = (0.906 * H as f64) as u32;
    let at = |f: &[u8], frac: f64| a(f, (W as f64 * (0.1 + 0.8 * frac)) as u32, yb);
    assert!(at(&q, 0.1) > 200 && at(&q, 0.6) < 100, "the bar is not filled to a quarter: {} {}", at(&q, 0.1), at(&q, 0.6));
    assert!(at(&frame(&pr, 10 * 90_000), 0.95) > 200, "the bar is not full at its fill time");
}

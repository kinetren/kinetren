//! The bytes path shows a stored moment as the display path does (disk-bytes-once, found-defects 774).
//! t3356.

use kt_engine::Player;
use kt_engine::render_cache::Config;
use kt_media::{MediaSource, Quality};
use kt_model::demo::{DemoSources, demo_project};
use kt_time::Tick;
use std::time::{Duration, Instant};

/// A player over the demo project with the tier at `root`, the GL preview's engine (off the window's
/// device: it keeps its bytes-path composites), and six moments.
fn player(root: &std::path::Path) -> Option<(Player, Vec<Tick>)> {
    kt_plugins_builtin::install();
    let gpu = kt_gpu::test_engine()?;
    gpu.keep_composites_for_bytes();
    let mut p = demo_project(DemoSources::Synth);
    for c in &mut p.clips {
        c.opacity = 0.67; // a picture no other test presents on the shared engine
    }
    let sources: Vec<(kt_model::MediaId, Box<dyn MediaSource>)> =
        p.media.iter().map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap())).collect();
    let (player, _ring) = Player::new(p, sources);
    player.set_preview_quality(Quality { width: 160, height: 90 });
    player.set_video_gpu(Some(gpu));
    player.render_cache().set_heavy_for_tests(Some(1));
    player.set_render_cache(Some(Config {
        codec: std::sync::Arc::new(kt_engine::render_cache::RawSegments),
        root: root.to_path_buf(),
        build: "test build".into(),
        base: None,
        cap: 1 << 30,
    }));
    let grid = player.video_grid();
    let start = grid.align_floor(Tick(4 * 90_000));
    let moments = (0..6).map(|i| Tick(start.0 + i * grid.frame_step())).collect();
    Some((player, moments))
}

/// Test t3356 (disk-bytes-once D2, found-defects 774): after a restart, a moment the disk tier holds,
/// composed to bytes ten times as a loop does, is read from the store ONCE — restored into the
/// executor's kept memo, then read back from it — and every output is the stored picture.
#[test]
fn t3356_a_disk_moment_is_restored_once_on_the_bytes_path() {
    let root = tempfile::tempdir().unwrap();
    let Some((a, moments)) = player(root.path()) else { return };
    for t in &moments {
        a.note_preview_cost(*t, 1_000_000_000, (160, 90));
    }
    a.render_cache().kick();
    let deadline = kt_time::test_deadline();
    while !moments.iter().all(|t| a.disk_picture(*t).is_some()) {
        assert!(Instant::now() < deadline, "the prerender never wrote the moments");
        std::thread::sleep(Duration::from_millis(20));
    }
    let t = moments[2];
    let stored = a.disk_picture(t).unwrap().rgba.to_vec();
    drop(a);
    // the restart: nothing is kept; the prerender may not write while this runs
    let Some((b, _)) = player(root.path()) else { return };
    b.render_cache().set_heavy_for_tests(Some(u64::MAX));
    let before = b.render_cache().counts().1;
    let mut buf = Vec::new();
    for lap in 0..10 {
        let (_, complete) = b.compose_preview_into(t, &mut buf);
        assert!(complete, "lap {lap}: the stored moment is complete");
        assert!(buf == stored, "lap {lap}: the bytes are the stored picture");
    }
    assert_eq!(b.render_cache().counts().1 - before, 1, "the store was read once over ten laps");
}

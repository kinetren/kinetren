//! Временный генератор перф-демо: реальный h264+aac клип, loop-регион.
use kt_model::*;
use kt_time::Tick;

fn main() {
    let mut a = std::env::args().skip(1);
    let media_path = a.next().unwrap();
    let out = a.next().unwrap();
    let mut p = Project::new(ProjectSettings { fps: 30, width: 1280, height: 720, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: media_path });
    for (id, kind) in [(1u64, ClipKind::Video), (2, ClipKind::Audio)] {
        p.clips.push(ClipEvent {
            id: ClipId(id), link_group: Some(GroupId(1)), kind, lane: 0,
            start: Tick(0), duration: Tick(90_000 * 2), source_in: Tick(0),
            media: MediaId(1), gain_db: 0.0, reversed: false, fx: vec![], fx_bypass: vec![],
        });
    }
    p.region = Some(Region { start: Tick(0), end: Tick(90_000 * 2) });
    p.loop_enabled = true;
    p.validate().unwrap();
    std::fs::write(out, p.to_json()).unwrap();
}

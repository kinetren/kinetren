//! Copies of heavy sources (persistent-cache D3). t3440, t3441, t3442. Skipped without FFmpeg, its binary
//! (the fixtures) or libx264.
#![cfg(feature = "ffmpeg")]

use kt_media::cache_folder::Folder;
use kt_media::copy::ffmpeg::FfmpegCopies;
use kt_media::copy::{self, CopyError, CopyMaker, CopySource, Go, Mismatch};
use kt_media::ffmpeg::FfmpegSource;
use kt_media::{MediaSource, Quality};
use kt_time::Tick;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ready() -> bool {
    let ok = kt_ffmpeg::rescan().is_ok() && Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success());
    if !ok {
        eprintln!("no FFmpeg: skipped");
    }
    ok
}

fn copies() -> FfmpegCopies {
    FfmpegCopies { forced: Some("libx264".into()) }
}

fn make_with(dir: &Path, name: &str, args: &[&str]) -> Option<PathBuf> {
    let out = dir.join(name);
    let r = Command::new("ffmpeg").args(["-hide_banner", "-loglevel", "error", "-y"]).args(args).arg(&out).output().unwrap();
    if !r.status.success() {
        eprintln!("{name}: {}", String::from_utf8_lossy(&r.stderr));
        return None;
    }
    Some(out)
}

/// 23.976 fps, 10-bit HEVC, 5.1 sound.
fn ntsc(dir: &Path) -> Option<PathBuf> {
    make_with(
        dir,
        "ntsc.mp4",
        &[
            "-f", "lavfi", "-i", "testsrc2=s=320x180:r=24000/1001:d=2", "-f", "lavfi", "-i", "sine=f=440:r=48000:d=2", "-pix_fmt", "yuv420p10le", "-c:v",
            "libx265", "-x265-params", "log-level=none:keyint=24", "-c:a", "aac", "-ac", "6",
        ],
    )
}

/// A variable rate: frames dropped at uneven places, their times kept.
fn vfr(dir: &Path) -> Option<PathBuf> {
    make_with(
        dir,
        "vfr.mp4",
        &[
            "-f", "lavfi", "-i", "testsrc2=s=320x180:r=30:d=3", "-f", "lavfi", "-i", "sine=f=330:r=48000:d=3", "-vf",
            "select='not(eq(mod(n,7),3)+eq(mod(n,11),5))'", "-fps_mode", "vfr", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-g", "30", "-c:a", "aac",
        ],
    )
}

/// The pictures' times, in ticks from the first, and which are keyframes.
fn times(path: &Path) -> (Vec<i128>, bool) {
    let mut s = kt_ffmpeg::Source::open(&path.to_string_lossy()).unwrap();
    let (n, d) = s.video_time_base().unwrap();
    let mut out = Vec::new();
    while let Some((pts, _)) = s.next_video(64, 36).unwrap() {
        // a common base: exact
        out.push(pts as i128 * n as i128 * (1 << 40) / d as i128);
    }
    let keys = kt_media::muxer::probe_output(&path.to_string_lossy()).unwrap().packets.iter().all(|p| p.2);
    (out, keys)
}

/// Test t3440 (persistent-cache D3): a copy of a 23.976 fps 10-bit HEVC file with 5.1 sound and of a
/// variable-rate one has every picture at the original's time, all keyframes; read through `CopySource` it
/// answers the original's duration and native size, the same picture at every tick of two project grids
/// (within the codec's tolerance), the original's sound byte for byte, and lies in the cache folder.
#[test]
fn t3440_a_copy_of_fractional_and_variable_rates_is_the_original() {
    if !ready() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let folder = Folder::open(&dir.path().join("kinetren-cache")).unwrap();
    let mut done = 0;
    for file in [ntsc(dir.path()), vfr(dir.path())].into_iter().flatten() {
        let c = copies();
        let f = c.facts(&folder, &file).expect("facts");
        assert!(f.copyable && f.long_gop, "{f:?}");
        let size = f.size_at(2);
        let rel = copy::copy_rel(f.id, size);
        let dest = folder.root().join(&rel);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        let made = match c.make(&file, &dest, size, &mut |_| Go::On) {
            Ok(m) => m,
            Err(CopyError::NoEncoder(why)) => {
                eprintln!("t3440: {why}: skipped");
                return;
            }
            Err(e) => panic!("{e:?}"),
        };
        assert_eq!(copy::copies_of(&folder, f.id), [size]);
        let ((a, long), (b, keys)) = (times(&file), times(&dest));
        assert!(!long && keys, "every frame of the copy is a keyframe, the original's are not");
        assert_eq!(made.frames as usize, a.len());
        assert_eq!(a, b, "{}: the pictures' own times", file.display());

        let mut orig = FfmpegSource::open(&file.to_string_lossy()).unwrap();
        let mut src = CopySource::new(
            Box::new(FfmpegSource::open(&file.to_string_lossy()).unwrap()),
            Box::new(FfmpegSource::open(&dest.to_string_lossy()).unwrap()),
            None,
        );
        assert_eq!((src.info().duration, src.info().width, src.info().height), (orig.info().duration, 320, 180));
        assert_ne!(src.identity(), orig.identity(), "another name in the memory and on disk");
        let q = Some(Quality { width: 80, height: 44 });
        for step in [3600, 3000, 3754] {
            for i in 0..40 {
                let t = Tick(i * step);
                let (x, y) = (orig.read_video_frame(t, q).unwrap(), src.read_video_frame(t, q).unwrap());
                let mean = x.rgba.iter().zip(y.rgba.iter()).map(|(a, b)| a.abs_diff(*b) as u64).sum::<u64>() as f64 / x.rgba.len() as f64;
                assert!(mean < 6.0, "{} at {t:?}: another picture, {mean:.1} apart", file.display());
            }
        }
        for i in 0..90 {
            assert_eq!(orig.read_audio_burst(i).ok(), src.read_audio_burst(i).ok(), "the sound is the original's, burst {i}");
        }
        done += 1;
    }
    assert!(done > 0, "no fixture was made");
}

/// Test t3441 (persistent-cache D3): a job stopped halfway leaves no file; one told to wait writes nothing
/// until it is let on, and then ends as it would have.
#[test]
fn t3441_stop_and_wait_leave_no_trace() {
    if !ready() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let Some(file) = vfr(dir.path()) else { return };
    let dest = dir.path().join("copy.mov");
    let c = copies();
    let r = c.make(&file, &dest, (160, 90), &mut |n| if n < 30 { Go::On } else { Go::Stop });
    if matches!(r, Err(CopyError::NoEncoder(_))) {
        return;
    }
    assert_eq!(r, Err(CopyError::Stopped));
    assert!(!dest.exists(), "a stopped job left its file");
    // ten asks of a pause: nothing is written meanwhile
    let mut waits = 0;
    let mut seen = Vec::new();
    let r = c.make(&file, &dest, (160, 90), &mut |n| {
        if n == 10 && waits < 10 {
            waits += 1;
            seen.push(n);
            return Go::Wait;
        }
        Go::On
    });
    assert_eq!((seen, waits), (vec![10; 10], 10));
    assert_eq!(r.unwrap().frames as usize, times(&dest).0.len());
}

/// Test t3442 (persistent-cache D3): a camera's copy a second shorter, or with another number of frames, is
/// refused and what differs is named; a matching one is accepted and bound.
#[test]
fn t3442_a_mismatched_camera_copy_is_refused_with_its_reason() {
    if !ready() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let gen_clip = |name: &str, rate: &str, secs: &str| {
        make_with(
            dir.path(),
            name,
            &[
                "-f", "lavfi", "-i", &format!("testsrc2=s=160x90:r={rate}:d={secs}"), "-f", "lavfi", "-i", &format!("sine=f=440:r=48000:d={secs}"),
                "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac",
            ],
        )
    };
    let (Some(orig), Some(short), Some(other), Some(good)) =
        (gen_clip("a.mp4", "25", "4"), gen_clip("short.mp4", "25", "3"), gen_clip("rate.mp4", "30", "4"), gen_clip("good.mp4", "25", "4"))
    else {
        return;
    };
    let c = copies();
    assert!(matches!(c.check_camera(&orig, &short), Err(Mismatch::Duration(a, b)) if a > b + 0.9));
    assert_eq!(c.check_camera(&orig, &other), Err(Mismatch::Frames(100, 120)));
    assert_eq!(c.check_camera(&orig, &good), Ok(()));
    assert!(matches!(c.check_camera(&orig, &dir.path().join("none.mp4")), Err(Mismatch::Unreadable(_))));
    let folder = Folder::open(&dir.path().join("kinetren-cache")).unwrap();
    let id = c.facts(&folder, &orig).unwrap().id;
    assert_eq!(copy::camera_copy(&folder, id), None);
    copy::bind_camera(&folder, id, &good).unwrap();
    assert_eq!(copy::camera_copy(&folder, id), Some(good.clone()));
    std::fs::remove_file(&good).unwrap();
    assert_eq!(copy::camera_copy(&folder, id), None, "a file that went is not bound");
}

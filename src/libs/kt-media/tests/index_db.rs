//! Tests 15–16: peak pyramid and the media index's invalidation, over the files of a cache folder.

use kt_media::index::{ContentId, MediaEntryStatus, MediaStore, PeakPyramid, ReelStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::placeholder::PlaceholderSource;
use kt_media::{MediaSource, PEAK_BLOCK_FRAMES};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

fn synth(seconds: i64, pattern: AudioPattern) -> SyntheticSource {
    SyntheticSource::new(Tick(90_000 * seconds), pattern)
}

// ---- Test 15: peak pyramid -------------------------------------------------

#[test]
fn t15_pyramid_matches_brute_force() {
    let mut src = synth(2, AudioPattern::Sine { hz: 997.0 });
    let pyr = PeakPyramid::build(&mut src);

    // a naive recomputation of all samples
    let frames = Tick(90_000 * 2).to_audio_frames();
    let bursts = frames / AUDIO_BURST_FRAMES as i64;
    let usable = (bursts * AUDIO_BURST_FRAMES as i64) as usize;
    let mut all = vec![0i16; usable * AUDIO_CHANNELS];
    let mut check = synth(2, AudioPattern::Sine { hz: 997.0 });
    for b in 0..bursts {
        let burst = check.read_audio_burst(b).unwrap();
        for (i, v) in burst.data.iter().enumerate() {
            all[b as usize * AUDIO_BURST_FRAMES * AUDIO_CHANNELS + i] =
                (v.clamp(-1.0, 1.0) * 32767.0) as i16;
        }
    }
    // each level: a block of 256*2^level frames
    for (level, channels) in pyr.levels.iter().enumerate() {
        let block = PEAK_BLOCK_FRAMES << level;
        for (ch, blocks) in channels.iter().enumerate() {
            for (bi, &(mn, mx)) in blocks.iter().enumerate() {
                let from = bi * block;
                let to = ((bi + 1) * block).min(usable);
                if from >= to {
                    continue;
                }
                let mut emn = i16::MAX;
                let mut emx = i16::MIN;
                for f in from..to {
                    let v = all[f * AUDIO_CHANNELS + ch];
                    emn = emn.min(v);
                    emx = emx.max(v);
                }
                assert_eq!((mn, mx), (emn, emx), "level {level} ch {ch} block {bi}");
            }
        }
    }
    // the pyramid's apex — one block
    assert_eq!(pyr.levels.last().unwrap()[0].len(), 1);
}

#[test]
fn t15_reopen_reads_the_entries_without_decode() {
    let dir = tempfile::tempdir().unwrap();
    let media_file = dir.path().join("tone.bin");
    std::fs::write(&media_file, b"fake media content v1").unwrap();
    let db_path = dir.path().join("kinetren-cache");

    {
        let mut db = MediaStore::open(&db_path).unwrap();
        let (id, status) = db.ensure_media(&media_file).unwrap();
        assert_eq!(status, MediaEntryStatus::Rebuilt);
        let mut src = synth(1, AudioPattern::Ramp);
        db.build_audio_peaks(id, &mut src).unwrap();
        assert!(src.audio_reads > 0);
        db.mark_complete(id).unwrap();
    }

    // reopening: records are reused, no decode
    let mut db = MediaStore::open(&db_path).unwrap();
    let (id, status) = db.ensure_media(&media_file).unwrap();
    assert_eq!(status, MediaEntryStatus::Reused);
    assert!(db.is_complete(id).unwrap());
    let peaks = db.audio_peaks(id, 0, 0).unwrap().expect("peaks persisted");
    assert!(!peaks.is_empty());
    // ramp: the min of each block = the block's first sample (scale 1e-6 * 32767)
    let expect0 = (SyntheticSource::sample_at(AudioPattern::Ramp, 0, 0) * 32767.0) as i16;
    assert_eq!(peaks[0].0, expect0);
}

// ---- Test 16: invalidation ---------------------------------------------------

#[test]
fn t16_content_change_rebuilds_only_that_media() {
    let dir = tempfile::tempdir().unwrap();
    let file_a = dir.path().join("a.bin");
    let file_b = dir.path().join("b.bin");
    std::fs::write(&file_a, b"content A v1").unwrap();
    std::fs::write(&file_b, b"content B v1").unwrap();
    let db_path = dir.path().join("kinetren-cache");

    let mut db = MediaStore::open(&db_path).unwrap();
    let (ida, _) = db.ensure_media(&file_a).unwrap();
    let (idb, _) = db.ensure_media(&file_b).unwrap();
    let mut src = synth(1, AudioPattern::Ramp);
    db.build_audio_peaks(ida, &mut src).unwrap();
    db.build_audio_peaks(idb, &mut src).unwrap();
    db.mark_complete(ida).unwrap();
    db.mark_complete(idb).unwrap();

    // content of A replaced at the same path
    std::fs::write(&file_a, b"content A v2 -- different").unwrap();
    let (ida2, status_a) = db.ensure_media(&file_a).unwrap();
    assert_eq!(status_a, MediaEntryStatus::Rebuilt);
    assert_ne!(ida2, ida, "another content is another entry");
    assert!(db.audio_peaks(ida2, 0, 0).unwrap().is_none(), "nothing is known of the new content");
    assert!(!db.is_complete(ida2).unwrap());
    // the old content under another name finds its entry again, with no decode
    let copy = dir.path().join("a copy.bin");
    std::fs::write(&copy, b"content A v1").unwrap();
    assert_eq!(db.ensure_media(&copy).unwrap(), (ida, MediaEntryStatus::Reused));
    assert!(db.audio_peaks(ida, 0, 0).unwrap().is_some());
    // B is untouched
    assert!(db.audio_peaks(idb, 0, 0).unwrap().is_some(), "B must not be harmed");
    assert!(db.is_complete(idb).unwrap());
}

#[test]
fn t16_mtime_touch_without_content_change_reuses() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.bin");
    std::fs::write(&file, b"stable content").unwrap();
    let db_path = dir.path().join("kinetren-cache");

    let mut db = MediaStore::open(&db_path).unwrap();
    let (id, _) = db.ensure_media(&file).unwrap();
    let mut src = synth(1, AudioPattern::Ramp);
    db.build_audio_peaks(id, &mut src).unwrap();
    db.mark_complete(id).unwrap();

    // touch: the same content, new mtime
    std::thread::sleep(std::time::Duration::from_millis(20));
    let content = std::fs::read(&file).unwrap();
    std::fs::write(&file, &content).unwrap();

    let (id2, status) = db.ensure_media(&file).unwrap();
    assert_eq!(id2, id);
    assert_eq!(status, MediaEntryStatus::Reused, "hash matched — records are reused");
    assert!(db.audio_peaks(id, 0, 0).unwrap().is_some());
}

#[test]
fn t16_a_corrupt_entry_is_a_miss_and_goes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("kinetren-cache");
    let file = dir.path().join("a.bin");
    std::fs::write(&file, b"x").unwrap();
    let mut db = MediaStore::open(&root).unwrap();
    let (id, _) = db.ensure_media(&file).unwrap();
    db.build_audio_peaks(id, &mut synth(1, AudioPattern::Ramp)).unwrap();
    db.set_loudness(id, -20.0, -1.0, 5.0).unwrap();
    let peaks = root.join(format!("media/{id}/peaks"));
    let mut b = std::fs::read(&peaks).unwrap();
    let n = b.len();
    b[n - 1] ^= 0x55;
    std::fs::write(&peaks, &b).unwrap();
    assert!(db.audio_peaks(id, 0, 0).unwrap().is_none(), "a broken checksum is a miss");
    assert!(!peaks.exists(), "the broken file is removed");
    assert_eq!(db.loudness(id).unwrap(), Some((-20.0, -1.0, 5.0)), "its neighbour still hits");
    db.build_audio_peaks(id, &mut synth(1, AudioPattern::Ramp)).unwrap();
    assert!(db.audio_peaks(id, 0, 0).unwrap().is_some(), "computed again");
}

/// t872: an entry another process is WRITING is not a broken one: its temporary file stands beside the
/// name, the reader misses and leaves it alone, and once renamed it is reused. Garbage under an entry's
/// name is removed, and nothing else is touched. (The sidecar was one SQLite file: an opening that met
/// a lock took it for broken and deleted it under the other connections, SIGBUS in t640.)
#[test]
fn t872_an_entry_being_written_is_a_miss_that_leaves_it_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("kinetren-cache");
    let file = dir.path().join("a.bin");
    std::fs::write(&file, b"media").unwrap();
    let mut db = MediaStore::open(&root).unwrap();
    let (id, _) = db.ensure_media(&file).unwrap();
    db.set_loudness(id, -20.0, -1.0, 5.0).unwrap();
    db.put_poster(id, 900, 7).unwrap();

    // the other process is half way: the final name is absent
    let entry = root.join(format!("media/{id}/loudness"));
    let tmp = root.join(format!("media/{id}/.loudness.99999-0.tmp"));
    std::fs::rename(&entry, &tmp).unwrap();
    assert_eq!(db.loudness(id).unwrap(), None);
    assert!(tmp.exists(), "the writer's file was taken away under it");
    std::fs::rename(&tmp, &entry).unwrap();
    assert_eq!(db.loudness(id).unwrap(), Some((-20.0, -1.0, 5.0)), "reused once renamed");

    std::fs::write(&entry, b"this is definitely not an entry").unwrap();
    assert_eq!(db.loudness(id).unwrap(), None);
    assert!(!entry.exists(), "garbage under an entry's name stays");
    assert_eq!(db.poster(id).unwrap(), Some(900), "another entry was touched");
    // another media's entry put under this name is no entry of it
    std::fs::copy(root.join(format!("media/{id}/poster")), &entry).unwrap();
    assert_eq!(db.loudness(id).unwrap(), None, "an entry of another kind was read");
    assert_eq!(db.poster(id).unwrap(), Some(900));
}

// ---- Test 70 (item 14, round 7): nearest thumbnail within tolerance -----------------------

#[test]
fn t70_thumbnail_near() {
    let dir = tempfile::tempdir().unwrap();
    let media_file = dir.path().join("v.bin");
    std::fs::write(&media_file, b"fake video").unwrap();
    let db_path = dir.path().join("kinetren-cache");
    let mut db = MediaStore::open(&db_path).unwrap();
    let (id, _) = db.ensure_media(&media_file).unwrap();
    db.put_thumbnail(id, 90_000, b"jpeg-a").unwrap();
    db.put_thumbnail(id, 180_000, b"jpeg-b").unwrap();

    // exact match takes priority
    assert_eq!(db.thumbnail_near(id, 90_000, 45_000).unwrap(), Some((90_000, b"jpeg-a".to_vec())));
    // nearest within tolerance (100_000 is closer to 90_000 than to 180_000)
    assert_eq!(db.thumbnail_near(id, 100_000, 45_000).unwrap(), Some((90_000, b"jpeg-a".to_vec())));
    assert_eq!(
        db.thumbnail_near(id, 150_000, 45_000).unwrap(),
        Some((180_000, b"jpeg-b".to_vec()))
    );
    // outside tolerance — None (a scrub must not misreport its position)
    assert_eq!(db.thumbnail_near(id, 400_000, 45_000).unwrap(), None);
    // BETWEEN bounds are inclusive
    assert_eq!(db.thumbnail_near(id, 135_000, 45_000).unwrap().map(|(t, _)| t), Some(90_000));
    // foreign media is not visible
    assert_eq!(db.thumbnail_near(ContentId([7; 16]), 90_000, 45_000).unwrap(), None);
    drop(db);

    // a process died writing a record: the whole ones before it are read, and the next is found after it
    let thumbs = dir.path().join(format!("kinetren-cache/media/{id}/thumbs"));
    let mut b = std::fs::read(&thumbs).unwrap();
    b.extend_from_slice(b"KTHR torn");
    std::fs::write(&thumbs, &b).unwrap();
    let db = MediaStore::open(&db_path).unwrap();
    assert_eq!(db.thumbnail(id, 180_000).unwrap().as_deref(), Some(&b"jpeg-b"[..]));
    db.put_thumbnail(id, 270_000, b"jpeg-c").unwrap();
    assert_eq!(db.thumbnail(id, 90_000).unwrap().as_deref(), Some(&b"jpeg-a"[..]), "the thumbs before are lost");
}

/// Test 401 (R.49 B3): the offline analysis survives a round trip byte-exactly: integers ×100, not
/// f64, otherwise “recalculated the same thing” would give a different number in the last
/// digit, and there would be no way to tell “recalculated” from “changed”.
#[test]
fn t401_analysis_entries_round_trip_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kinetren-cache");
    let (one, two) = (ContentId([1; 16]), ContentId([2; 16]));
    {
        let db = MediaStore::open(&path).unwrap();
        assert!(db.loudness(one).unwrap().is_none(), "an empty folder must know nothing");
        db.set_loudness(one, -23.45, -1.20, 7.80).unwrap();
        db.set_sync_offset(one, two, -4410, 0.87).unwrap();
    }
    // a new opening — the data is in place and the same
    {
        let db = MediaStore::open(&path).unwrap();
        assert_eq!(db.loudness(one).unwrap(), Some((-23.45, -1.20, 7.80)));
        assert_eq!(db.sync_offset(one, two).unwrap(), Some((-4410, 0.87)));
        // the pair is ORDERED: the sign of the shift depends on the order, and “simply find”
        // a record backwards would mean returning the shift with someone else's sign
        assert_eq!(db.sync_offset(two, one).unwrap(), None, "the pair was read backwards");
        // overwrite in place
        db.set_loudness(one, -16.0, -0.5, 3.0).unwrap();
        assert_eq!(db.loudness(one).unwrap(), Some((-16.0, -0.5, 3.0)));
    }
}

/// Test 402 (R.49 B3): a file that turned out to be ANOTHER one takes with it both its loudness
/// and the offsets of BOTH sides of each of its pairs: they are keyed by both contents. (Over tables a
/// uniform `DELETE ... WHERE media_id=?1` did not see the pair where the swapped file sits second.)
#[test]
fn t402_changed_file_misses_on_both_sides_of_its_pairs() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.wav");
    std::fs::write(&file, b"RIFF....WAVEfmt first").unwrap();
    let mut db = MediaStore::open(&dir.path().join("kinetren-cache")).unwrap();
    let (x, y) = (ContentId([7; 16]), ContentId([8; 16]));

    let (a, st) = db.ensure_media(&file).unwrap();
    assert_eq!(st, MediaEntryStatus::Rebuilt);
    db.set_loudness(a, -20.0, -1.0, 5.0).unwrap();
    // a pair where the swapped file sits SECOND
    db.set_sync_offset(x, a, 900, 0.9).unwrap();
    db.set_sync_offset(a, y, -900, 0.8).unwrap();
    // someone else's pair, which the swap does not touch
    db.set_sync_offset(x, y, 123, 0.5).unwrap();

    // the file was swapped: different size and content
    std::fs::write(&file, b"RIFF....WAVEfmt totally different content here").unwrap();
    let (a2, st) = db.ensure_media(&file).unwrap();
    assert_ne!(a2, a, "another content under the same path");
    assert_eq!(st, MediaEntryStatus::Rebuilt, "a file swap must count as a rebuild");

    assert_eq!(db.loudness(a2).unwrap(), None, "loudness computed from a different file");
    assert_eq!(db.sync_offset(x, a2).unwrap(), None, "a shift where the swapped file is the SECOND");
    assert_eq!(db.sync_offset(a2, y).unwrap(), None, "a shift where it is the first");
    assert_eq!(db.sync_offset(x, y).unwrap(), Some((123, 0.5)), "a FOREIGN pair was lost");
}

/// `build_observed` shows each burst and does NOT change the pyramid: the loudness
/// is computed on the pass that runs anyway (a second full decode of the hour-long
/// source would be minutes).
#[test]
fn t401b_build_observed_sees_every_burst_and_keeps_the_pyramid() {
    let mut a = synth(2, AudioPattern::Sine { hz: 440.0 });
    let plain = PeakPyramid::build(&mut a);

    let mut b = synth(2, AudioPattern::Sine { hz: 440.0 });
    let mut seen = 0usize;
    let mut frames = 0usize;
    let observed = PeakPyramid::build_observed(&mut b, &mut |burst| {
        seen += 1;
        frames += burst.data.len() / AUDIO_CHANNELS;
    });
    assert_eq!(observed.levels, plain.levels, "observer changed the pyramid");
    assert_eq!(
        seen as i64,
        Tick(90_000 * 2).to_audio_frames() / AUDIO_BURST_FRAMES as i64,
        "observer did not see all bursts"
    );
    assert_eq!(frames, seen * AUDIO_BURST_FRAMES);
}

/// Test 639 (R.58, item 463): the reel PREVIEW on disk.
///
/// The owner's complaint: “Composition previews on the timeline do not seem to be cached,
/// and every project opening builds them anew”. A reel has no file — the key on disk is the CONTENT:
/// (reel, edit, media fingerprint). A foreign edit or a foreign fingerprint must give `None`:
/// the edit does not see a file swap, and without the fingerprint a swapped source
/// would show the old strip.
#[test]
fn t639_reel_previews_round_trip_and_key_strictly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kinetren-cache");

    // the peak pyramid — a real one, from PeakAcc: the blob format is the same as for files
    let mut src = synth(2, AudioPattern::Sine { hz: 440.0 });
    let peaks = PeakPyramid::build(&mut src);
    assert!(peaks.levels.len() > 1, "pyramid has one level only, the test would prove nothing");
    let frames: Vec<Option<&[u8]>> = vec![Some(b"jpeg-0"), None, Some(b"jpeg-2")];

    {
        let mut db = ReelStore::open(&path, None).unwrap();
        assert!(db.reel_preview(7, 0xABCD, 0x1234).unwrap().is_none(), "empty db knows a reel");
        db.put_reel_frames(7, 0xABCD, 0x1234, 90_000, 250_000, &frames).unwrap();
        db.put_reel_peaks(7, 0xABCD, 0x1234, &peaks).unwrap();

        // byte-for-byte round trip: step, end, a hole in the frames, the whole pyramid
        let row = db.reel_preview(7, 0xABCD, 0x1234).unwrap().expect("stored reel is missing");
        assert_eq!(row.step, 90_000);
        assert_eq!(row.end_ticks, 250_000);
        assert_eq!(row.frames.len(), 3, "frame count is not the stored n");
        assert_eq!(row.frames[0].as_deref(), Some(&b"jpeg-0"[..]));
        assert_eq!(row.frames[1], None, "a missing frame came back as something");
        assert_eq!(row.frames[2].as_deref(), Some(&b"jpeg-2"[..]));
        assert_eq!(row.peaks, peaks, "peak pyramid did not survive the round trip");

        // the key is strict: a foreign edit / a foreign fingerprint / a foreign reel
        assert!(db.reel_preview(7, 0xABCD ^ 1, 0x1234).unwrap().is_none(), "wrong hash hit");
        assert!(db.reel_preview(7, 0xABCD, 0x1234 ^ 1).unwrap().is_none(), "wrong media_fp hit");
        assert!(db.reel_preview(8, 0xABCD, 0x1234).unwrap().is_none(), "wrong seq hit");

        // negative values after a bitwise i64 — also a key
        let big_hash = u64::MAX - 5;
        db.put_reel_frames(9, big_hash, u64::MAX, 45_000, 1, &[None]).unwrap();
        db.put_reel_peaks(9, big_hash, u64::MAX, &peaks).unwrap();
        assert!(db.reel_preview(9, big_hash, u64::MAX).unwrap().is_some(), "high-bit key lost");

        // a new version of the SAME reel evicts the previous one entirely — including
        // frames beyond the new n and the old peak levels
        let short: Vec<Option<&[u8]>> = vec![Some(b"v2")];
        let flat = PeakPyramid { levels: vec![vec![vec![(0, 0)], vec![(0, 0)]]] };
        db.put_reel_frames(7, 0xABCD, 0x1234, 180_000, 300_000, &short).unwrap();
        db.put_reel_peaks(7, 0xABCD, 0x1234, &flat).unwrap();
        let row = db.reel_preview(7, 0xABCD, 0x1234).unwrap().expect("re-put lost the reel");
        assert_eq!(row.step, 180_000, "old step survived the re-put");
        assert_eq!(row.frames, vec![Some(b"v2".to_vec())], "old frames leaked into the new strip");
        assert_eq!(row.peaks, flat, "old peak levels leaked into the new pyramid");
        // a new EDIT of the same reel — the previous edit is no longer found
        db.put_reel_frames(7, 0xBEEF, 0x1234, 180_000, 300_000, &short).unwrap();
        db.put_reel_peaks(7, 0xBEEF, 0x1234, &flat).unwrap();
        assert!(db.reel_preview(7, 0xABCD, 0x1234).unwrap().is_none(), "old edition survived");
        assert!(db.reel_preview(7, 0xBEEF, 0x1234).unwrap().is_some());
    }

    // reopening — rows are in place (this is “not rebuilt anew”)
    {
        let mut db = ReelStore::open(&path, None).unwrap();
        let row = db.reel_preview(7, 0xBEEF, 0x1234).unwrap().expect("reel gone after reopen");
        assert_eq!(row.frames, vec![Some(b"v2".to_vec())]);
        assert_eq!(row.peaks.levels, vec![vec![vec![(0, 0)], vec![(0, 0)]]]);
        assert!(db.reel_preview(9, u64::MAX - 5, u64::MAX).unwrap().is_some());

        // cleanup: reels that are not in the project are removed; named ones remain
        assert_eq!(db.prune_reels(&[7]).unwrap(), 1, "prune removed a wrong number of reels");
        assert!(db.reel_preview(7, 0xBEEF, 0x1234).unwrap().is_some(), "kept reel was pruned");
        assert!(
            db.reel_preview(9, u64::MAX - 5, u64::MAX).unwrap().is_none(),
            "unlisted reel kept"
        );
        assert_eq!(db.prune_reels(&[7]).unwrap(), 0);

        // and the media entries are not affected by this: a media's going does not touch reels
        let media_file = dir.path().join("v.bin");
        std::fs::write(&media_file, b"fake video").unwrap();
        let mut media = MediaStore::open(&path).unwrap();
        let (id, _) = media.ensure_media(&media_file).unwrap();
        media.mark_complete(id).unwrap();
        std::fs::remove_dir_all(path.join(format!("media/{id}"))).unwrap();
        assert!(db.reel_preview(7, 0xBEEF, 0x1234).unwrap().is_some());
    }

    // the folder is shared: another project's reel of the same id is another reel, and neither
    // project's edit or prune takes what the other still shows
    {
        let mut a = ReelStore::open(&path, Some(std::path::Path::new("/x/a.kt"))).unwrap();
        let mut b = ReelStore::open(&path, Some(std::path::Path::new("/x/b.kt"))).unwrap();
        a.put_reel_frames(1, 10, 1, 90_000, 90_000, &[Some(b"a")]).unwrap();
        b.put_reel_frames(1, 20, 1, 90_000, 90_000, &[Some(b"b")]).unwrap();
        assert!(a.reel_preview(1, 10, 1).unwrap().is_some(), "b's reel evicted a's");
        // the same content in both is one entry
        assert!(b.reel_preview(1, 10, 1).unwrap().is_some());
        a.put_reel_frames(1, 11, 1, 90_000, 90_000, &[Some(b"a2")]).unwrap();
        assert!(b.reel_preview(1, 10, 1).unwrap().is_some(), "a's edit took what b shows");
        assert_eq!(a.prune_reels(&[]).unwrap(), 1);
        assert!(b.reel_preview(1, 10, 1).unwrap().is_some(), "a's prune took what b shows");
        assert!(a.reel_preview(1, 11, 1).unwrap().is_none());
    }
}

/// Test 795b (R.67): the frames and peaks of a strip are written SEPARATELY.
/// A strip without an envelope is a legitimate state: frames are read from disk, and
/// `has_peaks` says the envelope still has to be computed. Peaks do not attach to a
/// foreign key.
#[test]
fn t795b_reel_frames_and_peaks_are_written_separately() {
    let dir = std::env::temp_dir().join(format!("kt-t795b-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("kinetren-cache");

    let frames: Vec<Option<&[u8]>> = vec![Some(&b"j0"[..]), None, Some(&b"j2"[..])];
    let peaks = PeakPyramid {
        levels: vec![vec![vec![(-100, 100)], vec![(-50, 50)]], vec![vec![(-100, 100)], vec![(-50, 50)]]],
    };
    {
        let mut db = ReelStore::open(&path, None).unwrap();

        // --- ONLY frames: the strip is read, there is no envelope yet ------------
        db.put_reel_frames(5, 0xAA, 0xBB, 90_000, 300_000, &frames).unwrap();
        let row = db.reel_preview(5, 0xAA, 0xBB).unwrap().expect("frames did not land on disk");
        assert_eq!(row.frames.len(), 3);
        assert_eq!(row.frames[0].as_deref(), Some(&b"j0"[..]));
        assert!(!row.has_peaks, "a band without peaks must say so");
        assert!(row.peaks.levels.is_empty(), "no peaks, but the pyramid is not empty");

        // --- peaks are APPENDED to the same row ---------------------------
        db.put_reel_peaks(5, 0xAA, 0xBB, &peaks).unwrap();
        let row = db.reel_preview(5, 0xAA, 0xBB).unwrap().expect("band disappeared after peaks");
        assert!(row.has_peaks, "peaks were appended, but the flag stays silent");
        assert_eq!(row.peaks, peaks, "pyramid did not survive a round trip");
        assert_eq!(row.frames[2].as_deref(), Some(&b"j2"[..]), "frames were lost when writing peaks");

        // --- peaks do NOT attach to a FOREIGN key ----------------------------
        let other = PeakPyramid { levels: vec![vec![vec![(-1, 1)], vec![(-1, 1)]]] };
        db.put_reel_peaks(5, 0xDEAD, 0xBB, &other).unwrap();
        let row = db.reel_preview(5, 0xAA, 0xBB).unwrap().unwrap();
        assert_eq!(row.peaks, peaks, "peaks of a foreign revision overwrote ours");

        // --- a new edit of frames WIPES the old peaks --------------------
        db.put_reel_frames(5, 0xCC, 0xBB, 90_000, 300_000, &frames).unwrap();
        let row = db.reel_preview(5, 0xCC, 0xBB).unwrap().expect("new revision did not land");
        assert!(!row.has_peaks, "peaks of the previous revision stayed on the new one");
        assert!(db.reel_preview(5, 0xAA, 0xBB).unwrap().is_none(), "old revision stayed");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// t1171 (worktree-version-and-thumb-lock): simultaneous openings of one project's cache all succeed.
///
/// A project opening starts several threads at once that each open it (index lane, thumbnails, reel
/// preview, sources). Over SQLite `PRAGMA journal_mode=WAL` came back `SQLITE_BUSY` in 0.2 ms and the
/// reel preview lost its disk tier (t640 red on a quiet machine). Asserted: on a new folder and on an
/// existing one, every one of the simultaneous openings succeeds, and their entries are all there.
#[test]
fn t1171_concurrent_openings_never_fail() {
    let dir = tempfile::tempdir().unwrap();
    const THREADS: usize = 12;
    for round in 0..20 {
        let root = dir.path().join(format!("p{round}/kinetren-cache"));
        for phase in 0..2u8 {
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(THREADS));
            let errs: Vec<String> = (0..THREADS as u8)
                .map(|n| {
                    let (p, b) = (root.clone(), barrier.clone());
                    std::thread::spawn(move || {
                        b.wait();
                        let db = MediaStore::open(&p).map_err(|e| format!("{e:?}"))?;
                        db.set_loudness(ContentId([n; 16]), -(phase as f64), 0.0, 0.0).map_err(|e| format!("{e:?}"))
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .filter_map(|h| h.join().unwrap().err())
                .collect();
            assert!(errs.is_empty(), "round {round} phase {phase}: {} openings failed: {errs:?}", errs.len());
            let db = MediaStore::open(&root).unwrap();
            for n in 0..THREADS as u8 {
                assert_eq!(db.loudness(ContentId([n; 16])).unwrap().map(|l| l.0), Some(-(phase as f64)));
            }
        }
    }
}

/// t1741 (test-stability, row 88): a pass over a file stops when it is told to, and keeps nothing.
/// The file is as long as it says, whatever it holds: a damaged header can promise hours, and the
/// program closing has no reason to wait for them. `stop` is asked before every burst.
#[test]
fn t1741_a_peak_pass_stops_when_told_and_stores_nothing() {
    let mut src = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 });
    let asked = std::cell::Cell::new(0);
    let stop = || {
        asked.set(asked.get() + 1);
        asked.get() > 10
    };
    let mut seen = 0;
    assert!(PeakPyramid::build_until(&mut src, &mut |_| seen += 1, &stop).is_none(), "the pass ran on");
    assert_eq!(seen, 10, "a burst was read after the stop");

    let dir = tempfile::tempdir().unwrap();
    let mut db = MediaStore::open(&dir.path().join("kinetren-cache")).unwrap();
    let file = dir.path().join("a.bin");
    std::fs::write(&file, b"payload").unwrap();
    let (id, _) = db.ensure_media(&file).unwrap();
    let built = db.build_audio_peaks_until(id, &mut src, &mut |_| {}, &|| true).unwrap();
    assert!(!built, "a stopped pass reports it");
    assert_eq!(db.audio_peaks(id, 0, 0).unwrap(), None, "a stopped pass left peaks");
}

/// t1742 (test-stability, row 88): a source with no end has no envelope, and nothing is read: a
/// placeholder, which also stands for a generator kind this build does not have, answers every
/// read at once with silence for 1000 hours.
#[test]
fn t1742_a_source_with_no_end_is_not_read() {
    struct Endless(PlaceholderSource, std::sync::Arc<std::sync::atomic::AtomicU32>);
    use std::sync::atomic::Ordering::Relaxed;
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    impl MediaSource for Endless {
        fn info(&self) -> &kt_media::MediaInfo {
            self.0.info()
        }
        fn read_audio_burst(&mut self, idx: i64) -> Result<kt_media::AudioBurst, kt_media::MediaError> {
            self.1.fetch_add(1, Relaxed);
            self.0.read_audio_burst(idx)
        }
        fn read_video_frame(
            &mut self,
            p: Tick,
            q: Option<kt_media::Quality>,
        ) -> Result<kt_media::VideoFrame, kt_media::MediaError> {
            self.0.read_video_frame(p, q)
        }
        fn keyframe_before(&self, p: Tick) -> Option<Tick> {
            self.0.keyframe_before(p)
        }
    }
    let mut src = Endless(PlaceholderSource::new("gone.mp4"), reads.clone());
    let pyr = PeakPyramid::build_until(&mut src, &mut |_| {}, &|| false).expect("nothing stopped it");
    assert!(pyr.levels.iter().flatten().all(|c| c.is_empty()), "an envelope of a source with no end");
    assert_eq!(reads.load(Relaxed), 0, "the source was read");
}

/// t2894 (source-colour-matrix): thumbs written before the decode took the source's colour
/// matrix are of an older pixel version: not read, and the first thumb written now drops them,
/// so a filmstrip never mixes old and new colours. Thumbs of the current version stay.
#[test]
fn t2894_thumbs_of_an_older_pixel_version_are_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("kinetren-cache");
    let file = dir.path().join("a.bin");
    std::fs::write(&file, b"media").unwrap();
    let mut db = MediaStore::open(&root).unwrap();
    let id = db.ensure_media(&file).unwrap().0;
    db.put_thumb_size(id, 320, 180).unwrap();
    db.put_thumbnail(id, 0, b"old0").unwrap();
    db.put_thumbnail(id, 9000, b"old1").unwrap();
    drop(db);
    // both files are of a former build's pixels: the version in their headers
    for name in ["thumbs", "thumb-size"] {
        let p = root.join(format!("media/{id}/{name}"));
        let mut b = std::fs::read(&p).unwrap();
        b[8..12].copy_from_slice(&(kt_media::index::THUMB_PIXELS_VERSION - 1).to_le_bytes());
        std::fs::write(&p, &b).unwrap();
    }

    let mut db = MediaStore::open(&root).unwrap();
    assert_eq!(db.thumb_size(id).unwrap(), None, "thumbs of an older pixel version are not read");
    db.put_thumb_size(id, 320, 180).unwrap();
    db.put_thumbnail(id, 0, b"new0").unwrap();
    assert_eq!(db.thumb_size(id).unwrap(), Some((320, 180)));
    assert_eq!(db.thumbnail(id, 0).unwrap().as_deref(), Some(&b"new0"[..]));
    assert_eq!(db.thumbnail(id, 9000).unwrap(), None, "the older thumb at another moment is gone");
    // a second write at the current version keeps what is there
    db.put_thumb_size(id, 320, 180).unwrap();
    assert_eq!(db.thumbnail(id, 0).unwrap().as_deref(), Some(&b"new0"[..]));
}

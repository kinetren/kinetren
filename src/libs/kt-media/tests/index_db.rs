//! Tests 15–16 (docs/porting-notes.md): peak pyramid and index DB invalidation.

use kt_media::index::{MediaEntryStatus, MediaIndexDb, PeakPyramid};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
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
fn t15_reopen_reads_from_db_without_decode() {
    let dir = tempfile::tempdir().unwrap();
    let media_file = dir.path().join("tone.bin");
    std::fs::write(&media_file, b"fake media content v1").unwrap();
    let db_path = dir.path().join("p.ktindex.db");

    {
        let mut db = MediaIndexDb::open(&db_path).unwrap();
        let (id, status) = db.ensure_media(&media_file).unwrap();
        assert_eq!(status, MediaEntryStatus::Rebuilt);
        let mut src = synth(1, AudioPattern::Ramp);
        db.build_audio_peaks(id, &mut src).unwrap();
        assert!(src.audio_reads > 0);
        db.mark_complete(id).unwrap();
    }

    // reopening: records are reused, no decode
    let mut db = MediaIndexDb::open(&db_path).unwrap();
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
    let db_path = dir.path().join("p.ktindex.db");

    let mut db = MediaIndexDb::open(&db_path).unwrap();
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
    assert_eq!(ida2, ida, "media_id is stable, only sections are rebuilt");
    assert!(db.audio_peaks(ida, 0, 0).unwrap().is_none(), "sections A reset");
    assert!(!db.is_complete(ida).unwrap());
    // B is untouched
    assert!(db.audio_peaks(idb, 0, 0).unwrap().is_some(), "B must not be harmed");
    assert!(db.is_complete(idb).unwrap());
}

#[test]
fn t16_mtime_touch_without_content_change_reuses() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.bin");
    std::fs::write(&file, b"stable content").unwrap();
    let db_path = dir.path().join("p.ktindex.db");

    let mut db = MediaIndexDb::open(&db_path).unwrap();
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
fn t16_corrupted_db_is_recreated_without_panic() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("p.ktindex.db");
    std::fs::write(&db_path, b"this is definitely not sqlite").unwrap();

    let mut db = MediaIndexDb::open(&db_path).expect("a broken DB is recreated, it does not crash");
    let file = dir.path().join("a.bin");
    std::fs::write(&file, b"x").unwrap();
    let (_, status) = db.ensure_media(&file).unwrap();
    assert_eq!(status, MediaEntryStatus::Rebuilt);
}

// ---- Test 70 (item 14, round 7): nearest thumbnail within tolerance -----------------------

#[test]
fn t70_thumbnail_near() {
    let dir = tempfile::tempdir().unwrap();
    let media_file = dir.path().join("v.bin");
    std::fs::write(&media_file, b"fake video").unwrap();
    let db_path = dir.path().join("p.ktindex.db");
    let mut db = MediaIndexDb::open(&db_path).unwrap();
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
    assert_eq!(db.thumbnail_near(id + 1, 90_000, 45_000).unwrap(), None);
}

/// Test 401 (R.49 B3): the offline-analysis tables live in the same DB, added
/// ADDITIVELY (without a SCHEMA_VERSION bump — the bump wipes the packet index, peaks and
/// thumbs of all project media) and survive a round trip byte-exactly: integers ×100, not
/// f64, otherwise “recalculated the same thing” would give a different number in the last
/// digit, and there would be no way to tell “recalculated” from “changed”.
#[test]
fn t401_analysis_tables_round_trip_exactly() {
    let dir = std::env::temp_dir().join(format!("kt-t401-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("p.ktindex.db");

    {
        let db = MediaIndexDb::open(&path).unwrap();
        assert!(db.loudness(1).unwrap().is_none(), "an empty DB must know nothing");
        db.set_loudness(1, -23.45, -1.20, 7.80).unwrap();
        db.set_sync_offset(1, 2, -4410, 0.87).unwrap();
    }
    // a new opening — the data is in place and the same
    {
        let db = MediaIndexDb::open(&path).unwrap();
        assert_eq!(db.loudness(1).unwrap(), Some((-23.45, -1.20, 7.80)));
        assert_eq!(db.sync_offset(1, 2).unwrap(), Some((-4410, 0.87)));
        // the pair is ORDERED: the sign of the shift depends on the order, and “simply find”
        // a record backwards would mean returning the shift with someone else's sign
        assert_eq!(db.sync_offset(2, 1).unwrap(), None, "the pair was read backwards");
        // overwrite in place
        db.set_loudness(1, -16.0, -0.5, 3.0).unwrap();
        assert_eq!(db.loudness(1).unwrap(), Some((-16.0, -0.5, 3.0)));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 402 (R.49 B3): a file that turned out to be ANOTHER one takes with it both its loudness
/// and the offsets of BOTH sides of each of its pairs.
///
/// The stage trap: a uniform `DELETE ... WHERE media_id=?1` clears `loudness`
/// (there the key is the media), but NOT `sync_offset` — there the key is the pair, and a record where
/// the swapped file sits second would survive the invalidation. The sync computed from
/// other sound would remain.
#[test]
fn t402_changed_file_invalidates_both_sides_of_its_pairs() {
    let dir = std::env::temp_dir().join(format!("kt-t402-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.wav");
    std::fs::write(&file, b"RIFF....WAVEfmt first").unwrap();
    let path = dir.join("p.ktindex.db");
    let mut db = MediaIndexDb::open(&path).unwrap();

    let (a, st) = db.ensure_media(&file).unwrap();
    assert_eq!(st, MediaEntryStatus::Rebuilt);
    db.set_loudness(a, -20.0, -1.0, 5.0).unwrap();
    // a pair where the swapped file sits SECOND
    db.set_sync_offset(777, a, 900, 0.9).unwrap();
    db.set_sync_offset(a, 888, -900, 0.8).unwrap();
    // someone else's pair, which the swap does not touch
    db.set_sync_offset(777, 888, 123, 0.5).unwrap();

    // the file was swapped: different size and content
    std::fs::write(&file, b"RIFF....WAVEfmt totally different content here").unwrap();
    let (a2, st) = db.ensure_media(&file).unwrap();
    assert_eq!(a2, a, "media_id stays with the path");
    assert_eq!(st, MediaEntryStatus::Rebuilt, "a file swap must count as a rebuild");

    assert_eq!(db.loudness(a).unwrap(), None, "loudness computed from a different file");
    assert_eq!(
        db.sync_offset(777, a).unwrap(),
        None,
        "a shift where the swapped file is the SECOND survived invalidation — the uniform \
         WHERE media_id does not see it"
    );
    assert_eq!(db.sync_offset(a, 888).unwrap(), None, "a shift where it is the first also goes away");
    assert_eq!(db.sync_offset(777, 888).unwrap(), Some((123, 0.5)), "a FOREIGN pair was deleted");
    let _ = std::fs::remove_dir_all(&dir);
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

/// Test 639 (R.58, item 463): the reel PREVIEW on disk — the `reel_*` tables.
///
/// The owner's complaint: “Composition previews on the timeline do not seem to be cached,
/// and every project opening builds them anew”. A reel has no file, and
/// `media(path, size, mtime, xxhash)` is not its home — the key on disk is the CONTENT:
/// (reel, edit, media fingerprint). A foreign edit or a foreign fingerprint must give `None`:
/// the edit does not see a file swap, and without the fingerprint a swapped source
/// would show the old strip.
#[test]
fn t639_reel_preview_tables_round_trip_and_key_strictly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.ktindex.db");

    // the peak pyramid — a real one, from PeakAcc: the blob format is the same as for files
    let mut src = synth(2, AudioPattern::Sine { hz: 440.0 });
    let peaks = PeakPyramid::build(&mut src);
    assert!(peaks.levels.len() > 1, "pyramid has one level only, the test would prove nothing");
    let frames: Vec<Option<&[u8]>> = vec![Some(b"jpeg-0"), None, Some(b"jpeg-2")];

    {
        let mut db = MediaIndexDb::open(&path).unwrap();
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
        let mut db = MediaIndexDb::open(&path).unwrap();
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

        // and the file tables are not affected by this: purging media does not touch reels
        let media_file = dir.path().join("v.bin");
        std::fs::write(&media_file, b"fake video").unwrap();
        let (id, _) = db.ensure_media(&media_file).unwrap();
        std::fs::write(&media_file, b"fake video, changed").unwrap();
        let (id2, st) = db.ensure_media(&media_file).unwrap();
        assert_eq!((id2, st), (id, MediaEntryStatus::Rebuilt));
        assert!(
            db.reel_preview(7, 0xBEEF, 0x1234).unwrap().is_some(),
            "rebuilding a media purged reel tables: they are keyed by content, not by media_id"
        );
    }
}

/// Test 795b (R.67): the frames and peaks of a strip are written in SEPARATE transactions.
/// A strip without an envelope is a legitimate state: frames are read from disk, and
/// `has_peaks` says the envelope still has to be computed. Peaks do not attach to a
/// foreign key; the schema is not bumped in this case (the packet index, thumbs and
/// peaks of ALL project media remain in place).
#[test]
fn t795b_reel_frames_and_peaks_are_written_separately() {
    let dir = std::env::temp_dir().join(format!("kt-t795b-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("i.ktindex.db");

    let frames: Vec<Option<&[u8]>> = vec![Some(&b"j0"[..]), None, Some(&b"j2"[..])];
    let peaks = PeakPyramid {
        levels: vec![vec![vec![(-100, 100)], vec![(-50, 50)]], vec![vec![(-100, 100)], vec![(-50, 50)]]],
    };
    {
        let mut db = MediaIndexDb::open(&path).unwrap();
        let before = kt_media::index::SCHEMA_VERSION;

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

        assert_eq!(
            kt_media::index::SCHEMA_VERSION,
            before,
            "the schema version must not move: a bump wipes the batch index, peaks and thumbnails of all media"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

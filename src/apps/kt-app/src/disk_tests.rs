//! Disk calls a gesture asks for (found-defects 840, the class of 911 and 1002).

use crate::i18n::{Lang, k, tr};
use crate::layout_tests::harness_lang_dt;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Holds the disk lane until it is dropped.
struct Held(Arc<(Mutex<bool>, Condvar)>);

impl Held {
    fn disk() -> Held {
        let h = Arc::new((Mutex::new(false), Condvar::new()));
        let g = h.clone();
        crate::disk::lane().post(move |_| {
            let mut free = g.0.lock().unwrap();
            while !*free {
                free = g.1.wait(free).unwrap();
            }
        });
        Held(h)
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        *self.0.0.lock().unwrap() = true;
        self.0.1.notify_all();
    }
}

/// t3736 (found-defects 840): an export to OpenTimelineIO on a disk that stands blocks no frame; its notice
/// comes when the file is there, a failure warns, another project opened meanwhile keeps it, and a close
/// waits for it.
#[test]
fn t3736_an_export_waits_for_no_disk() {
    crate::alone::own_process!();
    let dir = std::env::temp_dir().join(format!("kt-t3736-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = harness_lang_dt(Lang::En, Some(0.02));
    h.step();
    crate::app::set_media_grace(Some(Duration::ZERO));
    type H = egui_kittest::Harness<'static, crate::app::KtApp>;
    let said = |h: &H, t: &str| h.state().notices.iter().any(|n| n.text.contains(t));
    let lands = |h: &mut H, t: &str| {
        let deadline = kt_time::test_deadline();
        while !said(h, t) {
            assert!(std::time::Instant::now() < deadline, "{t:?} was never said");
            std::thread::sleep(Duration::from_millis(5));
            h.step();
        }
    };

    let (good, bad) = (dir.join("film.otio"), dir.join("taken.otio"));
    std::fs::create_dir_all(bad.join("inside")).unwrap();
    let held = Held::disk();
    let blocked = kt_time::block::here();
    h.state_mut().export_otio_to(&good);
    for _ in 0..5 {
        h.step();
    }
    assert_eq!(kt_time::block::here(), blocked, "a blocking call on the UI thread");
    assert!(!good.exists() && !said(&h, "film.otio"), "said before it was written");
    let next = crate::app::KtApp::new_empty(None);
    h.state_mut().replace_with(next);
    drop(held);
    lands(&mut h, &crate::tf!(k::msg::EXPORTED, good.display().to_string()));
    assert!(good.exists(), "the notice came before the file");

    // the name is a folder's: a warning, no success
    h.state_mut().notices.clear();
    h.state_mut().export_otio_to(&bad);
    lands(&mut h, tr(k::msg::SAVE_FAILED).split("{0}").next().unwrap());
    assert!(bad.join("inside").is_dir() && !said(&h, &crate::tf!(k::msg::EXPORTED, bad.display().to_string())));

    // asked just before the close
    let last = dir.join("last.otio");
    let held = Held::disk();
    h.state_mut().export_otio_to(&last);
    let freed = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        drop(held);
    });
    eframe::App::on_exit(h.state_mut());
    assert!(last.exists(), "the close did not wait for the export");
    freed.join().unwrap();
    crate::app::set_media_grace(None);
    let _ = std::fs::remove_dir_all(&dir);
}

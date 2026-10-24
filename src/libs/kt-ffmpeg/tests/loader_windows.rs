//! t846 (change windows-build, capability media-index): on Windows the five DLLs are loaded
//! from ONE folder even when a namesake sits earlier on PATH.
//!
//! Runs on Windows and under wine (`make windows-test`, the windows-latest CI job) with the
//! pinned gyan.dev build from `scripts/fetch-ffmpeg-win.sh`; elsewhere, or without that
//! build, it is skipped with the reason printed.

#[cfg(windows)]
#[test]
fn t846_windows_set_loads_from_one_folder() {
    use std::path::PathBuf;

    let dir: PathBuf = match std::env::var_os("KT_FFMPEG_WIN_DIR") {
        Some(d) => d.into(),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ffmpeg-win/8.1.2"),
    };
    if !dir.join("avcodec-62.dll").exists() {
        eprintln!("t846 skipped: no pinned FFmpeg build in {} (scripts/fetch-ffmpeg-win.sh)", dir.display());
        return;
    }
    let dir = std::path::absolute(&dir).unwrap();
    // a namesake earlier on PATH: a copy of avutil-60.dll in another folder
    let decoy = std::env::temp_dir().join(format!("kt-t846-decoy-{}", std::process::id()));
    std::fs::create_dir_all(&decoy).unwrap();
    std::fs::copy(dir.join("avutil-60.dll"), decoy.join("avutil-60.dll")).unwrap();
    let path = format!("{};{}", decoy.display(), std::env::var("PATH").unwrap_or_default());
    unsafe {
        std::env::set_var("PATH", &path);
        std::env::set_var("KINETREN_FFMPEG_DIR", &dir);
    }

    let info = kt_ffmpeg::rescan().unwrap_or_else(|e| panic!("the pinned build loads:\n{e}"));
    assert_eq!(info.major, 8);
    assert_eq!(info.avcodec_version.0, 62, "avcodec_version() major");
    let modules = kt_ffmpeg::loaded_modules();
    assert_eq!(modules.len(), 5);
    let want = dir.to_string_lossy().to_lowercase().replace('/', "\\");
    for m in &modules {
        let m = m.strip_prefix(r"\\?\").unwrap_or(m).to_lowercase().replace('/', "\\");
        let parent = m.rsplit_once('\\').map(|(p, _)| p.to_string()).unwrap_or_default();
        assert_eq!(parent, want.trim_end_matches('\\'), "module {m} lies in the chosen folder");
    }
    let _ = std::fs::remove_dir_all(&decoy);
}

#[cfg(not(windows))]
#[test]
fn t846_windows_set_loads_from_one_folder() {
    eprintln!("t846 skipped: Windows only (runs under wine in `make windows-test`)");
}

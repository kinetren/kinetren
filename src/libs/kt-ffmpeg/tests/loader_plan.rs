//! The FFmpeg search plan (change windows-build, capability media-index).
//!
//! The plan is pure, so the WINDOWS rules are checked here on any OS (t844, t845); t847
//! checks on the real loader that a failure is not remembered and a success is.

use std::collections::HashMap;

use kt_ffmpeg::search::{self, Candidate, Env, LOAD_ORDER, Lib, Os, Reject, Source};

#[derive(Default)]
struct FakeEnv {
    vars: HashMap<String, String>,
    exe_dir: Option<String>,
    user_choice: Option<String>,
    download_root: Option<String>,
    dirs: HashMap<String, Vec<String>>,
    links: HashMap<String, String>,
}

impl FakeEnv {
    fn var(mut self, k: &str, v: &str) -> Self {
        self.vars.insert(k.into(), v.into());
        self
    }
    fn dir(mut self, d: &str, files: &[&str]) -> Self {
        self.dirs.insert(d.into(), files.iter().map(|s| s.to_string()).collect());
        self
    }
    fn set(self, d: &str, os: Os, ffmpeg: u32) -> Self {
        let files: Vec<String> = LOAD_ORDER.iter().map(|l| search::file_name(os, *l, ffmpeg)).collect();
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        self.dir(d, &refs)
    }
}

impl Env for FakeEnv {
    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }
    fn exe_dir(&self) -> Option<String> {
        self.exe_dir.clone()
    }
    fn user_choice(&self) -> Option<String> {
        self.user_choice.clone()
    }
    fn download_root(&self) -> Option<String> {
        self.download_root.clone()
    }
    fn list(&self, dir: &str) -> Option<Vec<String>> {
        self.dirs.get(dir).cloned()
    }
    fn read_link(&self, path: &str) -> Option<String> {
        self.links.get(path).cloned()
    }
}

fn chosen(plan: &[Candidate]) -> Option<&Candidate> {
    plan.iter().find(|c| c.verdict.is_ok())
}

/// t844: the order of folders on Windows beats the order of majors; the DLL names of each
/// major and the load order are the table of the requirement.
#[test]
fn t844_windows_order_and_dll_names() {
    // the table of the requirement, written out by hand
    let want = [
        (9, ["avutil-61.dll", "swresample-7.dll", "swscale-10.dll", "avcodec-63.dll", "avformat-63.dll"]),
        (8, ["avutil-60.dll", "swresample-6.dll", "swscale-9.dll", "avcodec-62.dll", "avformat-62.dll"]),
        (7, ["avutil-59.dll", "swresample-5.dll", "swscale-8.dll", "avcodec-61.dll", "avformat-61.dll"]),
        (6, ["avutil-58.dll", "swresample-4.dll", "swscale-7.dll", "avcodec-60.dll", "avformat-60.dll"]),
    ];
    for (ffmpeg, names) in want {
        let got: Vec<String> = LOAD_ORDER.iter().map(|l| search::file_name(Os::Windows, *l, ffmpeg)).collect();
        assert_eq!(got, names, "FFmpeg {ffmpeg}");
    }
    assert_eq!(LOAD_ORDER, [Lib::Avutil, Lib::Swresample, Lib::Swscale, Lib::Avcodec, Lib::Avformat]);
    assert_eq!(search::file_name(Os::Linux, Lib::Avcodec, 8), "libavcodec.so.62");
    assert_eq!(search::file_name(Os::Linux, Lib::Avcodec, 9), "libavcodec.so.63");

    let mut env = FakeEnv::default()
        .var("KINETREN_FFMPEG_DIR", r"D:\env")
        .var("PATH", r"C:\Windows\system32;C:\p1;C:\p2;C:\p3")
        .var("ProgramFiles", r"C:\Program Files")
        .dir(r"D:\env", &[])
        .dir(r"C:\Windows\system32", &["kernel32.dll"])
        .dir(r"C:\p1", &["git.exe"])
        .dir(r"C:\p2", &[])
        .dir(r"C:\p3", &[])
        .dir(r"C:\Kinetren\ffmpeg", &[])
        .dir(r"C:\dl", &["9.0.2"])
        .dir(r"C:\dl\9.0.2", &[])
        .set(r"C:\Program Files\ffmpeg\bin", Os::Windows, 9)
        .set(r"E:\mine", Os::Windows, 7);
    env.exe_dir = Some(r"C:\Kinetren".into());
    env.download_root = Some(r"C:\dl".into());
    env.user_choice = Some(r"E:\mine".into());

    let plan = search::plan(Os::Windows, &env);
    let c = chosen(&plan).expect("a folder is chosen");
    assert_eq!(c.dir.as_deref(), Some(r"E:\mine"), "the person's choice beats a newer major elsewhere");
    assert_eq!(c.source, Source::UserChoice);
    assert_eq!(c.verdict, Ok(7));
    // the order of the plan: choice, variable, next to exe (two forms), download, known
    let order: Vec<(Source, &str)> = plan.iter().map(|c| (c.source, c.dir.as_deref().unwrap())).collect();
    assert_eq!(
        order,
        vec![
            (Source::UserChoice, r"E:\mine"),
            (Source::EnvVar, r"D:\env"),
            (Source::NextToExe, r"C:\Kinetren\ffmpeg"),
            (Source::NextToExe, r"C:\Kinetren\ffmpeg\bin"),
            (Source::Download, r"C:\dl\9.0.2"),
            (Source::Known, r"C:\Program Files\ffmpeg\bin"),
        ],
        "PATH folders without FFmpeg are not listed; missing known folders are skipped"
    );
    assert_eq!(plan[1].verdict, Err(Reject::NoAvcodec));
    assert_eq!(plan[3].verdict, Err(Reject::Unreadable));

    // without the choice the known location with FFmpeg 9 wins
    env.user_choice = None;
    let plan = search::plan(Os::Windows, &env);
    assert_eq!(chosen(&plan).unwrap().dir.as_deref(), Some(r"C:\Program Files\ffmpeg\bin"));
    assert_eq!(chosen(&plan).unwrap().verdict, Ok(9));

    // Linux: the variable's folder first, then the system by SONAME, newest first
    let env = FakeEnv::default().var("KINETREN_FFMPEG_DIR", "/opt/ff").set("/opt/ff", Os::Linux, 6);
    let plan = search::plan(Os::Linux, &env);
    let srcs: Vec<(Source, Option<&str>, Result<u32, Reject>)> =
        plan.iter().map(|c| (c.source, c.dir.as_deref(), c.verdict.clone())).collect();
    assert_eq!(
        srcs,
        vec![
            (Source::EnvVar, Some("/opt/ff"), Ok(6)),
            (Source::System, None, Ok(9)),
            (Source::System, None, Ok(8)),
            (Source::System, None, Ok(7)),
            (Source::System, None, Ok(6)),
        ]
    );
}

/// t845: every rejected folder is named with its reason; winget's link is followed to the
/// package folder.
#[test]
fn t845_rejections_named_and_winget_link_followed() {
    let links = r"C:\Users\u\AppData\Local\Microsoft\WinGet\Links";
    let pkg_bin = r"C:\Users\u\AppData\Local\Microsoft\WinGet\Packages\Gyan.FFmpeg.Shared_Microsoft.Winget.Source_8wekyb3d8bbwe\ffmpeg-8.1.2-full_build-shared\bin";
    let mut env = FakeEnv::default()
        .var("PATH", &format!(r"C:\static;{links};C:\ff10;C:\half"))
        .dir(r"C:\static", &["ffmpeg.exe", "ffprobe.exe", "ffplay.exe"])
        .dir(links, &["ffmpeg.exe", "ffprobe.exe"])
        .dir(r"C:\ff10", &["avcodec-64.dll", "avutil-62.dll", "ffmpeg.exe"])
        .dir(r"C:\half", &["avcodec-63.dll", "avformat-63.dll", "swscale-10.dll", "swresample-7.dll"])
        .set(pkg_bin, Os::Windows, 8);
    env.links.insert(format!(r"{links}\ffmpeg.exe"), format!(r"{pkg_bin}\ffmpeg.exe"));

    let plan = search::plan(Os::Windows, &env);
    let c = chosen(&plan).expect("the link's package is chosen");
    assert_eq!(c.dir.as_deref(), Some(pkg_bin));
    assert_eq!(c.source, Source::PathLink);
    assert_eq!(c.verdict, Ok(8));

    let reason = |dir: &str| -> String {
        let c = plan.iter().find(|c| c.dir.as_deref() == Some(dir)).unwrap_or_else(|| panic!("{dir} in plan"));
        c.verdict.clone().unwrap_err().to_string()
    };
    assert_eq!(reason(r"C:\static"), "static build — ffmpeg.exe without DLLs");
    assert_eq!(reason(r"C:\ff10"), "FFmpeg 10 (avcodec 64) is not supported yet");
    assert_eq!(reason(r"C:\half"), "FFmpeg 9: no avutil-61.dll");
    assert!(
        plan.iter().all(|c| c.dir.as_deref() != Some(links)),
        "the folder of links is not reported as a static build"
    );
}

/// t862 (ffmpeg-9): the table of library majors equals the vendored headers of every version
/// module, and every row has a module (and the other way round) — a row bound to the wrong
/// module's struct layouts would corrupt memory instead of failing. Needs no FFmpeg.
#[test]
fn t862_major_table_equals_vendored_headers() {
    macro_rules! headers {
        ($m:ident) => {
            search::Majors {
                avutil: kt_ffmpeg::$m::ffi::LIBAVUTIL_VERSION_MAJOR as u32,
                swresample: kt_ffmpeg::$m::ffi::LIBSWRESAMPLE_VERSION_MAJOR as u32,
                swscale: kt_ffmpeg::$m::ffi::LIBSWSCALE_VERSION_MAJOR as u32,
                avcodec: kt_ffmpeg::$m::ffi::LIBAVCODEC_VERSION_MAJOR as u32,
                avformat: kt_ffmpeg::$m::ffi::LIBAVFORMAT_VERSION_MAJOR as u32,
            }
        };
    }
    // a new major extends all three: the module list here, `VERSION_MODULES`, `MAJORS`
    let modules = [(6, headers!(v6)), (7, headers!(v7)), (8, headers!(v8)), (9, headers!(v9))];
    let mut rows: Vec<u32> = search::MAJORS.iter().map(|(f, _)| *f).collect();
    rows.sort();
    let listed: Vec<u32> = modules.iter().map(|(f, _)| *f).collect();
    assert_eq!(kt_ffmpeg::VERSION_MODULES.to_vec(), listed, "VERSION_MODULES = the modules of lib.rs");
    assert_eq!(rows, listed, "a row of MAJORS for every module and a module for every row");
    for (ffmpeg, from_headers) in modules {
        let row = search::MAJORS.iter().find(|(f, _)| *f == ffmpeg).unwrap().1;
        assert_eq!(row, from_headers, "FFmpeg {ffmpeg}: MAJORS against vendor/ffmpeg-{ffmpeg}");
    }
    assert_eq!(
        search::MAJORS[0].1,
        search::Majors { avutil: 61, swresample: 7, swscale: 10, avcodec: 63, avformat: 63 },
        "FFmpeg 9 is the newest row (the table is newest first)"
    );
}

/// t863 (ffmpeg-9): the Linux plan asks the system for FFmpeg 9 first and accepts a folder
/// with the FFmpeg 9 SONAMEs; an incomplete 9 folder is named with the missing library.
#[test]
fn t863_linux_plan_knows_ffmpeg_9() {
    let srcs = |env: &FakeEnv| -> Vec<(Source, Option<String>, Result<u32, Reject>)> {
        search::plan(Os::Linux, env).into_iter().map(|c| (c.source, c.dir, c.verdict)).collect()
    };
    let system = |f: u32| (Source::System, None, Ok(f));

    assert_eq!(srcs(&FakeEnv::default()), vec![system(9), system(8), system(7), system(6)]);
    assert_eq!(search::file_name(Os::Linux, Lib::Avcodec, 9), "libavcodec.so.63");

    let full = FakeEnv::default().var("KINETREN_FFMPEG_DIR", "/opt/ff9").dir(
        "/opt/ff9",
        &["libavutil.so.61", "libswresample.so.7", "libswscale.so.10", "libavcodec.so.63", "libavformat.so.63"],
    );
    let plan = srcs(&full);
    assert_eq!(plan[0], (Source::EnvVar, Some("/opt/ff9".into()), Ok(9)));
    assert_eq!(&plan[1..], &[system(9), system(8), system(7), system(6)]);

    let half = FakeEnv::default().var("KINETREN_FFMPEG_DIR", "/opt/half").dir(
        "/opt/half",
        &["libswresample.so.7", "libswscale.so.10", "libavcodec.so.63", "libavformat.so.63"],
    );
    let plan = srcs(&half);
    assert_eq!(
        plan[0].2.clone().unwrap_err().to_string(),
        "FFmpeg 9: no libavutil.so.61",
        "the incomplete folder is named with the missing library"
    );
    assert_eq!(&plan[1..], &[system(9), system(8), system(7), system(6)], "the system follows it");
}

/// t864 (ffmpeg-9): an earlier download keeps working after the pin moves — every version
/// folder under the download root is a candidate, newest first; the downloader's temporary
/// entries are not.
#[test]
fn t864_earlier_download_keeps_working() {
    let root = r"C:\Users\u\AppData\Local\kinetren\ffmpeg";
    let downloads = |plan: &[Candidate]| -> Vec<(String, Result<u32, Reject>)> {
        plan.iter()
            .filter(|c| c.source == Source::Download)
            .map(|c| (c.dir.clone().unwrap(), c.verdict.clone()))
            .collect()
    };
    let at = |v: &str| format!(r"{root}\{v}");

    // only the folder of the earlier pin (8.1.2), no 9.0.2 yet
    let mut env = FakeEnv::default().dir(root, &["8.1.2"]).set(&at("8.1.2"), Os::Windows, 8);
    env.download_root = Some(root.into());
    let plan = search::plan(Os::Windows, &env);
    assert_eq!(downloads(&plan), vec![(at("8.1.2"), Ok(8))]);
    let c = chosen(&plan).expect("the earlier download is used");
    assert_eq!((c.source, c.dir.clone(), c.verdict.clone()), (Source::Download, Some(at("8.1.2")), Ok(8)));

    // both: the current pin comes first and wins; the earlier one stays a candidate
    let mut env = FakeEnv::default()
        .dir(root, &["8.1.2", "9.0.2"])
        .set(&at("8.1.2"), Os::Windows, 8)
        .set(&at("9.0.2"), Os::Windows, 9);
    env.download_root = Some(root.into());
    let plan = search::plan(Os::Windows, &env);
    assert_eq!(downloads(&plan), vec![(at("9.0.2"), Ok(9)), (at("8.1.2"), Ok(8))]);
    assert_eq!(chosen(&plan).unwrap().dir.as_deref(), Some(at("9.0.2").as_str()));

    // versions compare as numbers, not text; temporary and foreign entries are not candidates
    let mut env = FakeEnv::default()
        .dir(root, &[".partial-123", "9.0.2", "10.0", "notes.txt", "8.1.10", "8.1.2"])
        .set(&at("9.0.2"), Os::Windows, 9);
    env.download_root = Some(root.into());
    let dirs: Vec<String> = downloads(&search::plan(Os::Windows, &env)).into_iter().map(|(d, _)| d).collect();
    assert_eq!(dirs, vec![at("10.0"), at("9.0.2"), at("8.1.10"), at("8.1.2")]);

    // the root does not exist yet (nothing downloaded): no download candidate, nothing reported
    let env = FakeEnv { download_root: Some(root.into()), ..Default::default() };
    assert!(downloads(&search::plan(Os::Windows, &env)).is_empty());
}

/// t865 (ffmpeg-9): a run meant for one FFmpeg major proves that major loaded. With
/// `KT_EXPECT_FFMPEG_MAJOR=N` (`make ffmpeg-test`, the `ffmpeg9` CI job) the normal search must
/// load FFmpeg N — a broken folder silently falling back to another FFmpeg, or to none, fails
/// here with the search report instead of passing as an FFmpeg N run. Unset: skipped.
#[test]
fn t865_the_run_proves_its_ffmpeg_major() {
    let Ok(want) = std::env::var("KT_EXPECT_FFMPEG_MAJOR") else {
        eprintln!("t865 skipped: KT_EXPECT_FFMPEG_MAJOR is not set (only the FFmpeg runs set it)");
        return;
    };
    let want: u32 = want.trim().parse().expect("KT_EXPECT_FFMPEG_MAJOR is a number");
    let report = kt_ffmpeg::search_report();
    let info = kt_ffmpeg::backend_info()
        .unwrap_or_else(|| panic!("expected FFmpeg {want}, nothing loaded:\n{report}"));
    assert_eq!(info.major, want, "expected FFmpeg {want}, loaded FFmpeg {}:\n{report}", info.major);
    let avcodec = search::MAJORS.iter().find(|(f, _)| *f == want).expect("a supported major").1.avcodec;
    assert_eq!(info.avcodec_version.0, avcodec, "avcodec_version() of FFmpeg {want}");
    eprintln!("t865: {}", report.lines().next().unwrap_or_default());
}

/// t847: a failed load is not remembered; a successful one is, and is not repeated.
#[cfg(target_os = "linux")]
#[test]
fn t847_failure_not_cached_success_is() {
    use std::os::unix::fs::symlink;

    // A folder with the system FFmpeg's libraries under their SONAMEs.
    let sys = "/usr/lib/x86_64-linux-gnu";
    let Some(ffmpeg) = search::MAJORS.iter().map(|(f, _)| *f).find(|f| {
        LOAD_ORDER.iter().all(|l| std::path::Path::new(sys).join(search::file_name(Os::Linux, *l, *f)).exists())
    }) else {
        eprintln!("t847 skipped: no system FFmpeg 6/7/8/9 in {sys}");
        return;
    };
    let tmp = std::env::temp_dir().join(format!("kt-t847-{}", std::process::id()));
    let empty = tmp.join("empty");
    let full = tmp.join("full");
    std::fs::create_dir_all(&empty).unwrap();
    std::fs::create_dir_all(&full).unwrap();
    for l in LOAD_ORDER {
        let n = search::file_name(Os::Linux, l, ffmpeg);
        let _ = symlink(std::path::Path::new(sys).join(&n), full.join(&n));
    }

    struct Dirs(Vec<String>);
    impl Env for Dirs {
        fn var(&self, name: &str) -> Option<String> {
            (name == "KINETREN_FFMPEG_DIR").then(|| self.0[0].clone())
        }
        fn exe_dir(&self) -> Option<String> {
            None
        }
        fn user_choice(&self) -> Option<String> {
            None
        }
        fn download_root(&self) -> Option<String> {
            None
        }
        fn list(&self, dir: &str) -> Option<Vec<String>> {
            let rd = std::fs::read_dir(dir).ok()?;
            Some(rd.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect())
        }
        fn read_link(&self, _: &str) -> Option<String> {
            None
        }
        fn system_search(&self) -> bool {
            false
        }
    }

    let first = kt_ffmpeg::rescan_with(&Dirs(vec![empty.to_string_lossy().into()]));
    let err = first.expect_err("nothing to load in an empty folder");
    assert!(err.contains("no avcodec"), "the report names the empty folder: {err}");
    assert!(kt_ffmpeg::backend_info().is_none(), "the failure is reported, nothing is loaded");

    let second = kt_ffmpeg::rescan_with(&Dirs(vec![full.to_string_lossy().into()]));
    let info = second.expect("the full folder loads after a rescan");
    assert_eq!(info.major, ffmpeg);
    let opens = kt_ffmpeg::library_opens();

    let third = kt_ffmpeg::rescan_with(&Dirs(vec![empty.to_string_lossy().into()]));
    assert_eq!(third.expect("a success is remembered").major, ffmpeg);
    assert_eq!(kt_ffmpeg::library_opens(), opens, "no library was opened again");
    let report = kt_ffmpeg::search_report();
    assert!(report.contains(&full.to_string_lossy().to_string()), "{report}");
    let _ = std::fs::remove_dir_all(&tmp);
}

//! t851 (change windows-build, capability durability): the per-user directories per OS,
//! never the working directory.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;

use kt_dirs::{Dirs, Env, KnownFolder, Os, resolve};

struct Fake {
    os: Os,
    vars: HashMap<&'static str, &'static str>,
    roaming: Option<&'static str>,
    local: Option<&'static str>,
}

impl Env for Fake {
    fn os(&self) -> Os {
        self.os
    }
    fn var_os(&self, name: &str) -> Option<OsString> {
        self.vars.get(name).map(OsString::from)
    }
    fn known_folder(&self, f: KnownFolder) -> Option<PathBuf> {
        match f {
            KnownFolder::RoamingAppData => self.roaming.map(PathBuf::from),
            KnownFolder::LocalAppData => self.local.map(PathBuf::from),
        }
    }
    fn temp_dir(&self) -> PathBuf {
        PathBuf::from("/tmp-for-test")
    }
}

fn linux(vars: &[(&'static str, &'static str)]) -> Fake {
    Fake { os: Os::Linux, vars: vars.iter().copied().collect(), roaming: None, local: None }
}

#[test]
fn t851_directories_per_os_never_the_working_directory() {
    // a folder of the former name in the working directory: the resolver must not even look at it
    let former = "kinotrain";
    let cwd = std::env::temp_dir().join(format!("kt-t851-{}", std::process::id()));
    std::fs::create_dir_all(cwd.join(former)).unwrap();
    std::fs::write(cwd.join(former).join("fx-presets.json"), "[1]").unwrap();
    std::env::set_current_dir(&cwd).unwrap();

    let d = resolve(&linux(&[("XDG_DATA_HOME", "/x/data"), ("HOME", "/home/u")]));
    assert_eq!(d, Dirs { data: "/x/data/kinetren".into(), local: "/x/data/kinetren".into(), fallback: false });

    let d = resolve(&linux(&[("HOME", "/home/u")]));
    assert_eq!(d.data, PathBuf::from("/home/u/.local/share/kinetren"));
    assert!(!d.fallback);

    // a relative XDG_DATA_HOME is ignored, as eframe ignores it
    let d = resolve(&linux(&[("XDG_DATA_HOME", "rel"), ("HOME", "/home/u")]));
    assert_eq!(d.data, PathBuf::from("/home/u/.local/share/kinetren"));

    let d = resolve(&linux(&[]));
    assert_eq!(d.data, PathBuf::from("/tmp-for-test/kinetren"));
    assert!(d.fallback, "without HOME: temp with a warning, not the working directory");

    // Windows: Roaming for data (eframe's own directory), Local for downloads; no HOME
    let win = Fake {
        os: Os::Windows,
        vars: HashMap::new(),
        roaming: Some(r"C:\Users\u\AppData\Roaming"),
        local: Some(r"C:\Users\u\AppData\Local"),
    };
    let d = resolve(&win);
    assert_eq!(d.data, PathBuf::from(r"C:\Users\u\AppData\Roaming").join("kinetren").join("data"));
    assert_eq!(d.local, PathBuf::from(r"C:\Users\u\AppData\Local").join("kinetren"));
    assert!(!d.fallback);

    let none = Fake { os: Os::Windows, vars: HashMap::new(), roaming: None, local: None };
    let d = resolve(&none);
    assert_eq!(d.data, PathBuf::from("/tmp-for-test/kinetren/data"));
    assert!(d.fallback);

    // the explicit override, on both OSes
    let ov = std::env::temp_dir().join("kt-t851-override");
    let ov_s: &'static str = Box::leak(ov.to_string_lossy().into_owned().into_boxed_str());
    for os in [Os::Linux, Os::Windows] {
        let f = Fake { os, vars: [("KINETREN_DATA_DIR", ov_s)].into_iter().collect(), roaming: None, local: None };
        assert_eq!(resolve(&f), Dirs { data: ov.clone(), local: ov.clone(), fallback: false });
    }

    // nothing above created, moved or touched the working directory's folder
    assert_eq!(std::fs::read_to_string(cwd.join(former).join("fx-presets.json")).unwrap(), "[1]");
    assert!(!cwd.join("kinetren").exists());
    let _ = std::fs::remove_dir_all(&cwd);
}

/// t970 (flatpak-flathub, D8), the seam: a Flatpak is `FLATPAK_ID` or `/.flatpak-info`, nothing else.
#[test]
fn t970_the_sandbox_seam() {
    use kt_dirs::{Sandbox, sandbox_of};
    assert_eq!(sandbox_of(&linux(&[]), false), Sandbox::None);
    assert_eq!(sandbox_of(&linux(&[("FLATPAK_ID", "com.kinetren.Kinetren")]), false), Sandbox::Flatpak);
    assert_eq!(sandbox_of(&linux(&[]), true), Sandbox::Flatpak, "the info file alone");
    assert_eq!(sandbox_of(&linux(&[("FLATPAK_ID", "")]), false), Sandbox::None, "an empty id is no sandbox");
}

/// t1062 (ci-green-again): a canonical path is shown and keyed without the Windows verbatim
/// prefix where the plain spelling names the same file, and kept where it would not.
///
/// windows-latest ran t986 red: `std::fs::canonicalize` gave `\\?\C:\…`, and the welcome card
/// named a folder nobody had typed. The rule is pure over `Os`, so it is held on Linux too.
#[test]
fn t1062_the_verbatim_prefix_is_dropped_where_it_means_nothing() {
    use kt_dirs::{Os, plain_path};
    let w = |s: &str| plain_path(s, Os::Windows).into_owned();
    assert_eq!(w(r"\\?\C:\Users\me\film.kt"), r"C:\Users\me\film.kt");
    assert_eq!(w(r"\\?\UNC\host\share\film.kt"), r"\\host\share\film.kt");
    assert_eq!(w(r"C:\Users\me\film.kt"), r"C:\Users\me\film.kt", "a plain path stays");
    // kept: not a drive path, or too long for the plain spelling to open
    assert_eq!(w(r"\\?\Volume{1234}\film.kt"), r"\\?\Volume{1234}\film.kt");
    let long = format!(r"\\?\C:\{}\film.kt", "d".repeat(300));
    assert_eq!(w(&long), long, "a path past MAX_PATH keeps its prefix");
    // Linux paths are never touched, even a file literally named so
    assert_eq!(plain_path(r"\\?\C:\x", Os::Linux), r"\\?\C:\x");
    // the native call is canonical: an existing file resolves to an absolute path without it
    let dir = std::env::temp_dir().join(format!("kt-t1062-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let c = kt_dirs::canonical(&dir).unwrap();
    assert!(c.is_absolute(), "{}", c.display());
    assert!(!c.to_string_lossy().starts_with(r"\\?\"), "{}", c.display());
    assert_eq!(std::fs::canonicalize(&c).unwrap(), std::fs::canonicalize(&dir).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

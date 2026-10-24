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

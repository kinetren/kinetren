//! A temporary dump of the thumbnail sample (not committed).
use kt_fx::video::{FxScratch, render_fx_thumb, test_pattern};
use kt_model::FxInstance;
use xxhash_rust::xxh3::xxh3_64;

#[test]
fn dump() {
    let dir = std::env::var("KT_DUMP_DIR")
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
    for (name, w, h, alpha) in
        [("ref", 320u32, 180u32, false), ("ref-alpha", 320, 180, true), ("icon", 96, 54, false)]
    {
        let p = test_pattern(w, h, alpha);
        println!("{name} {w}x{h} alpha={alpha} 0x{:016x}", xxh3_64(&p.rgba));
        std::fs::write(format!("{dir}/pat-{name}-{}x{}.raw", p.w, p.h), &p.rgba).unwrap();
    }
    // and several effects on the sample: look at what is visible
    let p = test_pattern(320, 180, false);
    let pa = test_pattern(320, 180, true);
    let mut s = FxScratch::default();
    for (name, base, fx) in [
        ("mirror", &p, FxInstance::Mirror { horizontal: true, vertical: false }),
        ("blur", &p, FxInstance::blur(8)),
        ("edges-ink", &p, FxInstance::edges(90)),
        ("vhs-trash", &p, FxInstance::vhs(10, 0.85)),
        ("green", &p, FxInstance::greenscreen([0, 255, 0], 70, 25)),
        ("pixelate", &p, FxInstance::pixelate()),
        ("halftone", &p, FxInstance::halftone()),
        ("glow", &p, FxInstance::glow(0.9)),
        ("shadow", &pa, FxInstance::drop_shadow()),
        ("twirl", &p, FxInstance::twirl()),
    ] {
        let f = render_fx_thumb(base, &[fx], 320, 180, 0.5, 90_000, None, &mut s);
        std::fs::write(format!("{dir}/fx-{name}.raw"), &f.rgba).unwrap();
    }
}

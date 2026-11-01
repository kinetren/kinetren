//! t1051 (plugin-api-barriers, 1.1): the CPU bytes of the twelve barrier-position kinds, frozen
//! BEFORE they move into plugins.
//!
//! Each kind is built from its FILE text (`{"kind": …}`), not from a constructor: the same line
//! reads as the core variant today and as the plugin slot after the move, so the table proves the
//! move changed nothing a project can see. Rows: the default and two non-default forms per kind, at
//! K=1 and K=2 (the barrier's `res_scale`, on a frame twice the size), on an opaque frame and on one
//! with transparent margins. The sizes are not multiples of 8, so the AVX2 tails run too.
//!
//! A new row is printed with `KT_PRINT_GOLDEN=1 cargo test -p kt-fx --test barrier_plugin` and
//! pasted ONCE; a changed hash is a moved byte, never a table to refresh.

use kt_fx::video::{apply_barrier, Frame, FxScratch, VideoFxCtx};
use kt_model::FxInstance;

/// The file forms per kind: the default and two forms that move every row the kind has.
const RECIPES: &[(&str, &[&str])] = &[
    ("blur", &[r#"{}"#, r#"{"radius":3,"radius_y":7}"#, r#"{"radius":12}"#]),
    (
        "glow",
        &[r#"{}"#, r#"{"strength":1.2,"radius":5,"threshold":100,"tint_r":255,"tint_g":180,"tint_b":40}"#, r#"{"strength":0.3,"radius":2,"threshold":10}"#],
    ),
    ("edges", &[r#"{}"#, r#"{"threshold":20,"thickness":2,"color_r":255,"color_g":0,"color_b":0}"#, r#"{"threshold":90,"thickness":3}"#]),
    ("vhs", &[r#"{}"#, r#"{"rgb_shift":7,"bleed":0.8,"scan_step":3,"jitter":0.5}"#, r#"{"rgb_shift":1,"bleed":0.1,"scan_step":5}"#]),
    ("noise", &[r#"{}"#, r#"{"amount":0.6,"size":3,"color":true}"#, r#"{"amount":0.15,"size":2}"#]),
    (
        "greenscreen",
        &[r#"{}"#, r#"{"key_r":0,"key_g":255,"key_b":0,"tolerance":40,"softness":60,"spill":0.5}"#, r#"{"key_r":200,"key_g":40,"key_b":20,"tolerance":90,"softness":10}"#],
    ),
    ("sharpen", &[r#"{}"#, r#"{"amount":2.5,"radius":4}"#, r#"{"amount":0.4,"radius":1}"#]),
    ("pixelate", &[r#"{}"#, r#"{"size":5}"#, r#"{"size":2}"#]),
    ("chromatic_aberration", &[r#"{}"#, r#"{"amount":0.03}"#, r#"{"amount":0.008}"#]),
    ("radial_blur", &[r#"{}"#, r#"{"amount":0.6,"center_x":0.3,"center_y":0.6}"#, r#"{"amount":0.4,"spin":true}"#]),
    (
        "drop_shadow",
        &[r#"{}"#, r#"{"offset_x":-5,"offset_y":3,"radius":6,"opacity":0.8,"tint_r":30,"tint_g":0,"tint_b":90}"#, r#"{"offset_x":2,"offset_y":2,"radius":1,"opacity":0.3}"#],
    ),
    ("halftone", &[r#"{}"#, r#"{"size":5,"angle":30}"#, r#"{"size":12,"angle":0}"#]),
];

/// The frozen hashes: (kind, recipe index, K, alpha frame) → xxh3 of the output bytes.
const GOLDEN: &[(&str, usize, u32, bool, u64)] = &[
    ("blur", 0, 1, false, 0x5f7bec60efe2b937),
    ("blur", 0, 1, true, 0x5afbc8707e58bf5f),
    ("blur", 0, 2, false, 0x47220dfaa0ad1f0d),
    ("blur", 0, 2, true, 0x2fbffd32768779c6),
    ("blur", 1, 1, false, 0x47ecd0fb673b8855),
    ("blur", 1, 1, true, 0x93b73bb52432a9fc),
    ("blur", 1, 2, false, 0x4b2a32dca09a1fa9),
    ("blur", 1, 2, true, 0x2b0f1259e9494a9f),
    ("blur", 2, 1, false, 0x89b5adee916e7ede),
    ("blur", 2, 1, true, 0x8d6ec5fe0225cfae),
    ("blur", 2, 2, false, 0x0bc915be5750918d),
    ("blur", 2, 2, true, 0x81511ced28f12144),
    ("glow", 0, 1, false, 0xf90294d6b667d0c9),
    ("glow", 0, 1, true, 0x128267969e2f0786),
    ("glow", 0, 2, false, 0x1e52c00ebaddd06f),
    ("glow", 0, 2, true, 0x2ee668fabd6294f6),
    ("glow", 1, 1, false, 0xfb756811e343eb17),
    ("glow", 1, 1, true, 0x87c65daa8624f5ec),
    ("glow", 1, 2, false, 0xa1915231d9e94c57),
    ("glow", 1, 2, true, 0x962d6499d1ca34c0),
    ("glow", 2, 1, false, 0x13b835a3d7b81ac3),
    ("glow", 2, 1, true, 0x90c9f0e864d7f4e3),
    ("glow", 2, 2, false, 0x708414b6d5fcc437),
    ("glow", 2, 2, true, 0x3e83191c9a124140),
    ("edges", 0, 1, false, 0x33455be4d05a6552),
    ("edges", 0, 1, true, 0x5d29a2dd896f65b6),
    ("edges", 0, 2, false, 0xbbf47a4e8521776d),
    ("edges", 0, 2, true, 0x7a310721f9af4711),
    ("edges", 1, 1, false, 0x5a895cf9efcbd8e1),
    ("edges", 1, 1, true, 0x0c631b73b1eb1b93),
    ("edges", 1, 2, false, 0x0075431a79b9dd37),
    ("edges", 1, 2, true, 0xbca46907ad4c550d),
    ("edges", 2, 1, false, 0x98abdfe944a3b79b),
    ("edges", 2, 1, true, 0x91f9c33cd7dad425),
    ("edges", 2, 2, false, 0x9cfa829a704c7c74),
    ("edges", 2, 2, true, 0x59eb034980be7071),
    ("vhs", 0, 1, false, 0x0232040eeff0613e),
    ("vhs", 0, 1, true, 0xb0a47d801288d980),
    ("vhs", 0, 2, false, 0x38fa23e128aa51b7),
    ("vhs", 0, 2, true, 0x7fa3c6483dfec471),
    ("vhs", 1, 1, false, 0x857aad14ec9bdbc6),
    ("vhs", 1, 1, true, 0xac365d482bb4b9ef),
    ("vhs", 1, 2, false, 0xf6a165c228ea9a51),
    ("vhs", 1, 2, true, 0x0719f4edd518defc),
    ("vhs", 2, 1, false, 0x6961c59c15221641),
    ("vhs", 2, 1, true, 0x08dc137f8288fb80),
    ("vhs", 2, 2, false, 0x792b8404781550d0),
    ("vhs", 2, 2, true, 0x163bc110304a5af4),
    ("noise", 0, 1, false, 0x5fe3f5b627925179),
    ("noise", 0, 1, true, 0xc3485fa8c369c6b0),
    ("noise", 0, 2, false, 0xf3a10701f0b6f86e),
    ("noise", 0, 2, true, 0x78ebd6def9e60627),
    ("noise", 1, 1, false, 0x9881529d27c80633),
    ("noise", 1, 1, true, 0x2e4e68d3b558caa0),
    ("noise", 1, 2, false, 0x7935a7f67e7d1072),
    ("noise", 1, 2, true, 0x99b62d769085a456),
    ("noise", 2, 1, false, 0x1ed1309a4728dfd4),
    ("noise", 2, 1, true, 0xb4ca20f761e11338),
    ("noise", 2, 2, false, 0x29834d65f247fe7d),
    ("noise", 2, 2, true, 0xbbd07b719c043413),
    ("greenscreen", 0, 1, false, 0xb0f4d13040ec2a7a),
    ("greenscreen", 0, 1, true, 0xf52a205df30fe230),
    ("greenscreen", 0, 2, false, 0x23854576681e14eb),
    ("greenscreen", 0, 2, true, 0x17c7203537a84ce9),
    ("greenscreen", 1, 1, false, 0xfa81ce967eb88fd3),
    ("greenscreen", 1, 1, true, 0x082881636486ef4a),
    ("greenscreen", 1, 2, false, 0x73cde7629f7a0759),
    ("greenscreen", 1, 2, true, 0x0bbdfd3498be29cc),
    ("greenscreen", 2, 1, false, 0x2d53ac69499a5c77),
    ("greenscreen", 2, 1, true, 0x5ca2d07ea7d0b78e),
    ("greenscreen", 2, 2, false, 0x899f6e80213d301f),
    ("greenscreen", 2, 2, true, 0x4f745f1522a1b442),
    ("sharpen", 0, 1, false, 0x129532cc4ef67894),
    ("sharpen", 0, 1, true, 0xb517041e615bf68d),
    ("sharpen", 0, 2, false, 0xa8467777b1a3e4bf),
    ("sharpen", 0, 2, true, 0x2b07d1d33eebaf07),
    ("sharpen", 1, 1, false, 0x24e4542c4ad417d7),
    ("sharpen", 1, 1, true, 0x8770418e1c15bce9),
    ("sharpen", 1, 2, false, 0x1c3d5d164cf23769),
    ("sharpen", 1, 2, true, 0x2887aab72b0fe3a7),
    ("sharpen", 2, 1, false, 0x56617df0f671bd04),
    ("sharpen", 2, 1, true, 0x18ac3d441702b72c),
    ("sharpen", 2, 2, false, 0x609df953d48fc75c),
    ("sharpen", 2, 2, true, 0xa3412dd4a69f1c3c),
    ("pixelate", 0, 1, false, 0x6182c30449427efd),
    ("pixelate", 0, 1, true, 0x7e744066bcccbd87),
    ("pixelate", 0, 2, false, 0x6805f46840872056),
    ("pixelate", 0, 2, true, 0x4a2ed0d37ab9f8cb),
    ("pixelate", 1, 1, false, 0xc0a4de2362a4b629),
    ("pixelate", 1, 1, true, 0x5a037e448e523a7c),
    ("pixelate", 1, 2, false, 0xec93a41d713f2cf5),
    ("pixelate", 1, 2, true, 0x49f12598b8de63ac),
    ("pixelate", 2, 1, false, 0x6b2b4a197690a1dc),
    ("pixelate", 2, 1, true, 0x1a4c08f4eebd1eed),
    ("pixelate", 2, 2, false, 0xb91e89e79ecab973),
    ("pixelate", 2, 2, true, 0x1b1d35b53c458534),
    ("chromatic_aberration", 0, 1, false, 0x033493318a87e6f0),
    ("chromatic_aberration", 0, 1, true, 0x64e600e23467e5c4),
    ("chromatic_aberration", 0, 2, false, 0x1ac491c6aa1f2d44),
    ("chromatic_aberration", 0, 2, true, 0x5ae52d0600f14968),
    ("chromatic_aberration", 1, 1, false, 0x626cdf7f1b296ec0),
    ("chromatic_aberration", 1, 1, true, 0x20a299906b136805),
    ("chromatic_aberration", 1, 2, false, 0x89a952ed4b0c2a5d),
    ("chromatic_aberration", 1, 2, true, 0x5a68acc5ee8ba7a0),
    ("chromatic_aberration", 2, 1, false, 0xbe63ca9edea79947),
    ("chromatic_aberration", 2, 1, true, 0xcf864cdf03070ab1),
    ("chromatic_aberration", 2, 2, false, 0x4d7283e3fb57c503),
    ("chromatic_aberration", 2, 2, true, 0xf6dbeffd2e44470f),
    ("radial_blur", 0, 1, false, 0x2ff9bbb50668fca5),
    ("radial_blur", 0, 1, true, 0x975afc5a86dd3024),
    ("radial_blur", 0, 2, false, 0xb95f820d46692d70),
    ("radial_blur", 0, 2, true, 0x5a732af9bc58cf5c),
    ("radial_blur", 1, 1, false, 0x3b708b5c41829923),
    ("radial_blur", 1, 1, true, 0x04caa5661f05c12a),
    ("radial_blur", 1, 2, false, 0x8af8e3777348c8f1),
    ("radial_blur", 1, 2, true, 0x0eb4591176715ac9),
    ("radial_blur", 2, 1, false, 0x9e2f1754b93f63d1),
    ("radial_blur", 2, 1, true, 0x11549f2a0e45e971),
    ("radial_blur", 2, 2, false, 0x39bedaf81a315348),
    ("radial_blur", 2, 2, true, 0x79c04b20f70a901f),
    ("drop_shadow", 0, 1, false, 0x5fe3f5b627925179),
    ("drop_shadow", 0, 1, true, 0x9a24a5d9342ae610),
    ("drop_shadow", 0, 2, false, 0xf3a10701f0b6f86e),
    ("drop_shadow", 0, 2, true, 0xdcb4ae93facdafc3),
    ("drop_shadow", 1, 1, false, 0x5fe3f5b627925179),
    ("drop_shadow", 1, 1, true, 0x435fcd4ab7810a96),
    ("drop_shadow", 1, 2, false, 0xf3a10701f0b6f86e),
    ("drop_shadow", 1, 2, true, 0x222e47389d66b53d),
    ("drop_shadow", 2, 1, false, 0x5fe3f5b627925179),
    ("drop_shadow", 2, 1, true, 0xfc5d604e642dda20),
    ("drop_shadow", 2, 2, false, 0xf3a10701f0b6f86e),
    ("drop_shadow", 2, 2, true, 0x77081e8735ff92b2),
    ("halftone", 0, 1, false, 0x19b9c9d01901daee),
    ("halftone", 0, 1, true, 0xe757dfbc73c806c4),
    ("halftone", 0, 2, false, 0x75f37707383ae4f9),
    ("halftone", 0, 2, true, 0x6dd1a3c1090dd05c),
    ("halftone", 1, 1, false, 0xf2ae4958c85dc42b),
    ("halftone", 1, 1, true, 0x43a3348e43383c3c),
    ("halftone", 1, 2, false, 0xfbd1a7033df2eaf2),
    ("halftone", 1, 2, true, 0x7ad440c3fe17a150),
    ("halftone", 2, 1, false, 0xa2073cc9b2c566d4),
    ("halftone", 2, 1, true, 0x239c10cd5fea2813),
    ("halftone", 2, 2, false, 0x8953dfdf25ffdcac),
    ("halftone", 2, 2, true, 0xf547236f0dc66364),
];

fn fx(kind: &str, body: &str) -> FxInstance {
    let mut v: serde_json::Value = serde_json::from_str(body).expect("recipe json");
    v.as_object_mut().expect("recipe object").insert("kind".into(), kind.into());
    serde_json::from_value(v).unwrap_or_else(|e| panic!("{kind} {body}: {e}"))
}

/// Gradients, hard edges, a green plate (greenscreen) and, with `alpha`, transparent margins
/// (drop_shadow draws only under alpha). `s` scales the logical 50×30 pattern for K.
fn frame(s: u32, alpha: bool) -> Frame {
    let (w, h) = (50 * s, 30 * s);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (lx, ly) = (x / s, y / s);
            let i = ((y * w + x) * 4) as usize;
            let px = if lx > 30 && ly > 15 {
                [20, 240, 30]
            } else if (lx / 5 + ly / 5) % 2 == 0 {
                [(lx * 5) as u8, (ly * 8) as u8, ((lx + ly) * 3) as u8]
            } else {
                [230, 220 - (ly * 4) as u8, 40]
            };
            rgba[i..i + 3].copy_from_slice(&px);
            let margin = !(6..44).contains(&lx) || !(4..26).contains(&ly);
            rgba[i + 3] = if alpha && margin { 0 } else { 255 };
        }
    }
    Frame { w, h, rgba, has_alpha: alpha }
}

fn ctx(s: u32) -> VideoFxCtx {
    VideoFxCtx { out_w: 50 * s, out_h: 30 * s, u: 0.5, burst_index: 90_000, cam3d: None, in_scale: 1, lead: None }
}

#[test]
fn t1051_barrier_bytes_are_frozen_before_the_move() {
    install();
    let print = std::env::var_os("KT_PRINT_GOLDEN").is_some();
    let mut got = Vec::new();
    for (kind, forms) in RECIPES {
        for (n, body) in forms.iter().enumerate() {
            let f = fx(kind, body);
            assert_eq!(f.tag(), *kind, "the file form reads as another kind");
            for k in [1u32, 2] {
                for alpha in [false, true] {
                    let out = apply_barrier(&f, frame(k, alpha), &ctx(k), &mut FxScratch::default(), k);
                    let h = xxhash_rust::xxh3::xxh3_64(&out.rgba) ^ ((out.w as u64) << 48) ^ ((out.has_alpha as u64) << 63);
                    got.push((*kind, n, k, alpha, h));
                }
            }
        }
    }
    if print {
        for (kind, n, k, alpha, h) in &got {
            println!("    ({kind:?}, {n}, {k}, {alpha}, 0x{h:016x}),");
        }
    }
    assert_eq!(got.len(), GOLDEN.len(), "a row without a hash or a hash without a row");
    for (g, w) in got.iter().zip(GOLDEN) {
        assert_eq!(g, w, "{} form {} K={} alpha={}: the bytes moved", g.0, g.1, g.2, g.3);
    }
}

// ---- t1055 / t1053: the host's rules over every registered barrier --------------------------------

use std::sync::atomic::{AtomicU32, Ordering};

/// The K the probe's CPU kernel last received.
static PROBE_K: AtomicU32 = AtomicU32::new(0);

/// A barrier plugin the host names nowhere: it records the K it is handed and passes the frame
/// through. CPU only (a test plugin, not a built-in: no GPU side is required of it).
struct KProbe {
    m: kt_plugin_api::Manifest,
}

impl KProbe {
    fn new() -> KProbe {
        KProbe {
            m: kt_plugin_api::Manifest {
                id: "k_probe".into(),
                media: kt_plugin_api::Media::VideoFx,
                passport: Vec::new(),
                rows: Vec::new(),
                decl: kt_plugin_api::Decl {
                    video_class: Some(kt_plugin_api::VideoClass::Barrier),
                    gpu: kt_plugin_api::Gpu::None,
                    builtin: false,
                    ..kt_plugin_api::Decl::default()
                },
                ui: Default::default(),
            },
        }
    }
}

impl kt_plugin_api::Plugin for KProbe {
    fn manifest(&self) -> &kt_plugin_api::Manifest {
        &self.m
    }
    fn barrier(&self) -> Option<&dyn kt_plugin_api::barrier::BarrierKernel> {
        Some(self)
    }
}

impl kt_plugin_api::barrier::BarrierKernel for KProbe {
    fn frame(
        &self,
        _k: &kt_plugin_api::Cooked,
        f: Frame,
        cx: &kt_plugin_api::barrier::BarrierCtx,
        _s: &mut dyn kt_plugin_api::barrier::Scratch,
    ) -> Frame {
        PROBE_K.store(cx.res_scale, Ordering::SeqCst);
        f
    }
}

/// The plugin set of this binary: the built-ins plus `k_probe`, installed once.
fn install() {
    kt_plugins_builtin::install_with(|reg| {
        reg.register_for_test(std::sync::Arc::new(KProbe::new())).expect("k_probe registers");
    });
}

/// t1055 (plugin-api-barriers 4.3; capability `video-fx`): a barrier plugin the host names nowhere is
/// held to the K rule — between a 2×2 grid and a ×2 zoom, and alone before a zoom at a decode step
/// of 2, the planner gives it the K a built-in barrier gets at that place, and its kernel receives it.
#[test]
fn t1055_an_unknown_barrier_is_held_to_the_k_rule() {
    install();
    use kt_fx::plan::{PlanOp, plan_chain};
    let probe = FxInstance::plugin("k_probe").expect("the test plugin is installed");
    let vhs = FxInstance::vhs(4, 0.4);
    let grid = FxInstance::plugin_with("event_grid", &[2.0, 2.0, 0.0]).unwrap();
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let k_of = |fx: &[FxInstance], s: u32| -> Vec<u32> {
        plan_chain(fx, 50 * s, 30 * s, s, 50, 30, 0.5, None, 90_000, &[])
            .ops
            .iter()
            .filter_map(|op| match op {
                PlanOp::Barrier { res_scale, .. } => Some(*res_scale),
                _ => None,
            })
            .collect()
    };
    for (what, chain, s) in [
        ("between a grid and a zoom", vec![grid.clone(), probe.clone(), zoom2.clone()], 1),
        ("before a zoom at a decode step of 2", vec![probe.clone(), zoom2.clone()], 2),
    ] {
        let as_vhs: Vec<FxInstance> = chain.iter().map(|f| if f == &probe { vhs.clone() } else { f.clone() }).collect();
        assert_eq!(k_of(&chain, s), vec![2], "{what}: the planner gave the plugin another K");
        assert_eq!(k_of(&chain, s), k_of(&as_vhs, s), "{what}: the plugin and a built-in barrier get different K");
        PROBE_K.store(0, Ordering::SeqCst);
        let ctx = VideoFxCtx { in_scale: s, ..ctx(1) };
        let _ = kt_fx::video::apply_video_chain(&chain, frame(s, false), &ctx);
        assert_eq!(PROBE_K.load(Ordering::SeqCst), 2, "{what}: the kernel was handed another K");
    }
}

/// t1053 (plugin-api-barriers 4.4; capability `plugins`): every registered barrier kernel that takes
/// a SIMD path gives the scalar bytes — over the registry, not a list: a kind added with a twin is
/// checked without editing the test. Defaults, the passport's top, the picker's variants; K 1 and 2;
/// opaque and alpha frames off the AVX2 width.
#[test]
fn t1053_every_barrier_twin_gives_the_scalar_bytes() {
    install();
    if !std::is_x86_feature_detected!("avx2") {
        assert!(std::env::var("KT_REQUIRE_SIMD").as_deref() != Ok("1"), "KT_REQUIRE_SIMD=1, but AVX2 is not on this CPU");
        eprintln!("t1053 skipped: the CPU has no AVX2 — the SIMD path is unreachable");
        return;
    }
    let reg = kt_plugin_api::installed().expect("installed");
    let mut kinds = 0;
    for p in reg.plugins().filter(|p| p.barrier().is_some() && p.manifest().media == kt_plugin_api::Media::VideoFx) {
        let m = p.manifest();
        let b = p.barrier().expect("filtered");
        let mut forms: Vec<Vec<f64>> = vec![
            kt_plugin_api::ParamValues::defaults(&m.passport).0,
            kt_plugin_api::ParamValues::top(&m.passport).0,
        ];
        for v in m.ui.variants {
            let mut vals = kt_plugin_api::ParamValues::defaults(&m.passport).0;
            for (name, x) in v.sets {
                let i = m.passport.iter().position(|s| s.name == *name).expect("a variant names a row");
                vals[i] = *x;
            }
            forms.push(vals);
        }
        for vals in &forms {
            for rs in [1u32, 2] {
                for alpha in [false, true] {
                    let run = |simd: bool| {
                        let cx = kt_plugin_api::barrier::BarrierCtx {
                            res_scale: rs,
                            burst_index: 90_000,
                            simd,
                            raster: &kt_fx::video::Raster,
                        };
                        b.frame_values(vals, frame(rs, alpha), &cx, &mut FxScratch::default()).rgba
                    };
                    assert!(run(true) == run(false), "{} {vals:?} K={rs} alpha={alpha}: the SIMD path moved a byte", m.id);
                }
            }
        }
        kinds += 1;
    }
    assert!(kinds >= 12, "the registry holds {kinds} barrier kinds, the twelve moved ones at least");
}

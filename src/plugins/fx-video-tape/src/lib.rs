//! Tape simulations (vhs-tape-simulation): `vhs_tape` writes the picture onto a modelled VHS tape
//! and reads it back — FM luma, colour-under chroma, helical scan with ±6° azimuth heads, tracking,
//! the channel of each head, noise, dropouts, head switching, the playback deck's demodulators and
//! its time base (note `docs/features/fx/simulate/vhs-tape-simulation.md`, change design D1–D11).
//! It is a SIMULATION: the look comes out of the process; the imitation `vhs` stays in
//! fx-video-stylize.
//!
//! The CPU text (`sim`, `deck`, `sig`) is the reference; every WGSL pass under `wgsl/` mirrors one
//! function of it and is edited in the same commit (t1186).

use std::sync::{Arc, OnceLock};

use kt_plugin_api::barrier::{AlphaRel, BarrierCtx, BarrierKernel, Frame, GpuCtx, Node, Scratch, Slot};
use kt_plugin_api::{
    Cooked, Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, RowDecl, Store, UiDecl, UiVariant,
    VideoClass,
};

pub mod deck;
pub mod sig;
pub mod sim;
pub mod standard;

use deck::{row, Deck, LT_W, WORDS};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(VhsTape::new())]
}

fn num(name: &'static str, min: f64, max: f64, default: f64, step: f64, unit: ParamUnit) -> ParamSpec {
    ParamSpec::num(name, min, max, default, step, unit, ParamKind::Smooth)
}

/// The passport, in file order (`deck::row`).
fn passport() -> Vec<ParamSpec> {
    let mut p = vec![
        ParamSpec::choice_row("standard", &["pal", "ntsc"], 0),
        ParamSpec::choice_row("speed", &["sp", "lp", "ep"], 0),
        ParamSpec::choice_row("decks", &["one", "two"], 0),
        ParamSpec::choice_row("heads", &["two", "four"], 0),
        ParamSpec::choice_row("transport", &["play", "still"], 0),
        ParamSpec::flag_row("tbc", false),
        ParamSpec::flag_row("doc", true),
        ParamSpec::flag_row("comb", true),
    ];
    for name in HEAD_ROWS {
        p.push(match name.rsplit('_').next() {
            Some("spacing") => num(name, 0.0, 5.0, 0.05, 0.01, ParamUnit::Um),
            Some("gap") => num(name, 0.1, 2.0, 0.3, 0.01, ParamUnit::Um),
            _ => num(name, -30.0, 0.0, 0.0, 0.1, ParamUnit::Db),
        });
    }
    p.extend([
        num("tracking_um", -60.0, 60.0, 0.0, 0.5, ParamUnit::Um),
        num("skew_um", -60.0, 60.0, 0.0, 0.5, ParamUnit::Um),
        num("cn_db", 20.0, 60.0, 43.0, 0.5, ParamUnit::Db),
        num("dropouts_per_s", 0.0, 500.0, 0.5, 0.5, ParamUnit::None),
        num("dropout_us", 0.5, 60.0, 6.0, 0.5, ParamUnit::Us),
        num("jitter_us", 0.0, 2.0, 0.05, 0.01, ParamUnit::Us),
        num("switch_lines", 2.0, 12.0, 6.5, 0.5, ParamUnit::None),
        num("switch_skew_us", -5.0, 5.0, 0.8, 0.05, ParamUnit::Us),
        num("white_clip", 1.0, 2.5, 1.6, 0.01, ParamUnit::Percent),
        num("dark_clip", -0.8, 0.0, -0.45, 0.01, ParamUnit::Percent),
        num("chroma_db", -24.0, -3.0, -12.0, 0.5, ParamUnit::Db),
        num("tape_saturation", 0.0, 1.0, 0.25, 0.01, ParamUnit::Norm),
    ]);
    debug_assert_eq!(p.len(), row::COUNT);
    p
}

/// The head rows of drum 1 (records; plays too with one deck) and drum 2 (plays, with two decks).
pub const HEAD_ROWS: [&str; 12] = [
    "rec_a_spacing",
    "rec_a_gap",
    "rec_a_output",
    "rec_b_spacing",
    "rec_b_gap",
    "rec_b_output",
    "play_a_spacing",
    "play_a_gap",
    "play_a_output",
    "play_b_spacing",
    "play_b_gap",
    "play_b_output",
];

/// The passport's defaults (a good deck).
pub fn defaults() -> Vec<f64> {
    static D: OnceLock<Vec<f64>> = OnceLock::new();
    D.get_or_init(|| passport().iter().map(|s| s.default).collect()).clone()
}

fn rows() -> Vec<RowDecl> {
    passport()
        .iter()
        .map(|s| {
            let store = match s.name.as_ref() {
                "standard" => Store::Choice(&["pal", "ntsc"]),
                "speed" => Store::Choice(&["sp", "lp", "ep"]),
                "decks" => Store::Choice(&["one", "two"]),
                "heads" => Store::Choice(&["two", "four"]),
                "transport" => Store::Choice(&["play", "still"]),
                "tbc" | "doc" | "comb" => Store::Bool,
                _ => Store::F32,
            };
            RowDecl { store, omit_default: true }
        })
        .collect()
}

/// The presets: devices and tapes, not amounts.
pub const VARIANTS: &[UiVariant] = &[
    UiVariant { name_key: "fxreg.var.good_deck", sets: &[("tbc", 1.0), ("jitter_us", 0.02)] },
    UiVariant {
        name_key: "fxreg.var.dirty_heads",
        sets: &[("rec_a_spacing", 0.45), ("rec_b_spacing", 0.55), ("cn_db", 40.0), ("dropouts_per_s", 4.0)],
    },
    UiVariant { name_key: "fxreg.var.clogged_head", sets: &[("rec_a_spacing", 3.0)] },
    UiVariant {
        name_key: "fxreg.var.chipped_head",
        sets: &[("rec_b_output", -6.0), ("rec_b_gap", 0.6)],
    },
    UiVariant { name_key: "fxreg.var.still_4head", sets: &[("heads", 1.0), ("transport", 1.0)] },
    UiVariant {
        name_key: "fxreg.var.worn_tape",
        sets: &[
            ("decks", 1.0),
            ("cn_db", 36.0),
            ("dropouts_per_s", 60.0),
            ("dropout_us", 10.0),
            ("jitter_us", 0.35),
            ("skew_um", 22.0),
            ("tracking_um", 6.0),
            ("play_a_spacing", 0.2),
            ("play_b_spacing", 0.25),
        ],
    },
    UiVariant { name_key: "fxreg.var.lp", sets: &[("speed", 1.0)] },
];

/// VHS as a simulation: write the tape, read it back.
pub struct VhsTape {
    m: Manifest,
}

impl Default for VhsTape {
    fn default() -> VhsTape {
        VhsTape::new()
    }
}

impl VhsTape {
    pub fn new() -> VhsTape {
        VhsTape {
            m: Manifest {
                id: "vhs_tape".into(),
                media: Media::VideoFx,
                passport: passport(),
                rows: rows(),
                decl: Decl {
                    video_class: Some(VideoClass::Barrier),
                    gpu: Gpu::Barrier,
                    // the tape records what a camera saw over black: the output is opaque
                    alpha: AlphaRel::Destroys,
                    ..Decl::default()
                },
                ui: UiDecl {
                    name: "VHS Tape",
                    category_key: "fxreg.cat.simulate",
                    keywords_key: "fxreg.kw.vhs_tape",
                    variants: VARIANTS,
                    ..UiDecl::default()
                },
            },
        }
    }
}

impl Plugin for VhsTape {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn barrier(&self) -> Option<&dyn BarrierKernel> {
        Some(self)
    }
}

/// The shader indices of the graph.
mod sh {
    pub const UP: u8 = 0;
    pub const LT: u8 = 1;
    pub const S1: u8 = 2;
    pub const S2: u8 = 3;
    pub const S3: u8 = 4;
    pub const S4: u8 = 5;
    pub const S5: u8 = 6;
    pub const S6: u8 = 7;
    pub const S7: u8 = 8;
    pub const S8: u8 = 9;
    pub const FIN: u8 = 10;
}

/// The plugin's WGSL: the shared helpers and the kernels' constants prepended to each pass.
fn shader_texts() -> &'static [&'static str] {
    static S: OnceLock<Vec<&'static str>> = OnceLock::new();
    S.get_or_init(|| {
        let head = format!("{}\n{}\n", standard::wgsl_consts(), include_str!("wgsl/common.wgsl"));
        [
            include_str!("wgsl/up.wgsl"),
            include_str!("wgsl/lt.wgsl"),
            include_str!("wgsl/s1.wgsl"),
            include_str!("wgsl/s2.wgsl"),
            include_str!("wgsl/s3.wgsl"),
            include_str!("wgsl/s4.wgsl"),
            include_str!("wgsl/s5.wgsl"),
            include_str!("wgsl/s6.wgsl"),
            include_str!("wgsl/s7.wgsl"),
            include_str!("wgsl/s8.wgsl"),
            include_str!("wgsl/fin.wgsl"),
        ]
        .iter()
        .map(|p| &*Box::leak(format!("{head}{p}").into_boxed_str()))
        .collect()
    })
}

/// The values a cooked form carries (the first eight rows; the kernel reads the values directly).
fn values_of(k: &Cooked) -> Vec<f64> {
    let mut v = defaults();
    v[..8].copy_from_slice(k);
    v
}

impl BarrierKernel for VhsTape {
    fn frame(&self, k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        self.frame_values(&values_of(k), f, cx, scratch)
    }

    fn frame_values(&self, values: &[f64], mut f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        let d = Deck::new(values);
        let st = d.st;
        if f.has_alpha {
            cx.raster.premultiply(&mut f.rgba);
        }
        let small = cx.raster.resample(&f, st.a, st.h, scratch);
        let s = sim::simulate(&d, small.rgba, cx.burst_index);
        let mut out = scratch.take(f.rgba.len());
        sim::read_out(&s, f.w, f.h, &mut out);
        scratch.put(s.rgb);
        scratch.put(f.rgba);
        Frame { w: f.w, h: f.h, rgba: out, has_alpha: false }
    }

    fn shaders(&self) -> &'static [&'static str] {
        shader_texts()
    }

    fn gpu(&self, k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        self.gpu_values(&values_of(k), g, out)
    }

    /// The graph of design D2: the host's resample to the standard's grid, the parameter upload,
    /// the line table, the stages at the grid's size, the frame pass.
    fn gpu_values(&self, values: &[f64], g: &GpuCtx, out: &mut Vec<Node>) {
        let d = Deck::new(values);
        let st = d.st;
        let c = d.words(g.burst_index);
        let u = sim::Uni::new(&d, &c);
        let (w, h) = (st.w, st.h);
        let at = |shader: u8, inputs: [Slot; 3], uu: [u32; 8], ff: [f32; 8], w: u32, h: u32| Node::PassAt {
            shader,
            inputs,
            u: uu,
            f: ff,
            w,
            h,
        };
        let n = Slot::None;
        if g.has_alpha() {
            out.push(Node::Premul);
        }
        out.push(Node::Resample { w: st.a, h: st.h });
        out.push(Node::Save(0));
        for b in 0..WORDS / 16 {
            let mut uu = [0u32; 8];
            let mut ff = [0f32; 8];
            uu.copy_from_slice(&c[16 * b..16 * b + 8]);
            for (i, x) in ff.iter_mut().enumerate() {
                *x = f32::from_bits(c[16 * b + 8 + i]);
            }
            out.push(at(sh::UP, [if b == 0 { n } else { Slot::Prev }, n, n], uu, ff, 16 * (b as u32 + 1), 1));
        }
        out.push(at(sh::LT, [Slot::Prev, n, n], [0; 8], [0.0; 8], LT_W as u32, h));
        out.push(Node::Save(1));
        let std = (st.std == standard::Std::Ntsc) as u32;
        out.push(at(
            sh::S1,
            [Slot::Saved(0), n, n],
            [std, w, u.m, u.a, 0, 0, 0, 0],
            [u.dark, u.white, u.fsync_n, u.dev_n, u.l_sync, 0.0, 0.0, 0.0],
            w,
            h,
        ));
        out.push(at(sh::S2, [Slot::Prev, n, n], [w, 0, 0, 0, 0, 0, 0, 0], [0.0; 8], w, h));
        out.push(at(sh::S3, [Slot::Prev, n, n], [w, u.frame, 0, 0, 0, 0, 0, 0], [0.0; 8], w, h));
        out.push(at(
            sh::S4,
            [Slot::Prev, Slot::Saved(0), n],
            [std, w, u.m, u.a, u.v_switch as u32, 0, 0, 0],
            [u.a_c, u.sat, u.fcu_n, u.cu_line, u.rot[0], u.rot[1], u.sat_fm, u.sat_c],
            w,
            h,
        ));
        out.push(Node::Save(2));
        out.push(at(
            sh::S5,
            [Slot::Saved(2), Slot::Saved(1), n],
            [w, h, u.seed, 0, 0, 0, 0, 0],
            [u.burst, sim::BURST_HALF, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            w,
            h,
        ));
        out.push(Node::Save(3));
        out.push(at(sh::S6, [Slot::Prev, n, n], [std, w, 0, 0, 0, 0, 0, 0], [u.fsync_n, u.dev_n, u.l_sync, 0.0, 0.0, 0.0, 0.0, 0.0], w, h));
        out.push(at(
            sh::S7,
            [Slot::Prev, n, n],
            [std, w, u.doc as u32, 0, 0, 0, 0, 0],
            [sim::DOC_THRESHOLD, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            w,
            h,
        ));
        out.push(Node::Save(4));
        out.push(at(sh::S8, [Slot::Saved(3), Slot::Saved(1), n], [std, w, 0, 0, 0, 0, 0, 0], [u.fcu_n, u.cu_line, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], w, h));
        out.push(Node::Pass {
            shader: sh::FIN,
            inputs: [Slot::Saved(4), Slot::Prev, Slot::Saved(1)],
            u: [g.w, g.h, w, h, u.m, u.a, u.comb_rows, 0],
            f: [0.0; 8],
        });
    }
}

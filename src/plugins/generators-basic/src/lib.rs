//! The basic generators (plugin-api-stage1, stage 6, D13): backgrounds and shapes. Each kind is its
//! former `GeneratorKind` variant: the passport rows in the variant's field order with its stores
//! (the file text does not move — t973), the `gen:` path tokens of the former `format_gen_path` /
//! `apply_token` arms, and the frame of the former `render_at` arm, verbatim (t1048).

use std::sync::Arc;

use kt_plugin_api::Plugin;
use kt_plugin_api::generator::{GenDecl, PathTok, PathVal, Tick, fill, kind, rgb, u8v};

/// The creation menu's group of the backgrounds.
const BG: &str = "genkind.group_bg";

/// No path tokens, no address, one menu row of defaults, no value-named row.
const fn decl(alpha: bool, live: bool, path: &'static [PathTok], colors: &'static [(&'static str, &'static str)]) -> GenDecl {
    GenDecl { alpha, live, blank: false, path, address: &[], menu: &[], names: None, colors }
}

fn solid(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    fill(w, h, [u8v(p[0]), u8v(p[1]), u8v(p[2]), 255])
}

/// The family's kinds, in `BUILTIN_IDS` order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        // a solid colour — the only generator used daily (F-TXT-003); the default is black (UI rule 4)
        Arc::new(kind(
            "solid",
            "genkind.solid",
            BG,
            vec![rgb("color_r", 0.0), rgb("color_g", 0.0), rgb("color_b", 0.0)],
            decl(false, false, &[PathTok { key: "c", val: PathVal::Hex3("color") }], &[("color", "trn.color")]),
            solid,
        )),
    ]
}

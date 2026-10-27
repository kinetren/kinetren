//! Palette and theme (item 23 round 6, v2: the user's decision “more brutal” —
//! darker + monospaced digits + zero corner radii for widgets + outlined
//! buttons, but NOT flat: depth, accents and glow effects remain).
//! The key colors are collected here; the timeline (the largest area on screen)
//! is painted ONLY from these constants.

use eframe::egui::{self, Color32};

/// Panel background — almost black.
pub const PANEL: Color32 = Color32::from_rgb(9, 10, 12);
/// Background of windows/popups/groups.
pub const WINDOW: Color32 = Color32::from_rgb(16, 17, 20);
/// The darkest (input fields, “wells”).
pub const EXTREME: Color32 = Color32::from_rgb(4, 5, 7);
/// 1px seams between panels — visible, “welded”.
pub const SEAM: Color32 = Color32::from_rgb(48, 52, 60);
/// Zero line of the value axis on the automation graph (R.57, item 456).
/// It is NEEDED by ANY parameter whose scale crosses zero — speed, pan,
/// loudness in dB: without it, “above zero” and “below zero” differ only
/// by the label on the edge.
pub const AXIS_ZERO: Color32 = Color32::from_rgb(88, 94, 106);
/// Widget outline (buttons are outlined, not “pills”).
pub const OUTLINE: Color32 = Color32::from_rgb(42, 46, 54);
/// Accent (selection, active elements).
pub const ACCENT: Color32 = Color32::from_rgb(70, 130, 235);
/// Hover fill.
pub const HOVER: Color32 = Color32::from_rgb(38, 42, 50);
/// Frame of the filmstrip's preview frames.
/// Lit track badge (R.62): M / S / lock / FX. One color for all —
/// it is the GLYPH that tells them apart, not the shade (otherwise four new colors in the theme).
pub const BADGE_ON: egui::Color32 = egui::Color32::from_rgb(232, 196, 96);
pub const THUMB_FRAME: Color32 = Color32::from_rgb(0, 0, 0);

/// Background of the timeline strip (between tracks, behind the ruler).
pub const TL_BG: Color32 = Color32::from_rgb(11, 12, 14);
/// Track backgrounds: tones of the old editor, but darker (item 23 round 6).
pub const LANE_VIDEO: Color32 = Color32::from_rgb(19, 24, 32);
pub const LANE_AUDIO: Color32 = Color32::from_rgb(26, 20, 20);
pub const LANE_MORPH: Color32 = Color32::from_rgb(28, 20, 42);
/// Purple morph slot in the chain strip (R.35 item 3: “mark its items purple and raise them in the fx list”) —
/// the same tone as the morpher block on the timeline,
/// only lighter for legibility against the background.
pub const MORPH_SLOT: Color32 = Color32::from_rgb(190, 150, 255);
/// Outline of a named area (R.35 item 2): it has no strip of its own,
/// so its color must read over ANY track background.
pub const AREA: Color32 = Color32::from_rgb(120, 210, 190);
/// User color palette for an area (R.38 item 1): the field
/// `MarkedArea.color` lived in the format from R.35, but the UI had none of it — the color
/// could be neither chosen nor seen. It is stored as a `#rrggbb` string,
/// so both the palette and the parser live here.
pub const AREA_PALETTE: [(&str, Color32); 5] = [
    ("#78d2be", AREA),
    ("#e0b060", Color32::from_rgb(224, 176, 96)),
    ("#d07890", Color32::from_rgb(208, 120, 144)),
    ("#8fa8e8", Color32::from_rgb(143, 168, 232)),
    ("#a8d878", Color32::from_rgb(168, 216, 120)),
];

/// `#rrggbb` → color. A foreign file may carry anything, so the parsing is
/// strict, and `None` means “draw as usual”.
pub fn parse_hex(s: &str) -> Option<Color32> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// Camera tracks (R.29): dark turquoise — distinguishable from both video and morph.
pub const LANE_CAMERA: Color32 = Color32::from_rgb(16, 34, 38);

/// Tone of the rows of an EXPANDED STRIP (R.55, C5). Laid OVER the ordinary track background,
/// so the alpha is small: the track's look must remain recognizable
/// (video bluish, audio warm), and the tone only says “this is another reel”.
/// Two coordinate systems on one screen are indistinguishable without this.
pub const BAND_TINT: Color32 = Color32::from_rgba_premultiplied(30, 18, 44, 70);

/// A veil over the reel material BEYOND the instance window. It is there, it is visible, and
/// it can be edited — but it will not play until the edge is pulled.
pub const BAND_TAIL_VEIL: Color32 = Color32::from_black_alpha(150);

/// The boundary of the instance window inside the strip: up to it the material sounds, beyond it —
/// not. The grip of the edge is the same vertical.
pub const BAND_EDGE: Color32 = Color32::from_rgb(196, 152, 240);
/// Left (opaque) end of the gutter gradient (item 20 round 6).
pub const GUTTER: Color32 = Color32::from_rgb(22, 24, 28);

/// Common egui visual setup (called from run()).
pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = WINDOW;
    v.extreme_bg_color = EXTREME;
    // minimal corner radii 1px (refinement of round 6.1); windows slightly softened
    v.widgets.noninteractive.corner_radius = 1.into();
    v.widgets.inactive.corner_radius = 1.into();
    v.widgets.hovered.corner_radius = 1.into();
    v.widgets.active.corner_radius = 1.into();
    v.window_corner_radius = 2.into();
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = egui::Stroke::new(1.0, Color32::from_rgb(150, 190, 255));
    // buttons: dark slab + hard 1px outline; hover highlights the outline
    v.widgets.inactive.bg_fill = Color32::from_rgb(21, 23, 27);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(21, 23, 27);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, OUTLINE);
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(90, 120, 180));
    v.widgets.active.bg_fill = Color32::from_rgb(34, 56, 96);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, SEAM);
    // ONE look, whatever theme the system reports (wine-frame-and-dark-theme): `set_visuals`
    // styles only the CURRENT theme, and with the default "follow the system" a light system
    // theme (wine reports one, some Linux desktops too) switched egui to its default LIGHT
    // style — the standard parts turned white over our dark panels. The preference is fixed,
    // and our visuals are the dark style's.
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |st| st.visuals = v);
    ctx.all_styles_mut(|st| {
        // item 14 round 8: there are NO tooltips at all — the only place for a hint
        // is the bottom right of the status line. The blocking delay also mutes
        // foreign on_hover_text from egui widgets.
        st.interaction.tooltip_delay = 1.0e6;
        st.interaction.tooltip_grace_time = 0.0;
        // item 9/item 12 round 8: 3 px (the default) meant a 6-pixel grab zone for the separator —
        // a one-pixel miss landed on the preview
        st.interaction.resize_grab_radius_side = 8.0;
        st.interaction.resize_grab_radius_corner = 14.0;
        st.spacing.item_spacing = egui::vec2(6.0, 3.0);
        st.spacing.button_padding = egui::vec2(5.0, 2.0);
        st.spacing.window_margin = 4.into();
        st.spacing.menu_margin = 4.into();
    });
}

/// KT_DEBUG_HITS=1 — the built-in egui highlight of “who catches the click under the cursor”
/// (item 9 round 8: this is how complaints “button/strip does not click” are diagnosed).
pub fn apply_debug_flags(ctx: &egui::Context) {
    // Style::debug exists only in the debug build of egui — in the release
    // profile the field is absent entirely (caught ONLY by the release build).
    #[cfg(debug_assertions)]
    if std::env::var_os("KT_DEBUG_HITS").is_some() {
        ctx.all_styles_mut(|st| {
            st.debug.show_widget_hits = true;
            st.debug.debug_on_hover = true;
        });
    }
    #[cfg(not(debug_assertions))]
    let _ = ctx;
}

/// Panel frame without air at the edges (item 22 round 6) + 1px seam.
pub fn panel_frame() -> egui::Frame {
    egui::Frame::new().fill(PANEL).inner_margin(2)
}

/// Horizontal gradient (one mesh): left color on the left → right color on the right.
pub fn hgrad(painter: &egui::Painter, rect: egui::Rect, left: Color32, right: Color32) {
    let mut mesh = egui::Mesh::default();
    for (pos, col) in [
        (rect.left_top(), left),
        (rect.right_top(), right),
        (rect.right_bottom(), right),
        (rect.left_bottom(), left),
    ] {
        mesh.vertices.push(egui::epaint::Vertex { pos, uv: egui::epaint::WHITE_UV, color: col });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

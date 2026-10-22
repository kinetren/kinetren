//! Form of the event GENERATOR (R.59, stage 2).
//!
//! Before this round the generator and the title had THEIR OWN windows, and the parameters lived in the media path.
//! The owner put it verbatim: the generator was “stuffed in the
//! ass”, and its form must be the first crumb of the effects chain
//! of THE SAME event — next to everything else that is edited on the event. That is why there are no windows any more:
//! here lives one form, drawn by `fx_editor` in the pinned head
//! of the strip (`widgets::HeadSlot`), and the reusable pieces
//! of the former two windows — the creation menu, kind names, colour groups, font and glyph warnings, title presets.
//!
//! Values travel to the EVENT (`Command::SetClipGen`), curves —
//! `SetClipGenAuto`; “edit everywhere at once” is a separate command
//! `ApplyGenToAll` (the owner’s decision: editing one event no
//! longer touches its neighbours). The generator kind remains
//! a property of the MEDIA: this is its address and its cache
//! strip (the R.53 decision stands), so changing the kind is
//! `SetMediaPath`, and the model itself re-dresses the events (`fit_gen_to_media`).

use crate::i18n::{k, tr};
use crate::widgets;
use eframe::egui;
use kt_model::{GeneratorKind, HasParams, TextAlign, TextAnchor};

/// The localized name of the kind. Full paths — test 125 looks up the key by them.
/// The “Create generator…” submenu — ONE BODY for all entries (rule 17).
///
/// R.53: there are nineteen kinds, and a flat list of them is a mess. A group is a FUNCTION of the kind (`GenGroup::of`),
/// there is no second list: a new kind gets into the menu by itself. The rectangles of items and groups are exposed
/// for live scenarios (R.49): a submenu is not searched in the accessibility tree.
///
/// R.55 (C4 claim 8): the entry point is no longer the only one.
/// Previously the generator was created ONLY by a right click
/// on the empty background of the bin — “I don’t know how to
/// create a media generator; the code exists, but how to create
/// it in the UI is unclear”. Now the same list hangs in the
/// main menu too, and no second list of kinds was created for it.
pub fn generator_menu(ui: &mut egui::Ui) -> Option<GeneratorKind> {
    generator_kind_menu(ui, tr(k::bin::menu::NEW_GENERATOR))
}

/// The same menu under another caption: “create” and “change kind” — ONE list of kinds (rule 17),
/// and a new kind reaches both entries by itself.
pub fn generator_kind_menu(ui: &mut egui::Ui, label: &str) -> Option<GeneratorKind> {
    let mut picked = None;
    let mr = ui.menu_button(label, |ui| {
        for grp in GenGroup::ALL {
            let kinds: Vec<_> =
                GeneratorKind::all().into_iter().filter(|k| GenGroup::of(k) == grp).collect();
            if kinds.is_empty() {
                continue;
            }
            let gr = ui.menu_button(grp.name(), |ui| {
                for kd in kinds {
                    let r = ui.button(kind_name(&kd));
                    let probe = match &kd {
                        // the tables have one tag for five kinds — we
                        // tell them apart by variety (live scenarios)
                        GeneratorKind::TestPattern { table } => {
                            format!("gen.new.table.{}", table.token())
                        }
                        other => format!("gen.new.{}", other.tag()),
                    };
                    crate::widgets::field_probe::note(&probe, r.rect);
                    if r.clicked() {
                        picked = Some(kd);
                        ui.close();
                    }
                }
            });
            crate::widgets::field_probe::note(
                &format!("gen.new.group.{}", grp.probe()),
                gr.response.rect,
            );
        }
    });
    crate::widgets::field_probe::note("gen.new", mr.response.rect);
    picked
}

pub fn kind_name(kind: &GeneratorKind) -> &'static str {
    match kind {
        GeneratorKind::Solid { .. } => tr(k::genkind::SOLID),
        GeneratorKind::Text { .. } => tr(k::genkind::TEXT),
        GeneratorKind::Gradient { .. } => tr(k::genkind::GRADIENT),
        GeneratorKind::Corners { .. } => tr(k::genkind::CORNERS),
        GeneratorKind::Checker { .. } => tr(k::genkind::CHECKER),
        GeneratorKind::NoiseField { .. } => tr(k::genkind::NOISE),
        GeneratorKind::SmpteBars { .. } => tr(k::genkind::SMPTE),
        GeneratorKind::Timecode { .. } => tr(k::genkind::TIMECODE),
        GeneratorKind::Transparent => tr(k::genkind::TRANSPARENT),
        GeneratorKind::Rect { .. } => tr(k::genkind::RECT),
        GeneratorKind::Ellipse { .. } => tr(k::genkind::ELLIPSE),
        GeneratorKind::Line { .. } => tr(k::genkind::LINE),
        GeneratorKind::Leader { .. } => tr(k::genkind::LEADER),
        // the table variety is part of the NAME: there are five of them in the menu, and “just a table”
        // would not say which one (UI rule 18)
        GeneratorKind::TestPattern { table } => match table {
            kt_model::TestTable::Ebu75 => tr(k::genkind::EBU75),
            kt_model::TestTable::Ebu100 => tr(k::genkind::EBU100),
            kt_model::TestTable::Grayscale => tr(k::genkind::GRAYSCALE),
            kt_model::TestTable::Steps10 => tr(k::genkind::STEPS10),
            kt_model::TestTable::RampYcbcr => tr(k::genkind::YCBCR),
        },
        GeneratorKind::WindowPattern { .. } => tr(k::genkind::WINDOW),
    }
}

/// Whether the kind WITH DEFAULTS gives an empty frame (R.59, stage 2).
///
/// The values have moved to the event, and the bin card now shows the kind with its defaults.
/// For a title the default is an empty string; for a transparent backdrop it is empty by meaning:
/// instead of a black rectangle the card writes the KIND’S NAME (rule
/// 16 — the impossibility of showing a picture is visible, not silent).
pub fn blank_default(kind: &GeneratorKind) -> bool {
    match kind {
        GeneratorKind::Text { text, .. } => text.trim().is_empty(),
        GeneratorKind::Transparent => true,
        _ => false,
    }
}

/// Creation-menu group (R.53, item 422): the kinds have
/// become nineteen, and a flat list of them is a mess.
/// Groups are a function of the KIND; we do not make a second list.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenGroup {
    Background,
    Shapes,
    Titles,
    Check,
}

impl GenGroup {
    pub(crate) const ALL: [GenGroup; 4] =
        [GenGroup::Background, GenGroup::Shapes, GenGroup::Titles, GenGroup::Check];

    pub(crate) fn name(self) -> &'static str {
        match self {
            GenGroup::Background => tr(k::genkind::GROUP_BG),
            GenGroup::Shapes => tr(k::genkind::GROUP_SHAPES),
            GenGroup::Titles => tr(k::genkind::GROUP_TITLES),
            GenGroup::Check => tr(k::genkind::GROUP_CHECK),
        }
    }

    /// Stable group key for live scenarios (the
    /// name is localized and cannot be probed by it).
    pub(crate) fn probe(self) -> &'static str {
        match self {
            GenGroup::Background => "bg",
            GenGroup::Shapes => "shapes",
            GenGroup::Titles => "titles",
            GenGroup::Check => "check",
        }
    }

    pub(crate) fn of(kind: &GeneratorKind) -> GenGroup {
        match kind {
            GeneratorKind::Solid { .. }
            | GeneratorKind::Gradient { .. }
            | GeneratorKind::Corners { .. }
            | GeneratorKind::Transparent
            | GeneratorKind::NoiseField { .. } => GenGroup::Background,
            GeneratorKind::Rect { .. }
            | GeneratorKind::Ellipse { .. }
            | GeneratorKind::Line { .. } => GenGroup::Shapes,
            GeneratorKind::Text { .. }
            | GeneratorKind::Timecode { .. }
            | GeneratorKind::Leader { .. } => GenGroup::Titles,
            GeneratorKind::Checker { .. }
            | GeneratorKind::SmpteBars { .. }
            | GeneratorKind::TestPattern { .. }
            | GeneratorKind::WindowPattern { .. } => GenGroup::Check,
        }
    }
}

/// The kind’s colour groups for the form’s eyedroppers (caption, [names R,G,B]).
/// Public for t508: a parameter is either captioned or closed
/// by an eyedropper — there is no third option (R.53, item 414).
pub(crate) fn color_groups(
    kind: &GeneratorKind,
) -> &'static [(crate::i18n::Key, [&'static str; 3])] {
    match kind {
        GeneratorKind::Solid { .. } => &[(k::trn::COLOR, ["color_r", "color_g", "color_b"])],
        GeneratorKind::Gradient { .. } => {
            &[(k::genw::COLOR_A, ["a_r", "a_g", "a_b"]), (k::genw::COLOR_B, ["b_r", "b_g", "b_b"])]
        }
        GeneratorKind::Corners { .. } => &[
            (k::genw::CORNER_TL, ["tl_r", "tl_g", "tl_b"]),
            (k::genw::CORNER_TR, ["tr_r", "tr_g", "tr_b"]),
            (k::genw::CORNER_BL, ["bl_r", "bl_g", "bl_b"]),
            (k::genw::CORNER_BR, ["br_r", "br_g", "br_b"]),
        ],
        GeneratorKind::Checker { .. } => {
            &[(k::genw::COLOR_A, ["a_r", "a_g", "a_b"]), (k::genw::COLOR_B, ["b_r", "b_g", "b_b"])]
        }
        // shapes have a colour WITH ALPHA (R.53, item 419)
        GeneratorKind::Rect { .. } | GeneratorKind::Ellipse { .. } | GeneratorKind::Line { .. } => {
            &[(k::trn::COLOR, ["color_r", "color_g", "color_b"])]
        }
        GeneratorKind::Text { .. }
        | GeneratorKind::NoiseField { .. }
        | GeneratorKind::SmpteBars { .. }
        | GeneratorKind::Timecode { .. }
        | GeneratorKind::Transparent
        | GeneratorKind::Leader { .. }
        | GeneratorKind::TestPattern { .. }
        | GeneratorKind::WindowPattern { .. } => &[],
    }
}

/// The name of the alpha component of the kind’s colour group, if there is one (R.53, item 419).
/// Shapes have a four-component colour; the other kinds do not.
pub(crate) fn color_alpha(kind: &GeneratorKind) -> Option<&'static str> {
    matches!(
        kind,
        GeneratorKind::Rect { .. } | GeneratorKind::Ellipse { .. } | GeneratorKind::Line { .. }
    )
    .then_some("color_a")
}

/// Warning about a FONT SUBSTITUTION (R.52 S4): the chosen family is not on this machine — the frame is drawn
/// with the vendored one (the owner’s decision: “a miss =
/// substitution by the vendored one + a visible notice”).
pub fn font_warning(kind: &GeneratorKind) -> Option<String> {
    let GeneratorKind::Text { family, bold, .. } = kind else { return None };
    match kt_text::resolve_family(family, *bold) {
        kt_text::FontResolve::MissingFallback => {
            Some(crate::tf!(k::titlew::FONT_MISSING, family.clone()))
        }
        _ => None,
    }
}

/// Warning “glyph missing” for the current text (UI
/// rule 11: t100 checks the catalogs, but the title text
/// comes from the user — the answer must be visible, tofu in the frame is acceptable, silence is not).
pub fn title_warning(kind: &GeneratorKind) -> Option<String> {
    let GeneratorKind::Text { text, family, bold, .. } = kind else { return None };
    let style = if *bold { kt_text::FaceStyle::Bold } else { kt_text::FaceStyle::Regular };
    let missing = kt_text::uncovered(text, family, style);
    let first = *missing.first()?;
    Some(crate::tf!(
        k::titlew::MISSING,
        format!("“{first}” U+{:04X}", first as u32),
        missing.len().saturating_sub(1)
    ))
}

/// Cache of the form’s warnings per frame (R.53, item
/// 413): “glyph missing” iterates the characters through the
/// style resolution, “font missing” — the family map, and both were computed ON EVERY FRAME of the window.
/// The key is the canonical path of the kind with its values: they have no other entry.
#[derive(Default)]
pub struct GenWarn {
    key: Option<String>,
    missing: Option<String>,
    font: Option<String>,
}

impl GenWarn {
    fn refresh(&mut self, kind: &GeneratorKind) {
        let key = kt_model::format_gen_path(kind);
        if self.key.as_deref() == Some(key.as_str()) {
            return;
        }
        self.missing = title_warning(kind);
        self.font = font_warning(kind);
        self.key = Some(key);
    }
}

/// Title presets (R.53, item 424). The owner’s decision: **a preset sets EVERYTHING** — the size, position, anchor,
/// alignment, colours and animation — and this is one history step: pressed it, did not like it, one Ctrl+Z
/// restored it as it was.
///
/// The active preset is determined by the INVERSE function — by
/// comparison with what the preset would produce from the current text.
/// That is why a manual tweak honestly shows “matches none”, rather than highlighting a button
/// to which the title no longer has any relation.
pub struct TitlePreset {
    pub key: crate::i18n::Key,
    apply: fn(&mut GeneratorKind),
    /// The preset’s animation (R.59, stage 2): it is set by CURVES on the event, and not by a field of the kind.
    /// The formulas are `kt_model::text_anim_curves`, and this
    /// is their only place: old files are read with the same ones.
    pub anim: kt_model::TextAnim,
    pub anim_secs: f64,
}

pub fn title_presets() -> &'static [TitlePreset] {
    use kt_model::TextAnim;
    &[
        TitlePreset {
            key: k::titlew::PRESET_LOWER_THIRD,
            anim: TextAnim::None,
            anim_secs: 1.0,
            apply: |g| {
                let GeneratorKind::Text {
                    size,
                    pos_x,
                    pos_y,
                    anchor,
                    align,
                    plate_a,
                    plate_pad_x,
                    plate_pad_y,
                    ..
                } = g
                else {
                    return;
                };
                *size = 0.055;
                *pos_x = 0.08;
                *pos_y = 0.82;
                *anchor = TextAnchor::BottomLeft;
                *align = TextAlign::Left;
                *plate_a = 170;
                *plate_pad_x = 0.35;
                *plate_pad_y = 0.2;
            },
        },
        TitlePreset {
            key: k::titlew::PRESET_HEADLINE,
            anim: TextAnim::None,
            anim_secs: 1.0,
            apply: |g| {
                let GeneratorKind::Text {
                    size,
                    pos_x,
                    pos_y,
                    anchor,
                    align,
                    plate_a,
                    stroke_w,
                    ..
                } = g
                else {
                    return;
                };
                *size = 0.14;
                *pos_x = 0.5;
                *pos_y = 0.5;
                *anchor = TextAnchor::Center;
                *align = TextAlign::Center;
                *plate_a = 0;
                *stroke_w = 0.02;
            },
        },
        TitlePreset {
            key: k::titlew::PRESET_TYPEWRITER,
            anim: TextAnim::Typewriter,
            anim_secs: 2.0,
            apply: |g| {
                let GeneratorKind::Text { size, pos_x, pos_y, anchor, align, plate_a, .. } = g
                else {
                    return;
                };
                *size = 0.07;
                *pos_x = 0.5;
                *pos_y = 0.5;
                *anchor = TextAnchor::Center;
                *align = TextAlign::Center;
                *plate_a = 0;
            },
        },
        TitlePreset {
            key: k::titlew::PRESET_CRAWL,
            anim: TextAnim::Crawl,
            anim_secs: 12.0,
            apply: |g| {
                let GeneratorKind::Text { size, pos_x, pos_y, anchor, align, plate_a, .. } = g
                else {
                    return;
                };
                *size = 0.06;
                *pos_x = 0.5;
                *pos_y = 0.9;
                *anchor = TextAnchor::Center;
                *align = TextAlign::Left;
                *plate_a = 0;
            },
        },
        TitlePreset {
            key: k::titlew::PRESET_ROLL,
            anim: TextAnim::Roll,
            anim_secs: 20.0,
            apply: |g| {
                let GeneratorKind::Text { size, pos_x, pos_y, anchor, align, plate_a, .. } = g
                else {
                    return;
                };
                *size = 0.06;
                *pos_x = 0.5;
                *pos_y = 0.5;
                *anchor = TextAnchor::Center;
                *align = TextAlign::Center;
                *plate_a = 0;
            },
        },
    ]
}

impl TitlePreset {
    /// What the preset would do to THIS title (the text and the family are not touched).
    ///
    /// The values that the animation sets (`reveal`, offsets)
    /// are included here too: otherwise the “active preset”
    /// would stop recognizing its own result as soon as the curves landed on the event.
    pub fn applied(&self, base: &GeneratorKind) -> GeneratorKind {
        let mut g = base.clone();
        (self.apply)(&mut g);
        kt_model::text_anim_values(&mut g, self.anim);
        g
    }

    /// The preset’s curves in the domain of the event’s MATERIAL: the formulas live in `kt_model::text_anim_curves`
    /// (old files are read with them too — if the migration diverged
    /// from the preset, a project of a past build would go wrong),
    /// and the caller appends the anchor `source_in`.
    pub fn curves(
        &self,
        applied: &GeneratorKind,
        frame: (u32, u32),
        source_in: kt_time::Tick,
    ) -> Vec<kt_model::ParamCurve> {
        let mut curves = kt_model::text_anim_curves(
            self.anim,
            self.anim_secs,
            anim_span(applied, self.anim, frame),
        );
        for c in &mut curves {
            for key in &mut c.keys {
                key.t = kt_time::Tick(key.t.0 + source_in.0);
            }
        }
        curves
    }
}

/// The full path of the block in fractions of the frame: `1 + block_size/frame_size`.
/// The layout is computed by the SAME function that draws the frame and the
/// drag frame — there is no second text meter in the program (kt-text/CLAUDE.md).
fn anim_span(kind: &GeneratorKind, anim: kt_model::TextAnim, (w, h): (u32, u32)) -> f64 {
    let (w, h) = (w.max(1), h.max(1));
    let Some(info) = kt_media::generator::title_layout(kind, w, h) else { return 2.0 };
    match anim {
        kt_model::TextAnim::Crawl => 1.0 + info.block.w / w as f64,
        kt_model::TextAnim::Roll => 1.0 + info.block.h / h as f64,
        _ => 2.0,
    }
}

/// The number of the preset with which the title matches EXACTLY, or `None` (“matches none”).
/// The inverse function is a comparison of the result, not a separate flag “which preset was pressed”:
/// the flag would lie after the first manual edit.
///
/// Since R.59 a preset sets the CURVES too, so both are compared:
/// the title’s default by values matches the “typewriter”
/// (the same size and the same middle of the frame), and
/// without the second half of the comparison a new empty title
/// would be declared typed.
pub fn active_preset(kind: &GeneratorKind, curves: &[kt_model::ParamCurve]) -> Option<usize> {
    title_presets().iter().position(|p| p.applied(kind) == *kind && anim_matches(p.anim, curves))
}

/// Whether the SET of animated rows is the one the preset sets. The key values are deliberately not compared:
/// the scrolling-text path is computed from the text layout,
/// and the person changes the text — otherwise the preset
/// would “fall off” its own result at the first letter.
fn anim_matches(anim: kt_model::TextAnim, curves: &[kt_model::ParamCurve]) -> bool {
    use kt_model::TextAnim as A;
    let want: &[&str] = match anim {
        A::None => &[],
        A::Typewriter | A::Fade => &["reveal"],
        A::Crawl => &["offset_x"],
        A::Roll => &["offset_y"],
    };
    let mut have: Vec<&str> = curves.iter().map(|c| c.param.as_str()).collect();
    have.sort_unstable();
    have == want
}

/// The order of the title’s layers (R.53, item 415). The `order`
/// field lives in the format since R.52 — a lesson from Kdenlive,
/// where the order was a constant and redrew other people’s projects — but there was no editor for it: there was nothing
/// to swap the fill and the outline with. Four slot buttons: a click swaps places with the neighbour; the caption says
/// that the top of the list is drawn LAST.
fn order_row(ui: &mut egui::Ui, g: &mut GeneratorKind) -> bool {
    let GeneratorKind::Text { order, .. } = g else { return false };
    let name = |l: kt_model::TextLayer| match l {
        kt_model::TextLayer::Plate => tr(k::titlew::PLATE),
        kt_model::TextLayer::Shadow => tr(k::titlew::SHADOW),
        kt_model::TextLayer::Stroke => tr(k::titlew::STROKE),
        kt_model::TextLayer::Fill => tr(k::titlew::FILL),
    };
    let mut changed = false;
    ui.horizontal(|ui| {
        crate::widgets::hint(&ui.label(tr(k::titlew::ORDER)), tr(k::titlew::HINT_ORDER));
        for i in 0..order.len() {
            let r = ui.small_button(name(order[i]));
            crate::widgets::field_probe::note(&format!("titlew.order.{i}"), r.rect);
            // a click — “raise above the neighbour”: a swap without a drag, as with the chain slots
            // (rule 1: hit it with the mouse the first time)
            if r.clicked() && i + 1 < order.len() {
                order.swap(i, i + 1);
                changed = true;
            }
        }
    });
    changed
}

/// What the generator form edited during the frame.
///
/// The split is the same as in `widgets::ParamsChanged`: the
/// base goes by `SetClipGen`, the curve — `SetClipGenAuto`.
/// The rest are gestures, each with its OWN history step (rule 20): the kind change and the preset.
#[derive(Default)]
pub struct GenEdit {
    pub changed: widgets::ParamsChanged,
    /// Change the generator KIND (a property of the media, its own history step).
    pub switch_kind: Option<GeneratorKind>,
    /// A preset was pressed: both values and curves, in one history step of its own.
    pub preset: bool,
    /// “Apply to all (×N)”.
    pub apply_all: bool,
    /// Where to move the main cursor (a gesture on the graph’s mini-timeline).
    pub seek: Option<kt_time::Tick>,
    pub axis: crate::automation::AxisReq,
}

/// What the form needs to know about the place where it is drawn.
pub struct GenCtx<'a> {
    /// The owner event: the curve domain is its material.
    pub clip: &'a kt_model::ClipEvent,
    /// The PUBLICATION context (R.59, stage 3): the generator field captions are the same door outward
    /// as the effect parameters. This is exactly where
    /// the “news headline” from the owner’s claim lives.
    pub pubc: Option<&'a crate::widgets::PublishCtx>,
    /// How many places on the timeline this generator has (“apply to all”).
    pub uses: usize,
    /// The project frame size — the scrolling-text path is computed from it.
    pub frame: (u32, u32),
    pub frame_step: i64,
    pub playhead: kt_time::Tick,
    /// The window of the MAIN timeline (the graph axis lock).
    pub tl_view: (kt_time::Tick, kt_time::Tick),
    pub axis_linked: bool,
    /// The segment shape of new curves — a PROJECT field (R.61).
    pub default_shape: kt_model::SegShape,
    /// The link targets “Is driven…” (R.67) — from a window that has a document.
    pub links: Option<&'a dyn Fn() -> Vec<crate::automation::LinkEvent>>,
}

/// The generator form: kind, values, curves — in ONE body for all entries.
///
/// It is drawn in the event’s effects window when the pinned head of the strip is selected. The generator and the title
/// no longer have a window of their own: the owner asked to see
/// the source as a crumb BEFORE the effects, in the same window.
#[allow(clippy::too_many_lines)]
pub fn draw_gen_section(
    ui: &mut egui::Ui,
    ctx: &GenCtx<'_>,
    kind: &mut GeneratorKind,
    auto: &mut Vec<kt_model::ParamCurve>,
    view: &mut crate::automation::AutoView,
    warn: &mut GenWarn,
    hover: &mut Option<String>,
) -> GenEdit {
    let mut out = GenEdit::default();
    let clip = ctx.clip;

    // ---- kind: the same picker that was in both former windows -------------
    ui.horizontal(|ui| {
        widgets::hint(&ui.label(tr(k::trn::KIND)), tr(k::genw::HINT_KIND));
        egui::ComboBox::from_id_salt(("genkind", clip.id.0))
            .selected_text(kind_name(kind))
            .show_ui(ui, |ui| {
                for kd in GeneratorKind::all() {
                    let same = kd.tag() == kind.tag();
                    let r = ui.selectable_label(same, kind_name(&kd));
                    crate::widgets::field_probe::note(&format!("genw.kind.{}", kd.tag()), r.rect);
                    if r.clicked() && !same {
                        // changing the kind = its defaults: the parameters of
                        // the kinds differ, so there is nothing to carry over
                        out.switch_kind = Some(kd);
                    }
                }
            });
    });
    if out.switch_kind.is_some() {
        // the kind goes away entirely — there is no point drawing the old form any further
        return out;
    }

    // ---- title: row, warnings, presets -------------------------
    if matches!(kind, GeneratorKind::Text { .. }) {
        {
            let GeneratorKind::Text { text, .. } = kind else { unreachable!() };
            let te = egui::TextEdit::multiline(text)
                .id(egui::Id::new(("title-text", clip.id.0)))
                .desired_rows(3)
                .desired_width(f32::INFINITY);
            let r = ui.add(te);
            crate::widgets::field_probe::note("titlew.text", r.rect);
            widgets::hint(&r, tr(k::titlew::HINT_TEXT));
            // PUBLISHING THE HEADLINE — the very reason the whole round
            // was started (“people will not climb into the composition
            // every time and change the text there”). A multi-line title
            // field has no caption, and one must not add one for a single
            // label (rule 13), so the door hangs on the field itself — right where the gesture is.
            widgets::publish_on_field(
                &r,
                ctx.pubc,
                "text",
                // the handle’s DEFAULT name — a short property caption,
                // not a hint phrase: the author edits it further on,
                // but it should not start with a sentence
                tr(k::genp::TEXT),
                kt_model::ParamType::Text { multiline: true },
            );
            if r.changed() {
                out.changed.base = true;
            }
        }
        warn.refresh(kind);
        for w in [&warn.missing, &warn.font].into_iter().flatten() {
            ui.colored_label(egui::Color32::from_rgb(255, 105, 97), w);
        }
        // presets live ABOVE the rest of the form: they are reached for first
        let active = active_preset(kind, auto);
        let mut pick: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(tr(k::titlew::PRESETS));
            for (i, pr) in title_presets().iter().enumerate() {
                let r = ui.selectable_label(active == Some(i), tr(pr.key));
                crate::widgets::field_probe::note(&format!("titlew.preset.{i}"), r.rect);
                widgets::hint(&r, tr(k::titlew::HINT_PRESET));
                if r.clicked() {
                    pick = Some(i);
                }
            }
            if active.is_none() {
                // an honest answer instead of highlighting a button to which the title no longer has any relation
                // (the owner’s decision)
                ui.weak(tr(k::titlew::PRESET_NONE));
            }
        });
        if let Some(i) = pick {
            let pr = &title_presets()[i];
            let made = pr.applied(kind);
            *auto = pr.curves(&made, ctx.frame, clip.source_in);
            *kind = made;
            out.preset = true;
            out.changed.base = true;
            out.changed.curve = true;
            // choosing a key on the graph became meaningless: the curves are different
            view.sel = None;
            view.active = 0;
        }
    }

    // ---- colours: Rgb8 triples — with eyedroppers (rule 13) -------------------
    let groups = color_groups(kind);
    let alpha = color_alpha(kind);
    let mut skip: Vec<&str> = Vec::new();
    for (_, names) in groups {
        skip.extend_from_slice(names);
    }
    for (i, (label, names)) in groups.iter().enumerate() {
        if widgets::color_row(
            ui,
            tr(*label),
            kind,
            *names,
            alpha,
            (i == 0).then_some("genw.color"),
            ctx.pubc,
        ) {
            out.changed.base = true;
        }
    }

    // ---- title: family, style, anchor, layer colours, order ----------
    if matches!(kind, GeneratorKind::Text { .. }) {
        out.changed.base |= draw_title_extras(ui, clip.id, kind);
    }

    // the timecode shows the time of the MATERIAL — say so directly
    if matches!(kind, GeneratorKind::Timecode { .. }) {
        ui.weak(tr(k::genw::TC_NOTE));
    }
    // the kind’s FLAGS — one widget for all forms (R.53, item 414)
    if widgets::flags_row(ui, ("genf", clip.id.0), kind) {
        out.changed.base = true;
    }

    // ---- numbers: the SAME grid as the effects, and with the link to curves -----
    // (R.59: each numeric row of the generator gets both a field and a curve)
    let at = kt_model::clip_material_at(
        clip.source_in,
        clip.start,
        clip.duration,
        clip.reversed,
        ctx.frame_step,
        ctx.playhead,
    );
    {
        let mut bind = widgets::CurveBind { curves: auto, sel: view.sel, at };
        out.changed |= widgets::params_grid_bound(
            ui,
            ("genp", clip.id.0),
            kind,
            &skip,
            Some(&mut bind),
            ctx.pubc,
        );
    }

    // ---- “apply to all”: a separate command, one history step ------
    // The places counter is IN THE BUTTON ITSELF; we
    // do not make a separate line under it (rule 13):
    // “places: N” next to “apply to all (×N)” would be the same number twice.
    ui.horizontal(|ui| {
        let r = crate::app::hint_into(
            hover,
            ui.add_enabled(
                ctx.uses > 1,
                egui::Button::new(crate::tf!(k::genw::APPLY_ALL, ctx.uses)),
            ),
            tr(k::genw::APPLY_ALL_HINT),
        );
        crate::widgets::field_probe::note("genw.apply_all", r.rect);
        if r.clicked() {
            out.apply_all = true;
        }
    });

    // ---- curves: the SAME widget as the effect slots have ------------------
    let r = draw_gen_auto(ui, ctx, kind, auto, view, hover);
    out.changed.curve |= r.changed;
    out.seek = r.seek;
    out.axis = out.axis.merge(&r);
    out
}

/// The parts of the title form that are not expressed by
/// a number (“the first legitimate exception” F-TXT-001):
/// family, style, alignment, anchor, layer colours and the drawing order.
fn draw_title_extras(ui: &mut egui::Ui, id: kt_model::ClipId, g: &mut GeneratorKind) -> bool {
    let mut changed = false;
    {
        let GeneratorKind::Text { family, .. } = g else { return false };
        ui.horizontal(|ui| {
            widgets::hint(&ui.label(tr(k::titlew::FAMILY)), tr(k::titlew::HINT_FAMILY));
            egui::ComboBox::from_id_salt(("title-family", id.0))
                .width(190.0)
                .selected_text(crate::widgets::elided_name(
                    ui,
                    family,
                    ui.visuals().text_color(),
                    false,
                ))
                .show_ui(ui, |ui| {
                    let vend = kt_text::VENDORED_FAMILY;
                    if ui.selectable_label(family == vend, vend).clicked() && family != vend {
                        *family = vend.to_string();
                        changed = true;
                    }
                    ui.separator();
                    for f in kt_text::system_families() {
                        if ui.selectable_label(family == f, f).clicked() && family != f {
                            family.clone_from(f);
                            changed = true;
                        }
                    }
                });
        });
    }

    let mut center = false;
    {
        let GeneratorKind::Text { bold, align, anchor, .. } = g else { unreachable!() };
        ui.horizontal(|ui| {
            // “bold” — a passport flag (R.53, item 414), but
            // it is drawn right here, next to the alignment
            changed |= ui.checkbox(bold, tr(k::titlew::BOLD)).changed();
            ui.separator();
            for (a, label) in [
                (TextAlign::Left, tr(k::titlew::ALIGN_L)),
                (TextAlign::Center, tr(k::titlew::ALIGN_C)),
                (TextAlign::Right, tr(k::titlew::ALIGN_R)),
            ] {
                if ui.selectable_label(*align == a, label).clicked() && *align != a {
                    *align = a;
                    changed = true;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr(k::titlew::ANCHOR));
            egui::Grid::new(("title-anchor", id.0)).spacing([2.0, 2.0]).show(ui, |ui| {
                use TextAnchor as A;
                for anchors in [
                    [A::TopLeft, A::TopCenter, A::TopRight],
                    [A::CenterLeft, A::Center, A::CenterRight],
                    [A::BottomLeft, A::BottomCenter, A::BottomRight],
                ] {
                    for a in anchors {
                        let on = *anchor == a;
                        if ui.add_sized([18.0, 14.0], egui::Button::selectable(on, "•")).clicked()
                            && !on
                        {
                            *anchor = a;
                            changed = true;
                        }
                    }
                    ui.end_row();
                }
            });
            // “centre” — one gesture instead of three fields (item 415)
            let r = ui.button(tr(k::titlew::CENTER));
            widgets::hint(&r, tr(k::titlew::HINT_CENTER));
            crate::widgets::field_probe::note("titlew.center", r.rect);
            if r.clicked() {
                *anchor = TextAnchor::Center;
                center = true;
                changed = true;
            }
        });
    }
    if center {
        g.set_param("pos_x", 0.5);
        g.set_param("pos_y", 0.5);
    }

    // layer colours: RGBA eyedropper + HEX. The title colours are DIRECT fields, not a `ParamSpec`,
    // so the widget is its own too: a path through `get_param` would silently return zero, and the fill
    // would become transparent black (item 415).
    {
        let GeneratorKind::Text {
            fill_r,
            fill_g,
            fill_b,
            fill_a,
            stroke_r,
            stroke_g,
            stroke_b,
            stroke_a,
            shadow_r,
            shadow_g,
            shadow_b,
            shadow_a,
            plate_r,
            plate_g,
            plate_b,
            plate_a,
            ..
        } = g
        else {
            unreachable!()
        };
        changed |= widgets::color_row_rgba(
            ui,
            tr(k::titlew::FILL),
            (fill_r, fill_g, fill_b, fill_a),
            Some("titlew.fill"),
        );
        changed |= widgets::color_row_rgba(
            ui,
            tr(k::titlew::STROKE),
            (stroke_r, stroke_g, stroke_b, stroke_a),
            Some("titlew.stroke"),
        );
        changed |= widgets::color_row_rgba(
            ui,
            tr(k::titlew::SHADOW),
            (shadow_r, shadow_g, shadow_b, shadow_a),
            Some("titlew.shadow"),
        );
        changed |= widgets::color_row_rgba(
            ui,
            tr(k::titlew::PLATE),
            (plate_r, plate_g, plate_b, plate_a),
            Some("titlew.plate"),
        );
    }
    changed |= order_row(ui, g);
    changed
}

/// The generator’s automation section — a mirror of
/// `fx_editor::draw_auto_for` for the second carrier of curves.
///
/// The widget itself is one (`automation::draw`, the rule
/// “no one should have their own keyframe editor”); here
/// only what concerns the curve as a property of the source:
/// the list of free parameters, the form of all segments and
/// removing the curve. The domain is the event’s material —
/// exactly the `pts` by which the curves are resolved by the
/// SOURCE (`GenState::kind_at`): if they diverged, the cache key would stop determining the frame.
fn draw_gen_auto(
    ui: &mut egui::Ui,
    ctx: &GenCtx<'_>,
    kind: &GeneratorKind,
    curves: &mut Vec<kt_model::ParamCurve>,
    view: &mut crate::automation::AutoView,
    hover: &mut Option<String>,
) -> crate::automation::AutoResult {
    let specs = kind.param_specs();
    if specs.is_empty() {
        return crate::automation::AutoResult::default();
    }
    let clip = ctx.clip;
    let src_in = clip.source_in;
    let mat_end = src_in.0 + clip.duration.0.max(1);
    let mut changed = false;
    ui.separator();
    ui.horizontal(|ui| {
        let free: Vec<&kt_model::ParamSpec> =
            specs.iter().filter(|s| !curves.iter().any(|c| c.param == s.name)).collect();
        if !free.is_empty() {
            ui.menu_button(tr(k::fx::AUTO_ADD), |ui| {
                for spec in &free {
                    let n: &str = &spec.name;
                    let label = crate::widgets::param_label_scoped(kind.params_scope(), n);
                    if ui.button(label).clicked() {
                        let v0 = kind.get_param(n).unwrap_or(spec.default);
                        curves.push(kt_model::ParamCurve::constant(
                            n,
                            src_in,
                            kt_time::Tick(mat_end),
                            v0,
                            ctx.default_shape,
                        ));
                        view.active = curves.len() - 1;
                        view.sel = None;
                        changed = true;
                    }
                }
            });
        }
        // the form of all segments, the formula, link and
        // delete — in the curve block of the widget itself (R.67)
    });
    let frame_step = ctx.frame_step;
    let to_tl = |t: kt_time::Tick| {
        kt_model::clip_timeline_at(
            clip.source_in,
            clip.start,
            clip.duration,
            clip.reversed,
            frame_step,
            t,
        )
    };
    let to_mat = |t: kt_time::Tick| {
        kt_model::clip_material_at(
            clip.source_in,
            clip.start,
            clip.duration,
            clip.reversed,
            frame_step,
            t,
        )
    };
    let actx = crate::automation::AutoCtx {
        links: ctx.links,
        specs,
        range: (src_in, kt_time::Tick(mat_end)),
        view: crate::automation::view_in_curve(&to_mat, ctx.tl_view),
        to_timeline: &to_tl,
        to_curve: &to_mat,
        frame_step,
        linked: ctx.axis_linked,
        default_shape: ctx.default_shape,
    };
    let mut res = crate::automation::draw(
        ui,
        ("genauto", clip.id.0),
        &actx,
        curves,
        view,
        ctx.playhead,
        hover,
    );
    res.changed |= changed;
    res
}

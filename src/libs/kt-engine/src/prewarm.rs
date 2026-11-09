//! What a project reaches on the GPU (pipeline-prewarm D5): the ids of every effect in its chains —
//! clips, tracks, morpher patches, compositions — and of its generator kinds. The player hands the
//! new ones to the executor's `prewarm` and the compute host's, so an effect's first frame compiles
//! nothing. A walk of the model: no allocation per element (the tags are `&'static str`).

use std::collections::BTreeSet;

use kt_model::{ClipEvent, FxInstance, MorpherEvent, Project, Track};

fn chain(out: &mut BTreeSet<&'static str>, fx: &[FxInstance]) {
    out.extend(fx.iter().map(FxInstance::tag));
}

fn lanes(out: &mut BTreeSet<&'static str>, clips: &[ClipEvent], tracks: &[Track], morphers: &[MorpherEvent]) {
    for c in clips {
        chain(out, &c.fx);
        if let Some(g) = &c.r#gen {
            out.insert(g.tag());
        }
    }
    for t in tracks {
        chain(out, &t.fx);
    }
    for m in morphers {
        chain(out, &m.patch.video_fx);
    }
}

/// What a project reaches on the GPU: its effect and generator ids and its point-wise run shapes. The
/// player asks the executor again only when it changed.
#[derive(Default, PartialEq, Eq, Debug)]
pub struct Reach {
    pub ids: BTreeSet<&'static str>,
    pub runs: BTreeSet<Vec<&'static str>>,
}

impl Reach {
    /// The ids and the run shapes as the executor takes them.
    pub fn lists(&self) -> (Vec<String>, Vec<Vec<String>>) {
        (self.ids.iter().map(|s| s.to_string()).collect(), run_list(&self.runs))
    }
}

/// The effect and generator ids `p` reaches, its compositions included.
pub fn effect_ids(p: &Project) -> BTreeSet<&'static str> {
    let mut out = BTreeSet::new();
    lanes(&mut out, &p.clips, &p.tracks, &p.morphers);
    for s in &p.sequences {
        lanes(&mut out, &s.clips, &s.tracks, &s.morphers);
    }
    out
}

/// The point-wise run shapes `p` reaches (fused-pointwise-runs): each run of two or more fusable
/// stages in a chain, as plugin ids in chain order (`kt_fx::plan::run_shapes`). The executor compiles
/// their generated passes beside, so a run's first frame does not run as its parts.
pub fn run_shapes(p: &Project) -> BTreeSet<Vec<&'static str>> {
    fn lanes(out: &mut BTreeSet<Vec<&'static str>>, clips: &[ClipEvent], tracks: &[Track], morphers: &[MorpherEvent]) {
        let chains = clips.iter().map(|c| &c.fx).chain(tracks.iter().map(|t| &t.fx)).chain(morphers.iter().map(|m| &m.patch.video_fx));
        for fx in chains {
            out.extend(kt_fx::plan::run_shapes(fx));
        }
    }
    let mut out = BTreeSet::new();
    lanes(&mut out, &p.clips, &p.tracks, &p.morphers);
    for s in &p.sequences {
        lanes(&mut out, &s.clips, &s.tracks, &s.morphers);
    }
    out
}

/// The run shapes as the executor takes them.
pub fn run_list(runs: &BTreeSet<Vec<&'static str>>) -> Vec<Vec<String>> {
    runs.iter().map(|r| r.iter().map(|s| s.to_string()).collect()).collect()
}

/// Prepare on the compute host the shaders the plugins `ids` name (`Plugin::compute_shaders`).
/// Returns at once; nothing happens without an installed host or such a plugin.
pub fn compute(ids: &[String]) {
    let Some(reg) = kt_plugin_api::installed() else { return };
    let shaders: Vec<_> = reg
        .plugins()
        .filter(|p| ids.iter().any(|id| *id == *p.manifest().id))
        .flat_map(|p| p.compute_shaders())
        .collect();
    if !shaders.is_empty() {
        kt_plugin_api::compute::prewarm(shaders);
    }
}
